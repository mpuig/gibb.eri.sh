mod loader;
mod nemo_ctc;
mod whisper;
mod worker;

pub use loader::{SherpaNemoCtcLoader, SherpaWhisperLoader, SherpaZipformerLoader};
pub use nemo_ctc::SherpaNemoCtcEngine;
pub use whisper::SherpaWhisperEngine;
pub use worker::{InferenceRequest, InferenceResult, SherpaWorker};

use gibberish_stt::{Segment, SttEngine, Word};
use std::any::Any;
use std::ffi::{CStr, CString};
use std::path::Path;
use std::ptr;
use std::sync::{Arc, Mutex};

/// Arc-wrapped handle to the ONNX recognizer.
///
/// This ensures the C++ recognizer is not destroyed until all references
/// (both SherpaStreamingEngine and SherpaWorker) are dropped.
/// Prevents use-after-free when switching models.
#[derive(Debug)]
pub struct RecognizerHandle {
    ptr: *const sherpa_rs_sys::SherpaOnnxOnlineRecognizer,
}

impl RecognizerHandle {
    /// Get the raw pointer for FFI calls.
    pub fn ptr(&self) -> *const sherpa_rs_sys::SherpaOnnxOnlineRecognizer {
        self.ptr
    }
}

// Safety: The recognizer is thread-safe for inference operations.
unsafe impl Send for RecognizerHandle {}
unsafe impl Sync for RecognizerHandle {}

impl Drop for RecognizerHandle {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                sherpa_rs_sys::SherpaOnnxDestroyOnlineRecognizer(self.ptr);
            }
            tracing::debug!("Destroyed Sherpa recognizer");
        }
    }
}

/// Latency profile for decoder endpointing.
/// Controls how quickly the decoder commits transcription after silence.
#[derive(Debug, Clone, Copy, Default)]
pub enum LatencyProfile {
    /// Hyper-responsive (0.2s silence). For fast speakers, may cut off words.
    HyperResponsive,
    /// Fast commits (0.3s silence). Best for dictation.
    Fast,
    /// Balanced (0.4s silence). Default, good for most uses.
    #[default]
    Balanced,
    /// Accurate (0.8s silence). Better sentence structure, higher latency.
    Accurate,
}

impl LatencyProfile {
    /// Get rule2 trailing silence (quick commits after short utterances).
    pub fn rule2_silence(&self) -> f32 {
        match self {
            LatencyProfile::HyperResponsive => 0.2,
            LatencyProfile::Fast => 0.3,
            LatencyProfile::Balanced => 0.4,
            LatencyProfile::Accurate => 0.8,
        }
    }

    /// Get rule1 trailing silence (longer silence for longer utterances).
    pub fn rule1_silence(&self) -> f32 {
        match self {
            LatencyProfile::HyperResponsive => 1.2,
            LatencyProfile::Fast => 1.8,
            LatencyProfile::Balanced => 2.4,
            LatencyProfile::Accurate => 3.0,
        }
    }

    /// Get minimum utterance length for rule3.
    pub fn min_utterance(&self) -> f32 {
        match self {
            LatencyProfile::HyperResponsive => 0.3,
            LatencyProfile::Fast => 0.5,
            LatencyProfile::Balanced => 0.8,
            LatencyProfile::Accurate => 1.2,
        }
    }
}

/// Get the optimal ONNX execution provider for the current platform.
///
/// Note: CoreML requires models to be compiled with CoreML support.
/// Most pre-built sherpa-onnx models only support CPU provider.
/// We default to "cpu" for compatibility.
fn get_optimal_provider() -> &'static str {
    // CoreML would require specially compiled models, so we use CPU for now.
    // To enable CoreML: compile models with --provider=coreml flag
    "cpu"
}

#[derive(Debug, thiserror::Error)]
pub enum SherpaError {
    #[error("model files not found: {0}")]
    MissingFiles(String),
    #[error("load failed: {0}")]
    LoadFailed(String),
    #[error("transcription failed: {0}")]
    TranscriptionFailed(String),
}

pub type Result<T> = std::result::Result<T, SherpaError>;

/// Internal streaming state for the engine's own stream.
struct StreamingState {
    /// Arc-wrapped recognizer handle (shared with workers).
    recognizer: Arc<RecognizerHandle>,
    /// Stream pointer (owned by this engine instance).
    stream: *const sherpa_rs_sys::SherpaOnnxOnlineStream,
    committed_text: String,
    last_partial: String,
    total_samples: u64,
    buffer_duration_ms: u64,
    last_committed_delta: Option<String>,
}

// Safety: Stream is only accessed from one thread at a time (via Mutex).
// Recognizer is thread-safe via Arc<RecognizerHandle>.
unsafe impl Send for StreamingState {}

#[derive(Debug)]
struct ModelStrings {
    encoder: CString,
    decoder: CString,
    joiner: CString,
    tokens: CString,
    provider: CString,
    decoding_method: CString,
}

pub struct SherpaStreamingEngine {
    model_name: String,
    #[allow(dead_code)]
    latency_profile: LatencyProfile,
    #[allow(dead_code)]
    strings: ModelStrings,
    state: Mutex<StreamingState>,
}

impl SherpaStreamingEngine {
    pub fn new_zipformer_transducer(model_dir: impl AsRef<Path>) -> Result<Self> {
        Self::new_zipformer_transducer_with_profile(model_dir, LatencyProfile::default())
    }

    pub fn new_zipformer_transducer_with_profile(
        model_dir: impl AsRef<Path>,
        latency_profile: LatencyProfile,
    ) -> Result<Self> {
        let model_dir = model_dir.as_ref();

        let encoder = model_dir.join("encoder.onnx");
        let decoder = model_dir.join("decoder.onnx");
        let joiner = model_dir.join("joiner.onnx");
        let tokens = model_dir.join("tokens.txt");

        for p in [&encoder, &decoder, &joiner, &tokens] {
            if !p.exists() {
                return Err(SherpaError::MissingFiles(format!(
                    "expected {}",
                    p.display()
                )));
            }
        }

        let strings =
            ModelStrings {
                encoder: CString::new(encoder.to_str().ok_or_else(|| {
                    SherpaError::MissingFiles("invalid encoder path".to_string())
                })?)
                .map_err(|e| SherpaError::LoadFailed(e.to_string()))?,
                decoder: CString::new(decoder.to_str().ok_or_else(|| {
                    SherpaError::MissingFiles("invalid decoder path".to_string())
                })?)
                .map_err(|e| SherpaError::LoadFailed(e.to_string()))?,
                joiner: CString::new(
                    joiner.to_str().ok_or_else(|| {
                        SherpaError::MissingFiles("invalid joiner path".to_string())
                    })?,
                )
                .map_err(|e| SherpaError::LoadFailed(e.to_string()))?,
                tokens: CString::new(
                    tokens.to_str().ok_or_else(|| {
                        SherpaError::MissingFiles("invalid tokens path".to_string())
                    })?,
                )
                .map_err(|e| SherpaError::LoadFailed(e.to_string()))?,
                provider: CString::new(get_optimal_provider()).expect("valid cstring"),
                decoding_method: CString::new("greedy_search").expect("valid cstring"),
            };

        tracing::info!(
            provider = get_optimal_provider(),
            rule2_silence = latency_profile.rule2_silence(),
            "Initializing Sherpa recognizer"
        );

        // Create the recognizer and wrap in Arc for shared ownership with workers
        let recognizer_handle = unsafe {
            let config = sherpa_rs_sys::SherpaOnnxOnlineRecognizerConfig {
                feat_config: sherpa_rs_sys::SherpaOnnxFeatureConfig {
                    sample_rate: 16000,
                    feature_dim: 80,
                },
                model_config: sherpa_rs_sys::SherpaOnnxOnlineModelConfig {
                    transducer: sherpa_rs_sys::SherpaOnnxOnlineTransducerModelConfig {
                        encoder: strings.encoder.as_ptr(),
                        decoder: strings.decoder.as_ptr(),
                        joiner: strings.joiner.as_ptr(),
                    },
                    tokens: strings.tokens.as_ptr(),
                    num_threads: 2,
                    debug: 0,
                    provider: strings.provider.as_ptr(),

                    // Unused model types
                    paraformer: std::mem::zeroed::<_>(),
                    zipformer2_ctc: std::mem::zeroed::<_>(),
                    model_type: std::mem::zeroed::<_>(),
                    modeling_unit: std::mem::zeroed::<_>(),
                    bpe_vocab: std::mem::zeroed::<_>(),
                    tokens_buf: std::mem::zeroed::<_>(),
                    tokens_buf_size: std::mem::zeroed::<_>(),
                    nemo_ctc: std::mem::zeroed::<_>(),
                },
                decoding_method: strings.decoding_method.as_ptr(),
                max_active_paths: 0,
                enable_endpoint: 1,
                rule1_min_trailing_silence: latency_profile.rule1_silence(),
                rule2_min_trailing_silence: latency_profile.rule2_silence(),
                rule3_min_utterance_length: latency_profile.min_utterance(),

                hotwords_file: ptr::null(),
                hotwords_score: 0.0,
                ctc_fst_decoder_config: std::mem::zeroed::<_>(),
                rule_fsts: ptr::null(),
                rule_fars: ptr::null(),
                blank_penalty: 0.0,

                hotwords_buf: ptr::null(),
                hotwords_buf_size: 0,
                hr: std::mem::zeroed::<_>(),
            };

            let recognizer_ptr = sherpa_rs_sys::SherpaOnnxCreateOnlineRecognizer(&config);
            if recognizer_ptr.is_null() {
                return Err(SherpaError::LoadFailed(
                    "SherpaOnnxCreateOnlineRecognizer failed".to_string(),
                ));
            }
            Arc::new(RecognizerHandle {
                ptr: recognizer_ptr,
            })
        };

        let stream =
            unsafe { sherpa_rs_sys::SherpaOnnxCreateOnlineStream(recognizer_handle.ptr()) };
        if stream.is_null() {
            // RecognizerHandle will be dropped here, destroying the recognizer
            return Err(SherpaError::LoadFailed(
                "SherpaOnnxCreateOnlineStream failed".to_string(),
            ));
        }

        let model_name = model_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("sherpa-zipformer")
            .to_string();

        Ok(Self {
            model_name,
            latency_profile,
            strings,
            state: Mutex::new(StreamingState {
                recognizer: recognizer_handle,
                stream,
                committed_text: String::new(),
                last_partial: String::new(),
                total_samples: 0,
                buffer_duration_ms: 0,
                last_committed_delta: None,
            }),
        })
    }

    pub fn reset_streaming(&self) {
        if let Ok(mut guard) = self.state.lock() {
            unsafe {
                sherpa_rs_sys::SherpaOnnxOnlineStreamReset(guard.recognizer.ptr(), guard.stream);
            }
            guard.committed_text.clear();
            guard.last_partial.clear();
            guard.total_samples = 0;
            guard.buffer_duration_ms = 0;
            guard.last_committed_delta = None;
        }
    }

    pub fn buffer_duration_ms(&self) -> u64 {
        self.state
            .lock()
            .ok()
            .map(|g| g.buffer_duration_ms)
            .unwrap_or(0)
    }

    pub fn take_last_committed_delta(&self) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|mut g| g.last_committed_delta.take())
    }

    /// Create a non-blocking worker for this engine.
    ///
    /// The worker runs inference on a dedicated thread, preventing mutex
    /// contention during decoding. Audio chunks are sent via channels.
    ///
    /// Note: This creates a new stream that shares the recognizer (via Arc).
    /// The recognizer stays alive as long as either engine or worker exists.
    pub fn create_worker(&self) -> Result<SherpaWorker> {
        let guard = self
            .state
            .lock()
            .map_err(|_| SherpaError::LoadFailed("lock poisoned".to_string()))?;

        let stream = unsafe { sherpa_rs_sys::SherpaOnnxCreateOnlineStream(guard.recognizer.ptr()) };
        if stream.is_null() {
            return Err(SherpaError::LoadFailed(
                "SherpaOnnxCreateOnlineStream failed".to_string(),
            ));
        }

        // Clone the Arc so the worker keeps the recognizer alive
        let recognizer_handle = Arc::clone(&guard.recognizer);
        Ok(SherpaWorker::new(
            recognizer_handle,
            stream,
            self.model_name.clone(),
        ))
    }

    pub fn accept_streaming_chunk(
        &self,
        sample_rate: u32,
        audio_chunk: &[f32],
    ) -> Result<(String, String, bool, u64)> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| SherpaError::TranscriptionFailed("lock poisoned".to_string()))?;

        guard.total_samples = guard.total_samples.saturating_add(audio_chunk.len() as u64);

        unsafe {
            sherpa_rs_sys::SherpaOnnxOnlineStreamAcceptWaveform(
                guard.stream,
                sample_rate as i32,
                audio_chunk.as_ptr(),
                audio_chunk.len() as i32,
            );

            while sherpa_rs_sys::SherpaOnnxIsOnlineStreamReady(guard.recognizer.ptr(), guard.stream)
                == 1
            {
                sherpa_rs_sys::SherpaOnnxDecodeOnlineStream(guard.recognizer.ptr(), guard.stream);
            }
        }

        let partial = unsafe {
            let result_ptr = sherpa_rs_sys::SherpaOnnxGetOnlineStreamResult(
                guard.recognizer.ptr(),
                guard.stream,
            );
            if result_ptr.is_null() {
                return Err(SherpaError::TranscriptionFailed(
                    "SherpaOnnxGetOnlineStreamResult returned NULL".to_string(),
                ));
            }
            let text = if (*result_ptr).text.is_null() {
                String::new()
            } else {
                CStr::from_ptr((*result_ptr).text)
                    .to_string_lossy()
                    .to_string()
            };
            sherpa_rs_sys::SherpaOnnxDestroyOnlineRecognizerResult(result_ptr);
            text
        };

        let is_endpoint = unsafe {
            sherpa_rs_sys::SherpaOnnxOnlineStreamIsEndpoint(guard.recognizer.ptr(), guard.stream)
                == 1
        };

        if is_endpoint && !partial.trim().is_empty() {
            // Commit at endpoint. This gives the lowest jitter and maps well to "utterance commits".
            let delta = partial.trim().to_string();
            guard.last_committed_delta = Some(delta);
            if !guard.committed_text.is_empty() {
                guard.committed_text.push(' ');
            }
            guard.committed_text.push_str(partial.trim());

            guard.last_partial.clear();
            guard.total_samples = 0;
            guard.buffer_duration_ms = 0;

            unsafe {
                sherpa_rs_sys::SherpaOnnxOnlineStreamReset(guard.recognizer.ptr(), guard.stream);
            }
        } else {
            guard.last_partial = partial.clone();
        }

        let buffer_duration_ms =
            (guard.total_samples as f64 / sample_rate as f64 * 1000.0).round() as u64;
        guard.buffer_duration_ms = buffer_duration_ms;
        Ok((
            guard.committed_text.clone(),
            guard.last_partial.clone(),
            !is_endpoint,
            buffer_duration_ms,
        ))
    }

    fn transcribe_offline_via_online_api(&self, audio: &[f32]) -> Result<String> {
        let guard = self
            .state
            .lock()
            .map_err(|_| SherpaError::TranscriptionFailed("lock poisoned".to_string()))?;

        let stream = unsafe { sherpa_rs_sys::SherpaOnnxCreateOnlineStream(guard.recognizer.ptr()) };
        if stream.is_null() {
            return Err(SherpaError::TranscriptionFailed(
                "SherpaOnnxCreateOnlineStream returned NULL".to_string(),
            ));
        }

        let chunk = 3200; // ~200ms at 16kHz
        for window in audio.chunks(chunk) {
            unsafe {
                sherpa_rs_sys::SherpaOnnxOnlineStreamAcceptWaveform(
                    stream,
                    16000,
                    window.as_ptr(),
                    window.len() as i32,
                );
                while sherpa_rs_sys::SherpaOnnxIsOnlineStreamReady(guard.recognizer.ptr(), stream)
                    == 1
                {
                    sherpa_rs_sys::SherpaOnnxDecodeOnlineStream(guard.recognizer.ptr(), stream);
                }
            }
        }

        unsafe {
            sherpa_rs_sys::SherpaOnnxOnlineStreamInputFinished(stream);
            while sherpa_rs_sys::SherpaOnnxIsOnlineStreamReady(guard.recognizer.ptr(), stream) == 1
            {
                sherpa_rs_sys::SherpaOnnxDecodeOnlineStream(guard.recognizer.ptr(), stream);
            }
        }

        let text = unsafe {
            let result_ptr =
                sherpa_rs_sys::SherpaOnnxGetOnlineStreamResult(guard.recognizer.ptr(), stream);
            if result_ptr.is_null() {
                sherpa_rs_sys::SherpaOnnxDestroyOnlineStream(stream);
                return Err(SherpaError::TranscriptionFailed(
                    "SherpaOnnxGetOnlineStreamResult returned NULL".to_string(),
                ));
            }
            let text = if (*result_ptr).text.is_null() {
                String::new()
            } else {
                CStr::from_ptr((*result_ptr).text)
                    .to_string_lossy()
                    .to_string()
            };
            sherpa_rs_sys::SherpaOnnxDestroyOnlineRecognizerResult(result_ptr);
            sherpa_rs_sys::SherpaOnnxDestroyOnlineStream(stream);
            text
        };

        Ok(text.trim().to_string())
    }
}

impl SttEngine for SherpaStreamingEngine {
    fn transcribe(&self, audio: &[f32]) -> gibberish_stt::Result<Vec<Segment>> {
        let text = self
            .transcribe_offline_via_online_api(audio)
            .map_err(|e| gibberish_stt::SttError::TranscriptionFailed(e.to_string()))?;

        let end_ms = (audio.len() as f64 / 16000.0 * 1000.0).round() as u64;
        Ok(vec![Segment {
            text,
            start_ms: 0,
            end_ms,
            words: Vec::<Word>::new(),
            speaker: None,
        }])
    }

    fn is_streaming_capable(&self) -> bool {
        true
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn supported_languages(&self) -> Vec<&'static str> {
        vec!["en"]
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Drop for SherpaStreamingEngine {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.state.lock() {
            // Only destroy the stream - recognizer is destroyed via Arc<RecognizerHandle>
            // when all references (engine + workers) are dropped.
            unsafe {
                if !guard.stream.is_null() {
                    sherpa_rs_sys::SherpaOnnxDestroyOnlineStream(guard.stream);
                    guard.stream = ptr::null();
                }
            }
        }
    }
}

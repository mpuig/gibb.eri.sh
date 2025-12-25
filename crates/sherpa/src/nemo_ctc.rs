//! NeMo CTC offline recognizer engine.
//!
//! Supports NeMo Conformer CTC models exported to ONNX format.
//! Used for language-specific models like Catalan.

use std::any::Any;
use std::ffi::CString;
use std::mem;
use std::path::Path;
use std::sync::Mutex;

use gibberish_stt::{Segment, SttEngine, Word};

use crate::{Result, SherpaError};

/// NeMo CTC offline recognizer for batch transcription.
pub struct SherpaNemoCtcEngine {
    recognizer: Mutex<*const sherpa_rs_sys::SherpaOnnxOfflineRecognizer>,
    model_name: String,
    // Keep CStrings alive for the duration of the recognizer
    #[allow(dead_code)]
    strings: NemoCtcStrings,
}

#[derive(Debug)]
struct NemoCtcStrings {
    model: CString,
    tokens: CString,
    provider: CString,
    decoding_method: CString,
}

impl SherpaNemoCtcEngine {
    /// Create a new NeMo CTC engine from the given model directory.
    ///
    /// The directory should contain:
    /// - `model.onnx` - The ONNX model file
    /// - `tokens.txt` - The vocabulary file
    pub fn new(model_dir: impl AsRef<Path>) -> Result<Self> {
        let model_dir = model_dir.as_ref();

        let model_path = model_dir.join("model.onnx");
        let tokens_path = model_dir.join("tokens.txt");

        if !model_path.exists() {
            return Err(SherpaError::MissingFiles(format!(
                "expected {}",
                model_path.display()
            )));
        }
        if !tokens_path.exists() {
            return Err(SherpaError::MissingFiles(format!(
                "expected {}",
                tokens_path.display()
            )));
        }

        let strings = NemoCtcStrings {
            model: CString::new(
                model_path
                    .to_str()
                    .ok_or_else(|| SherpaError::MissingFiles("invalid model path".to_string()))?,
            )
            .map_err(|e| SherpaError::LoadFailed(e.to_string()))?,
            tokens: CString::new(
                tokens_path
                    .to_str()
                    .ok_or_else(|| SherpaError::MissingFiles("invalid tokens path".to_string()))?,
            )
            .map_err(|e| SherpaError::LoadFailed(e.to_string()))?,
            provider: CString::new("cpu").expect("valid cstring"),
            decoding_method: CString::new("greedy_search").expect("valid cstring"),
        };

        tracing::info!(
            model = ?model_path,
            "Initializing NeMo CTC recognizer"
        );

        let recognizer = unsafe {
            let model_config = sherpa_rs_sys::SherpaOnnxOfflineModelConfig {
                debug: 0,
                num_threads: 2,
                provider: strings.provider.as_ptr(),
                nemo_ctc: sherpa_rs_sys::SherpaOnnxOfflineNemoEncDecCtcModelConfig {
                    model: strings.model.as_ptr(),
                },
                tokens: strings.tokens.as_ptr(),
                // Zero out unused model types
                dolphin: mem::zeroed::<_>(),
                paraformer: mem::zeroed::<_>(),
                tdnn: mem::zeroed::<_>(),
                telespeech_ctc: mem::zeroed::<_>(),
                fire_red_asr: mem::zeroed::<_>(),
                transducer: mem::zeroed::<_>(),
                whisper: mem::zeroed::<_>(),
                sense_voice: mem::zeroed::<_>(),
                moonshine: mem::zeroed::<_>(),
                bpe_vocab: mem::zeroed::<_>(),
                model_type: mem::zeroed::<_>(),
                modeling_unit: mem::zeroed::<_>(),
                zipformer_ctc: mem::zeroed::<_>(),
                canary: mem::zeroed::<_>(),
            };

            let config = sherpa_rs_sys::SherpaOnnxOfflineRecognizerConfig {
                decoding_method: strings.decoding_method.as_ptr(),
                model_config,
                feat_config: sherpa_rs_sys::SherpaOnnxFeatureConfig {
                    sample_rate: 16000,
                    feature_dim: 80,
                },
                hotwords_file: mem::zeroed::<_>(),
                hotwords_score: mem::zeroed::<_>(),
                lm_config: mem::zeroed::<_>(),
                max_active_paths: mem::zeroed::<_>(),
                rule_fars: mem::zeroed::<_>(),
                rule_fsts: mem::zeroed::<_>(),
                blank_penalty: mem::zeroed::<_>(),
                hr: mem::zeroed::<_>(),
            };

            sherpa_rs_sys::SherpaOnnxCreateOfflineRecognizer(&config)
        };

        if recognizer.is_null() {
            return Err(SherpaError::LoadFailed(
                "SherpaOnnxCreateOfflineRecognizer failed for NeMo CTC".to_string(),
            ));
        }

        let model_name = model_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("nemo-ctc")
            .to_string();

        tracing::info!("NeMo CTC model loaded: {}", model_name);

        Ok(Self {
            recognizer: Mutex::new(recognizer),
            model_name,
            strings,
        })
    }

    /// Transcribe audio samples.
    pub fn transcribe_samples(&self, samples: &[f32], sample_rate: u32) -> Result<String> {
        let recognizer = self
            .recognizer
            .lock()
            .map_err(|_| SherpaError::TranscriptionFailed("lock poisoned".to_string()))?;

        let text = unsafe {
            let stream = sherpa_rs_sys::SherpaOnnxCreateOfflineStream(*recognizer);
            if stream.is_null() {
                return Err(SherpaError::TranscriptionFailed(
                    "Failed to create offline stream".to_string(),
                ));
            }

            sherpa_rs_sys::SherpaOnnxAcceptWaveformOffline(
                stream,
                sample_rate as i32,
                samples.as_ptr(),
                samples.len() as i32,
            );

            sherpa_rs_sys::SherpaOnnxDecodeOfflineStream(*recognizer, stream);

            let result_ptr = sherpa_rs_sys::SherpaOnnxGetOfflineStreamResult(stream);
            let text = if result_ptr.is_null() || (*result_ptr).text.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr((*result_ptr).text)
                    .to_string_lossy()
                    .to_string()
            };

            if !result_ptr.is_null() {
                sherpa_rs_sys::SherpaOnnxDestroyOfflineRecognizerResult(result_ptr);
            }
            sherpa_rs_sys::SherpaOnnxDestroyOfflineStream(stream);

            text
        };

        Ok(text.trim().to_string())
    }
}

impl SttEngine for SherpaNemoCtcEngine {
    fn transcribe(&self, audio: &[f32]) -> gibberish_stt::Result<Vec<Segment>> {
        let text = self
            .transcribe_samples(audio, 16000)
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

    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn supported_languages(&self) -> Vec<&'static str> {
        // NeMo CTC models are language-specific
        vec!["ca"]
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

unsafe impl Send for SherpaNemoCtcEngine {}
unsafe impl Sync for SherpaNemoCtcEngine {}

impl Drop for SherpaNemoCtcEngine {
    fn drop(&mut self) {
        if let Ok(guard) = self.recognizer.lock() {
            if !(*guard).is_null() {
                unsafe {
                    sherpa_rs_sys::SherpaOnnxDestroyOfflineRecognizer(*guard);
                }
                tracing::debug!("Destroyed NeMo CTC recognizer");
            }
        }
    }
}

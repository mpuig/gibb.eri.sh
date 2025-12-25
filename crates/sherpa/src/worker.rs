//! Channel-based inference worker for non-blocking STT.
//!
//! Decouples audio ingestion from inference by running decoding on a dedicated thread.

use std::ffi::CStr;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use gibberish_stt::SILENCE_INJECTION_SAMPLES;

use crate::RecognizerHandle;

/// Wrapper for sherpa pointers that can be sent to a worker thread.
///
/// # Safety
/// These are safe to send because:
/// - The recognizer is ref-counted via Arc and thread-safe for inference
/// - The stream is exclusively owned by the worker thread after creation
struct SendablePointers {
    /// Arc-wrapped recognizer handle - keeps recognizer alive as long as worker runs.
    recognizer: Arc<RecognizerHandle>,
    /// Stream pointer (exclusively owned by worker thread).
    stream: *const sherpa_rs_sys::SherpaOnnxOnlineStream,
}

// Safety: The sherpa recognizer is designed to be shared across threads (via Arc),
// and each stream is exclusively used by one thread.
unsafe impl Send for SendablePointers {}

/// Request sent to the inference worker.
pub enum InferenceRequest {
    /// Process an audio chunk.
    Chunk {
        sample_rate: u32,
        samples: Arc<[f32]>,
    },
    /// Inject silence to help acoustic model reset context.
    InjectSilence { sample_rate: u32 },
    /// Reset the stream state.
    Reset,
    /// Shutdown the worker.
    Shutdown,
}

/// Result from the inference worker.
#[derive(Debug, Clone)]
pub struct InferenceResult {
    pub committed_text: String,
    pub partial_text: String,
    pub is_partial: bool,
    pub buffer_duration_ms: u64,
    pub committed_delta: Option<String>,
}

/// A non-blocking wrapper around SherpaStreamingEngine.
///
/// Audio chunks are sent via a channel to a dedicated inference thread,
/// preventing mutex contention during decoding.
pub struct SherpaWorker {
    request_tx: mpsc::Sender<InferenceRequest>,
    result_rx: mpsc::Receiver<InferenceResult>,
    worker_handle: Option<JoinHandle<()>>,
    model_name: String,
    /// Cache of the latest result for synchronous queries.
    latest_result: std::sync::Mutex<InferenceResult>,
}

impl SherpaWorker {
    /// Create a new worker with the given recognizer handle and stream.
    ///
    /// The Arc<RecognizerHandle> ensures the recognizer stays alive as long as
    /// this worker is running, preventing use-after-free on engine drop.
    pub fn new(
        recognizer: Arc<RecognizerHandle>,
        stream: *const sherpa_rs_sys::SherpaOnnxOnlineStream,
        model_name: String,
    ) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<InferenceRequest>();
        let (result_tx, result_rx) = mpsc::channel::<InferenceResult>();

        let pointers = SendablePointers { recognizer, stream };

        let worker_handle = thread::spawn(move || {
            inference_loop(pointers, request_rx, result_tx);
        });

        Self {
            request_tx,
            result_rx,
            worker_handle: Some(worker_handle),
            model_name,
            latest_result: std::sync::Mutex::new(InferenceResult {
                committed_text: String::new(),
                partial_text: String::new(),
                is_partial: true,
                buffer_duration_ms: 0,
                committed_delta: None,
            }),
        }
    }

    /// Send an audio chunk for processing (non-blocking).
    ///
    /// Returns an error if the worker thread has panicked or shut down.
    pub fn send_chunk(
        &self,
        sample_rate: u32,
        samples: Arc<[f32]>,
    ) -> Result<(), mpsc::SendError<InferenceRequest>> {
        self.request_tx.send(InferenceRequest::Chunk {
            sample_rate,
            samples,
        })
    }

    /// Inject silence samples to help acoustic model reset context.
    /// Call this when VAD detects speech-to-silence transition.
    pub fn inject_silence(
        &self,
        sample_rate: u32,
    ) -> Result<(), mpsc::SendError<InferenceRequest>> {
        self.request_tx
            .send(InferenceRequest::InjectSilence { sample_rate })
    }

    /// Try to receive the latest result (non-blocking).
    pub fn try_recv(&self) -> Option<InferenceResult> {
        match self.result_rx.try_recv() {
            Ok(result) => {
                // Update cache
                if let Ok(mut cache) = self.latest_result.lock() {
                    *cache = result.clone();
                }
                Some(result)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => None,
        }
    }

    /// Drain all pending results and return the latest.
    pub fn drain_results(&self) -> Option<InferenceResult> {
        let mut latest = None;
        while let Ok(result) = self.result_rx.try_recv() {
            latest = Some(result);
        }
        if let Some(ref result) = latest {
            if let Ok(mut cache) = self.latest_result.lock() {
                *cache = result.clone();
            }
        }
        latest
    }

    /// Get the cached latest result.
    pub fn get_latest(&self) -> InferenceResult {
        self.latest_result
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| InferenceResult {
                committed_text: String::new(),
                partial_text: String::new(),
                is_partial: true,
                buffer_duration_ms: 0,
                committed_delta: None,
            })
    }

    /// Reset the stream state.
    pub fn reset(&self) {
        let _ = self.request_tx.send(InferenceRequest::Reset);
        // Clear cached result
        if let Ok(mut cache) = self.latest_result.lock() {
            *cache = InferenceResult {
                committed_text: String::new(),
                partial_text: String::new(),
                is_partial: true,
                buffer_duration_ms: 0,
                committed_delta: None,
            };
        }
    }

    pub fn model_name(&self) -> &str {
        &self.model_name
    }
}

impl Drop for SherpaWorker {
    fn drop(&mut self) {
        let _ = self.request_tx.send(InferenceRequest::Shutdown);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

/// The inference loop running on a dedicated thread.
fn inference_loop(
    ptrs: SendablePointers,
    request_rx: mpsc::Receiver<InferenceRequest>,
    result_tx: mpsc::Sender<InferenceResult>,
) {
    // Keep the Arc alive for the duration of the loop - this prevents the
    // recognizer from being destroyed while we're using it.
    let recognizer_handle = ptrs.recognizer;
    let recognizer = recognizer_handle.ptr();
    let stream = ptrs.stream;

    let mut committed_text = String::new();
    let mut last_partial = String::new();
    let mut total_samples: u64 = 0;

    while let Ok(request) = request_rx.recv() {
        match request {
            InferenceRequest::Chunk {
                sample_rate,
                samples,
            } => {
                let chunk_samples = samples.len();
                total_samples = total_samples.saturating_add(chunk_samples as u64);

                let inference_start = std::time::Instant::now();

                // Accept waveform
                unsafe {
                    sherpa_rs_sys::SherpaOnnxOnlineStreamAcceptWaveform(
                        stream,
                        sample_rate as i32,
                        samples.as_ptr(),
                        samples.len() as i32,
                    );

                    // Decode all available frames
                    while sherpa_rs_sys::SherpaOnnxIsOnlineStreamReady(recognizer, stream) == 1 {
                        sherpa_rs_sys::SherpaOnnxDecodeOnlineStream(recognizer, stream);
                    }
                }

                let inference_ms = inference_start.elapsed().as_millis() as u64;
                let audio_ms = (chunk_samples as f64 / sample_rate as f64 * 1000.0) as u64;
                if inference_ms > audio_ms {
                    tracing::debug!(
                        inference_ms,
                        audio_ms,
                        rtf = inference_ms as f32 / audio_ms.max(1) as f32,
                        "Inference slower than real-time"
                    );
                }

                // Get result
                let partial = unsafe {
                    let result_ptr =
                        sherpa_rs_sys::SherpaOnnxGetOnlineStreamResult(recognizer, stream);
                    if result_ptr.is_null() {
                        String::new()
                    } else {
                        let text = if (*result_ptr).text.is_null() {
                            String::new()
                        } else {
                            CStr::from_ptr((*result_ptr).text)
                                .to_string_lossy()
                                .to_string()
                        };
                        sherpa_rs_sys::SherpaOnnxDestroyOnlineRecognizerResult(result_ptr);
                        text
                    }
                };

                // Check endpoint
                let is_endpoint = unsafe {
                    sherpa_rs_sys::SherpaOnnxOnlineStreamIsEndpoint(recognizer, stream) == 1
                };

                let mut committed_delta = None;

                if is_endpoint && !partial.trim().is_empty() {
                    let delta = partial.trim().to_string();
                    committed_delta = Some(delta.clone());
                    if !committed_text.is_empty() {
                        committed_text.push(' ');
                    }
                    committed_text.push_str(&delta);
                    last_partial.clear();
                    total_samples = 0;

                    unsafe {
                        sherpa_rs_sys::SherpaOnnxOnlineStreamReset(recognizer, stream);
                    }
                } else {
                    last_partial = partial;
                }

                let buffer_duration_ms =
                    (total_samples as f64 / sample_rate as f64 * 1000.0).round() as u64;

                let result = InferenceResult {
                    committed_text: committed_text.clone(),
                    partial_text: last_partial.clone(),
                    is_partial: !is_endpoint,
                    buffer_duration_ms,
                    committed_delta,
                };

                // Send result (ignore error if receiver dropped)
                let _ = result_tx.send(result);
            }

            InferenceRequest::InjectSilence { sample_rate } => {
                // Feed silence samples to help acoustic model reset context
                let silence = vec![0.0f32; SILENCE_INJECTION_SAMPLES];
                unsafe {
                    sherpa_rs_sys::SherpaOnnxOnlineStreamAcceptWaveform(
                        stream,
                        sample_rate as i32,
                        silence.as_ptr(),
                        silence.len() as i32,
                    );

                    while sherpa_rs_sys::SherpaOnnxIsOnlineStreamReady(recognizer, stream) == 1 {
                        sherpa_rs_sys::SherpaOnnxDecodeOnlineStream(recognizer, stream);
                    }
                }
                tracing::debug!("Injected {} silence samples", SILENCE_INJECTION_SAMPLES);
            }

            InferenceRequest::Reset => {
                unsafe {
                    sherpa_rs_sys::SherpaOnnxOnlineStreamReset(recognizer, stream);
                }
                committed_text.clear();
                last_partial.clear();
                total_samples = 0;

                let result = InferenceResult {
                    committed_text: String::new(),
                    partial_text: String::new(),
                    is_partial: true,
                    buffer_duration_ms: 0,
                    committed_delta: None,
                };
                let _ = result_tx.send(result);
            }

            InferenceRequest::Shutdown => {
                break;
            }
        }
    }

    // Clean up the stream (recognizer is destroyed when Arc drops)
    unsafe {
        if !stream.is_null() {
            sherpa_rs_sys::SherpaOnnxDestroyOnlineStream(stream);
        }
    }
    // recognizer_handle (Arc) is dropped here, decrementing the ref count.
    // If this was the last reference, the recognizer is destroyed.
}

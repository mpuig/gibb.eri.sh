use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream};
use crossbeam_channel::{Receiver, Sender};
use std::sync::{Arc, Mutex};

#[cfg(target_os = "macos")]
use crate::speaker::SpeakerInput;
#[cfg(target_os = "macos")]
use futures::StreamExt;

const TARGET_SAMPLE_RATE: u32 = 16000;

/// Minimum samples needed before mixing (10ms at 16kHz)
const MIXER_MIN_SAMPLES: usize = 160;

/// Fallback threshold for single-channel audio (100ms at 16kHz)
const MIXER_FALLBACK_SAMPLES: usize = 1600;

/// Time to wait before falling back to single-channel when one source stops producing.
/// Reduced from 200ms to 50ms for lower latency.
const MIXER_FALLBACK_TIMEOUT_MS: u128 = 50;

/// Timeout for checking if speaker stream should exit
const SPEAKER_POLL_TIMEOUT_MS: u64 = 500;

#[derive(Debug, Clone)]
pub enum AudioSource {
    Microphone {
        device_id: Option<String>,
    },
    SystemAudio {
        device_id: Option<String>,
    },
    Combined {
        mic_device_id: Option<String>,
        system_device_id: Option<String>,
    },
    /// System audio capture via Core Audio Tap (macOS, no virtual device needed, no permission required)
    #[cfg(target_os = "macos")]
    SystemAudioNative,
    /// Combined microphone + Core Audio Tap system audio
    #[cfg(target_os = "macos")]
    CombinedNative {
        mic_device_id: Option<String>,
    },
}

pub struct AudioStream {
    _streams: Vec<Stream>,
    #[cfg(target_os = "macos")]
    _speaker_handle: Option<std::thread::JoinHandle<()>>,
    receiver: Option<Receiver<Vec<f32>>>,
}

impl AudioStream {
    /// Take the receiver out of this AudioStream (can only be called once).
    ///
    /// The receiver supports blocking `recv()` and `recv_timeout()` for
    /// efficient single-consumer use without polling.
    pub fn take_receiver(&mut self) -> Option<Receiver<Vec<f32>>> {
        self.receiver.take()
    }
}

impl Drop for AudioStream {
    fn drop(&mut self) {
        tracing::info!("AudioStream::drop starting");
        // Drop receiver first to signal senders that no one is listening
        self.receiver.take();
        tracing::info!("AudioStream::drop: receiver dropped");

        // Drop streams to stop audio callbacks
        self._streams.clear();
        tracing::info!("AudioStream::drop: streams cleared");

        // Don't block waiting for speaker thread - it will exit when mixer.push fails
        // Just log that we're not waiting
        #[cfg(target_os = "macos")]
        if self._speaker_handle.is_some() {
            tracing::info!("AudioStream::drop: speaker thread will exit when mixer channel closes");
        }
        tracing::info!("AudioStream::drop complete");
    }
}

impl AudioStream {
    pub fn new(source: AudioSource) -> crate::Result<Self> {
        let host = cpal::default_host();

        match source {
            AudioSource::Microphone { device_id } => {
                let device = get_device(&host, device_id.as_deref(), false)?;
                let (tx, rx) = crossbeam_channel::unbounded::<Vec<f32>>();
                let stream = build_stream(device, tx)?;
                Ok(Self {
                    _streams: vec![stream],
                    #[cfg(target_os = "macos")]
                    _speaker_handle: None,
                    receiver: Some(rx),
                })
            }
            AudioSource::SystemAudio { device_id } => {
                let device = get_device(&host, device_id.as_deref(), true)?;
                let (tx, rx) = crossbeam_channel::unbounded::<Vec<f32>>();
                let stream = build_stream(device, tx)?;
                Ok(Self {
                    _streams: vec![stream],
                    #[cfg(target_os = "macos")]
                    _speaker_handle: None,
                    receiver: Some(rx),
                })
            }
            AudioSource::Combined {
                mic_device_id,
                system_device_id,
            } => {
                let mic_device = get_device(&host, mic_device_id.as_deref(), false)?;
                let system_device = get_device(&host, system_device_id.as_deref(), true)?;

                let (tx, rx) = crossbeam_channel::unbounded::<Vec<f32>>();
                let mixer = AudioMixer::new(tx);

                let mic_stream = build_stream_with_mixer(mic_device, mixer.clone(), 0)?;
                let system_stream = build_stream_with_mixer(system_device, mixer, 1)?;

                Ok(Self {
                    _streams: vec![mic_stream, system_stream],
                    #[cfg(target_os = "macos")]
                    _speaker_handle: None,
                    receiver: Some(rx),
                })
            }
            #[cfg(target_os = "macos")]
            AudioSource::SystemAudioNative => {
                tracing::info!("AudioStream: Creating SystemAudioNative source");
                let speaker_input = SpeakerInput::new()?;
                let source_sample_rate = speaker_input.sample_rate();
                tracing::info!(
                    sample_rate = source_sample_rate,
                    "AudioStream: SpeakerInput created"
                );
                let mut speaker_stream = speaker_input.stream()?;
                tracing::info!("AudioStream: SpeakerStream created, starting poll thread");

                let (tx, rx) = crossbeam_channel::unbounded::<Vec<f32>>();

                let handle = std::thread::spawn(move || {
                    tracing::info!("AudioStream: Speaker poll thread started");
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("failed to create tokio runtime for speaker thread");

                    rt.block_on(async {
                        let mut sample_count = 0u64;
                        let mut send_count = 0u64;
                        let mut agc = AgcState::default();
                        let mut resampler =
                            SincResampler::new(source_sample_rate, TARGET_SAMPLE_RATE);
                        while let Some(samples) = speaker_stream.next().await {
                            sample_count += samples.len() as u64;
                            if send_count < 5 || sample_count % 48000 == 0 {
                                tracing::info!(
                                    total_samples = sample_count,
                                    batch_size = samples.len(),
                                    send_count,
                                    "AudioStream: received speaker samples"
                                );
                            }
                            let mut processed =
                                process_audio_with_resampler(&samples, 1, resampler.as_mut());
                            agc.process(&mut processed);
                            match tx.send(processed) {
                                Ok(()) => {
                                    send_count += 1;
                                    if send_count <= 5 {
                                        tracing::info!(send_count, "AudioStream: sent to channel");
                                    }
                                }
                                Err(_) => {
                                    tracing::info!("AudioStream: channel closed, stopping");
                                    break;
                                }
                            }
                        }
                        tracing::info!("AudioStream: Speaker stream ended");
                    });
                });

                Ok(Self {
                    _streams: vec![],
                    _speaker_handle: Some(handle),
                    receiver: Some(rx),
                })
            }
            #[cfg(target_os = "macos")]
            AudioSource::CombinedNative { mic_device_id } => {
                tracing::info!("CombinedNative: getting mic device");
                let mic_device = get_device(&host, mic_device_id.as_deref(), false)?;
                tracing::info!("CombinedNative: mic device obtained");

                let (tx, rx) = crossbeam_channel::unbounded::<Vec<f32>>();
                let mixer = AudioMixer::new(tx);

                // Microphone stream via cpal
                tracing::info!("CombinedNative: building mic stream");
                let mic_stream = build_stream_with_mixer(mic_device, mixer.clone(), 0)?;
                tracing::info!("CombinedNative: mic stream built");

                // System audio via Core Audio Tap
                tracing::info!("CombinedNative: creating SpeakerInput");
                let speaker_input = SpeakerInput::new()?;
                tracing::info!("CombinedNative: SpeakerInput created");
                let source_sample_rate = speaker_input.sample_rate();
                tracing::info!("CombinedNative: creating speaker stream");
                let mut speaker_stream = speaker_input.stream()?;
                tracing::info!("CombinedNative: speaker stream created");

                let mixer_for_speaker = mixer;
                let handle = std::thread::spawn(move || {
                    tracing::info!("CombinedNative: speaker polling thread started");
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("failed to create tokio runtime for combined speaker thread");

                    rt.block_on(async {
                        use tokio::time::{timeout, Duration};
                        let mut poll_count = 0u64;
                        let mut agc = AgcState::default();
                        let mut resampler = SincResampler::new(source_sample_rate, TARGET_SAMPLE_RATE);
                        loop {
                            // Use timeout so we can periodically check if channel is closed
                            match timeout(Duration::from_millis(SPEAKER_POLL_TIMEOUT_MS), speaker_stream.next()).await {
                                Ok(Some(samples)) => {
                                    poll_count += 1;
                                    if poll_count <= 3 {
                                        tracing::info!(poll_count, samples_len = samples.len(), "CombinedNative: speaker got samples");
                                    }
                                    let mut processed = process_audio_with_resampler(&samples, 1, resampler.as_mut());
                                    agc.process(&mut processed);
                                    if !mixer_for_speaker.push(1, processed) {
                                        tracing::info!("CombinedNative: mixer channel closed, stopping speaker thread");
                                        break;
                                    }
                                }
                                Ok(None) => {
                                    tracing::info!("CombinedNative: speaker stream ended");
                                    break;
                                }
                                Err(_) => {
                                    // Timeout - check if we should exit by trying an empty push
                                    if !mixer_for_speaker.push(1, vec![]) {
                                        tracing::info!("CombinedNative: mixer channel closed (timeout check), stopping speaker thread");
                                        break;
                                    }
                                }
                            }
                        }
                        tracing::info!(poll_count, "CombinedNative: speaker polling loop ended");
                    });
                    tracing::info!("CombinedNative: speaker polling thread exiting");
                });

                Ok(Self {
                    _streams: vec![mic_stream],
                    _speaker_handle: Some(handle),
                    receiver: Some(rx),
                })
            }
        }
    }
}

fn get_device(
    host: &cpal::Host,
    device_id: Option<&str>,
    prefer_virtual: bool,
) -> crate::Result<Device> {
    match device_id {
        Some(id) => host
            .input_devices()?
            .find(|d| d.name().ok().as_deref() == Some(id))
            .ok_or_else(|| crate::AudioError::DeviceNotFound(id.to_string())),
        None => {
            if prefer_virtual {
                // Look for virtual audio device (BlackHole, Soundflower, etc.)
                host.input_devices()?
                    .find(|d| {
                        d.name()
                            .map(|n| {
                                let lower = n.to_lowercase();
                                lower.contains("blackhole")
                                    || lower.contains("soundflower")
                                    || lower.contains("loopback")
                            })
                            .unwrap_or(false)
                    })
                    .ok_or_else(|| {
                        crate::AudioError::DeviceNotFound(
                            "No virtual audio device found (BlackHole/Soundflower)".to_string(),
                        )
                    })
            } else {
                host.default_input_device()
                    .ok_or_else(|| crate::AudioError::DeviceNotFound("default".to_string()))
            }
        }
    }
}

fn build_stream(device: Device, tx: Sender<Vec<f32>>) -> crate::Result<Stream> {
    let config = device.default_input_config().map_err(|e| {
        crate::AudioError::StreamError(format!("failed to get default config: {e}"))
    })?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let agc = Arc::new(Mutex::new(AgcState::default()));

    // Use sinc resampler for high-quality resampling when needed
    let resampler = if sample_rate != TARGET_SAMPLE_RATE {
        SincResampler::new(sample_rate, TARGET_SAMPLE_RATE).map(|r| Arc::new(Mutex::new(r)))
    } else {
        None
    };

    let stream = match config.sample_format() {
        SampleFormat::F32 => {
            let agc = agc.clone();
            let resampler = resampler.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[f32], _| {
                    let mut samples = if let Some(ref resampler) = resampler {
                        if let Ok(mut r) = resampler.lock() {
                            process_audio_with_resampler(data, channels, Some(&mut r))
                        } else {
                            process_audio(data, channels, sample_rate, TARGET_SAMPLE_RATE)
                                .into_owned()
                        }
                    } else {
                        process_audio(data, channels, sample_rate, TARGET_SAMPLE_RATE).into_owned()
                    };
                    if let Ok(mut agc) = agc.lock() {
                        agc.process(&mut samples);
                    }
                    let _ = tx.send(samples);
                },
                |err| tracing::error!("audio stream error: {}", err),
                None,
            )?
        }
        SampleFormat::I16 => {
            let agc = agc.clone();
            let resampler = resampler.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _| {
                    let float: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                    let mut samples = if let Some(ref resampler) = resampler {
                        if let Ok(mut r) = resampler.lock() {
                            process_audio_with_resampler(&float, channels, Some(&mut r))
                        } else {
                            process_audio(&float, channels, sample_rate, TARGET_SAMPLE_RATE)
                                .into_owned()
                        }
                    } else {
                        process_audio(&float, channels, sample_rate, TARGET_SAMPLE_RATE)
                            .into_owned()
                    };
                    if let Ok(mut agc) = agc.lock() {
                        agc.process(&mut samples);
                    }
                    let _ = tx.send(samples);
                },
                |err| tracing::error!("audio stream error: {}", err),
                None,
            )?
        }
        format => {
            return Err(crate::AudioError::StreamError(format!(
                "unsupported sample format: {format:?}"
            )));
        }
    };

    stream
        .play()
        .map_err(|e| crate::AudioError::StreamError(format!("failed to start stream: {e}")))?;

    Ok(stream)
}

#[derive(Clone)]
struct AudioMixer {
    tx: Sender<Vec<f32>>,
    state: Arc<Mutex<MixerState>>,
}

struct MixerState {
    buffers: [Vec<f32>; 2],
    /// When we emit audio while one channel is missing, we treat the missing portion as silence
    /// and mark the channel as "behind". Any late-arriving samples for that time window must be
    /// dropped to avoid time-misaligned mixing ("fragment mixing").
    pending_drop: [usize; 2],
    last_emit: std::time::Instant,
}

impl AudioMixer {
    fn new(tx: Sender<Vec<f32>>) -> Self {
        Self {
            tx,
            state: Arc::new(Mutex::new(MixerState {
                buffers: [Vec::new(), Vec::new()],
                pending_drop: [0, 0],
                last_emit: std::time::Instant::now(),
            })),
        }
    }

    /// Push samples to the mixer. Returns false if the channel is closed (no receivers).
    fn push(&self, channel: usize, samples: Vec<f32>) -> bool {
        let mut state = self.state.lock().expect("audio mixer state mutex poisoned");
        state.buffers[channel].extend(samples);

        // If we previously emitted while this channel was missing, discard any late-arriving
        // samples so we don't mix across time.
        let pending = state.pending_drop[channel].min(state.buffers[channel].len());
        if pending > 0 {
            state.buffers[channel].drain(..pending);
            state.pending_drop[channel] -= pending;
        }

        self.emit_ready(&mut state)
    }

    fn emit_ready(&self, state: &mut MixerState) -> bool {
        let min_len = state.buffers[0].len().min(state.buffers[1].len());
        let max_len = state.buffers[0].len().max(state.buffers[1].len());

        // Fast path: both channels have data; emit in batches aligned to MIXER_MIN_SAMPLES.
        if min_len >= MIXER_MIN_SAMPLES {
            let len = (min_len / MIXER_MIN_SAMPLES) * MIXER_MIN_SAMPLES;
            let buf0: Vec<f32> = state.buffers[0].drain(..len).collect();
            let buf1: Vec<f32> = state.buffers[1].drain(..len).collect();

            let mixed: Vec<f32> = buf0
                .into_iter()
                .zip(buf1)
                .map(|(a, b)| (a + b) * 0.5)
                .collect();

            if self.tx.send(mixed).is_err() {
                return false;
            }
            state.last_emit = std::time::Instant::now();
            return true;
        }

        // Fallback: if one channel is not producing, emit from the other to keep streaming alive.
        // IMPORTANT: we must preserve time alignment. We treat missing samples as silence and
        // mark the missing channel for dropping the equivalent amount if it produces audio later.
        if max_len >= MIXER_FALLBACK_SAMPLES
            && state.last_emit.elapsed().as_millis() > MIXER_FALLBACK_TIMEOUT_MS
        {
            let len = (max_len.min(MIXER_FALLBACK_SAMPLES) / MIXER_MIN_SAMPLES) * MIXER_MIN_SAMPLES;
            if len == 0 {
                return true;
            }

            let take0 = state.buffers[0].len().min(len);
            let take1 = state.buffers[1].len().min(len);
            let buf0: Vec<f32> = state.buffers[0].drain(..take0).collect();
            let buf1: Vec<f32> = state.buffers[1].drain(..take1).collect();

            if take0 < len {
                state.pending_drop[0] = state.pending_drop[0].saturating_add(len - take0);
            }
            if take1 < len {
                state.pending_drop[1] = state.pending_drop[1].saturating_add(len - take1);
            }

            let mut mixed = Vec::with_capacity(len);
            for i in 0..len {
                let a = buf0.get(i).copied().unwrap_or(0.0);
                let b = buf1.get(i).copied().unwrap_or(0.0);
                mixed.push((a + b) * 0.5);
            }

            if self.tx.send(mixed).is_err() {
                return false;
            }
            state.last_emit = std::time::Instant::now();
        }

        true
    }
}

fn build_stream_with_mixer(
    device: Device,
    mixer: AudioMixer,
    channel: usize,
) -> crate::Result<Stream> {
    tracing::info!(channel, "build_stream_with_mixer: getting default config");
    let config = device.default_input_config().map_err(|e| {
        tracing::error!(channel, error = %e, "build_stream_with_mixer: failed to get config");
        crate::AudioError::StreamError(format!("failed to get default config: {e}"))
    })?;
    tracing::info!(
        channel,
        sample_rate = config.sample_rate().0,
        channels = config.channels(),
        "build_stream_with_mixer: config obtained"
    );

    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let agc = Arc::new(Mutex::new(AgcState::default()));

    // Use sinc resampler for high-quality resampling when needed
    let resampler = if sample_rate != TARGET_SAMPLE_RATE {
        SincResampler::new(sample_rate, TARGET_SAMPLE_RATE).map(|r| Arc::new(Mutex::new(r)))
    } else {
        None
    };

    tracing::info!(channel, format = ?config.sample_format(), "build_stream_with_mixer: building input stream");
    let stream = match config.sample_format() {
        SampleFormat::F32 => {
            let agc = agc.clone();
            let resampler = resampler.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[f32], _| {
                    let mut samples = if let Some(ref resampler) = resampler {
                        if let Ok(mut r) = resampler.lock() {
                            process_audio_with_resampler(data, channels, Some(&mut r))
                        } else {
                            process_audio(data, channels, sample_rate, TARGET_SAMPLE_RATE)
                                .into_owned()
                        }
                    } else {
                        process_audio(data, channels, sample_rate, TARGET_SAMPLE_RATE).into_owned()
                    };
                    if let Ok(mut agc) = agc.lock() {
                        agc.process(&mut samples);
                    }
                    mixer.push(channel, samples);
                },
                |err| tracing::error!("audio stream error: {}", err),
                None,
            )?
        }
        SampleFormat::I16 => {
            let agc = agc.clone();
            let resampler = resampler.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _| {
                    let float: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                    let mut samples = if let Some(ref resampler) = resampler {
                        if let Ok(mut r) = resampler.lock() {
                            process_audio_with_resampler(&float, channels, Some(&mut r))
                        } else {
                            process_audio(&float, channels, sample_rate, TARGET_SAMPLE_RATE)
                                .into_owned()
                        }
                    } else {
                        process_audio(&float, channels, sample_rate, TARGET_SAMPLE_RATE)
                            .into_owned()
                    };
                    if let Ok(mut agc) = agc.lock() {
                        agc.process(&mut samples);
                    }
                    mixer.push(channel, samples);
                },
                |err| tracing::error!("audio stream error: {}", err),
                None,
            )?
        }
        format => {
            tracing::error!(
                channel,
                ?format,
                "build_stream_with_mixer: unsupported format"
            );
            return Err(crate::AudioError::StreamError(format!(
                "unsupported sample format: {format:?}"
            )));
        }
    };

    tracing::info!(
        channel,
        "build_stream_with_mixer: stream built, calling play()"
    );
    stream.play().map_err(|e| {
        tracing::error!(channel, error = %e, "build_stream_with_mixer: play() failed");
        crate::AudioError::StreamError(format!("failed to start stream: {e}"))
    })?;

    tracing::info!(
        channel,
        "build_stream_with_mixer: stream started successfully"
    );
    Ok(stream)
}

use std::borrow::Cow;

// ============================================================================
// Automatic Gain Control (AGC)
// ============================================================================

/// Target RMS level in dBFS (decibels relative to full scale).
/// -20 dBFS is a common target for speech that avoids clipping while being loud enough.
const AGC_TARGET_DBFS: f32 = -20.0;

/// Minimum RMS threshold in dBFS below which we don't boost.
/// Prevents boosting silence/noise floor.
const AGC_NOISE_FLOOR_DBFS: f32 = -50.0;

/// Maximum gain to apply (prevents extreme amplification of quiet signals).
const AGC_MAX_GAIN: f32 = 10.0;

/// Minimum gain to apply (prevents extreme attenuation).
const AGC_MIN_GAIN: f32 = 0.1;

/// Smoothing factor for gain changes (0-1, higher = faster response).
/// 0.1 gives smooth transitions over ~100ms at typical chunk rates.
const AGC_SMOOTHING: f32 = 0.1;

/// AGC state that tracks running gain and applies smooth adjustments.
#[derive(Clone)]
struct AgcState {
    current_gain: f32,
}

impl Default for AgcState {
    fn default() -> Self {
        Self { current_gain: 1.0 }
    }
}

impl AgcState {
    /// Apply AGC to audio samples in place.
    fn process(&mut self, samples: &mut [f32]) {
        if samples.is_empty() {
            return;
        }

        // Calculate RMS (root mean square) of the signal
        let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
        let rms = (sum_sq / samples.len() as f32).sqrt();

        // Convert to dBFS
        let rms_dbfs = if rms > 0.0 {
            20.0 * rms.log10()
        } else {
            -100.0 // Silence
        };

        // Only apply AGC if signal is above noise floor
        if rms_dbfs > AGC_NOISE_FLOOR_DBFS {
            // Calculate target gain
            let target_gain = 10.0_f32.powf((AGC_TARGET_DBFS - rms_dbfs) / 20.0);
            let target_gain = target_gain.clamp(AGC_MIN_GAIN, AGC_MAX_GAIN);

            // Smooth gain transition
            self.current_gain =
                self.current_gain * (1.0 - AGC_SMOOTHING) + target_gain * AGC_SMOOTHING;
        }
        // If below noise floor, gradually return to unity gain
        else {
            self.current_gain =
                self.current_gain * (1.0 - AGC_SMOOTHING * 0.5) + 1.0 * AGC_SMOOTHING * 0.5;
        }

        // Apply gain and soft clip to prevent distortion
        for sample in samples.iter_mut() {
            *sample *= self.current_gain;
            // Soft clipping using tanh for smooth limiting
            if sample.abs() > 0.9 {
                *sample = sample.signum() * (0.9 + 0.1 * ((*sample).abs() - 0.9).tanh());
            }
        }
    }
}

// ============================================================================
// High-Quality Resampling with Rubato
// ============================================================================

use rubato::{FftFixedIn, Resampler as RubatoResampler};

/// Wrapper for rubato sinc resampler with buffering for variable input sizes.
struct SincResampler {
    resampler: FftFixedIn<f32>,
    input_buffer: Vec<f32>,
    chunk_size: usize,
}

impl SincResampler {
    fn new(from_rate: u32, to_rate: u32) -> Option<Self> {
        // Use a reasonable chunk size for low-latency processing
        let chunk_size = 256;

        let resampler = FftFixedIn::<f32>::new(
            from_rate as usize,
            to_rate as usize,
            chunk_size,
            2, // Sub-chunks for better quality
            1, // Mono channel
        )
        .ok()?;

        Some(Self {
            resampler,
            input_buffer: Vec::with_capacity(chunk_size * 2),
            chunk_size,
        })
    }

    /// Process input samples and return resampled output.
    fn process(&mut self, samples: &[f32]) -> Vec<f32> {
        self.input_buffer.extend_from_slice(samples);

        let mut output = Vec::new();

        // Process complete chunks
        while self.input_buffer.len() >= self.chunk_size {
            let chunk: Vec<f32> = self.input_buffer.drain(..self.chunk_size).collect();

            if let Ok(resampled) = self.resampler.process(&[chunk], None) {
                if !resampled.is_empty() {
                    output.extend_from_slice(&resampled[0]);
                }
            }
        }

        output
    }
}

// ============================================================================
// Audio Processing Pipeline
// ============================================================================

/// Process audio: convert to mono and resample in a single pass when possible.
/// Uses Cow to avoid allocation when no processing is needed.
fn process_audio<'a>(
    samples: &'a [f32],
    channels: usize,
    from_rate: u32,
    to_rate: u32,
) -> Cow<'a, [f32]> {
    let needs_mono = channels > 1;
    let needs_resample = from_rate != to_rate;

    match (needs_mono, needs_resample) {
        // No processing needed
        (false, false) => Cow::Borrowed(samples),

        // Only mono conversion
        (true, false) => Cow::Owned(to_mono_only(samples, channels)),

        // Only resampling (already mono) - use linear for stateless operation
        (false, true) => Cow::Owned(resample_linear(samples, from_rate, to_rate)),

        // Both needed - combined pass
        (true, true) => Cow::Owned(mono_and_resample_linear(
            samples, channels, from_rate, to_rate,
        )),
    }
}

/// Process audio with stateful rubato resampler for high-quality output.
fn process_audio_with_resampler(
    samples: &[f32],
    channels: usize,
    resampler: Option<&mut SincResampler>,
) -> Vec<f32> {
    let mono = if channels > 1 {
        to_mono_only(samples, channels)
    } else {
        samples.to_vec()
    };

    match resampler {
        Some(r) => r.process(&mono),
        None => mono,
    }
}

#[inline]
fn to_mono_only(samples: &[f32], channels: usize) -> Vec<f32> {
    let mono_len = samples.len() / channels;
    let mut output = Vec::with_capacity(mono_len);
    let inv_channels = 1.0 / channels as f32;

    for chunk in samples.chunks_exact(channels) {
        let sum: f32 = chunk.iter().sum();
        output.push(sum * inv_channels);
    }
    output
}

/// Linear interpolation resampling (fallback for stateless operation).
#[inline]
fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    let ratio = to_rate as f64 / from_rate as f64;
    let new_len = (samples.len() as f64 * ratio) as usize;
    let mut output = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_idx = i as f64 / ratio;
        let idx = src_idx.floor() as usize;
        let frac = src_idx.fract() as f32;
        let sample = if idx + 1 < samples.len() {
            samples[idx] * (1.0 - frac) + samples[idx + 1] * frac
        } else if idx < samples.len() {
            samples[idx]
        } else {
            0.0
        };
        output.push(sample);
    }
    output
}

/// Combined mono conversion and linear resampling in a single pass.
fn mono_and_resample_linear(
    samples: &[f32],
    channels: usize,
    from_rate: u32,
    to_rate: u32,
) -> Vec<f32> {
    let mono_len = samples.len() / channels;
    let ratio = to_rate as f64 / from_rate as f64;
    let output_len = (mono_len as f64 * ratio) as usize;
    let mut output = Vec::with_capacity(output_len);
    let inv_channels = 1.0 / channels as f32;

    for i in 0..output_len {
        let src_idx = i as f64 / ratio;
        let idx = src_idx.floor() as usize;
        let frac = src_idx.fract() as f32;

        // Get mono sample at idx
        let sample_at = |mono_idx: usize| -> f32 {
            if mono_idx >= mono_len {
                return 0.0;
            }
            let start = mono_idx * channels;
            samples[start..start + channels].iter().sum::<f32>() * inv_channels
        };

        let s0 = sample_at(idx);
        let s1 = sample_at(idx + 1);
        output.push(s0 * (1.0 - frac) + s1 * frac);
    }
    output
}

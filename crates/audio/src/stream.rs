use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

#[cfg(target_os = "macos")]
use crate::speaker::SpeakerInput;
#[cfg(target_os = "macos")]
use futures::StreamExt;

const TARGET_SAMPLE_RATE: u32 = 16000;

#[derive(Debug, Clone)]
pub enum AudioSource {
    Microphone { device_id: Option<String> },
    SystemAudio { device_id: Option<String> },
    Combined {
        mic_device_id: Option<String>,
        system_device_id: Option<String>,
    },
    /// System audio capture via Core Audio Tap (macOS, no virtual device needed, no permission required)
    #[cfg(target_os = "macos")]
    SystemAudioNative,
    /// Combined microphone + Core Audio Tap system audio
    #[cfg(target_os = "macos")]
    CombinedNative { mic_device_id: Option<String> },
}

pub struct AudioStream {
    _streams: Vec<Stream>,
    #[cfg(target_os = "macos")]
    _speaker_handle: Option<std::thread::JoinHandle<()>>,
    receiver: Option<broadcast::Receiver<Vec<f32>>>,
}

impl AudioStream {
    /// Take the receiver out of this AudioStream (can only be called once)
    pub fn take_receiver(&mut self) -> Option<broadcast::Receiver<Vec<f32>>> {
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
                let (tx, rx) = broadcast::channel::<Vec<f32>>(100);
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
                let (tx, rx) = broadcast::channel::<Vec<f32>>(100);
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

                let (tx, rx) = broadcast::channel::<Vec<f32>>(100);
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
                tracing::info!(sample_rate = source_sample_rate, "AudioStream: SpeakerInput created");
                let mut speaker_stream = speaker_input.stream()?;
                tracing::info!("AudioStream: SpeakerStream created, starting poll thread");

                let (tx, rx) = broadcast::channel::<Vec<f32>>(100);

                let handle = std::thread::spawn(move || {
                    tracing::info!("AudioStream: Speaker poll thread started");
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();

                    rt.block_on(async {
                        let mut sample_count = 0u64;
                        let mut send_count = 0u64;
                        while let Some(samples) = speaker_stream.next().await {
                            sample_count += samples.len() as u64;
                            if send_count < 5 || sample_count % 48000 == 0 {
                                tracing::info!(total_samples = sample_count, batch_size = samples.len(), send_count, "AudioStream: received speaker samples");
                            }
                            let resampled = resample(&samples, source_sample_rate, TARGET_SAMPLE_RATE);
                            match tx.send(resampled) {
                                Ok(receivers) => {
                                    send_count += 1;
                                    if send_count <= 5 {
                                        tracing::info!(receivers, send_count, "AudioStream: sent to broadcast");
                                    }
                                }
                                Err(_) => {
                                    tracing::info!("AudioStream: broadcast channel closed, stopping");
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

                let (tx, rx) = broadcast::channel::<Vec<f32>>(100);
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
                        .unwrap();

                    rt.block_on(async {
                        use tokio::time::{timeout, Duration};
                        let mut poll_count = 0u64;
                        loop {
                            // Use timeout so we can periodically check if channel is closed
                            match timeout(Duration::from_millis(500), speaker_stream.next()).await {
                                Ok(Some(samples)) => {
                                    poll_count += 1;
                                    if poll_count <= 3 {
                                        tracing::info!(poll_count, samples_len = samples.len(), "CombinedNative: speaker got samples");
                                    }
                                    let resampled = resample(&samples, source_sample_rate, TARGET_SAMPLE_RATE);
                                    if !mixer_for_speaker.push(1, resampled) {
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

fn get_device(host: &cpal::Host, device_id: Option<&str>, prefer_virtual: bool) -> crate::Result<Device> {
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

fn build_stream(device: Device, tx: broadcast::Sender<Vec<f32>>) -> crate::Result<Stream> {
    let config = device.default_input_config().map_err(|e| {
        crate::AudioError::StreamError(format!("failed to get default config: {}", e))
    })?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;

    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let mono = to_mono(data, channels);
                let resampled = resample(&mono, sample_rate, TARGET_SAMPLE_RATE);
                let _ = tx.send(resampled);
            },
            |err| tracing::error!("audio stream error: {}", err),
            None,
        )?,
        SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _| {
                let float: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                let mono = to_mono(&float, channels);
                let resampled = resample(&mono, sample_rate, TARGET_SAMPLE_RATE);
                let _ = tx.send(resampled);
            },
            |err| tracing::error!("audio stream error: {}", err),
            None,
        )?,
        format => {
            return Err(crate::AudioError::StreamError(format!(
                "unsupported sample format: {:?}",
                format
            )));
        }
    };

    stream.play().map_err(|e| {
        crate::AudioError::StreamError(format!("failed to start stream: {}", e))
    })?;

    Ok(stream)
}

#[derive(Clone)]
struct AudioMixer {
    tx: broadcast::Sender<Vec<f32>>,
    buffers: Arc<Mutex<[Vec<f32>; 2]>>,
    last_emit: Arc<Mutex<std::time::Instant>>,
}

impl AudioMixer {
    fn new(tx: broadcast::Sender<Vec<f32>>) -> Self {
        Self {
            tx,
            buffers: Arc::new(Mutex::new([Vec::new(), Vec::new()])),
            last_emit: Arc::new(Mutex::new(std::time::Instant::now())),
        }
    }

    /// Push samples to the mixer. Returns false if the channel is closed (no receivers).
    fn push(&self, channel: usize, samples: Vec<f32>) -> bool {
        let mut buffers = self.buffers.lock().unwrap();
        buffers[channel].extend(samples);

        let min_len = buffers[0].len().min(buffers[1].len());
        let max_len = buffers[0].len().max(buffers[1].len());

        // Mix when both channels have data
        if min_len >= 160 {
            let buf0: Vec<f32> = buffers[0].drain(..min_len).collect();
            let buf1: Vec<f32> = buffers[1].drain(..min_len).collect();

            let mixed: Vec<f32> = buf0
                .into_iter()
                .zip(buf1)
                .map(|(a, b)| (a + b) * 0.5)
                .collect();

            if self.tx.send(mixed).is_err() {
                return false; // No receivers, channel closed
            }
            *self.last_emit.lock().unwrap() = std::time::Instant::now();
        }
        // Fallback: emit from single channel if the other is silent/unavailable
        else if max_len >= 1600 {
            // ~100ms of audio from one channel with nothing from the other
            let elapsed = self.last_emit.lock().unwrap().elapsed();
            if elapsed.as_millis() > 200 {
                // If we haven't emitted in 200ms, just send what we have
                let len = max_len.min(1600);
                let output: Vec<f32> = if buffers[0].len() >= len {
                    buffers[0].drain(..len).collect()
                } else {
                    buffers[1].drain(..len).collect()
                };
                if self.tx.send(output).is_err() {
                    return false; // No receivers, channel closed
                }
                *self.last_emit.lock().unwrap() = std::time::Instant::now();
            }
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
        crate::AudioError::StreamError(format!("failed to get default config: {}", e))
    })?;
    tracing::info!(channel, sample_rate = config.sample_rate().0, channels = config.channels(), "build_stream_with_mixer: config obtained");

    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;

    tracing::info!(channel, format = ?config.sample_format(), "build_stream_with_mixer: building input stream");
    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let mono = to_mono(data, channels);
                let resampled = resample(&mono, sample_rate, TARGET_SAMPLE_RATE);
                mixer.push(channel, resampled);
            },
            |err| tracing::error!("audio stream error: {}", err),
            None,
        )?,
        SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _| {
                let float: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                let mono = to_mono(&float, channels);
                let resampled = resample(&mono, sample_rate, TARGET_SAMPLE_RATE);
                mixer.push(channel, resampled);
            },
            |err| tracing::error!("audio stream error: {}", err),
            None,
        )?,
        format => {
            tracing::error!(channel, ?format, "build_stream_with_mixer: unsupported format");
            return Err(crate::AudioError::StreamError(format!(
                "unsupported sample format: {:?}",
                format
            )));
        }
    };

    tracing::info!(channel, "build_stream_with_mixer: stream built, calling play()");
    stream.play().map_err(|e| {
        tracing::error!(channel, error = %e, "build_stream_with_mixer: play() failed");
        crate::AudioError::StreamError(format!("failed to start stream: {}", e))
    })?;

    tracing::info!(channel, "build_stream_with_mixer: stream started successfully");
    Ok(stream)
}

fn to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels == 1 {
        return samples.to_vec();
    }
    samples
        .chunks(channels)
        .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
        .collect()
}

fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }
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

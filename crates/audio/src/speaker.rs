//! System audio capture using Core Audio Tap (macOS).
//!
//! This uses native macOS APIs to capture system-wide audio output
//! without requiring screen recording permission.

#[cfg(target_os = "macos")]
mod macos {
    use std::any::TypeId;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};
    use std::task::{Poll, Waker};

    use futures::Stream;
    use ringbuf::{
        HeapCons, HeapProd, HeapRb,
        traits::{Consumer, Producer, Split},
    };

    use cidre::{arc, av, cat, cf, core_audio as ca, ns, os};
    use cidre::core_audio::aggregate_device_keys as agg_keys;

    pub const TAP_DEVICE_NAME: &str = "gibberish-audio-tap";
    const CHUNK_SIZE: usize = 4096;

    pub struct SpeakerInput {
        tap: ca::TapGuard,
        agg_desc: arc::Retained<cf::DictionaryOf<cf::String, cf::Type>>,
    }

    struct WakerState {
        waker: Option<Waker>,
        has_data: bool,
    }

    pub struct SpeakerStream {
        consumer: HeapCons<f32>,
        _device: ca::hardware::StartedDevice<ca::AggregateDevice>,
        _ctx: Box<Ctx>,
        _tap: ca::TapGuard,
        waker_state: Arc<Mutex<WakerState>>,
        current_sample_rate: Arc<AtomicU32>,
        read_buffer: Vec<f32>,
    }

    impl SpeakerStream {
        pub fn sample_rate(&self) -> u32 {
            self.current_sample_rate.load(Ordering::Acquire)
        }
    }

    struct Ctx {
        format: arc::R<av::AudioFormat>,
        producer: HeapProd<f32>,
        waker_state: Arc<Mutex<WakerState>>,
        current_sample_rate: Arc<AtomicU32>,
    }

    impl SpeakerInput {
        pub fn new() -> Result<Self, crate::AudioError> {
            tracing::info!("Creating SpeakerInput with Core Audio Tap");
            let tap_desc = ca::TapDesc::with_mono_global_tap_excluding_processes(&ns::Array::new());
            let tap = tap_desc.create_process_tap().map_err(|e| {
                tracing::error!("Failed to create audio tap: {:?}", e);
                crate::AudioError::StreamError(format!("Failed to create audio tap: {:?}", e))
            })?;
            tracing::info!("Audio tap created successfully, sample_rate: {}", tap.asbd().map(|a| a.sample_rate).unwrap_or(0.0));

            let sub_tap = cf::DictionaryOf::with_keys_values(
                &[ca::sub_device_keys::uid()],
                &[tap.uid().unwrap().as_type_ref()],
            );

            let agg_desc = cf::DictionaryOf::with_keys_values(
                &[
                    agg_keys::is_private(),
                    agg_keys::tap_auto_start(),
                    agg_keys::name(),
                    agg_keys::uid(),
                    agg_keys::tap_list(),
                ],
                &[
                    cf::Boolean::value_true().as_type_ref(),
                    cf::Boolean::value_true().as_type_ref(),
                    cf::String::from_str(TAP_DEVICE_NAME).as_ref(),
                    &cf::Uuid::new().to_cf_string(),
                    &cf::ArrayOf::from_slice(&[sub_tap.as_ref()]),
                ],
            );

            Ok(Self { tap, agg_desc })
        }

        pub fn sample_rate(&self) -> u32 {
            self.tap.asbd().unwrap().sample_rate as u32
        }

        fn start_device(
            &self,
            ctx: &mut Box<Ctx>,
        ) -> Result<ca::hardware::StartedDevice<ca::AggregateDevice>, crate::AudioError> {
            extern "C" fn proc(
                device: ca::Device,
                _now: &cat::AudioTimeStamp,
                input_data: &cat::AudioBufList<1>,
                _input_time: &cat::AudioTimeStamp,
                _output_data: &mut cat::AudioBufList<1>,
                _output_time: &cat::AudioTimeStamp,
                ctx: Option<&mut Ctx>,
            ) -> os::Status {
                static PROC_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                let proc_count = PROC_COUNTER.fetch_add(1, Ordering::Relaxed);

                if proc_count == 0 {
                    tracing::info!("speaker_proc: first callback received!");
                }
                if proc_count % 500 == 0 {
                    tracing::info!(count = proc_count, "speaker_proc: callback count");
                }

                let ctx = ctx.unwrap();

                let after = device
                    .nominal_sample_rate()
                    .unwrap_or(ctx.format.absd().sample_rate) as u32;
                let before = ctx.current_sample_rate.load(Ordering::Acquire);

                if before != after {
                    ctx.current_sample_rate.store(after, Ordering::Release);
                    tracing::info!(before = before, after = after, "sample_rate_changed");
                }

                let first_buffer = &input_data.buffers[0];
                if proc_count < 5 {
                    tracing::info!(
                        buffer_size = first_buffer.data_bytes_size,
                        data_null = first_buffer.data.is_null(),
                        format = ?ctx.format.common_format(),
                        "speaker_proc: buffer info"
                    );
                }

                if let Some(view) =
                    av::AudioPcmBuf::with_buf_list_no_copy(&ctx.format, input_data, None)
                {
                    if let Some(data) = view.data_f32_at(0) {
                        if proc_count < 5 {
                            tracing::info!(samples = data.len(), "speaker_proc: got f32 data via view");
                        }
                        process_audio_data(ctx, data);
                    } else if proc_count < 5 {
                        tracing::warn!("speaker_proc: view created but no f32 data");
                    }
                } else {
                    if first_buffer.data_bytes_size == 0 || first_buffer.data.is_null() {
                        return os::Status::NO_ERR;
                    }

                    if proc_count < 5 {
                        tracing::info!("speaker_proc: fallback to manual format handling");
                    }

                    match ctx.format.common_format() {
                        av::audio::CommonFormat::PcmF32 => {
                            process_samples(ctx, first_buffer, |sample: f32| sample);
                        }
                        av::audio::CommonFormat::PcmF64 => {
                            process_samples(ctx, first_buffer, |sample: f64| sample as f32);
                        }
                        av::audio::CommonFormat::PcmI32 => {
                            let scale = i32::MAX as f32;
                            process_samples(ctx, first_buffer, move |sample: i32| {
                                if sample == i32::MIN {
                                    -1.0
                                } else {
                                    sample as f32 / scale
                                }
                            });
                        }
                        av::audio::CommonFormat::PcmI16 => {
                            let scale = i16::MAX as f32;
                            process_samples(ctx, first_buffer, move |sample: i16| {
                                if sample == i16::MIN {
                                    -1.0
                                } else {
                                    sample as f32 / scale
                                }
                            });
                        }
                        _ => {
                            if proc_count < 5 {
                                tracing::warn!(format = ?ctx.format.common_format(), "speaker_proc: unsupported format");
                            }
                        }
                    }
                }

                os::Status::NO_ERR
            }

            tracing::info!("speaker: creating aggregate device");
            let agg_device = ca::AggregateDevice::with_desc(&self.agg_desc).map_err(|e| {
                tracing::error!("speaker: failed to create aggregate device: {:?}", e);
                crate::AudioError::StreamError(format!("Failed to create aggregate device: {:?}", e))
            })?;
            tracing::info!("speaker: aggregate device created, creating IO proc");
            let proc_id = agg_device.create_io_proc_id(proc, Some(ctx)).map_err(|e| {
                tracing::error!("speaker: failed to create IO proc: {:?}", e);
                crate::AudioError::StreamError(format!("Failed to create IO proc: {:?}", e))
            })?;
            tracing::info!("speaker: IO proc created, starting device");
            let started_device = ca::device_start(agg_device, Some(proc_id)).map_err(|e| {
                tracing::error!("speaker: failed to start device: {:?}", e);
                crate::AudioError::StreamError(format!("Failed to start device: {:?}", e))
            })?;
            tracing::info!("speaker: device started successfully");

            Ok(started_device)
        }

        pub fn stream(self) -> Result<SpeakerStream, crate::AudioError> {
            tracing::info!("Starting speaker stream");
            let asbd = self.tap.asbd().map_err(|e| {
                tracing::error!("Failed to get ASBD: {:?}", e);
                crate::AudioError::StreamError(format!("Failed to get ASBD: {:?}", e))
            })?;
            tracing::info!("ASBD: sample_rate={}, channels={}, format={:?}",
                asbd.sample_rate, asbd.channels_per_frame, asbd.format);

            let format = av::AudioFormat::with_asbd(&asbd).ok_or_else(|| {
                crate::AudioError::StreamError("Failed to create audio format".to_string())
            })?;

            let buffer_size = CHUNK_SIZE * 16; // ~1 second at 48kHz
            let rb = HeapRb::<f32>::new(buffer_size);
            let (producer, consumer) = rb.split();

            let waker_state = Arc::new(Mutex::new(WakerState {
                waker: None,
                has_data: false,
            }));

            let current_sample_rate = Arc::new(AtomicU32::new(asbd.sample_rate as u32));
            tracing::info!(init = asbd.sample_rate, "speaker_stream_sample_rate");

            let mut ctx = Box::new(Ctx {
                format,
                producer,
                waker_state: waker_state.clone(),
                current_sample_rate: current_sample_rate.clone(),
            });

            let device = self.start_device(&mut ctx)?;

            Ok(SpeakerStream {
                consumer,
                _device: device,
                _ctx: ctx,
                _tap: self.tap,
                waker_state,
                current_sample_rate,
                read_buffer: vec![0.0f32; CHUNK_SIZE],
            })
        }
    }

    fn read_samples<T: Copy>(buffer: &cat::AudioBuf) -> Option<&[T]> {
        let byte_count = buffer.data_bytes_size as usize;

        if byte_count == 0 || buffer.data.is_null() {
            return None;
        }

        let sample_count = byte_count / std::mem::size_of::<T>();
        if sample_count == 0 {
            return None;
        }

        Some(unsafe { std::slice::from_raw_parts(buffer.data as *const T, sample_count) })
    }

    fn process_samples<T, F>(ctx: &mut Ctx, buffer: &cat::AudioBuf, mut convert: F)
    where
        T: Copy + 'static,
        F: FnMut(T) -> f32,
    {
        if let Some(samples) = read_samples::<T>(buffer) {
            if samples.is_empty() {
                return;
            }

            if TypeId::of::<T>() == TypeId::of::<f32>() {
                let data = unsafe {
                    std::slice::from_raw_parts(samples.as_ptr() as *const f32, samples.len())
                };
                process_audio_data(ctx, data);
                return;
            }

            let mut converted = Vec::with_capacity(samples.len());
            for sample in samples {
                converted.push(convert(*sample));
            }
            if !converted.is_empty() {
                process_audio_data(ctx, &converted);
            }
        }
    }

    fn process_audio_data(ctx: &mut Ctx, data: &[f32]) {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        if count % 100 == 0 {
            tracing::debug!("process_audio_data: received {} samples (call #{})", data.len(), count);
        }

        let pushed = ctx.producer.push_slice(data);

        if pushed < data.len() {
            let dropped = data.len() - pushed;
            tracing::warn!(dropped, "samples_dropped");
        }

        if pushed > 0 {
            let should_wake = {
                let mut waker_state = ctx.waker_state.lock().unwrap();
                let had_waker = waker_state.waker.is_some();
                let was_has_data = waker_state.has_data;

                if !waker_state.has_data {
                    waker_state.has_data = true;
                    let waker = waker_state.waker.take();
                    if count < 5 {
                        tracing::info!(
                            had_waker = had_waker,
                            was_has_data = was_has_data,
                            will_wake = waker.is_some(),
                            "process_audio_data: waker state"
                        );
                    }
                    waker
                } else {
                    if count < 5 {
                        tracing::info!(
                            had_waker = had_waker,
                            was_has_data = was_has_data,
                            "process_audio_data: skipping wake (has_data=true)"
                        );
                    }
                    None
                }
            };

            if let Some(waker) = should_wake {
                if count < 5 {
                    tracing::info!("process_audio_data: calling waker.wake()");
                }
                waker.wake();
            }
        }
    }

    impl Stream for SpeakerStream {
        type Item = Vec<f32>;

        fn poll_next(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> Poll<Option<Self::Item>> {
            static POLL_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let poll_count = POLL_COUNTER.fetch_add(1, Ordering::Relaxed);

            let this = self.as_mut().get_mut();
            let popped = this.consumer.pop_slice(&mut this.read_buffer);

            if poll_count < 10 {
                tracing::info!(poll_count = poll_count, popped = popped, "SpeakerStream::poll_next");
            }

            if popped > 0 {
                return Poll::Ready(Some(this.read_buffer[..popped].to_vec()));
            }

            {
                let mut state = this.waker_state.lock().unwrap();
                state.has_data = false;
                state.waker = Some(cx.waker().clone());
                if poll_count < 10 {
                    tracing::info!("SpeakerStream::poll_next: registered waker, returning Pending");
                }
            }

            Poll::Pending
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::{SpeakerInput, SpeakerStream, TAP_DEVICE_NAME};

#[cfg(not(target_os = "macos"))]
pub struct SpeakerInput;

#[cfg(not(target_os = "macos"))]
pub struct SpeakerStream;

#[cfg(not(target_os = "macos"))]
impl SpeakerInput {
    pub fn new() -> Result<Self, crate::AudioError> {
        Err(crate::AudioError::StreamError(
            "System audio capture not supported on this platform".to_string(),
        ))
    }
}

#[cfg(not(target_os = "macos"))]
impl SpeakerStream {
    pub fn sample_rate(&self) -> u32 {
        0
    }
}

#[cfg(not(target_os = "macos"))]
impl futures::Stream for SpeakerStream {
    type Item = Vec<f32>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::task::Poll::Ready(None)
    }
}

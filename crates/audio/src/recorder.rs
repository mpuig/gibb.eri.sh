use hound::{WavSpec, WavWriter};
use std::io::BufWriter;
use std::path::Path;
use std::sync::{Arc, Mutex};

const SAMPLE_RATE: u32 = 16000;

pub struct AudioRecorder {
    samples: Arc<Mutex<Vec<f32>>>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            samples: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn push_samples(&self, samples: &[f32]) {
        self.samples
            .lock()
            .expect("audio recorder mutex poisoned")
            .extend_from_slice(samples);
    }

    pub fn sample_count(&self) -> usize {
        self.samples
            .lock()
            .expect("audio recorder mutex poisoned")
            .len()
    }

    pub fn duration_secs(&self) -> f32 {
        self.sample_count() as f32 / SAMPLE_RATE as f32
    }

    pub fn get_samples(&self) -> Vec<f32> {
        self.samples
            .lock()
            .expect("audio recorder mutex poisoned")
            .clone()
    }

    pub fn clear(&self) {
        self.samples
            .lock()
            .expect("audio recorder mutex poisoned")
            .clear();
    }

    /// Trim the buffer to keep only the last `duration_secs` of audio.
    /// Used for rolling buffer in listen-only mode.
    pub fn trim_to_duration(&self, duration_secs: f32) {
        let max_samples = (duration_secs * SAMPLE_RATE as f32) as usize;
        let mut samples = self.samples.lock().expect("audio recorder mutex poisoned");
        if samples.len() > max_samples {
            let excess = samples.len() - max_samples;
            samples.drain(..excess);
        }
    }

    pub fn save_wav(&self, path: impl AsRef<Path>) -> crate::Result<()> {
        let samples = self.samples.lock().expect("audio recorder mutex poisoned");
        let spec = WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let file = std::fs::File::create(path.as_ref())
            .map_err(|e| crate::AudioError::StreamError(format!("failed to create file: {e}")))?;
        let mut writer = WavWriter::new(BufWriter::new(file), spec).map_err(|e| {
            crate::AudioError::StreamError(format!("failed to create wav writer: {e}"))
        })?;

        for &sample in samples.iter() {
            let int_sample = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            writer.write_sample(int_sample).map_err(|e| {
                crate::AudioError::StreamError(format!("failed to write sample: {e}"))
            })?;
        }

        writer
            .finalize()
            .map_err(|e| crate::AudioError::StreamError(format!("failed to finalize wav: {e}")))?;

        Ok(())
    }
}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AudioRecorder {
    fn clone(&self) -> Self {
        Self {
            samples: Arc::clone(&self.samples),
        }
    }
}

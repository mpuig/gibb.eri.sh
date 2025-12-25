mod device;
mod recorder;
mod stream;

#[cfg(target_os = "macos")]
mod speaker;

pub use device::{
    find_device_by_id, find_virtual_device, get_default_device, list_devices, AudioDevice,
    DeviceType,
};
pub use recorder::AudioRecorder;
pub use stream::{AudioSource, AudioStream};

#[cfg(target_os = "macos")]
pub use speaker::{SpeakerInput, SpeakerStream, TAP_DEVICE_NAME};

pub const SAMPLE_RATE: u32 = 16000;

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("device not found: {0}")]
    DeviceNotFound(String),
    #[error("permission denied")]
    PermissionDenied,
    #[error("stream error: {0}")]
    StreamError(String),
    #[error("device error: {0}")]
    DeviceError(#[from] cpal::DevicesError),
    #[error("build stream error: {0}")]
    BuildStreamError(#[from] cpal::BuildStreamError),
}

pub type Result<T> = std::result::Result<T, AudioError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_devices() {
        let devices = list_devices().unwrap();
        println!("Found {} audio devices:", devices.len());
        for device in &devices {
            println!("  - {} (default: {})", device.name, device.is_default);
        }
    }

    #[test]
    fn test_recorder() {
        let recorder = AudioRecorder::new();
        assert_eq!(recorder.sample_count(), 0);

        recorder.push_samples(&[0.0, 0.5, -0.5, 1.0]);
        assert_eq!(recorder.sample_count(), 4);
        assert!((recorder.duration_secs() - 4.0 / 16000.0).abs() < 0.0001);

        let samples = recorder.get_samples();
        assert_eq!(samples.len(), 4);

        recorder.clear();
        assert_eq!(recorder.sample_count(), 0);
    }
}

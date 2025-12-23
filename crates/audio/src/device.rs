use cpal::traits::{DeviceTrait, HostTrait};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DeviceType {
    Physical,
    Virtual,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub device_type: DeviceType,
}

impl AudioDevice {
    pub fn is_virtual(&self) -> bool {
        self.device_type == DeviceType::Virtual
    }
}

const VIRTUAL_DEVICE_PATTERNS: &[&str] = &[
    "blackhole",
    "soundflower",
    "loopback",
    "virtual",
    "vb-audio",
    "voicemeeter",
    "cable",
];

fn detect_device_type(name: &str) -> DeviceType {
    let lower = name.to_lowercase();
    if VIRTUAL_DEVICE_PATTERNS.iter().any(|p| lower.contains(p)) {
        DeviceType::Virtual
    } else {
        DeviceType::Physical
    }
}

pub fn list_devices() -> crate::Result<Vec<AudioDevice>> {
    let host = cpal::default_host();
    let default_device = host.default_input_device();
    let default_name = default_device
        .as_ref()
        .and_then(|d| d.name().ok());

    let mut devices = Vec::new();
    for device in host.input_devices()? {
        let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        let is_default = default_name.as_ref() == Some(&name);
        let device_type = detect_device_type(&name);
        devices.push(AudioDevice {
            id: name.clone(),
            name,
            is_default,
            device_type,
        });
    }

    Ok(devices)
}

pub fn get_default_device() -> crate::Result<Option<AudioDevice>> {
    let host = cpal::default_host();
    match host.default_input_device() {
        Some(device) => {
            let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
            let device_type = detect_device_type(&name);
            Ok(Some(AudioDevice {
                id: name.clone(),
                name,
                is_default: true,
                device_type,
            }))
        }
        None => Ok(None),
    }
}

pub fn find_virtual_device() -> crate::Result<Option<AudioDevice>> {
    let devices = list_devices()?;
    Ok(devices.into_iter().find(|d| d.is_virtual()))
}

pub fn find_device_by_id(id: &str) -> crate::Result<Option<AudioDevice>> {
    let devices = list_devices()?;
    Ok(devices.into_iter().find(|d| d.id == id))
}

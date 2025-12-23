use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum DetectEvent {
    #[serde(rename = "micStarted")]
    MicStarted {
        key: String,
        apps: Vec<gibberish_detect::InstalledApp>,
    },
    #[serde(rename = "micStopped")]
    MicStopped {
        apps: Vec<gibberish_detect::InstalledApp>,
    },
}

impl From<gibberish_detect::DetectEvent> for DetectEvent {
    fn from(event: gibberish_detect::DetectEvent) -> Self {
        match event {
            gibberish_detect::DetectEvent::MicStarted(apps) => Self::MicStarted {
                key: uuid::Uuid::new_v4().to_string(),
                apps,
            },
            gibberish_detect::DetectEvent::MicStopped(apps) => Self::MicStopped { apps },
        }
    }
}

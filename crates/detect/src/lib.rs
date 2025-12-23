mod list;
mod mic;
mod utils;

pub use list::*;
pub use mic::*;
use utils::*;

#[derive(Debug, Clone)]
pub enum DetectEvent {
    MicStarted(Vec<InstalledApp>),
    MicStopped(Vec<InstalledApp>),
}

pub type DetectCallback = std::sync::Arc<dyn Fn(DetectEvent) + Send + Sync + 'static>;

pub fn new_callback<F>(f: F) -> DetectCallback
where
    F: Fn(DetectEvent) + Send + Sync + 'static,
{
    std::sync::Arc::new(f)
}

trait Observer: Send + Sync {
    fn start(&mut self, f: DetectCallback);
    fn stop(&mut self);
}

#[derive(Default)]
pub struct Detector {
    mic_detector: MicDetector,
}

impl Detector {
    pub fn start(&mut self, f: DetectCallback) {
        self.mic_detector.start(f.clone());
    }

    pub fn stop(&mut self) {
        self.mic_detector.stop();
    }
}

/// List of meeting app bundle IDs to detect
pub const MEETING_APPS: &[&str] = &[
    "us.zoom.xos",
    "Cisco-Systems.Spark",
    "com.microsoft.teams",
    "com.microsoft.teams2",
    "com.google.Chrome",
    "com.brave.Browser",
    "com.apple.Safari",
    "org.mozilla.firefox",
    "com.discord.Discord",
    "com.slack.Slack",
];

/// Check if an app is a known meeting app
pub fn is_meeting_app(bundle_id: &str) -> bool {
    MEETING_APPS.contains(&bundle_id)
}

/// Filter apps to only include meeting apps
pub fn filter_meeting_apps(apps: Vec<InstalledApp>) -> Vec<InstalledApp> {
    apps.into_iter()
        .filter(|app| is_meeting_app(&app.id))
        .collect()
}

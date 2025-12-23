pub const SAMPLE_RATE: usize = 16000;

/// Minimum new audio (in samples) before triggering transcription
pub const TRANSCRIBE_THRESHOLD: usize = SAMPLE_RATE * 2; // 2 seconds

/// Buffer duration (in samples) that triggers word commit
pub const COMMIT_THRESHOLD: usize = SAMPLE_RATE * 6; // 6 seconds

/// Words ending before this time (ms) from buffer end are considered stable
pub const STABLE_WINDOW_MS: u64 = 4000; // 4 seconds

/// Padding (in ms) to keep before first unstable word when trimming
pub const TRIM_PADDING_MS: u64 = 500;

pub const SAMPLE_RATE: usize = 16000;

/// Minimum new audio (in samples) before triggering transcription.
/// Lower = more responsive but higher CPU. Modal/Sherpa uses ~200ms.
pub const TRANSCRIBE_THRESHOLD: usize = SAMPLE_RATE / 4; // 250ms (was 1s)

/// Maximum buffer duration (in samples) to bound hypothesis variance.
/// Longer buffers cause more "dancing" text. Sherpa C++ uses 5s.
pub const MAX_BUFFER_SAMPLES: usize = SAMPLE_RATE * 5; // 5 seconds

/// Buffer duration (in samples) that triggers word commit.
/// Reduced since we now commit more aggressively on VAD boundaries.
pub const COMMIT_THRESHOLD: usize = SAMPLE_RATE * 4; // 4 seconds (was 6s)

/// Words ending before this time (ms) from buffer end are considered stable
pub const STABLE_WINDOW_MS: u64 = 3000; // 3 seconds (was 4s)

/// Padding (in ms) to keep before first unstable word when trimming
pub const TRIM_PADDING_MS: u64 = 500;

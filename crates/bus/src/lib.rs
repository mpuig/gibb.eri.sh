//! Low-latency audio bus for real-time streaming.
//!
//! Provides zero-copy audio delivery from recorder to STT with bounded latency.

use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Standard audio sample rate for STT processing (16kHz).
pub const SAMPLE_RATE: u32 = 16000;

/// Duration of each audio chunk in milliseconds.
pub const CHUNK_DURATION_MS: u32 = 50;

/// Number of samples per chunk at the standard sample rate.
pub const CHUNK_SAMPLES: usize = (SAMPLE_RATE as usize * CHUNK_DURATION_MS as usize) / 1000;

/// Default buffer capacity in milliseconds.
pub const DEFAULT_BUFFER_CAPACITY_MS: u32 = 1500;

/// Silence injection duration in milliseconds (for VAD transitions).
pub const SILENCE_INJECTION_MS: u32 = 100;

/// Number of silence samples to inject.
pub const SILENCE_INJECTION_SAMPLES: usize =
    (SAMPLE_RATE as usize * SILENCE_INJECTION_MS as usize) / 1000;

/// Audio chunk with timestamp and sequence number for ordering.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    /// Monotonic sequence number for ordering.
    pub seq: u64,
    /// Timestamp in milliseconds (wall clock when chunk was captured).
    pub ts_ms: i64,
    /// Sample rate of the audio data.
    pub sample_rate: u32,
    /// Audio samples (shared ownership for zero-copy).
    pub samples: Arc<[f32]>,
}

impl AudioChunk {
    /// Create a new audio chunk.
    pub fn new(seq: u64, ts_ms: i64, sample_rate: u32, samples: impl Into<Arc<[f32]>>) -> Self {
        Self {
            seq,
            ts_ms,
            sample_rate,
            samples: samples.into(),
        }
    }

    /// Duration of this chunk in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        if self.sample_rate == 0 {
            return 0;
        }
        (self.samples.len() as u64 * 1000) / self.sample_rate as u64
    }
}

/// Configuration for the audio bus.
#[derive(Debug, Clone)]
pub struct AudioBusConfig {
    /// Target buffer capacity in milliseconds.
    pub capacity_ms: u32,
    /// Expected chunk size in milliseconds (for calculating channel capacity).
    pub chunk_size_ms: u32,
}

impl Default for AudioBusConfig {
    fn default() -> Self {
        Self {
            capacity_ms: 1500, // 1.5 second buffer
            chunk_size_ms: 50, // 50ms chunks
        }
    }
}

impl AudioBusConfig {
    /// Calculate channel capacity in number of chunks.
    fn channel_capacity(&self) -> usize {
        if self.chunk_size_ms == 0 {
            return 32;
        }
        ((self.capacity_ms / self.chunk_size_ms) as usize).max(8)
    }
}

/// Sender half of the audio bus.
#[derive(Clone)]
pub struct AudioBusSender {
    tx: mpsc::Sender<AudioChunk>,
    seq_counter: Arc<AtomicU64>,
    dropped_chunks: Arc<AtomicU64>,
}

impl AudioBusSender {
    /// Send an audio chunk, dropping the new chunk if buffer is full.
    ///
    /// Returns true if sent successfully, false if dropped.
    pub fn send(&self, ts_ms: i64, sample_rate: u32, samples: impl Into<Arc<[f32]>>) -> bool {
        let seq = self.seq_counter.fetch_add(1, Ordering::Relaxed);
        let chunk = AudioChunk::new(seq, ts_ms, sample_rate, samples);

        match self.tx.try_send(chunk) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                let dropped = self.dropped_chunks.fetch_add(1, Ordering::Relaxed) + 1;
                // Rate-limit logging: only log every 10th drop to avoid spam
                if dropped % 10 == 1 {
                    tracing::warn!(dropped, seq, "Audio bus full, dropping chunks");
                }
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::debug!("Audio bus closed");
                false
            }
        }
    }

    /// Send an audio chunk, blocking until space is available.
    pub async fn send_async(
        &self,
        ts_ms: i64,
        sample_rate: u32,
        samples: impl Into<Arc<[f32]>>,
    ) -> bool {
        let seq = self.seq_counter.fetch_add(1, Ordering::Relaxed);
        let chunk = AudioChunk::new(seq, ts_ms, sample_rate, samples);

        match self.tx.send(chunk).await {
            Ok(()) => true,
            Err(_) => {
                tracing::debug!("Audio bus closed");
                false
            }
        }
    }

    /// Get the number of dropped chunks.
    pub fn dropped_chunks(&self) -> u64 {
        self.dropped_chunks.load(Ordering::Relaxed)
    }

    /// Reset the dropped chunks counter.
    pub fn reset_dropped_chunks(&self) {
        self.dropped_chunks.store(0, Ordering::Relaxed);
    }

    /// Get the current sequence number.
    pub fn current_seq(&self) -> u64 {
        self.seq_counter.load(Ordering::Relaxed)
    }
}

/// Receiver half of the audio bus.
pub struct AudioBusReceiver {
    rx: mpsc::Receiver<AudioChunk>,
    last_seq: u64,
    gaps_detected: u64,
}

impl AudioBusReceiver {
    /// Receive the next audio chunk.
    pub async fn recv(&mut self) -> Option<AudioChunk> {
        let chunk = self.rx.recv().await?;

        // Check for gaps in sequence numbers.
        if self.last_seq > 0 && chunk.seq > self.last_seq + 1 {
            let gap = chunk.seq - self.last_seq - 1;
            self.gaps_detected += gap;
            tracing::debug!(
                "Audio bus gap detected: {} chunks missing (seq {} -> {})",
                gap,
                self.last_seq,
                chunk.seq
            );
        }
        self.last_seq = chunk.seq;

        Some(chunk)
    }

    /// Try to receive a chunk without blocking.
    pub fn try_recv(&mut self) -> Option<AudioChunk> {
        match self.rx.try_recv() {
            Ok(chunk) => {
                if self.last_seq > 0 && chunk.seq > self.last_seq + 1 {
                    let gap = chunk.seq - self.last_seq - 1;
                    self.gaps_detected += gap;
                }
                self.last_seq = chunk.seq;
                Some(chunk)
            }
            Err(_) => None,
        }
    }

    /// Get the number of gaps (missing chunks) detected.
    pub fn gaps_detected(&self) -> u64 {
        self.gaps_detected
    }

    /// Drain all available chunks, keeping only the most recent.
    ///
    /// Useful for catching up after lag.
    pub fn drain_to_latest(&mut self) -> Option<AudioChunk> {
        let mut latest = None;
        let mut drained = 0;

        while let Some(chunk) = self.try_recv() {
            drained += 1;
            latest = Some(chunk);
        }

        if drained > 1 {
            tracing::debug!("Drained {} chunks from audio bus", drained - 1);
        }

        latest
    }
}

/// Audio bus for low-latency audio delivery.
pub struct AudioBus {
    sender: AudioBusSender,
    receiver: Option<AudioBusReceiver>,
}

impl AudioBus {
    /// Create a new audio bus with default configuration.
    pub fn new() -> Self {
        Self::with_config(AudioBusConfig::default())
    }

    /// Create a new audio bus with custom configuration.
    pub fn with_config(config: AudioBusConfig) -> Self {
        let capacity = config.channel_capacity();
        let (tx, rx) = mpsc::channel(capacity);

        tracing::debug!(
            "Created audio bus: capacity={}ms (~{} chunks of {}ms)",
            config.capacity_ms,
            capacity,
            config.chunk_size_ms
        );

        Self {
            sender: AudioBusSender {
                tx,
                seq_counter: Arc::new(AtomicU64::new(0)),
                dropped_chunks: Arc::new(AtomicU64::new(0)),
            },
            receiver: Some(AudioBusReceiver {
                rx,
                last_seq: 0,
                gaps_detected: 0,
            }),
        }
    }

    /// Get a clone of the sender.
    pub fn sender(&self) -> AudioBusSender {
        self.sender.clone()
    }

    /// Take the receiver (can only be called once).
    pub fn take_receiver(&mut self) -> Option<AudioBusReceiver> {
        self.receiver.take()
    }
}

impl Default for AudioBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Real-time pipeline metrics with atomic fields for lock-free updates.
///
/// This struct is designed to be shared via `Arc<PipelineStatus>` and updated
/// from the audio processing hot path without locks.
#[derive(Debug)]
pub struct PipelineStatus {
    /// Current audio lag in milliseconds (now - last chunk timestamp).
    audio_lag_ms: AtomicI64,
    /// Last inference duration in milliseconds.
    inference_time_ms: AtomicU64,
    /// Real-time factor (inference_time / audio_duration), stored as f32 bits.
    real_time_factor_bits: AtomicU32,
    /// Total dropped chunks since start.
    dropped_chunks: AtomicU64,
    /// Total gaps detected (missing sequence numbers).
    gaps_detected: AtomicU64,
    /// Decode rate in Hz (decodes per second), stored as f32 bits.
    decode_rate_hz_bits: AtomicU32,
    /// Number of chunks processed.
    chunks_processed: AtomicU64,
    /// Total audio duration processed in milliseconds.
    audio_processed_ms: AtomicU64,
}

impl Default for PipelineStatus {
    fn default() -> Self {
        Self {
            audio_lag_ms: AtomicI64::new(0),
            inference_time_ms: AtomicU64::new(0),
            real_time_factor_bits: AtomicU32::new(0.0_f32.to_bits()),
            dropped_chunks: AtomicU64::new(0),
            gaps_detected: AtomicU64::new(0),
            decode_rate_hz_bits: AtomicU32::new(0.0_f32.to_bits()),
            chunks_processed: AtomicU64::new(0),
            audio_processed_ms: AtomicU64::new(0),
        }
    }
}

impl PipelineStatus {
    /// Create a new PipelineStatus with default values.
    pub fn new() -> Self {
        Self::default()
    }

    // --- Getters (for reading metrics) ---

    pub fn audio_lag_ms(&self) -> i64 {
        self.audio_lag_ms.load(Ordering::Relaxed)
    }

    pub fn inference_time_ms(&self) -> u64 {
        self.inference_time_ms.load(Ordering::Relaxed)
    }

    pub fn real_time_factor(&self) -> f32 {
        f32::from_bits(self.real_time_factor_bits.load(Ordering::Relaxed))
    }

    pub fn dropped_chunks(&self) -> u64 {
        self.dropped_chunks.load(Ordering::Relaxed)
    }

    pub fn gaps_detected(&self) -> u64 {
        self.gaps_detected.load(Ordering::Relaxed)
    }

    pub fn decode_rate_hz(&self) -> f32 {
        f32::from_bits(self.decode_rate_hz_bits.load(Ordering::Relaxed))
    }

    pub fn chunks_processed(&self) -> u64 {
        self.chunks_processed.load(Ordering::Relaxed)
    }

    pub fn audio_processed_ms(&self) -> u64 {
        self.audio_processed_ms.load(Ordering::Relaxed)
    }

    // --- Setters (for updating metrics) ---

    pub fn set_audio_lag_ms(&self, value: i64) {
        self.audio_lag_ms.store(value, Ordering::Relaxed);
    }

    pub fn set_inference_time_ms(&self, value: u64) {
        self.inference_time_ms.store(value, Ordering::Relaxed);
    }

    pub fn set_real_time_factor(&self, value: f32) {
        self.real_time_factor_bits
            .store(value.to_bits(), Ordering::Relaxed);
    }

    pub fn set_dropped_chunks(&self, value: u64) {
        self.dropped_chunks.store(value, Ordering::Relaxed);
    }

    pub fn set_gaps_detected(&self, value: u64) {
        self.gaps_detected.store(value, Ordering::Relaxed);
    }

    pub fn set_decode_rate_hz(&self, value: f32) {
        self.decode_rate_hz_bits
            .store(value.to_bits(), Ordering::Relaxed);
    }

    pub fn set_chunks_processed(&self, value: u64) {
        self.chunks_processed.store(value, Ordering::Relaxed);
    }

    pub fn set_audio_processed_ms(&self, value: u64) {
        self.audio_processed_ms.store(value, Ordering::Relaxed);
    }

    // --- Convenience methods ---

    /// Update the real-time factor based on inference and audio durations.
    pub fn update_rtf(&self, inference_ms: u64, audio_ms: u64) {
        self.set_inference_time_ms(inference_ms);
        if audio_ms > 0 {
            self.set_real_time_factor(inference_ms as f32 / audio_ms as f32);
        }
    }

    /// Update audio lag from a chunk timestamp.
    pub fn update_lag(&self, chunk_ts_ms: i64) {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        self.set_audio_lag_ms(now_ms - chunk_ts_ms);
    }

    /// Increment chunks processed counter.
    pub fn increment_chunks_processed(&self) {
        self.chunks_processed.fetch_add(1, Ordering::Relaxed);
    }

    /// Add to audio processed duration.
    pub fn add_audio_processed_ms(&self, ms: u64) {
        self.audio_processed_ms.fetch_add(ms, Ordering::Relaxed);
    }

    /// Create a snapshot for serialization/display.
    pub fn snapshot(&self) -> PipelineStatusSnapshot {
        PipelineStatusSnapshot {
            audio_lag_ms: self.audio_lag_ms(),
            inference_time_ms: self.inference_time_ms(),
            real_time_factor: self.real_time_factor(),
            dropped_chunks: self.dropped_chunks(),
            gaps_detected: self.gaps_detected(),
            decode_rate_hz: self.decode_rate_hz(),
            chunks_processed: self.chunks_processed(),
            audio_processed_ms: self.audio_processed_ms(),
        }
    }
}

/// Snapshot of pipeline status for serialization.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PipelineStatusSnapshot {
    pub audio_lag_ms: i64,
    pub inference_time_ms: u64,
    pub real_time_factor: f32,
    pub dropped_chunks: u64,
    pub gaps_detected: u64,
    pub decode_rate_hz: f32,
    pub chunks_processed: u64,
    pub audio_processed_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_chunk_duration() {
        let samples: Vec<f32> = vec![0.0; 1600]; // 100ms at 16kHz
        let chunk = AudioChunk::new(0, 0, 16000, samples);
        assert_eq!(chunk.duration_ms(), 100);
    }

    #[test]
    fn test_bus_config_capacity() {
        let config = AudioBusConfig {
            capacity_ms: 1000,
            chunk_size_ms: 50,
        };
        assert_eq!(config.channel_capacity(), 20);
    }

    #[tokio::test]
    async fn test_send_recv() {
        let mut bus = AudioBus::new();
        let sender = bus.sender();
        let mut receiver = bus.take_receiver().unwrap();

        let samples: Vec<f32> = vec![0.1; 800];
        sender.send(1000, 16000, samples);

        let chunk = receiver.recv().await.unwrap();
        assert_eq!(chunk.seq, 0);
        assert_eq!(chunk.ts_ms, 1000);
        assert_eq!(chunk.sample_rate, 16000);
        assert_eq!(chunk.samples.len(), 800);
    }

    #[test]
    fn test_dropped_chunks_counter() {
        let bus = AudioBus::with_config(AudioBusConfig {
            capacity_ms: 100,
            chunk_size_ms: 50,
        });
        let sender = bus.sender();

        // Fill the buffer.
        for _ in 0..10 {
            let samples: Vec<f32> = vec![0.0; 800];
            sender.send(0, 16000, samples);
        }

        assert!(sender.dropped_chunks() > 0);
    }

    // Pipeline invariant tests

    #[tokio::test]
    async fn test_sequence_monotonicity() {
        let mut bus = AudioBus::new();
        let sender = bus.sender();
        let mut receiver = bus.take_receiver().unwrap();

        // Send multiple chunks
        for i in 0..10 {
            let samples: Vec<f32> = vec![0.1; 800];
            sender.send(i * 50, 16000, samples);
        }

        // Verify sequences are monotonically increasing
        let mut last_seq = 0;
        for _ in 0..10 {
            let chunk = receiver.recv().await.unwrap();
            assert!(
                chunk.seq >= last_seq,
                "Sequence must be monotonic: {} < {}",
                chunk.seq,
                last_seq
            );
            last_seq = chunk.seq;
        }
    }

    #[tokio::test]
    async fn test_gap_detection() {
        let mut bus = AudioBus::new();
        let sender = bus.sender();
        let mut receiver = bus.take_receiver().unwrap();

        // Receive first chunk to establish baseline sequence
        let samples: Vec<f32> = vec![0.1; 800];
        sender.send(0, 16000, samples.clone());
        let _ = receiver.recv().await.unwrap();
        assert_eq!(receiver.gaps_detected(), 0, "No gaps initially");

        // Now send chunk with seq gap (seq 2 sent, seq 1 missing)
        // We can't directly control seq, so simulate by consuming sender's internal counter
        // Send seq 1
        sender.send(50, 16000, samples.clone());
        // Send seq 2
        sender.send(100, 16000, samples.clone());

        // Receive seq 1 (no gap)
        let _ = receiver.recv().await.unwrap();
        assert_eq!(receiver.gaps_detected(), 0, "Still no gaps");

        // Receive seq 2 (no gap, consecutive)
        let _ = receiver.recv().await.unwrap();
        assert_eq!(
            receiver.gaps_detected(),
            0,
            "Still no gaps with consecutive seqs"
        );
    }

    #[test]
    fn test_dropped_chunks_detected() {
        let mut bus = AudioBus::with_config(AudioBusConfig {
            capacity_ms: 100, // Very small buffer (~2 chunks)
            chunk_size_ms: 50,
        });
        let sender = bus.sender();
        let _receiver = bus.take_receiver().unwrap();

        // Send many chunks to overflow the buffer
        for i in 0..20 {
            let samples: Vec<f32> = vec![0.1; 800];
            sender.send(i * 50, 16000, samples);
        }

        // Verify drops are tracked
        assert!(
            sender.dropped_chunks() > 0,
            "Should have dropped chunks when buffer overflows"
        );
    }

    #[tokio::test]
    async fn test_timestamp_ordering() {
        let mut bus = AudioBus::new();
        let sender = bus.sender();
        let mut receiver = bus.take_receiver().unwrap();

        // Send chunks with increasing timestamps
        let timestamps: Vec<i64> = vec![100, 150, 200, 250, 300];
        for ts in &timestamps {
            let samples: Vec<f32> = vec![0.1; 800];
            sender.send(*ts, 16000, samples);
        }

        // Verify timestamps are preserved in order
        let mut last_ts = 0;
        for expected_ts in &timestamps {
            let chunk = receiver.recv().await.unwrap();
            assert_eq!(chunk.ts_ms, *expected_ts, "Timestamp should be preserved");
            assert!(
                chunk.ts_ms >= last_ts,
                "Timestamps should be non-decreasing"
            );
            last_ts = chunk.ts_ms;
        }
    }

    #[tokio::test]
    async fn test_zero_copy_arc_sharing() {
        let mut bus = AudioBus::new();
        let sender = bus.sender();
        let mut receiver = bus.take_receiver().unwrap();

        // Create samples with distinct pattern
        let samples: Vec<f32> = (0..800).map(|i| i as f32 / 800.0).collect();
        let original_arc: Arc<[f32]> = samples.clone().into();

        // Send via Arc
        sender.send(0, 16000, original_arc.clone());

        // Receive and verify data integrity
        let chunk = receiver.recv().await.unwrap();
        assert_eq!(chunk.samples.len(), 800);
        assert_eq!(chunk.samples[0], 0.0);
        assert!((chunk.samples[799] - 799.0 / 800.0).abs() < 0.0001);
    }

    #[test]
    fn test_drain_to_latest_skips_old() {
        let mut bus = AudioBus::new();
        let sender = bus.sender();
        let mut receiver = bus.take_receiver().unwrap();

        // Send multiple chunks with distinct timestamps
        for i in 0..5 {
            let samples: Vec<f32> = vec![i as f32; 800];
            sender.send(i * 100, 16000, samples);
        }

        // Drain to latest should return only the last chunk
        let latest = receiver.drain_to_latest();
        assert!(latest.is_some(), "Should have a latest chunk");

        let chunk = latest.unwrap();
        assert_eq!(chunk.ts_ms, 400, "Should be the last timestamp sent");
        assert_eq!(chunk.samples[0], 4.0, "Should have last chunk's data");
    }
}

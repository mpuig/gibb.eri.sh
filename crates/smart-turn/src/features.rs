//! Audio feature extraction for Smart Turn models.
//!
//! Computes log-mel spectrograms from raw audio, matching the Whisper feature extractor.

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::sync::{Arc, OnceLock};

const SAMPLE_RATE: usize = 16_000;
const MAX_SECONDS: usize = 8;
const N_SAMPLES: usize = SAMPLE_RATE * MAX_SECONDS; // 128000

const N_FFT: usize = 400;
const HOP: usize = 160;
const N_FREQ: usize = (N_FFT / 2) + 1; // 201
const N_MELS: usize = 80;

const PAD: usize = N_FFT / 2; // 200

const N_FRAMES_WITH_PAD: usize = 1 + (N_SAMPLES / HOP); // 801
const N_FRAMES: usize = N_FRAMES_WITH_PAD - 1; // 800

/// Output shape for the model input features.
pub const FEATURE_SHAPE: (usize, usize) = (N_MELS, N_FRAMES); // (80, 800)

/// Cached precomputed values for feature extraction.
struct CachedFeatureData {
    hann_window: Vec<f64>,
    mel_filters: Vec<Vec<f64>>,
    fft: Arc<dyn Fft<f64>>,
}

static CACHED_DATA: OnceLock<CachedFeatureData> = OnceLock::new();

fn get_cached_data() -> &'static CachedFeatureData {
    CACHED_DATA.get_or_init(|| {
        let mut planner = FftPlanner::<f64>::new();
        CachedFeatureData {
            hann_window: hann_window(N_FFT),
            mel_filters: mel_filter_bank_slaney(N_FREQ, N_MELS, SAMPLE_RATE, 0.0, 8000.0),
            fft: planner.plan_fft_forward(N_FFT),
        }
    })
}

/// Compute log-mel spectrogram features from 16kHz mono audio.
///
/// Matches the Whisper feature extractor pipeline:
/// 1. Truncate/pad to 8 seconds (keeping the end, padding with zeros at the start)
/// 2. Normalize to zero mean, unit variance
/// 3. Center-pad with reflection
/// 4. Compute STFT power spectrum
/// 5. Apply mel filterbank
/// 6. Log scale with clamping
pub fn compute_input_features(audio_16k_mono: &[f32]) -> Vec<f32> {
    let mut audio = truncate_or_left_pad(audio_16k_mono, N_SAMPLES);
    zero_mean_unit_var_norm(&mut audio);
    let padded = reflect_pad_1d(&audio, PAD);

    // Use cached precomputed data (hann window, mel filters, FFT plan)
    let cached = get_cached_data();
    let window = &cached.hann_window;
    let mel_filters = &cached.mel_filters;
    let fft = &cached.fft;

    let mut log_mel = vec![0.0f32; N_MELS * N_FRAMES_WITH_PAD];
    let mut frame_in: Vec<Complex<f64>> = vec![Complex { re: 0.0, im: 0.0 }; N_FFT];

    for frame_idx in 0..N_FRAMES_WITH_PAD {
        let start = frame_idx * HOP;
        let frame = &padded[start..start + N_FFT];

        for (out, (sample, win)) in frame_in.iter_mut().zip(frame.iter().zip(window.iter())) {
            out.re = (*sample as f64) * win;
            out.im = 0.0;
        }

        fft.process(&mut frame_in);

        let mut power = [0.0f64; N_FREQ];
        for (p, c) in power.iter_mut().zip(frame_in.iter().take(N_FREQ)) {
            *p = c.re * c.re + c.im * c.im;
        }

        for m in 0..N_MELS {
            let mut v = 0.0f64;
            for k in 0..N_FREQ {
                v += mel_filters[k][m] * power[k];
            }
            let v = v.max(1e-10).log10();
            log_mel[(m * N_FRAMES_WITH_PAD) + frame_idx] = v as f32;
        }
    }

    // Drop the last frame to match Whisper behavior
    let mut log_mel_final = vec![0.0f32; N_MELS * N_FRAMES];
    for m in 0..N_MELS {
        let src = &log_mel[m * N_FRAMES_WITH_PAD..m * N_FRAMES_WITH_PAD + N_FRAMES];
        let dst = &mut log_mel_final[m * N_FRAMES..(m + 1) * N_FRAMES];
        dst.copy_from_slice(src);
    }

    // Clamp to max-8 and scale
    let max_val = log_mel_final
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let floor = max_val - 8.0;
    for v in log_mel_final.iter_mut() {
        if *v < floor {
            *v = floor;
        }
        *v = (*v + 4.0) / 4.0;
    }

    log_mel_final
}

fn truncate_or_left_pad(audio: &[f32], n_samples: usize) -> Vec<f32> {
    if audio.len() > n_samples {
        audio[audio.len() - n_samples..].to_vec()
    } else if audio.len() < n_samples {
        let mut out = vec![0.0f32; n_samples - audio.len()];
        out.extend_from_slice(audio);
        out
    } else {
        audio.to_vec()
    }
}

fn zero_mean_unit_var_norm(x: &mut [f32]) {
    if x.is_empty() {
        return;
    }
    let mean = x.iter().map(|v| *v as f64).sum::<f64>() / x.len() as f64;
    let var = x
        .iter()
        .map(|v| {
            let d = (*v as f64) - mean;
            d * d
        })
        .sum::<f64>()
        / x.len() as f64;
    let denom = (var + 1e-7).sqrt();
    for v in x.iter_mut() {
        *v = ((*v as f64 - mean) / denom) as f32;
    }
}

fn reflect_pad_1d(x: &[f32], pad: usize) -> Vec<f32> {
    if pad == 0 {
        return x.to_vec();
    }
    if x.len() < pad + 1 {
        let mut out = vec![0.0; pad];
        out.extend_from_slice(x);
        out.extend(std::iter::repeat_n(0.0, pad));
        return out;
    }

    let mut out = Vec::with_capacity(x.len() + (2 * pad));
    for i in 0..pad {
        out.push(x[pad - i]);
    }
    out.extend_from_slice(x);
    for i in 0..pad {
        out.push(x[x.len() - 2 - i]);
    }
    out
}

fn hann_window(n: usize) -> Vec<f64> {
    let n_f = n as f64;
    (0..n)
        .map(|i| 0.5 - 0.5 * ((2.0 * std::f64::consts::PI * i as f64) / n_f).cos())
        .collect()
}

fn hertz_to_mel_slaney(freq: f64) -> f64 {
    let min_log_hertz = 1000.0;
    let min_log_mel = 15.0;
    let logstep = 27.0 / 6.4_f64.ln();
    let mut mels = 3.0 * freq / 200.0;
    if freq >= min_log_hertz {
        mels = min_log_mel + (freq / min_log_hertz).ln() * logstep;
    }
    mels
}

fn mel_to_hertz_slaney(mels: f64) -> f64 {
    let min_log_hertz = 1000.0;
    let min_log_mel = 15.0;
    let logstep = 6.4_f64.ln() / 27.0;
    let mut freq = 200.0 * mels / 3.0;
    if mels >= min_log_mel {
        freq = min_log_hertz * (logstep * (mels - min_log_mel)).exp();
    }
    freq
}

fn mel_filter_bank_slaney(
    num_frequency_bins: usize,
    num_mel_filters: usize,
    sampling_rate: usize,
    min_frequency: f64,
    max_frequency: f64,
) -> Vec<Vec<f64>> {
    let mel_min = hertz_to_mel_slaney(min_frequency);
    let mel_max = hertz_to_mel_slaney(max_frequency);

    let mut mel_freqs = Vec::with_capacity(num_mel_filters + 2);
    for i in 0..(num_mel_filters + 2) {
        let t = i as f64 / (num_mel_filters + 1) as f64;
        mel_freqs.push(mel_min + t * (mel_max - mel_min));
    }

    let mut filter_freqs = Vec::with_capacity(num_mel_filters + 2);
    for m in mel_freqs {
        filter_freqs.push(mel_to_hertz_slaney(m));
    }

    let nyquist = (sampling_rate as f64) / 2.0;
    let mut fft_freqs = Vec::with_capacity(num_frequency_bins);
    if num_frequency_bins == 1 {
        fft_freqs.push(0.0);
    } else {
        for i in 0..num_frequency_bins {
            let t = i as f64 / (num_frequency_bins - 1) as f64;
            fft_freqs.push(t * nyquist);
        }
    }

    let mut mel_filters = vec![vec![0.0f64; num_mel_filters]; num_frequency_bins];
    for f in 0..num_frequency_bins {
        let ff = fft_freqs[f];
        for m in 0..num_mel_filters {
            let f_left = filter_freqs[m];
            let f_center = filter_freqs[m + 1];
            let f_right = filter_freqs[m + 2];

            let down = (ff - f_left) / (f_center - f_left);
            let up = (f_right - ff) / (f_right - f_center);
            let v = down.min(up).max(0.0);
            mel_filters[f][m] = v;
        }
    }

    // Slaney area normalization
    for m in 0..num_mel_filters {
        let enorm = 2.0 / (filter_freqs[m + 2] - filter_freqs[m]);
        for row in &mut mel_filters {
            row[m] *= enorm;
        }
    }

    mel_filters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_or_left_pad_shorter() {
        let audio = vec![1.0, 2.0, 3.0];
        let result = truncate_or_left_pad(&audio, 5);
        assert_eq!(result, vec![0.0, 0.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_truncate_or_left_pad_longer() {
        let audio = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = truncate_or_left_pad(&audio, 3);
        assert_eq!(result, vec![3.0, 4.0, 5.0]);
    }

    #[test]
    fn test_truncate_or_left_pad_exact() {
        let audio = vec![1.0, 2.0, 3.0];
        let result = truncate_or_left_pad(&audio, 3);
        assert_eq!(result, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_zero_mean_unit_var_norm() {
        let mut x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        zero_mean_unit_var_norm(&mut x);

        let mean: f64 = x.iter().map(|v| *v as f64).sum::<f64>() / x.len() as f64;
        assert!(mean.abs() < 1e-5, "mean should be ~0, got {mean}");

        let var: f64 = x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len() as f64;
        assert!((var - 1.0).abs() < 1e-5, "variance should be ~1, got {var}");
    }

    #[test]
    fn test_reflect_pad_1d() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = reflect_pad_1d(&x, 2);
        assert_eq!(result, vec![3.0, 2.0, 1.0, 2.0, 3.0, 4.0, 5.0, 4.0, 3.0]);
    }

    #[test]
    fn test_hann_window_symmetry() {
        let window = hann_window(10);
        assert_eq!(window.len(), 10);
        assert!(window[0].abs() < 1e-10);
    }

    #[test]
    fn test_mel_conversion_roundtrip() {
        let freq = 1000.0;
        let mel = hertz_to_mel_slaney(freq);
        let back = mel_to_hertz_slaney(mel);
        assert!((freq - back).abs() < 1e-6);
    }

    #[test]
    fn test_compute_input_features_shape() {
        let audio = vec![0.0f32; N_SAMPLES];
        let features = compute_input_features(&audio);
        assert_eq!(features.len(), FEATURE_SHAPE.0 * FEATURE_SHAPE.1);
    }

    #[test]
    fn test_compute_input_features_short_audio() {
        let audio = vec![0.1f32; 8000]; // 0.5 seconds
        let features = compute_input_features(&audio);
        assert_eq!(features.len(), FEATURE_SHAPE.0 * FEATURE_SHAPE.1);
    }
}

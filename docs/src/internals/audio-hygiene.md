# Audio Hygiene

Bad microphones shouldn't mean bad transcripts. We fix what we can.

## The Problems

Consumer microphones vary wildly:
- Built-in laptop mics pick up fan noise
- USB mics have different gain settings
- Sample rates range from 8kHz to 96kHz
- Some mics clip, others are too quiet

Models expect clean, consistent 16kHz audio. We bridge the gap.

## Resampling

All models need 16kHz mono audio. Users have everything else.

### Why Sinc Interpolation?

```rust
use rubato::{FftFixedIn, Resampler};

let resampler = FftFixedIn::<f32>::new(
    input_rate,   // e.g., 44100
    16000,        // target
    chunk_size,
    2,            // sub-chunks
    1,            // channels
)?;
```

We use `rubato`'s FFT-based sinc resampling. Alternatives:

| Method | Quality | Speed | Our Use |
|--------|---------|-------|---------|
| Nearest neighbor | Terrible | Fast | Never |
| Linear | Poor | Fast | Never |
| Sinc (rubato) | Excellent | Medium | Yes |

Linear interpolation creates aliasing artifacts that sound "robotic." Speech recognition models weren't trained on robotic audio—they perform worse.

The CPU cost of proper resampling is negligible compared to inference.

## Automatic Gain Control

### The Problem

```
User A (quiet voice):     ▁▁▂▁▁▂▁ (signal barely visible)
User B (loud voice):      ▇▇█▇▇█▇ (signal clipping)
Model expects:            ▃▄▅▄▃▅▄ (normalized range)
```

### Our Solution

Soft-knee compression with `tanh`:

```rust
const TARGET_DB: f32 = -20.0;
const ATTACK_MS: f32 = 10.0;
const RELEASE_MS: f32 = 100.0;

pub struct Agc {
    gain: f32,
    target_rms: f32,
}

impl Agc {
    pub fn process(&mut self, samples: &mut [f32]) {
        let rms = calculate_rms(samples);
        let target_gain = self.target_rms / rms.max(1e-10);

        // Smooth gain changes to avoid clicks
        self.gain = lerp(self.gain, target_gain, self.smoothing);

        // Apply gain with soft clipping
        for sample in samples.iter_mut() {
            *sample = (*sample * self.gain).tanh();
        }
    }
}
```

The `tanh` function provides soft clipping—instead of hard clipping at ±1.0 (which sounds harsh), it smoothly compresses peaks.

### Target Level

We target **-20 dBFS**. Why?
- Leaves headroom for peaks
- Matches typical model training data
- Consistent across different mic gains

## DC Offset Removal

Some cheap mics have DC offset—the signal "floats" above or below zero:

```
Bad:   ▄▅▆▅▄▅▆▅▄▅  (offset from zero)
Good:  ▃▄▅▄▃▄▅▄▃▄  (centered on zero)
```

We use a simple high-pass filter:

```rust
const CUTOFF_HZ: f32 = 20.0; // Remove everything below 20Hz

pub fn remove_dc(samples: &mut [f32], state: &mut f32) {
    let alpha = 1.0 - (2.0 * PI * CUTOFF_HZ / 16000.0);
    for sample in samples.iter_mut() {
        let new_state = *sample + alpha * *state;
        *sample = new_state - *state;
        *state = new_state;
    }
}
```

## Noise Gate

We don't use one. Here's why:

Noise gates cut audio below a threshold. In theory, they reduce background noise. In practice:
1. They clip word beginnings ("hello" → "ello")
2. Silero VAD already handles speech detection
3. Models are trained on noisy data and handle it fine

If the environment is so noisy that VAD triggers incorrectly, a noise gate won't help—the user needs a better mic or quieter room.

## Preprocessing Pipeline

Audio flows through these stages in order:

```
Mic → DC Remove → Resample → AGC → Model
```

Each stage is independent and stateless (except AGC's smoothing state).

## Testing

We keep a collection of "pathological" audio files:
- Recorded at 8kHz
- Heavy background noise
- Extreme clipping
- Strong DC offset

CI runs inference on these files. If accuracy drops, we investigate.

## Code

- Resampling: `crates/audio/src/resample.rs`
- AGC: `crates/audio/src/agc.rs`
- Pipeline: `crates/audio/src/stream.rs`

# Implementation Details

This section documents implementation details that affect perceived responsiveness.

### [Simulated Streaming](./simulated-streaming.md)

Making batch models feel real-time.

### [Silence Injection](./silence-injection.md)

Prepending silence to prevent hallucinations.

### [Lock-Free Metrics](./atomic-metrics.md)

Using atomics for metrics instead of mutexes.

### [Threading Model](./threading.md)

Why we use `std::thread` instead of `tokio::spawn` for inference.

### [Audio Hygiene](./audio-hygiene.md)

Resampling and AGC for consistent input quality.

### [Meeting Detection](./meeting-detection.md)

Detecting when Zoom/Teams is running.

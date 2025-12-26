# Adding Languages

gibb.eri.sh can transcribe any language for which a model exists. Here's how to add one.

## Overview

1. Find a compatible model (CTC or Transducer)
2. Convert to ONNX format
3. Register in the model metadata
4. Test!

## Case Study: Adding Catalan

We added Catalan using a NeMo Conformer CTC model from Hugging Face.

### Step 1: Find a Model

Good sources:
- [Hugging Face Models](https://huggingface.co/models?pipeline_tag=automatic-speech-recognition)
- [NVIDIA NeMo](https://catalog.ngc.nvidia.com/models?filters=&orderBy=dateModifiedDESC&query=asr)

Look for:
- **CTC** or **Transducer** architecture (NOT encoder-decoder like Whisper)
- 16kHz sample rate
- Good accuracy on your target language

### Step 2: Convert to ONNX

Most models are in PyTorch format. We need ONNX for Sherpa.

#### For NeMo Models

We provide a conversion script:

```bash
cd scripts
python export_nemo_ctc.py \
    --model "path/to/model.nemo" \
    --output "catalan-nemo-ctc" \
    --language "ca"
```

This produces:
- `model.onnx` — The neural network
- `tokens.txt` — The vocabulary

#### What the Script Does

```python
import nemo.collections.asr as nemo_asr

# Load PyTorch model
model = nemo_asr.models.EncDecCTCModel.restore_from("model.nemo")

# Create dummy input for tracing
dummy_audio = torch.randn(1, 16000)  # 1 second of audio
dummy_length = torch.tensor([16000])

# Export to ONNX
torch.onnx.export(
    model,
    (dummy_audio, dummy_length),
    "model.onnx",
    input_names=["audio", "length"],
    output_names=["logits"],
    dynamic_axes={
        "audio": {0: "batch", 1: "time"},
        "length": {0: "batch"},
    },
)

# Extract vocabulary
with open("tokens.txt", "w") as f:
    for token in model.decoder.vocabulary:
        f.write(token + "\n")
```

### Step 3: Host the Model

Upload to a public URL. Options:
- Hugging Face Hub
- GitHub Releases
- S3/GCS bucket

### Step 4: Register the Model

Edit `crates/models/src/metadata.rs`:

```rust
pub const MODELS: &[ModelMetadata] = &[
    // ... existing models
    ModelMetadata {
        id: "catalan-nemo-ctc",
        name: "NeMo Conformer (Catalan)",
        language: "ca",
        model_type: ModelType::NemoCtc,
        url: "https://huggingface.co/your-org/catalan-nemo-ctc/resolve/main/model.tar.gz",
        size_mb: 120,
        description: "Catalan speech recognition trained on Common Voice",
    },
];
```

### Step 5: Implement the Engine (if needed)

If using an existing architecture (NeMo CTC), the engine already exists:

```rust
// crates/sherpa/src/nemo_ctc.rs
pub struct NemoCtcEngine {
    recognizer: sherpa_rs::OfflineRecognizer,
}

impl SttEngine for NemoCtcEngine {
    fn transcribe(&self, audio: &[f32]) -> Result<Vec<Segment>> {
        // ... implementation
    }
}
```

### Step 6: Test

```bash
# Unit test
cargo test -p gibberish-sherpa nemo_ctc

# Integration test
cd apps/desktop && npm run tauri dev
# Select "NeMo Conformer (Catalan)" in Settings
# Speak in Catalan!
```

## Model Requirements

### Architecture Support

| Architecture | Supported | Notes |
|--------------|-----------|-------|
| CTC | ✓ | NeMo, Wav2Vec2 |
| Transducer | ✓ | Zipformer, Conformer |
| Encoder-Decoder | Via Whisper | Use Whisper models directly |

### Audio Format

All models must accept:
- **Sample rate**: 16000 Hz
- **Channels**: Mono
- **Format**: Float32 PCM

Our `gibberish-audio` crate handles resampling automatically.

### Vocabulary Format

`tokens.txt` should contain one token per line:

```
<blk>
a
b
c
...
z
'
<space>
```

Special tokens:
- `<blk>` or `<blank>` — CTC blank token
- `<space>` or `▁` — Word separator
- `<unk>` — Unknown token

## Troubleshooting

### "Model produces garbage output"

Check vocabulary alignment. The token indices must match exactly.

### "Model is slow"

Try quantization:

```bash
python -m onnxruntime.quantization.quantize \
    --input model.onnx \
    --output model_int8.onnx \
    --quant_format QDQ
```

### "Model crashes on long audio"

Some models have maximum sequence length. Chunk the audio:

```rust
const MAX_SECONDS: usize = 30;
let chunks = audio.chunks(MAX_SECONDS * 16000);
```

## Contributing Models

If you successfully add a language:

1. Upload to Hugging Face with a clear model card
2. Add to `MODELS` in `metadata.rs`
3. Submit a PR!

Contributions welcome for:
- Spanish
- French
- German
- Portuguese

# /// script
# requires-python = ">=3.10,<3.13"
# dependencies = [
#     "nemo_toolkit[asr]",
#     "torch<2.6",
#     "onnx",
#     "onnxruntime",
#     "huggingface_hub",
# ]
# ///
"""Export NeMo CTC model to ONNX format for sherpa-onnx.

Usage:
    uv run --python 3.11 scripts/export_nemo_ctc.py

Output files are written to the gibberish models directory:
    ~/Library/Application Support/gibberish/models/nemo-conformer-ca/
"""

import os
from pathlib import Path

import nemo.collections.asr as nemo_asr
from huggingface_hub import hf_hub_download

HF_REPO = "nvidia/stt_ca_conformer_ctc_large"
HF_FILENAME = "stt_ca_conformer_ctc_large.nemo"

# Output directory
if os.name == "nt":
    models_dir = Path(os.environ.get("LOCALAPPDATA", "")) / "gibb.eri.sh" / "models"
else:
    models_dir = Path.home() / "Library" / "Application Support" / "gibb.eri.sh" / "models"

output_dir = models_dir / "nemo-conformer-ca"
output_dir.mkdir(parents=True, exist_ok=True)

# Download .nemo file from HuggingFace
print(f"Downloading {HF_FILENAME} from {HF_REPO}...")
nemo_path = hf_hub_download(repo_id=HF_REPO, filename=HF_FILENAME)
print(f"Downloaded to: {nemo_path}")

print("Loading NeMo model...")
m = nemo_asr.models.EncDecCTCModel.restore_from(nemo_path)
m.eval()

# Export tokens in sherpa-onnx format: TOKEN ID per line, with <blk> at end
tokens_path = output_dir / "tokens.txt"

# BPE models use tokenizer, not labels
if hasattr(m, 'tokenizer') and m.tokenizer is not None:
    vocab_size = m.tokenizer.vocab_size
    print(f"Writing {vocab_size} BPE tokens + <blk> to {tokens_path}")
    with open(tokens_path, "w", encoding="utf-8") as f:
        for i in range(vocab_size):
            token = m.tokenizer.ids_to_tokens([i])[0]
            # Replace special characters for sherpa compatibility
            if token == " ":
                token = "▁"  # SentencePiece space marker
            f.write(f"{token} {i}\n")
        f.write(f"<blk> {vocab_size}\n")
else:
    # Character-based CTC model
    labels = list(m.cfg.labels)
    print(f"Writing {len(labels)} tokens + <blk> to {tokens_path}")
    with open(tokens_path, "w", encoding="utf-8") as f:
        for i, t in enumerate(labels):
            f.write(f"{t} {i}\n")
        f.write(f"<blk> {len(labels)}\n")

# Export ONNX model
model_path = output_dir / "model.onnx"
print(f"Exporting ONNX model to {model_path}")
m.export(str(model_path))

# Add required metadata for sherpa-onnx compatibility
import onnx

print("Adding sherpa-onnx metadata to model...")
model = onnx.load(str(model_path))

# Get vocab size (including blank token)
if hasattr(m, 'tokenizer') and m.tokenizer is not None:
    vocab_size = m.tokenizer.vocab_size + 1  # +1 for blank
else:
    vocab_size = len(m.cfg.labels) + 1

# Extract config from model
normalize_type = str(m.preprocessor._cfg.get("normalize", ""))
subsampling_factor = str(m.encoder._cfg.get("subsampling_factor", 4))

# Add metadata
metadata = {
    "vocab_size": str(vocab_size),
    "normalize_type": normalize_type,
    "subsampling_factor": subsampling_factor,
    "model_type": "EncDecCTCModelBPE",
    "version": "1",
}

for key, value in metadata.items():
    meta = model.metadata_props.add()
    meta.key = key
    meta.value = value

onnx.save(model, str(model_path))
print(f"Added metadata: vocab_size={vocab_size}")

print(f"\nDone! Files written to: {output_dir}")
print("  - model.onnx")
print("  - tokens.txt")
if (output_dir / "model.onnx_data").exists():
    print("  - model.onnx_data")

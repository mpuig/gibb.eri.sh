# Scripts (Model Conversion)

This folder contains helper scripts used during development of `gibb.eri.sh` to convert upstream speech models into formats that the app can run offline on-device.

## NeMo CTC → ONNX (Catalan)

`scripts/export_nemo_ctc.py` converts NVIDIA’s original NeMo CTC model to a `sherpa-onnx` compatible layout:

- Downloads the source `.nemo` model from Hugging Face:
  - `nvidia/stt_ca_conformer_ctc_large` (`stt_ca_conformer_ctc_large.nemo`)
- Exports:
  - `model.onnx` (and optional `model.onnx_data`)
  - `tokens.txt` in `sherpa-onnx` format (`TOKEN ID` per line, plus `<blk>` as the final token)
- Writes outputs into the app’s local model directory:
  - `~/Library/Application Support/gibberish/models/nemo-conformer-ca/` (macOS)

The resulting ONNX artifacts are published here so `gibb.eri.sh` can download and load them like the other speech models:

- https://huggingface.co/mpuig/stt_ca_conformer_ctc_large_onnx

## Running the exporter

The script uses a NeMo-compatible Python range (see the header in `scripts/export_nemo_ctc.py`). If your system Python is newer, run it with `uv` and a supported Python:

- `uv run --python 3.11 scripts/export_nemo_ctc.py`

After export, verify the output folder contains at least:

- `model.onnx`
- `tokens.txt`


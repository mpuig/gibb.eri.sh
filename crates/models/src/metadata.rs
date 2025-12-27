//! Model metadata registry.
//!
//! Separates infrastructure details (URLs, sizes, file paths) from domain
//! identity (SttModel enum), following Clean Architecture principles.

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

/// Infrastructure metadata for a model.
///
/// Contains all the details needed to download and verify a model,
/// separate from its domain identity.
#[derive(Debug, Clone)]
pub struct ModelMetadata {
    /// Human-readable display name.
    pub display_name: &'static str,
    /// Directory name for local storage.
    pub dir_name: &'static str,
    /// HuggingFace repository (empty for non-HF sources).
    pub huggingface_repo: &'static str,
    /// Approximate size in bytes (for progress display).
    pub size_bytes: u64,
    /// Function to check if required files exist.
    pub is_downloaded: fn(&Path) -> bool,
    /// Model category for grouping.
    pub category: ModelCategory,
}

/// Model categories for organization and filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelCategory {
    /// Whisper GGML models (whisper.cpp format).
    WhisperGgml,
    /// Whisper ONNX models (sherpa-onnx format).
    WhisperOnnx,
    /// Parakeet ONNX models.
    Parakeet,
    /// Sherpa-ONNX streaming models.
    SherpaStreaming,
    /// NeMo CTC models (ONNX format).
    NemoCtc,
}

/// Global registry of model metadata, keyed by model ID (name).
static MODEL_REGISTRY: LazyLock<HashMap<&'static str, ModelMetadata>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Whisper GGML models
    m.insert(
        "whisper-small",
        ModelMetadata {
            display_name: "Whisper Small",
            dir_name: "whisper-small",
            huggingface_repo: "ggerganov/whisper.cpp",
            size_bytes: 466_000_000,
            is_downloaded: is_whisper_ggml_downloaded,
            category: ModelCategory::WhisperGgml,
        },
    );
    m.insert(
        "whisper-small.en",
        ModelMetadata {
            display_name: "Whisper Small (English)",
            dir_name: "whisper-small.en",
            huggingface_repo: "ggerganov/whisper.cpp",
            size_bytes: 466_000_000,
            is_downloaded: is_whisper_ggml_downloaded,
            category: ModelCategory::WhisperGgml,
        },
    );
    m.insert(
        "whisper-large-v3-turbo",
        ModelMetadata {
            display_name: "Whisper Large v3 Turbo",
            dir_name: "whisper-large-v3-turbo",
            huggingface_repo: "ggerganov/whisper.cpp",
            size_bytes: 1_600_000_000,
            is_downloaded: is_whisper_ggml_downloaded,
            category: ModelCategory::WhisperGgml,
        },
    );

    // Whisper ONNX models (sherpa-onnx format)
    m.insert(
        "whisper-onnx-small",
        ModelMetadata {
            display_name: "Whisper Small ONNX",
            dir_name: "sherpa-onnx-whisper-small",
            huggingface_repo: "", // Downloaded from GitHub releases
            size_bytes: 490_000_000,
            is_downloaded: is_whisper_onnx_small_downloaded,
            category: ModelCategory::WhisperOnnx,
        },
    );

    // Parakeet models
    m.insert(
        "parakeet-ctc",
        ModelMetadata {
            display_name: "Parakeet CTC 0.6B",
            dir_name: "parakeet-ctc-0.6b",
            huggingface_repo: "onnx-community/parakeet-ctc-0.6b-ONNX",
            size_bytes: 1_220_000_000,
            is_downloaded: is_parakeet_ctc_downloaded,
            category: ModelCategory::Parakeet,
        },
    );
    m.insert(
        "parakeet-tdt",
        ModelMetadata {
            display_name: "Parakeet TDT 0.6B",
            dir_name: "parakeet-tdt-0.6b",
            huggingface_repo: "istupakov/parakeet-tdt-0.6b-v3-onnx",
            size_bytes: 700_000_000,
            is_downloaded: is_parakeet_tdt_downloaded,
            category: ModelCategory::Parakeet,
        },
    );
    m.insert(
        "parakeet-eou",
        ModelMetadata {
            display_name: "Parakeet EOU 120M",
            dir_name: "parakeet-eou-120m",
            huggingface_repo: "CHRV/parakeet_realtime_eou_120m-v1-onnx",
            size_bytes: 140_000_000,
            is_downloaded: is_parakeet_eou_downloaded,
            category: ModelCategory::Parakeet,
        },
    );

    // Sherpa streaming models
    m.insert(
        "sherpa-zipformer-en",
        ModelMetadata {
            display_name: "Sherpa Zipformer (English)",
            dir_name: "sherpa-zipformer-en-2023-06-21-320ms",
            huggingface_repo: "nytopop/zipformer-en-2023-06-21-320ms",
            size_bytes: 250_000_000,
            is_downloaded: is_sherpa_zipformer_downloaded,
            category: ModelCategory::SherpaStreaming,
        },
    );

    // NeMo CTC models (ONNX-converted)
    m.insert(
        "nemo-conformer-ca",
        ModelMetadata {
            display_name: "Conformer CTC (Catalan)",
            dir_name: "nemo-conformer-ca",
            huggingface_repo: "mpuig/stt_ca_conformer_ctc_large_onnx",
            size_bytes: 507_000_000,
            is_downloaded: is_nemo_ctc_downloaded,
            category: ModelCategory::NemoCtc,
        },
    );

    m
});

/// Get metadata for a model by ID.
pub fn get_metadata(model_id: &str) -> Option<&'static ModelMetadata> {
    MODEL_REGISTRY.get(model_id)
}

// Download verification functions

fn is_whisper_ggml_downloaded(dir: &Path) -> bool {
    dir.join("model.bin").exists()
}

fn is_whisper_onnx_small_downloaded(dir: &Path) -> bool {
    let has_encoder =
        dir.join("small-encoder.int8.onnx").exists() || dir.join("small-encoder.onnx").exists();
    let has_decoder =
        dir.join("small-decoder.int8.onnx").exists() || dir.join("small-decoder.onnx").exists();
    let has_tokens = dir.join("small-tokens.txt").exists();
    has_encoder && has_decoder && has_tokens
}

fn is_parakeet_ctc_downloaded(dir: &Path) -> bool {
    dir.join("model_fp16.onnx").exists() && dir.join("tokenizer.json").exists()
}

fn is_parakeet_tdt_downloaded(dir: &Path) -> bool {
    dir.join("encoder-model.onnx").exists()
        && dir.join("decoder_joint-model.onnx").exists()
        && dir.join("vocab.txt").exists()
}

fn is_parakeet_eou_downloaded(dir: &Path) -> bool {
    dir.join("encoder.onnx").exists()
        && dir.join("decoder_joint.onnx").exists()
        && dir.join("vocab.txt").exists()
}

fn is_sherpa_zipformer_downloaded(dir: &Path) -> bool {
    dir.join("encoder.onnx").exists()
        && dir.join("decoder.onnx").exists()
        && dir.join("joiner.onnx").exists()
        && dir.join("tokens.txt").exists()
}

fn is_nemo_ctc_downloaded(dir: &Path) -> bool {
    dir.join("model.onnx").exists() && dir.join("tokens.txt").exists()
}

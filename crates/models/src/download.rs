use crate::{is_downloaded, models_dir, ModelError, Result, SttModel};
use futures::StreamExt;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

pub async fn download_model<F>(model: SttModel, on_progress: F) -> Result<PathBuf>
where
    F: Fn(u64, u64),
{
    let model_dir = models_dir().join(model.dir_name());

    // Check if all required files exist, not just the directory
    if is_downloaded(model) {
        return Ok(model_dir);
    }

    std::fs::create_dir_all(&model_dir)?;

    match model {
        SttModel::ParakeetCtc | SttModel::ParakeetTdt | SttModel::ParakeetEou => {
            download_parakeet_model(model, &model_dir, on_progress).await?;
        }
        SttModel::SherpaZipformerEn => {
            download_sherpa_streaming_model(model, &model_dir, on_progress).await?;
        }
        SttModel::WhisperOnnxSmall => {
            download_whisper_onnx_model(model, &model_dir, on_progress).await?;
        }
        SttModel::NemoConformerCatalan => {
            download_nemo_ctc_model(model, &model_dir, on_progress).await?;
        }
        _ => {
            download_whisper_model(model, &model_dir, on_progress).await?;
        }
    };

    Ok(model_dir)
}

async fn download_parakeet_model<F>(model: SttModel, model_dir: &Path, on_progress: F) -> Result<()>
where
    F: Fn(u64, u64),
{
    use std::sync::atomic::{AtomicU64, Ordering};

    let repo = model.huggingface_repo();
    let total_size = model.size_bytes();
    let downloaded = AtomicU64::new(0);

    // Different file structures for CTC vs TDT/EOU models
    // Using int8 quantized models for more reliable downloads
    let files: Vec<(&str, &str)> = match model {
        SttModel::ParakeetCtc => vec![
            // Keep original filenames - ONNX model has hardcoded reference to data file
            ("onnx/model_fp16.onnx", "model_fp16.onnx"),
            ("onnx/model_fp16.onnx_data", "model_fp16.onnx_data"),
            ("tokenizer.json", "tokenizer.json"),
        ],
        SttModel::ParakeetTdt => vec![
            // Use int8 quantized models - self-contained, no external data files
            ("encoder-model.int8.onnx", "encoder-model.onnx"),
            ("decoder_joint-model.int8.onnx", "decoder_joint-model.onnx"),
            ("vocab.txt", "vocab.txt"),
        ],
        SttModel::ParakeetEou => vec![
            // EOU model expects encoder.onnx and decoder_joint.onnx (no dashes)
            ("encoder-model.int8.onnx", "encoder.onnx"),
            ("decoder_joint-model.int8.onnx", "decoder_joint.onnx"),
            ("vocab.txt", "vocab.txt"),
        ],
        _ => return Err(ModelError::NotFound("Not a Parakeet model".to_string())),
    };

    for (remote_path, local_name) in files {
        let dest = model_dir.join(local_name);

        // Skip if file already exists and has reasonable size
        if dest.exists() {
            if let Ok(meta) = std::fs::metadata(&dest) {
                if meta.len() > 1000 {
                    tracing::info!("Skipping {} (already exists)", local_name);
                    continue;
                }
            }
        }

        let url = format!("https://huggingface.co/{repo}/resolve/main/{remote_path}");

        tracing::info!("Downloading {} to {:?}", url, dest);

        download_file(&url, &dest, |chunk_size| {
            let new_total = downloaded.fetch_add(chunk_size, Ordering::Relaxed) + chunk_size;
            on_progress(new_total, total_size);
        })
        .await?;
    }

    Ok(())
}

async fn download_sherpa_streaming_model<F>(
    model: SttModel,
    model_dir: &Path,
    on_progress: F,
) -> Result<()>
where
    F: Fn(u64, u64),
{
    use std::sync::atomic::{AtomicU64, Ordering};

    let repo = model.huggingface_repo();
    let downloaded = AtomicU64::new(0);
    let client = reqwest::Client::new();

    // sherpa-onnx streaming zipformer transducer: encoder/decoder/joiner + tokens
    let files: Vec<(&str, &str)> = vec![
        ("encoder.onnx", "encoder.onnx"),
        ("decoder.onnx", "decoder.onnx"),
        ("joiner.onnx", "joiner.onnx"),
        ("tokens.txt", "tokens.txt"),
        // config.json is optional for runtime but handy for debugging
        ("config.json", "config.json"),
    ];

    // Best-effort: compute a realistic total so UI progress doesn't exceed 100%.
    // If HEAD doesn't give Content-Length, fall back to the model's rough size estimate.
    let mut total_size = 0u64;
    for (remote_path, local_name) in &files {
        let dest = model_dir.join(local_name);
        if let Ok(meta) = std::fs::metadata(&dest) {
            if meta.len() > 1000 {
                total_size = total_size.saturating_add(meta.len());
                downloaded.fetch_add(meta.len(), Ordering::Relaxed);
                continue;
            }
        }

        let url = format!("https://huggingface.co/{repo}/resolve/main/{remote_path}");

        if let Ok(resp) = client.head(&url).send().await {
            if resp.status().is_success() {
                if let Some(len) = resp
                    .headers()
                    .get(reqwest::header::CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                {
                    total_size = total_size.saturating_add(len);
                    continue;
                }
            }
        }
    }
    total_size = total_size.max(model.size_bytes());
    on_progress(downloaded.load(Ordering::Relaxed), total_size);

    for (remote_path, local_name) in files {
        let dest = model_dir.join(local_name);

        if dest.exists() {
            if let Ok(meta) = std::fs::metadata(&dest) {
                if meta.len() > 1000 {
                    tracing::info!("Skipping {} (already exists)", local_name);
                    continue;
                }
            }
        }

        let url = format!("https://huggingface.co/{repo}/resolve/main/{remote_path}");

        tracing::info!("Downloading {} to {:?}", url, dest);

        download_file(&url, &dest, |chunk_size| {
            let new_total = downloaded.fetch_add(chunk_size, Ordering::Relaxed) + chunk_size;
            on_progress(new_total, total_size);
        })
        .await?;
    }

    Ok(())
}

/// Download Whisper ONNX models from sherpa-onnx releases.
///
/// These models are hosted as tar.bz2 archives on GitHub releases.
async fn download_whisper_onnx_model<F>(
    model: SttModel,
    model_dir: &Path,
    on_progress: F,
) -> Result<()>
where
    F: Fn(u64, u64),
{
    use std::io::Read;

    // Model name to archive name mapping
    let archive_name = match model {
        SttModel::WhisperOnnxSmall => "sherpa-onnx-whisper-small",
        _ => return Err(ModelError::NotFound("Not a Whisper ONNX model".to_string())),
    };

    let url = format!(
        "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/{archive_name}.tar.bz2"
    );

    let total_size = model.size_bytes();
    let mut downloaded = 0u64;

    tracing::info!("Downloading Whisper ONNX model from {}", url);

    // Download the archive to a temp file
    let temp_archive = model_dir.join("model.tar.bz2");
    download_file(&url, &temp_archive, |chunk_size| {
        downloaded += chunk_size;
        on_progress(downloaded, total_size);
    })
    .await?;

    tracing::info!("Extracting Whisper ONNX model archive");

    // Extract the archive
    let archive_file = std::fs::File::open(&temp_archive)
        .map_err(|e| ModelError::DownloadFailed(e.to_string()))?;

    let decoder = bzip2::read::BzDecoder::new(archive_file);
    let mut archive = tar::Archive::new(decoder);

    // Extract all entries, stripping the top-level directory
    for entry in archive
        .entries()
        .map_err(|e| ModelError::DownloadFailed(e.to_string()))?
    {
        let mut entry = entry.map_err(|e| ModelError::DownloadFailed(e.to_string()))?;
        let path = entry
            .path()
            .map_err(|e| ModelError::DownloadFailed(e.to_string()))?;

        // Strip the top-level directory (e.g., "sherpa-onnx-whisper-tiny/")
        let components: Vec<_> = path.components().collect();
        if components.len() <= 1 {
            continue; // Skip the top-level directory itself
        }

        // Build path without the first component
        let relative_path: PathBuf = components[1..].iter().collect();

        // Skip test_wavs directory
        if relative_path.starts_with("test_wavs") {
            continue;
        }

        let dest_path = model_dir.join(&relative_path);

        // Create parent directories
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Extract file
        if entry.header().entry_type().is_file() {
            let mut content = Vec::new();
            entry
                .read_to_end(&mut content)
                .map_err(|e| ModelError::DownloadFailed(e.to_string()))?;
            std::fs::write(&dest_path, &content)?;
            tracing::debug!("Extracted: {:?}", relative_path);
        }
    }

    // Clean up temp archive
    let _ = std::fs::remove_file(&temp_archive);

    tracing::info!("Whisper ONNX model extracted successfully");

    Ok(())
}

async fn download_whisper_model<F>(model: SttModel, model_dir: &Path, on_progress: F) -> Result<()>
where
    F: Fn(u64, u64),
{
    let url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{}.bin",
        model.dir_name().replace("whisper-", "")
    );
    let dest = model_dir.join("model.bin");

    let total_size = model.size_bytes();
    let mut downloaded = 0u64;

    download_file(&url, &dest, |chunk_size| {
        downloaded += chunk_size;
        on_progress(downloaded, total_size);
    })
    .await?;

    Ok(())
}

/// Download NeMo CTC models (ONNX converted format).
///
/// NeMo models from NVIDIA need to be converted to ONNX format.
/// This function downloads pre-converted ONNX files from HuggingFace.
async fn download_nemo_ctc_model<F>(model: SttModel, model_dir: &Path, on_progress: F) -> Result<()>
where
    F: Fn(u64, u64),
{
    use std::sync::atomic::{AtomicU64, Ordering};

    let repo = model.huggingface_repo();
    let total_size = model.size_bytes();
    let downloaded = AtomicU64::new(0);

    // NeMo CTC models converted to ONNX format
    // Expected files: model.onnx and tokens.txt
    let files: Vec<(&str, &str)> = vec![("model.onnx", "model.onnx"), ("tokens.txt", "tokens.txt")];

    for (remote_path, local_name) in files {
        let dest = model_dir.join(local_name);

        // Skip if file already exists and has reasonable size
        if dest.exists() {
            if let Ok(meta) = std::fs::metadata(&dest) {
                if meta.len() > 1000 {
                    tracing::info!("Skipping {} (already exists)", local_name);
                    continue;
                }
            }
        }

        let url = format!("https://huggingface.co/{repo}/resolve/main/{remote_path}");

        tracing::info!("Downloading {} to {:?}", url, dest);

        download_file(&url, &dest, |chunk_size| {
            let new_total = downloaded.fetch_add(chunk_size, Ordering::Relaxed) + chunk_size;
            on_progress(new_total, total_size);
        })
        .await?;
    }

    Ok(())
}

pub(crate) async fn download_file<F>(url: &str, dest: &Path, mut on_chunk: F) -> Result<u64>
where
    F: FnMut(u64),
{
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| ModelError::DownloadFailed(e.to_string()))?;

    if !response.status().is_success() {
        return Err(ModelError::DownloadFailed(format!(
            "HTTP {}: {}",
            response.status(),
            url
        )));
    }

    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(ModelError::IoError)?;

    let mut stream = response.bytes_stream();
    let mut total = 0u64;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ModelError::DownloadFailed(e.to_string()))?;
        file.write_all(&chunk).await.map_err(ModelError::IoError)?;
        total += chunk.len() as u64;
        on_chunk(chunk.len() as u64);
    }

    file.flush().await.map_err(ModelError::IoError)?;

    Ok(total)
}

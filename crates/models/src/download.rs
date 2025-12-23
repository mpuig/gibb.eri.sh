use crate::{is_downloaded, models_dir, ModelError, Result, SttModel};
use futures::StreamExt;
use std::path::PathBuf;
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
        _ => {
            download_whisper_model(model, &model_dir, on_progress).await?;
        }
    }

    Ok(model_dir)
}

async fn download_parakeet_model<F>(
    model: SttModel,
    model_dir: &PathBuf,
    on_progress: F,
) -> Result<()>
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

        let url = format!(
            "https://huggingface.co/{}/resolve/main/{}",
            repo, remote_path
        );

        tracing::info!("Downloading {} to {:?}", url, dest);

        download_file(&url, &dest, |chunk_size| {
            let new_total = downloaded.fetch_add(chunk_size, Ordering::Relaxed) + chunk_size;
            on_progress(new_total, total_size);
        })
        .await?;
    }

    Ok(())
}

async fn download_whisper_model<F>(
    model: SttModel,
    model_dir: &PathBuf,
    on_progress: F,
) -> Result<()>
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

async fn download_file<F>(url: &str, dest: &PathBuf, mut on_chunk: F) -> Result<u64>
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
        .map_err(|e| ModelError::IoError(e))?;

    let mut stream = response.bytes_stream();
    let mut total = 0u64;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ModelError::DownloadFailed(e.to_string()))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| ModelError::IoError(e))?;
        total += chunk.len() as u64;
        on_chunk(chunk.len() as u64);
    }

    file.flush().await.map_err(|e| ModelError::IoError(e))?;

    Ok(total)
}

use futures::StreamExt;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("http error: {0}")]
    Http(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("cancelled")]
    Cancelled,
}

pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub file: String,
    pub file_downloaded_bytes: u64,
    pub file_total_bytes: u64,
}

fn hf_resolve_url(repo: &str, remote_path: &str) -> String {
    format!("https://huggingface.co/{}/resolve/main/{}", repo, remote_path)
}

async fn download_file<F>(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    cancel: &tokio_util::sync::CancellationToken,
    mut on_progress: F,
) -> Result<(u64, u64), DownloadError>
where
    F: FnMut(u64, u64),
{
    if cancel.is_cancelled() {
        return Err(DownloadError::Cancelled);
    }

    let resp = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "gibberish-tools/0.1.0")
        .send()
        .await
        .map_err(|e| DownloadError::Http(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(DownloadError::Http(format!(
            "HTTP {}: {}",
            resp.status(),
            url
        )));
    }

    let total = resp.content_length().unwrap_or(0);

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| DownloadError::Io(e.to_string()))?;
    }

    let tmp = dest.with_extension("part");
    let mut file = tokio::fs::File::create(&tmp)
        .await
        .map_err(|e| DownloadError::Io(e.to_string()))?;

    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        if cancel.is_cancelled() {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(DownloadError::Cancelled);
        }

        let chunk = chunk.map_err(|e| DownloadError::Http(e.to_string()))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| DownloadError::Io(e.to_string()))?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total);
    }

    file.flush()
        .await
        .map_err(|e| DownloadError::Io(e.to_string()))?;

    // Atomic-ish replace.
    tokio::fs::rename(&tmp, dest)
        .await
        .map_err(|e| DownloadError::Io(e.to_string()))?;

    Ok((downloaded, total))
}

pub struct FunctionGemmaDownloadPlan {
    pub repo: &'static str,
    pub files: Vec<(String, PathBuf)>,
}

impl FunctionGemmaDownloadPlan {
    pub fn for_variant(base_dir: &Path, variant: &str) -> Self {
        // Start with the single recommended repo Marc linked. Easy to expand later.
        let repo = "onnx-community/functiongemma-270m-it-ONNX";
        let variant = variant.to_string();

        let onnx_name = format!("{}.onnx", variant);
        let data_name = format!("{}.onnx_data", variant);

        Self {
            repo,
            files: vec![
                // ONNX model + external data (required for these repos).
                (format!("onnx/{}", onnx_name), base_dir.join(&onnx_name)),
                (format!("onnx/{}", data_name), base_dir.join(&data_name)),
                // Tokenizer (must be tokenizers JSON).
                ("tokenizer.json".to_string(), base_dir.join("tokenizer.json")),
            ],
        }
    }
}

pub async fn download_functiongemma<F>(
    client: &reqwest::Client,
    plan: &FunctionGemmaDownloadPlan,
    cancel: &tokio_util::sync::CancellationToken,
    mut on_progress: F,
) -> Result<(), DownloadError>
where
    F: FnMut(DownloadProgress),
{
    let mut downloaded_total: u64 = 0;
    let mut known_totals: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

    for (remote, dest) in &plan.files {
        if cancel.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }

        // Skip if file already exists and looks non-empty.
        if dest.exists() {
            if let Ok(meta) = std::fs::metadata(dest) {
                if meta.len() > 1024 {
                    continue;
                }
            }
        }

        let url = hf_resolve_url(plan.repo, remote);
        let file_label = remote.clone();

        let (file_downloaded, file_total) = download_file(client, &url, dest, cancel, |d, t| {
            if t > 0 {
                known_totals.entry(file_label.clone()).or_insert(t);
            }

            let total = known_totals.values().fold(0u64, |acc, v| acc.saturating_add(*v));
            let downloaded = downloaded_total.saturating_add(d);
            on_progress(DownloadProgress {
                downloaded_bytes: downloaded,
                total_bytes: total,
                file: file_label.clone(),
                file_downloaded_bytes: d,
                file_total_bytes: t,
            });
        })
        .await?;

        downloaded_total = downloaded_total.saturating_add(file_downloaded);
        if file_total > 0 {
            known_totals.entry(file_label.clone()).or_insert(file_total);
        }

        let total = known_totals.values().fold(0u64, |acc, v| acc.saturating_add(*v));
        on_progress(DownloadProgress {
            downloaded_bytes: downloaded_total,
            total_bytes: total,
            file: file_label,
            file_downloaded_bytes: file_downloaded,
            file_total_bytes: file_total,
        });
    }

    Ok(())
}

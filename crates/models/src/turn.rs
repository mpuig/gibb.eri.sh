use crate::download::download_file;
use crate::{models_dir, ModelError, Result};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnModel {
    /// Smart Turn v3.1 CPU (8MB int8) semantic endpoint detector
    SmartTurnV31Cpu,
}

impl TurnModel {
    pub fn name(&self) -> &'static str {
        match self {
            Self::SmartTurnV31Cpu => "smart-turn-v3.1-cpu",
        }
    }

    pub fn dir_name(&self) -> &'static str {
        match self {
            Self::SmartTurnV31Cpu => "smart-turn-v3.1-cpu",
        }
    }

    pub fn huggingface_repo(&self) -> &'static str {
        match self {
            Self::SmartTurnV31Cpu => "pipecat-ai/smart-turn-v3",
        }
    }

    pub fn remote_path(&self) -> &'static str {
        match self {
            Self::SmartTurnV31Cpu => "smart-turn-v3.1-cpu.onnx",
        }
    }

    pub fn local_filename(&self) -> &'static str {
        match self {
            Self::SmartTurnV31Cpu => "smart-turn-v3.1-cpu.onnx",
        }
    }

    pub fn size_bytes(&self) -> u64 {
        match self {
            Self::SmartTurnV31Cpu => 9_000_000,
        }
    }
}

fn turn_models_dir() -> PathBuf {
    models_dir().join("turn")
}

pub fn turn_model_path(model: TurnModel) -> PathBuf {
    turn_models_dir().join(model.dir_name())
}

pub fn is_turn_model_downloaded(model: TurnModel) -> bool {
    let dir = turn_model_path(model);
    dir.join(model.local_filename()).exists()
}

pub async fn download_turn_model<F>(model: TurnModel, on_progress: F) -> Result<PathBuf>
where
    F: Fn(u64, u64),
{
    let model_dir = turn_model_path(model);

    if is_turn_model_downloaded(model) {
        return Ok(model_dir);
    }

    std::fs::create_dir_all(&model_dir)?;

    let url = format!(
        "https://huggingface.co/{}/resolve/main/{}",
        model.huggingface_repo(),
        model.remote_path()
    );
    let dest = model_dir.join(model.local_filename());

    let total = model.size_bytes();
    let mut downloaded = 0u64;

    download_file(&url, &dest, |chunk| {
        downloaded = downloaded.saturating_add(chunk);
        on_progress(downloaded, total.max(downloaded));
    })
    .await
    .map_err(|e| match e {
        ModelError::DownloadFailed(msg) => ModelError::DownloadFailed(msg),
        other => other,
    })?;

    Ok(model_dir)
}

//! Help tool implementation.
//!
//! Lists available tools for the current mode.
//! Uses a ToolInfoProvider trait to decouple from the registry.

use super::{Mode, Tool, ToolContext, ToolError, ToolResult};
use async_trait::async_trait;
use std::sync::Arc;

/// Tool info for help output.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub examples: Vec<String>,
    pub modes: Vec<String>,
}

/// Trait for providing tool information.
/// This decouples the help tool from the registry implementation.
pub trait ToolInfoProvider: Send + Sync {
    fn get_tools_for_mode(&self, mode: Mode) -> Vec<ToolInfo>;
}

/// Tool for listing available tools.
pub struct HelpTool {
    provider: Arc<dyn ToolInfoProvider>,
}

impl HelpTool {
    #[allow(dead_code)]
    pub fn new(provider: Arc<dyn ToolInfoProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl Tool for HelpTool {
    fn name(&self) -> &'static str {
        "help"
    }

    fn description(&self) -> &'static str {
        "List available voice commands for current mode"
    }

    fn example_phrases(&self) -> &'static [&'static str] {
        &[
            "what can I do",
            "help",
            "list commands",
            "what tools are available",
        ]
    }

    fn modes(&self) -> &'static [Mode] {
        &[]
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let mode_str = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("Global");

        let mode = match mode_str {
            "Meeting" => Mode::Meeting,
            "Dev" => Mode::Dev,
            "Writer" => Mode::Writer,
            _ => Mode::Global,
        };

        let tools = self.provider.get_tools_for_mode(mode);

        Ok(ToolResult {
            event_name: "tools:help",
            payload: serde_json::json!({
                "mode": mode.to_string(),
                "tools": tools,
                "count": tools.len(),
            }),
            cache_key: Some(format!("help:{}", mode)),
            cooldown_key: None,
        })
    }
}

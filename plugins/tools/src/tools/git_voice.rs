//! Git voice command tool implementation.
//!
//! Executes common git commands via voice in Dev mode.

use super::{Mode, Tool, ToolContext, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Tool for executing git commands via voice.
pub struct GitVoiceTool;

/// Git commands supported by the tool.
#[derive(Debug, Clone)]
pub enum GitCommand {
    Status,
    Diff { staged: bool },
    Add { path: Option<String> },
    Commit { message: String },
    Push,
    Pull,
    Branch { name: Option<String> },
    Checkout { target: String },
    Log { count: u8 },
    Stash { pop: bool },
}

impl GitCommand {
    fn parse(action: &str, args: &serde_json::Value) -> Option<Self> {
        match action.to_lowercase().as_str() {
            "status" => Some(GitCommand::Status),
            "diff" => {
                let staged = args
                    .get("staged")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                Some(GitCommand::Diff { staged })
            }
            "add" => {
                let path = args.get("path").and_then(|v| v.as_str()).map(String::from);
                Some(GitCommand::Add { path })
            }
            "commit" => {
                let message = args
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(String::from)?;
                Some(GitCommand::Commit { message })
            }
            "push" => Some(GitCommand::Push),
            "pull" => Some(GitCommand::Pull),
            "branch" => {
                let name = args.get("name").and_then(|v| v.as_str()).map(String::from);
                Some(GitCommand::Branch { name })
            }
            "checkout" | "switch" => {
                let target = args
                    .get("target")
                    .or_else(|| args.get("branch"))
                    .and_then(|v| v.as_str())
                    .map(String::from)?;
                Some(GitCommand::Checkout { target })
            }
            "log" => {
                let count = args
                    .get("count")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u8)
                    .unwrap_or(5);
                Some(GitCommand::Log { count })
            }
            "stash" => {
                let pop = args.get("pop").and_then(|v| v.as_bool()).unwrap_or(false);
                Some(GitCommand::Stash { pop })
            }
            _ => None,
        }
    }

    fn to_args(&self) -> Vec<String> {
        match self {
            GitCommand::Status => vec!["status".into(), "--short".into()],
            GitCommand::Diff { staged } => {
                if *staged {
                    vec!["diff".into(), "--staged".into()]
                } else {
                    vec!["diff".into()]
                }
            }
            GitCommand::Add { path } => {
                let mut args = vec!["add".into()];
                if let Some(p) = path {
                    args.push(p.clone());
                } else {
                    args.push("-A".into());
                }
                args
            }
            GitCommand::Commit { message } => vec!["commit".into(), "-m".into(), message.clone()],
            GitCommand::Push => vec!["push".into()],
            GitCommand::Pull => vec!["pull".into()],
            GitCommand::Branch { name } => {
                if let Some(n) = name {
                    vec!["branch".into(), n.clone()]
                } else {
                    vec!["branch".into(), "--list".into()]
                }
            }
            GitCommand::Checkout { target } => vec!["checkout".into(), target.clone()],
            GitCommand::Log { count } => {
                vec!["log".into(), "--oneline".into(), format!("-{}", count)]
            }
            GitCommand::Stash { pop } => {
                if *pop {
                    vec!["stash".into(), "pop".into()]
                } else {
                    vec!["stash".into()]
                }
            }
        }
    }

    fn is_read_only(&self) -> bool {
        matches!(
            self,
            GitCommand::Status
                | GitCommand::Diff { .. }
                | GitCommand::Branch { name: None }
                | GitCommand::Log { .. }
        )
    }
}

use std::borrow::Cow;

#[async_trait]
impl Tool for GitVoiceTool {
    fn name(&self) -> Cow<'static, str> {
        "git_voice".into()
    }

    fn description(&self) -> Cow<'static, str> {
        "Execute git commands via voice".into()
    }

    fn selection_hint(&self) -> Option<Cow<'static, str>> {
        Some("git commands (status, commit, push, diff)".into())
    }

    fn example_phrases(&self) -> &'static [&'static str] {
        &[
            "git status",
            "git diff",
            "commit with message fix bug",
            "push",
            "checkout main",
            "show last 5 commits",
        ]
    }

    fn few_shot_examples(&self) -> &'static [&'static str] {
        &[
            "User: git status\n<start_function_call>call:git_voice{action:<escape>status<escape>}<end_function_call>",
            "User: commit with message fix bug\n<start_function_call>call:git_voice{action:<escape>commit<escape>,message:<escape>fix bug<escape>}<end_function_call>",
        ]
    }

    // Only available in Dev mode
    fn modes(&self) -> Cow<'static, [Mode]> {
        Cow::Borrowed(&[Mode::Dev])
    }

    fn is_read_only(&self) -> bool {
        // Git commands can have side effects - router should check command-level read_only
        false
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Git action: status, diff, add, commit, push, pull, branch, checkout, log, stash",
                    "enum": ["status", "diff", "add", "commit", "push", "pull", "branch", "checkout", "log", "stash"]
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory (must be under home directory)"
                },
                "message": {
                    "type": "string",
                    "description": "Commit message (required for commit action)"
                },
                "path": {
                    "type": "string",
                    "description": "File path for add action"
                },
                "target": {
                    "type": "string",
                    "description": "Branch name for checkout action"
                },
                "staged": {
                    "type": "boolean",
                    "description": "Show staged changes only (for diff action)",
                    "default": false
                },
                "count": {
                    "type": "integer",
                    "description": "Number of commits to show (for log action)",
                    "default": 5
                },
                "pop": {
                    "type": "boolean",
                    "description": "Pop stash instead of pushing (for stash action)",
                    "default": false
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or(ToolError::MissingArg("action"))?;

        let cwd = args.get("cwd").and_then(|v| v.as_str());

        // Safety check using environment abstraction
        if let Some(dir) = cwd {
            if !ctx.env.is_safe_path(dir) {
                return Err(ToolError::ExecutionFailed(
                    "cwd must be under home directory".to_string(),
                ));
            }
        }

        let command = GitCommand::parse(action, args)
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Unknown git action: {}", action)))?;

        let git_args = command.to_args();
        let git_args_refs: Vec<&str> = git_args.iter().map(|s| s.as_str()).collect();

        // Execute using environment abstraction
        let result = ctx.env.execute_command("git", &git_args_refs, cwd).await;

        let output = match result {
            Ok(out) => {
                if out.success {
                    if out.stdout.is_empty() {
                        out.stderr
                    } else {
                        out.stdout
                    }
                } else {
                    return Err(ToolError::ExecutionFailed(format!(
                        "git failed: {}",
                        if out.stderr.is_empty() {
                            &out.stdout
                        } else {
                            &out.stderr
                        }
                    )));
                }
            }
            Err(e) => return Err(ToolError::ExecutionFailed(format!("git error: {}", e))),
        };

        Ok(ToolResult {
            event_name: Cow::Borrowed("tools:git_voice"),
            payload: json!({
                "action": action,
                "command": format!("git {}", git_args.join(" ")),
                "output": output.trim(),
                "read_only": command.is_read_only(),
            }),
            cache_key: None,
            cooldown_key: if command.is_read_only() {
                None
            } else {
                Some(format!("git:{}", action))
            },
        })
    }
}

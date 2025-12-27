//! Pure router logic - no IO, no async, fully testable.
//!
//! This module contains the decision-making logic for the router,
//! separated from Tauri state management and async IO.

use crate::functiongemma::Proposal;
use crate::policy::{CLARIFICATION_THRESHOLD, MIN_CONFIDENCE};
use crate::tool_manifest::ToolPolicy;
use gibberish_context::Mode;
use std::collections::HashMap;

/// Configuration for router decision-making.
#[derive(Debug, Clone)]
pub struct RouterConfig {
    pub auto_run_read_only: bool,
    pub auto_run_all: bool,
    pub current_mode: Mode,
    pub min_confidence: f32,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            auto_run_read_only: true,
            auto_run_all: false,
            current_mode: Mode::Global,
            min_confidence: MIN_CONFIDENCE,
        }
    }
}

/// Find the best proposal above confidence threshold.
///
/// Returns the highest-confidence proposal that:
/// 1. Meets the minimum confidence threshold
/// 2. Has a matching policy in the tool registry
pub fn find_best_proposal<'a>(
    proposals: &'a [Proposal],
    policies: &HashMap<String, ToolPolicy>,
    min_confidence: f32,
) -> Option<&'a Proposal> {
    proposals
        .iter()
        .filter(|p| p.confidence >= min_confidence)
        .filter(|p| policies.contains_key(&p.tool))
        .max_by(|a, b| {
            a.confidence
                .partial_cmp(&b.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Determine execution mode based on policy and config.
///
/// Returns true if should auto-execute, false if needs approval.
pub fn determine_execution_mode(policy: &ToolPolicy, config: &RouterConfig) -> bool {
    config.auto_run_all || (policy.read_only && config.auto_run_read_only)
}

/// Check if a proposal needs clarification due to low confidence.
///
/// Returns true if the proposal's confidence is between MIN_CONFIDENCE
/// and CLARIFICATION_THRESHOLD, indicating we should ask the user
/// to clarify their intent.
pub fn needs_clarification(proposal: &Proposal) -> bool {
    proposal.confidence >= MIN_CONFIDENCE && proposal.confidence < CLARIFICATION_THRESHOLD
}

/// Suggested clarification questions based on the proposal.
pub fn clarification_suggestions(proposal: &Proposal, user_text: &str) -> Vec<String> {
    let mut suggestions = Vec::new();

    // Generic clarification suggestions
    suggestions.push(format!(
        "Did you mean to use '{}' for \"{}\"?",
        proposal.tool,
        truncate_text(user_text, 50)
    ));

    // Tool-specific suggestions
    match proposal.tool.as_str() {
        "typer" => {
            suggestions.push("What text would you like me to type?".to_string());
        }
        "web_search" => {
            suggestions.push("What would you like me to search for?".to_string());
        }
        "app_launcher" => {
            suggestions.push("Which application should I open?".to_string());
        }
        "system_control" => {
            suggestions.push("What system action would you like?".to_string());
        }
        _ => {}
    }

    suggestions
}

/// Truncate text for display, adding ellipsis if needed.
fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_proposal(tool: &str, confidence: f32) -> Proposal {
        Proposal {
            tool: tool.to_string(),
            args: serde_json::json!({}),
            evidence: "test evidence".to_string(),
            confidence,
        }
    }

    fn make_policy(read_only: bool) -> ToolPolicy {
        ToolPolicy {
            read_only,
            default_lang: None,
            default_sentences: None,
            required_args: vec![],
            arg_types: HashMap::new(),
        }
    }

    #[test]
    fn test_find_best_proposal_filters_by_confidence() {
        let proposals = vec![
            make_proposal("tool_a", 0.9),
            make_proposal("tool_b", 0.2), // below threshold
        ];
        let mut policies = HashMap::new();
        policies.insert("tool_a".to_string(), make_policy(true));
        policies.insert("tool_b".to_string(), make_policy(true));

        let best = find_best_proposal(&proposals, &policies, 0.5);
        assert_eq!(best.map(|p| p.tool.as_str()), Some("tool_a"));
    }

    #[test]
    fn test_find_best_proposal_filters_by_policy() {
        let proposals = vec![
            make_proposal("unknown_tool", 0.9),
            make_proposal("known_tool", 0.8),
        ];
        let mut policies = HashMap::new();
        policies.insert("known_tool".to_string(), make_policy(true));

        let best = find_best_proposal(&proposals, &policies, 0.5);
        assert_eq!(best.map(|p| p.tool.as_str()), Some("known_tool"));
    }

    #[test]
    fn test_determine_execution_mode_auto_run_read_only() {
        let policy = make_policy(true); // read-only
        let config = RouterConfig {
            auto_run_read_only: true,
            auto_run_all: false,
            current_mode: Mode::Global,
            min_confidence: 0.5,
        };
        assert!(determine_execution_mode(&policy, &config));
    }

    #[test]
    fn test_determine_execution_mode_require_approval_non_readonly() {
        let policy = make_policy(false); // not read-only
        let config = RouterConfig {
            auto_run_read_only: true,
            auto_run_all: false,
            current_mode: Mode::Global,
            min_confidence: 0.5,
        };
        assert!(!determine_execution_mode(&policy, &config));
    }

    #[test]
    fn test_determine_execution_mode_auto_run_all_overrides() {
        let policy = make_policy(false); // not read-only
        let config = RouterConfig {
            auto_run_read_only: false,
            auto_run_all: true, // Override - run everything
            current_mode: Mode::Global,
            min_confidence: 0.5,
        };
        assert!(determine_execution_mode(&policy, &config));
    }
}

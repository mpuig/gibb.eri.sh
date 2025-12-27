//! Pure router logic - no IO, no async, fully testable.
//!
//! This module contains the decision-making logic for the router,
//! separated from Tauri state management and async IO.

use crate::functiongemma::Proposal;
use crate::tool_manifest::ToolPolicy;
use gibberish_context::Mode;
use std::collections::HashMap;

/// Router decision after evaluating proposals.
///
/// Note: This enum is part of the planned full migration. Currently only
/// `find_best_proposal` and `determine_execution_mode` are used by router.rs.
/// The `decide` function will be wired in during the next refactoring phase.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum RouterDecision {
    /// Execute the tool automatically.
    Execute {
        tool: String,
        args: serde_json::Value,
        evidence: String,
    },
    /// Emit proposal for user approval.
    RequireApproval {
        tool: String,
        args: serde_json::Value,
        evidence: String,
    },
    /// No valid tool found for the input.
    NoMatch { reason: String },
    /// Tool exists but is not available in current mode.
    ModeFiltered { tool: String, mode: Mode },
    /// Args validation failed and couldn't be repaired.
    ArgsInvalid { tool: String },
    /// Skip processing (empty input, disabled, etc).
    Skip,
}

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
            min_confidence: 0.35,
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
pub fn determine_execution_mode(
    policy: &ToolPolicy,
    config: &RouterConfig,
) -> bool {
    // Returns true if should auto-execute, false if needs approval
    config.auto_run_all || (policy.read_only && config.auto_run_read_only)
}


/// Make a router decision based on proposals and configuration.
///
/// This is the core pure logic of the router. It takes:
/// - Model proposals from inference
/// - Tool policies (validation rules, read_only flags)
/// - Router configuration (auto_run settings, min_confidence)
/// - Pre-validated args (after repair if needed)
/// - Mode availability (checked externally via registry)
///
/// Note: Mode availability must be checked externally before calling this,
/// as it requires access to the Tool trait (not just ToolPolicy).
///
/// And returns a decision about what action to take.
#[allow(dead_code)]
pub fn decide(
    proposals: &[Proposal],
    policies: &HashMap<String, ToolPolicy>,
    config: &RouterConfig,
    validated_args: Option<serde_json::Value>,
) -> RouterDecision {
    // Find best proposal
    let Some(proposal) = find_best_proposal(proposals, policies, config.min_confidence) else {
        return RouterDecision::NoMatch {
            reason: "No matching tool found above confidence threshold".to_string(),
        };
    };

    // Get policy
    let Some(policy) = policies.get(&proposal.tool) else {
        return RouterDecision::NoMatch {
            reason: format!("Tool '{}' not found in policies", proposal.tool),
        };
    };

    // Use validated args or original args
    let args = validated_args.unwrap_or_else(|| proposal.args.clone());

    // Validate args (if not pre-validated)
    if policy.validate_args(&args).is_err() {
        return RouterDecision::ArgsInvalid {
            tool: proposal.tool.clone(),
        };
    }

    // Determine execution mode
    if determine_execution_mode(policy, config) {
        RouterDecision::Execute {
            tool: proposal.tool.clone(),
            args,
            evidence: proposal.evidence.clone(),
        }
    } else {
        RouterDecision::RequireApproval {
            tool: proposal.tool.clone(),
            args,
            evidence: proposal.evidence.clone(),
        }
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
    fn test_decide_auto_run_read_only() {
        let proposals = vec![make_proposal("web_search", 0.9)];
        let mut policies = HashMap::new();
        policies.insert("web_search".to_string(), make_policy(true));

        let config = RouterConfig {
            auto_run_read_only: true,
            auto_run_all: false,
            current_mode: Mode::Global,
            min_confidence: 0.5,
        };

        let decision = decide(&proposals, &policies, &config, Some(serde_json::json!({})));
        assert!(matches!(decision, RouterDecision::Execute { .. }));
    }

    #[test]
    fn test_decide_require_approval_non_readonly() {
        let proposals = vec![make_proposal("typer", 0.9)];
        let mut policies = HashMap::new();
        policies.insert("typer".to_string(), make_policy(false)); // not read-only

        let config = RouterConfig {
            auto_run_read_only: true,
            auto_run_all: false,
            current_mode: Mode::Global,
            min_confidence: 0.5,
        };

        let decision = decide(&proposals, &policies, &config, Some(serde_json::json!({})));
        assert!(matches!(decision, RouterDecision::RequireApproval { .. }));
    }

    #[test]
    fn test_decide_no_match() {
        let proposals = vec![make_proposal("tool", 0.3)]; // below threshold
        let mut policies = HashMap::new();
        policies.insert("tool".to_string(), make_policy(true));

        let config = RouterConfig::default();
        let decision = decide(&proposals, &policies, &config, None);
        assert!(matches!(decision, RouterDecision::NoMatch { .. }));
    }

    #[test]
    fn test_auto_run_all_overrides() {
        let proposals = vec![make_proposal("dangerous_tool", 0.9)];
        let mut policies = HashMap::new();
        policies.insert("dangerous_tool".to_string(), make_policy(false)); // not read-only

        let config = RouterConfig {
            auto_run_read_only: false,
            auto_run_all: true, // Override - run everything
            current_mode: Mode::Global,
            min_confidence: 0.5,
        };

        let decision = decide(&proposals, &policies, &config, Some(serde_json::json!({})));
        assert!(matches!(decision, RouterDecision::Execute { .. }));
    }
}

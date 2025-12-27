//! Pure router logic - no IO, no async, fully testable.
//!
//! This module contains the decision-making logic for the router,
//! separated from Tauri state management and async IO.

use crate::functiongemma::Proposal;
use crate::tool_manifest::ToolPolicy;
use gibberish_context::Mode;
use std::collections::HashMap;

/// Default minimum confidence threshold for tool proposals.
pub const DEFAULT_MIN_CONFIDENCE: f32 = 0.35;

/// Default clarification threshold. Proposals with confidence between
/// min_confidence and this value trigger clarification requests.
pub const DEFAULT_CLARIFICATION_THRESHOLD: f32 = 0.50;

/// Configuration for router decision-making.
///
/// All policy values are configurable, with sensible defaults.
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Auto-execute read-only tools without approval.
    pub auto_run_read_only: bool,
    /// Auto-execute ALL tools without approval (dangerous, for testing).
    pub auto_run_all: bool,
    /// Current operating mode for filtering tools.
    pub current_mode: Mode,
    /// Minimum confidence threshold for tool proposals.
    pub min_confidence: f32,
    /// Confidence below this triggers clarification instead of execution.
    pub clarification_threshold: f32,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            auto_run_read_only: true,
            auto_run_all: false,
            current_mode: Mode::Global,
            min_confidence: DEFAULT_MIN_CONFIDENCE,
            clarification_threshold: DEFAULT_CLARIFICATION_THRESHOLD,
        }
    }
}

/// Find the best proposal above confidence threshold.
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
pub fn determine_execution_mode(policy: &ToolPolicy, config: &RouterConfig) -> bool {
    config.auto_run_all || (policy.read_only && config.auto_run_read_only)
}

/// Check if a proposal needs clarification due to low confidence.
///
/// Returns true if the proposal's confidence is between min_confidence
/// and clarification_threshold.
pub fn needs_clarification(proposal: &Proposal, config: &RouterConfig) -> bool {
    proposal.confidence >= config.min_confidence
        && proposal.confidence < config.clarification_threshold
}

/// Suggested clarification questions based on the proposal.
pub fn clarification_suggestions(proposal: &Proposal, user_text: &str) -> Vec<String> {
    let mut suggestions = Vec::new();
    suggestions.push(format!(
        "Did you mean to use '{}' for \"{}\"?",
        proposal.tool,
        truncate_text(user_text, 50)
    ));
    match proposal.tool.as_str() {
        "typer" => suggestions.push("What text would you like me to type?".to_string()),
        "web_search" => suggestions.push("What would you like me to search for?".to_string()),
        "app_launcher" => suggestions.push("Which application should I open?".to_string()),
        "system_control" => suggestions.push("What system action would you like?".to_string()),
        _ => {}
    }
    suggestions
}

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
            evidence: "test".to_string(),
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
            make_proposal("tool_b", 0.2),
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
        let policy = make_policy(true);
        let config = RouterConfig {
            auto_run_read_only: true,
            auto_run_all: false,
            current_mode: Mode::Global,
            min_confidence: 0.5,
            clarification_threshold: 0.6,
        };
        assert!(determine_execution_mode(&policy, &config));
    }

    #[test]
    fn test_determine_execution_mode_require_approval_non_readonly() {
        let policy = make_policy(false);
        let config = RouterConfig {
            auto_run_read_only: true,
            auto_run_all: false,
            current_mode: Mode::Global,
            min_confidence: 0.5,
            clarification_threshold: 0.6,
        };
        assert!(!determine_execution_mode(&policy, &config));
    }

    #[test]
    fn test_determine_execution_mode_auto_run_all_overrides() {
        let policy = make_policy(false);
        let config = RouterConfig {
            auto_run_read_only: false,
            auto_run_all: true,
            current_mode: Mode::Global,
            min_confidence: 0.5,
            clarification_threshold: 0.6,
        };
        assert!(determine_execution_mode(&policy, &config));
    }

    #[test]
    fn scenario_web_search_global() {
        let config = RouterConfig {
            current_mode: Mode::Global,
            min_confidence: 0.6,
            auto_run_read_only: true,
            ..Default::default()
        };
        let proposals = vec![make_proposal("web_search", 0.85)];
        let mut policies = HashMap::new();
        policies.insert("web_search".to_string(), make_policy(true));

        let best = find_best_proposal(&proposals, &policies, config.min_confidence).unwrap();
        assert_eq!(best.tool, "web_search");
        let policy = policies.get("web_search").unwrap();
        assert!(determine_execution_mode(policy, &config));
    }

    #[test]
    fn scenario_dangerous_git_dev() {
        let config = RouterConfig {
            current_mode: Mode::Dev,
            auto_run_read_only: true,
            ..Default::default()
        };
        let proposals = vec![make_proposal("git_reset", 0.90)];
        let mut policies = HashMap::new();
        policies.insert("git_reset".to_string(), make_policy(false));

        let best = find_best_proposal(&proposals, &policies, config.min_confidence).unwrap();
        assert_eq!(best.tool, "git_reset");
        let policy = policies.get("git_reset").unwrap();
        assert!(!determine_execution_mode(policy, &config));
    }

    #[test]
    fn scenario_clarification_low_confidence() {
        // Test uses default config: min_confidence=0.35, clarification_threshold=0.50
        // Proposal 0.40 falls in between, triggering clarification.
        let config = RouterConfig::default();
        let proposals = vec![make_proposal("app_launcher", 0.40)];
        let mut policies = HashMap::new();
        policies.insert("app_launcher".to_string(), make_policy(true));

        let best = find_best_proposal(&proposals, &policies, config.min_confidence).unwrap();
        assert!(needs_clarification(best, &config));
    }

    #[test]
    fn scenario_multi_tool_competition() {
        let config = RouterConfig::default();
        let proposals = vec![
            make_proposal("system_control", 0.4),
            make_proposal("app_launcher", 0.85),
        ];
        let mut policies = HashMap::new();
        policies.insert("app_launcher".to_string(), make_policy(true));
        policies.insert("system_control".to_string(), make_policy(true));

        let best = find_best_proposal(&proposals, &policies, config.min_confidence).unwrap();
        assert_eq!(best.tool, "app_launcher");
    }

    #[test]
    fn scenario_open_ended_chat_no_match() {
        let config = RouterConfig::default();
        let proposals = vec![];
        let mut policies = HashMap::new();
        policies.insert("web_search".to_string(), make_policy(true));

        let best = find_best_proposal(&proposals, &policies, config.min_confidence);
        assert!(best.is_none());
    }

    // --- Stress tests for high-frequency proposal processing ---

    #[test]
    fn stress_high_frequency_proposals() {
        // Simulate 1000 rapid-fire proposal batches (10 commits/second for 100s)
        let config = RouterConfig::default();
        let mut policies = HashMap::new();
        for i in 0..20 {
            policies.insert(format!("tool_{}", i), make_policy(i % 2 == 0));
        }

        for batch in 0..1000 {
            // Varying number of proposals per batch (1-5)
            let num_proposals = (batch % 5) + 1;
            let proposals: Vec<Proposal> = (0..num_proposals)
                .map(|i| {
                    let tool_idx = (batch + i) % 20;
                    let confidence = 0.3 + (((batch * i) % 60) as f32 / 100.0);
                    make_proposal(&format!("tool_{}", tool_idx), confidence)
                })
                .collect();

            let best = find_best_proposal(&proposals, &policies, config.min_confidence);

            // Verify best has highest confidence among those above threshold
            if let Some(winner) = best {
                for p in &proposals {
                    if p.confidence >= config.min_confidence && policies.contains_key(&p.tool) {
                        assert!(winner.confidence >= p.confidence);
                    }
                }
            }
        }
    }

    #[test]
    fn stress_rapid_mode_switching() {
        // Simulate rapid context switches (mode changes) during proposal evaluation
        let modes = [Mode::Global, Mode::Dev, Mode::Meeting, Mode::Writer];
        let mut policies = HashMap::new();
        policies.insert("read_only_tool".to_string(), make_policy(true));
        policies.insert("write_tool".to_string(), make_policy(false));

        for i in 0..500 {
            let config = RouterConfig {
                current_mode: modes[i % modes.len()],
                auto_run_read_only: i % 3 != 0,
                auto_run_all: i % 10 == 0,
                min_confidence: 0.3 + ((i % 20) as f32 / 100.0),
                clarification_threshold: 0.5 + ((i % 10) as f32 / 100.0),
            };

            let proposals = vec![
                make_proposal("read_only_tool", 0.7),
                make_proposal("write_tool", 0.8),
            ];

            let best = find_best_proposal(&proposals, &policies, config.min_confidence);
            if let Some(winner) = best {
                let policy = policies.get(&winner.tool).unwrap();
                let auto_run = determine_execution_mode(policy, &config);

                // Verify auto_run logic is consistent
                if config.auto_run_all {
                    assert!(auto_run);
                } else if policy.read_only && config.auto_run_read_only {
                    assert!(auto_run);
                } else if !policy.read_only && !config.auto_run_all {
                    assert!(!auto_run);
                }
            }
        }
    }

    #[test]
    fn stress_clarification_boundary() {
        // Test proposals at clarification boundary with varying thresholds
        let mut policies = HashMap::new();
        policies.insert("test_tool".to_string(), make_policy(true));

        for i in 0..100 {
            let min_conf = 0.30 + (i as f32 / 500.0);
            let clar_thresh = min_conf + 0.15;

            let config = RouterConfig {
                min_confidence: min_conf,
                clarification_threshold: clar_thresh,
                ..Default::default()
            };

            // Test at boundary: min_conf, midpoint, clar_thresh
            let boundary_values = [
                min_conf - 0.01,
                min_conf,
                min_conf + 0.01,
                (min_conf + clar_thresh) / 2.0,
                clar_thresh - 0.01,
                clar_thresh,
                clar_thresh + 0.01,
            ];

            for conf in boundary_values {
                let proposals = vec![make_proposal("test_tool", conf)];
                let best = find_best_proposal(&proposals, &policies, config.min_confidence);

                if conf < min_conf {
                    assert!(best.is_none(), "Should reject below min_confidence");
                } else {
                    assert!(best.is_some(), "Should accept at or above min_confidence");
                    let p = best.unwrap();
                    let needs_clar = needs_clarification(p, &config);
                    if conf < clar_thresh {
                        assert!(needs_clar, "Should need clarification below threshold");
                    } else {
                        assert!(!needs_clar, "Should not need clarification at/above threshold");
                    }
                }
            }
        }
    }

    #[test]
    fn stress_many_competing_tools() {
        // Simulate many tools competing for selection
        let config = RouterConfig::default();
        let mut policies = HashMap::new();

        // 100 tools with various policies
        for i in 0..100 {
            policies.insert(format!("tool_{}", i), make_policy(i % 3 == 0));
        }

        for round in 0..200 {
            // Create proposals for subset of tools with random-ish confidences
            let proposals: Vec<Proposal> = (0..50)
                .map(|i| {
                    let tool_idx = (round * 7 + i * 3) % 100;
                    let conf_base = ((round + i) % 70) as f32 / 100.0;
                    make_proposal(&format!("tool_{}", tool_idx), 0.25 + conf_base)
                })
                .collect();

            let best = find_best_proposal(&proposals, &policies, config.min_confidence);

            // Find expected winner manually
            let expected = proposals
                .iter()
                .filter(|p| p.confidence >= config.min_confidence && policies.contains_key(&p.tool))
                .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap());

            match (best, expected) {
                (Some(b), Some(e)) => assert_eq!(b.tool, e.tool),
                (None, None) => {}
                _ => panic!("Mismatch in best proposal selection"),
            }
        }
    }
}

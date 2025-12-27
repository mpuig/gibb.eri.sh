//! Tool chaining pipeline with formal limits and contracts.
//!
//! Replaces ad-hoc followup logic with explicit, testable pipeline.
//!
//! # Pipeline Flow
//!
//! ```text
//! User Intent → Primary Inference → Tool Exec → Followup Inference → Tool Exec → ...
//!                                      ↑              ↓
//!                                      └──────────────┘ (up to MAX_CHAIN_DEPTH)
//! ```
//!
//! # Chain Depth
//!
//! Max depth prevents infinite loops (e.g., summarize → search → summarize...).
//! Depth 1 means: primary tool + 1 followup tool.

/// Maximum number of followup tool executions after the primary tool.
///
/// Depth 0 = primary tool only (no followups)
/// Depth 1 = primary tool + 1 followup (current behavior)
/// Depth N = primary tool + N followups
pub const MAX_CHAIN_DEPTH: usize = 1;

/// Represents a step in the tool execution pipeline.
#[derive(Debug, Clone)]
pub struct PipelineStep {
    /// Tool name
    pub tool: String,
    /// Tool arguments
    pub args: serde_json::Value,
    /// Evidence/reasoning for this tool call
    pub evidence: String,
    /// Depth in chain (0 = primary, 1+ = followup)
    pub depth: usize,
}

/// Context passed through the pipeline.
#[derive(Debug, Clone)]
pub struct PipelineContext {
    /// Current chain depth
    pub depth: usize,
}

impl PipelineContext {
    /// Create new pipeline context for primary inference.
    pub fn new() -> Self {
        Self { depth: 0 }
    }

    /// Check if we can continue chaining.
    pub fn can_chain(&self) -> bool {
        self.depth < MAX_CHAIN_DEPTH
    }

    // Depth advancement is handled by callers when constructing followup steps.
}

/// Decision after a tool execution.
#[derive(Debug)]
pub enum ChainDecision {
    /// Execute another tool (within depth limit)
    Continue(PipelineStep),
    /// No more tools to execute
    Stop,
    /// Depth limit reached, cannot continue
    LimitReached,
}

/// Determine if chaining should continue after tool execution.
///
/// # Arguments
/// * `ctx` - Pipeline context
/// * `proposals` - Model's followup proposals
/// * `min_confidence` - Minimum confidence threshold
/// * `tool_filter` - Function to check if tool is allowed
///
/// # Returns
/// ChainDecision indicating whether to continue chaining
pub fn should_chain<F>(
    ctx: &PipelineContext,
    proposals: &[crate::functiongemma::Proposal],
    min_confidence: f32,
    tool_filter: F,
) -> ChainDecision
where
    F: Fn(&str) -> bool,
{
    // Check depth limit
    if !ctx.can_chain() {
        return ChainDecision::LimitReached;
    }

    // Find best valid proposal
    for proposal in proposals {
        if proposal.confidence >= min_confidence && tool_filter(&proposal.tool) {
            return ChainDecision::Continue(PipelineStep {
                tool: proposal.tool.clone(),
                args: proposal.args.clone(),
                evidence: proposal.evidence.clone(),
                depth: ctx.depth + 1,
            });
        }
    }

    ChainDecision::Stop
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::functiongemma::Proposal;

    fn make_proposal(tool: &str, confidence: f32) -> Proposal {
        Proposal {
            tool: tool.to_string(),
            args: serde_json::json!({}),
            evidence: "test".to_string(),
            confidence,
        }
    }

    #[test]
    fn test_can_chain_at_zero_depth() {
        let ctx = PipelineContext::new();
        assert!(ctx.can_chain());
    }

    #[test]
    fn test_cannot_chain_at_max_depth() {
        let ctx = PipelineContext {
            depth: MAX_CHAIN_DEPTH,
        };
        assert!(!ctx.can_chain());
    }

    #[test]
    fn test_chain_decision_respects_depth_limit() {
        let ctx = PipelineContext {
            depth: MAX_CHAIN_DEPTH,
        };
        let proposals = vec![make_proposal("typer", 0.9)];

        let decision = should_chain(&ctx, &proposals, 0.5, |_| true);
        assert!(matches!(decision, ChainDecision::LimitReached));
    }

    #[test]
    fn test_chain_decision_finds_valid_proposal() {
        let ctx = PipelineContext::new();
        let proposals = vec![
            make_proposal("blocked_tool", 0.95),
            make_proposal("allowed_tool", 0.9),
        ];

        let decision = should_chain(&ctx, &proposals, 0.5, |t| t == "allowed_tool");
        match decision {
            ChainDecision::Continue(step) => {
                assert_eq!(step.tool, "allowed_tool");
                assert_eq!(step.depth, 1);
            }
            _ => panic!("Expected Continue"),
        }
    }

    #[test]
    fn test_chain_decision_respects_confidence() {
        let ctx = PipelineContext::new();
        let proposals = vec![make_proposal("typer", 0.3)];

        let decision = should_chain(&ctx, &proposals, 0.5, |_| true);
        assert!(matches!(decision, ChainDecision::Stop));
    }

    #[test]
    fn test_chain_decision_stops_when_no_proposals() {
        let ctx = PipelineContext::new();
        let decision = should_chain(&ctx, &[], 0.5, |_| true);
        assert!(matches!(decision, ChainDecision::Stop));
    }
}

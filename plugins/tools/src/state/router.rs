//! Router state for action detection and dispatch.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::registry::ToolRegistry;
use crate::tool_manifest;
use crate::tool_manifest::ToolPolicy;
use gibberish_context::Mode;

/// State for the action router.
pub struct RouterState {
    pub enabled: bool,
    pub auto_run_read_only: bool,
    /// Auto-run ALL tools without approval (dangerous, for testing only).
    pub auto_run_all: bool,
    pub default_lang: String,
    pub tool_manifest: Arc<str>,
    pub tool_policies: Arc<HashMap<String, ToolPolicy>>,
    pub functiongemma_instructions: Arc<str>,
    pub functiongemma_declarations: Arc<str>,
    pub functiongemma_developer_context: Arc<str>,
    pub min_confidence: f32,
    /// Generic cooldown tracking: maps cooldown_key -> last execution time.
    pub cooldowns: HashMap<String, Instant>,
    pub pending_text: String,
    pub inflight: bool,
    pub infer_cancel: CancellationToken,
    /// Notify to wake up the router when new text arrives.
    pub text_notify: Arc<Notify>,
}

impl std::fmt::Debug for RouterState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouterState")
            .field("enabled", &self.enabled)
            .field("auto_run_read_only", &self.auto_run_read_only)
            .field("pending_text", &self.pending_text)
            .field("inflight", &self.inflight)
            .finish_non_exhaustive()
    }
}

impl RouterState {
    /// Update the manifest and instructions for a new mode.
    pub fn update_for_mode(&mut self, mode: Mode) {
        let registry = ToolRegistry::build_all();

        // Build dynamic manifest
        let manifest_json = registry.manifest_json_for_mode(mode);
        self.tool_manifest = Arc::from(manifest_json.clone());

        // Compile policies + declarations from the same manifest so the model sees
        // the exact function declaration format that our parser expects.
        let compiled = tool_manifest::validate_and_compile(&manifest_json).unwrap_or_else(|err| {
            tracing::warn!(mode = %mode, error = %err, "Failed to compile tool manifest");
            tool_manifest::CompiledManifest::default()
        });
        self.tool_policies = Arc::new(compiled.policies);
        self.functiongemma_declarations = Arc::from(compiled.function_declarations);

        // Build dynamic instructions
        let instructions = registry.functiongemma_instructions_for_mode(mode);
        self.functiongemma_instructions = Arc::from(instructions.clone());

        // Rebuild developer context
        let declarations = self.functiongemma_declarations.clone();
        self.functiongemma_developer_context = Arc::from(format!(
            "You are a model that can do function calling with the following functions\n{}\n{}",
            instructions, declarations
        ));

        tracing::debug!(
            mode = %mode,
            tool_count = self.tool_policies.len(),
            "Updated router manifest for mode"
        );
    }
}

impl Default for RouterState {
    fn default() -> Self {
        // Build with Global mode by default
        let registry = ToolRegistry::build_all();
        let mode = Mode::Global;

        let manifest_json = registry.manifest_json_for_mode(mode);
        let tool_manifest: Arc<str> = Arc::from(manifest_json.clone());

        let compiled = tool_manifest::validate_and_compile(&manifest_json).unwrap_or_default();
        let tool_policies: Arc<HashMap<String, ToolPolicy>> = Arc::new(compiled.policies);

        let functiongemma_instructions: Arc<str> =
            Arc::from(registry.functiongemma_instructions_for_mode(mode));
        let functiongemma_declarations: Arc<str> = Arc::from(compiled.function_declarations);
        let functiongemma_developer_context: Arc<str> = Arc::from(format!(
            "You are a model that can do function calling with the following functions\n{}\n{}",
            functiongemma_instructions, functiongemma_declarations
        ));

        Self {
            enabled: true,
            auto_run_read_only: true,
            auto_run_all: true, // Default to true for testing; disable in production
            default_lang: "en".to_string(),
            tool_manifest,
            tool_policies,
            functiongemma_instructions,
            functiongemma_declarations,
            functiongemma_developer_context,
            min_confidence: 0.35,
            cooldowns: HashMap::new(),
            pending_text: String::new(),
            inflight: false,
            infer_cancel: CancellationToken::new(),
            text_notify: Arc::new(Notify::new()),
        }
    }
}

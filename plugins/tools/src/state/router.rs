//! Router state for action detection and dispatch.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::tool_manifest::ToolPolicy;

/// State for the action router.
pub struct RouterState {
    pub enabled: bool,
    pub auto_run_read_only: bool,
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

impl Default for RouterState {
    fn default() -> Self {
        let tool_manifest: Arc<str> = Arc::from(
            r#"{
  "tools": [
    {
      "name": "wikipedia_city_lookup",
      "description": "Lookup a city on Wikipedia and return a short summary and URL.",
      "read_only": true,
      "args_schema": {
        "type": "object",
        "properties": {
          "city": { "type": "string", "description": "City name only (no extra words)." },
          "lang": { "type": "string", "description": "Wikipedia language code, e.g. en, es, ca.", "default": "en" },
          "sentences": { "type": "integer", "description": "How many sentences to return (1-10).", "minimum": 1, "maximum": 10, "default": 2 }
        },
        "required": ["city"]
      }
    }
  ]
}"#
                .to_string(),
        );

        let compiled =
            crate::tool_manifest::validate_and_compile(&tool_manifest).unwrap_or_default();
        let tool_policies: Arc<HashMap<String, ToolPolicy>> = Arc::new(compiled.policies);

        let functiongemma_instructions: Arc<str> = Arc::from(
            "You are an action router that reads live transcript commits.\n\
You do not chat. You never write natural language.\n\
\n\
CRITICAL RULES:\n\
1. ONLY call wikipedia_city_lookup if a city name appears VERBATIM in the user text.\n\
2. The city argument must be COPIED EXACTLY from the user text.\n\
3. If NO city name appears in the text, output <end_of_turn> immediately.\n\
4. Generic words like 'city', 'town', 'place' are NOT city names.\n\
\n\
Format: <start_function_call>call:wikipedia_city_lookup{city:<escape>CITY_FROM_TEXT<escape>}<end_function_call>\n"
                .to_string(),
        );
        let functiongemma_declarations: Arc<str> = Arc::from(compiled.function_declarations);
        let functiongemma_developer_context: Arc<str> = Arc::from(format!(
            "You are a model that can do function calling with the following functions\n{functiongemma_instructions}\n{functiongemma_declarations}"
        ));

        Self {
            enabled: true,
            auto_run_read_only: true,
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

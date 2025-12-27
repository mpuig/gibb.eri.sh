//! Tool registry for settings-driven tool dispatch.
//!
//! The registry builds tool instances based on the tool manifest policies,
//! allowing dynamic tool dispatch without hardcoded builders.

use std::collections::HashMap;
use std::sync::Arc;

use crate::skill_loader::SkillManager;
use crate::tool_manifest::ToolPolicy;
use crate::tools::{
    AddTodoTool, AppLauncherTool, FileFinderTool, GitVoiceTool, PasteTool, SystemControlTool, Tool,
    ToolDefinition, TranscriptMarkerTool, TyperTool, WebSearchTool,
};
use gibberish_context::Mode;

/// Registry of available tools.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

/// All known tool names (for building a complete registry).
const ALL_TOOL_NAMES: &[&str] = &[
    "web_search",
    "system_control",
    "app_launcher",
    "git_voice",
    "file_finder",
    "add_todo",
    "transcript_marker",
    "typer",
    "paste",
];

impl ToolRegistry {
    /// Create a registry with all known tools (built-in only).
    pub fn build_all() -> Self {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();

        for name in ALL_TOOL_NAMES {
            if let Some(tool) = create_tool(name) {
                tools.insert((*name).to_string(), tool);
            }
        }

        Self { tools }
    }

    /// Create a registry with all built-in tools plus skill tools.
    pub fn build_with_skills(skill_manager: &SkillManager) -> Self {
        let mut registry = Self::build_all();
        registry.register_skills(skill_manager);
        registry
    }

    /// Register skill tools from a skill manager.
    pub fn register_skills(&mut self, skill_manager: &SkillManager) {
        for loaded in skill_manager.skills() {
            for tool in &loaded.tools {
                // Clone the GenericSkillTool and wrap in Arc
                let tool_arc: Arc<dyn Tool> = Arc::new(tool.clone());
                self.tools.insert(tool.name().to_string(), tool_arc);
            }
        }

        tracing::debug!(
            skill_count = skill_manager.skill_count(),
            total_tools = self.tools.len(),
            "Registered skill tools"
        );
    }

    /// Create a registry from tool policies.
    ///
    /// Only tools that have both a policy entry and a known implementation
    /// will be registered.
    pub fn from_policies(policies: &HashMap<String, ToolPolicy>) -> Self {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();

        for name in policies.keys() {
            if let Some(tool) = create_tool(name) {
                tools.insert(name.clone(), tool);
            }
        }

        Self { tools }
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// Check if a tool is registered.
    #[cfg(test)]
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get all registered tool names.
    #[cfg(test)]
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.tools.keys().map(|s| s.as_str())
    }

    /// Get all tools available in the given mode.
    pub fn tools_for_mode(&self, mode: Mode) -> Vec<Arc<dyn Tool>> {
        self.tools
            .values()
            .filter(|tool| tool.is_available_in(mode))
            .cloned()
            .collect()
    }

    /// Get tool names available in the given mode.
    #[cfg(test)]
    pub fn names_for_mode(&self, mode: Mode) -> Vec<&str> {
        self.tools
            .iter()
            .filter(|(_, tool)| tool.is_available_in(mode))
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// Get tool definitions for the given mode (for building dynamic manifests).
    pub fn definitions_for_mode(&self, mode: Mode) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .filter(|tool| tool.is_available_in(mode))
            .map(|tool| tool.definition())
            .collect()
    }

    /// Build tool policies for the given mode.
    #[cfg(test)]
    pub fn policies_for_mode(&self, mode: Mode) -> HashMap<String, ToolPolicy> {
        self.tools
            .iter()
            .filter(|(_, tool)| tool.is_available_in(mode))
            .map(|(name, tool)| {
                let def = tool.definition();
                let required_args = def
                    .args_schema
                    .get("required")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                (
                    name.clone(),
                    ToolPolicy {
                        read_only: def.read_only,
                        default_lang: None,
                        default_sentences: None,
                        required_args,
                        arg_types: HashMap::new(),
                    },
                )
            })
            .collect()
    }

    /// Build the tool manifest JSON for the given mode.
    pub fn manifest_json_for_mode(&self, mode: Mode) -> String {
        let definitions = self.definitions_for_mode(mode);
        let manifest = serde_json::json!({
            "tools": definitions
        });
        serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| "{}".to_string())
    }

    /// Build FunctionGemma instructions for the given mode.
    pub fn functiongemma_instructions_for_mode(&self, mode: Mode) -> String {
        let tools = self.tools_for_mode(mode);
        let tool_names: Vec<String> = tools.iter().map(|t| t.name().to_string()).collect();

        if tool_names.is_empty() {
            return "You are an action router. No tools are available in this mode. Output <end_of_turn> immediately.".to_string();
        }

        let tool_list = tool_names.join(", ");

        // Build selection hints dynamically from tools
        let selection_hints: Vec<String> = tools
            .iter()
            .filter_map(|t| {
                t.selection_hint()
                    .map(|hint| format!("- {} → {}", hint, t.name()))
            })
            .collect();
        let selection_text = selection_hints.join("\n");

        // Collect examples from tools (supports both static and dynamic examples)
        let examples: Vec<String> = tools
            .iter()
            .flat_map(|t| t.owned_few_shot_examples())
            .collect();
        let examples_text = examples.join("\n\n");

        tracing::debug!(
            example_count = examples.len(),
            selection_count = selection_hints.len(),
            "FunctionGemma instructions built"
        );

        format!(
            "You are an action router. Match user intent to ONE tool.\n\
            \n\
            Tools: {tool_list}\n\
            \n\
            TOOL SELECTION:\n\
            {selection_text}\n\
            \n\
            Examples:\n\
            {examples_text}"
        )
    }

    // `functiongemma_declarations_for_mode` is intentionally omitted: production code
    // compiles declarations from the manifest directly in the router.
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Create a tool instance by name.
fn create_tool(name: &str) -> Option<Arc<dyn Tool>> {
    match name {
        // Global tools
        "web_search" => Some(Arc::new(WebSearchTool)),
        "system_control" => Some(Arc::new(SystemControlTool)),
        "app_launcher" => Some(Arc::new(AppLauncherTool)),
        // Dev mode tools
        "git_voice" => Some(Arc::new(GitVoiceTool)),
        "file_finder" => Some(Arc::new(FileFinderTool)),
        // Meeting mode tools
        "add_todo" => Some(Arc::new(AddTodoTool)),
        "transcript_marker" => Some(Arc::new(TranscriptMarkerTool)),
        // Global tools (with special permissions)
        "typer" => Some(Arc::new(TyperTool)),
        "paste" => Some(Arc::new(PasteTool)),
        _ => None,
    }
}

impl std::fmt::Debug for dyn Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tool({})", self.name())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_known_tool() {
        let tool = create_tool("web_search");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name(), "web_search");
    }

    #[test]
    fn test_create_unknown_tool() {
        let tool = create_tool("unknown_tool");
        assert!(tool.is_none());
    }

    #[test]
    fn test_registry_from_policies() {
        let mut policies = HashMap::new();
        policies.insert(
            "web_search".to_string(),
            ToolPolicy {
                read_only: true,
                default_lang: Some("en".to_string()),
                default_sentences: Some(3),
                required_args: vec!["query".to_string()],
                arg_types: HashMap::new(),
            },
        );
        policies.insert(
            "unknown_tool".to_string(),
            ToolPolicy {
                read_only: true,
                default_lang: None,
                default_sentences: None,
                required_args: vec![],
                arg_types: HashMap::new(),
            },
        );

        let registry = ToolRegistry::from_policies(&policies);

        assert!(registry.contains("web_search"));
        assert!(!registry.contains("unknown_tool"));
    }
}

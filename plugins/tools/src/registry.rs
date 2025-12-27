//! Tool registry for settings-driven tool dispatch.
//!
//! The registry builds tool instances based on the tool manifest policies,
//! allowing dynamic tool dispatch without hardcoded builders.

use std::collections::HashMap;
use std::sync::Arc;

use crate::tool_manifest::ToolPolicy;
use crate::tools::{
    AddTodoTool, AppLauncherTool, FileFinderTool, GitVoiceTool, SystemControlTool,
    Tool, ToolDefinition, ToolInfo, ToolInfoProvider, TranscriptMarkerTool, WikipediaTool,
};
use gibberish_context::Mode;

/// Registry of available tools.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

/// All known tool names (for building a complete registry).
const ALL_TOOL_NAMES: &[&str] = &[
    "wikipedia_city_lookup",
    "system_control",
    "app_launcher",
    "git_voice",
    "file_finder",
    "add_todo",
    "transcript_marker",
];

impl ToolRegistry {
    /// Create a registry with all known tools.
    pub fn build_all() -> Self {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();

        for name in ALL_TOOL_NAMES {
            if let Some(tool) = create_tool(name) {
                tools.insert((*name).to_string(), tool);
            }
        }

        Self { tools }
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
    #[allow(dead_code)]
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get all registered tool names.
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name()).collect();

        if tool_names.is_empty() {
            return "You are an action router. No tools are available in this mode. Output <end_of_turn> immediately.".to_string();
        }

        let tool_list = tool_names.join(", ");

        format!(
            "You are an action router that reads live transcript commits.\n\
            You do not chat. You never write natural language.\n\
            \n\
            Available tools in {mode} mode: {tool_list}\n\
            \n\
            CRITICAL RULES:\n\
            1. ONLY call tools if the user's intent clearly matches a tool's purpose.\n\
            2. Extract arguments EXACTLY from the user text - do not invent values.\n\
            3. If NO tool matches the user's intent, output <end_of_turn> immediately.\n\
            4. Generic phrases like 'do something' are NOT actionable.\n\
            \n\
            OUTPUT FORMAT:\n\
            <start_function_call>call:TOOL_NAME{{arg:<escape>value<escape>}}<end_function_call>\n\
            \n\
            EXAMPLE: User says 'tell me about Barcelona'\n\
            <start_function_call>call:wikipedia_city_lookup{{city:<escape>Barcelona<escape>}}<end_function_call>\n"
        )
    }

    /// Build FunctionGemma function declarations for the given mode.
    pub fn functiongemma_declarations_for_mode(&self, mode: Mode) -> String {
        let definitions = self.definitions_for_mode(mode);
        let declarations: Vec<String> = definitions
            .iter()
            .map(|def| {
                let args_desc = if let Some(props) = def.args_schema.get("properties") {
                    if let Some(obj) = props.as_object() {
                        let arg_names: Vec<&str> = obj.keys().map(|s| s.as_str()).collect();
                        if arg_names.is_empty() {
                            "no args".to_string()
                        } else {
                            format!("args: {}", arg_names.join(", "))
                        }
                    } else {
                        "no args".to_string()
                    }
                } else {
                    "no args".to_string()
                };
                format!("- {}: {} ({})", def.name, def.description, args_desc)
            })
            .collect();
        declarations.join("\n")
    }
}

/// Implement ToolInfoProvider for ToolRegistry.
/// This allows the help tool to query available tools without tight coupling.
impl ToolInfoProvider for ToolRegistry {
    fn get_tools_for_mode(&self, mode: Mode) -> Vec<ToolInfo> {
        self.tools
            .values()
            .filter(|t| t.is_available_in(mode))
            .map(|t| ToolInfo {
                name: t.name().to_string(),
                description: t.description().to_string(),
                examples: t.example_phrases().iter().map(|s| s.to_string()).collect(),
                modes: if t.modes().is_empty() {
                    vec!["Global".to_string()]
                } else {
                    t.modes().iter().map(|m| m.to_string()).collect()
                },
            })
            .collect()
    }
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
        "wikipedia_city_lookup" => Some(Arc::new(WikipediaTool)),
        "system_control" => Some(Arc::new(SystemControlTool)),
        "app_launcher" => Some(Arc::new(AppLauncherTool)),
        // Dev mode tools
        "git_voice" => Some(Arc::new(GitVoiceTool)),
        "file_finder" => Some(Arc::new(FileFinderTool)),
        // Meeting mode tools
        "add_todo" => Some(Arc::new(AddTodoTool)),
        "transcript_marker" => Some(Arc::new(TranscriptMarkerTool)),
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
        let tool = create_tool("wikipedia_city_lookup");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name(), "wikipedia_city_lookup");
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
            "wikipedia_city_lookup".to_string(),
            ToolPolicy {
                read_only: true,
                default_lang: Some("en".to_string()),
                default_sentences: Some(2),
                required_args: vec!["city".to_string()],
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

        assert!(registry.contains("wikipedia_city_lookup"));
        assert!(!registry.contains("unknown_tool"));
    }
}

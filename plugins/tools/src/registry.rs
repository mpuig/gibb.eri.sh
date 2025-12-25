//! Tool registry for settings-driven tool dispatch.
//!
//! The registry builds tool instances based on the tool manifest policies,
//! allowing dynamic tool dispatch without hardcoded builders.

use std::collections::HashMap;
use std::sync::Arc;

use crate::tool_manifest::ToolPolicy;
use crate::tools::{Tool, WikipediaTool};

/// Registry of available tools.
#[derive(Debug)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
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
}

/// Create a tool instance by name.
fn create_tool(name: &str) -> Option<Arc<dyn Tool>> {
    match name {
        "wikipedia_city_lookup" => Some(Arc::new(WikipediaTool)),
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

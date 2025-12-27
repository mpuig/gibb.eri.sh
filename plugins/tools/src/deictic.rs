//! Deictic reference resolution.
//!
//! Handles references like 'this', 'selection', 'clipboard' in tool arguments.
//! These references are resolved at execution time to actual values.
//!
//! Currently supported references:
//! - `clipboard`: Resolved via arboard clipboard access
//!
//! Planned references (not yet implemented):
//! - `selection`: Currently selected text (requires Accessibility API)
//! - `last_transcript`: Recent transcript text (requires STT history)

use serde::{Deserialize, Serialize};

/// A deictic reference that can be resolved to a concrete value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Reference {
    /// Content from the system clipboard.
    Clipboard,

    /// Currently selected text (requires Accessibility API).
    Selection,

    /// Recent transcript text.
    LastTranscript {
        /// Number of seconds to look back. Defaults to 30.
        #[serde(default)]
        seconds: Option<u32>,
    },

    /// A literal string value (already resolved).
    Literal { value: String },
}

impl Reference {
    /// Check if this reference requires external resolution.
    #[allow(dead_code)]
    pub fn needs_resolution(&self) -> bool {
        !matches!(self, Reference::Literal { .. })
    }
}

/// Trait for providing clipboard content.
pub trait ClipboardProvider: Send + Sync {
    /// Get the current clipboard text content.
    fn get_text(&self) -> Option<String>;
}

/// Trait for providing selected text.
pub trait SelectionProvider: Send + Sync {
    /// Get the currently selected text in the active application.
    fn get_selected_text(&self) -> Option<String>;
}

/// Trait for providing transcript history.
pub trait TranscriptProvider: Send + Sync {
    /// Get transcript text from the last N seconds.
    fn get_last(&self, seconds: u32) -> Option<String>;
}

/// Context for resolving deictic references.
pub struct ResolverContext<'a> {
    pub clipboard: Option<&'a dyn ClipboardProvider>,
    pub selection: Option<&'a dyn SelectionProvider>,
    pub transcript: Option<&'a dyn TranscriptProvider>,
}

impl<'a> Default for ResolverContext<'a> {
    fn default() -> Self {
        Self {
            clipboard: None,
            selection: None,
            transcript: None,
        }
    }
}

/// Error during reference resolution.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("clipboard is empty")]
    ClipboardEmpty,

    #[error("no text selected")]
    NoSelection,

    #[error("no recent transcript available")]
    NoTranscript,

    #[error("provider not available: {0}")]
    ProviderUnavailable(&'static str),
}

/// Resolve a deictic reference to a concrete string value.
pub fn resolve_reference(
    reference: &Reference,
    ctx: &ResolverContext<'_>,
) -> Result<String, ResolveError> {
    match reference {
        Reference::Clipboard => {
            let provider = ctx
                .clipboard
                .ok_or(ResolveError::ProviderUnavailable("clipboard"))?;
            provider.get_text().ok_or(ResolveError::ClipboardEmpty)
        }
        Reference::Selection => {
            let provider = ctx
                .selection
                .ok_or(ResolveError::ProviderUnavailable("selection"))?;
            provider
                .get_selected_text()
                .ok_or(ResolveError::NoSelection)
        }
        Reference::LastTranscript { seconds } => {
            let provider = ctx
                .transcript
                .ok_or(ResolveError::ProviderUnavailable("transcript"))?;
            let secs = seconds.unwrap_or(30);
            provider.get_last(secs).ok_or(ResolveError::NoTranscript)
        }
        Reference::Literal { value } => Ok(value.clone()),
    }
}

/// Walk a JSON value and resolve any deictic references found.
///
/// References are detected by objects with a "type" field matching
/// one of: "clipboard", "selection", "last_transcript", "literal".
pub fn resolve_args(
    args: &serde_json::Value,
    ctx: &ResolverContext<'_>,
) -> Result<serde_json::Value, ResolveError> {
    match args {
        serde_json::Value::Object(map) => {
            // Check if this object is a Reference
            if let Some(type_val) = map.get("type") {
                if let Some(type_str) = type_val.as_str() {
                    if matches!(
                        type_str,
                        "clipboard" | "selection" | "last_transcript" | "literal"
                    ) {
                        // Try to parse as Reference
                        if let Ok(reference) = serde_json::from_value::<Reference>(
                            serde_json::Value::Object(map.clone()),
                        ) {
                            let resolved = resolve_reference(&reference, ctx)?;
                            return Ok(serde_json::Value::String(resolved));
                        }
                    }
                }
            }

            // Not a reference, recurse into children
            let mut result = serde_json::Map::new();
            for (key, value) in map {
                result.insert(key.clone(), resolve_args(value, ctx)?);
            }
            Ok(serde_json::Value::Object(result))
        }
        serde_json::Value::Array(arr) => {
            let resolved: Result<Vec<_>, _> = arr.iter().map(|v| resolve_args(v, ctx)).collect();
            Ok(serde_json::Value::Array(resolved?))
        }
        // Primitives pass through unchanged
        other => Ok(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockClipboard(String);
    impl ClipboardProvider for MockClipboard {
        fn get_text(&self) -> Option<String> {
            Some(self.0.clone())
        }
    }

    #[test]
    fn test_resolve_clipboard() {
        let clipboard = MockClipboard("hello from clipboard".to_string());
        let ctx = ResolverContext {
            clipboard: Some(&clipboard),
            ..Default::default()
        };

        let reference = Reference::Clipboard;
        let resolved = resolve_reference(&reference, &ctx).unwrap();
        assert_eq!(resolved, "hello from clipboard");
    }

    #[test]
    fn test_resolve_literal() {
        let ctx = ResolverContext::default();
        let reference = Reference::Literal {
            value: "literal text".to_string(),
        };
        let resolved = resolve_reference(&reference, &ctx).unwrap();
        assert_eq!(resolved, "literal text");
    }

    #[test]
    fn test_resolve_args_nested() {
        let clipboard = MockClipboard("clipboard content".to_string());
        let ctx = ResolverContext {
            clipboard: Some(&clipboard),
            ..Default::default()
        };

        let args = serde_json::json!({
            "text": { "type": "clipboard" },
            "format": "markdown"
        });

        let resolved = resolve_args(&args, &ctx).unwrap();
        assert_eq!(resolved["text"], "clipboard content");
        assert_eq!(resolved["format"], "markdown");
    }

    #[test]
    fn test_resolve_args_array() {
        let clipboard = MockClipboard("clipboard".to_string());
        let ctx = ResolverContext {
            clipboard: Some(&clipboard),
            ..Default::default()
        };

        let args = serde_json::json!({
            "items": [
                { "type": "clipboard" },
                { "type": "literal", "value": "literal" }
            ]
        });

        let resolved = resolve_args(&args, &ctx).unwrap();
        assert_eq!(resolved["items"][0], "clipboard");
        assert_eq!(resolved["items"][1], "literal");
    }
}

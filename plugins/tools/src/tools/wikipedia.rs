//! Wikipedia city lookup tool implementation.

use super::{Tool, ToolContext, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::json;

const DEFAULT_SENTENCES: u8 = 2;

/// Tool for looking up city information on Wikipedia.
pub struct WikipediaTool;

#[async_trait]
impl Tool for WikipediaTool {
    fn name(&self) -> &'static str {
        "wikipedia_city_lookup"
    }

    fn description(&self) -> &'static str {
        "Look up city information from Wikipedia"
    }

    fn example_phrases(&self) -> &'static [&'static str] {
        &[
            "tell me about Barcelona",
            "what is Tokyo",
            "lookup New York",
        ]
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "City name only (no extra words)."
                },
                "lang": {
                    "type": "string",
                    "description": "Wikipedia language code, e.g. en, es, ca.",
                    "default": "en"
                },
                "sentences": {
                    "type": "integer",
                    "description": "How many sentences to return (1-10).",
                    "minimum": 1,
                    "maximum": 10,
                    "default": 2
                }
            },
            "required": ["city"]
        })
    }

    fn cache_key(&self, args: &serde_json::Value) -> Option<String> {
        let city = args.get("city")?.as_str()?;
        let lang = args.get("lang").and_then(|v| v.as_str()).unwrap_or("en");
        Some(format!("{}:{}", lang, city.to_lowercase()))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let city = args
            .get("city")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or(ToolError::MissingArg("city"))?;

        let lang = args
            .get("lang")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| ctx.default_lang.clone());

        let sentences = args
            .get("sentences")
            .and_then(|v| v.as_u64())
            .map(|n| (n as u8).clamp(1, 10))
            .unwrap_or(DEFAULT_SENTENCES);

        let summary =
            crate::wikipedia::fetch_city_summary_with_client(ctx.client(), &lang, city, sentences)
                .await?;

        // Build cache/cooldown key from lang + normalized city
        let cache_key = format!("{}:{}", lang, city.to_lowercase());

        // Return ready-to-emit payload (frontend expects { city, result })
        Ok(ToolResult {
            event_name: "tools:wikipedia_city",
            payload: serde_json::json!({
                "city": summary.title,
                "result": summary,
            }),
            cache_key: Some(cache_key.clone()),
            cooldown_key: Some(cache_key),
        })
    }
}

//! Web search tool implementation.
//!
//! Generic search tool that can query different sources.
//! Currently supports Wikipedia (default), extensible to other sources.

use super::{Tool, ToolContext, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::borrow::Cow;

const DEFAULT_SENTENCES: u8 = 3;

/// Tool for searching the web for information on any topic.
pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("web_search")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Search for information about a topic, look up facts, or answer questions")
    }

    fn example_phrases(&self) -> &'static [&'static str] {
        &[
            "tell me about Barcelona",
            "what is quantum computing",
            "search for machine learning",
            "look up Albert Einstein",
        ]
    }

    fn few_shot_examples(&self) -> &'static [&'static str] {
        &[
            "User: search for quantum physics\n<start_function_call>call:web_search{query:<escape>quantum physics<escape>}<end_function_call>",
            "User: what is quantum computing\n<start_function_call>call:web_search{query:<escape>quantum computing<escape>}<end_function_call>",
            "User: tell me about Barcelona\n<start_function_call>call:web_search{query:<escape>Barcelona<escape>}<end_function_call>",
            "User: look up machine learning\n<start_function_call>call:web_search{query:<escape>machine learning<escape>}<end_function_call>",
        ]
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The topic or question to search for."
                },
                "source": {
                    "type": "string",
                    "description": "Search source to use. Currently only 'wikipedia' is supported.",
                    "enum": ["wikipedia"],
                    "default": "wikipedia"
                },
                "lang": {
                    "type": "string",
                    "description": "Language code for results, e.g. en, es, ca.",
                    "default": "en"
                }
            },
            "required": ["query"]
        })
    }

    fn cache_key(&self, args: &serde_json::Value) -> Option<String> {
        let query = args.get("query")?.as_str()?;
        let source = args
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("wikipedia");
        let lang = args.get("lang").and_then(|v| v.as_str()).unwrap_or("en");
        Some(format!("{}:{}:{}", source, lang, query.to_lowercase()))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or(ToolError::MissingArg("query"))?;

        let source = args
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("wikipedia");

        let lang = args
            .get("lang")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| ctx.default_lang.clone());

        match source {
            "wikipedia" => {
                let summary = crate::wikipedia::fetch_city_summary_with_client(
                    ctx.client(),
                    &lang,
                    query,
                    DEFAULT_SENTENCES,
                )
                .await?;

                let cache_key = format!("wikipedia:{}:{}", lang, query.to_lowercase());

                Ok(ToolResult {
                    event_name: Cow::Borrowed("tools:search_result"),
                    payload: json!({
                        "query": query,
                        "source": "wikipedia",
                        "result": {
                            "title": summary.title,
                            "summary": summary.summary,
                            "url": summary.url,
                            "thumbnail_url": summary.thumbnail_url,
                        },
                    }),
                    cache_key: Some(cache_key.clone()),
                    cooldown_key: Some(cache_key),
                })
            }
            _ => Err(ToolError::InvalidArg {
                field: "source",
                reason: "unsupported search source".to_string(),
            }),
        }
    }
}

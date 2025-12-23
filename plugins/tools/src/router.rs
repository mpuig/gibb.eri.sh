use crate::state::{CacheEntry, CacheKey, ToolsState, WikiSummaryDto};
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager, Runtime};

const ROUTER_CITY_COOLDOWN: Duration = Duration::from_secs(45);
const ROUTER_DEBOUNCE: Duration = Duration::from_millis(650);
const CACHE_TTL: Duration = Duration::from_secs(60 * 15);
const DEFAULT_SENTENCES: u8 = 2;
const PROPOSAL_MIN_CONFIDENCE: f32 = 0.35;

const TOOL_MANIFEST: &str = r#"{
  "tools": [
    {
      "name": "wikipedia_city_lookup",
      "description": "Lookup a city on Wikipedia and return a short summary and URL.",
      "args_schema": {
        "type": "object",
        "properties": {
          "city": { "type": "string" },
          "lang": { "type": "string", "default": "en" },
          "sentences": { "type": "integer", "minimum": 1, "maximum": 10, "default": 2 }
        },
        "required": ["city"]
      }
    }
  ]
}"#;

#[derive(Debug, serde::Deserialize)]
struct SttStreamCommitPayload {
    text: String,
    #[allow(dead_code)]
    ts_ms: Option<i64>,
}

fn normalize_token(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn is_punct_only(s: &str) -> bool {
    normalize_token(s).is_empty()
}

fn extract_city_candidate(text: &str) -> Option<String> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    // Simple heuristic triggers for common location prepositions.
    // This is only a fallback when FunctionGemma isn't available or fails.
    const KEYWORDS: &[&str] = &["from", "in", "at", "to", "near"];
    const STOP_WORDS: &[&str] = &["the", "a", "an", "my", "your", "our", "their", "his", "her"];

    for (i, tok) in tokens.iter().enumerate() {
        let norm = normalize_token(tok);
        if KEYWORDS.iter().any(|k| *k == norm) {
            // Collect up to 4 tokens after "from", stopping at punctuation-only tokens.
            let mut parts: Vec<&str> = Vec::new();
            for t in tokens.iter().skip(i + 1).take(4) {
                if is_punct_only(t) {
                    break;
                }
                parts.push(*t);
            }
            if parts.is_empty() {
                return None;
            }
            let candidate = parts.join(" ").trim().to_string();
            let candidate_norm = normalize_token(&candidate);
            if candidate_norm.is_empty() || STOP_WORDS.iter().any(|w| *w == candidate_norm) {
                return None;
            }
            return Some(candidate);
        }
    }

    None
}

fn should_query_city(state: &mut ToolsState, city: &str) -> bool {
    let city_norm = city.trim().to_lowercase();
    if city_norm.is_empty() {
        return false;
    }

    if let Some((prev_city, prev_at)) = &state.router_last_city_query {
        if prev_city == &city_norm && prev_at.elapsed() < ROUTER_CITY_COOLDOWN {
            return false;
        }
    }

    state.router_last_city_query = Some((city_norm, Instant::now()));
    true
}

fn emit_router_status<R: Runtime>(
    app: &tauri::AppHandle<R>,
    phase: &str,
    payload: serde_json::Value,
) {
    let _ = app.emit(
        "tools:router_status",
        serde_json::json!({
            "phase": phase,
            "ts_ms": chrono::Utc::now().timestamp_millis(),
            "payload": payload,
        }),
    );
}

fn normalize_city_arg(v: &serde_json::Value) -> Option<String> {
    let s = v.as_str()?.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

async fn maybe_run_wikipedia_city<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &crate::SharedState,
    city: String,
    lang: String,
    evidence: String,
) {
    let (enabled, auto_run) = {
        let guard = state.lock().await;
        (guard.router_enabled, guard.router_auto_run_read_only)
    };

    if !enabled {
        return;
    }

    if !auto_run {
        emit_router_status(
            app,
            "proposal",
            serde_json::json!({
                "tool": "wikipedia_city_lookup",
                "args": { "city": city, "lang": lang, "sentences": DEFAULT_SENTENCES },
                "evidence": evidence,
            }),
        );
        let _ = app.emit(
            "tools:action_proposed",
            serde_json::json!({
                "tool": "wikipedia_city_lookup",
                "args": { "city": city, "lang": lang, "sentences": DEFAULT_SENTENCES },
                "evidence": evidence,
            }),
        );
        return;
    }

    let key = CacheKey::new(&lang, &city);

    // Cache hit?
    if let Some(hit) = {
        let mut guard = state.lock().await;
        if !should_query_city(&mut guard, &city) {
            return;
        }
        guard
            .cache
            .get(&key)
            .filter(|entry| entry.fetched_at.elapsed() <= CACHE_TTL)
            .map(|entry| entry.value.clone())
    } {
        emit_router_status(
            app,
            "tool_result",
            serde_json::json!({ "tool": "wikipedia_city_lookup", "city": city, "cached": true }),
        );
        let _ = app.emit(
            "tools:wikipedia_city",
            serde_json::json!({ "city": city, "result": hit }),
        );
        return;
    }

    emit_router_status(
        app,
        "tool_start",
        serde_json::json!({ "tool": "wikipedia_city_lookup", "city": city, "lang": lang }),
    );

    let fetched: Result<WikiSummaryDto, String> =
        crate::wikipedia::fetch_city_summary(&lang, &city, DEFAULT_SENTENCES)
            .await
            .map_err(|e| e.to_string());

    match fetched {
        Ok(result) => {
            {
                let mut guard = state.lock().await;
                guard.cache.insert(
                    key,
                    CacheEntry {
                        fetched_at: Instant::now(),
                        value: result.clone(),
                    },
                );
            }

            emit_router_status(
                app,
                "tool_result",
                serde_json::json!({ "tool": "wikipedia_city_lookup", "city": city, "cached": false }),
            );
            let _ = app.emit(
                "tools:wikipedia_city",
                serde_json::json!({ "city": city, "result": result }),
            );
        }
        Err(err) => {
            emit_router_status(
                app,
                "tool_error",
                serde_json::json!({ "tool": "wikipedia_city_lookup", "city": city, "error": err }),
            );
            let _ = app.emit(
                "tools:wikipedia_city_error",
                serde_json::json!({ "city": city, "error": err }),
            );
        }
    }
}

async fn process_router_queue<R: Runtime>(app: tauri::AppHandle<R>) {
    loop {
        tokio::time::sleep(ROUTER_DEBOUNCE).await;

        let state = app.state::<crate::SharedState>();

        let (pending_text, runner, enabled, default_lang) = {
            let mut guard = state.lock().await;

            let pending_text = guard.router_pending_text.trim().to_string();
            guard.router_pending_text.clear();

            let runner = guard.functiongemma.as_ref().map(|m| std::sync::Arc::clone(&m.runner));

            (pending_text, runner, guard.router_enabled, guard.router_default_lang.clone())
        };

        if !enabled {
            let mut guard = state.lock().await;
            guard.router_inflight = false;
            return;
        }

        if pending_text.is_empty() {
            let mut guard = state.lock().await;
            if guard.router_pending_text.trim().is_empty() {
                guard.router_inflight = false;
                emit_router_status(&app, "idle", serde_json::json!({}));
                return;
            }
            continue;
        }

        emit_router_status(
            &app,
            "queued",
            serde_json::json!({ "text": pending_text }),
        );

        // Prefer FunctionGemma if loaded; otherwise fallback to heuristic extraction.
        if let Some(runner) = runner {
            emit_router_status(&app, "infer_start", serde_json::json!({}));
            let pending_text_for_model = pending_text.clone();

            let result = tokio::task::spawn_blocking(move || {
                runner.infer_once(TOOL_MANIFEST, &pending_text_for_model)
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r.map_err(|e| e.to_string()));

            match result {
                Ok(model_out) => {
                    emit_router_status(
                        &app,
                        "infer_done",
                        serde_json::json!({ "raw": model_out.raw_text }),
                    );

                    if let Some(p) = model_out
                        .proposals
                        .iter()
                        .find(|p| p.confidence >= PROPOSAL_MIN_CONFIDENCE)
                    {
                        if p.tool == "wikipedia_city_lookup" {
                            let city = p
                                .args
                                .get("city")
                                .and_then(normalize_city_arg)
                                .or_else(|| extract_city_candidate(&pending_text));
                            if let Some(city) = city {
                                let lang = p
                                    .args
                                    .get("lang")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| default_lang.clone());

                                maybe_run_wikipedia_city(&app, &*state, city, lang, p.evidence.clone()).await;
                                continue;
                            }
                        }
                    }

                    // No valid proposal: do nothing.
                    continue;
                }
                Err(err) => {
                    emit_router_status(
                        &app,
                        "infer_error",
                        serde_json::json!({ "error": err }),
                    );
                    // fall through to heuristic
                }
            }
        }

        if let Some(city) = extract_city_candidate(&pending_text) {
            maybe_run_wikipedia_city(&app, &*state, city, default_lang.clone(), pending_text).await;
        }
    }
}

pub fn on_stt_stream_commit<R: Runtime>(app: &tauri::AppHandle<R>, payload_json: &str) {
    let Ok(payload) = serde_json::from_str::<SttStreamCommitPayload>(payload_json) else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<crate::SharedState>();

        emit_router_status(
            &app,
            "commit_received",
            serde_json::json!({ "text": payload.text }),
        );

        let should_spawn = {
            let mut guard = state.lock().await;
            if !guard.router_enabled {
                return;
            }
            if !guard.router_pending_text.is_empty() {
                guard.router_pending_text.push(' ');
            }
            guard.router_pending_text.push_str(payload.text.trim());

            if guard.router_inflight {
                false
            } else {
                guard.router_inflight = true;
                true
            }
        };

        if should_spawn {
            emit_router_status(&app, "worker_start", serde_json::json!({}));
            process_router_queue(app).await;
        }
    });
}

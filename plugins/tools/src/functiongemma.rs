use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::{DynTensor, DynTensorValueType, DynValue, Tensor};
use std::path::Path;
use std::sync::Mutex;
use tokenizers::Tokenizer;

#[derive(Debug, thiserror::Error)]
pub enum FunctionGemmaError {
    #[error("failed to load tokenizer: {0}")]
    Tokenizer(String),
    #[error("failed to load model: {0}")]
    Model(String),
    #[error("inference failed: {0}")]
    Inference(String),
    #[error("invalid model output")]
    InvalidOutput,
}

#[derive(Debug, Clone)]
pub struct Proposal {
    pub tool: String,
    pub args: serde_json::Value,
    pub confidence: f32,
    pub evidence: String,
}

#[derive(Debug, Clone)]
pub struct ModelOutput {
    pub raw_text: String,
    pub proposals: Vec<Proposal>,
}

#[derive(Debug)]
pub struct FunctionGemmaRunner {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
    input_names: Vec<String>,
    output_name: String,
    past_input_names: Vec<String>,
    present_output_names: Vec<String>,
    empty_past_specs: Vec<Option<EmptyPastSpec>>,
}

#[derive(Debug, Clone, Copy)]
enum PastElemType {
    F16,
    F32,
}

#[derive(Debug, Clone)]
struct EmptyPastSpec {
    elem: PastElemType,
    shape: Vec<i64>,
}

fn parse_shape_and_symbols_from_valuetype_debug(s: &str) -> (Option<Vec<i64>>, Option<Vec<String>>) {
    // Example:
    // Tensor { ty: Float16, shape: Shape { inner: [-1, 1, -1, 64] }, dimension_symbols: SymbolicDimensions(["batch", "kv_heads", "past_sequence_length", "head_dim"]) }
    let shape = s
        .split("shape: Shape { inner: [")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .map(|inner| {
            inner
                .split(',')
                .filter_map(|p| {
                    let p = p.trim();
                    if p.is_empty() {
                        None
                    } else {
                        p.parse::<i64>().ok()
                    }
                })
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty());

    let symbols = s
        .split("dimension_symbols: SymbolicDimensions([")
        .nth(1)
        .and_then(|rest| rest.split("])").next())
        .map(|inner| {
            // Parse quoted strings.
            let mut out = Vec::new();
            let mut cur = String::new();
            let mut in_str = false;
            for ch in inner.chars() {
                if ch == '"' {
                    if in_str {
                        out.push(cur.clone());
                        cur.clear();
                        in_str = false;
                    } else {
                        in_str = true;
                    }
                    continue;
                }
                if in_str {
                    cur.push(ch);
                }
            }
            out
        })
        .filter(|v| !v.is_empty());

    (shape, symbols)
}

fn guess_past_seq_dim(dims: &[i64], symbols: Option<&[String]>) -> usize {
    if let Some(symbols) = symbols {
        for (i, sym) in symbols.iter().enumerate() {
            let s = sym.to_lowercase();
            if s.contains("past") || s.contains("seq") || s.contains("sequence") {
                return i;
            }
        }
    }
    if dims.len() >= 3 {
        2
    } else {
        dims.len().saturating_sub(1)
    }
}

fn build_empty_past_spec(input_type_debug: &str) -> Option<EmptyPastSpec> {
    let elem = if input_type_debug.contains("ty: Float16") {
        PastElemType::F16
    } else if input_type_debug.contains("ty: Float32") {
        PastElemType::F32
    } else {
        // Unknown element type (e.g. BF16); bail out and use initializer.
        return None;
    };

    let (shape_opt, symbols_opt) = parse_shape_and_symbols_from_valuetype_debug(input_type_debug);
    let mut dims = shape_opt?;
    let seq_dim = guess_past_seq_dim(&dims, symbols_opt.as_deref());

    for d in dims.iter_mut() {
        if *d < 0 {
            *d = 1;
        }
    }
    if seq_dim < dims.len() {
        dims[seq_dim] = 0;
    }

    Some(EmptyPastSpec { elem, shape: dims })
}

impl FunctionGemmaRunner {
    pub fn load(model_path: impl AsRef<Path>, tokenizer_path: impl AsRef<Path>) -> Result<Self, FunctionGemmaError> {
        let tokenizer = Tokenizer::from_file(tokenizer_path.as_ref())
            .map_err(|e| FunctionGemmaError::Tokenizer(e.to_string()))?;

        let session = Session::builder()
            .map_err(|e| FunctionGemmaError::Model(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| FunctionGemmaError::Model(e.to_string()))?
            .commit_from_file(model_path.as_ref())
            .map_err(|e| FunctionGemmaError::Model(e.to_string()))?;

        let input_names = session.inputs.iter().map(|i| i.name.clone()).collect::<Vec<_>>();
        let output_names = session.outputs.iter().map(|o| o.name.clone()).collect::<Vec<_>>();
        let output_name = output_names
            .iter()
            .find(|n| n.as_str() == "logits")
            .cloned()
            .or_else(|| output_names.first().cloned())
            .ok_or_else(|| FunctionGemmaError::Model("model has no outputs".to_string()))?;

        let mut past_input_names = Vec::new();
        let mut present_output_names = Vec::new();
        let mut empty_past_specs: Vec<Option<EmptyPastSpec>> = Vec::new();
        for input in &session.inputs {
            if input.name.starts_with("past_key_values.") {
                past_input_names.push(input.name.clone());
                present_output_names.push(input.name.replacen("past_key_values", "present", 1));
                let dbg = format!("{:?}", input.input_type);
                let spec = build_empty_past_spec(&dbg);
                if spec.is_none() {
                    tracing::warn!(
                        past_input = %input.name,
                        input_type = %dbg,
                        "FunctionGemma: unable to infer past KV empty shape; inference may fail"
                    );
                }
                empty_past_specs.push(spec);
            }
        }

        tracing::info!(
            model_inputs = ?input_names,
            model_outputs = ?output_names,
            selected_output = %output_name,
            past_kv_inputs = past_input_names.len(),
            "FunctionGemma model loaded"
        );

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
            input_names,
            output_name,
            past_input_names,
            present_output_names,
            empty_past_specs,
        })
    }

    fn build_prompt(tool_manifest: &str, committed_text: &str) -> String {
        format!(
            r#"You are an assistant that decides whether to call a tool.
Return ONLY valid JSON. No extra text.

Tool manifest (JSON schema for args):
{tool_manifest}

Given this new transcript chunk, propose at most 1 tool call if helpful.
Transcript: "{committed_text}"

Output JSON format:
{{
  "proposals": [
    {{
      "tool": "tool_name",
      "args": {{}},
      "confidence": 0.0,
      "evidence": "short quote from transcript"
    }}
  ]
}}
"#
        )
    }

    fn extract_json(raw: &str) -> Option<&str> {
        let start = raw.find('{')?;
        let end = raw.rfind('}')?;
        if end <= start {
            return None;
        }
        Some(&raw[start..=end])
    }

    fn parse_output(raw_text: String) -> Result<ModelOutput, FunctionGemmaError> {
        let json_str = Self::extract_json(&raw_text).ok_or(FunctionGemmaError::InvalidOutput)?;
        let value: serde_json::Value =
            serde_json::from_str(json_str).map_err(|_| FunctionGemmaError::InvalidOutput)?;
        let proposals_val = value
            .get("proposals")
            .and_then(|p| p.as_array())
            .cloned()
            .unwrap_or_default();

        let mut proposals = Vec::new();
        for p in proposals_val {
            let tool = p.get("tool").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if tool.is_empty() {
                continue;
            }
            let args = p.get("args").cloned().unwrap_or_else(|| serde_json::json!({}));
            let confidence = p
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as f32;
            let evidence = p
                .get("evidence")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            proposals.push(Proposal {
                tool,
                args,
                confidence,
                evidence,
            });
        }

        Ok(ModelOutput { raw_text, proposals })
    }

    /// Greedy decode for a short JSON response.
    ///
    /// This is intentionally minimal; if the ONNX export supports KV cache, we can optimize later.
    pub fn infer_once(&self, tool_manifest: &str, committed_text: &str) -> Result<ModelOutput, FunctionGemmaError> {
        let prompt = Self::build_prompt(tool_manifest, committed_text);

        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| FunctionGemmaError::Tokenizer(e.to_string()))?;

        let mut input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let mut attention_mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&m| m as i64)
            .collect();

        // Cap prompt length for safety.
        const MAX_PROMPT_TOKENS: usize = 1024;
        if input_ids.len() > MAX_PROMPT_TOKENS {
            let start = input_ids.len() - MAX_PROMPT_TOKENS;
            input_ids = input_ids[start..].to_vec();
            attention_mask = attention_mask[start..].to_vec();
        }

        let mut generated: Vec<u32> = Vec::new();
        let mut decoded = String::new();

        // Small generation budget; we just want JSON with 0..1 proposals.
        const MAX_NEW_TOKENS: usize = 160;

        // If the ONNX export exposes KV cache inputs, use them. This avoids shape mismatches with models that require past inputs.
        let use_kv_cache = !self.past_input_names.is_empty()
            && self.past_input_names.len() == self.present_output_names.len()
            && self.empty_past_specs.len() == self.past_input_names.len()
            && self.empty_past_specs.iter().all(|s| s.is_some());

        // Initial past: empty tensors with correct head/head_dim, past_seq_len=0.
        let mut past: Vec<DynTensor> = Vec::new();
        if use_kv_cache {
            for spec in self.empty_past_specs.iter().filter_map(|s| s.as_ref()) {
                let t: DynTensor = match spec.elem {
                    PastElemType::F32 => {
                        let numel = spec
                            .shape
                            .iter()
                            .copied()
                            .fold(1i64, |acc, d| acc.saturating_mul(d.max(0)))
                            as usize;
                        let data = vec![0f32; numel];
                        Tensor::<f32>::from_array((spec.shape.clone(), data))
                            .map_err(|e| FunctionGemmaError::Inference(e.to_string()))?
                            .upcast()
                    }
                    PastElemType::F16 => {
                        let numel = spec
                            .shape
                            .iter()
                            .copied()
                            .fold(1i64, |acc, d| acc.saturating_mul(d.max(0)))
                            as usize;
                        let data = vec![half::f16::from_f32(0.0); numel];
                        Tensor::<half::f16>::from_array((spec.shape.clone(), data))
                            .map_err(|e| FunctionGemmaError::Inference(e.to_string()))?
                            .upcast()
                    }
                };
                past.push(t);
            }
        }

        let mut total_len = input_ids.len();

        for step in 0..MAX_NEW_TOKENS {
            let is_first = step == 0 || !use_kv_cache;

            let (step_ids, step_positions, step_mask_len) = if is_first {
                // First pass: feed the full prompt.
                (
                    input_ids.clone(),
                    (0..total_len as i64).collect::<Vec<i64>>(),
                    total_len,
                )
            } else {
                // Subsequent passes: feed only the last generated token, use cached past.
                let last = *input_ids.last().ok_or(FunctionGemmaError::InvalidOutput)?;
                (vec![last], vec![(total_len as i64) - 1], total_len)
            };

            let ids_tensor = Tensor::<i64>::from_array(([1usize, step_ids.len()], step_ids))
                .map_err(|e| FunctionGemmaError::Inference(e.to_string()))?;
            let mask = vec![1i64; step_mask_len];
            let mask_tensor = Tensor::<i64>::from_array(([1usize, mask.len()], mask))
                .map_err(|e| FunctionGemmaError::Inference(e.to_string()))?;
            let pos_tensor = Tensor::<i64>::from_array(([1usize, step_positions.len()], step_positions))
                .map_err(|e| FunctionGemmaError::Inference(e.to_string()))?;

            let mut inputs: Vec<(String, DynTensor)> = Vec::new();
            inputs.push(("input_ids".to_string(), ids_tensor.upcast()));
            if self.input_names.iter().any(|n| n == "attention_mask") {
                inputs.push(("attention_mask".to_string(), mask_tensor.upcast()));
            }
            if self.input_names.iter().any(|n| n == "position_ids") {
                inputs.push(("position_ids".to_string(), pos_tensor.upcast()));
            }

            if use_kv_cache {
                // Move current past tensors into inputs, then replace with present outputs.
                let past_to_use = std::mem::take(&mut past);
                for (name, tensor) in self
                    .past_input_names
                    .iter()
                    .cloned()
                    .zip(past_to_use.into_iter())
                {
                    inputs.push((name, tensor));
                }
            }

            let mut session = self
                .session
                .lock()
                .map_err(|_| FunctionGemmaError::Inference("model session lock poisoned".to_string()))?;

            let mut outputs = session
                .run(inputs)
                .map_err(|e| FunctionGemmaError::Inference(e.to_string()))?;

            let logits = outputs[self.output_name.as_str()]
                .try_extract_array::<f32>()
                .map_err(|e| FunctionGemmaError::Inference(e.to_string()))?;

            // Extract last logits.
            let shape = logits.shape().to_vec();
            if shape.len() < 2 {
                return Err(FunctionGemmaError::InvalidOutput);
            }
            let vocab = *shape.last().unwrap();
            let last_logits: Vec<f32> = match shape.len() {
                3 => {
                    let seq = shape[1];
                    let base = (seq - 1) * vocab;
                    logits
                        .as_slice()
                        .ok_or(FunctionGemmaError::InvalidOutput)?[base..base + vocab]
                        .to_vec()
                }
                2 => {
                    let seq = shape[0];
                    let base = (seq - 1) * vocab;
                    logits
                        .as_slice()
                        .ok_or(FunctionGemmaError::InvalidOutput)?[base..base + vocab]
                        .to_vec()
                }
                _ => {
                    let flat = logits.as_slice().ok_or(FunctionGemmaError::InvalidOutput)?;
                    if flat.len() < vocab {
                        return Err(FunctionGemmaError::InvalidOutput);
                    }
                    flat[flat.len() - vocab..].to_vec()
                }
            };

            // Update past from present outputs (only when using cache).
            if use_kv_cache {
                let mut next_past = Vec::with_capacity(self.present_output_names.len());
                for present_name in &self.present_output_names {
                    let v: DynValue = outputs
                        .remove(present_name.as_str())
                        .ok_or(FunctionGemmaError::InvalidOutput)?;
                    let t: DynTensor = v
                        .downcast::<DynTensorValueType>()
                        .map_err(|e| FunctionGemmaError::Inference(e.to_string()))?;
                    next_past.push(t);
                }
                past = next_past;
            }

            // Greedy argmax
            let mut best_id: u32 = 0;
            let mut best_val: f32 = f32::NEG_INFINITY;
            for (i, v) in last_logits.iter().enumerate() {
                if *v > best_val {
                    best_val = *v;
                    best_id = i as u32;
                }
            }

            generated.push(best_id);
            input_ids.push(best_id as i64);
            attention_mask.push(1);
            total_len += 1;

            decoded = self
                .tokenizer
                .decode(&generated, true)
                .map_err(|e| FunctionGemmaError::Tokenizer(e.to_string()))?;

            if let Some(candidate) = Self::extract_json(&decoded) {
                if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
                    break;
                }
            }
        }

        Self::parse_output(decoded)
    }
}

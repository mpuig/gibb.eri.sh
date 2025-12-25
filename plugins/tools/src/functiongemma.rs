use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::ValueType;
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
    banned_token_ids: Vec<u32>,
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

fn guess_past_seq_dim(dims: &[i64], symbols: Option<&[String]>) -> usize {
    if let Some(symbols) = symbols {
        for (i, sym) in symbols.iter().enumerate() {
            let s = sym.to_lowercase();
            if s.contains("past") || s.contains("seq") || s.contains("sequence") {
                return i;
            }
        }
    }
    // Heuristic: sequence/past length is usually the last dynamic dimension (-1).
    dims.iter()
        .enumerate()
        .rev()
        .find(|(_, d)| **d < 0)
        .map(|(i, _)| i)
        .unwrap_or_else(|| dims.len().saturating_sub(1))
}

fn build_empty_past_spec(value_type: &ValueType) -> Option<EmptyPastSpec> {
    let ValueType::Tensor {
        ty,
        shape,
        dimension_symbols,
    } = value_type
    else {
        return None;
    };

    let elem = match ty {
        ort::tensor::TensorElementType::Float16 => PastElemType::F16,
        ort::tensor::TensorElementType::Float32 => PastElemType::F32,
        _ => return None,
    };

    let mut dims: Vec<i64> = shape.iter().copied().collect();
    let symbols: &[String] = dimension_symbols.as_ref();
    let symbols_opt: Option<&[String]> = if symbols.is_empty() {
        None
    } else {
        Some(symbols)
    };
    let seq_dim = guess_past_seq_dim(&dims, symbols_opt);

    for d in dims.iter_mut() {
        if *d < 0 {
            *d = 1;
        }
    }
    if seq_dim < dims.len() {
        // `ort::value::Tensor::from_array` rejects zero-sized dimensions. Many KV-cache ONNX exports
        // accept a "past_seq_len = 0" tensor, but we can't construct one via `from_array` here.
        // Instead, we prime the cache with a single masked timestep; callers must ensure the
        // corresponding attention mask position is 0 so it is never attended to.
        dims[seq_dim] = 1;
    }

    Some(EmptyPastSpec { elem, shape: dims })
}

impl FunctionGemmaRunner {
    pub fn load(
        model_path: impl AsRef<Path>,
        tokenizer_path: impl AsRef<Path>,
    ) -> Result<Self, FunctionGemmaError> {
        let tokenizer = Tokenizer::from_file(tokenizer_path.as_ref())
            .map_err(|e| FunctionGemmaError::Tokenizer(e.to_string()))?;

        let session = Session::builder()
            .map_err(|e| FunctionGemmaError::Model(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| FunctionGemmaError::Model(e.to_string()))?
            .commit_from_file(model_path.as_ref())
            .map_err(|e| FunctionGemmaError::Model(e.to_string()))?;

        let input_names = session
            .inputs
            .iter()
            .map(|i| i.name.clone())
            .collect::<Vec<_>>();
        let output_names = session
            .outputs
            .iter()
            .map(|o| o.name.clone())
            .collect::<Vec<_>>();
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
                let spec = build_empty_past_spec(&input.input_type);
                if spec.is_none() {
                    tracing::warn!(
                        past_input = %input.name,
                        input_type = ?input.input_type,
                        "FunctionGemma: unable to infer past KV empty shape; inference may fail"
                    );
                } else if empty_past_specs.len() < 4 {
                    tracing::debug!(
                        past_input = %input.name,
                        inferred_empty_shape = ?spec.as_ref().unwrap().shape,
                        input_type = ?input.input_type,
                        "FunctionGemma: inferred empty past KV"
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

        // We want the model to emit `<start_function_call>...<end_function_call>`, but discourage it
        // from generating new turns or tool definitions/results.
        let banned_tokens = [
            "<start_function_declaration>",
            "<end_function_declaration>",
            "<start_function_response>",
            "<end_function_response>",
            "<start_of_turn>",
        ];
        let mut banned_token_ids = Vec::new();
        for t in banned_tokens {
            if let Some(id) = tokenizer.token_to_id(t) {
                banned_token_ids.push(id);
            }
        }

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
            input_names,
            output_name,
            past_input_names,
            present_output_names,
            empty_past_specs,
            banned_token_ids,
        })
    }

    fn build_prompt(developer_context: &str, committed_text: &str) -> String {
        // Follow the official FunctionGemma formatting:
        // - a developer turn that contains the trigger phrase + function declarations (+ optional policy text)
        // - a user turn with the request
        // - the model emits `<start_function_call>call:...{...}<end_function_call>` when appropriate
        format!(
            "<start_of_turn>developer\n\
{developer_context}<end_of_turn>\n\
<start_of_turn>user\n\
{committed_text}<end_of_turn>\n\
<start_of_turn>model\n"
        )
    }

    fn build_args_prompt(developer_context: &str, tool: &str, committed_text: &str) -> String {
        format!(
            "<start_of_turn>developer\n\
{developer_context}<end_of_turn>\n\
<start_of_turn>user\n\
Call the function {tool} with the correct arguments for this text:\n\
{committed_text}<end_of_turn>\n\
<start_of_turn>model\n"
        )
    }

    fn build_repair_prompt(
        developer_context: &str,
        committed_text: &str,
        bad_output: &str,
    ) -> String {
        format!(
            "<start_of_turn>developer\n\
{developer_context}<end_of_turn>\n\
<start_of_turn>user\n\
The previous model output was invalid.\n\
\n\
Output ONLY valid function call(s) using EXACTLY this format:\n\
<start_function_call>call:TOOL_NAME{{arg1:<escape>value<escape>,arg2:...}}<end_function_call>\n\
\n\
Text:\n\
{committed_text}\n\
\n\
Invalid output:\n\
{bad_output}<end_of_turn>\n\
<start_of_turn>model\n"
        )
    }

    fn parse_output(raw_text: String, evidence: &str) -> ModelOutput {
        let mut proposals = Vec::new();
        for block in crate::parser::find_function_call_blocks(&raw_text) {
            if let Ok((tool, args)) = crate::parser::parse_functiongemma_call(block) {
                proposals.push(Proposal {
                    tool,
                    args,
                    confidence: 1.0,
                    evidence: evidence.to_string(),
                });
            }
        }
        ModelOutput {
            raw_text,
            proposals,
        }
    }

    fn generate_text(
        &self,
        prompt: &str,
        max_new_tokens: usize,
    ) -> Result<String, FunctionGemmaError> {
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

        // If the ONNX export exposes KV cache inputs, use them.
        let use_kv_cache = !self.past_input_names.is_empty()
            && self.past_input_names.len() == self.present_output_names.len()
            && self.empty_past_specs.len() == self.past_input_names.len()
            && self.empty_past_specs.iter().all(|s| s.is_some());
        tracing::debug!(use_kv_cache, "FunctionGemma generate_text");
        let past_prefix_len: usize = if use_kv_cache { 1 } else { 0 };
        let position_offset: i64 = past_prefix_len as i64;

        // Initial past: primed tensors with correct head/head_dim and a single masked timestep.
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

        for step in 0..max_new_tokens {
            let is_first = step == 0 || !use_kv_cache;

            let (step_ids, step_positions) = if is_first {
                (
                    input_ids.clone(),
                    (position_offset..position_offset + total_len as i64).collect::<Vec<i64>>(),
                )
            } else {
                let last = *input_ids.last().ok_or(FunctionGemmaError::InvalidOutput)?;
                (vec![last], vec![position_offset + (total_len as i64) - 1])
            };
            let step_mask_len = total_len + past_prefix_len;

            let ids_tensor = Tensor::<i64>::from_array(([1usize, step_ids.len()], step_ids))
                .map_err(|e| FunctionGemmaError::Inference(e.to_string()))?;
            let mask: Vec<i64> = if past_prefix_len == 0 {
                attention_mask.clone()
            } else {
                let mut m = Vec::with_capacity(step_mask_len);
                m.extend(std::iter::repeat_n(0i64, past_prefix_len));
                m.extend(attention_mask.iter().copied());
                m
            };
            let mask_tensor = Tensor::<i64>::from_array(([1usize, mask.len()], mask))
                .map_err(|e| FunctionGemmaError::Inference(e.to_string()))?;
            let pos_tensor =
                Tensor::<i64>::from_array(([1usize, step_positions.len()], step_positions))
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

            let mut session = self.session.lock().map_err(|_| {
                FunctionGemmaError::Inference("model session lock poisoned".to_string())
            })?;

            let mut outputs = session
                .run(inputs)
                .map_err(|e| FunctionGemmaError::Inference(e.to_string()))?;

            // Take logits out of the map so we can mutably remove KV outputs without borrow conflicts.
            let logits_value: DynValue = outputs
                .remove(self.output_name.as_str())
                .ok_or(FunctionGemmaError::InvalidOutput)?;
            let logits = logits_value
                .try_extract_array::<f32>()
                .map_err(|e| FunctionGemmaError::Inference(e.to_string()))?;

            let shape = logits.shape().to_vec();
            if shape.len() < 2 {
                return Err(FunctionGemmaError::InvalidOutput);
            }
            let vocab = *shape.last().unwrap();
            let mut last_logits: Vec<f32> = match shape.len() {
                3 => {
                    let seq = shape[1];
                    let base = (seq - 1) * vocab;
                    logits.as_slice().ok_or(FunctionGemmaError::InvalidOutput)?[base..base + vocab]
                        .to_vec()
                }
                2 => {
                    let seq = shape[0];
                    let base = (seq - 1) * vocab;
                    logits.as_slice().ok_or(FunctionGemmaError::InvalidOutput)?[base..base + vocab]
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
            // Prevent the model from escaping into special-token loops; we want plain JSON output.
            for id in &self.banned_token_ids {
                if let Some(v) = last_logits.get_mut(*id as usize) {
                    *v = f32::NEG_INFINITY;
                }
            }
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
                .decode(&generated, false)
                .map_err(|e| FunctionGemmaError::Tokenizer(e.to_string()))?;

            let saw_start_call = decoded.contains("<start_function_call>");
            if decoded.contains("<end_function_call>") {
                break;
            }
            // The model sometimes emits `<end_of_turn>` prematurely. If it has already started a
            // function-call block, keep generating until we see `<end_function_call>` (or hit the
            // token budget) so parsing has a chance to succeed.
            if decoded.contains("<end_of_turn>") && !saw_start_call {
                break;
            }
        }

        Ok(decoded)
    }

    /// Greedy decode for a short JSON response.
    ///
    /// This is intentionally minimal; if the ONNX export supports KV cache, we can optimize later.
    pub fn infer_once(
        &self,
        developer_context: &str,
        committed_text: &str,
    ) -> Result<ModelOutput, FunctionGemmaError> {
        const MAX_NEW_TOKENS: usize = 160;
        let prompt = Self::build_prompt(developer_context, committed_text);
        let decoded = self.generate_text(&prompt, MAX_NEW_TOKENS)?;

        let out = Self::parse_output(decoded.clone(), committed_text);
        if !out.proposals.is_empty() {
            return Ok(out);
        }

        // Retry once with a stricter repair prompt (still FunctionGemma-native).
        let repair_prompt = Self::build_repair_prompt(developer_context, committed_text, &decoded);
        let repaired = self.generate_text(&repair_prompt, 200)?;
        let mut repaired_out = Self::parse_output(repaired.clone(), committed_text);
        if repaired_out.proposals.is_empty() {
            repaired_out.raw_text = format!(
                "{}\n\n<repair>\n{}",
                decoded.lines().take(80).collect::<Vec<_>>().join("\n"),
                repaired.lines().take(80).collect::<Vec<_>>().join("\n")
            );
        }
        Ok(repaired_out)
    }

    pub fn infer_args_object(
        &self,
        developer_context: &str,
        tool: &str,
        committed_text: &str,
    ) -> Result<serde_json::Value, FunctionGemmaError> {
        let prompt = Self::build_args_prompt(developer_context, tool, committed_text);
        let decoded = self.generate_text(&prompt, 96)?;
        if let Some(call) = crate::parser::extract_function_call_json_tagged(&decoded) {
            let (_, args) = crate::parser::parse_functiongemma_call(call)?;
            return Ok(args);
        }
        Err(FunctionGemmaError::InvalidOutput)
    }
}

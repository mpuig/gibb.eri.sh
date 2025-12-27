//! Pure prompt building for FunctionGemma - no IO, fully testable.
//!
//! Separates prompt construction from inference for testability.

/// Build a standard function-calling prompt.
///
/// Follows FunctionGemma formatting:
/// - Developer turn with instructions + function declarations
/// - User turn with the request
/// - Model prefix for generation
pub fn build_prompt(developer_context: &str, committed_text: &str) -> String {
    format!(
        "<start_of_turn>developer\n\
{developer_context}<end_of_turn>\n\
<start_of_turn>user\n\
{committed_text}<end_of_turn>\n\
<start_of_turn>model\n"
    )
}

/// Build a prompt specifically for argument extraction.
///
/// Used when the tool is known but args need to be inferred.
pub fn build_args_prompt(developer_context: &str, tool: &str, committed_text: &str) -> String {
    format!(
        "<start_of_turn>developer\n\
{developer_context}<end_of_turn>\n\
<start_of_turn>user\n\
Call the function {tool} with the correct arguments for this text:\n\
{committed_text}<end_of_turn>\n\
<start_of_turn>model\n"
    )
}

/// Build a repair prompt after failed parsing.
///
/// Provides the model with context about the invalid output
/// and explicitly guides toward valid function call format.
pub fn build_repair_prompt(
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

/// Build a summarization prompt for tool output.
///
/// Used in the feedback loop (Phase 3) to generate human-friendly
/// summaries of tool execution results.
pub fn build_summary_prompt(tool_name: &str, output_preview: &str, user_request: &str) -> String {
    format!(
        "<start_of_turn>user\n\
The user asked: \"{}\"\n\
The {} tool returned:\n{}\n\n\
Respond with a brief, natural language summary (1-2 sentences) of the result. \
Do not mention the tool name. Just describe what you found.\n\
<end_of_turn>\n\
<start_of_turn>model\n",
        user_request, tool_name, output_preview
    )
}

/// Truncate text to a maximum length with ellipsis.
pub fn truncate_preview(text: &str, max_len: usize) -> String {
    if text.len() > max_len {
        format!("{}...", &text[..max_len])
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_format() {
        let prompt = build_prompt("You are an action router.", "open Safari");
        assert!(prompt.contains("<start_of_turn>developer"));
        assert!(prompt.contains("You are an action router."));
        assert!(prompt.contains("<start_of_turn>user"));
        assert!(prompt.contains("open Safari"));
        assert!(prompt.ends_with("<start_of_turn>model\n"));
    }

    #[test]
    fn test_build_args_prompt_includes_tool() {
        let prompt = build_args_prompt("context", "app_launcher", "open chrome");
        assert!(prompt.contains("app_launcher"));
        assert!(prompt.contains("Call the function app_launcher"));
    }

    #[test]
    fn test_build_repair_prompt_includes_bad_output() {
        let prompt = build_repair_prompt("context", "open Safari", "invalid json garbage");
        assert!(prompt.contains("invalid json garbage"));
        assert!(prompt.contains("Invalid output:"));
        assert!(prompt.contains("<start_function_call>"));
    }

    #[test]
    fn test_build_summary_prompt() {
        let prompt = build_summary_prompt("web_search", "{\"results\": []}", "what is rust");
        assert!(prompt.contains("web_search"));
        assert!(prompt.contains("what is rust"));
        assert!(prompt.contains("brief, natural language summary"));
    }

    #[test]
    fn test_truncate_preview_short() {
        let text = "short text";
        assert_eq!(truncate_preview(text, 100), "short text");
    }

    #[test]
    fn test_truncate_preview_long() {
        let text = "this is a longer text that should be truncated";
        let truncated = truncate_preview(text, 20);
        assert!(truncated.ends_with("..."));
        assert_eq!(truncated.len(), 23); // 20 + "..."
    }
}

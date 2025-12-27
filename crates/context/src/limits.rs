//! Context injection limits and sanitization policy.
//!
//! Centralizes all limits and redaction rules for prompt context injection.
//! This is the single source of truth for context hygiene.

/// Maximum length for clipboard content in prompts (chars).
pub const MAX_CLIPBOARD_LEN: usize = 200;

/// Maximum length for selection content in prompts (chars).
pub const MAX_SELECTION_LEN: usize = 200;

/// Maximum length for URL in prompts (chars).
pub const MAX_URL_LEN: usize = 200;

/// Maximum total context snippet length (chars).
pub const MAX_CONTEXT_SNIPPET_LEN: usize = 1024;

/// Patterns that should be redacted from context (potential secrets).
/// These are case-insensitive prefix patterns.
pub const REDACTION_PATTERNS: &[&str] = &[
    "password",
    "secret",
    "api_key",
    "apikey",
    "api-key",
    "token",
    "bearer",
    "authorization",
    "credential",
    "private_key",
    "privatekey",
    "access_key",
    "accesskey",
];

/// Context precedence order (higher = more important, shown first).
/// This documents the order in which context elements appear in prompts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContextPrecedence {
    /// Mode is highest priority - always shown first.
    Mode = 100,
    /// Active app context.
    ActiveApp = 90,
    /// Meeting status.
    MeetingStatus = 80,
    /// Selected text (user's current focus).
    Selection = 70,
    /// Clipboard content.
    Clipboard = 60,
    /// Browser URL.
    Url = 50,
    /// Date/time (lowest priority context).
    DateTime = 10,
}

/// Check if content contains potential secrets that should be redacted.
pub fn contains_sensitive_pattern(content: &str) -> bool {
    let lower = content.to_lowercase();
    REDACTION_PATTERNS.iter().any(|p| lower.contains(p))
}

/// Redact content if it contains sensitive patterns.
/// Returns "[REDACTED - may contain sensitive data]" if sensitive.
pub fn redact_if_sensitive(content: &str) -> Option<String> {
    if contains_sensitive_pattern(content) {
        None // Don't include at all
    } else {
        Some(content.to_string())
    }
}

/// Sanitize and limit content for prompt injection.
///
/// Applies:
/// 1. Length truncation
/// 2. Angle bracket escaping (prevents XML injection)
/// 3. Whitespace normalization
/// 4. Sensitive content redaction
pub fn sanitize_for_prompt(content: &str, max_len: usize) -> Option<String> {
    // Check for sensitive content first
    if contains_sensitive_pattern(content) {
        return None;
    }

    // Truncate first to avoid processing huge strings
    let truncated = if content.len() > max_len {
        format!("{}...", &content[..max_len])
    } else {
        content.to_string()
    };

    let sanitized = truncated
        // Escape angle brackets to prevent XML/marker injection
        .replace('<', "‹")
        .replace('>', "›")
        // Normalize whitespace (newlines, tabs -> space)
        .replace('\n', " ")
        .replace('\r', " ")
        .replace('\t', " ")
        // Collapse multiple spaces
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redaction_detects_password() {
        assert!(contains_sensitive_pattern("my password is 123"));
        assert!(contains_sensitive_pattern("PASSWORD=secret"));
        assert!(contains_sensitive_pattern("api_key: xyz"));
    }

    #[test]
    fn test_redaction_allows_normal_content() {
        assert!(!contains_sensitive_pattern("hello world"));
        assert!(!contains_sensitive_pattern("search for cats"));
    }

    #[test]
    fn test_sanitize_redacts_sensitive() {
        assert_eq!(sanitize_for_prompt("password=123", 200), None);
        assert_eq!(sanitize_for_prompt("api_key: xyz", 200), None);
    }

    #[test]
    fn test_sanitize_truncates() {
        let long = "a".repeat(300);
        let result = sanitize_for_prompt(&long, 200).unwrap();
        assert!(result.len() <= 203); // 200 + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_sanitize_escapes_brackets() {
        let result = sanitize_for_prompt("<script>alert</script>", 200).unwrap();
        assert!(!result.contains('<'));
        assert!(!result.contains('>'));
        assert!(result.contains('‹'));
        assert!(result.contains('›'));
    }

    #[test]
    fn test_sanitize_normalizes_whitespace() {
        let result = sanitize_for_prompt("line1\nline2\tline3", 200).unwrap();
        assert_eq!(result, "line1 line2 line3");
    }

    #[test]
    fn test_precedence_ordering() {
        assert!(ContextPrecedence::Mode > ContextPrecedence::Selection);
        assert!(ContextPrecedence::Selection > ContextPrecedence::Clipboard);
        assert!(ContextPrecedence::Clipboard > ContextPrecedence::Url);
    }

    #[test]
    fn test_constants_are_reasonable() {
        assert!(MAX_CLIPBOARD_LEN > 50);
        assert!(MAX_CLIPBOARD_LEN < 1000);
        assert!(MAX_CONTEXT_SNIPPET_LEN > MAX_CLIPBOARD_LEN);
    }
}

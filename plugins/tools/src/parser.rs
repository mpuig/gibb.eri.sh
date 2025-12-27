//! FunctionGemma output parsing.
//!
//! Parses the custom FunctionGemma output format:
//! `<start_function_call>call:TOOL{arg:<escape>value<escape>}<end_function_call>`

use crate::functiongemma::FunctionGemmaError;

const START_TAG: &str = "<start_function_call>";
const END_TAG: &str = "<end_function_call>";
const ESCAPE_TAG: &str = "<escape>";

/// Cursor for parsing FunctionGemma output format.
#[derive(Debug)]
pub struct Cursor<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(s: &'a str) -> Self {
        Self { s, i: 0 }
    }

    pub fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }

    pub fn eat_ws(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.i += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    pub fn starts_with(&self, lit: &str) -> bool {
        self.s[self.i..].starts_with(lit)
    }

    pub fn consume(&mut self, lit: &str) -> bool {
        if self.starts_with(lit) {
            self.i += lit.len();
            true
        } else {
            false
        }
    }

    pub fn expect_char(&mut self, expected: char) -> Result<(), FunctionGemmaError> {
        self.eat_ws();
        if self.peek() == Some(expected) {
            self.i += expected.len_utf8();
            Ok(())
        } else {
            Err(FunctionGemmaError::InvalidOutput)
        }
    }

    pub fn take_key(&mut self) -> Result<String, FunctionGemmaError> {
        self.eat_ws();
        let start = self.i;
        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' {
                self.i += ch.len_utf8();
                continue;
            }
            break;
        }
        if self.i == start {
            return Err(FunctionGemmaError::InvalidOutput);
        }
        Ok(self.s[start..self.i].to_string())
    }

    pub fn take_until_top_level_delim(&mut self) -> String {
        let start = self.i;
        while let Some(ch) = self.peek() {
            if ch == ',' || ch == '}' || ch == ']' {
                break;
            }
            self.i += ch.len_utf8();
        }
        self.s[start..self.i].to_string()
    }

    pub fn parse_string_escape(&mut self) -> Result<String, FunctionGemmaError> {
        if !self.consume(ESCAPE_TAG) {
            return Err(FunctionGemmaError::InvalidOutput);
        }
        let rest = &self.s[self.i..];
        let Some(end) = rest.find(ESCAPE_TAG) else {
            return Err(FunctionGemmaError::InvalidOutput);
        };
        let val = rest[..end].to_string();
        self.i += end + ESCAPE_TAG.len();
        Ok(val)
    }

    pub fn parse_value(&mut self) -> Result<serde_json::Value, FunctionGemmaError> {
        self.eat_ws();
        if self.starts_with(ESCAPE_TAG) {
            let s = self.parse_string_escape()?;
            return Ok(serde_json::Value::String(s));
        }
        match self.peek() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            _ => {
                let tok = self.take_until_top_level_delim();
                Ok(parse_bare_value(&tok))
            }
        }
    }

    pub fn parse_object(&mut self) -> Result<serde_json::Value, FunctionGemmaError> {
        self.expect_char('{')?;
        let mut map = serde_json::Map::new();
        loop {
            self.eat_ws();
            if self.consume("}") {
                break;
            }
            let key = self.take_key()?;
            self.expect_char(':')?;
            let val = self.parse_value()?;
            map.insert(key, val);
            self.eat_ws();
            if self.consume(",") {
                continue;
            }
            if self.consume("}") {
                break;
            }
            return Err(FunctionGemmaError::InvalidOutput);
        }
        Ok(serde_json::Value::Object(map))
    }

    pub fn parse_array(&mut self) -> Result<serde_json::Value, FunctionGemmaError> {
        self.expect_char('[')?;
        let mut items = Vec::new();
        loop {
            self.eat_ws();
            if self.consume("]") {
                break;
            }
            let v = self.parse_value()?;
            items.push(v);
            self.eat_ws();
            if self.consume(",") {
                continue;
            }
            if self.consume("]") {
                break;
            }
            return Err(FunctionGemmaError::InvalidOutput);
        }
        Ok(serde_json::Value::Array(items))
    }
}

/// Parse a bare value (not wrapped in escape tags).
pub fn parse_bare_value(token: &str) -> serde_json::Value {
    let t = token.trim();
    if t.eq_ignore_ascii_case("true") {
        return serde_json::Value::Bool(true);
    }
    if t.eq_ignore_ascii_case("false") {
        return serde_json::Value::Bool(false);
    }
    if let Ok(n) = t.parse::<i64>() {
        return serde_json::Value::Number(n.into());
    }
    if let Ok(n) = t.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(n) {
            return serde_json::Value::Number(n);
        }
    }
    serde_json::Value::String(t.to_string())
}

/// Parse a FunctionGemma object format (custom, not JSON).
pub fn parse_functiongemma_object(input: &str) -> Result<serde_json::Value, FunctionGemmaError> {
    let mut c = Cursor::new(input);
    c.parse_object()
}

/// Find all function call blocks in raw model output.
pub fn find_function_call_blocks(raw: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = raw;
    while let Some(start) = rest.find(START_TAG) {
        let after_start = &rest[start + START_TAG.len()..];
        let Some(end) = after_start.find(END_TAG) else {
            break;
        };
        let inner = after_start[..end].trim();
        if !inner.is_empty() {
            out.push(inner);
        }
        rest = &after_start[end + END_TAG.len()..];
    }
    out
}

/// Extract the first function call block content (for tagged JSON extraction).
pub fn extract_function_call_json_tagged(raw: &str) -> Option<&str> {
    let start = raw.find(START_TAG)? + START_TAG.len();
    let end = raw[start..].find(END_TAG)? + start;
    let inner = raw[start..end].trim();
    if inner.is_empty() {
        None
    } else {
        Some(inner)
    }
}

/// Parse a single function call: `call:TOOL{args}`.
pub fn parse_functiongemma_call(
    inner: &str,
) -> Result<(String, serde_json::Value), FunctionGemmaError> {
    let inner = inner.trim();
    let Some(rest) = inner.strip_prefix("call:") else {
        return Err(FunctionGemmaError::InvalidOutput);
    };
    let rest = rest.trim_start();
    let Some((name, args_tail)) = rest.split_once('{') else {
        return Err(FunctionGemmaError::InvalidOutput);
    };
    let tool = name.trim().to_string();
    if tool.is_empty() {
        return Err(FunctionGemmaError::InvalidOutput);
    }
    let args_with_brace = format!("{{{args_tail}");
    let args_value = parse_functiongemma_object(&args_with_brace)?;
    Ok((tool, args_value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bare_value_bool() {
        assert_eq!(parse_bare_value("true"), serde_json::Value::Bool(true));
        assert_eq!(parse_bare_value("TRUE"), serde_json::Value::Bool(true));
        assert_eq!(parse_bare_value("false"), serde_json::Value::Bool(false));
        assert_eq!(parse_bare_value("FALSE"), serde_json::Value::Bool(false));
    }

    #[test]
    fn test_parse_bare_value_number() {
        assert_eq!(parse_bare_value("42"), serde_json::json!(42));
        assert_eq!(parse_bare_value("-10"), serde_json::json!(-10));
        assert_eq!(parse_bare_value("2.5"), serde_json::json!(2.5));
    }

    #[test]
    fn test_parse_bare_value_string() {
        assert_eq!(parse_bare_value("hello"), serde_json::json!("hello"));
        assert_eq!(parse_bare_value("  spaced  "), serde_json::json!("spaced"));
    }

    #[test]
    fn test_cursor_parse_escape() {
        let mut c = Cursor::new("<escape>Madrid<escape>");
        let result = c.parse_string_escape().unwrap();
        assert_eq!(result, "Madrid");
    }

    #[test]
    fn test_cursor_parse_escape_with_special_chars() {
        let mut c = Cursor::new("<escape>Hello, World!<escape>");
        let result = c.parse_string_escape().unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_cursor_parse_escape_missing_end() {
        let mut c = Cursor::new("<escape>unclosed");
        assert!(c.parse_string_escape().is_err());
    }

    #[test]
    fn test_parse_functiongemma_object_simple() {
        let obj = parse_functiongemma_object("{city:<escape>Madrid<escape>}").unwrap();
        assert_eq!(obj["city"], "Madrid");
    }

    #[test]
    fn test_parse_functiongemma_object_multiple_args() {
        let obj = parse_functiongemma_object(
            "{city:<escape>Barcelona<escape>,lang:<escape>es<escape>,sentences:3}",
        )
        .unwrap();
        assert_eq!(obj["city"], "Barcelona");
        assert_eq!(obj["lang"], "es");
        assert_eq!(obj["sentences"], 3);
    }

    #[test]
    fn test_parse_functiongemma_object_nested() {
        let obj = parse_functiongemma_object("{outer:{inner:<escape>value<escape>}}").unwrap();
        assert_eq!(obj["outer"]["inner"], "value");
    }

    #[test]
    fn test_parse_functiongemma_object_array() {
        let obj = parse_functiongemma_object("{items:[1,2,3]}").unwrap();
        assert_eq!(obj["items"], serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn test_find_function_call_blocks_single() {
        let raw = "<start_function_call>call:test{}<end_function_call>";
        let blocks = find_function_call_blocks(raw);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0], "call:test{}");
    }

    #[test]
    fn test_find_function_call_blocks_multiple() {
        let raw = "<start_function_call>call:foo{}<end_function_call> some text <start_function_call>call:bar{}<end_function_call>";
        let blocks = find_function_call_blocks(raw);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0], "call:foo{}");
        assert_eq!(blocks[1], "call:bar{}");
    }

    #[test]
    fn test_find_function_call_blocks_empty() {
        let raw = "no function calls here";
        let blocks = find_function_call_blocks(raw);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_find_function_call_blocks_unclosed() {
        let raw = "<start_function_call>call:test{} but no end tag";
        let blocks = find_function_call_blocks(raw);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_extract_function_call_json_tagged() {
        let raw = "prefix <start_function_call>call:test{arg:<escape>val<escape>}<end_function_call> suffix";
        let inner = extract_function_call_json_tagged(raw).unwrap();
        assert_eq!(inner, "call:test{arg:<escape>val<escape>}");
    }

    #[test]
    fn test_extract_function_call_json_tagged_none() {
        assert!(extract_function_call_json_tagged("no tags").is_none());
        assert!(
            extract_function_call_json_tagged("<start_function_call><end_function_call>").is_none()
        );
    }

    #[test]
    fn test_parse_functiongemma_call_valid() {
        let (tool, args) =
            parse_functiongemma_call("call:wikipedia_city_lookup{city:<escape>Madrid<escape>}")
                .unwrap();
        assert_eq!(tool, "wikipedia_city_lookup");
        assert_eq!(args["city"], "Madrid");
    }

    #[test]
    fn test_parse_functiongemma_call_with_lang() {
        let (tool, args) = parse_functiongemma_call(
            "call:wikipedia_city_lookup{city:<escape>Barcelona<escape>,lang:<escape>ca<escape>}",
        )
        .unwrap();
        assert_eq!(tool, "wikipedia_city_lookup");
        assert_eq!(args["city"], "Barcelona");
        assert_eq!(args["lang"], "ca");
    }

    #[test]
    fn test_parse_functiongemma_call_missing_prefix() {
        assert!(
            parse_functiongemma_call("wikipedia_city_lookup{city:<escape>Madrid<escape>}").is_err()
        );
    }

    #[test]
    fn test_parse_functiongemma_call_empty_tool() {
        assert!(parse_functiongemma_call("call:{city:<escape>Madrid<escape>}").is_err());
    }

    #[test]
    fn test_parse_functiongemma_call_no_args() {
        assert!(parse_functiongemma_call("call:test").is_err());
    }
}

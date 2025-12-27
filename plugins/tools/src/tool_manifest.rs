use crate::tools::ToolDefinition;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct CompiledManifest {
    pub policies: HashMap<String, ToolPolicy>,
    pub function_declarations: String,
}

#[derive(Debug, Clone)]
pub struct ToolPolicy {
    pub read_only: bool,
    pub default_lang: Option<String>,
    pub default_sentences: Option<u8>,
    pub required_args: Vec<String>,
    pub arg_types: HashMap<String, JsonArgType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonArgType {
    String,
    Integer,
    Number,
    Boolean,
    Object,
    Array,
}

impl ToolPolicy {
    pub fn validate_args(&self, args: &serde_json::Value) -> Result<(), String> {
        let Some(obj) = args.as_object() else {
            return Err("args must be an object".to_string());
        };

        for req in &self.required_args {
            if !obj.contains_key(req) {
                return Err(format!("args.{req} is required"));
            }
        }

        for (k, ty) in &self.arg_types {
            let Some(v) = obj.get(k) else { continue };
            let ok = match ty {
                JsonArgType::String => v.is_string(),
                JsonArgType::Integer => v.as_i64().is_some() || v.as_u64().is_some(),
                JsonArgType::Number => v.is_number(),
                JsonArgType::Boolean => v.is_boolean(),
                JsonArgType::Object => v.is_object(),
                JsonArgType::Array => v.is_array(),
            };
            if !ok {
                return Err(format!("args.{k} has wrong type"));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct ToolManifest {
    tools: Vec<ToolDef>,
}

#[derive(Debug, Deserialize)]
struct ToolDef {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    read_only: bool,
    #[serde(default)]
    args_schema: serde_json::Value,
}

const MAX_MANIFEST_BYTES: usize = 64 * 1024;
const MAX_TOOLS: usize = 32;

pub fn validate_and_compile(manifest_json: &str) -> Result<CompiledManifest, String> {
    let manifest_json = manifest_json.trim();
    if manifest_json.is_empty() {
        return Err("tool_manifest cannot be empty".to_string());
    }
    if manifest_json.len() > MAX_MANIFEST_BYTES {
        return Err(format!(
            "tool_manifest too large ({} bytes > {} bytes)",
            manifest_json.len(),
            MAX_MANIFEST_BYTES
        ));
    }

    let manifest: ToolManifest = serde_json::from_str(manifest_json).map_err(|e| e.to_string())?;
    if manifest.tools.is_empty() {
        return Err("tool_manifest.tools must be a non-empty array".to_string());
    }
    if manifest.tools.len() > MAX_TOOLS {
        return Err(format!(
            "tool_manifest.tools too large ({} > {})",
            manifest.tools.len(),
            MAX_TOOLS
        ));
    }

    let mut policies: HashMap<String, ToolPolicy> = HashMap::new();
    let mut function_declarations = String::new();

    for (i, t) in manifest.tools.into_iter().enumerate() {
        let name = t.name.trim();
        if name.is_empty() {
            return Err(format!("tool_manifest.tools[{i}].name is required"));
        }
        if name.len() > 128 {
            return Err(format!("tool_manifest.tools[{i}].name too long"));
        }
        if policies.contains_key(name) {
            return Err(format!("tool_manifest.tools[{i}].name duplicated: {name}"));
        }

        let schema_info = extract_schema_info(&t.args_schema, i)?;

        policies.insert(
            name.to_string(),
            ToolPolicy {
                read_only: t.read_only,
                default_lang: schema_info.default_lang,
                default_sentences: schema_info.default_sentences,
                required_args: schema_info.required_args,
                arg_types: schema_info.arg_types,
            },
        );

        let description = t.description.unwrap_or_default();

        // FunctionGemma expects a very specific, non-JSON function declaration format.
        // See: https://ai.google.dev/gemma/docs/functiongemma/formatting-and-best-practices
        function_declarations.push_str(&functiongemma_declaration(
            name,
            &description,
            &t.args_schema,
        )?);
    }

    Ok(CompiledManifest {
        policies,
        function_declarations,
    })
}

/// Generate FunctionGemma declarations from tool definitions.
pub fn generate_declarations(tools: &[ToolDefinition]) -> String {
    let mut declarations = String::new();
    for tool in tools {
        match functiongemma_declaration(&tool.name, &tool.description, &tool.args_schema) {
            Ok(decl) => declarations.push_str(&decl),
            Err(e) => {
                tracing::warn!(tool = %tool.name, error = %e, "Failed to generate declaration");
            }
        }
    }
    declarations
}

pub fn policy_from_definition(tool: &ToolDefinition) -> ToolPolicy {
    let schema_info = extract_schema_info(&tool.args_schema, 0).unwrap_or_else(|_| SchemaInfo {
        default_lang: None,
        default_sentences: None,
        required_args: Vec::new(),
        arg_types: HashMap::new(),
    });

    ToolPolicy {
        read_only: tool.read_only,
        default_lang: schema_info.default_lang,
        default_sentences: schema_info.default_sentences,
        required_args: schema_info.required_args,
        arg_types: schema_info.arg_types,
    }
}

fn parse_json_arg_type(s: &str) -> Option<JsonArgType> {
    match s {
        "string" => Some(JsonArgType::String),
        "integer" => Some(JsonArgType::Integer),
        "number" => Some(JsonArgType::Number),
        "boolean" => Some(JsonArgType::Boolean),
        "object" => Some(JsonArgType::Object),
        "array" => Some(JsonArgType::Array),
        _ => None,
    }
}

#[derive(Debug, Clone)]
enum FgValue {
    Str(String),
    Obj(Vec<(String, FgValue)>),
    Arr(Vec<FgValue>),
}

impl FgValue {
    fn render(&self, out: &mut String) {
        match self {
            FgValue::Str(s) => {
                out.push_str("<escape>");
                out.push_str(s);
                out.push_str("<escape>");
            }
            FgValue::Obj(fields) => {
                out.push('{');
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(k);
                    out.push(':');
                    v.render(out);
                }
                out.push('}');
            }
            FgValue::Arr(items) => {
                out.push('[');
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.render(out);
                }
                out.push(']');
            }
        }
    }
}

fn fg_clean_string(s: &str) -> String {
    // `<escape>` is itself a token delimiter; avoid producing it inside literals.
    s.replace("<escape>", " ")
        .replace('\n', " ")
        .trim()
        .to_string()
}

fn fg_type_from_json_schema(schema: &serde_json::Value) -> &'static str {
    match schema.get("type").and_then(|t| t.as_str()) {
        Some("string") => "STRING",
        Some("integer") => "INTEGER",
        Some("number") => "NUMBER",
        Some("boolean") => "BOOLEAN",
        Some("array") => "ARRAY",
        Some("object") => "OBJECT",
        _ => "OBJECT",
    }
}

fn schema_to_fg(schema: &serde_json::Value) -> FgValue {
    let ty = fg_type_from_json_schema(schema).to_string();
    let mut fields: Vec<(String, FgValue)> = Vec::new();

    if let Some(desc) = schema.get("description").and_then(|d| d.as_str()) {
        let d = fg_clean_string(desc);
        if !d.is_empty() {
            fields.push(("description".to_string(), FgValue::Str(d)));
        }
    }

    if let Some(enum_vals) = schema.get("enum").and_then(|e| e.as_array()) {
        let mut items = Vec::new();
        for v in enum_vals {
            if let Some(s) = v.as_str() {
                let s = fg_clean_string(s);
                if !s.is_empty() {
                    items.push(FgValue::Str(s));
                }
            }
        }
        if !items.is_empty() {
            fields.push(("enum".to_string(), FgValue::Arr(items)));
        }
    }

    if ty == "OBJECT" {
        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
            let mut prop_fields = Vec::new();
            for (k, v) in props {
                prop_fields.push((k.clone(), schema_to_fg(v)));
            }
            if !prop_fields.is_empty() {
                fields.push(("properties".to_string(), FgValue::Obj(prop_fields)));
            }
        }
        if let Some(req) = schema.get("required").and_then(|r| r.as_array()) {
            let mut req_items = Vec::new();
            for r in req {
                if let Some(s) = r.as_str() {
                    let s = fg_clean_string(s);
                    if !s.is_empty() {
                        req_items.push(FgValue::Str(s));
                    }
                }
            }
            if !req_items.is_empty() {
                fields.push(("required".to_string(), FgValue::Arr(req_items)));
            }
        }
    }

    if ty == "ARRAY" {
        if let Some(items_schema) = schema.get("items") {
            fields.push(("items".to_string(), schema_to_fg(items_schema)));
        }
    }

    fields.push(("type".to_string(), FgValue::Str(ty)));
    FgValue::Obj(fields)
}

fn functiongemma_declaration(
    name: &str,
    description: &str,
    parameters_schema: &serde_json::Value,
) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("tool name is empty".to_string());
    }

    let mut out = String::new();
    out.push_str("<start_function_declaration>");
    out.push_str("declaration:");
    out.push_str(name);
    out.push('{');
    out.push_str("description:");
    FgValue::Str(fg_clean_string(description)).render(&mut out);
    out.push_str(",parameters:");
    schema_to_fg(parameters_schema).render(&mut out);
    out.push('}');
    out.push_str("<end_function_declaration>\n");
    Ok(out)
}

struct SchemaInfo {
    default_lang: Option<String>,
    default_sentences: Option<u8>,
    required_args: Vec<String>,
    arg_types: HashMap<String, JsonArgType>,
}

fn extract_schema_info(
    args_schema: &serde_json::Value,
    tool_index: usize,
) -> Result<SchemaInfo, String> {
    let mut default_lang: Option<String> = None;
    let mut default_sentences: Option<u8> = None;
    let mut required_args: Vec<String> = Vec::new();
    let mut arg_types: HashMap<String, JsonArgType> = HashMap::new();

    let Some(props) = args_schema.get("properties").and_then(|p| p.as_object()) else {
        return Ok(SchemaInfo {
            default_lang: None,
            default_sentences: None,
            required_args,
            arg_types,
        });
    };

    if let Some(req) = args_schema.get("required").and_then(|r| r.as_array()) {
        for (j, v) in req.iter().enumerate() {
            let Some(s) = v.as_str() else {
                return Err(format!(
                    "tool_manifest.tools[{tool_index}].args_schema.required[{j}] must be a string"
                ));
            };
            let s = s.trim();
            if !s.is_empty() {
                required_args.push(s.to_string());
            }
        }
    }

    for (name, schema) in props {
        if let Some(ty) = schema
            .get("type")
            .and_then(|t| t.as_str())
            .and_then(parse_json_arg_type)
        {
            arg_types.insert(name.to_string(), ty);
        }
    }

    if let Some(lang) = props.get("lang") {
        if let Some(d) = lang.get("default") {
            if let Some(s) = d.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    default_lang = Some(trimmed.to_string());
                }
            } else {
                return Err(format!(
                    "tool_manifest.tools[{tool_index}].args_schema.properties.lang.default must be a string"
                ));
            }
        }
    }

    if let Some(sentences) = props.get("sentences") {
        if let Some(d) = sentences.get("default") {
            if let Some(n) = d.as_u64() {
                if let Ok(n) = u8::try_from(n) {
                    default_sentences = Some(n.clamp(1, 10));
                } else {
                    return Err(format!(
                        "tool_manifest.tools[{tool_index}].args_schema.properties.sentences.default out of range"
                    ));
                }
            } else {
                return Err(format!(
                    "tool_manifest.tools[{tool_index}].args_schema.properties.sentences.default must be an integer"
                ));
            }
        }
    }

    Ok(SchemaInfo {
        default_lang,
        default_sentences,
        required_args,
        arg_types,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_policy_and_defaults() {
        let json = r#"{
          "tools": [
            {
              "name": "wikipedia_city_lookup",
              "read_only": true,
              "args_schema": {
                "type": "object",
                "properties": {
                  "city": { "type": "string" },
                  "lang": { "type": "string", "default": "en" },
                  "sentences": { "type": "integer", "default": 2 }
                },
                "required": ["city"]
              }
            }
          ]
        }"#;

        let compiled = validate_and_compile(json).unwrap();
        let p = compiled.policies.get("wikipedia_city_lookup").unwrap();
        assert!(p.read_only);
        assert_eq!(p.default_lang.as_deref(), Some("en"));
        assert_eq!(p.default_sentences, Some(2));
        assert_eq!(p.required_args, vec!["city".to_string()]);
        assert_eq!(p.arg_types.get("city").copied(), Some(JsonArgType::String));
        assert!(compiled
            .function_declarations
            .contains("<start_function_declaration>"));
        assert!(compiled
            .function_declarations
            .contains("declaration:wikipedia_city_lookup{"));
        assert!(compiled.function_declarations.contains("parameters:{"));
    }

    #[test]
    fn rejects_empty_name() {
        let json = r#"{"tools":[{"name":"  ","read_only":true,"args_schema":{}}]}"#;
        let err = validate_and_compile(json).unwrap_err();
        assert!(err.contains("tools[0].name"));
    }

    #[test]
    fn rejects_bad_sentences_default() {
        let json = r#"{
          "tools": [
            {
              "name": "wikipedia_city_lookup",
              "read_only": true,
              "args_schema": { "properties": { "sentences": { "default": "two" } } }
            }
          ]
        }"#;
        let err = validate_and_compile(json).unwrap_err();
        assert!(err.contains("sentences.default"));
    }

    #[test]
    fn generate_declarations_from_tool_definitions() {
        let tools = vec![
            ToolDefinition {
                name: "web_search".to_string(),
                description: "Search the web".to_string(),
                read_only: true,
                args_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" }
                    },
                    "required": ["query"]
                }),
            },
            ToolDefinition {
                name: "app_launcher".to_string(),
                description: "Launch applications".to_string(),
                read_only: true,
                args_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "app": { "type": "string" }
                    },
                    "required": ["app"]
                }),
            },
        ];

        let declarations = generate_declarations(&tools);
        assert!(declarations.contains("declaration:web_search{"));
        assert!(declarations.contains("declaration:app_launcher{"));
        assert!(declarations.contains("Search the web"));
        assert!(declarations.contains("<start_function_declaration>"));
        assert!(declarations.contains("<end_function_declaration>"));
    }

    #[test]
    fn policy_from_definition_extracts_metadata() {
        let tool = ToolDefinition {
            name: "test_tool".to_string(),
            description: "Test".to_string(),
            read_only: false,
            args_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "count": { "type": "integer" }
                },
                "required": ["query"]
            }),
        };

        let policy = policy_from_definition(&tool);
        assert!(!policy.read_only);
        assert_eq!(policy.required_args, vec!["query".to_string()]);
        assert_eq!(policy.arg_types.get("query").copied(), Some(JsonArgType::String));
        assert_eq!(policy.arg_types.get("count").copied(), Some(JsonArgType::Integer));
    }
}

//! Output validation for LLM responses (Task 8.6) —
//! port of `helen/runtime/output_validator.py`.
//!
//! Validates LLM output against an agent's `output_contract`:
//! simple string contracts ("json"/"text") or JSON-schema dicts.

use serde_json::{json, Value};

/// `validate_output` — validate LLM output against a contract.
/// Returns `{valid, violation, parsed}`.
pub fn validate_output(output: &str, contract: Option<&Value>) -> Value {
    match contract {
        None => json!({"valid": true, "violation": "", "parsed": output}),
        Some(Value::String(c)) => validate_simple_contract(output, c),
        Some(v @ Value::Object(_)) => validate_schema_contract(output, v),
        Some(other) => json!({
            "valid": false,
            "violation": format!("Invalid contract type: {}", type_name(other)),
            "parsed": null,
        }),
    }
}

fn type_name(v: &Value) -> String {
    match v {
        Value::Null => "NoneType".into(),
        Value::Bool(_) => "bool".into(),
        Value::Number(n) if n.is_i64() || n.is_u64() => "int".into(),
        Value::Number(_) => "float".into(),
        Value::String(_) => "str".into(),
        Value::Array(_) => "list".into(),
        Value::Object(_) => "dict".into(),
    }
}

fn validate_simple_contract(output: &str, contract: &str) -> Value {
    match contract {
        "json" => validate_json(output),
        "text" => json!({"valid": true, "violation": "", "parsed": output}),
        other => json!({
            "valid": false,
            "violation": format!("Unknown contract type: {other}"),
            "parsed": null,
        }),
    }
}

/// `_validate_json` — parse output as JSON.
pub fn validate_json(output: &str) -> Value {
    let trimmed = output.trim();
    match serde_json::from_str::<Value>(trimmed) {
        Ok(parsed) => json!({"valid": true, "violation": "", "parsed": parsed}),
        Err(e) => json!({
            "valid": false,
            "violation": format!("Output is not valid JSON: {e}"),
            "parsed": null,
        }),
    }
}

fn validate_schema_contract(output: &str, schema: &Value) -> Value {
    // First parse as JSON.
    let json_result = validate_json(output);
    if !json_result["valid"].as_bool().unwrap_or(false) {
        return json_result;
    }
    let parsed = json_result["parsed"].clone();

    // Validate top-level type.
    let schema_type = schema.get("type").and_then(|v| v.as_str());
    if let Some(t) = schema_type {
        if !validate_type(&parsed, t) {
            return json!({
                "valid": false,
                "violation": format!("Expected type '{t}', got {}", type_name(&parsed)),
                "parsed": parsed,
            });
        }
    }

    // Validate required fields.
    if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
        if let Some(obj) = parsed.as_object() {
            let missing: Vec<String> = required
                .iter()
                .filter_map(|f| f.as_str().map(String::from))
                .filter(|f| !obj.contains_key(f))
                .collect();
            if !missing.is_empty() {
                return json!({
                    "valid": false,
                    "violation": format!("Missing required fields: {}", missing.join(", ")),
                    "parsed": parsed,
                });
            }
        }
    }

    // Validate properties.
    if let Some(properties) = schema.get("properties").and_then(|v| v.as_object()) {
        if let Some(obj) = parsed.as_object() {
            for (prop_name, prop_schema) in properties {
                if let Some(prop_value) = obj.get(prop_name) {
                    let result = validate_property(prop_value, prop_schema, prop_name);
                    if !result["valid"].as_bool().unwrap_or(false) {
                        return result;
                    }
                }
            }
        }
    }

    json!({"valid": true, "violation": "", "parsed": parsed})
}

fn validate_type(value: &Value, expected: &str) -> bool {
    match expected {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        _ => true, // Unknown type, allow
    }
}

fn validate_property(value: &Value, schema: &Value, prop_name: &str) -> Value {
    // Type check.
    if let Some(t) = schema.get("type").and_then(|v| v.as_str()) {
        if !validate_type(value, t) {
            return json!({
                "valid": false,
                "violation": format!(
                    "Property '{prop_name}': expected type '{t}', got {}",
                    type_name(value)
                ),
                "parsed": value,
            });
        }
    }

    // Enum check.
    if let Some(enum_values) = schema.get("enum").and_then(|v| v.as_array()) {
        if !enum_values.contains(value) {
            let allowed = serde_json::to_string(enum_values).unwrap_or_default();
            return json!({
                "valid": false,
                "violation": format!(
                    "Property '{prop_name}': value {value} not in allowed values {allowed}"
                ),
                "parsed": value,
            });
        }
    }

    // Number constraints.
    if value.is_number() {
        if let Some(min) = schema.get("min").and_then(|v| v.as_f64()) {
            if value.as_f64().unwrap_or(f64::MAX) < min {
                return json!({
                    "valid": false,
                    "violation": format!(
                        "Property '{prop_name}': value {value} is less than minimum {min}"
                    ),
                    "parsed": value,
                });
            }
        }
        if let Some(max) = schema.get("max").and_then(|v| v.as_f64()) {
            if value.as_f64().unwrap_or(f64::MIN) > max {
                return json!({
                    "valid": false,
                    "violation": format!(
                        "Property '{prop_name}': value {value} is greater than maximum {max}"
                    ),
                    "parsed": value,
                });
            }
        }
    }

    // String constraints.
    if let Some(s) = value.as_str() {
        let len = s.chars().count();
        if let Some(min_len) = schema.get("minLength").and_then(|v| v.as_u64()) {
            if (len as u64) < min_len {
                return json!({
                    "valid": false,
                    "violation": format!(
                        "Property '{prop_name}': string length {len} is less than minimum {min_len}"
                    ),
                    "parsed": value,
                });
            }
        }
        if let Some(max_len) = schema.get("maxLength").and_then(|v| v.as_u64()) {
            if (len as u64) > max_len {
                return json!({
                    "valid": false,
                    "violation": format!(
                        "Property '{prop_name}': string length {len} is greater than maximum {max_len}"
                    ),
                    "parsed": value,
                });
            }
        }
    }

    json!({"valid": true, "violation": "", "parsed": value})
}

/// Convenience: returns true if `output` is valid per `contract`.
pub fn is_valid(output: &str, contract: Option<&Value>) -> bool {
    validate_output(output, contract)["valid"]
        .as_bool()
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_contract_always_valid() {
        let r = validate_output("anything", None);
        assert!(r["valid"].as_bool().expect("bool value"));
        assert_eq!(r["parsed"], "anything");
    }

    #[test]
    fn text_contract_passes() {
        let r = validate_output("plain text", Some(&json!("text")));
        assert!(r["valid"].as_bool().expect("bool value"));
    }

    #[test]
    fn json_contract_valid() {
        let r = validate_output(r#"{"a": 1}"#, Some(&json!("json")));
        assert!(r["valid"].as_bool().expect("bool value"));
        assert_eq!(r["parsed"]["a"], 1);
    }

    #[test]
    fn json_contract_invalid() {
        let r = validate_output("{not json", Some(&json!("json")));
        assert!(!r["valid"].as_bool().expect("bool value"));
        assert!(r["violation"].as_str().expect("string value").contains("not valid JSON"));
    }

    #[test]
    fn schema_required_fields() {
        let schema = json!({"type": "object", "required": ["name"]});
        let r = validate_output(r#"{"age": 3}"#, Some(&schema));
        assert!(!r["valid"].as_bool().expect("bool value"));
        assert!(r["violation"].as_str().expect("string value").contains("name"));
    }

    #[test]
    fn schema_type_mismatch() {
        let schema = json!({"type": "object"});
        let r = validate_output("[1,2]", Some(&schema));
        assert!(!r["valid"].as_bool().expect("bool value"));
        assert!(r["violation"]
            .as_str()
            .unwrap()
            .contains("Expected type 'object'"));
    }

    #[test]
    fn schema_property_enum() {
        let schema = json!({
            "type": "object",
            "properties": {"color": {"type": "string", "enum": ["red", "blue"]}}
        });
        let r = validate_output(r#"{"color": "green"}"#, Some(&schema));
        assert!(!r["valid"].as_bool().expect("bool value"));
        assert!(r["violation"].as_str().expect("string value").contains("green"));
    }

    #[test]
    fn schema_property_numeric_bounds() {
        let schema = json!({
            "type": "object",
            "properties": {"n": {"type": "number", "min": 0, "max": 10}}
        });
        assert!(validate_output(r#"{"n": 5}"#, Some(&schema))["valid"]
            .as_bool()
            .unwrap());
        let r = validate_output(r#"{"n": 20}"#, Some(&schema));
        assert!(!r["valid"].as_bool().expect("bool value"));
        assert!(r["violation"]
            .as_str()
            .unwrap()
            .contains("greater than maximum 10"));
    }

    #[test]
    fn schema_property_string_length() {
        let schema = json!({
            "type": "object",
            "properties": {"name": {"type": "string", "minLength": 2, "maxLength": 5}}
        });
        assert!(
            validate_output(r#"{"name": "abc"}"#, Some(&schema))["valid"]
                .as_bool()
                .unwrap()
        );
        let r = validate_output(r#"{"name": "x"}"#, Some(&schema));
        assert!(!r["valid"].as_bool().expect("bool value"));
        assert!(r["violation"]
            .as_str()
            .unwrap()
            .contains("less than minimum 2"));
    }

    #[test]
    fn unknown_contract_type() {
        let r = validate_output("x", Some(&json!("yaml")));
        assert!(!r["valid"].as_bool().expect("bool value"));
        assert!(r["violation"]
            .as_str()
            .unwrap()
            .contains("Unknown contract type"));
    }

    #[test]
    fn invalid_contract_value() {
        let r = validate_output("x", Some(&json!(42)));
        assert!(!r["valid"].as_bool().expect("bool value"));
        assert!(r["violation"]
            .as_str()
            .unwrap()
            .contains("Invalid contract type"));
    }
}

//! Error diagnostics for AI-native debugging (Task 8.6) —
//! port of `helen/runtime/error_diagnostics.py`.
//!
//! Deterministic (zero-LLM) classification and suggestion generation for
//! Helen runtime errors: template registry + rule-based matching.

use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// `ERROR_SUGGESTION_REGISTRY` — exception type name -> diagnostic template.
/// Each entry: category (semantic class), template with `{field}` placeholders,
/// fields to extract from exception context, and optional regex rules.
pub fn error_suggestion_registry() -> HashMap<String, RegistryEntry> {
    let mut m = HashMap::new();
    m.insert(
        "AnyError".into(),
        RegistryEntry {
            category: "GenericError".into(),
            template: "通用错误。检查错误消息 '{message}' 里的具体描述。".into(),
            fields: vec!["message".into()],
            rules: vec![],
        },
    );
    m.insert(
        "LLMError".into(),
        RegistryEntry {
            category: "LLMGenericError".into(),
            template: "LLM 调用失败。检查 LLM 配置（base_url、api_key、model）是否正确。如果问题持续，查看 :llm_log 获取详细调用日志。".into(),
            fields: vec![],
            rules: vec![],
        },
    );
    m.insert(
        "TimeoutError".into(),
        RegistryEntry {
            category: "LLMTimeout".into(),
            template: "LLM 调用超时。考虑：(1) 增加 timeout 配置，(2) 减小 prompt 长度，(3) 检查网络连接，(4) 确认 LLM 服务是否可用。".into(),
            fields: vec![],
            rules: vec![],
        },
    );
    m.insert(
        "ModelError".into(),
        RegistryEntry {
            category: "LLMModelUnavailable".into(),
            template: "模型不可用或配额耗尽。检查：(1) model 名称是否正确，(2) API key 是否有效，(3) 账户余额是否充足。".into(),
            fields: vec![],
            rules: vec![],
        },
    );
    m.insert(
        "PromptTooLongError".into(),
        RegistryEntry {
            category: "LLMContextOverflow".into(),
            template: "Prompt 超出模型上下文窗口（{tokens_used}/{tokens_limit} tokens）。使用 compress_context() 压缩历史，或 clear_context() 清空，或减小 agent prompt 模板大小。".into(),
            fields: vec!["tokens_used".into(), "tokens_limit".into()],
            rules: vec![],
        },
    );
    m.insert(
        "AgentError".into(),
        RegistryEntry {
            category: "AgentCallFailed".into(),
            template: "Agent '{agent_name}' 调用失败。根因：{cause}。检查：(1) agent 参数类型是否匹配，(2) agent 内部逻辑是否有 bug，(3) agent 的 LLM 调用是否失败（用 :llm_log 查看）。".into(),
            fields: vec!["agent_name".into(), "cause".into()],
            rules: vec![],
        },
    );
    m.insert(
        "LLMOutputContractError".into(),
        RegistryEntry {
            category: "LLMOutputContractViolation".into(),
            template: "Agent '{agent_name}' 的 LLM 输出不符合契约要求。违反：{violation}。检查：(1) agent prompt 是否明确要求输出格式，(2) output_contract 定义是否正确，(3) 考虑在 prompt 中添加更明确的格式说明或示例。".into(),
            fields: vec!["agent_name".into(), "violation".into()],
            rules: vec![],
        },
    );
    m.insert(
        "ToolError".into(),
        RegistryEntry {
            category: "ToolCallFailed".into(),
            template: "工具调用失败。检查：(1) 工具参数是否符合 schema，(2) 工具是否返回错误，(3) 加重试逻辑或 try/catch 包裹。".into(),
            fields: vec![],
            rules: vec![],
        },
    );
    m.insert(
        "RuntimeError".into(),
        RegistryEntry {
            category: "RuntimeGenericError".into(),
            template: "运行时错误：{message}。检查变量类型和边界条件。".into(),
            fields: vec!["message".into()],
            rules: vec![
                Rule {
                    pattern: "division by zero".into(),
                    suggestion: "除零错误。在除法前检查分母是否为 0。".into(),
                },
                Rule {
                    pattern: "expected .*, got .*".into(),
                    suggestion: "类型不匹配。检查函数返回值类型是否符合预期。".into(),
                },
                Rule {
                    pattern: "undefined variable .*".into(),
                    suggestion: "未定义变量。检查变量是否已声明，或作用域是否正确。".into(),
                },
                Rule {
                    pattern: "index .* out of range".into(),
                    suggestion: "索引越界。检查数组/列表长度，确保索引在有效范围内。".into(),
                },
                Rule {
                    pattern: "key .* not found".into(),
                    suggestion: "字典键不存在。检查键名是否正确，或用 get() 方法提供默认值。"
                        .into(),
                },
            ],
        },
    );
    m.insert(
        "AssertionError".into(),
        RegistryEntry {
            category: "AssertionFailed".into(),
            template: "断言失败：{message}。程序状态不符合预期。检查断言条件是否正确，以及上游数据是否异常。".into(),
            fields: vec!["message".into()],
            rules: vec![],
        },
    );
    m.insert(
        "AggregateError".into(),
        RegistryEntry {
            category: "MultipleFailures".into(),
            template: "{error_count} 个并发任务失败。查看 errors 列表里的每个具体错误。通常先修第一个错误，后续错误可能是级联失败。".into(),
            fields: vec!["error_count".into()],
            rules: vec![],
        },
    );
    m
}

pub struct Rule {
    pub pattern: String,
    pub suggestion: String,
}

pub struct RegistryEntry {
    pub category: String,
    pub template: String,
    pub fields: Vec<String>,
    pub rules: Vec<Rule>,
}

/// `generate_suggestion` — produce (category, suggestion) for an error.
/// Rule-based matching first (more specific), then template fallback.
pub fn generate_suggestion(
    error_type: &str,
    message: &str,
    context: Option<&Map<String, Value>>,
) -> (String, String) {
    let ctx = context.cloned().unwrap_or_default();
    let registry = error_suggestion_registry();

    let Some(entry) = registry.get(error_type) else {
        return (
            "UnknownError".into(),
            format!("未知错误类型 '{error_type}'。检查错误消息：{message}"),
        );
    };

    // Rule-based matching (case-insensitive regex).
    for rule in &entry.rules {
        let re = regex::RegexBuilder::new(&rule.pattern)
            .case_insensitive(true)
            .build()
            .ok();
        if let Some(re) = re {
            if re.is_match(message) {
                return (entry.category.clone(), rule.suggestion.clone());
            }
        }
    }

    // Template-based suggestion.
    let mut kwargs: HashMap<String, String> = HashMap::new();
    kwargs.insert("message".into(), message.to_string());
    for field in &entry.fields {
        if field == "message" {
            continue;
        }
        if let Some(v) = ctx.get(field) {
            kwargs.insert(field.clone(), stringify_value(v));
        } else if field == "error_count" && ctx.contains_key("errors") {
            let n = ctx
                .get("errors")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            kwargs.insert(field.clone(), n.to_string());
        } else {
            kwargs.insert(field.clone(), format!("<{field} not available>"));
        }
    }

    // Replace {field} placeholders; unknown braces left as-is (Python .format
    // would raise on unknown keys, but we pre-fill every field).
    let mut suggestion = entry.template.clone();
    for (k, v) in &kwargs {
        suggestion = suggestion.replace(&format!("{{{k}}}"), v);
    }

    (entry.category.clone(), suggestion)
}

fn stringify_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "<null>".into(),
        other => other.to_string(),
    }
}

/// `_is_message_like` — check if a value looks like a Message (uuid+role+content).
fn is_message_like(v: &Value) -> bool {
    v.is_object()
        && v.get("uuid").is_some()
        && v.get("role").is_some()
        && v.get("content").is_some()
}

/// `build_data_flow` — infer data flow from scope + call stack.
pub fn build_data_flow(scope: &Value, call_stack: &[Value]) -> Vec<Value> {
    let mut flow = Vec::new();

    // Rule 1: Message-like scope vars trace their origin.
    if let Some(map) = scope.as_object() {
        for (name, value) in map {
            if is_message_like(value) {
                let uuid = value.get("uuid").and_then(|v| v.as_str()).unwrap_or("");
                let agent = value
                    .get("agent_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                if !uuid.is_empty() {
                    flow.push(json!({
                        "variable": name,
                        "source": uuid,
                        "via": agent,
                    }));
                }
            }
        }
    }

    // Rule 2: If no message vars, trace from call stack frame locations.
    if flow.is_empty() {
        for frame in call_stack.iter().take(5) {
            flow.push(json!({
                "variable": "",
                "source": frame.get("location").cloned().unwrap_or(json!("")),
                "via": frame.get("function").cloned().unwrap_or(json!("")),
            }));
        }
    }

    flow
}

/// `generate_diagnostics` — main entry: category + suggestion + data_flow.
pub fn generate_diagnostics(
    error_type: &str,
    message: &str,
    scope: Option<&Value>,
    call_stack: Option<&[Value]>,
    exception_context: Option<&Value>,
) -> Value {
    let scope = scope.cloned().unwrap_or_else(|| json!({}));
    let call_stack = call_stack.unwrap_or(&[]).to_vec();
    let exc_ctx = exception_context
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let (category, suggestion) = generate_suggestion(error_type, message, Some(&exc_ctx));
    let data_flow = build_data_flow(&scope, &call_stack);

    json!({
        "diagnostic_category": category,
        "suggestion": suggestion,
        "data_flow": data_flow,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_based_runtime_error() {
        let (cat, sug) = generate_suggestion("RuntimeError", "division by zero", None);
        assert_eq!(cat, "RuntimeGenericError");
        assert!(sug.contains("除零错误"), "{sug}");
    }

    #[test]
    fn template_prompt_too_long() {
        let mut ctx = Map::new();
        ctx.insert("tokens_used".into(), json!(9000));
        ctx.insert("tokens_limit".into(), json!(10000));
        let (cat, sug) = generate_suggestion("PromptTooLongError", "prompt too long", Some(&ctx));
        assert_eq!(cat, "LLMContextOverflow");
        assert!(sug.contains("9000/10000"), "{sug}");
    }

    #[test]
    fn unknown_type_fallback() {
        let (cat, sug) = generate_suggestion("BogusError", "boom", None);
        assert_eq!(cat, "UnknownError");
        assert!(sug.contains("BogusError"), "{sug}");
    }

    #[test]
    fn aggregate_error_count_from_errors() {
        let mut ctx = Map::new();
        ctx.insert("errors".into(), json!(["a", "b", "c"]));
        let (cat, sug) = generate_suggestion("AggregateError", "3 failed", Some(&ctx));
        assert_eq!(cat, "MultipleFailures");
        assert!(sug.starts_with("3 个并发任务失败"), "{sug}");
    }

    #[test]
    fn missing_field_placeholder() {
        let (cat, sug) = generate_suggestion("AgentError", "x", None);
        assert_eq!(cat, "AgentCallFailed");
        assert!(sug.contains("<agent_name not available>"), "{sug}");
    }

    #[test]
    fn data_flow_from_message_scope() {
        let scope = json!({
            "m": {"uuid": "u1", "role": "user", "content": "hi", "agent_name": "a1"}
        });
        let flow = build_data_flow(&scope, &[]);
        assert_eq!(flow.len(), 1);
        assert_eq!(flow[0]["variable"], "m");
        assert_eq!(flow[0]["source"], "u1");
        assert_eq!(flow[0]["via"], "a1");
    }

    #[test]
    fn data_flow_falls_back_to_call_stack() {
        let cs = vec![json!({"location": "f:1:1", "function": "g"})];
        let flow = build_data_flow(&json!({"x": 1}), &cs);
        assert_eq!(flow.len(), 1);
        assert_eq!(flow[0]["source"], "f:1:1");
        assert_eq!(flow[0]["via"], "g");
    }

    #[test]
    fn diagnostics_bundle() {
        let d = generate_diagnostics("TimeoutError", "timeout", None, None, None);
        assert_eq!(d["diagnostic_category"], "LLMTimeout");
        assert!(d["suggestion"].as_str().expect("string value").contains("超时"));
        assert!(d["data_flow"].as_array().expect("array exists").is_empty());
    }
}

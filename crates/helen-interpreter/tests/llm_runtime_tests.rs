//! Tests for llm_runtime module — LlmResponse, MockLlmRuntime, LlmRuntime trait.

use helen_interpreter::llm_runtime::*;
use helen_interpreter::exceptions::ExceptionValue;

// ── LlmResponse tests ───────────────────────────────────────────────────

#[test]
fn llm_response_default() {
    let r = LlmResponse::default();
    assert_eq!(r.text(), "");
    assert!(r.tool_calls.is_empty());
    assert!(r.model.is_none());
}

#[test]
fn llm_response_text_some() {
    let r = LlmResponse {
        text: Some("hello".into()),
        tool_calls: vec![],
        model: Some("gpt-4".into()),
    };
    assert_eq!(r.text(), "hello");
}

#[test]
fn llm_response_text_none() {
    let r = LlmResponse {
        text: None,
        tool_calls: vec![],
        model: None,
    };
    assert_eq!(r.text(), "");
}

#[test]
fn llm_response_clone() {
    let r = LlmResponse {
        text: Some("clone".into()),
        tool_calls: vec![serde_json::json!({"id": "1"})],
        model: Some("m".into()),
    };
    let c = r.clone();
    assert_eq!(c.text(), "clone");
    assert_eq!(c.tool_calls.len(), 1);
}

#[test]
fn llm_response_debug() {
    let r = LlmResponse::default();
    let debug = format!("{:?}", r);
    assert!(debug.contains("LlmResponse"));
}

// ── MockLlmRuntime tests ────────────────────────────────────────────────

#[test]
fn mock_runtime_default() {
    let m = MockLlmRuntime::default();
    assert!(m.route_return.is_none());
    assert!(m.act_return.is_none());
    assert!(m.route_fail.is_none());
    assert!(m.act_fail.is_none());
}

#[test]
fn mock_runtime_new() {
    let m = MockLlmRuntime::new(Some("branch1".into()), None);
    assert_eq!(m.route_return.as_deref(), Some("branch1"));
    assert!(m.act_return.is_none());
}

#[test]
fn mock_runtime_with_act_text() {
    let m = MockLlmRuntime::with_act_text("response text");
    assert_eq!(m.act_return.as_ref().unwrap().text(), "response text");
    assert!(m.route_return.is_none());
}

#[test]
fn mock_runtime_route_success() {
    let m = MockLlmRuntime::new(Some("yes".into()), None);
    let result = m.route("test", &["yes".into(), "no".into()], None).unwrap();
    assert_eq!(result.as_deref(), Some("yes"));
}

#[test]
fn mock_runtime_route_none() {
    let m = MockLlmRuntime::new(None, None);
    let result = m.route("test", &["a".into()], None).unwrap();
    assert!(result.is_none());
}

#[test]
fn mock_runtime_route_fail() {
    let mut m = MockLlmRuntime::default();
    m.route_fail = Some(ExceptionValue::new("Error", "route failed".into(), None));
    let result = m.route("test", &["a".into()], None);
    assert!(result.is_err());
}

#[test]
fn mock_runtime_route_history() {
    let m = MockLlmRuntime::new(Some("x".into()), None);
    let _ = m.route("desc1", &["a".into()], Some("ctx"));
    let _ = m.route("desc2", &["b".into()], None);
    let history = m.route_history.borrow();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].0, "desc1");
    assert_eq!(history[0].1, vec!["a".to_string()]);
    assert_eq!(history[0].2.as_deref(), Some("ctx"));
    assert_eq!(history[1].0, "desc2");
    assert!(history[1].2.is_none());
}

#[test]
fn mock_runtime_act_success() {
    let m = MockLlmRuntime::with_act_text("act result");
    let result = m.act("prompt", &[], None, 0.7, 1, None, &[], None, None, false, None).unwrap();
    assert_eq!(result.text(), "act result");
}

#[test]
fn mock_runtime_act_none_returns_empty() {
    let m = MockLlmRuntime::new(None, None);
    let result = m.act("prompt", &[], None, 0.7, 1, None, &[], None, None, false, None).unwrap();
    assert_eq!(result.text(), "");
}

#[test]
fn mock_runtime_act_fail() {
    let mut m = MockLlmRuntime::default();
    m.act_fail = Some(ExceptionValue::new("Error", "act failed".into(), None));
    let result = m.act("prompt", &[], None, 0.7, 1, None, &[], None, None, false, None);
    assert!(result.is_err());
}

#[test]
fn mock_runtime_act_history() {
    let m = MockLlmRuntime::with_act_text("ok");
    let tools = vec![serde_json::json!({"name": "tool1"})];
    let _ = m.act("prompt1", &tools, None, 0.7, 1, None, &[], None, None, false, None);
    let _ = m.act("prompt2", &[], None, 0.5, 2, None, &[], None, None, false, None);
    let history = m.act_history.borrow();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].0, "prompt1");
    assert_eq!(history[0].1.len(), 1);
    assert_eq!(history[1].0, "prompt2");
    assert!(history[1].1.is_empty());
}

#[test]
fn mock_runtime_act_stream_default() {
    let m = MockLlmRuntime::with_act_text("streamed");
    let mut events: Vec<serde_json::Value> = Vec::new();
    let result = m.act_stream(
        "prompt", None, 0.7, None, &[], 1, &[], None,
        &mut |ev| { events.push(ev); true },
        false, None,
    );
    assert!(result.is_ok());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["type"], "content");
    assert_eq!(events[0]["content"], "streamed");
}

#[test]
fn mock_runtime_act_stream_empty_text_no_event() {
    let m = MockLlmRuntime::new(None, None); // act returns empty text
    let mut events: Vec<serde_json::Value> = Vec::new();
    let result = m.act_stream(
        "prompt", None, 0.7, None, &[], 1, &[], None,
        &mut |ev| { events.push(ev); true },
        false, None,
    );
    assert!(result.is_ok());
    assert!(events.is_empty()); // empty text -> no content event
}

#[test]
fn mock_runtime_act_stream_callback_returns_false() {
    let m = MockLlmRuntime::with_act_text("data");
    let mut call_count = 0;
    let result = m.act_stream(
        "prompt", None, 0.7, None, &[], 1, &[], None,
        &mut |_ev| { call_count += 1; false }, // stop after first event
        false, None,
    );
    assert!(result.is_ok());
    assert_eq!(call_count, 1);
}

#[test]
fn mock_runtime_clone() {
    let m = MockLlmRuntime::with_act_text("original");
    let c = m.clone();
    assert_eq!(c.act_return.as_ref().unwrap().text(), "original");
    // History is shared via Rc
    let _ = c.act("test", &[], None, 0.7, 1, None, &[], None, None, false, None);
    assert_eq!(m.act_history.borrow().len(), 1);
}

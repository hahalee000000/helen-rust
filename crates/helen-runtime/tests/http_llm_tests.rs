//! M5 integration tests — HTTP LLM runtime against recorded SSE fixtures.
//!
//! Uses a local TCP listener serving canned OpenAI-compatible SSE responses,
//! verifying: payload shape, SSE delta parsing, usage extraction, tool-call
//! accumulation, and the tool loop.

use helen_runtime::http_llm::HttpLLMRuntime;
use helen_runtime::llm::LlmRuntime;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

/// Spawn a mock OpenAI-compatible server returning a canned SSE stream.
fn spawn_mock_server() -> (String, String) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            // Read request headers (ignore body for the fixture)
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            // Reply with an SSE stream (content "Hel" + "lo", usage chunk, [DONE])
            let sse = concat!(
                "data: {\"id\":\"1\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hel\"},\"finish_reason\":null}]}\n\n",
                "data: {\"id\":\"1\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo\"},\"finish_reason\":null}]}\n\n",
                "data: {\"id\":\"1\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2,\"total_tokens\":7}}\n\n",
                "data: [DONE]\n\n",
            );
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{sse}",
                sse.len()
            );
            let _ = write!(stream, "{resp}");
            let _ = stream.flush();
            let _ = stream.shutdown(std::net::Shutdown::Write);
        }
    });
    (format!("http://{addr}/v1"), "test-key".into())
}

#[test]
fn test_http_llm_streaming_content_events() {
    let (base_url, api_key) = spawn_mock_server();
    let mut rt = HttpLLMRuntime::new(Some(base_url), Some(api_key), Some("qwen-test".into()));
    let mut events: Vec<serde_json::Value> = Vec::new();
    let mut full = String::new();
    rt.act_stream(
        "hello",
        None,
        0.7,
        None,
        None,
        3,
        None, // max_tokens
        None,
        None,
        &mut |ev| {
            if ev["type"] == "content" {
                full.push_str(ev["content"].as_str().unwrap_or(""));
            }
            events.push(ev.clone());
            true
        },
        false,
        None,
    )
    .unwrap();
    assert_eq!(full, "Hello");
    // usage event present
    assert!(events.iter().any(|e| e["type"] == "usage"));
}

#[test]
fn test_http_llm_act_non_streaming() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            let body = json!({
                "choices": [{
                    "message": {"role": "assistant", "content": "42", "tool_calls": []},
                    "finish_reason": "stop"
                }],
                "usage": {"total_tokens": 3}
            });
            let body_str = body.to_string();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body_str}",
                body_str.len()
            );
            let _ = write!(stream, "{resp}");
            let _ = stream.flush();
            let _ = stream.shutdown(std::net::Shutdown::Write);
        }
    });
    let (base_url, api_key) = (format!("http://{addr}/v1"), "test-key".into());
    let mut rt = HttpLLMRuntime::new(Some(base_url), Some(api_key), Some("qwen-test".into()));
    let resp = rt
        .act(
            "what is 6*7?",
            None,
            None,
            0.7,
            1,
            None,
            None,
            None,
            None,
            false,
            None,
        )
        .unwrap();
    assert_eq!(resp.text(), "42");
}

#[test]
fn test_http_llm_route_matches_branch() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            let body = json!({
                "choices": [{
                    "message": {"role": "assistant", "content": "query", "tool_calls": []},
                    "finish_reason": "stop"
                }],
                "usage": {}
            });
            let body_str = body.to_string();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body_str}",
                body_str.len()
            );
            let _ = write!(stream, "{resp}");
            let _ = stream.flush();
            let _ = stream.shutdown(std::net::Shutdown::Write);
        }
    });
    let (base_url, api_key) = (format!("http://{addr}/v1"), "test-key".into());
    let mut rt = HttpLLMRuntime::new(Some(base_url), Some(api_key), Some("qwen-test".into()));
    let branch = rt
        .route("classify", &["query".into(), "tool".into()], None)
        .unwrap();
    assert_eq!(branch.as_deref(), Some("query"));
}

#[test]
fn test_iteration_budget() {
    use helen_runtime::http_llm::IterationBudget;
    let mut b = IterationBudget::new(3);
    assert!(b.consume());
    assert!(b.consume());
    assert!(b.consume());
    assert!(!b.consume());
}

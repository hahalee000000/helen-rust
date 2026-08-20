//! LLM runtime interface and deterministic mock (Task 3.6).
//!
//! Byte-faithful port of `helen/runtime/llm_runtime.py` (the MockLLMRuntime
//! subset): `route()` backs `llm if`, `act()` backs `llm act`. M3 ships the
//! mock only — the HTTP runtime is a later milestone.

use std::cell::RefCell;
use std::rc::Rc;

/// `LLMResponse` — the result of an `act()` call.
#[derive(Debug, Clone, Default)]
pub struct LlmResponse {
    pub text: Option<String>,
    pub tool_calls: Vec<serde_json::Value>,
    pub model: Option<String>,
}

impl LlmResponse {
    pub fn text(&self) -> String {
        self.text.clone().unwrap_or_default()
    }
}

/// `LLMRuntime` — abstract interface mapping to Helen's llm statements.
/// Full signature mirrors `helen/runtime/llm_runtime.py` (M5).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub trait LlmRuntime {
    /// `llm if` — route to one of the branches by description.
    fn route(
        &self,
        description: &str,
        branches: &[String],
        context: Option<&str>,
    ) -> Result<Option<String>, ExceptionValue>;
    /// `llm act` — execute an autonomous action, returning response text.
    fn act(
        &self,
        prompt: &str,
        tools: &[serde_json::Value],
        model: Option<&str>,
        temperature: f64,
        max_turns: usize,
        max_tokens: Option<u64>,
        history: &[serde_json::Value],
        system_prompt: Option<&str>,
        dispatch_fn: Option<&dyn Fn(&str, &serde_json::Value) -> String>,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Result<LlmResponse, ExceptionValue>;
    /// Streaming `llm act` — event callback; returns false to stop.
    fn act_stream(
        &self,
        prompt: &str,
        model: Option<&str>,
        temperature: f64,
        system_prompt: Option<&str>,
        tools: &[serde_json::Value],
        max_turns: usize,
        history: &[serde_json::Value],
        dispatch_fn: Option<&dyn Fn(&str, &serde_json::Value) -> String>,
        on_event: &mut dyn FnMut(serde_json::Value) -> bool,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Result<(), ExceptionValue> {
        // Default: wrap act() and emit a single content event (Python
        // `LLMRuntime.act_stream` default implementation parity).
        let response = self.act(
            prompt,
            tools,
            model,
            temperature,
            max_turns,
            None,
            history,
            system_prompt,
            dispatch_fn,
            thinking_enabled,
            reasoning_effort,
        )?;
        if let Some(text) = &response.text {
            if !text.is_empty()
                && !on_event(serde_json::json!({"type": "content", "content": text}))
            {
                return Ok(());
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Recording / replay (v1.40) — optional; default: not supported.
    // -----------------------------------------------------------------------

    /// Whether this runtime supports recording LLM calls to a cassette.
    fn supports_recording(&self) -> bool {
        false
    }

    /// Start recording LLM calls to the given cassette path.
    fn enable_recording(&self, _cassette_path: &str) -> Result<(), String> {
        Err("Recording not supported by this LLM runtime".to_string())
    }

    /// Stop recording LLM calls.
    fn disable_recording(&self) -> Result<(), String> {
        Err("Recording not supported by this LLM runtime".to_string())
    }

    /// Switch this runtime into replay mode using a cassette.
    fn enable_replay(&self, _cassette_path: &str) -> Result<(), String> {
        Err("Replay not supported by this LLM runtime".to_string())
    }
}

/// A recorded `route()` call: (description, branch names, context).
pub type RouteHistoryEntry = (String, Vec<String>, Option<String>);
/// A recorded `act()` call: (prompt, tools).
pub type ActHistoryEntry = (String, Vec<serde_json::Value>);

/// `MockLLMRuntime` — deterministic canned responses for tests.
#[derive(Clone, Default)]
pub struct MockLlmRuntime {
    pub route_return: Option<String>,
    pub act_return: Option<LlmResponse>,
    pub route_fail: Option<ExceptionValue>,
    pub act_fail: Option<ExceptionValue>,
    pub route_history: Rc<RefCell<Vec<RouteHistoryEntry>>>,
    pub act_history: Rc<RefCell<Vec<ActHistoryEntry>>>,
    /// Cassette writer when recording is enabled (v1.40).
    pub cassette: Rc<RefCell<Option<helen_runtime::recording::CassetteWriter>>>,
    /// Cassette reader when replay mode is enabled (v1.40).
    pub replay: Rc<RefCell<Option<helen_runtime::recording::CassetteReader>>>,
    /// Current replay sequence position.
    pub replay_seq: Rc<RefCell<u64>>,
    /// Replay exhausted message (set on first miss).
    pub replay_exhausted: Rc<RefCell<Option<String>>>,
}

impl MockLlmRuntime {
    pub fn new(route_return: Option<String>, act_return: Option<LlmResponse>) -> Self {
        MockLlmRuntime {
            route_return,
            act_return,
            route_fail: None,
            act_fail: None,
            route_history: Rc::new(RefCell::new(Vec::new())),
            act_history: Rc::new(RefCell::new(Vec::new())),
            cassette: Rc::new(RefCell::new(None)),
            replay: Rc::new(RefCell::new(None)),
            replay_seq: Rc::new(RefCell::new(u64::MAX)),
            replay_exhausted: Rc::new(RefCell::new(None)),
        }
    }

    /// Shortcut: `MockLLMRuntime(act_return="ok")` — a string becomes text.
    pub fn with_act_text(text: &str) -> Self {
        MockLlmRuntime::new(
            None,
            Some(LlmResponse {
                text: Some(text.to_string()),
                ..Default::default()
            }),
        )
    }
}

impl LlmRuntime for MockLlmRuntime {
    fn route(
        &self,
        description: &str,
        branches: &[String],
        context: Option<&str>,
    ) -> Result<Option<String>, ExceptionValue> {
        self.route_history.borrow_mut().push((
            description.to_string(),
            branches.to_vec(),
            context.map(|s| s.to_string()),
        ));
        if let Some(f) = &self.route_fail {
            return Err(f.clone());
        }
        Ok(self.route_return.clone())
    }

    fn act(
        &self,
        prompt: &str,
        tools: &[serde_json::Value],
        model: Option<&str>,
        _temperature: f64,
        _max_turns: usize,
        _max_tokens: Option<u64>,
        _history: &[serde_json::Value],
        _system_prompt: Option<&str>,
        _dispatch_fn: Option<&dyn Fn(&str, &serde_json::Value) -> String>,
        _thinking_enabled: bool,
        _reasoning_effort: Option<&str>,
    ) -> Result<LlmResponse, ExceptionValue> {
        self.act_history
            .borrow_mut()
            .push((prompt.to_string(), tools.to_vec()));

        // v1.40 replay mode: serve the next cassette entry instead of the
        // canned `act_return`.
        if self.replay.borrow().is_some() {
            let mut seq = self.replay_seq.borrow_mut();
            let next = if *seq == u64::MAX { 0 } else { *seq + 1 };
            let entry = {
                let reader = self.replay.borrow();
                let r = reader.as_ref().unwrap();
                r.get_entry(next)
                    .or_else(|| r.get_next_entry(*seq))
                    .cloned()
            };
            let Some(entry) = entry else {
                if self.replay_exhausted.borrow().is_none() {
                    let total = self.replay.borrow().as_ref().map(|r| r.len()).unwrap_or(0);
                    *self.replay_exhausted.borrow_mut() = Some(format!(
                        "No more recorded interactions in cassette. Used {} of {} entries.",
                        seq.saturating_add(1),
                        total
                    ));
                }
                return Err(ExceptionValue::new(
                    "RuntimeError",
                    self.replay_exhausted.borrow().clone().unwrap_or_default(),
                    None,
                ));
            };
            *seq = entry.seq;
            let text = entry
                .response
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let tool_calls = entry
                .response
                .get("tool_calls")
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();
            return Ok(LlmResponse {
                text: Some(text),
                tool_calls,
                model: Some(entry.model.clone()),
            });
        }

        // v1.40 recording: write the request to the cassette before answering.
        if let Some(w) = self.cassette.borrow_mut().as_mut() {
            let _ = w.write_entry(
                &serde_json::json!({"messages": [{"role": "user", "content": prompt}]}),
                &serde_json::json!({"content": self.act_return.clone().and_then(|r| r.text).unwrap_or_default(), "tool_calls": []}),
                &serde_json::json!({}),
                0.0,
                None,
                model.unwrap_or(""),
                None,
                None,
            );
        }

        if let Some(f) = &self.act_fail {
            return Err(f.clone());
        }
        // Python parity: `act_return is None` -> LLMResponse(text="").
        Ok(self.act_return.clone().unwrap_or_else(|| LlmResponse {
            text: Some(String::new()),
            ..Default::default()
        }))
    }

    fn supports_recording(&self) -> bool {
        true
    }

    fn enable_recording(&self, cassette_path: &str) -> Result<(), String> {
        let w = helen_runtime::recording::CassetteWriter::new(std::path::Path::new(cassette_path))
            .map_err(|e| format!("Failed to create cassette: {e}"))?;
        *self.cassette.borrow_mut() = Some(w);
        Ok(())
    }

    fn disable_recording(&self) -> Result<(), String> {
        if let Some(w) = self.cassette.borrow_mut().as_mut() {
            w.close();
        }
        *self.cassette.borrow_mut() = None;
        Ok(())
    }

    fn enable_replay(&self, cassette_path: &str) -> Result<(), String> {
        let reader =
            helen_runtime::recording::CassetteReader::new(std::path::Path::new(cassette_path));
        if reader.is_empty() {
            return Err(format!("Cassette is empty: {cassette_path}"));
        }
        *self.replay.borrow_mut() = Some(reader);
        *self.replay_seq.borrow_mut() = u64::MAX;
        *self.replay_exhausted.borrow_mut() = None;
        Ok(())
    }
}

use crate::exceptions::ExceptionValue;

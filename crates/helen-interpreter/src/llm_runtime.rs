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
        &mut self,
        description: &str,
        branches: &[String],
        context: Option<&str>,
    ) -> Result<Option<String>, ExceptionValue>;
    /// `llm act` — execute an autonomous action, returning response text.
    fn act(
        &mut self,
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
        &mut self,
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
        &mut self,
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
        &mut self,
        prompt: &str,
        tools: &[serde_json::Value],
        _model: Option<&str>,
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
        if let Some(f) = &self.act_fail {
            return Err(f.clone());
        }
        // Python parity: `act_return is None` -> LLMResponse(text="").
        Ok(self.act_return.clone().unwrap_or_else(|| LlmResponse {
            text: Some(String::new()),
            ..Default::default()
        }))
    }
}

use crate::exceptions::ExceptionValue;

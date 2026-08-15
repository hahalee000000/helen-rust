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
    ) -> Result<LlmResponse, ExceptionValue>;
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

//! Adapter: wrap `HttpLLMRuntime` (helen-runtime) to implement
//! `helen_interpreter::llm_runtime::LlmRuntime` trait.
//!
//! The interpreter expects `helen_interpreter::llm_runtime::LlmRuntime`,
//! but `HttpLLMRuntime` implements `helen_runtime::llm::LlmRuntime`.
//! This adapter bridges the two.

use helen_interpreter::exceptions::ExceptionValue;
use helen_interpreter::llm_runtime::{LlmResponse as InterpreterLlmResponse, LlmRuntime};
use helen_runtime::http_llm::HttpLLMRuntime;
use helen_runtime::llm::LlmRuntime as RuntimeLlmRuntime;
use std::cell::RefCell;

/// Adapter that wraps `HttpLLMRuntime` to implement the interpreter's trait.
pub struct HttpLlmAdapter {
    inner: RefCell<HttpLLMRuntime>,
}

impl HttpLlmAdapter {
    pub fn new(runtime: HttpLLMRuntime) -> Self {
        Self {
            inner: RefCell::new(runtime),
        }
    }
}

impl LlmRuntime for HttpLlmAdapter {
    fn route(
        &self,
        description: &str,
        branches: &[String],
        context: Option<&str>,
    ) -> Result<Option<String>, ExceptionValue> {
        let mut runtime = self.inner.borrow_mut();
        runtime
            .route(description, branches, context)
            .map_err(|e| ExceptionValue::new("RuntimeError", e, None))
    }

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
    ) -> Result<InterpreterLlmResponse, ExceptionValue> {
        let mut runtime = self.inner.borrow_mut();
        let response = runtime
            .act(
                prompt,
                Some(tools),
                model,
                temperature,
                max_turns,
                max_tokens,
                Some(history),
                system_prompt,
                dispatch_fn,
                thinking_enabled,
                reasoning_effort,
            )
            .map_err(|e| ExceptionValue::new("RuntimeError", e, None))?;
        // Convert runtime LlmResponse to interpreter LlmResponse
        Ok(InterpreterLlmResponse {
            text: response.text,
            tool_calls: response.tool_calls,
            model: response.model,
        })
    }

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
        let mut runtime = self.inner.borrow_mut();
        runtime
            .act_stream(
                prompt,
                model,
                temperature,
                system_prompt,
                Some(tools),
                max_turns,
                None,
                Some(history),
                dispatch_fn,
                on_event,
                thinking_enabled,
                reasoning_effort,
            )
            .map_err(|e| ExceptionValue::new("RuntimeError", e, None))
    }
}

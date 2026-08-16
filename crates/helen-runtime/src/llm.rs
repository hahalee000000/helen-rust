//! LLM runtime interface (Task 5.1) — byte-faithful port of
//! `helen/runtime/llm_runtime.py` (the abstract `LLMRuntime` + `LLMResponse`).
//!
//! The deterministic `MockLlmRuntime` lives in `helen-interpreter` (it needs
//! `ExceptionValue` from that crate). This crate defines the production-facing
//! interface and the HTTP implementation (`HttpLLMRuntime`).

/// `LLMResponse` — the result of an `act()` call (Python dataclass).
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

/// `LlmRuntime` — abstract interface mapping to Helen's llm statements.
/// Full signature mirrors `helen/runtime/llm_runtime.py`.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub trait LlmRuntime: Send + Sync {
    /// `llm if` — route input to one of the given branches.
    fn route(
        &mut self,
        description: &str,
        branches: &[String],
        context: Option<&str>,
    ) -> Result<Option<String>, String>;

    /// `llm act` — execute an autonomous action (sync).
    /// `dispatch_fn` is optional custom tool dispatch (name, args_json) -> result.
    fn act(
        &mut self,
        prompt: &str,
        tools: Option<&[serde_json::Value]>,
        model: Option<&str>,
        temperature: f64,
        max_turns: usize,
        max_tokens: Option<u64>,
        history: Option<&[serde_json::Value]>,
        system_prompt: Option<&str>,
        dispatch_fn: Option<&dyn Fn(&str, &serde_json::Value) -> String>,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Result<LlmResponse, String>;

    /// Stream `llm act` — returns a callback-driven stream. The closure is
    /// invoked with event dicts (`{"type": "content"|"tool_call"|...}`).
    fn act_stream(
        &mut self,
        prompt: &str,
        model: Option<&str>,
        temperature: f64,
        system_prompt: Option<&str>,
        tools: Option<&[serde_json::Value]>,
        max_turns: usize,
        history: Option<&[serde_json::Value]>,
        dispatch_fn: Option<&dyn Fn(&str, &serde_json::Value) -> String>,
        on_event: &mut dyn FnMut(serde_json::Value) -> bool,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Result<(), String>;

    // -----------------------------------------------------------------------
    // Recording / replay (optional — default: not supported)
    // -----------------------------------------------------------------------

    /// Whether this runtime supports recording LLM calls to a cassette.
    fn supports_recording(&self) -> bool {
        false
    }

    /// Start recording LLM calls to the given cassette path.
    fn enable_recording(&mut self, _cassette_path: &str) -> Result<(), String> {
        Err("Recording not supported by this LLM runtime".to_string())
    }

    /// Stop recording LLM calls.
    fn disable_recording(&mut self) -> Result<(), String> {
        Err("Recording not supported by this LLM runtime".to_string())
    }
}

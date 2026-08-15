//! helen-runtime — M5: LLM runtime (providers, HTTP client, config, prompt
//! building, model capabilities, token counting, probe).
//!
//! Byte-faithful port of `helen/runtime/{llm_runtime,provider_protocol,
//! config,prompt_builder,model_capabilities,token_utils,http_llm,probe}.py`.
//! The interpreter keeps its own `MockLlmRuntime` (deterministic tests); this
//! crate provides the production runtime used when the interpreter is built
//! with real-provider support.

pub mod calc;
pub mod config;
pub mod fuzzy_match;
pub mod http_llm;
pub mod llm;
pub mod model_caps;
pub mod prompt;
pub mod provider;
pub mod skills;
pub mod token;
pub mod tools;

pub use tools::{dispatch_tool, get_tool_schemas, tools_dispatch};

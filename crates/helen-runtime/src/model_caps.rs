//! Model-level capability detection (Task 5.4) — port of
//! `helen/runtime/model_capabilities.py`.
//!
//! While PlatformProtocol handles protocol FORMAT differences (base_url),
//! ModelCapabilities handles feature AVAILABILITY differences (model_id).

/// Model-level feature detection (Python dataclass parity).
#[derive(Debug, Clone)]
pub struct ModelCapabilities {
    pub supports_thinking: bool,
    pub thinking_enabled_by_default: bool,
    pub forced_thinking: bool,
    pub supports_tool_choice_required: bool,
    pub supports_tool_choice_none: bool,
    pub supports_parallel_tools: bool,
    /// "incremental" | "cumulative" (MiniMax) | "mutually_exclusive" (DeepSeek)
    pub reasoning_content_streaming: &'static str,
    pub has_encrypted_content: bool,
    pub has_reasoning_details: bool,
    pub default_temperature: f64,
    pub default_top_p: f64,
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        ModelCapabilities {
            supports_thinking: true,
            thinking_enabled_by_default: false,
            forced_thinking: false,
            supports_tool_choice_required: true,
            supports_tool_choice_none: true,
            supports_parallel_tools: true,
            reasoning_content_streaming: "incremental",
            has_encrypted_content: false,
            has_reasoning_details: false,
            default_temperature: 1.0,
            default_top_p: 1.0,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn caps(
    supports_thinking: bool,
    thinking_enabled_by_default: bool,
    forced_thinking: bool,
    tool_choice_required: bool,
    tool_choice_none: bool,
    reasoning_streaming: &'static str,
    encrypted: bool,
    reasoning_details: bool,
    temp: f64,
) -> ModelCapabilities {
    ModelCapabilities {
        supports_thinking,
        thinking_enabled_by_default,
        forced_thinking,
        supports_tool_choice_required: tool_choice_required,
        supports_tool_choice_none: tool_choice_none,
        supports_parallel_tools: true,
        reasoning_content_streaming: reasoning_streaming,
        has_encrypted_content: encrypted,
        has_reasoning_details: reasoning_details,
        default_temperature: temp,
        default_top_p: 1.0,
    }
}

/// Model capability registry (Python `_MODEL_CAPABILITIES` parity).
pub fn model_capabilities() -> Vec<(&'static str, ModelCapabilities)> {
    let mut v: Vec<(&'static str, ModelCapabilities)> = Vec::new();
    let mut push = |name: &'static str, c: ModelCapabilities| v.push((name, c));

    // --- DashScope (Qwen) ---
    push(
        "qwen3-max",
        caps(
            true,
            false,
            false,
            true,
            true,
            "incremental",
            false,
            false,
            1.0,
        ),
    );
    push(
        "qwen3.7-plus",
        caps(
            true,
            false,
            false,
            true,
            true,
            "incremental",
            false,
            false,
            1.0,
        ),
    );
    push(
        "qwen3.8-max",
        caps(
            true,
            false,
            false,
            true,
            true,
            "incremental",
            false,
            false,
            1.0,
        ),
    );
    push(
        "qwen-max",
        caps(
            false,
            false,
            false,
            true,
            true,
            "incremental",
            false,
            false,
            1.0,
        ),
    );
    push(
        "qwen-plus",
        caps(
            false,
            false,
            false,
            true,
            true,
            "incremental",
            false,
            false,
            1.0,
        ),
    );
    push(
        "qwen-turbo",
        caps(
            false,
            false,
            false,
            true,
            true,
            "incremental",
            false,
            false,
            1.0,
        ),
    );

    // --- Zhipu (GLM) — only "auto" tool_choice ---
    push(
        "glm-5.2",
        caps(
            true,
            false,
            false,
            false,
            false,
            "incremental",
            false,
            false,
            1.0,
        ),
    );
    push(
        "glm-5.1",
        caps(
            true,
            false,
            false,
            false,
            false,
            "incremental",
            false,
            false,
            1.0,
        ),
    );
    push(
        "glm-5",
        caps(
            true,
            false,
            false,
            false,
            false,
            "incremental",
            false,
            false,
            1.0,
        ),
    );
    push(
        "glm-4.7",
        caps(
            true,
            false,
            true,
            false,
            false,
            "incremental",
            false,
            false,
            1.0,
        ),
    );
    push(
        "glm-4.6",
        caps(
            true,
            false,
            false,
            false,
            false,
            "incremental",
            false,
            false,
            1.0,
        ),
    );
    push(
        "glm-4.5",
        caps(
            true,
            false,
            false,
            false,
            false,
            "incremental",
            false,
            false,
            0.6,
        ),
    );

    // --- DeepSeek — mutually_exclusive reasoning streaming ---
    push(
        "deepseek-v4-flash",
        caps(
            true,
            false,
            false,
            true,
            true,
            "mutually_exclusive",
            false,
            false,
            1.0,
        ),
    );
    push(
        "deepseek-v4-pro",
        caps(
            true,
            false,
            false,
            true,
            true,
            "mutually_exclusive",
            false,
            false,
            1.0,
        ),
    );
    push(
        "deepseek-reasoner",
        caps(
            true,
            false,
            true,
            true,
            true,
            "mutually_exclusive",
            false,
            false,
            1.0,
        ),
    );
    push(
        "deepseek-chat",
        caps(
            false,
            false,
            false,
            true,
            true,
            "incremental",
            false,
            false,
            1.0,
        ),
    );

    // --- MiniMax — cumulative reasoning_details ---
    push(
        "MiniMax-M3",
        caps(
            true,
            false,
            false,
            true,
            true,
            "cumulative",
            false,
            true,
            1.0,
        ),
    );
    push(
        "MiniMax-M2.7",
        caps(
            true,
            false,
            true,
            true,
            true,
            "cumulative",
            false,
            true,
            1.0,
        ),
    );
    push(
        "MiniMax-M2.5",
        caps(
            true,
            false,
            true,
            true,
            true,
            "cumulative",
            false,
            true,
            1.0,
        ),
    );
    push(
        "MiniMax-M2.1",
        caps(
            true,
            false,
            true,
            true,
            true,
            "cumulative",
            false,
            true,
            1.0,
        ),
    );

    // --- Kimi/Moonshot ---
    push(
        "kimi-k3",
        caps(
            true,
            true,
            false,
            true,
            true,
            "incremental",
            false,
            false,
            1.0,
        ),
    );
    push(
        "kimi-k2.7-code",
        caps(
            true,
            false,
            true,
            true,
            true,
            "incremental",
            false,
            false,
            1.0,
        ),
    );
    push(
        "kimi-k2.6",
        caps(
            true,
            true,
            false,
            true,
            true,
            "incremental",
            false,
            false,
            1.0,
        ),
    );
    push(
        "moonshot-v1-8k",
        caps(
            false,
            false,
            false,
            true,
            true,
            "incremental",
            false,
            false,
            0.0,
        ),
    );
    push(
        "moonshot-v1-32k",
        caps(
            false,
            false,
            false,
            true,
            true,
            "incremental",
            false,
            false,
            0.0,
        ),
    );
    push(
        "moonshot-v1-128k",
        caps(
            false,
            false,
            false,
            true,
            true,
            "incremental",
            false,
            false,
            0.0,
        ),
    );

    // --- Doubao (Volcengine) — encrypted content ---
    push(
        "doubao-seed-2.1-pro",
        caps(
            true,
            false,
            false,
            true,
            true,
            "incremental",
            true,
            false,
            1.0,
        ),
    );
    push(
        "doubao-seed-1.6",
        caps(
            true,
            false,
            false,
            true,
            true,
            "incremental",
            true,
            false,
            1.0,
        ),
    );
    push(
        "doubao-seed-1.6-thinking",
        caps(
            true,
            false,
            true,
            true,
            true,
            "incremental",
            true,
            false,
            1.0,
        ),
    );
    push(
        "doubao-1.5-pro-256k",
        caps(
            true,
            false,
            false,
            true,
            true,
            "incremental",
            false,
            false,
            1.0,
        ),
    );
    push(
        "doubao-pro-128k",
        caps(
            false,
            false,
            false,
            true,
            true,
            "incremental",
            false,
            false,
            1.0,
        ),
    );
    push(
        "doubao-pro-32k",
        caps(
            false,
            false,
            false,
            true,
            true,
            "incremental",
            false,
            false,
            1.0,
        ),
    );
    push(
        "doubao-lite-128k",
        caps(
            false,
            false,
            false,
            true,
            true,
            "incremental",
            false,
            false,
            1.0,
        ),
    );

    v
}

/// Get capabilities for a specific model. Lookup order:
/// 1. Exact match; 2. Prefix match (e.g. "qwen3-max-2024" -> "qwen3-max");
/// 3. Default OpenAI-compatible capabilities.
pub fn get_model_capabilities(model_id: Option<&str>) -> ModelCapabilities {
    let Some(model_id) = model_id else {
        return ModelCapabilities::default();
    };
    if model_id.is_empty() {
        return ModelCapabilities::default();
    }
    for (registered, c) in model_capabilities() {
        if registered == model_id {
            return c;
        }
    }
    // Prefix match
    for (registered, c) in model_capabilities() {
        if model_id.starts_with(registered) {
            return c;
        }
    }
    ModelCapabilities::default()
}

/// List registered model ids.
pub fn list_registered_models() -> Vec<&'static str> {
    model_capabilities().into_iter().map(|(n, _)| n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let c = get_model_capabilities(Some("deepseek-reasoner"));
        assert!(c.forced_thinking);
        assert_eq!(c.reasoning_content_streaming, "mutually_exclusive");
    }

    #[test]
    fn test_prefix_match() {
        let c = get_model_capabilities(Some("qwen3-max-2025-01-01"));
        assert!(c.supports_thinking);
    }

    #[test]
    fn test_default_fallback() {
        let c = get_model_capabilities(Some("unknown-model"));
        assert!(c.supports_thinking);
        assert_eq!(c.reasoning_content_streaming, "incremental");
    }

    #[test]
    fn test_minimax_cumulative() {
        let c = get_model_capabilities(Some("MiniMax-M3"));
        assert!(c.has_reasoning_details);
        assert_eq!(c.reasoning_content_streaming, "cumulative");
    }

    #[test]
    fn test_glm_tool_choice_restricted() {
        let c = get_model_capabilities(Some("glm-4.6"));
        assert!(!c.supports_tool_choice_required);
        assert!(!c.supports_tool_choice_none);
    }
}

"""Custom provider with thinking + error parsing overrides.

This is the most common pattern: a provider that uses a non-standard thinking
field format AND has a custom error envelope.

Scenario (fictitious): "PrivateLLM" uses:
  - `x_thinking` field (instead of OpenAI's standard or DeepSeek's thinking.type)
  - `x_thinking_budget` for effort-level control
  - Custom error envelope: `{ "error_code": "...", "error_msg": "..." }`
    instead of OpenAI's `{ "error": { "message": "..." } }`
  - Unique wording for context overflow: "input too long for model context"

Install location:  ~/.helen/providers/private_llm.py
Config usage:      protocol: "private_llm"
"""
from helen.runtime.provider_protocol import PlatformProtocol


class PrivateLLMProtocol(PlatformProtocol):
    name = "private_llm"

    # -------------------------------------------------------------------------
    # Override 1: thinking field
    # -------------------------------------------------------------------------
    #
    # PrivateLLM uses `x_thinking: true` to enable thinking mode,
    # and `x_thinking_budget: <int>` for effort levels.

    def build_request_payload(
        self,
        base_payload,
        *,
        model_id,
        thinking_enabled=False,
        reasoning_effort=None,
    ):
        if thinking_enabled:
            base_payload["x_thinking"] = True
            if reasoning_effort:
                # Map Helen's effort levels to PrivateLLM's token budget
                budget_map = {
                    "low": 512,
                    "medium": 2048,
                    "high": 8192,
                    "max": 16384,
                }
                base_payload["x_thinking_budget"] = budget_map.get(
                    reasoning_effort, 2048
                )
        return base_payload

    # -------------------------------------------------------------------------
    # Override 2: non-standard error envelope
    # -------------------------------------------------------------------------
    #
    # PrivateLLM returns errors as:
    #   { "error_code": "E1001", "error_msg": "Invalid request" }
    # instead of OpenAI's:
    #   { "error": { "message": "Invalid request" } }

    def parse_error(self, status_code, response_body):
        if isinstance(response_body, dict):
            # Try the custom envelope first
            error_msg = response_body.get("error_msg")
            if error_msg:
                error_code = response_body.get("error_code", "")
                return f"[{error_code}] {error_msg}" if error_code else error_msg
            # Fall back to standard OpenAI shape in case some endpoints use it
            error = response_body.get("error", {})
            if isinstance(error, dict):
                return error.get("message", str(response_body))
        return str(response_body)

    # -------------------------------------------------------------------------
    # Override 3: unique context overflow wording
    # -------------------------------------------------------------------------
    #
    # PrivateLLM says "input too long for model context" instead of the
    # six standard markers that PlatformProtocol matches by default.

    def is_context_overflow_error(self, error_msg):
        if "input too long for model context" in error_msg.lower():
            return True
        # Also keep the default markers as a fallback
        return super().is_context_overflow_error(error_msg)


# -----------------------------------------------------------------------------
# Verification
# -----------------------------------------------------------------------------
#
# After saving this file:
#
#     $ helen provider list
#     • private_llm  (/home/you/.helen/providers/private_llm.py)
#
#     $ helen repl
#     >>> from helen.runtime.provider_protocol import detect_protocol
#     >>> p = detect_protocol("https://private-llm.example.com/v1",
#     ...                     protocol_name="private_llm")
#     >>> p.name
#     'private_llm'
#
#     >>> payload = p.build_request_payload(
#     ...     {"model": "m", "messages": []},
#     ...     model_id="m",
#     ...     thinking_enabled=True,
#     ...     reasoning_effort="high",
#     ... )
#     >>> payload["x_thinking"], payload["x_thinking_budget"]
#     (True, 8192)
#
#     >>> p.parse_error(400, {"error_code": "E1001", "error_msg": "Bad request"})
#     '[E1001] Bad request'
#
#     >>> p.is_context_overflow_error("input too long for model context")
#     True

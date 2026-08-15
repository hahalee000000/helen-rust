"""Custom provider that overrides streaming usage extraction.

This is a focused example showing how to handle a provider that places
streaming usage at `choices[0].usage` instead of the standard top-level
`chunk["usage"]` (same pattern as Kimi/Moonshot).

Scenario (fictitious): "StreamLLM" sends SSE chunks shaped like:

    data: {"choices": [{"delta": {"content": "Hi"}, "usage": {"prompt_tokens": 10, ...}}]}

instead of OpenAI's:

    data: {"choices": [{"delta": {"content": "Hi"}}], "usage": {"prompt_tokens": 10, ...}}

Install location:  ~/.helen/providers/streaming_usage.py
Config usage:      protocol: "streaming_usage_example"
"""
from helen.runtime.provider_protocol import PlatformProtocol


class StreamingUsageProtocol(PlatformProtocol):
    name = "streaming_usage_example"

    # -------------------------------------------------------------------------
    # Override: extract_streaming_usage
    # -------------------------------------------------------------------------
    #
    # Standard OpenAI places usage at chunk["usage"]. This provider places it
    # at chunk["choices"][0]["usage"] — same pattern as Kimi/Moonshot.
    #
    # The override tries the custom location first, then falls back to the
    # standard location for robustness.

    def extract_streaming_usage(self, chunk):
        # Custom location first
        choices = chunk.get("choices", [])
        if choices and isinstance(choices, list):
            first_choice = choices[0]
            if isinstance(first_choice, dict):
                usage = first_choice.get("usage")
                if usage:
                    return usage
        # Fall back to standard location
        return chunk.get("usage")


# -----------------------------------------------------------------------------
# Verification
# -----------------------------------------------------------------------------
#
#     >>> p = StreamingUsageProtocol()
#
#     >>> # Custom location works
#     >>> chunk_custom = {
#     ...     "choices": [{
#     ...         "usage": {"prompt_tokens": 10, "completion_tokens": 20}
#     ...     }]
#     ... }
#     >>> p.extract_streaming_usage(chunk_custom)
#     {'prompt_tokens': 10, 'completion_tokens': 20}
#
#     >>> # Standard location still works as fallback
#     >>> chunk_standard = {
#     ...     "choices": [],
#     ...     "usage": {"prompt_tokens": 10, "completion_tokens": 20}
#     ... }
#     >>> p.extract_streaming_usage(chunk_standard)
#     {'prompt_tokens': 10, 'completion_tokens': 20}
#
#     >>> # No usage anywhere → None
#     >>> p.extract_streaming_usage({"choices": []})
#     None

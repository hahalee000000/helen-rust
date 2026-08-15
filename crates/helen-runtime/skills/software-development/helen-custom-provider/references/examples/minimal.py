"""Minimal custom provider — zero overrides.

Use this template when the target provider is 100% OpenAI-compatible and you
just want a stable `protocol:` name in config.yaml.

Install location:  ~/.helen/providers/minimal.py
Config usage:      protocol: "minimal_example"
"""
from helen.runtime.provider_protocol import PlatformProtocol


class MinimalExampleProtocol(PlatformProtocol):
    """OpenAI-compatible provider, no quirks.

    All methods inherit the default OpenAI implementation from PlatformProtocol.
    The only customization is the `name` attribute, which must be set explicitly
    on every subclass.
    """

    name = "minimal_example"

    # That's it. No methods overridden.
    #
    # Save this file to ~/.helen/providers/minimal_example.py, then in
    # ~/.helen/config.yaml:
    #
    #     llm:
    #       base_url: "https://your-openai-compatible-provider.com/v1"
    #       api_key: "sk-..."
    #       model: "your-model"
    #       protocol: "minimal_example"
    #
    # Verify:
    #     $ helen provider list
    #     • minimal_example  (/home/you/.helen/providers/minimal_example.py)

# helen-rust

Helen language Rust runtime — Python bridge (M11).

Import and call Helen agents/functions from Python:

```python
import helen_rust                      # auto-installs the .helen import hook
from translator import SumAgent        # translator.helen in sys.path

agent = SumAgent()
agent(10, 20)                          # 30
await agent.async_call(10, 20)         # 30 (executor thread)

# Direct API
from helen_rust import load_agent
native = load_agent("translator.helen", "SumAgent")
native(a=15, b=25)                     # 40
```

Also exposes `parse_check(source) -> list[str]` (semantic error codes),
`eval_helen(source, globals)`, and `helen_agent`/`helen_module` decorators.

Built with maturin: `maturin develop --release` (development) or
`pip install helen-rust` (wheel).

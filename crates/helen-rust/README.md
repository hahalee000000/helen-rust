# helen-rust

Helen 语言的 Rust 实现 — 完整的 Helen 语言运行时和 CLI 工具。

## 特性

- ✅ **完整的 Helen 语言实现** — 词法分析、语法分析、语义分析、解释执行
- ✅ **378+ 内置函数** — 包含 374 个中文别名，完全兼容
- ✅ **LLM 运行时** — 支持 AI Agent、工具调用、技能系统
- ✅ **Python FFI** — Helen 可以调用 Python 代码
- ✅ **Python Bridge** — Python 可以调用 Helen agents
- ✅ **Language Server** — LSP 支持，IDE 集成
- ✅ **Web UI** — 基于 Axum + React 的 Web 界面

## 安装

### 1. 安装 CLI (必需)

使用 Cargo 安装 Helen CLI：

```bash
cargo install helen-rust
```

验证安装：

```bash
helen --version
# helen 1.46.22
```

### 2. 安装 Python Bridge (可选)

如果你想在 Python 中使用 Helen agents，需要安装 Python bridge：

```bash
pip install helen-rust
```

验证安装：

```bash
python -c "import helen_rust; print('✅ helen-rust installed')"
```

**注意**: Python bridge 是可选的。如果你只需要 CLI 功能，可以跳过这一步。

## 使用

### CLI 使用

#### 运行 Helen 程序

```bash
# 运行 .helen 文件
helen run main.helen

# 带参数运行
helen run main.helen arg1 arg2
```

#### 检查语法

```bash
# 检查语法错误
helen check main.helen

# 详细输出
helen check main.helen --verbose
```

#### REPL 交互

```bash
# 启动 REPL
helen repl

# 执行代码
> let x = 10
> let y = 20
> x + y
30
```

#### 运行测试

```bash
# 运行测试文件
helen test test_main.helen

# 运行特定测试
helen test test_main.helen --filter test_addition
```

### Python 使用

#### 导入 Helen agents

```python
import helen_rust
from translator import SumAgent  # translator.helen 在当前目录

# 创建 agent 实例
agent = SumAgent()

# 调用 agent
result = agent(10, 20)
print(result)  # 30
```

#### 异步调用

```python
import asyncio
from helen_rust import load_agent

async def main():
    agent = load_agent("translator.helen", "SumAgent")
    result = await agent.async_call(10, 20)
    print(result)  # 30

asyncio.run(main())
```

#### 直接 API

```python
from helen_rust import load_agent, parse_check, eval_helen

# 加载 agent
agent = load_agent("translator.helen", "SumAgent")
result = agent(a=15, b=25)
print(result)  # 40

# 语义检查
errors = parse_check("let x = 10")
print(errors)  # [] (无错误)

# 执行代码
result = eval_helen("1 + 2 + 3", {})
print(result)  # 6
```

#### 装饰器

```python
from helen_rust import helen_agent, helen_module

# 标记为 Helen agent
@helen_agent
class MyAgent:
    def __call__(self, x, y):
        return x + y

# 标记为 Helen 模块
@helen_module
class MyModule:
    def helper(self):
        return "help"
```

## 完整安装示例

### 场景 1: 只需要 CLI

```bash
# 安装 CLI
cargo install helen-rust

# 使用
helen run main.helen
```

### 场景 2: CLI + Python 集成

```bash
# 1. 安装 CLI
cargo install helen-rust

# 2. 安装 Python bridge
pip install helen-rust

# 3. 在 Python 中使用
python -c "
import helen_rust
from translator import SumAgent
agent = SumAgent()
print(agent(10, 20))  # 30
"
```

### 场景 3: 开发环境

```bash
# 1. 安装 CLI
cargo install helen-rust

# 2. 安装 Python bridge (开发模式)
pip install maturin pytest
cd /path/to/helen-rust/crates/helen-python-bridge
maturin develop --release

# 3. 运行测试
cargo test
python -m pytest tests/
```

## 系统要求

### CLI

- Rust 1.70+ (用于编译)
- Linux, macOS, 或 Windows

### Python Bridge

- Python 3.9+
- Linux, macOS, 或 Windows
- 已安装 helen-rust CLI (可选，但推荐)

## 文档

- [语言参考](../../docs/language.md) — Helen 语言完整参考
- [标准库](../../docs/stdlib.md) — 378+ 内置函数
- [Python Bridge](../helen-python-bridge/README.md) — Python 集成指南
- [架构文档](../../wiki/rust/) — Rust 实现细节

## 示例

### Helen 程序

```helen
// main.helen
agent SumAgent(a: int, b: int): int {
    description "计算两个数的和"
    
    main {
        return a + b
    }
}

// 使用
let agent = SumAgent(10, 20)
println(agent)  // 30
```

### Python 调用

```python
# app.py
import helen_rust
from main import SumAgent

agent = SumAgent()
result = agent(10, 20)
print(f"10 + 20 = {result}")  # 10 + 20 = 30
```

运行：

```bash
python app.py
```

## 故障排除

### 问题: `helen: command not found`

**解决方案**: 确保 `~/.cargo/bin` 在 PATH 中

```bash
export PATH="$HOME/.cargo/bin:$PATH"
# 添加到 ~/.bashrc 或 ~/.zshrc 以永久生效
```

### 问题: `ModuleNotFoundError: No module named 'helen_rust'`

**解决方案**: 安装 Python bridge

```bash
pip install helen-rust
```

### 问题: Python bridge 构建失败

**解决方案**: 安装 maturin 和 Python 开发头文件

```bash
# 安装 maturin
cargo install maturin

# Ubuntu/Debian
sudo apt-get install python3-dev

# macOS (通常已包含)
```

## 贡献

欢迎贡献！请参阅 [贡献指南](../../CONTRIBUTING.md)。

## 许可证

MIT License

## 相关链接

- [Helen 语言](https://github.com/hahalee000000/helen) — 原始 Python 实现
- [问题反馈](https://github.com/your-repo/helen-rust/issues)
- [讨论区](https://github.com/your-repo/helen-rust/discussions)

---

**版本**: 1.46.22  
**最后更新**: 2026-11-13

# 长期方案：架构重构详解

## 目标

让 `cargo install helen-rust` 时自动包含 Python bridge 的核心功能，同时保持 Python 用户可以从 PyPI 安装。

---

## 当前架构 vs 目标架构

### 当前架构

```
┌─────────────────────────────────────────────────────────┐
│                    helen-rust (CLI)                      │
│  cargo install helen-rust                                │
│  ├── helen-core                                          │
│  ├── helen-parser                                        │
│  ├── helen-semantic                                      │
│  ├── helen-interpreter                                   │
│  ├── helen-runtime                                       │
│  └── helen-lsp                                           │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│              helen-python-bridge (独立)                  │
│  pip install helen-rust                                  │
│  ├── helen-core (重复依赖)                               │
│  ├── helen-parser (重复依赖)                             │
│  ├── helen-semantic (重复依赖)                           │
│  ├── helen-interpreter (重复依赖)                        │
│  ├── helen-ffi (重复依赖)                                │
│  └── PyO3 绑定层                                         │
└─────────────────────────────────────────────────────────┘

问题：
❌ 两个独立的包，需要分别安装
❌ 依赖重复编译
❌ Rust 用户无法使用 Python bridge 的功能
```

### 目标架构

```
┌─────────────────────────────────────────────────────────┐
│              helen-bridge-core (新 crate)                │
│  cargo install helen-bridge-core                         │
│  ├── 核心桥接逻辑 (不依赖 PyO3)                          │
│  ├── 可被其他 Rust crate 依赖                            │
│  └── 发布到 crates.io                                    │
└─────────────────────────────────────────────────────────┘
         ↓                    ↓
    ┌────────┴────────┐  ┌────┴────────────────────┐
    │                 │  │                          │
┌───▼──────────┐  ┌───▼──────────────────────┐
│ helen-rust   │  │ helen-python-bridge      │
│ (CLI)        │  │ (Python 扩展)            │
│              │  │                          │
│ 依赖         │  │ 依赖                     │
│ helen-bridge │  │ helen-bridge-core        │
│ -core        │  │ + PyO3 绑定层            │
│              │  │                          │
│ cargo install│  │ pip install              │
│ helen-rust   │  │ helen-rust               │
└──────────────┘  └──────────────────────────┘

优势：
✅ cargo install helen-rust 自动包含 bridge-core
✅ Rust 用户可以直接使用桥接功能
✅ Python 用户从 PyPI 安装
✅ 核心逻辑只编译一次
```

---

## 详细设计

### 1. helen-bridge-core (新 crate)

**位置**: `crates/helen-bridge-core/`

**职责**:
- 提供与 helen-python-bridge 相同的核心功能
- 不依赖 PyO3 (纯 Rust)
- 可以被其他 Rust crate 依赖
- 发布到 crates.io

**核心 API**:

```rust
// crates/helen-bridge-core/src/lib.rs

pub struct BridgeCore {
    // 内部状态
}

impl BridgeCore {
    /// 加载 Helen agent
    pub fn load_agent(file: &str, name: &str) -> Result<AgentHandle, BridgeError> {
        // 实现逻辑
    }
    
    /// 加载 Helen function
    pub fn load_function(file: &str, name: &str) -> Result<FunctionHandle, BridgeError> {
        // 实现逻辑
    }
    
    /// 语义检查
    pub fn parse_check(source: &str) -> Vec<String> {
        // 实现逻辑
    }
    
    /// 执行 Helen 代码
    pub fn eval_helen(source: &str, globals: HashMap<String, Value>) -> Result<Value, BridgeError> {
        // 实现逻辑
    }
    
    /// 描述文件内容
    pub fn describe_file(file: &str) -> Result<FileDescription, BridgeError> {
        // 实现逻辑
    }
}

/// Agent 句柄
pub struct AgentHandle {
    // 内部状态
}

impl AgentHandle {
    /// 调用 agent
    pub fn call(&self, args: Vec<Value>, kwargs: HashMap<String, Value>) -> Result<Value, BridgeError> {
        // 实现逻辑
    }
}

/// Function 句柄
pub struct FunctionHandle {
    // 内部状态
}

impl FunctionHandle {
    /// 调用 function
    pub fn call(&self, args: Vec<Value>, kwargs: HashMap<String, Value>) -> Result<Value, BridgeError> {
        // 实现逻辑
    }
}

/// 错误类型
#[derive(Debug)]
pub enum BridgeError {
    ParseError(String),
    LoadError(String),
    RuntimeError(String),
    TypeError(String),
    // ...
}

/// 值类型 (与 Helen 的 Value 对应)
#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Value>),
    Map(HashMap<String, Value>),
    // ...
}
```

**Cargo.toml**:

```toml
[package]
name = "helen-bridge-core"
version = "0.1.5"
edition = "2021"
description = "Core bridge logic for Helen language (Rust→Helen integration)"
license = "MIT"
repository = "https://github.com/your-repo/helen-rust"

[dependencies]
helen-core = { version = "0.1.5", path = "../helen-core" }
helen-parser = { version = "0.1.5", path = "../helen-parser" }
helen-semantic = { version = "0.1.5", path = "../helen-semantic" }
helen-interpreter = { version = "0.1.5", path = "../helen-interpreter" }
```

**注意**: 不依赖 PyO3，纯 Rust 实现。

---

### 2. helen-python-bridge (重构)

**位置**: `crates/helen-python-bridge/` (保持不变)

**职责**:
- 提供 PyO3 绑定层
- 将 helen-bridge-core 的 API 暴露给 Python
- 发布到 PyPI (通过 maturin)

**重构后的代码**:

```rust
// crates/helen-python-bridge/src/lib.rs

use pyo3::prelude::*;
use helen_bridge_core::{BridgeCore, BridgeError, Value as CoreValue};

/// Python 包装器
#[pyclass]
pub struct PyAgent {
    inner: helen_bridge_core::AgentHandle,
}

#[pymethods]
impl PyAgent {
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__(
        &self,
        args: &PyTuple,
        kwargs: Option<&PyDict>,
    ) -> PyResult<PyObject> {
        // 转换 Python 参数为 CoreValue
        let core_args = convert_py_args(args)?;
        let core_kwargs = convert_py_kwargs(kwargs)?;
        
        // 调用核心逻辑
        let result = self.inner.call(core_args, core_kwargs)
            .map_err(|e| BridgeError::to_pyerr(e))?;
        
        // 转换结果为 Python 对象
        convert_to_py(result)
    }
}

/// 加载 agent (Python API)
#[pyfunction]
fn load_agent(file: &str, name: &str) -> PyResult<PyAgent> {
    let inner = BridgeCore::load_agent(file, name)
        .map_err(|e| BridgeError::to_pyerr(e))?;
    Ok(PyAgent { inner })
}

/// 加载 function (Python API)
#[pyfunction]
fn load_function(file: &str, name: &str) -> PyResult<PyFunction> {
    let inner = BridgeCore::load_function(file, name)
        .map_err(|e| BridgeError::to_pyerr(e))?;
    Ok(PyFunction { inner })
}

// ... 其他 API

#[pymodule]
fn helen_python_bridge(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(load_agent, m)?)?;
    m.add_function(wrap_pyfunction!(load_function, m)?)?;
    m.add_class::<PyAgent>()?;
    m.add_class::<PyFunction>()?;
    Ok(())
}
```

**Cargo.toml** (重构后):

```toml
[package]
name = "helen-python-bridge"
version = "0.1.5"
edition = "2021"
publish = false  # 仍然不发布到 crates.io

[lib]
name = "helen_python_bridge"
crate-type = ["cdylib", "rlib"]

[dependencies]
pyo3 = { version = "0.23", features = ["auto-initialize"] }
helen-bridge-core = { version = "0.1.5", path = "../helen-bridge-core" }  # ← 依赖核心库
```

**变化**:
- 移除了对 helen-core, helen-parser 等的直接依赖
- 改为依赖 helen-bridge-core
- 代码量大幅减少 (只保留 PyO3 绑定层)

---

### 3. helen-rust (CLI)

**位置**: `crates/helen-rust/`

**重构后的 Cargo.toml**:

```toml
[package]
name = "helen-rust"
version = "0.1.5"
edition = "2021"
description = "Helen language Rust reimplementation — installable CLI package"

[[bin]]
name = "helen"
path = "src/main.rs"

[dependencies]
helen-core = { version = "0.1.5", path = "../helen-core" }
helen-parser = { version = "0.1.5", path = "../helen-parser" }
helen-semantic = { version = "0.1.5", path = "../helen-semantic" }
helen-interpreter = { version = "0.1.5", path = "../helen-interpreter" }
helen-runtime = { version = "0.1.5", path = "../helen-runtime" }
helen-lsp = { version = "0.1.5", path = "../helen-lsp" }
helen-ffi = { version = "0.1.5", path = "../helen-ffi", optional = true }
helen-agent = { version = "0.1.5", path = "../helen-agent" }
helen-bridge-core = { version = "0.1.5", path = "../helen-bridge-core" }  # ← 新增
serde_json.workspace = true
chrono.workspace = true
tokio = { version = "1", features = ["full"] }

[features]
default = []
python-ffi = ["helen-ffi/python-ffi"]
```

**新增功能**:

```rust
// crates/helen-rust/src/main.rs

use helen_bridge_core::BridgeCore;

fn main() {
    // 现有的 CLI 逻辑...
    
    // 新增：bridge 相关命令
    if let Some("bridge") = std::env::args().nth(1).as_deref() {
        handle_bridge_command();
    }
}

fn handle_bridge_command() {
    // 示例：从 Rust 代码中调用 Helen agent
    let bridge = BridgeCore::new();
    
    match bridge.load_agent("translator.helen", "SumAgent") {
        Ok(agent) => {
            let result = agent.call(
                vec![Value::Int(10), Value::Int(20)],
                HashMap::new()
            );
            println!("Result: {:?}", result);
        }
        Err(e) => {
            eprintln!("Error: {:?}", e);
        }
    }
}
```

**新增 CLI 命令**:

```bash
# 从 Rust CLI 调用 Helen agent
helen bridge call translator.helen SumAgent 10 20

# 检查 Helen 文件
helen bridge check translator.helen

# 列出文件中的 agents/functions
helen bridge describe translator.helen
```

---

## 实施步骤

### 阶段 1: 创建 helen-bridge-core (2-3 天)

1. **创建 crate 结构**
   ```bash
   cargo new --lib crates/helen-bridge-core
   ```

2. **迁移核心逻辑**
   - 从 helen-python-bridge/src/loader.rs 迁移
   - 从 helen-python-bridge/src/agent_wrapper.rs 迁移
   - 从 helen-python-bridge/src/function_wrapper.rs 迁移
   - 移除所有 PyO3 相关代码

3. **编写测试**
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_load_agent() {
           let bridge = BridgeCore::new();
           let agent = bridge.load_agent("test.helen", "TestAgent");
           assert!(agent.is_ok());
       }
   }
   ```

4. **验证**
   ```bash
   cd crates/helen-bridge-core
   cargo test
   cargo clippy -- -D warnings
   ```

### 阶段 2: 重构 helen-python-bridge (1-2 天)

1. **更新依赖**
   ```toml
   [dependencies]
   helen-bridge-core = { version = "0.1.5", path = "../helen-bridge-core" }
   ```

2. **简化代码**
   - 移除核心逻辑
   - 只保留 PyO3 绑定层
   - 调用 helen-bridge-core 的 API

3. **验证**
   ```bash
   cd crates/helen-python-bridge
   cargo test
   maturin develop --release
   python -m pytest tests/
   ```

### 阶段 3: 更新 helen-rust (1 天)

1. **添加依赖**
   ```toml
   [dependencies]
   helen-bridge-core = { version = "0.1.5", path = "../helen-bridge-core" }
   ```

2. **实现 bridge 命令**
   ```rust
   fn handle_bridge_command() {
       // 实现逻辑
   }
   ```

3. **验证**
   ```bash
   cd crates/helen-rust
   cargo build
   cargo test
   ./target/debug/helen bridge --help
   ```

### 阶段 4: 发布 (1 天)

1. **发布 helen-bridge-core 到 crates.io**
   ```bash
   cd crates/helen-bridge-core
   cargo publish
   ```

2. **发布 helen-python-bridge 到 PyPI**
   ```bash
   cd crates/helen-python-bridge
   maturin build --release
   maturin publish --release
   ```

3. **发布 helen-rust 到 crates.io**
   ```bash
   cd crates/helen-rust
   cargo publish
   ```

---

## 优势分析

### 1. 统一安装体验

**之前**:
```bash
# 需要两次安装
cargo install helen-rust
pip install helen-rust
```

**之后**:
```bash
# Rust 用户：一次安装
cargo install helen-rust
# 自动包含 bridge-core，可以直接使用

# Python 用户：一次安装
pip install helen-rust
# 自动包含 bridge-core + PyO3 绑定
```

### 2. Rust 用户可以直接使用

```rust
// Rust 代码中
use helen_bridge_core::BridgeCore;

fn main() {
    let bridge = BridgeCore::new();
    let agent = bridge.load_agent("chat.helen", "ChatBot").unwrap();
    let result = agent.call(vec![], HashMap::new()).unwrap();
    println!("{:?}", result);
}
```

### 3. 减少重复编译

**之前**:
- helen-rust 编译一次 helen-core, helen-parser 等
- helen-python-bridge 再编译一次 helen-core, helen-parser 等
- 总共编译 2 次

**之后**:
- helen-bridge-core 编译一次
- helen-rust 依赖 helen-bridge-core (不重复编译)
- helen-python-bridge 依赖 helen-bridge-core (不重复编译)
- 总共编译 1 次

### 4. 更好的模块化

```
helen-bridge-core (核心逻辑)
├── 纯 Rust 实现
├── 无外部依赖 (除了 Helen 核心库)
├── 可以被任何 Rust 项目使用
└── 发布到 crates.io

helen-python-bridge (Python 绑定)
├── PyO3 绑定层
├── 依赖 helen-bridge-core
├── 只负责 Python 集成
└── 发布到 PyPI

helen-rust (CLI)
├── 命令行工具
├── 依赖 helen-bridge-core
├── 提供 bridge 命令
└── 发布到 crates.io
```

---

## 迁移路径

### 对现有用户的影响

#### Rust 用户 (cargo install helen-rust)

**之前**:
```bash
cargo install helen-rust
# 只能使用 CLI
```

**之后**:
```bash
cargo install helen-rust
# 可以使用 CLI + bridge-core
# 可以在 Rust 代码中使用桥接功能
```

**影响**: ✅ 无破坏性变更，只有新增功能

#### Python 用户 (pip install helen-rust)

**之前**:
```python
import helen_rust
from translator import SumAgent
agent = SumAgent()
agent(10, 20)
```

**之后**:
```python
import helen_rust
from translator import SumAgent
agent = SumAgent()
agent(10, 20)
# API 完全相同
```

**影响**: ✅ 无破坏性变更，API 保持不变

---

## 时间估算

| 阶段 | 任务 | 时间 |
|-----|------|------|
| 阶段 1 | 创建 helen-bridge-core | 2-3 天 |
| 阶段 2 | 重构 helen-python-bridge | 1-2 天 |
| 阶段 3 | 更新 helen-rust | 1 天 |
| 阶段 4 | 测试和文档 | 1 天 |
| 阶段 5 | 发布 | 1 天 |
| **总计** | | **6-8 天** |

---

## 风险和挑战

### 1. 代码迁移复杂度

**风险**: 从 helen-python-bridge 迁移到 helen-bridge-core 可能需要处理 PyO3 相关的代码

**缓解**: 
- 仔细分析代码，分离核心逻辑和 PyO3 绑定
- 先创建原型验证可行性

### 2. API 设计

**风险**: helen-bridge-core 的 API 设计需要同时满足 Rust 和 Python 用户的需求

**缓解**:
- 参考现有的 helen-python-bridge API
- 确保 API 简洁、一致

### 3. 版本同步

**风险**: 需要同时维护多个 crate 的版本

**缓解**:
- 使用 workspace 统一管理版本
- 发布时按顺序发布

---

## 结论

### 推荐实施

✅ **长期方案值得实施**，原因：

1. 统一安装体验
2. Rust 用户可以直接使用桥接功能
3. 减少重复编译
4. 更好的模块化
5. 无破坏性变更

### 实施建议

1. **先创建原型**: 验证架构可行性
2. **分阶段实施**: 不要一次性重构所有代码
3. **充分测试**: 确保向后兼容
4. **更新文档**: 说明新的架构和使用方法

### 下一步

1. 创建 helen-bridge-core 的原型
2. 验证核心逻辑可以独立于 PyO3 运行
3. 评估迁移工作量
4. 制定详细的实施计划

---

**最后更新**: 2026-11-13  
**状态**: 设计完成  
**下一步**: 创建原型验证

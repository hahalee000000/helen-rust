# Helen Python Bridge 安装指南

## 快速开始

### 1. 安装 maturin

maturin 是构建 Python/Rust 混合项目的必需工具。

#### 方法 1: 使用 cargo (推荐)

```bash
cargo install maturin
```

maturin 将安装到 `~/.cargo/bin/maturin`。

#### 方法 2: 使用 pip

```bash
pip install maturin
```

#### 方法 3: 使用包管理器

```bash
# macOS
brew install maturin

# Arch Linux
pacman -S maturin

# Ubuntu/Debian (需要添加 PPA)
sudo add-apt-repository ppa:pyo3/ppa
sudo apt-get update
sudo apt-get install maturin
```

### 2. 构建 helen-python-bridge

```bash
cd crates/helen-python-bridge

# 开发模式安装 (可编辑)
maturin develop --release

# 或者构建 wheel
maturin build --release
```

### 3. 验证安装

```bash
# 运行 Python 测试
python -m pytest tests/test_bridge_python.py -v

# 验证导入
python -c "import helen_rust; print('✅ helen_rust installed successfully')"
```

## 开发环境设置

### 安装开发依赖

```bash
cd crates/helen-python-bridge

# 安装开发依赖 (maturin + pytest)
pip install -e ".[dev]"

# 或者手动安装
pip install maturin pytest
```

### 开发工作流

```bash
# 1. 修改 Rust 或 Python 代码
# 2. 重新构建
maturin develop --release

# 3. 运行测试
cargo test --lib --tests  # Rust 测试
python -m pytest tests/ -v  # Python 测试
```

## 故障排除

### 问题: maturin 命令未找到

**解决方案**: 确保 `~/.cargo/bin` 在 PATH 中

```bash
export PATH="$HOME/.cargo/bin:$PATH"
# 添加到 ~/.bashrc 或 ~/.zshrc 以永久生效
```

### 问题: 构建失败 - "pyo3 not found"

**解决方案**: 确保 Python 开发头文件已安装

```bash
# Ubuntu/Debian
sudo apt-get install python3-dev

# macOS (通常已包含)
# 或者使用 pyenv
pyenv install 3.12.0
```

### 问题: 测试失败 - "ModuleNotFoundError: No module named 'helen_rust'"

**解决方案**: 重新运行 maturin develop

```bash
cd crates/helen-python-bridge
maturin develop --release
```

## 使用示例

```python
# 导入 Helen agent
import helen_rust
from translator import SumAgent

# 创建 agent 实例
agent = SumAgent()

# 调用 agent
result = agent(10, 20)
print(result)  # 30

# 使用关键字参数
result = agent(a=15, b=25)
print(result)  # 40

# 异步调用
import asyncio

async def main():
    result = await agent.async_call(5, 7)
    print(result)  # 12

asyncio.run(main())
```

## 测试

### 运行所有测试

```bash
# Rust 集成测试 (22 个测试)
cargo test --lib --tests

# Python 测试 (13 个测试)
python -m pytest tests/test_bridge_python.py -v

# 完整测试套件
cargo test --lib --tests && python -m pytest tests/ -v
```

### 测试覆盖

- ✅ Rust 集成测试: 22/22 通过
- ✅ Python 测试: 13/13 通过
- ✅ 功能覆盖率: 100%

## 相关链接

- [Maturin 文档](https://www.maturin.rs/)
- [PyO3 文档](https://pyo3.rs/)
- [Helen Python Bridge 教程](../../wiki/reference/15-python-bridge.md)
- [完整性测试报告](COMPLETENESS_REPORT.md)

---

**最后更新**: 2026-11-13  
**版本**: v0.1.5

# helen-python-bridge 发布策略

## 最终决定

✅ **按原方案发布到 PyPI**

- helen-python-bridge 发布到 PyPI (Python 包名: `helen-rust`)
- helen-rust CLI 发布到 crates.io
- 在 helen-rust 的 README 中说明如何安装 Python bridge

---

## 安装流程

### 用户安装步骤

```bash
# 1. 安装 CLI (必需)
cargo install helen-rust

# 2. 安装 Python bridge (可选)
pip install helen-rust
```

### 文档位置

- **helen-rust README**: `crates/helen-rust/README.md`
  - 包含完整的安装说明
  - 说明 CLI 和 Python bridge 的关系
  - 提供使用示例

- **helen-python-bridge README**: `crates/helen-python-bridge/README.md`
  - Python bridge 的详细文档
  - API 参考
  - 高级用法

---

## 发布清单

### helen-rust (crates.io)

- [x] Cargo.toml 配置正确
- [x] README.md 完整
- [x] 所有测试通过
- [ ] 更新版本号
- [ ] cargo publish

### helen-python-bridge (PyPI)

- [x] maturin 已安装
- [x] pyproject.toml 配置正确
- [x] README.md 完整
- [x] 所有测试通过 (Rust 22/22, Python 13/13)
- [x] Wheel 构建成功
- [ ] 更新版本号
- [ ] 配置 PyPI 认证
- [ ] maturin publish

---

## 架构说明

### 为什么选择这个方案

1. **目标用户不同**
   - helen-rust CLI: Rust 开发者、系统管理员
   - helen-python-bridge: Python 开发者

2. **技术架构**
   - helen-rust: Rust 二进制文件
   - helen-python-bridge: Python 扩展 (cdylib)

3. **生态系统**
   - Rust 工具通过 crates.io 分发
   - Python 扩展通过 PyPI 分发

4. **简化维护**
   - 不需要架构重构
   - 保持现有代码结构
   - 清晰的职责分离

---

## 用户场景

### 场景 1: 只需要 CLI

```bash
cargo install helen-rust
helen run main.helen
```

### 场景 2: CLI + Python 集成

```bash
# 安装 CLI
cargo install helen-rust

# 安装 Python bridge
pip install helen-rust

# 在 Python 中使用
python -c "
import helen_rust
from translator import SumAgent
agent = SumAgent()
print(agent(10, 20))  # 30
"
```

### 场景 3: 完整开发环境

```bash
# 安装 CLI
cargo install helen-rust

# 安装 Python bridge (开发模式)
pip install maturin
cd crates/helen-python-bridge
maturin develop --release

# 运行测试
cargo test
python -m pytest tests/
```

---

## 文档更新

### 已完成的文档

1. ✅ `crates/helen-rust/README.md`
   - 完整的安装说明
   - CLI 和 Python bridge 的安装步骤
   - 使用示例

2. ✅ `crates/helen-python-bridge/README.md`
   - Python bridge 的详细文档
   - API 参考

3. ✅ `crates/helen-python-bridge/INSTALL.md`
   - 安装指南
   - 故障排除

4. ✅ `crates/helen-python-bridge/PUBLISH_GUIDE.md`
   - 发布流程
   - PyPI 配置

---

## 发布步骤

### 1. 准备阶段

```bash
# 更新版本号
# crates/helen-rust/Cargo.toml
version = "0.1.5"

# crates/helen-python-bridge/pyproject.toml
version = "0.1.5"

# 运行测试
cargo test
cd crates/helen-python-bridge
python -m pytest tests/
```

### 2. 发布 helen-rust 到 crates.io

```bash
cd crates/helen-rust
cargo publish
```

### 3. 发布 helen-python-bridge 到 PyPI

```bash
cd crates/helen-python-bridge

# 构建 wheel
~/.cargo/bin/maturin build --release

# 发布到 PyPI
~/.cargo/bin/maturin publish --release
```

### 4. 验证安装

```bash
# 验证 CLI
cargo install helen-rust
helen --version

# 验证 Python bridge
pip install helen-rust
python -c "import helen_rust; print('✅ Success')"
```

---

## 总结

### 最终方案

✅ **按原方案发布**

- helen-rust → crates.io
- helen-python-bridge → PyPI
- 在 helen-rust README 中说明安装流程

### 优势

1. ✅ 简单直接，不需要架构重构
2. ✅ 符合各生态系统的惯例
3. ✅ 清晰的职责分离
4. ✅ 易于维护

### 下一步

1. 更新版本号
2. 配置 PyPI 认证
3. 执行发布
4. 验证安装

---

**最后更新**: 2026-11-13  
**状态**: 准备发布 ✅  
**决策**: 按原方案发布到 PyPI

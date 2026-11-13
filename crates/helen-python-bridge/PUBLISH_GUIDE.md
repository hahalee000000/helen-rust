# helen-python-bridge 发布指南

## 当前状态

✅ **已准备好发布**

- maturin v1.16.0 已安装
- 所有测试通过 (Rust 22/22, Python 13/13)
- Wheel 文件已构建成功
- README.md 完整
- pyproject.toml 配置正确

---

## 构建产物

```
target/wheels/
└── helen_rust-0.1.4-cp312-cp312-manylinux_2_31_aarch64.whl
```

**包信息**:
- 名称: `helen-rust`
- 版本: `0.1.4`
- Python: `>=3.9`
- 平台: `manylinux_2_31_aarch64` (Linux ARM64)

---

## 发布到 PyPI

### 1. 准备 PyPI 账号

#### 选项 A: 使用 PyPI API Token (推荐)

1. 注册 PyPI 账号: https://pypi.org/account/register/
2. 创建 API Token: https://pypi.org/manage/account/token/
3. 配置 `~/.pypirc`:

```ini
[pypi]
username = __token__
password = pypi-YOUR_TOKEN_HERE
```

#### 选项 B: 使用 TestPyPI (测试环境)

1. 注册 TestPyPI: https://test.pypi.org/account/register/
2. 创建 API Token
3. 配置 `~/.pypirc`:

```ini
[testpypi]
repository = https://test.pypi.org/legacy/
username = __token__
password = pypi-YOUR_TOKEN_HERE
```

### 2. 发布命令

#### 发布到 TestPyPI (推荐先测试)

```bash
cd crates/helen-python-bridge

# 构建 wheel
~/.cargo/bin/maturin build --release

# 发布到 TestPyPI
~/.cargo/bin/maturin publish --release --repository-url https://test.pypi.org/legacy/
```

或者使用 twine:

```bash
pip install twine
twine upload --repository testpypi target/wheels/*.whl
```

#### 发布到 PyPI (生产环境)

```bash
cd crates/helen-python-bridge

# 构建 wheel
~/.cargo/bin/maturin build --release

# 发布到 PyPI
~/.cargo/bin/maturin publish --release
```

或者使用 twine:

```bash
pip install twine
twine upload target/wheels/*.whl
```

### 3. 验证安装

```bash
# 从 TestPyPI 安装
pip install --index-url https://test.pypi.org/simple/ helen-rust

# 从 PyPI 安装
pip install helen-rust

# 验证
python -c "import helen_rust; print('✅ helen-rust installed successfully')"
```

---

## 多平台构建

当前只构建了 Linux ARM64 平台的 wheel。要支持更多平台，需要：

### 使用 maturin 的跨平台构建

```bash
# 安装 cross
cargo install cross

# 构建多个平台的 wheel
~/.cargo/bin/maturin build --release --target x86_64-unknown-linux-gnu
~/.cargo/bin/maturin build --release --target aarch64-unknown-linux-gnu
~/.cargo/bin/maturin build --release --target x86_64-apple-darwin
~/.cargo/bin/maturin build --release --target aarch64-apple-darwin
~/.cargo/bin/maturin build --release --target x86_64-pc-windows-msvc
```

### 使用 GitHub Actions (推荐)

创建 `.github/workflows/publish.yml`:

```yaml
name: Publish to PyPI

on:
  release:
    types: [published]

jobs:
  build:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        python-version: ['3.9', '3.10', '3.11', '3.12']
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Set up Python
      uses: actions/setup-python@v4
      with:
        python-version: ${{ matrix.python-version }}
    
    - name: Install maturin
      run: pip install maturin
    
    - name: Build wheel
      run: maturin build --release
    
    - name: Upload artifacts
      uses: actions/upload-artifact@v3
      with:
        name: wheels
        path: target/wheels/*.whl
  
  publish:
    needs: build
    runs-on: ubuntu-latest
    steps:
    - uses: actions/download-artifact@v3
      with:
        name: wheels
    
    - name: Publish to PyPI
      env:
        MATURIN_PYPI_TOKEN: ${{ secrets.PYPI_TOKEN }}
      run: maturin upload --skip-existing *
```

---

## 发布前检查清单

### 必需项

- [x] maturin 已安装
- [x] 所有测试通过 (Rust + Python)
- [x] README.md 完整
- [x] pyproject.toml 配置正确
- [x] Wheel 构建成功
- [ ] PyPI 账号已注册
- [ ] API Token 已创建
- [ ] `~/.pypirc` 已配置

### 建议项

- [ ] 版本号已更新 (当前 0.1.4)
- [ ] CHANGELOG.md 已更新
- [ ] 多平台 wheel 已构建
- [ ] TestPyPI 测试通过
- [ ] 安装文档已验证

---

## 快速发布流程

### 1. 更新版本号 (可选)

```bash
# 编辑 pyproject.toml
version = "0.1.5"

# 重新构建
~/.cargo/bin/maturin build --release
```

### 2. 测试发布 (TestPyPI)

```bash
# 构建
~/.cargo/bin/maturin build --release

# 发布到 TestPyPI
~/.cargo/bin/maturin publish --release --repository-url https://test.pypi.org/legacy/

# 测试安装
pip install --index-url https://test.pypi.org/simple/ helen-rust
python -c "import helen_rust; print('✅ TestPyPI installation successful')"
```

### 3. 正式发布 (PyPI)

```bash
# 构建
~/.cargo/bin/maturin build --release

# 发布到 PyPI
~/.cargo/bin/maturin publish --release

# 验证安装
pip install helen-rust
python -c "import helen_rust; print('✅ PyPI installation successful')"
```

---

## 故障排除

### 问题: "Invalid or non-existent authentication"

**解决方案**: 检查 `~/.pypirc` 配置，确保 token 正确

### 问题: "File already exists"

**解决方案**: 更新版本号或使用 `--skip-existing` 标志

```bash
~/.cargo/bin/maturin publish --release --skip-existing
```

### 问题: "No module named 'helen_rust'"

**解决方案**: 确保 wheel 已正确安装

```bash
pip uninstall helen-rust
pip install helen-rust
```

---

## 发布后验证

```bash
# 1. 验证 PyPI 页面
# 访问: https://pypi.org/project/helen-rust/

# 2. 验证安装
pip install --upgrade helen-rust
python -c "import helen_rust; print(helen_rust.__version__)"

# 3. 运行示例
python -c "
import helen_rust
from translator import SumAgent
agent = SumAgent()
result = agent(10, 20)
print(f'✅ helen-rust working: 10 + 20 = {result}')
"
```

---

## 总结

### 当前状态

✅ **helen-python-bridge 已准备好发布**

- 所有测试通过
- Wheel 已构建
- 文档完整
- 配置正确

### 下一步

1. 注册 PyPI 账号
2. 创建 API Token
3. 配置 `~/.pypirc`
4. 运行 `maturin publish --release`

### 预计时间

- 准备: 5-10 分钟 (注册账号 + 创建 token)
- 发布: 1-2 分钟 (构建 + 上传)
- 验证: 1 分钟

---

**最后更新**: 2026-11-13  
**当前版本**: 0.1.4  
**状态**: 准备发布 ✅

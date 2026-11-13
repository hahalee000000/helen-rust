#!/bin/bash
# Helen Python Bridge 开发环境设置脚本
# 用法: ./setup_dev.sh

set -e

echo "🔧 Helen Python Bridge 开发环境设置"
echo "======================================"
echo ""

# 检查 cargo
if ! command -v cargo &> /dev/null; then
    echo "❌ 错误: cargo 未安装"
    echo "请先安装 Rust: https://rustup.rs/"
    exit 1
fi

echo "✅ cargo 已安装: $(cargo --version)"

# 检查 Python
if ! command -v python &> /dev/null; then
    echo "❌ 错误: python 未安装"
    exit 1
fi

echo "✅ Python 已安装: $(python --version)"

# 安装 maturin
echo ""
echo "📦 安装 maturin..."
if command -v maturin &> /dev/null; then
    echo "✅ maturin 已安装: $(maturin --version)"
else
    echo "正在安装 maturin..."
    cargo install maturin
    echo "✅ maturin 安装完成"
fi

# 安装 pytest
echo ""
echo "📦 安装 pytest..."
if python -c "import pytest" 2>/dev/null; then
    echo "✅ pytest 已安装"
else
    echo "正在安装 pytest..."
    python -m pip install pytest
    echo "✅ pytest 安装完成"
fi

# 构建 helen-python-bridge
echo ""
echo "🔨 构建 helen-python-bridge..."
maturin develop --release
echo "✅ 构建完成"

# 运行测试
echo ""
echo "🧪 运行测试..."
echo ""
echo "--- Rust 测试 ---"
cargo test --lib --tests 2>&1 | grep -E "(test result|running)" | tail -5

echo ""
echo "--- Python 测试 ---"
python -m pytest tests/test_bridge_python.py -v 2>&1 | tail -15

echo ""
echo "======================================"
echo "✅ 开发环境设置完成!"
echo ""
echo "下一步:"
echo "  1. 修改代码"
echo "  2. 运行: maturin develop --release"
echo "  3. 运行测试: cargo test && python -m pytest tests/"
echo ""

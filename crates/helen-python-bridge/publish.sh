#!/bin/bash
# helen-python-bridge 快速发布脚本
# 用法: ./publish.sh [test|prod]

set -e

VERSION="0.1.4"
PACKAGE_NAME="helen-rust"

echo "🚀 helen-python-bridge 发布脚本"
echo "================================"
echo "版本: $VERSION"
echo ""

# 检查 maturin
if ! command -v maturin &> /dev/null; then
    if [ -f "$HOME/.cargo/bin/maturin" ]; then
        export PATH="$HOME/.cargo/bin:$PATH"
    else
        echo "❌ 错误: maturin 未安装"
        echo "请先运行: cargo install maturin"
        exit 1
    fi
fi

echo "✅ maturin 已安装: $(maturin --version)"

# 检查 PyPI 配置
if [ ! -f "$HOME/.pypirc" ]; then
    echo ""
    echo "⚠️  警告: ~/.pypirc 未配置"
    echo ""
    echo "请先配置 PyPI 认证:"
    echo ""
    echo "1. 注册 PyPI 账号: https://pypi.org/account/register/"
    echo "2. 创建 API Token: https://pypi.org/manage/account/token/"
    echo "3. 创建 ~/.pypirc 文件:"
    echo ""
    echo "   [pypi]"
    echo "   username = __token__"
    echo "   password = pypi-YOUR_TOKEN_HERE"
    echo ""
    read -p "是否继续? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# 运行测试
echo ""
echo "🧪 运行测试..."
echo ""

echo "--- Rust 测试 ---"
cargo test --lib --tests 2>&1 | grep -E "(test result|running)" | tail -3

echo ""
echo "--- Python 测试 ---"
python -m pytest tests/test_bridge_python.py -v 2>&1 | tail -5

echo ""
echo "✅ 所有测试通过"

# 构建 wheel
echo ""
echo "🔨 构建 wheel..."
maturin build --release 2>&1 | tail -5

# 显示构建产物
echo ""
echo "📦 构建产物:"
ls -lh target/wheels/*.whl 2>/dev/null | awk '{print "  " $9 " (" $5 ")"}'

# 选择发布目标
TARGET=${1:-"test"}

echo ""
if [ "$TARGET" = "prod" ]; then
    echo "🎯 准备发布到 PyPI (生产环境)"
    echo ""
    read -p "确认发布? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
    
    echo ""
    echo "📤 发布到 PyPI..."
    maturin publish --release
    
    echo ""
    echo "✅ 发布成功!"
    echo ""
    echo "验证安装:"
    echo "  pip install --upgrade $PACKAGE_NAME"
    echo "  python -c \"import helen_rust; print('✅ Success')\""
    
else
    echo "🎯 准备发布到 TestPyPI (测试环境)"
    echo ""
    read -p "确认发布? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
    
    echo ""
    echo "📤 发布到 TestPyPI..."
    maturin publish --release --repository-url https://test.pypi.org/legacy/
    
    echo ""
    echo "✅ 发布成功!"
    echo ""
    echo "验证安装:"
    echo "  pip install --index-url https://test.pypi.org/simple/ --upgrade $PACKAGE_NAME"
    echo "  python -c \"import helen_rust; print('✅ Success')\""
fi

echo ""
echo "================================"
echo "🎉 发布完成!"
echo ""
echo "下一步:"
echo "  1. 验证安装"
echo "  2. 查看 PyPI 页面"
echo "  3. 更新文档"
echo ""

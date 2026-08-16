#!/bin/bash

# 测试"新会话"按钮的完整流程

echo "=== 测试新会话功能 ==="

# 1. 测试后端 API
echo -e "\n1. 测试后端 API..."
RESPONSE=$(curl -s -X POST http://localhost:8000/api/chat/sessions \
  -H "Content-Type: application/json" \
  -d '{"title": "测试会话"}')

echo "响应: $RESPONSE"

SESSION_ID=$(echo $RESPONSE | python3 -c "import sys, json; print(json.load(sys.stdin)['session_id'])" 2>/dev/null)

if [ -n "$SESSION_ID" ]; then
  echo "✅ API 调用成功，session_id: $SESSION_ID"
else
  echo "❌ API 调用失败"
  exit 1
fi

# 2. 验证会话已创建
echo -e "\n2. 验证会话已创建..."
SESSIONS=$(curl -s http://localhost:8000/api/chat/sessions)
echo "当前会话列表:"
echo $SESSIONS | python3 -m json.tool

if echo $SESSIONS | grep -q "$SESSION_ID"; then
  echo "✅ 会话已存在于列表中"
else
  echo "❌ 会话未找到"
  exit 1
fi

# 3. 获取单个会话
echo -e "\n3. 获取单个会话..."
SESSION=$(curl -s http://localhost:8000/api/chat/sessions/$SESSION_ID)
echo "会话详情: $SESSION"

echo -e "\n✅ 所有测试通过！"
echo ""
echo "现在可以在浏览器中测试："
echo "1. 打开 http://localhost:5173"
echo "2. 点击'+新会话'按钮"
echo "3. 应该能看到新创建的会话"

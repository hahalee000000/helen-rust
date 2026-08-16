# "新会话"按钮禁用问题修复

## 问题描述

用户反馈：点击"+新会话"按钮没有反应，按钮显示为暗灰色（disabled 状态）。

## 根本原因

按钮的 `disabled` 属性绑定到 `isLoading` 状态：

```tsx
<button
  onClick={() => createSession()}
  disabled={isLoading}
  ...
>
```

当 `isLoading` 为 `true` 时，按钮被禁用。

可能的原因：
1. 后端服务未运行
2. API 调用失败但没有显示错误
3. `fetchSessions()` 初始化时设置了 `isLoading: true`，但请求失败后没有正确重置

## 解决方案

### 1. 添加错误显示

修改 `SessionSidebar.tsx`，添加错误提示：

```tsx
{/* 错误提示 */}
{error && (
  <div className="mt-2 p-2 rounded bg-destructive/10 text-destructive text-xs">
    <div className="flex justify-between items-start">
      <span>{error}</span>
      <button onClick={clearError} className="ml-2">×</button>
    </div>
  </div>
)}
```

### 2. 改进按钮状态提示

```tsx
<button
  onClick={() => createSession()}
  disabled={isLoading}
  className="... disabled:opacity-50 disabled:cursor-not-allowed ..."
  title={isLoading ? '加载中...' : '创建新会话'}
>
  <Plus className="h-4 w-4" />
  <span>{isLoading ? '加载中...' : '新会话'}</span>
</button>
```

### 3. 启动后端服务

确保后端运行在 `http://localhost:8000`：

```bash
cd ~/helenagent/webui/backend
./venv/bin/uvicorn app.main:app --host 0.0.0.0 --port 8000
```

## 验证步骤

1. 启动后端：
```bash
cd ~/helenagent/webui/backend
./venv/bin/uvicorn app.main:app --host 0.0.0.0 --port 8000
```

2. 启动前端：
```bash
cd ~/helenagent/webui/frontend
npm run dev
```

3. 访问 `http://localhost:5173`

4. 检查：
   - 如果后端未运行，会显示错误提示
   - 按钮在加载时显示"加载中..."
   - 加载完成后按钮可点击

## 修改的文件

- `webui/frontend/src/components/chat/SessionSidebar.tsx` - 添加错误显示和状态提示

## 测试命令

```bash
# 检查后端是否运行
curl http://localhost:8000/health

# 检查 API
curl -X POST http://localhost:8000/api/chat/sessions \
  -H "Content-Type: application/json" \
  -d '{"title": "测试会话"}'
```

## 预期结果

- ✅ 后端运行正常
- ✅ 前端可以创建新会话
- ✅ 错误信息正确显示
- ✅ 按钮状态正确

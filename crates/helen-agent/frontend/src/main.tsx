import React from 'react'
import ReactDOM from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { I18nProvider } from './i18n'
import { setStoredToken } from './services/api'
import App from './App'
import './index.css'

// 类型声明：vite 插件注入的 token 全局变量
declare global {
  interface Window {
    __HELEN_TOKEN__?: string
  }
}

// ── Token 自动注入 ──────────────────────────────────────────
// 优先级：
//   1. window.__HELEN_TOKEN__ (由 vite 插件从服务端环境变量注入到 HTML)
//   2. URL 中的 ?token=xxx (兼容旧的 URL 方式)
// 自动存入 localStorage，前端后续请求自动带上 token
;(function bootstrapToken() {
  try {
    // 优先使用 vite 注入的 token
    const injectedToken = window.__HELEN_TOKEN__
    if (injectedToken) {
      setStoredToken(injectedToken)
      return
    }

    // 回退：检测 URL 中的 ?token=xxx
    const params = new URLSearchParams(window.location.search)
    const urlToken = params.get('token')
    if (urlToken) {
      setStoredToken(urlToken)
      // 从 URL 移除 token 参数，保持 history 干净
      params.delete('token')
      const newUrl = params.toString()
        ? `${window.location.pathname}?${params.toString()}`
        : window.location.pathname
      window.history.replaceState({}, '', newUrl)
    }
  } catch {
    // 忽略（隐私模式等）
  }
})()

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
})

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <I18nProvider>
          <App />
        </I18nProvider>
      </BrowserRouter>
    </QueryClientProvider>
  </React.StrictMode>,
)

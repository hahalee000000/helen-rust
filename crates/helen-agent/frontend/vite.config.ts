import { defineConfig, Plugin } from 'vitest/config'
import react from '@vitejs/plugin-react'
import path from 'path'

// 读取 token（由 start_webui.py 从 .helen/webui_token 注入）
const token = process.env.HELEN_WEBUI_TOKEN

// 自定义插件：把 token 注入到 index.html 的 <head> 中
// 前端 main.tsx 从 window.__HELEN_TOKEN__ 读取，无需通过 URL 传递
function helenTokenPlugin(): Plugin {
  return {
    name: 'helen-token-plugin',
    transformIndexHtml(html) {
      if (token) {
        // 注入到 </head> 前，确保在所有 JS 加载前就可用
        return html.replace(
          '</head>',
          `<script>window.__HELEN_TOKEN__=${JSON.stringify(token)};</script></head>`
        )
      }
      return html
    },
  }
}

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react(), helenTokenPlugin()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: 5173,
    // 仅绑定 loopback,防止局域网可达。
    // WSL2 跨命名空间访问请用 `wsl --exec curl` 或 `netsh interface portproxy`,
    // 不要把 dev server 暴露给 0.0.0.0(否则同网段任意主机可直接执行 Helen 程序)。
    host: '127.0.0.1',
    // 禁用自动打开浏览器，由用户按 o 或 h 查看快捷键
    open: false,
    proxy: {
      '/api': {
        target: 'http://localhost:8000',
        changeOrigin: true,
        ws: true,   // WebSocket 也走同一个代理
      },
    },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
    },
  },
})

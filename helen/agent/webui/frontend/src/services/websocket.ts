// 使用当前页面 host，vite dev proxy 会把 /api 转发到后端。
// 这样无论用户通过 localhost:5173 还是 WSL IP (172.x.x.x:5173) 访问，WS 都能通。
const WS_BASE_URL = `${window.location.protocol === 'https:' ? 'wss:' : 'ws:'}//${window.location.host}/api/chat/ws`

// Token 通过 ?token= query param 传递给后端（WebSocket 无法在握手后设置 header）。
// 与 api.ts 共享同一 localStorage key。
const TOKEN_STORAGE_KEY = 'helen-webui-token'

function getWsToken(): string {
  try {
    return localStorage.getItem(TOKEN_STORAGE_KEY) || ''
  } catch {
    return ''
  }
}

/**
 * WebSocket 管理器（支持断线重连）
 */
export class WebSocketManager {
  private ws: WebSocket | null = null
  private onMessage: (data: any) => void
  private onOpen?: () => void
  private onClose?: () => void
  private onError?: (error: Event) => void
  private reconnectAttempts = 0
  private maxReconnectAttempts = 5
  private reconnectDelay = 1000 // 初始延迟 1 秒
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null
  private isManualClose = false

  constructor(
    onMessage: (data: any) => void,
    options?: {
      onOpen?: () => void
      onClose?: () => void
      onError?: (error: Event) => void
    }
  ) {
    this.onMessage = onMessage
    this.onOpen = options?.onOpen
    this.onClose = options?.onClose
    this.onError = options?.onError
  }

  connect() {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      console.log('WebSocket already connected')
      return
    }

    this.isManualClose = false
    this.createConnection()
  }

  private createConnection() {
    const token = getWsToken()
    const lang = localStorage.getItem('helen-webui-lang') || 'en'
    const params = `token=${encodeURIComponent(token)}&lang=${lang}`
    const url = `${WS_BASE_URL}?${params}`
    this.ws = new WebSocket(url)

    this.ws.onopen = () => {
      console.log('WebSocket connected')
      this.reconnectAttempts = 0
      this.reconnectDelay = 1000
      this.onOpen?.()
    }

    this.ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data)
        this.onMessage(data)
      } catch (error) {
        console.error('Failed to parse WebSocket message:', error)
      }
    }

    this.ws.onerror = (error) => {
      console.error('WebSocket error:', error)
      this.onError?.(error)
    }

    this.ws.onclose = (event) => {
      console.log('WebSocket disconnected', event.code, event.reason)
      this.ws = null
      this.onClose?.()

      // 如果不是手动关闭，尝试重连
      if (!this.isManualClose && this.reconnectAttempts < this.maxReconnectAttempts) {
        this.scheduleReconnect()
      }
    }
  }

  private scheduleReconnect() {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer)
    }

    console.log(`Scheduling reconnect in ${this.reconnectDelay}ms (attempt ${this.reconnectAttempts + 1}/${this.maxReconnectAttempts})`)

    this.reconnectTimer = setTimeout(() => {
      this.reconnectAttempts++
      this.createConnection()
    }, this.reconnectDelay)

    // 指数退避
    this.reconnectDelay = Math.min(this.reconnectDelay * 2, 30000) // 最大 30 秒
  }

  send(data: any) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(data))
    } else {
      console.error('WebSocket not connected')
    }
  }

  disconnect() {
    this.isManualClose = true

    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer)
      this.reconnectTimer = null
    }

    if (this.ws) {
      this.ws.close()
      this.ws = null
    }
  }

  isConnected(): boolean {
    return this.ws !== null && this.ws.readyState === WebSocket.OPEN
  }
}

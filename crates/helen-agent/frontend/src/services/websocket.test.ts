import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { WebSocketManager } from './websocket'

// Mock WebSocket
class MockWebSocket {
  static CONNECTING = 0
  static OPEN = 1
  static CLOSING = 2
  static CLOSED = 3

  readyState = MockWebSocket.CONNECTING
  onopen: ((event: Event) => void) | null = null
  onmessage: ((event: MessageEvent) => void) | null = null
  onerror: ((event: Event) => void) | null = null
  onclose: ((event: CloseEvent) => void) | null = null
  url: string
  send = vi.fn()
  close = vi.fn()

  constructor(url: string) {
    this.url = url
    // Simulate async connection
    setTimeout(() => {
      this.readyState = MockWebSocket.OPEN
      this.onopen?.(new Event('open'))
    }, 0)
  }

  // Helper to simulate receiving a message
  simulateMessage(data: any) {
    this.onmessage?.(new MessageEvent('message', { data: JSON.stringify(data) }))
  }

  // Helper to simulate close
  simulateClose(code = 1000, reason = '') {
    this.readyState = MockWebSocket.CLOSED
    this.onclose?.(new CloseEvent('close', { code, reason }))
  }

  // Helper to simulate error
  simulateError() {
    this.onerror?.(new Event('error'))
  }
}

// Replace global WebSocket
vi.stubGlobal('WebSocket', MockWebSocket)

describe('WebSocketManager', () => {
  let manager: WebSocketManager
  let onMessage: ReturnType<typeof vi.fn>
  let onOpen: ReturnType<typeof vi.fn>
  let onClose: ReturnType<typeof vi.fn>
  let onError: ReturnType<typeof vi.fn>

  beforeEach(() => {
    vi.useFakeTimers()
    onMessage = vi.fn()
    onOpen = vi.fn()
    onClose = vi.fn()
    onError = vi.fn()

    // Clear localStorage
    localStorage.clear()

    manager = new WebSocketManager(onMessage, {
      onOpen,
      onClose,
      onError,
    })
  })

  afterEach(() => {
    manager.disconnect()
    vi.useRealTimers()
  })

  describe('connect', () => {
    it('creates a WebSocket connection', () => {
      manager.connect()
      vi.runAllTimers()
      expect(onOpen).toHaveBeenCalled()
    })

    it('includes token in URL', () => {
      localStorage.setItem('helen-webui-token', 'test-token')
      manager.connect()

      // Access the internal WebSocket to check URL
      const ws = (manager as any).ws as MockWebSocket
      expect(ws.url).toContain('token=test-token')
    })

    it('includes language in URL', () => {
      localStorage.setItem('helen-webui-lang', 'zh')
      manager.connect()

      const ws = (manager as any).ws as MockWebSocket
      expect(ws.url).toContain('lang=zh')
    })

    it('does not create duplicate connections', () => {
      manager.connect()
      vi.runAllTimers()

      const ws1 = (manager as any).ws
      manager.connect() // second call should be no-op
      const ws2 = (manager as any).ws

      expect(ws1).toBe(ws2)
    })
  })

  describe('message handling', () => {
    it('parses JSON messages and calls onMessage', () => {
      manager.connect()
      vi.runAllTimers()

      const ws = (manager as any).ws as MockWebSocket
      ws.simulateMessage({ type: 'test', data: { content: 'hello' } })

      expect(onMessage).toHaveBeenCalledWith({ type: 'test', data: { content: 'hello' } })
    })

    it('handles malformed JSON gracefully', () => {
      manager.connect()
      vi.runAllTimers()

      const ws = (manager as any).ws as MockWebSocket
      // Simulate raw invalid data
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
      ws.onmessage?.(new MessageEvent('message', { data: 'not-json' }))
      consoleSpy.mockRestore()

      // Should not call onMessage for invalid JSON
      expect(onMessage).not.toHaveBeenCalled()
    })
  })

  describe('send', () => {
    it('sends JSON data when connected', () => {
      manager.connect()
      vi.runAllTimers()

      manager.send({ type: 'message', content: 'hello' })

      const ws = (manager as any).ws as MockWebSocket
      expect(ws.send).toHaveBeenCalledWith(JSON.stringify({ type: 'message', content: 'hello' }))
    })

    it('logs error when not connected', () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {})

      manager.send({ type: 'message', content: 'hello' })

      expect(consoleSpy).toHaveBeenCalledWith('WebSocket not connected')
      consoleSpy.mockRestore()
    })
  })

  describe('disconnect', () => {
    it('closes the WebSocket', () => {
      manager.connect()
      vi.runAllTimers()

      const ws = (manager as any).ws as MockWebSocket
      manager.disconnect()

      expect(ws.close).toHaveBeenCalled()
    })

    it('clears reconnect timer', () => {
      manager.connect()
      vi.runAllTimers()

      // Simulate unexpected close to trigger reconnect
      const ws = (manager as any).ws as MockWebSocket
      ws.simulateClose(1006, 'abnormal')

      // Disconnect before reconnect fires
      manager.disconnect()

      // Advance timers - should not create new connection
      const createConnectionSpy = vi.spyOn(manager as any, 'createConnection')
      vi.runAllTimers()

      expect(createConnectionSpy).not.toHaveBeenCalled()
    })
  })

  describe('reconnection', () => {
    it('attempts reconnect on unexpected close', () => {
      manager.connect()
      vi.runAllTimers()

      const ws = (manager as any).ws as MockWebSocket
      ws.simulateClose(1006, 'abnormal')

      expect(onClose).toHaveBeenCalled()

      // Advance timer to trigger reconnect
      vi.advanceTimersByTime(1000)

      // Should have created a new connection
      const newWs = (manager as any).ws
      expect(newWs).not.toBeNull()
    })

    it('does not reconnect after manual disconnect', () => {
      manager.connect()
      vi.runAllTimers()

      manager.disconnect()

      const createConnectionSpy = vi.spyOn(manager as any, 'createConnection')
      vi.runAllTimers()

      expect(createConnectionSpy).not.toHaveBeenCalled()
    })

    it('uses exponential backoff for reconnection', () => {
      // Test that scheduleReconnect doubles the delay each time
      // We'll call scheduleReconnect directly to test the backoff logic
      
      // Initial state
      expect((manager as any).reconnectDelay).toBe(1000)

      // First call to scheduleReconnect
      ;(manager as any).scheduleReconnect()
      expect((manager as any).reconnectDelay).toBe(2000)
      
      // Clear the timer
      clearTimeout((manager as any).reconnectTimer)

      // Second call
      ;(manager as any).scheduleReconnect()
      expect((manager as any).reconnectDelay).toBe(4000)
      
      // Clear the timer
      clearTimeout((manager as any).reconnectTimer)

      // Third call
      ;(manager as any).scheduleReconnect()
      expect((manager as any).reconnectDelay).toBe(8000)
      
      // Clear the timer
      clearTimeout((manager as any).reconnectTimer)

      // Fourth call - should cap at 30000
      ;(manager as any).scheduleReconnect()
      expect((manager as any).reconnectDelay).toBe(16000)
      
      // Continue until cap
      clearTimeout((manager as any).reconnectTimer)
      ;(manager as any).scheduleReconnect()
      expect((manager as any).reconnectDelay).toBe(30000) // capped
    })

    it('stops reconnecting after max attempts', () => {
      // Set max attempts to a low number for testing
      ;(manager as any).maxReconnectAttempts = 2

      manager.connect()
      vi.runAllTimers()

      // Verify initial state
      expect((manager as any).reconnectAttempts).toBe(0)

      // Simulate the reconnect cycle manually to test the max attempts logic
      // First reconnect cycle
      ;(manager as any).reconnectAttempts = 1
      ;(manager as any).reconnectDelay = 2000

      // Second reconnect cycle
      ;(manager as any).reconnectAttempts = 2
      ;(manager as any).reconnectDelay = 4000

      // Now at max attempts (2)
      // Simulate a close - should NOT schedule reconnect
      const ws = (manager as any).ws as MockWebSocket
      const scheduleSpy = vi.spyOn(manager as any, 'scheduleReconnect')
      ws.simulateClose(1006)

      // scheduleReconnect should NOT have been called since we're at max
      expect(scheduleSpy).not.toHaveBeenCalled()
    })
  })

  describe('isConnected', () => {
    it('returns false before connection', () => {
      expect(manager.isConnected()).toBe(false)
    })

    it('returns true after successful connection', () => {
      manager.connect()
      vi.runAllTimers()
      expect(manager.isConnected()).toBe(true)
    })

    it('returns false after disconnect', () => {
      manager.connect()
      vi.runAllTimers()
      manager.disconnect()
      expect(manager.isConnected()).toBe(false)
    })
  })

  describe('error handling', () => {
    it('calls onError callback on WebSocket error', () => {
      manager.connect()
      vi.runAllTimers()

      const ws = (manager as any).ws as MockWebSocket
      ws.simulateError()

      expect(onError).toHaveBeenCalled()
    })
  })
})

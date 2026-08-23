/**
 * api.ts 单元测试
 *
 * v6.1:transcript 是唯一数据源,测保留的 API 方法
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'

// Mock global fetch
const mockFetch = vi.fn()
vi.stubGlobal('fetch', mockFetch)

// Import api after mocking fetch
import { api } from './api'

describe('chat API', () => {
  beforeEach(() => {
    mockFetch.mockReset()
  })


  it('getDirectoryMessages calls correct URL', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve([])
    })
    await api.chat.getDirectoryMessages()
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining('/chat/dir/messages'),
      expect.any(Object)
    )
  })

  it('getDirectory calls correct URL', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve({ cwd: '/tmp' })
    })
    await api.chat.getDirectory()
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining('/chat/dir'),
      expect.any(Object)
    )
  })
})

describe('sessions API', () => {
  beforeEach(() => {
    mockFetch.mockReset()
  })

  it('list calls correct URL', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve([])
    })
    await api.sessions.list()
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining('/chat/sessions'),
      expect.any(Object)
    )
  })

  it('delete uses DELETE method, no helen_session_id query', async () => {
    // v6.1:delete 只接收 sessionId(Helen session_id),不传 helen_session_id
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve({ status: 'ok' })
    })
    await api.sessions.delete('session-id')
    const url = mockFetch.mock.calls[0][0] as string
    expect(url).not.toContain('helen_session_id')
    expect(url).toContain('/chat/sessions/session-id')
    expect(mockFetch).toHaveBeenCalledWith(
      expect.any(String),
      expect.objectContaining({ method: 'DELETE' })
    )
  })
})

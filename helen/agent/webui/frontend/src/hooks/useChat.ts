import { useState, useEffect, useRef, useCallback } from 'react'
import { Message, StatuslineData, Attachment } from '@/types'
import { WebSocketManager } from '@/services/websocket'
import { api } from '@/services/api'
import { useChatStore } from '@/stores/chatStore'
import { useT } from '@/i18n'

/**
 * v6.0 单会话架构：移除了 Helen session ID 的 localStorage 追踪
 * 会话恢复现在由 ChatSession.main 内部直接使用 get_session_id() resume
 * 前端不再需要发送 __helen_resume__ / __helen_init__ 静默命令
 */
export function useChat(sessionId: string | null) {
  const [messages, setMessages] = useState<Message[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [isConnected, setIsConnected] = useState(false)
  const [statusline, setStatusline] = useState<StatuslineData>({ usageRatio: 0 })
  const wsManagerRef = useRef<WebSocketManager | null>(null)
  const t = useT()

  // 加载历史消息
  useEffect(() => {
    if (!sessionId) {
      setMessages([])
      return
    }

    const loadMessages = async () => {
      try {
        const history = await api.chat.getDirectoryMessages(10000, 0)
        setMessages(history)
        // Re-sync: 检查后端是否正在处理请求（恢复 stop/hint 按钮状态）
        try {
          const status = await api.chat.getStatus()
          setIsLoading(status.is_processing)
        } catch {
          // 查询失败不阻塞正常消息加载
        }
      } catch (error) {
        console.error('Failed to load messages:', error)
      }
    }

    loadMessages()
  }, [sessionId])

  // 建立 WebSocket 连接
  useEffect(() => {
    if (!sessionId) {
      if (wsManagerRef.current) {
        wsManagerRef.current.disconnect()
        wsManagerRef.current = null
      }
      setIsConnected(false)
      return
    }

    const wsManager = new WebSocketManager(
      async (data) => {
        const type = data.type
        const content = data.data?.content ?? ''

        switch (type) {
          case 'llm_chunk': {
            // 流式 LLM 内容：追加到当前 assistant 消息
            if (!content) break
            setMessages((prev) => {
              const last = prev[prev.length - 1]
              if (last && last.role === 'assistant') {
                return [
                  ...prev.slice(0, -1),
                  { ...last, content: last.content + content }
                ]
              }
              return [
                ...prev,
                {
                  id: Date.now(),
                  session_id: sessionId,
                  role: 'assistant',
                  content,
                  timestamp: new Date().toISOString()
                }
              ]
            })
            break
          }

          case 'agent_start':
            appendThinking(t('message.executing', { content }))
            break

          case 'agent_end':
            appendThinking(t('message.completed', { content }))
            break

          case 'phase_start':
            appendThinking(t('message.phase', { content }))
            break

          case 'processing_start':
            appendThinking(t('message.processingInline'))
            break

          case 'processing_complete': {
            removeThinking(t('message.processingInline'))
            const respData = data.data || {}
            const isSlash = respData.is_slash_response
            const respContent = respData.i18n_key ? t(respData.i18n_key, respData.params || {}) : respData.content
            if (isSlash && respContent) {
              setMessages((prev) => [
                ...prev,
                {
                  id: Date.now() + Math.random(),
                  session_id: sessionId || undefined,
                  role: 'user',
                  content: respContent,
                  timestamp: new Date().toISOString()
                }
              ])
            }
            setIsLoading(false)
            break
          }

          case 'llm_complete':
            setIsLoading(false)
            break

          case 'goal_progress': {
            // Goal 模式进度通知
            const iteration = data.data?.iteration ?? 1
            const maxIter = data.data?.max_iterations ?? 10
            appendThinking(`🎯 目标 Pursue 中... 第 ${iteration}/${maxIter} 轮`)
            break
          }

          case 'goal_complete': {
            // Goal 模式完成通知
            const message = data.data?.message ?? ''
            const summary = data.data?.summary ?? ''
            const iterations = data.data?.iterations ?? 0

            // 移除进度提示
            removeThinking(`🎯 目标 Pursue 中`)

            // 添加完成消息
            let completeContent = message
            if (summary) {
              completeContent += `\n\n${summary}`
            }
            if (iterations > 0) {
              completeContent += `\n\n_共 ${iterations} 轮迭代_`
            }

            setMessages((prev) => [
              ...prev,
              {
                id: Date.now() + Math.random(),
                session_id: sessionId || undefined,
                role: 'assistant',
                content: completeContent,
                timestamp: new Date().toISOString()
              }
            ])
            setIsLoading(false)
            break
          }

          case 'cancelled':
            setIsLoading(false)
            appendThinking(t('message.stoppedInline'))
            break

          case 'hint_queued': {
            const cid = data.data?.client_id
            if (cid) {
              setMessages(prev => prev.map(m =>
                String(m.id) === String(cid)
                  ? { ...m, hintStatus: 'queued' as const }
                  : m
              ))
            }
            appendThinking(t('message.queuedInline'))
            break
          }

          case 'hint_injected': {
            removeThinking(t('message.queuedInline'))
            appendThinking(t('message.injectedInline'))
            setMessages(prev => prev.map(m =>
              m.hintStatus === 'queued' ? { ...m, hintStatus: 'injected' as const } : m
            ))
            setTimeout(() => {
              removeThinking(t('message.injectedInline'))
            }, 2000)
            break
          }

          case 'status_update': {
            const raw = data.data
            let parsed: Record<string, any> = {}
            if (typeof raw === 'string') {
              try { parsed = JSON.parse(raw) } catch { parsed = {} }
            } else if (raw && typeof raw === 'object') {
              parsed = raw as Record<string, any>
            }
            setStatusline({
              hostname: parsed.hostname,
              cwd: parsed.cwd,
              user: parsed.user,
              model: parsed.model,
              usageRatio: typeof parsed.usage_ratio === 'number' ? parsed.usage_ratio : 0,
            })
            break
          }

          case 'directory_changed': {
            // v6.0 单会话架构：后端工作目录已切换
            // 通知全局事件（DirectoryBar 等组件监听）
            window.dispatchEvent(new CustomEvent('helen-event', {
              detail: { type: 'directory_changed', data: data.data }
            }))
            // 同时更新 chatStore 的 currentSessionId —— 触发 WebSocket 重建，
            // 后续消息保存到新目录的 per-project DB。
            const newSessionId = data.data?.session_id || data.data?.cwd
            if (newSessionId && newSessionId !== sessionId) {
              useChatStore.getState().setCurrentSession(newSessionId)
            }
            break
          }

          case 'clear_messages':
          case 'reload_messages': {
            // /clear:transcript 已插入 BoundaryMarker,清空显示
            // v6.1:不再从 DB 重载(transcript 是唯一数据源,已空)
            setMessages([])
            setIsLoading(false)
            break
          }

          case 'error': {
            const d = data.data || {}
            const text = d.i18n_key ? t(d.i18n_key, d.params || {}) : (d.content || content)
            appendThinking(`⚠️ ${text}`)
            setIsLoading(false)
            break
          }
        }
      },
      {
        onOpen: async () => {
          setIsConnected(true)
          // v6.0 单会话架构：不再发送 __helen_resume__ / __helen_init__
          // 会话恢复由 ChatSession.main 内部使用 get_session_id() resume 处理
          // Re-sync: WS 重连后恢复 isLoading 状态（页面刷新/网络中断恢复时按钮能正确显示）
          try {
            const status = await api.chat.getStatus()
            setIsLoading(status.is_processing)
          } catch {
            // 查询失败保持当前状态
          }
        },
        onClose: () => {
          setIsConnected(false)
        },
        onError: () => setIsConnected(false)
      }
    )

    wsManager.connect()
    wsManagerRef.current = wsManager

    return () => {
      wsManager.disconnect()
      wsManagerRef.current = null
    }
  }, [sessionId])

  // 追加 thinking（中间过程）消息
  const appendThinking = useCallback((content: string) => {
    setMessages((prev) => [
      ...prev,
      {
        id: Date.now() + Math.random(),
        session_id: sessionId || undefined,
        role: 'thinking',
        content,
        timestamp: new Date().toISOString()
      }
    ])
  }, [sessionId])

  // 移除特定内容的 thinking 消息
  const removeThinking = useCallback((content: string) => {
    setMessages((prev) => prev.filter(m => !(m.role === 'thinking' && m.content === content)))
  }, [])

  // 请求中断当前 LLM 流
  const stopGeneration = useCallback(() => {
    if (!wsManagerRef.current || !isConnected) return
    wsManagerRef.current.send({ type: 'cancel' })
  }, [isConnected])

  const sendMessage = useCallback((content: string, attachments?: Attachment[]) => {
    if (!wsManagerRef.current || !isConnected) {
      console.error('WebSocket not connected')
      return
    }
    if (!content.trim() && (!attachments || attachments.length === 0)) return

    const messageId = Date.now() + Math.floor(Math.random() * 1000)
    const isHint = isLoading

    const userMessage: Message = {
      id: messageId,
      session_id: sessionId || undefined,
      role: 'user',
      content,
      timestamp: new Date().toISOString(),
      isHint,
      hintStatus: isHint ? 'queued' : undefined,
      client_id: isHint ? String(messageId) : undefined,
      attachments: attachments && attachments.length > 0 ? attachments : undefined,
    }

    setMessages((prev) => [...prev, userMessage])

    const wsMessage: Record<string, any> = {
      type: isHint ? 'hint' : 'message',
      content,
      client_id: String(messageId)
    }
    // v6.2 多模态：附带附件 upload_id 列表（只发 ID，不发完整对象）
    if (attachments && attachments.length > 0) {
      wsMessage.attachments = attachments.map(a => a.id)
    }
    wsManagerRef.current.send(wsMessage)

    if (!isHint) {
      setIsLoading(true)
    }
  }, [sessionId, isConnected, isLoading])

  return {
    messages,
    sendMessage,
    stopGeneration,
    isLoading,
    isConnected,
    statusline
  }
}

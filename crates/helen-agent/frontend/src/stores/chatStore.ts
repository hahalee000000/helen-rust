import { create } from 'zustand'
import { Session, Message } from '@/types'
import { api } from '@/services/api'

/**
 * v6.0 单会话架构：移除了 Helen session ID 的 localStorage 追踪
 * clearHelenSessionId 不再需要 — 会话恢复由后端 ChatSession.main 内部处理
 * 
 * v6.3: 将 messages 状态提升到全局 store，解决路由切换时消息丢失问题
 */

interface ChatStore {
  sessions: Session[]
  currentSessionId: string | null
  isLoading: boolean
  error: string | null
  messages: Message[]  // v6.3: 全局消息状态，路由切换时保持

  // Actions
  fetchSessions: () => Promise<void>
  setCurrentSession: (sessionId: string | null) => void
  deleteSession: (sessionId: string) => Promise<void>
  clearError: () => void
  // v6.3: 消息管理
  setMessages: (messages: Message[]) => void
  addMessage: (message: Message) => void
  updateLastMessage: (updater: (msg: Message) => Message) => void
  clearMessages: () => void
}

export const useChatStore = create<ChatStore>((set, get) => ({
  sessions: [],
  currentSessionId: null,
  isLoading: false,
  error: null,
  messages: [],  // v6.3: 初始化为空数组

  fetchSessions: async () => {
    set({ isLoading: true, error: null })
    try {
      const sessions = await api.sessions.list()
      set({ sessions, isLoading: false })
    } catch (error) {
      set({ error: (error as Error).message, isLoading: false })
    }
  },

  setCurrentSession: (sessionId) => {
    set({ currentSessionId: sessionId })
  },

  deleteSession: async (sessionId) => {
    set({ isLoading: true, error: null })
    try {
      await api.sessions.delete(sessionId)
      await get().fetchSessions()

      // 如果删除的是当前会话，清除选中
      if (get().currentSessionId === sessionId) {
        set({ currentSessionId: null })
      }

      set({ isLoading: false })
    } catch (error) {
      set({ error: (error as Error).message, isLoading: false })
    }
  },

  clearError: () => {
    set({ error: null })
  },

  // v6.3: 消息管理方法
  setMessages: (messages) => {
    set({ messages })
  },

  addMessage: (message) => {
    set((state) => ({ messages: [...state.messages, message] }))
  },

  updateLastMessage: (updater) => {
    set((state) => {
      const last = state.messages[state.messages.length - 1]
      if (!last) return state
      return {
        messages: [...state.messages.slice(0, -1), updater(last)]
      }
    })
  },

  clearMessages: () => {
    set({ messages: [] })
  }
}))

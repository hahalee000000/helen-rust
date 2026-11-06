import { describe, it, expect, beforeEach } from 'vitest'
import { useChatStore } from './chatStore'
import { Message } from '@/types'

/**
 * M25.20 regression tests: messages should persist across route navigation
 * 
 * Root cause: messages were stored in component-local state (useChat hook),
 * causing them to be lost when navigating between pages.
 * 
 * Fix: Lift messages to global chatStore (Zustand) so they persist across
 * component unmount/remount cycles.
 */

describe('chatStore - message persistence (M25.20)', () => {
  beforeEach(() => {
    // Reset store state before each test
    useChatStore.setState({
      sessions: [],
      currentSessionId: null,
      isLoading: false,
      error: null,
      messages: []
    })
  })

  describe('setMessages', () => {
    it('replaces all messages', () => {
      const messages: Message[] = [
        { id: '1', role: 'user', content: 'hello', timestamp: Date.now() },
        { id: '2', role: 'assistant', content: 'hi', timestamp: Date.now() }
      ]

      useChatStore.getState().setMessages(messages)

      expect(useChatStore.getState().messages).toEqual(messages)
      expect(useChatStore.getState().messages).toHaveLength(2)
    })

    it('can clear messages by setting empty array', () => {
      const messages: Message[] = [
        { id: '1', role: 'user', content: 'hello', timestamp: Date.now() }
      ]
      useChatStore.getState().setMessages(messages)
      expect(useChatStore.getState().messages).toHaveLength(1)

      useChatStore.getState().setMessages([])
      expect(useChatStore.getState().messages).toHaveLength(0)
    })
  })

  describe('addMessage', () => {
    it('appends a message to existing messages', () => {
      const msg1: Message = { id: '1', role: 'user', content: 'first', timestamp: Date.now() }
      const msg2: Message = { id: '2', role: 'assistant', content: 'second', timestamp: Date.now() }

      useChatStore.getState().addMessage(msg1)
      expect(useChatStore.getState().messages).toHaveLength(1)
      expect(useChatStore.getState().messages[0]).toEqual(msg1)

      useChatStore.getState().addMessage(msg2)
      expect(useChatStore.getState().messages).toHaveLength(2)
      expect(useChatStore.getState().messages[1]).toEqual(msg2)
    })

    it('works when messages array is initially empty', () => {
      const msg: Message = { id: '1', role: 'user', content: 'hello', timestamp: Date.now() }
      
      useChatStore.getState().addMessage(msg)
      
      expect(useChatStore.getState().messages).toHaveLength(1)
      expect(useChatStore.getState().messages[0]).toEqual(msg)
    })
  })

  describe('updateLastMessage', () => {
    it('updates the last message using updater function', () => {
      const msg1: Message = { id: '1', role: 'user', content: 'hello', timestamp: Date.now() }
      const msg2: Message = { id: '2', role: 'assistant', content: 'partial', timestamp: Date.now() }

      useChatStore.getState().setMessages([msg1, msg2])

      // Update last message (streaming scenario)
      useChatStore.getState().updateLastMessage((msg) => ({
        ...msg,
        content: msg.content + ' response'
      }))

      const messages = useChatStore.getState().messages
      expect(messages).toHaveLength(2)
      expect(messages[0].content).toBe('hello') // unchanged
      expect(messages[1].content).toBe('partial response') // updated
    })

    it('does nothing when messages array is empty', () => {
      useChatStore.getState().updateLastMessage((msg) => ({
        ...msg,
        content: 'updated'
      }))

      expect(useChatStore.getState().messages).toHaveLength(0)
    })

    it('preserves other fields when updating', () => {
      const msg: Message = { 
        id: '1', 
        role: 'assistant', 
        content: 'initial', 
        timestamp: 1234567890 
      }

      useChatStore.getState().setMessages([msg])
      useChatStore.getState().updateLastMessage((m) => ({
        ...m,
        content: 'updated'
      }))

      const updated = useChatStore.getState().messages[0]
      expect(updated.id).toBe('1')
      expect(updated.role).toBe('assistant')
      expect(updated.timestamp).toBe(1234567890)
      expect(updated.content).toBe('updated')
    })
  })

  describe('clearMessages', () => {
    it('clears all messages', () => {
      const messages: Message[] = [
        { id: '1', role: 'user', content: 'msg1', timestamp: Date.now() },
        { id: '2', role: 'assistant', content: 'msg2', timestamp: Date.now() },
        { id: '3', role: 'user', content: 'msg3', timestamp: Date.now() }
      ]

      useChatStore.getState().setMessages(messages)
      expect(useChatStore.getState().messages).toHaveLength(3)

      useChatStore.getState().clearMessages()
      expect(useChatStore.getState().messages).toHaveLength(0)
    })
  })

  describe('message persistence across simulated route changes', () => {
    it('messages persist when store is accessed multiple times (simulates remount)', () => {
      // Simulate: User sends messages on Chat page
      const msg1: Message = { id: '1', role: 'user', content: 'hello', timestamp: Date.now() }
      const msg2: Message = { id: '2', role: 'assistant', content: 'hi there', timestamp: Date.now() }
      
      useChatStore.getState().addMessage(msg1)
      useChatStore.getState().addMessage(msg2)

      // Simulate: User navigates to Settings page (component unmounts)
      // Store state should remain intact
      const stateAfterUnmount = useChatStore.getState()
      expect(stateAfterUnmount.messages).toHaveLength(2)

      // Simulate: User navigates back to Chat page (component remounts)
      // Messages should still be there
      const stateAfterRemount = useChatStore.getState()
      expect(stateAfterRemount.messages).toHaveLength(2)
      expect(stateAfterRemount.messages[0]).toEqual(msg1)
      expect(stateAfterRemount.messages[1]).toEqual(msg2)
    })

    it('streaming response persists across simulated route changes', () => {
      // Simulate: User sends message, assistant starts streaming
      const userMsg: Message = { id: '1', role: 'user', content: 'tell me a story', timestamp: Date.now() }
      const assistantMsg: Message = { id: '2', role: 'assistant', content: 'Once upon', timestamp: Date.now() }

      useChatStore.getState().addMessage(userMsg)
      useChatStore.getState().addMessage(assistantMsg)

      // Simulate: User navigates away while streaming
      // (In real app, WebSocket continues in background)

      // Simulate: Streaming updates continue
      useChatStore.getState().updateLastMessage((msg) => ({
        ...msg,
        content: msg.content + ' a time...'
      }))

      // Simulate: User navigates back
      const messages = useChatStore.getState().messages
      expect(messages).toHaveLength(2)
      expect(messages[1].content).toBe('Once upon a time...')
    })

    it('multiple addMessage calls accumulate correctly', () => {
      // Simulate a conversation with multiple exchanges
      for (let i = 0; i < 5; i++) {
        useChatStore.getState().addMessage({
          id: `user-${i}`,
          role: 'user',
          content: `message ${i}`,
          timestamp: Date.now()
        })
        useChatStore.getState().addMessage({
          id: `assistant-${i}`,
          role: 'assistant',
          content: `response ${i}`,
          timestamp: Date.now()
        })
      }

      expect(useChatStore.getState().messages).toHaveLength(10)
      
      // Verify order is preserved
      const messages = useChatStore.getState().messages
      expect(messages[0].id).toBe('user-0')
      expect(messages[1].id).toBe('assistant-0')
      expect(messages[8].id).toBe('user-4')
      expect(messages[9].id).toBe('assistant-4')
    })
  })
})

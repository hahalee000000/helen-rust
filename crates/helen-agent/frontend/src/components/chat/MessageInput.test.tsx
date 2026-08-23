import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@/test/test-utils'
import { MessageInput } from './MessageInput'

// 默认语言的 placeholder(英文)
const MSG_PLACEHOLDER = 'Type a message... (Shift+Enter for newline)'
const HINT_PLACEHOLDER = 'Type a 💡 hint, press Enter or click Append (won\'t interrupt current generation)...'

describe('MessageInput - basic', () => {
  it('renders textarea and send button', () => {
    const onSend = vi.fn()
    render(<MessageInput onSend={onSend} />)

    expect(screen.getByPlaceholderText(MSG_PLACEHOLDER)).toBeInTheDocument()
    // send 按钮是 Send 图标,没有文本;用 form submit 按钮查询
    expect(screen.getByRole('button', { name: '' })).toBeInTheDocument()
  })

  it('calls onSend when user submits a non-empty message', async () => {
    const onSend = vi.fn()
    render(<MessageInput onSend={onSend} />)

    const textarea = screen.getByPlaceholderText(MSG_PLACEHOLDER)
    fireEvent.change(textarea, { target: { value: 'hello' } })
    fireEvent.submit(textarea.closest('form')!)

    await waitFor(() => expect(onSend).toHaveBeenCalledWith('hello', undefined))
  })

  it('does NOT call onSend when message is empty', () => {
    const onSend = vi.fn()
    render(<MessageInput onSend={onSend} />)
    const textarea = screen.getByPlaceholderText(MSG_PLACEHOLDER)
    fireEvent.submit(textarea.closest('form')!)
    expect(onSend).not.toHaveBeenCalled()
  })
})

describe('MessageInput - history navigation (↑/↓)', () => {
  it('ArrowUp with empty history does nothing', () => {
    const onSend = vi.fn()
    render(<MessageInput onSend={onSend} />)

    const textarea = screen.getByPlaceholderText(MSG_PLACEHOLDER)
    fireEvent.keyDown(textarea, { key: 'ArrowUp' })
    expect(textarea).toHaveValue('')
  })

  it('ArrowUp recalls the most recent sent message', async () => {
    const onSend = vi.fn()
    render(<MessageInput onSend={onSend} />)

    const textarea = screen.getByPlaceholderText(MSG_PLACEHOLDER)

    // 发送 "first"
    fireEvent.change(textarea, { target: { value: 'first' } })
    fireEvent.submit(textarea.closest('form')!)
    await waitFor(() => expect(onSend).toHaveBeenCalledWith('first', undefined))

    // 发送 "second"
    fireEvent.change(textarea, { target: { value: 'second' } })
    fireEvent.submit(textarea.closest('form')!)
    await waitFor(() => expect(onSend).toHaveBeenCalledWith('second', undefined))

    // 输入框已被清空
    expect(textarea).toHaveValue('')

    // ↑ 第一次 → "second" (最新)
    fireEvent.keyDown(textarea, { key: 'ArrowUp' })
    expect(textarea).toHaveValue('second')

    // ↑ 第二次 → "first"
    fireEvent.keyDown(textarea, { key: 'ArrowUp' })
    expect(textarea).toHaveValue('first')

    // ↑ 第三次 → 停留在 "first" (历史最老)
    fireEvent.keyDown(textarea, { key: 'ArrowUp' })
    expect(textarea).toHaveValue('first')
  })

  it('ArrowDown returns to draft after ArrowUp', async () => {
    const onSend = vi.fn()
    render(<MessageInput onSend={onSend} />)

    const textarea = screen.getByPlaceholderText(MSG_PLACEHOLDER)

    // 发送一条历史
    fireEvent.change(textarea, { target: { value: 'history entry' } })
    fireEvent.submit(textarea.closest('form')!)
    await waitFor(() => expect(onSend).toHaveBeenCalled())

    // 输入草稿
    fireEvent.change(textarea, { target: { value: 'my draft' } })

    // ↑ → 历史
    fireEvent.keyDown(textarea, { key: 'ArrowUp' })
    expect(textarea).toHaveValue('history entry')

    // ↓ → 回到草稿
    fireEvent.keyDown(textarea, { key: 'ArrowDown' })
    expect(textarea).toHaveValue('my draft')

    // ↓ 再按 → 仍在草稿 (不会再变)
    fireEvent.keyDown(textarea, { key: 'ArrowDown' })
    expect(textarea).toHaveValue('my draft')
  })

  it('typing while in history mode exits history navigation', async () => {
    const onSend = vi.fn()
    render(<MessageInput onSend={onSend} />)

    const textarea = screen.getByPlaceholderText(MSG_PLACEHOLDER)

    // 发送一条
    fireEvent.change(textarea, { target: { value: 'original' } })
    fireEvent.submit(textarea.closest('form')!)
    await waitFor(() => expect(onSend).toHaveBeenCalled())

    // ↑ 召回
    fireEvent.keyDown(textarea, { key: 'ArrowUp' })
    expect(textarea).toHaveValue('original')

    // 编辑(触发 onChange) → 退出历史模式
    fireEvent.change(textarea, { target: { value: 'original + edit' } })

    // ↑ → 再次从最新历史开始,不会再深入
    fireEvent.keyDown(textarea, { key: 'ArrowUp' })
    expect(textarea).toHaveValue('original')
  })

  it('consecutive duplicate messages are NOT duplicated in history', async () => {
    const onSend = vi.fn()
    render(<MessageInput onSend={onSend} />)

    const textarea = screen.getByPlaceholderText(MSG_PLACEHOLDER)

    // 连续发送相同消息两次
    for (let i = 0; i < 2; i++) {
      fireEvent.change(textarea, { target: { value: 'same' } })
      fireEvent.submit(textarea.closest('form')!)
      await waitFor(() => expect(onSend).toHaveBeenCalled())
    }

    // ↑ 第一次 → "same"
    fireEvent.keyDown(textarea, { key: 'ArrowUp' })
    expect(textarea).toHaveValue('same')

    // ↑ 第二次 → 没有更早的条目,停留在 "same"
    fireEvent.keyDown(textarea, { key: 'ArrowUp' })
    expect(textarea).toHaveValue('same')
  })

  it('hint submit (Enter during isLoading) does NOT add to history', () => {
    const onSend = vi.fn()
    render(<MessageInput onSend={onSend} isLoading={true} />)

    const textarea = screen.getByPlaceholderText(HINT_PLACEHOLDER)
    fireEvent.change(textarea, { target: { value: 'hint content' } })
    // isLoading 模式下 Enter 发送 hint
    fireEvent.keyDown(textarea, { key: 'Enter' })
    expect(onSend).toHaveBeenCalledWith('hint content')

    // ↑ 不应召回 hint
    fireEvent.keyDown(textarea, { key: 'ArrowUp' })
    expect(textarea).toHaveValue('')
  })

  it('ArrowDown without prior ArrowUp does nothing', () => {
    const onSend = vi.fn()
    render(<MessageInput onSend={onSend} />)

    const textarea = screen.getByPlaceholderText(MSG_PLACEHOLDER)
    fireEvent.change(textarea, { target: { value: 'draft' } })
    fireEvent.keyDown(textarea, { key: 'ArrowDown' })
    expect(textarea).toHaveValue('draft')
  })
})

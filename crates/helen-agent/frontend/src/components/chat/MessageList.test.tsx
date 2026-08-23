import { describe, it, expect } from 'vitest'
import { render, screen } from '@/test/test-utils'
import { MessageList } from './MessageList'
import { Message } from '@/types'

describe('MessageList', () => {
  it('renders empty state', () => {
    render(<MessageList messages={[]} />)
    expect(screen.getByText('Start a conversation!')).toBeInTheDocument()
  })

  it('renders messages', () => {
    const messages: Message[] = [
      {
        id: 1,
        role: 'user',
        content: '你好',
        timestamp: '2024-01-01T00:00:00Z'
      },
      {
        id: 2,
        role: 'assistant',
        content: '你好！有什么可以帮助你的？',
        timestamp: '2024-01-01T00:00:01Z'
      }
    ]

    render(<MessageList messages={messages} />)

    expect(screen.getByText('你好')).toBeInTheDocument()
    expect(screen.getByText('你好！有什么可以帮助你的？')).toBeInTheDocument()
  })

  it('renders user messages on the right', () => {
    const messages: Message[] = [
      {
        id: 1,
        role: 'user',
        content: '用户消息',
        timestamp: '2024-01-01T00:00:00Z'
      }
    ]

    render(<MessageList messages={messages} />)

    // 用户消息容器使用 ml-auto 把气泡推到右边缘
    const messageWrapper = screen.getByText('用户消息').closest('.max-w-\\[80\\%\\]')
    expect(messageWrapper).toHaveClass('ml-auto')
  })

  it('renders assistant messages on the left', () => {
    const messages: Message[] = [
      {
        id: 1,
        role: 'assistant',
        content: '助手消息',
        timestamp: '2024-01-01T00:00:00Z'
      }
    ]

    render(<MessageList messages={messages} />)

    // 助手消息容器使用 flex-1 占满空间，没有 ml-auto
    const messageWrapper = screen.getByText('助手消息').closest('.max-w-\\[80\\%\\]')
    expect(messageWrapper).toHaveClass('flex-1')
    expect(messageWrapper).not.toHaveClass('ml-auto')
  })

  it('renders message attachments', () => {
    const messages: Message[] = [
      {
        id: 1,
        role: 'user',
        content: '看看这张图',
        timestamp: '2024-01-01T00:00:00Z',
        attachments: [
          {
            id: 'upload-1',
            filename: 'test.png',
            mime_type: 'image/png',
            size: 1024,
            url: '/api/chat/uploads/upload-1/file'
          }
        ]
      }
    ]

    render(<MessageList messages={messages} />)
    const img = screen.getByRole('img', { name: /test\.png/i })
    expect(img).toBeInTheDocument()
    expect(img).toHaveAttribute('src', '/api/chat/uploads/upload-1/file')
  })

  it('renders message without attachments normally', () => {
    const messages: Message[] = [
      {
        id: 1,
        role: 'user',
        content: '普通消息',
        timestamp: '2024-01-01T00:00:00Z'
      }
    ]

    render(<MessageList messages={messages} />)
    expect(screen.getByText('普通消息')).toBeInTheDocument()
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
  })
})

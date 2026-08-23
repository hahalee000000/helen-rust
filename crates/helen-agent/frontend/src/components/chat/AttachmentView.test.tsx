import { describe, it, expect } from 'vitest'
import { render, screen } from '@/test/test-utils'
import { AttachmentView } from './AttachmentView'
import { Attachment } from '@/types'

describe('AttachmentView', () => {
  it('renders image attachment as <img>', () => {
    const attachment: Attachment = {
      id: 'upload-1',
      filename: 'test.png',
      mime_type: 'image/png',
      size: 1024,
      url: '/api/chat/uploads/upload-1/file'
    }
    render(<AttachmentView attachment={attachment} />)
    const img = screen.getByRole('img', { name: /test\.png/i })
    expect(img).toBeInTheDocument()
    expect(img).toHaveAttribute('src', '/api/chat/uploads/upload-1/file')
  })

  it('renders audio attachment with controls', () => {
    const attachment: Attachment = {
      id: 'upload-2',
      filename: 'audio.mp3',
      mime_type: 'audio/mpeg',
      size: 2048,
      url: '/api/chat/uploads/upload-2/file'
    }
    const { container } = render(<AttachmentView attachment={attachment} />)
    const audio = container.querySelector('audio')
    expect(audio).toBeInTheDocument()
    expect(audio).toHaveAttribute('controls')
    expect(audio).toHaveAttribute('src', '/api/chat/uploads/upload-2/file')
  })

  it('renders video attachment with controls', () => {
    const attachment: Attachment = {
      id: 'upload-3',
      filename: 'video.mp4',
      mime_type: 'video/mp4',
      size: 10240,
      url: '/api/chat/uploads/upload-3/file'
    }
    const { container } = render(<AttachmentView attachment={attachment} />)
    const video = container.querySelector('video')
    expect(video).toBeInTheDocument()
    expect(video).toHaveAttribute('controls')
  })

  it('renders unknown file type as download link', () => {
    const attachment: Attachment = {
      id: 'upload-4',
      filename: 'document.pdf',
      mime_type: 'application/pdf',
      size: 5000,
      url: '/api/chat/uploads/upload-4/file'
    }
    render(<AttachmentView attachment={attachment} />)
    const link = screen.getByRole('link', { name: /document\.pdf/i })
    expect(link).toBeInTheDocument()
    expect(link).toHaveAttribute('href', '/api/chat/uploads/upload-4/file')
    expect(link).toHaveAttribute('download', 'document.pdf')
  })
})

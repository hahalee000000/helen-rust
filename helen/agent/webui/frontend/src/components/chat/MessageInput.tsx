import { useState, useRef } from 'react'
import { Send, Square, Lightbulb, Paperclip, X } from 'lucide-react'
import { api } from '@/services/api'
import { Attachment } from '@/types'
import { useT } from '@/i18n'

interface MessageInputProps {
  onSend: (message: string, attachments?: Attachment[]) => void
  onStop?: () => void
  disabled?: boolean
  isLoading?: boolean
}

// 历史输入最大保留条数
const HISTORY_MAX = 100

export function MessageInput({ onSend, onStop, disabled, isLoading }: MessageInputProps) {
  const [input, setInput] = useState('')
  const [pendingFiles, setPendingFiles] = useState<File[]>([])
  const [uploading, setUploading] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)
  const t = useT()
  // ── 历史输入导航 ──
  // history: 最近已发送的消息,最新在前 (history[0] = 最新)
  const [history, setHistory] = useState<string[]>([])
  // historyIndexRef: 当前在 history 中的位置,-1 表示不在导航模式
  const historyIndexRef = useRef(-1)
  // savedDraftRef: 进入导航模式前保存的当前输入草稿
  //   用户按 ArrowDown 一路回到底时,恢复这个草稿
  const savedDraftRef = useRef('')

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if ((!input.trim() && pendingFiles.length === 0) || disabled || isLoading) return

    // 记录到历史(仅完整发送,不含 hint;保留去重:与最近一条相同则跳过)
    if (input.trim()) {
      setHistory(prev => {
        const next = prev[0] === input ? prev : [input, ...prev]
        return next.slice(0, HISTORY_MAX)
      })
    }
    historyIndexRef.current = -1
    savedDraftRef.current = ''

    // 上传附件（如果有）
    let attachments: Attachment[] = []
    if (pendingFiles.length > 0) {
      setUploading(true)
      try {
        const uploadResults = await Promise.all(
          pendingFiles.map(file => api.upload.file(file))
        )
        // 构造完整的 Attachment 对象供前端显示
        attachments = uploadResults.map(r => ({
          id: r.upload_id,
          filename: r.filename,
          mime_type: r.mime_type,
          size: r.size,
          url: r.url
        }))
      } catch (err) {
        console.error('Upload failed:', err)
        setUploading(false)
        return
      }
      setUploading(false)
    }

    onSend(input, attachments.length > 0 ? attachments : undefined)
    setInput('')
    setPendingFiles([])
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      if (isLoading) {
        // 处理中 Enter 发送 hint
        handleSendHint()
      } else {
        handleSubmit(e)
      }
      return
    }

    // ── 历史输入导航(↑/↓) ──
    // 仅在非多行编辑时触发(输入内容不含换行,或光标在首/末行)。
    // 为简化 UX,这里采用 shell 约定:无条件拦截 ↑/↓。
    // 多行编辑用 Shift+Enter 插入换行。
    if (e.key === 'ArrowUp') {
      if (history.length === 0) return
      e.preventDefault()
      if (historyIndexRef.current === -1) {
        // 进入导航模式:保存当前草稿
        savedDraftRef.current = input
        historyIndexRef.current = 0
      } else if (historyIndexRef.current < history.length - 1) {
        historyIndexRef.current++
      }
      setInput(history[historyIndexRef.current])
      return
    }

    if (e.key === 'ArrowDown') {
      if (historyIndexRef.current === -1) return
      e.preventDefault()
      const newIndex = historyIndexRef.current - 1
      if (newIndex < 0) {
        // 回到草稿
        historyIndexRef.current = -1
        setInput(savedDraftRef.current)
        savedDraftRef.current = ''
      } else {
        historyIndexRef.current = newIndex
        setInput(history[historyIndexRef.current])
      }
      return
    }
  }

  const handleSendHint = () => {
    if (!input.trim()) return
    onSend(input)
    setInput('')
  }

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(e.target.files || [])
    setPendingFiles(prev => [...prev, ...files])
    // 清空 input 以便再次选择相同文件
    if (fileInputRef.current) fileInputRef.current.value = ''
  }

  const handleRemoveFile = (index: number) => {
    setPendingFiles(prev => prev.filter((_, i) => i !== index))
  }

  // 格式化文件大小
  const formatSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }

  return (
    <form onSubmit={handleSubmit} className="border-t p-4 bg-card" style={{ backgroundColor: '#EAE9E5' }}>
      {/* 附件预览区 */}
      {pendingFiles.length > 0 && (
        <div className="flex flex-wrap gap-2 mb-2 p-2 border rounded-lg bg-muted/30">
          {pendingFiles.map((file, idx) => (
            <div key={idx} className="relative group">
              {file.type.startsWith('image/') ? (
                <img
                  src={URL.createObjectURL(file)}
                  alt={file.name}
                  className="w-16 h-16 object-cover rounded border"
                />
              ) : (
                <div className="w-16 h-16 flex flex-col items-center justify-center rounded border bg-background text-xs text-center p-1">
                  <span className="text-lg">📄</span>
                  <span className="truncate w-full">{file.name}</span>
                </div>
              )}
              <button
                type="button"
                onClick={() => handleRemoveFile(idx)}
                className="absolute -top-1 -right-1 w-5 h-5 bg-red-500 text-white rounded-full flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
                title={t('message.remove')}
              >
                <X className="w-3 h-3" />
              </button>
              <div className="text-xs text-muted-foreground text-center mt-0.5">
                {formatSize(file.size)}
              </div>
            </div>
          ))}
        </div>
      )}

      <div className="flex gap-2">
        {/* 附件按钮 */}
        <input
          ref={fileInputRef}
          type="file"
          multiple
          accept="image/*,audio/*,video/*"
          onChange={handleFileSelect}
          className="hidden"
          data-testid="file-input"
        />
        <button
          type="button"
          onClick={() => fileInputRef.current?.click()}
          disabled={isLoading || uploading}
          className="rounded-lg border border-input bg-background p-2 hover:bg-muted transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          title={t('message.attach')}
          aria-label={t('message.attachment')}
        >
          <Paperclip className="h-5 w-5" />
        </button>

        <textarea
          value={input}
          onChange={(e) => {
            setInput(e.target.value)
            // 用户主动编辑 → 退出历史导航模式(保留输入内容,下次 ↑ 从最新历史重新开始)
            historyIndexRef.current = -1
            savedDraftRef.current = ''
          }}
          onKeyDown={handleKeyDown}
          placeholder={isLoading
            ? t('message.hintPlaceholder')
            : uploading
              ? t('message.uploading')
              : t('message.placeholder')}
          disabled={disabled || uploading}
          rows={3}
          className="flex-1 resize-none rounded-lg border border-input bg-background px-4 py-2 focus:outline-none focus:ring-2 focus:ring-ring disabled:opacity-50"
          style={{ minHeight: '40px', maxHeight: '200px' }}
        />
        {isLoading ? (
          <>
            <button
              type="button"
              onClick={handleSendHint}
              disabled={!input.trim()}
              className="rounded-lg bg-amber-500 px-3 py-2 text-white hover:bg-amber-600 disabled:opacity-50 disabled:cursor-not-allowed transition-colors flex items-center gap-1"
              title={t('message.appendTitle')}
            >
              <Lightbulb className="h-4 w-4" />
              <span className="text-sm">{t('message.hint')}</span>
            </button>
            {onStop && (
              <button
                type="button"
                onClick={onStop}
                className="rounded-lg bg-red-500 px-3 py-2 text-white hover:bg-red-600 transition-colors"
                title={t('message.stopTitle')}
              >
                <Square className="h-4 w-4 fill-current" />
              </button>
            )}
          </>
        ) : (
          <button
            type="submit"
            disabled={disabled || uploading || (!input.trim() && pendingFiles.length === 0)}
            className="rounded-lg bg-primary px-4 py-2 text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {uploading ? t('message.uploading') : <Send className="h-5 w-5" />}
          </button>
        )}
      </div>
    </form>
  )
}


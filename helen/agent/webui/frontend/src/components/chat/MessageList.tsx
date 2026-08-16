import { useState, useEffect } from 'react'
import { Message } from '@/types'
import { formatTime } from '@/utils/format'
import { User, Wrench, CheckCircle2 } from 'lucide-react'
import { AttachmentView } from './AttachmentView'
import { useT } from '@/i18n'

interface MessageListProps {
  messages: Message[]
}

export function MessageList({ messages }: MessageListProps) {
  const t = useT()
  if (messages.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-muted-foreground gap-4">
        <img src="/helen-logo-128.png" alt="Helen" className="w-24 h-24 rounded-2xl opacity-50" />
        <p>{t('chat.empty')}</p>
      </div>
    )
  }

  return (
    <div className="space-y-2">
      {messages.map((message) => (
        <MessageItem key={message.id} message={message} />
      ))}
    </div>
  )
}

function MessageItem({ message }: { message: Message }) {
  // Thinking 消息（中间过程）：单行灰色样式
  if (message.role === 'thinking') {
    return (
      <div className="flex gap-2 text-sm text-muted-foreground font-mono">
        <span className="whitespace-pre-wrap">{message.content}</span>
      </div>
    )
  }

  const isUser = message.role === 'user'

  // 头像样式（user=深色，assistant=浅色）
  const avatarCls = `flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center ${
    isUser ? 'bg-primary text-primary-foreground' : 'bg-secondary text-secondary-foreground overflow-hidden'
  }`
  // 气泡样式（user=深色+inline-block 收缩贴文字；assistant=浅色占满宽度）
  const bubbleCls = `rounded-lg px-4 py-2 ${
    isUser
      ? 'bg-primary text-primary-foreground inline-block text-left'
      : 'bg-secondary text-secondary-foreground'
  }`
  // 时间戳（user=右对齐，贴气泡底部；assistant=左对齐）
  const timeCls = `text-xs text-muted-foreground mt-1 ${isUser ? 'text-right' : ''}`

  // 消息内容容器：
  //   user：flex 行内顺序 = 消息 → 头像，`ml-auto` 把消息容器推向右边缘，
  //         容器不 flex-1（保持 inline-block 收缩宽度），头像紧贴其左。
  //   assistant：flex 行内顺序 = 头像 → 消息，容器 flex-1 占满剩余空间，
  //              inline-block 气泡贴容器左边 = 贴屏幕左边。
  // 注：`flex-row-reverse + flex-1 + justify-end` 在数学上不可行
  //     （flex-row-reverse 的第 2 DOM 项始终在主轴 end 的反方向，
  //      justify-end 在 flex-row-reverse 中把整组推向 left，
  //      消息容器仍在左、头像仍在右 → 不是用户消息期望的布局）。
  //     直接用 DOM 顺序控制最简洁可靠。

  return isUser ? (
    <div className="flex gap-3">
      {/* 消息内容：ml-auto 把容器推到右边缘 */}
      <div className="max-w-[80%] ml-auto">
        {/* v6.2 多模态：渲染附件 */}
        {message.attachments && message.attachments.length > 0 && (
          <div className="flex flex-wrap gap-2 mb-2">
            {message.attachments.map((att) => (
              <AttachmentView key={att.id} attachment={att} />
            ))}
          </div>
        )}
        <div className={bubbleCls}>
          <div className="prose prose-sm max-w-none">
            <Markdownish content={message.content} />
          </div>
        </div>
        <div className={timeCls}>{formatTime(message.timestamp)}</div>
      </div>
      {/* 头像：紧贴消息容器左侧 */}
      <div className={avatarCls}>
        <User className="w-4 h-4" />
      </div>
    </div>
  ) : (
    <div className="flex gap-3">
      {/* 头像：屏幕左边，使用 Helen logo */}
      <div className={avatarCls}>
        <img src="/helen-logo-64.png" alt="Helen" className="w-full h-full object-cover" />
      </div>
      {/* 消息内容：flex-1 占满剩余空间，气泡 inline-block 贴左 */}
      <div className="flex-1 max-w-[80%]">
        {/* v6.2 多模态：渲染附件 */}
        {message.attachments && message.attachments.length > 0 && (
          <div className="flex flex-wrap gap-2 mb-2">
            {message.attachments.map((att) => (
              <AttachmentView key={att.id} attachment={att} />
            ))}
          </div>
        )}
        <div className={bubbleCls}>
          <div className="prose prose-sm max-w-none">
            <AssistantContent content={message.content} />
          </div>
        </div>
        <div className={timeCls}>{formatTime(message.timestamp)}</div>
      </div>
    </div>
  )
}

// ── Assistant 内容渲染：分离普通文本 / 工具调用 / 工具结果 ──

interface Segment {
  type: 'text' | 'tool_call' | 'tool_result'
  content: string
}

/** 把 assistant 消息内容拆成 文本/工具调用/工具结果 三段 */
function parseAssistantContent(content: string): Segment[] {
  const segments: Segment[] = []
  const lines = content.split('\n')
  let buffer: string[] = []
  let i = 0

  while (i < lines.length) {
    const line = lines[i]

    // 工具调用行：🔧 Calling tool_name(params)...
    if (line.startsWith('🔧 Calling ')) {
      // 先把之前累积的普通文本刷出去
      if (buffer.length > 0) {
        segments.push({ type: 'text', content: buffer.join('\n') })
        buffer = []
      }
      // 收集多行调用（如果下一行还是同一调用的续行）
      let callText = line
      while (i + 1 < lines.length && !lines[i + 1].startsWith('✅') && !lines[i + 1].startsWith('🔧 Calling ') && !isPlainTextLine(lines[i + 1])) {
        i++
        callText += '\n' + lines[i]
      }
      segments.push({ type: 'tool_call', content: callText })
      i++
      continue
    }

    // 工具结果行：✅ tool_name returned: {...}
    if (line.startsWith('✅')) {
      if (buffer.length > 0) {
        segments.push({ type: 'text', content: buffer.join('\n') })
        buffer = []
      }
      segments.push({ type: 'tool_result', content: line })
      i++
      continue
    }

    buffer.push(line)
    i++
  }

  if (buffer.length > 0) {
    segments.push({ type: 'text', content: buffer.join('\n') })
  }

  return segments
}

/** 判断一行是否属于普通文本（不是工具调用续行） */
function isPlainTextLine(line: string): boolean {
  if (line.trim() === '') return true
  // 工具调用续行的特征：以参数键=值形式开头，或仍在函数参数括号内
  // 简化判断：如果下一行不是 🔧/✅ 开头，且当前行没有闭合的函数调用特征，就认为是续行
  // 保守策略：让 parser 把非 🔧/✅ 的都归入 buffer，工具调用只取单行
  return true
}

function AssistantContent({ content }: { content: string }) {
  const segments = parseAssistantContent(content)

  return (
    <>
      {segments.map((seg, idx) => {
        switch (seg.type) {
          case 'text':
            return seg.content.trim() ? (
              <Markdownish key={idx} content={seg.content} />
            ) : null
          case 'tool_call':
            return <ToolCallView key={idx} raw={seg.content} />
          case 'tool_result':
            return <ToolResultView key={idx} raw={seg.content} />
          default:
            return null
        }
      })}
    </>
  )
}

// ── 工具调用视图 ──

interface ToolCallInfo {
  name: string
  args: Record<string, string>
}

function parseToolCallLine(line: string): ToolCallInfo | null {
  const m = line.match(/^🔧 Calling (\w+)\((.*)$/)
  if (!m) return null
  const name = m[1]
  const argsStr = m[2].replace(/\)\s*$/, '').replace(/\)\.\.\.$/, '') // 去掉尾部 ) 或 )...
  const args = parseKwargs(argsStr)
  return { name, args }
}

/** 解析 key='value' 或 key="value" 形式的参数串（允许 value 内转义） */
function parseKwargs(s: string): Record<string, string> {
  const result: Record<string, string> = {}
  let i = 0
  while (i < s.length) {
    // 跳过空白和逗号
    while (i < s.length && /[\s,]/.test(s[i])) i++
    if (i >= s.length) break

    // 读 key
    let key = ''
    while (i < s.length && s[i] !== '=') {
      key += s[i]
      i++
    }
    if (i >= s.length) break
    i++ // 跳过 '='
    key = key.trim()

    // 读 value（带引号）
    const quote = s[i]
    if (quote !== '"' && quote !== "'") break
    i++
    let value = ''
    let escaped = false
    while (i < s.length) {
      const ch = s[i]
      if (escaped) {
        // 转义字符
        if (ch === 'n') value += '\n'
        else if (ch === 't') value += '\t'
        else value += ch
        escaped = false
      } else if (ch === '\\') {
        escaped = true
      } else if (ch === quote) {
        i++
        break
      } else {
        value += ch
      }
      i++
    }
    result[key] = value
  }
  return result
}

function ToolCallView({ raw }: { raw: string }) {
  const t = useT()
  const firstLine = raw.split('\n')[0]
  const info = parseToolCallLine(firstLine)
  if (!info) {
    // Parse failed, fall back to raw display (dim style)
    return (
      <div className="flex items-center gap-2 text-xs text-muted-foreground font-mono my-1">
        <Wrench className="w-3 h-3 flex-shrink-0" />
        <span className="truncate">{raw.slice(0, 120)}</span>
      </div>
    )
  }

  const { label, detail } = formatToolCall(info, t)

  return (
    <div className="flex items-start gap-2 rounded border border-border/50 bg-muted/30 px-3 py-2 my-2 text-xs">
      <Wrench className="w-3.5 h-3.5 text-blue-500 mt-0.5 flex-shrink-0" />
      <div className="flex-1 min-w-0">
        <div className="font-medium text-foreground">{label}</div>
        {detail && (
          <div className="text-muted-foreground font-mono mt-0.5 truncate" title={detail}>
            {detail}
          </div>
        )}
      </div>
    </div>
  )
}

// ── 工具结果视图 ──

function ToolResultView({ raw }: { raw: string }) {
  const t = useT()
  const returnedPos = raw.indexOf(' returned: ')
  if (returnedPos < 0) {
    // No JSON part, just show status
    return (
      <div className="flex items-center gap-2 text-xs text-green-600 my-1">
        <CheckCircle2 className="w-3.5 h-3.5 flex-shrink-0" />
        <span>{raw.slice(2).trim()}</span>
      </div>
    )
  }

  const prefix = raw.slice(2, returnedPos).trim() // tool name
  const jsonStr = raw.slice(returnedPos + ' returned: '.length)

  let parsed: any = null
  try {
    parsed = JSON.parse(jsonStr)
  } catch {
    // JSON 解析失败，显示原文（可能还在流式中）
    return (
      <div className="flex items-center gap-2 text-xs text-muted-foreground font-mono my-1">
        <CheckCircle2 className="w-3.5 h-3.5 flex-shrink-0 text-green-500" />
        <span className="truncate">{prefix} → {jsonStr.slice(0, 100)}</span>
      </div>
    )
  }

  const summary = formatToolResult(prefix, parsed, t)

  return (
    <div className="flex items-start gap-2 text-xs text-green-700 dark:text-green-400 my-1">
      <CheckCircle2 className="w-3.5 h-3.5 flex-shrink-0 mt-0.5" />
      <span className="font-mono">{summary}</span>
    </div>
  )
}

// ── Tool-specific formatting ──

type TFn = (key: any, params?: Record<string, string | number>) => string

function formatToolCall(info: ToolCallInfo, t: TFn): { label: string; detail: string } {
  const { name, args } = info
  switch (name) {
    case 'write_file': {
      const path = args.path || ''
      const content = args.content || ''
      return {
        label: t('tool.write', { path: shortPath(path) }),
        detail: t('tool.writeDetail', { size: formatBytes(content.length), n: countLines(content) })
      }
    }
    case 'read_file':
      return { label: t('tool.read'), detail: shortPath(args.path || '') }
    case 'patch_code_file':
      return { label: t('tool.patch'), detail: shortPath(args.path || '') }
    case 'shell_exec':
      return { label: t('tool.shell'), detail: truncate(args.command || '', 80) }
    case 'list_dir':
      return { label: t('tool.listDir'), detail: shortPath(args.path || '.') }
    case 'run_helen_check':
      return { label: t('tool.helenCheck'), detail: shortPath(args.path || '') }
    case 'run_helen_tests':
      return { label: t('tool.helenTests'), detail: shortPath(args.path || '') }
    case 'quality_score':
      return { label: t('tool.quality'), detail: shortPath(args.path || args.filename || '') }
    case 'web_search':
      return { label: t('tool.webSearch'), detail: truncate(args.query || '', 60) }
    case 'web_fetch':
      return { label: t('tool.webFetch'), detail: truncate(args.url || '', 60) }
    default:
      return { label: name, detail: Object.keys(args).join(', ') }
  }
}

function formatToolResult(toolName: string, data: any, t: TFn): string {
  if (!data || typeof data !== 'object') return `${toolName}: ${String(data)}`
  switch (toolName) {
    case 'write_file':
      return data.bytes_written
        ? t('msg.writtenWithSize', { path: shortPath(data.path || ''), size: formatBytes(data.bytes_written) })
        : t('msg.written', { path: shortPath(data.path || '') })
    case 'read_file': {
      const content = data.content || ''
      const lines = countLines(content)
      return `✓ ${shortPath(data.path || '')} · ${t('msg.lines', { n: lines })}`
    }
    case 'shell_exec':
      return data.exit_code !== undefined
        ? t('msg.commandDoneWithExit', { code: data.exit_code })
        : t('msg.commandDone')
    case 'run_helen_check':
      return data.status === 'success'
        ? t('msg.syntaxPassed')
        : t('msg.syntaxError', { error: data.error || t('msg.unknown') })
    case 'run_helen_tests':
      return data.status === 'success'
        ? (data.passed ? t('msg.testsPassedCount', { passed: data.passed, total: data.total }) : t('msg.testsPassed'))
        : (data.failed ? t('msg.testsFailedCount', { failed: data.failed, total: data.total }) : t('msg.testsFailed'))
    case 'quality_score':
      return data.score !== undefined ? t('msg.qualityScore', { score: data.score }) : t('msg.evalComplete')
    default:
      return `✓ ${toolName}: ${data.status || t('msg.done')}`
  }
}

// ── 工具函数 ──

function shortPath(p: string): string {
  if (!p) return ''
  // 去掉项目根目录前缀，只保留相对路径
  return p.replace(/^.*?\/(project\/.*)$/, '$1').replace(/^\/+/, '')
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / 1024 / 1024).toFixed(1)} MB`
}

function countLines(s: string): number {
  if (!s) return 0
  return s.split('\n').length
}

function truncate(s: string, n: number): string {
  return s.length <= n ? s : s.slice(0, n - 1) + '…'
}

// ── Markdownish（保留原有极简 Markdown 渲染） ──

import mermaid from 'mermaid'

// 初始化 mermaid（延迟到首次使用）
let mermaidInitialized = false
let mermaidCounter = 0
function initMermaid() {
  if (mermaidInitialized) return
  mermaid.initialize({
    startOnLoad: false,
    theme: 'default',
    securityLevel: 'loose',
    suppressErrorRendering: true,  // 阻止 Mermaid 把错误 SVG 泄漏到 document.body
  })
  mermaidInitialized = true
}

function generateMermaidId() {
  // Mermaid 要求 id 必须以字母开头且唯一
  return `mermaid-diagram-${mermaidCounter++}`
}

/**
 * Mermaid 图渲染组件
 */
function MermaidDiagram({ chart }: { chart: string }) {
  const [svg, setSvg] = useState<string>('')
  const [error, setError] = useState<string>('')
  const t = useT()

  useEffect(() => {
    let cancelled = false
    initMermaid()
    const id = generateMermaidId()

    // debounce：流式输出时每 300ms 最多渲染一次
    const timer = setTimeout(() => {
      mermaid.render(id, chart)
        .then(({ svg }) => {
          if (!cancelled) {
            setSvg(svg)
            setError('')
          }
        })
        .catch((err) => {
          if (!cancelled) {
            const msg = typeof err === 'string' ? err : (err.message || t('msg.mermaidFailed'))
            setError(msg)
            setSvg('')
          }
        })
        .finally(() => {
          // 清理 Mermaid 可能泄漏到 body 的临时 DOM 节点
          if (!cancelled) {
            document.querySelectorAll(`#d${id}, #i${id}`).forEach(el => el.remove())
          }
        })
    }, 300)

    return () => {
      cancelled = true
      clearTimeout(timer)
      // 组件卸载时清理可能的泄漏
      document.querySelectorAll(`#d${id}, #i${id}`).forEach(el => el.remove())
    }
  }, [chart])

  if (error) {
    return (
      <div className="bg-red-50 dark:bg-red-950/20 border border-red-200 dark:border-red-800 rounded p-3 my-2">
        <div className="text-xs text-red-600 dark:text-red-400 mb-1">Mermaid 渲染错误:</div>
        <pre className="text-xs text-red-700 dark:text-red-300 overflow-x-auto whitespace-pre-wrap">{error}</pre>
        <details className="mt-2">
          <summary className="text-xs cursor-pointer text-muted-foreground">查看源码</summary>
          <pre className="text-xs bg-muted rounded p-2 mt-1 overflow-x-auto whitespace-pre-wrap">{chart}</pre>
        </details>
      </div>
    )
  }

  if (!svg) {
    return (
      <div className="bg-muted rounded p-3 my-2 text-sm text-muted-foreground animate-pulse">
        渲染图表中...
      </div>
    )
  }

  return (
    <div
      className="my-2 flex justify-center overflow-x-auto"
      dangerouslySetInnerHTML={{ __html: svg }}
    />
  )
}

/**
 * 极简 Markdown 渲染（只处理标题、粗体、列表、代码块）
 * 完整 Markdown 渲染后续可接入 react-markdown
 */
function Markdownish({ content }: { content: string }) {
  const lines = content.split('\n')
  const elements: JSX.Element[] = []
  let inCodeBlock = false
  let codeBuffer: string[] = []
  let codeLang = ''

  lines.forEach((line, idx) => {
    if (line.startsWith('```')) {
      if (inCodeBlock) {
        // 代码块结束
        const code = codeBuffer.join('\n')
        if (codeLang === 'mermaid') {
          elements.push(<MermaidDiagram key={`mermaid-${idx}`} chart={code} />)
        } else {
          elements.push(
            <pre key={`code-${idx}`} className="bg-muted rounded p-3 my-2 overflow-x-auto text-xs">
              <code>{code}</code>
            </pre>
          )
        }
        codeBuffer = []
        codeLang = ''
        inCodeBlock = false
      } else {
        inCodeBlock = true
        // 提取语言标识（```python / ```mermaid 等）
        codeLang = line.slice(3).trim().toLowerCase()
      }
      return
    }

    if (inCodeBlock) {
      codeBuffer.push(line)
      return
    }

    // 标题
    if (line.startsWith('### ')) {
      elements.push(<h3 key={idx} className="text-base font-bold mt-3 mb-1">{line.slice(4)}</h3>)
    } else if (line.startsWith('## ')) {
      elements.push(<h2 key={idx} className="text-lg font-bold mt-3 mb-1">{line.slice(3)}</h2>)
    } else if (line.startsWith('# ')) {
      elements.push(<h1 key={idx} className="text-xl font-bold mt-3 mb-1">{line.slice(2)}</h1>)
    }
    // 表格（保留原样，用等宽字体）
    else if (line.startsWith('|') && line.endsWith('|')) {
      elements.push(
        <div key={idx} className="font-mono text-xs whitespace-pre">{line}</div>
      )
    }
    // 列表
    else if (line.startsWith('- ') || line.startsWith('* ')) {
      elements.push(
        <div key={idx} className="ml-4">• {renderInline(line.slice(2))}</div>
      )
    }
    // 普通段落
    else if (line.trim()) {
      elements.push(<p key={idx} className="my-1">{renderInline(line)}</p>)
    }
    // 空行
    else {
      elements.push(<div key={idx} className="h-2" />)
    }
  })

  // 未闭合的代码块：mermaid 不渲染（流式 partial 语法必然报错），其他语言正常显示
  if (inCodeBlock && codeBuffer.length > 0) {
    const code = codeBuffer.join('\n')
    if (codeLang === 'mermaid') {
      elements.push(
        <div key="mermaid-pending" className="bg-muted/50 rounded p-3 my-2 text-xs text-muted-foreground flex items-center gap-2">
          <span className="inline-block w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin" />
          Mermaid 图表生成中…
        </div>
      )
    } else {
      elements.push(
        <pre key="code-pending" className="bg-muted rounded p-3 my-2 overflow-x-auto text-xs">
          <code>{code}</code>
        </pre>
      )
    }
  }

  return <>{elements}</>
}

/**
 * 行内格式：**粗体** 和 `代码`
 */
function renderInline(text: string): JSX.Element {
  // 简化版：按 **bold** 和 `code` 拆分
  const parts: JSX.Element[] = []
  const regex = /(\*\*[^*]+\*\*|`[^`]+`)/g
  let lastIndex = 0
  let match

  while ((match = regex.exec(text)) !== null) {
    // 前面的普通文本
    if (match.index > lastIndex) {
      parts.push(<span key={`t-${lastIndex}`}>{text.slice(lastIndex, match.index)}</span>)
    }
    const token = match[0]
    if (token.startsWith('**') && token.endsWith('**')) {
      parts.push(<strong key={`b-${match.index}`}>{token.slice(2, -2)}</strong>)
    } else if (token.startsWith('`') && token.endsWith('`')) {
      parts.push(
        <code key={`c-${match.index}`} className="bg-muted text-foreground px-1 rounded text-xs font-mono">
          {token.slice(1, -1)}
        </code>
      )
    }
    lastIndex = match.index + token.length
  }
  if (lastIndex < text.length) {
    parts.push(<span key={`t-${lastIndex}`}>{text.slice(lastIndex)}</span>)
  }

  return <>{parts}</>
}

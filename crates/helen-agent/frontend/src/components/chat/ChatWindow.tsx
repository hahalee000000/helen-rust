import { useEffect, useRef, useState, useCallback } from 'react'
import { MessageList } from './MessageList'
import { MessageInput } from './MessageInput'
import { StatusLine } from './StatusLine'
import { useChat } from '@/hooks/useChat'
import { useT } from '@/i18n'
import { ArrowDown, Pause, Play } from 'lucide-react'

interface ChatWindowProps {
  sessionId: string | null
}

// 距离底部多少像素内算"在底部"(用于隐藏/显示 "回到底部" 按钮)
const BOTTOM_THRESHOLD = 50

export function ChatWindow({ sessionId }: ChatWindowProps) {
  const { messages, sendMessage, stopGeneration, isLoading, isConnected, statusline } = useChat(sessionId)
  const containerRef = useRef<HTMLDivElement>(null)
  const t = useT()
  // showScrollBtn: 控制 "回到底部" 浮动按钮显示(基于用户当前滚动位置)
  const [showScrollBtn, setShowScrollBtn] = useState(false)
  // autoScrollEnabled: 用户是否启用了自动滚动
  //   - 初始为 true (新会话开始自动跟随)
  //   - 用户任何形式的滚动/滚轮/触摸都视为"手动干预",暂停自动滚动
  //   - 显式恢复途径:
  //       1. 用户发送新消息 (通过 sendMessage 包装)
  //       2. 用户点击 "回到底部" 按钮
  //       3. 切换会话
  //       4. 用户点击 "启用自动滚动" 按钮
  //   用 state 而不是 ref,因为按钮状态需要反映到 UI
  const [autoScrollEnabled, setAutoScrollEnabled] = useState(true)
  // ──────────────────────────────────────────────────────────────────────────
  // 用户滚动检测策略 (v2 — 解决流式推理中滚动条拖拽无法暂停的问题)
  //
  // 之前的方案用 isProgrammaticScrollRef 标志区分程序化滚动和用户滚动,
  // 但流式推理时 scrollToBottom 每帧都被调用,导致该标志几乎永远为 true,
  // 用户的滚动事件全被当成"程序化滚动"忽略,autoScrollEnabled 永远无法变 false.
  //
  // 新方案:基于滚动方向检测用户意图
  //   - 流式推理中,内容持续增长,scrollHeight 单调递增
  //   - 程序化滚动 (scrollToBottom) 总是让 scrollTop 增加(向下)
  //   - 用户向上滚动 (拖动滚动条/鼠标滚轮/键盘) 让 scrollTop 减少
  //   - 因此 scrollTop 减少是用户滚动的明确信号
  //
  // 兜底方案:wheel 和 touchmove 事件是 100% 用户意图(程序化滚动不触发),
  // 用于捕获 scrollTop 增加但仍为用户意图的罕见场景(例如用户在底部附近
  // 轻微向下滚动).
  // ──────────────────────────────────────────────────────────────────────────
  const lastScrollTopRef = useRef(0)

  // 判断是否接近底部
  const isNearBottom = (): boolean => {
    const el = containerRef.current
    if (!el) return true
    return el.scrollHeight - el.scrollTop - el.clientHeight < BOTTOM_THRESHOLD
  }

  // 滚动到最底部(瞬间).
  const scrollToBottom = useCallback(() => {
    const el = containerRef.current
    if (!el) return
    el.scrollTop = el.scrollHeight
    // 记录实际滚动位置（浏览器会 clamp 到 scrollHeight - clientHeight）
    lastScrollTopRef.current = el.scrollTop
    setShowScrollBtn(false)
  }, [])

  // 重新启用自动滚动 + 立即滚到底部
  const enableAutoScroll = useCallback(() => {
    setAutoScrollEnabled(true)
    scrollToBottom()
  }, [scrollToBottom])

  // 暂停自动滚动
  const pauseAutoScroll = useCallback(() => {
    setAutoScrollEnabled(false)
  }, [])

  // ── 滚动/滚轮/触摸事件统一监听 ────────────────────────────────────────────
  // 单 useEffect 注册所有事件,减少重复代码和清理逻辑
  useEffect(() => {
    const el = containerRef.current
    if (!el) return

    // 1. scroll 事件:基于滚动方向检测用户滚动
    //    流式推理中,程序化滚动使 scrollTop 增加,用户向上滚动使 scrollTop 减少.
    //    阈值 8px 用于吸收浏览器四舍五入抖动.
    const handleScroll = () => {
      const current = el.scrollTop
      const last = lastScrollTopRef.current
      lastScrollTopRef.current = current

      // scrollTop 减少 = 用户明确向上滚动(对抗程序化向下滚动)
      if (current < last - 8) {
        setAutoScrollEnabled(false)
        setShowScrollBtn(!isNearBottom())
      }
    }

    // 2. wheel 事件:鼠标滚轮 — 100% 用户意图(程序化 scrollTop 赋值不触发 wheel)
    //    无论向上/向下,都视为用户想控制滚动
    const handleWheel = () => {
      setAutoScrollEnabled(false)
      requestAnimationFrame(() => {
        lastScrollTopRef.current = el.scrollTop
        setShowScrollBtn(!isNearBottom())
      })
    }

    // 3. touchmove 事件:触摸滑动 — 同样 100% 用户意图
    const handleTouchMove = () => {
      setAutoScrollEnabled(false)
      requestAnimationFrame(() => {
        lastScrollTopRef.current = el.scrollTop
        setShowScrollBtn(!isNearBottom())
      })
    }

    el.addEventListener('scroll', handleScroll, { passive: true })
    el.addEventListener('wheel', handleWheel, { passive: true })
    el.addEventListener('touchmove', handleTouchMove, { passive: true })
    return () => {
      el.removeEventListener('scroll', handleScroll)
      el.removeEventListener('wheel', handleWheel)
      el.removeEventListener('touchmove', handleTouchMove)
    }
  }, [])

  // messages 变化:只有自动滚动启用时才跟随
  useEffect(() => {
    if (autoScrollEnabled) {
      scrollToBottom()
    }
  }, [messages, autoScrollEnabled, scrollToBottom])

  // 切换会话:重置自动滚动 + 历史加载后滚到底
  useEffect(() => {
    setAutoScrollEnabled(true)
    setShowScrollBtn(false)
    requestAnimationFrame(() => {
      const el = containerRef.current
      if (el) {
        el.scrollTop = el.scrollHeight
        lastScrollTopRef.current = el.scrollTop  // 使用实际值
      }
    })
  }, [sessionId])

  // 包装 sendMessage:发送消息 = 重新启用自动滚动
  const sendMessageWithAutoScroll = (content: string, attachments?: any[]) => {
    enableAutoScroll()
    sendMessage(content, attachments)
  }

  if (!sessionId) {
    return (
      <div className="flex items-center justify-center h-full text-muted-foreground">
        <div className="text-center">
          <p className="text-xl mb-2">{t('chat.selectSession')}</p>
          <p className="text-sm">{t('chat.orCreate')}</p>
        </div>
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full">
      {/* 状态栏 */}
      <div className="border-b px-4 py-2 bg-card flex items-center gap-3" style={{ backgroundColor: '#EAE9E5' }}>
        <img src="/helen-logo-64.png" alt="Helen" className="w-6 h-6 rounded-full" />
        <div className="flex items-center gap-2 flex-1">
          <div className={`w-2 h-2 rounded-full ${isConnected ? 'bg-green-500' : 'bg-red-500'}`} />
          <span className="text-sm text-muted-foreground">
            {isConnected ? t('status.connected') : t('status.disconnected')}
          </span>
        </div>
        {isLoading && (
          <span className="flex items-center gap-2 text-sm text-muted-foreground">
            <img src="/helen-logo-64.png" alt="" className="w-4 h-4 rounded-full animate-pulse" />
            {t('chat.thinking')}
          </span>
        )}
      </div>

      {/* 消息列表 + 浮动按钮（relative 容器用于定位） */}
      <div className="relative flex-1 min-h-0">
        {/* 可滚动消息区域 */}
        <div
          ref={containerRef}
          className="h-full overflow-y-auto p-4"
          style={{
            backgroundImage: "url('/wallpaper.png')",
            backgroundSize: 'cover',
            backgroundPosition: 'center',
            backgroundRepeat: 'no-repeat',
          }}
        >
          <MessageList messages={messages} />
        </div>

        {/* 浮动按钮区：定位在 relative 容器内，不受 overflow 影响 */}
        <div className="absolute bottom-4 right-4 z-10 flex flex-col gap-2">
          {/* 暂停/启用 自动滚动切换按钮: 始终可见 */}
          <button
            onClick={() => autoScrollEnabled ? pauseAutoScroll() : enableAutoScroll()}
            className={`flex items-center justify-center rounded-full shadow-lg w-10 h-10 transition-colors ${
              autoScrollEnabled
                ? 'bg-emerald-500 hover:bg-emerald-600 text-white'
                : 'bg-amber-500 hover:bg-amber-600 text-white'
            }`}
            title={autoScrollEnabled ? t('chat.pauseAutoScroll') : t('chat.resumeAutoScroll')}
          >
            {autoScrollEnabled ? <Pause className="w-5 h-5" /> : <Play className="w-5 h-5" />}
          </button>

          {/* "回到底部" 浮动按钮：仅在用户上滚离开底部后出现 */}
          {showScrollBtn && (
            <button
              onClick={() => enableAutoScroll()}
              className="flex items-center gap-1.5 rounded-full bg-blue-500 hover:bg-blue-600 text-white shadow-lg px-4 py-2.5 text-sm font-medium transition-colors"
              title={t('chat.scrollBottom')}
            >
              <ArrowDown className="w-4 h-4" />
              <span>{t('chat.atBottom')}</span>
            </button>
          )}
        </div>
      </div>

      {/* 输入框 */}
      <MessageInput
        onSend={sendMessageWithAutoScroll}
        onStop={stopGeneration}
        disabled={!isConnected}
        isLoading={isLoading}
      />

      {/* 底部状态栏（仿 Claude Code）：hostname · cwd · model · 上下文占用 % */}
      <StatusLine data={statusline} connected={isConnected} />
    </div>
  )
}

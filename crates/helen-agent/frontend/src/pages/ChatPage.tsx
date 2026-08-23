import { useEffect } from 'react'
import { DirectoryBar } from '@/components/chat/DirectoryBar'
import { ChatWindow } from '@/components/chat/ChatWindow'
import { useChatStore } from '@/stores/chatStore'
import { api } from '@/services/api'

/**
 * v6.0 单会话架构：
 * - 移除了 SessionSidebar（会话列表）
 * - 用 DirectoryBar 显示当前工作目录
 * - 会话 = 工作目录，通过 /dir 命令切换
 * - 进入页面时自动以当前目录为 session_id 建立会话
 */
export function ChatPage() {
  const { currentSessionId, setCurrentSession } = useChatStore()

  // 首次挂载：从后端获取当前目录信息，自动建立会话（目录 = 会话边界）
  useEffect(() => {
    if (currentSessionId) return  // 已有会话（例如目录切换后）

    api.chat.getDirectory()
      .then((info) => {
        if (info?.session_id) {
          setCurrentSession(info.session_id)
        }
      })
      .catch((err) => {
        console.error('Failed to initialize session from directory:', err)
      })
  }, [currentSessionId, setCurrentSession])

  return (
    <div className="flex flex-col h-full">
      <DirectoryBar />
      <div className="flex-1 min-h-0">
        <ChatWindow sessionId={currentSessionId} />
      </div>
    </div>
  )
}

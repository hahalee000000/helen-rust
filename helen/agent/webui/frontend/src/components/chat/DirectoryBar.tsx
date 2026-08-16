import { useState, useEffect } from 'react'
import { Folder } from 'lucide-react'
import { api } from '@/services/api'

/**
 * 工作目录显示组件
 *
 * 显示当前工作目录，替代会话列表。
 * 监听 directory_changed 事件，实时更新显示。
 */
export function DirectoryBar() {
  const [dirInfo, setDirInfo] = useState<{
    cwd: string
    display_name: string
    helen_session_id?: string
  } | null>(null)

  // 初始化：获取当前目录
  useEffect(() => {
    loadDirectoryInfo()
  }, [])

  // 监听 WebSocket 事件
  useEffect(() => {
    // 注册全局事件监听器
    const handler = (event: CustomEvent) => {
      if (event.detail?.type === 'directory_changed') {
        setDirInfo({
          cwd: event.detail.data.cwd,
          display_name: event.detail.data.display_name,
          helen_session_id: event.detail.data.helen_session_id,
        })
      }
    }

    window.addEventListener('helen-event', handler as EventListener)

    return () => {
      window.removeEventListener('helen-event', handler as EventListener)
    }
  }, [])

  const loadDirectoryInfo = async () => {
    try {
      const info = await api.chat.getDirectory()
      setDirInfo(info)
    } catch (error) {
      console.error('Failed to load directory info:', error)
    }
  }

  if (!dirInfo) {
    return (
      <div className="flex items-center gap-2 px-4 py-2 bg-gray-100 dark:bg-gray-800 border-b">
        <span className="text-sm text-gray-500">加载中...</span>
      </div>
    )
  }

  return (
    <div className="flex items-center gap-2 px-4 py-2 bg-gray-100 dark:bg-gray-800 border-b">
      <Folder className="w-4 h-4 text-gray-500" />
      <span className="text-sm font-medium text-gray-900 dark:text-gray-100">
        {dirInfo.display_name}
      </span>
      <span className="text-xs text-gray-400 font-mono truncate max-w-md">
        {dirInfo.cwd}
      </span>
    </div>
  )
}

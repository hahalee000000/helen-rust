import { Attachment } from '@/types'

/**
 * 附件渲染组件（多模态支持）
 *
 * 根据 MIME 类型渲染不同的 HTML 元素：
 * - image/* → <img> 缩略图
 * - audio/* → <audio controls>
 * - video/* → <video controls>
 * - 其他 → <a download> 下载链接
 */
export function AttachmentView({ attachment }: { attachment: Attachment }) {
  const { mime_type, url, filename, size } = attachment

  // 格式化文件大小
  const formatSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }

  if (mime_type.startsWith('image/')) {
    return (
      <div className="attachment attachment-image">
        <img
          src={url}
          alt={filename}
          className="max-w-full max-h-64 rounded border border-gray-200 dark:border-gray-700"
        />
        <div className="text-xs text-gray-500 mt-1">
          {filename} ({formatSize(size)})
        </div>
      </div>
    )
  }

  if (mime_type.startsWith('audio/')) {
    return (
      <div className="attachment attachment-audio">
        <audio controls src={url} className="w-full max-w-md" />
        <div className="text-xs text-gray-500 mt-1">
          🎵 {filename} ({formatSize(size)})
        </div>
      </div>
    )
  }

  if (mime_type.startsWith('video/')) {
    return (
      <div className="attachment attachment-video">
        <video
          controls
          src={url}
          className="max-w-full max-h-64 rounded border border-gray-200 dark:border-gray-700"
        />
        <div className="text-xs text-gray-500 mt-1">
          🎬 {filename} ({formatSize(size)})
        </div>
      </div>
    )
  }

  // 其他文件类型：下载链接
  return (
    <div className="attachment attachment-file">
      <a
        href={url}
        download={filename}
        className="inline-flex items-center gap-2 px-3 py-2 bg-gray-100 dark:bg-gray-800 rounded hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
      >
        <span>📄</span>
        <span>{filename}</span>
        <span className="text-xs text-gray-500">({formatSize(size)})</span>
      </a>
    </div>
  )
}

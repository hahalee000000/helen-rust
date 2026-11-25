import { useState, useEffect, useCallback } from 'react'
import { X, Search, User, Bot, Wrench, FileText, Filter } from 'lucide-react'
import { useChatStore } from '@/stores/chatStore'
import { api } from '@/services/api'

interface TranscriptEntry {
  type?: string
  role?: string
  content?: string
  timestamp?: string | number
  tool_calls?: unknown[]
  _line?: number
  [key: string]: unknown
}

interface TranscriptData {
  session_id: string
  file: string
  total_entries: number
  roles: Record<string, number>
  tool_calls_count: number
  entries: TranscriptEntry[]
}

/**
 * v1.46.37: Transcript Viewer Modal
 * Triggered by `/session view <sid>` — shows full transcript history
 * for the specified session in a modal overlay.
 */
export function TranscriptViewerModal() {
  const sessionId = useChatStore((s) => s.viewingTranscriptSessionId)
  const close = () => useChatStore.getState().setViewingTranscript(null)

  const [data, setData] = useState<TranscriptData | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [searchQuery, setSearchQuery] = useState('')
  const [roleFilter, setRoleFilter] = useState<string | null>(null)

  // Fetch transcript when sessionId changes
  useEffect(() => {
    if (!sessionId) {
      setData(null)
      setError(null)
      setSearchQuery('')
      setRoleFilter(null)
      return
    }
    setLoading(true)
    setError(null)
    api.chat.getTranscript(sessionId)
      .then((result) => {
        setData(result as TranscriptData)
        setLoading(false)
      })
      .catch((err) => {
        setError((err as Error).message)
        setLoading(false)
      })
  }, [sessionId])

  // Close on Escape key
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.key === 'Escape') close()
  }, [])

  useEffect(() => {
    if (!sessionId) return
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [sessionId, handleKeyDown])

  if (!sessionId) return null

  // Filter entries
  const filteredEntries = (data?.entries ?? []).filter((entry) => {
    // Role filter
    if (roleFilter && entry.role !== roleFilter) return false
    // Search filter
    if (searchQuery) {
      const q = searchQuery.toLowerCase()
      const content = (entry.content ?? '').toLowerCase()
      const type = (entry.type ?? '').toLowerCase()
      const role = (entry.role ?? '').toLowerCase()
      if (!content.includes(q) && !type.includes(q) && !role.includes(q)) {
        return false
      }
    }
    return true
  })

  const roles = data?.roles ?? {}
  const roleNames = Object.keys(roles)

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onClick={(e) => { if (e.target === e.currentTarget) close() }}
    >
      <div className="bg-white rounded-xl shadow-2xl w-[90vw] max-w-5xl h-[85vh] flex flex-col overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b bg-gray-50">
          <div className="flex items-center gap-3">
            <FileText className="w-5 h-5 text-primary" />
            <div>
              <h2 className="text-lg font-semibold">
                Transcript Viewer
              </h2>
              <p className="text-xs text-muted-foreground font-mono truncate max-w-md">
                {sessionId}
              </p>
            </div>
          </div>
          <button
            onClick={close}
            className="p-2 rounded-lg hover:bg-gray-200 transition-colors"
            aria-label="Close"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Stats bar */}
        {data && (
          <div className="flex items-center gap-4 px-6 py-2 text-xs text-muted-foreground border-b bg-gray-50/50">
            <span>{data.total_entries} entries</span>
            <span>{data.tool_calls_count} tool calls</span>
            {Object.entries(roles).map(([role, count]) => (
              <span key={role}>{role}: {count}</span>
            ))}
          </div>
        )}

        {/* Search & Filter bar */}
        <div className="flex items-center gap-3 px-6 py-3 border-b">
          <div className="flex-1 relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search messages..."
              className="w-full pl-9 pr-3 py-2 text-sm border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/30"
            />
          </div>
          {roleNames.length > 0 && (
            <div className="flex items-center gap-1">
              <Filter className="w-4 h-4 text-muted-foreground" />
              <button
                onClick={() => setRoleFilter(null)}
                className={`px-2 py-1 text-xs rounded ${!roleFilter ? 'bg-primary text-white' : 'bg-gray-100 hover:bg-gray-200'}`}
              >
                All
              </button>
              {roleNames.map((role) => (
                <button
                  key={role}
                  onClick={() => setRoleFilter(roleFilter === role ? null : role)}
                  className={`px-2 py-1 text-xs rounded ${roleFilter === role ? 'bg-primary text-white' : 'bg-gray-100 hover:bg-gray-200'}`}
                >
                  {role}
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-6 py-4">
          {loading && (
            <div className="flex items-center justify-center h-full text-muted-foreground">
              Loading transcript...
            </div>
          )}
          {error && (
            <div className="flex items-center justify-center h-full text-red-500">
              Error: {error}
            </div>
          )}
          {!loading && !error && filteredEntries.length === 0 && (
            <div className="flex items-center justify-center h-full text-muted-foreground">
              {searchQuery || roleFilter ? 'No matching entries' : 'No entries'}
            </div>
          )}
          {!loading && !error && filteredEntries.length > 0 && (
            <div className="space-y-3">
              {filteredEntries.map((entry, idx) => (
                <TranscriptEntryItem key={entry._line ?? idx} entry={entry} />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

function TranscriptEntryItem({ entry }: { entry: TranscriptEntry }) {
  const type = entry.type ?? 'unknown'
  const role = entry.role ?? ''
  const content = entry.content ?? ''
  const line = entry._line

  // Color/icon by type+role
  let icon = <FileText className="w-4 h-4" />
  let borderColor = 'border-gray-200'
  let bgColor = 'bg-white'
  let label = type

  if (type === 'message') {
    if (role === 'user') {
      icon = <User className="w-4 h-4 text-blue-500" />
      borderColor = 'border-blue-200'
      bgColor = 'bg-blue-50/50'
      label = 'User'
    } else if (role === 'assistant') {
      icon = <Bot className="w-4 h-4 text-green-500" />
      borderColor = 'border-green-200'
      bgColor = 'bg-green-50/50'
      label = 'Assistant'
    } else if (role === 'system') {
      icon = <FileText className="w-4 h-4 text-gray-500" />
      borderColor = 'border-gray-300'
      bgColor = 'bg-gray-50'
      label = 'System'
    } else {
      label = role || 'Message'
    }
  } else if (type === 'tool_call' || type === 'tool_result') {
    icon = <Wrench className="w-4 h-4 text-orange-500" />
    borderColor = 'border-orange-200'
    bgColor = 'bg-orange-50/50'
    label = type === 'tool_call' ? 'Tool Call' : 'Tool Result'
  }

  // Truncate very long content for display (expandable)
  const isLong = content.length > 500
  const [expanded, setExpanded] = useState(false)
  const displayContent = isLong && !expanded ? content.slice(0, 500) + '...' : content

  return (
    <div className={`border rounded-lg ${borderColor} ${bgColor} overflow-hidden`}>
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-inherit bg-white/50 text-xs">
        {icon}
        <span className="font-medium">{label}</span>
        {line != null && (
          <span className="text-muted-foreground ml-auto">L{line}</span>
        )}
      </div>
      <div className="px-3 py-2 text-sm font-mono whitespace-pre-wrap break-words max-h-96 overflow-y-auto">
        {displayContent || <span className="text-muted-foreground italic">(empty)</span>}
        {isLong && (
          <button
            onClick={() => setExpanded(!expanded)}
            className="block mt-1 text-xs text-primary hover:underline"
          >
            {expanded ? 'Show less' : `Show full (${content.length} chars)`}
          </button>
        )}
      </div>
    </div>
  )
}

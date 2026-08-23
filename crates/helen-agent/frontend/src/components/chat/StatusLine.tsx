import { StatuslineData } from '@/types'
import { useT } from '@/i18n'

/**
 * Statusline — Claude Code-style bottom status bar
 *
 * Shows 4 key items for the current session:
 *   hostname · cwd (shortened) · model · context usage %
 *
 * Data pushed by Helen via Python FFI (ui.status_emitter) at key points
 * (ChatSession entry / llm_complete / after hint injection via on_tool_end).
 *
 * Props:
 *   data:     StatuslineData (from useChat hook)
 *   connected: whether WebSocket is connected (red dot when disconnected)
 */
interface StatusLineProps {
  data: StatuslineData
  connected: boolean
}

/** Shorten an absolute cwd to ~/xxx form; if too long, show only last segment */
function shortenCwd(cwd: string | undefined): string {
  if (!cwd) return ''
  // /home/<user>/... → ~/...
  const homeShort = cwd.replace(/^\/home\/[^/]+/, '~')
  if (homeShort !== cwd) return homeShort
  // Other paths: keep last two segments
  const parts = cwd.split('/').filter(Boolean)
  if (parts.length === 0) return cwd
  if (parts.length <= 2) return cwd
  return parts.slice(-2).join('/')
}

export function StatusLine({ data, connected }: StatusLineProps) {
  const t = useT()
  const usagePct = Math.round((data.usageRatio ?? 0) * 100)
  const shortCwd = shortenCwd(data.cwd)

  // Usage color thresholds: <60% green, 60-85% yellow, >85% red
  const usageColor = usagePct > 85
    ? 'text-red-500'
    : usagePct > 60
      ? 'text-amber-500'
      : 'text-emerald-500'

  // Assemble items in order, filter empty
  const items: Array<{ key: string; text: string; title?: string; className?: string } | null> = [
    !connected ? { key: 'conn', text: t('status.disconnectedShort'), className: 'text-red-500' } : null,
    data.hostname ? { key: 'host', text: data.hostname } : null,
    shortCwd ? { key: 'cwd', text: shortCwd, title: data.cwd } : null,
    data.model ? { key: 'model', text: data.model, className: 'text-muted-foreground' } : null,
    {
      key: 'usage',
      text: `${usagePct}%`,
      title: t('status.contextUsage', { pct: usagePct }),
      className: usageColor,
    },
  ]

  const rendered = items.filter((x): x is NonNullable<typeof x> => x !== null)

  return (
    <div
      className="border-t border-border/40 bg-muted/30 px-4 py-1 text-xs text-muted-foreground flex items-center gap-1 overflow-hidden"
      role="status"
      aria-label={t('status.sessionState')}
    >
      {rendered.length === 0 ? (
        <span className="italic opacity-60">{t('status.waiting')}</span>
      ) : (
        rendered.map((item, i) => (
          <span key={item.key} className="flex items-center gap-1 min-w-0">
            {i > 0 && (
              <span className="text-muted-foreground/40 select-none" aria-hidden>·</span>
            )}
            <span
              className={`truncate ${item.className ?? ''}`}
              title={item.title}
            >
              {item.text}
            </span>
          </span>
        ))
      )}
    </div>
  )
}

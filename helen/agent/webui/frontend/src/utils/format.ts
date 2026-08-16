import type { TranslationKey } from '@/i18n'

type TFn = (key: TranslationKey, params?: Record<string, string | number>) => string

/**
 * Format absolute timestamp using locale-aware format.
 */
export function formatTime(timestamp: string, lang: string = 'en'): string {
  const date = new Date(timestamp)
  const locale = lang === 'zh' ? 'zh-CN' : 'en-US'
  return date.toLocaleString(locale, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

/**
 * Format relative time ("just now", "5m ago", etc.)
 */
export function formatRelativeTime(timestamp: string, t: TFn, lang: string = 'en'): string {
  const date = new Date(timestamp)
  const now = new Date()
  const diff = now.getTime() - date.getTime()

  const minutes = Math.floor(diff / 60000)
  const hours = Math.floor(diff / 3600000)
  const days = Math.floor(diff / 86400000)

  if (minutes < 1) return t('time.justNow')
  if (minutes < 60) return t('time.minutesAgo', { n: minutes })
  if (hours < 24) return t('time.hoursAgo', { n: hours })
  if (days < 7) return t('time.daysAgo', { n: days })

  return formatTime(timestamp, lang)
}

/**
 * Generate a v4 UUID
 */
export function generateUUID(): string {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function (c) {
    const r = (Math.random() * 16) | 0
    const v = c === 'x' ? r : (r & 0x3) | 0x8
    return v.toString(16)
  })
}

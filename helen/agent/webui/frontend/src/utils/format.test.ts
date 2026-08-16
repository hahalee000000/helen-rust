import { describe, it, expect } from 'vitest'
import { formatTime, formatRelativeTime, generateUUID } from './format'
import { cn } from './cn'
import { translations } from '@/i18n/translations'

// Test helper: build a t() function from English translations
const t = ((key: string, params?: Record<string, string | number>) => {
  let text = (translations.en as Record<string, string>)[key] ?? key
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      text = text.replace(`{${k}}`, String(v))
    }
  }
  return text
}) as any

describe('cn', () => {
  it('merges class names', () => {
    expect(cn('foo', 'bar')).toBe('foo bar')
  })

  it('handles conditional classes', () => {
    expect(cn('foo', true && 'bar', false && 'baz')).toBe('foo bar')
  })

  it('merges tailwind classes', () => {
    expect(cn('px-2 py-1', 'px-4')).toBe('py-1 px-4')
  })
})

describe('formatTime', () => {
  it('formats timestamp', () => {
    const result = formatTime('2024-01-15T10:30:00Z', 'en')
    expect(result).toMatch(/2024/)
    expect(result).toMatch(/01/)
    expect(result).toMatch(/15/)
  })
})

describe('formatRelativeTime', () => {
  it('returns just now for recent times', () => {
    const now = new Date().toISOString()
    expect(formatRelativeTime(now, t, 'en')).toBe('just now')
  })

  it('returns minutes ago', () => {
    const fiveMinutesAgo = new Date(Date.now() - 5 * 60 * 1000).toISOString()
    expect(formatRelativeTime(fiveMinutesAgo, t, 'en')).toBe('5m ago')
  })

  it('returns hours ago', () => {
    const twoHoursAgo = new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString()
    expect(formatRelativeTime(twoHoursAgo, t, 'en')).toBe('2h ago')
  })

  it('returns days ago', () => {
    const threeDaysAgo = new Date(Date.now() - 3 * 24 * 60 * 60 * 1000).toISOString()
    expect(formatRelativeTime(threeDaysAgo, t, 'en')).toBe('3d ago')
  })

  it('supports Chinese locale via t()', () => {
    const tZh = ((key: string, params?: Record<string, string | number>) => {
      let text = (translations.zh as Record<string, string>)[key] ?? key
      if (params) {
        for (const [k, v] of Object.entries(params)) {
          text = text.replace(`{${k}}`, String(v))
        }
      }
      return text
    }) as any
    const now = new Date().toISOString()
    expect(formatRelativeTime(now, tZh, 'zh')).toBe('刚刚')
  })
})

describe('generateUUID', () => {
  it('generates valid UUID format', () => {
    const uuid = generateUUID()
    expect(uuid).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/)
  })

  it('generates unique UUIDs', () => {
    const uuid1 = generateUUID()
    const uuid2 = generateUUID()
    expect(uuid1).not.toBe(uuid2)
  })
})

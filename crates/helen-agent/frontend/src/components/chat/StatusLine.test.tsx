import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { StatusLine } from './StatusLine'
import { StatuslineData } from '@/types'

// Mock i18n
vi.mock('@/i18n', () => ({
  useT: () => (key: string, params?: Record<string, any>) => {
    if (key === 'status.contextUsage') return `Context: ${params?.pct}%`
    if (key === 'status.sessionState') return 'Session state'
    if (key === 'status.waiting') return 'Waiting...'
    if (key === 'status.disconnectedShort') return 'DISC'
    return key
  },
}))

describe('StatusLine', () => {
  const defaultData: StatuslineData = {
    hostname: 'myhost',
    cwd: '/home/user/project',
    user: 'testuser',
    model: 'gpt-4',
    usageRatio: 0.5,
  }

  it('renders all status items when data is present', () => {
    render(<StatusLine data={defaultData} connected={true} />)

    expect(screen.getByText('myhost')).toBeInTheDocument()
    expect(screen.getByText('~/project')).toBeInTheDocument()
    expect(screen.getByText('gpt-4')).toBeInTheDocument()
    expect(screen.getByText('50%')).toBeInTheDocument()
  })

  it('shows disconnected indicator when not connected', () => {
    render(<StatusLine data={defaultData} connected={false} />)

    expect(screen.getByText('DISC')).toBeInTheDocument()
    expect(screen.getByText('DISC')).toHaveClass('text-red-500')
  })

  it('does not show disconnected indicator when connected', () => {
    render(<StatusLine data={defaultData} connected={true} />)

    expect(screen.queryByText('DISC')).not.toBeInTheDocument()
  })

  it('shortens home directory to ~', () => {
    const data = { ...defaultData, cwd: '/home/testuser/my/project' }
    render(<StatusLine data={data} connected={true} />)

    expect(screen.getByText('~/my/project')).toBeInTheDocument()
  })

  it('shortens long paths to last two segments', () => {
    const data = { ...defaultData, cwd: '/var/log/nginx/access' }
    render(<StatusLine data={data} connected={true} />)

    expect(screen.getByText('nginx/access')).toBeInTheDocument()
  })

  it('shows 0% usage when no data provided', () => {
    const emptyData: StatuslineData = { usageRatio: 0 }
    render(<StatusLine data={emptyData} connected={true} />)

    // Usage is always shown, defaults to 0%
    expect(screen.getByText('0%')).toBeInTheDocument()
    // But no hostname, cwd, or model
    expect(screen.queryByText('myhost')).not.toBeInTheDocument()
  })

  it('uses green color for usage < 60%', () => {
    const data = { ...defaultData, usageRatio: 0.3 }
    render(<StatusLine data={data} connected={true} />)

    const usageEl = screen.getByText('30%')
    expect(usageEl).toHaveClass('text-emerald-500')
  })

  it('uses yellow color for usage 60-85%', () => {
    const data = { ...defaultData, usageRatio: 0.7 }
    render(<StatusLine data={data} connected={true} />)

    const usageEl = screen.getByText('70%')
    expect(usageEl).toHaveClass('text-amber-500')
  })

  it('uses red color for usage > 85%', () => {
    const data = { ...defaultData, usageRatio: 0.9 }
    render(<StatusLine data={data} connected={true} />)

    const usageEl = screen.getByText('90%')
    expect(usageEl).toHaveClass('text-red-500')
  })

  it('has correct role and aria-label', () => {
    render(<StatusLine data={defaultData} connected={true} />)

    const statusEl = screen.getByRole('status')
    expect(statusEl).toHaveAttribute('aria-label', 'Session state')
  })

  it('shows full cwd in title attribute', () => {
    const data = { ...defaultData, cwd: '/home/user/very/long/path' }
    render(<StatusLine data={data} connected={true} />)

    const cwdEl = screen.getByText('~/very/long/path')
    expect(cwdEl).toHaveAttribute('title', '/home/user/very/long/path')
  })

  it('handles undefined usageRatio', () => {
    const data: StatuslineData = { ...defaultData, usageRatio: 0 }
    render(<StatusLine data={data} connected={true} />)

    expect(screen.getByText('0%')).toBeInTheDocument()
  })
})

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { LanguageSwitcher } from './LanguageSwitcher'

// Create a mock for useI18n that we can control
const mockUseI18n = vi.fn()
vi.mock('./context', () => ({
  useI18n: () => mockUseI18n(),
  I18nProvider: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}))

describe('LanguageSwitcher - English mode', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockUseI18n.mockReturnValue({
      lang: 'en',
      setLang: vi.fn(),
      t: (key: string) => key === 'lang.switch' ? '中文' : key,
    })
  })

  it('renders with switch label', () => {
    render(<LanguageSwitcher />)
    expect(screen.getByText('中文')).toBeInTheDocument()
  })

  it('has correct aria-label for English', () => {
    render(<LanguageSwitcher />)
    const button = screen.getByRole('button')
    expect(button).toHaveAttribute('aria-label', 'Switch to Chinese')
  })

  it('has correct title for English', () => {
    render(<LanguageSwitcher />)
    const button = screen.getByRole('button')
    expect(button).toHaveAttribute('title', 'Switch to Chinese')
  })

  it('calls setLang with zh when clicked (from en)', async () => {
    const user = userEvent.setup()
    const setLang = vi.fn()
    mockUseI18n.mockReturnValue({
      lang: 'en',
      setLang,
      t: (key: string) => key === 'lang.switch' ? '中文' : key,
    })
    
    render(<LanguageSwitcher />)
    const button = screen.getByRole('button')
    await user.click(button)

    expect(setLang).toHaveBeenCalledWith('zh')
  })

  it('applies custom className', () => {
    render(<LanguageSwitcher className="custom-class" />)
    const button = screen.getByRole('button')
    expect(button).toHaveClass('custom-class')
  })

  it('has default styling classes', () => {
    render(<LanguageSwitcher />)
    const button = screen.getByRole('button')
    expect(button).toHaveClass('px-2')
    expect(button).toHaveClass('py-1')
    expect(button).toHaveClass('text-xs')
    expect(button).toHaveClass('border')
    expect(button).toHaveClass('rounded')
  })
})

describe('LanguageSwitcher - Chinese mode', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockUseI18n.mockReturnValue({
      lang: 'zh',
      setLang: vi.fn(),
      t: (key: string) => key === 'lang.switch' ? 'English' : key,
    })
  })

  it('renders with English label', () => {
    render(<LanguageSwitcher />)
    expect(screen.getByText('English')).toBeInTheDocument()
  })

  it('has correct aria-label for Chinese', () => {
    render(<LanguageSwitcher />)
    const button = screen.getByRole('button')
    expect(button).toHaveAttribute('aria-label', 'Switch to English')
  })

  it('has correct title for Chinese', () => {
    render(<LanguageSwitcher />)
    const button = screen.getByRole('button')
    expect(button).toHaveAttribute('title', '切换到英文')
  })

  it('calls setLang with en when clicked (from zh)', async () => {
    const user = userEvent.setup()
    const setLang = vi.fn()
    mockUseI18n.mockReturnValue({
      lang: 'zh',
      setLang,
      t: (key: string) => key === 'lang.switch' ? 'English' : key,
    })
    
    render(<LanguageSwitcher />)
    const button = screen.getByRole('button')
    await user.click(button)

    expect(setLang).toHaveBeenCalledWith('en')
  })
})

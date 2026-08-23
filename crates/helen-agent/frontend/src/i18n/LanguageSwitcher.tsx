import { useI18n } from './context'
import { Lang } from './translations'

/**
 * Language switcher button. Toggles between en ↔ zh.
 * Label shows the OTHER language (so users know what they'll switch to).
 */
export function LanguageSwitcher({ className = '' }: { className?: string }) {
  const { lang, setLang, t } = useI18n()

  const toggle = () => {
    const next: Lang = lang === 'en' ? 'zh' : 'en'
    setLang(next)
  }

  return (
    <button
      onClick={toggle}
      className={`px-2 py-1 text-xs border rounded hover:bg-accent transition-colors ${className}`}
      title={lang === 'en' ? 'Switch to Chinese' : '切换到英文'}
      aria-label={lang === 'en' ? 'Switch to Chinese' : 'Switch to English'}
    >
      {t('lang.switch')}
    </button>
  )
}

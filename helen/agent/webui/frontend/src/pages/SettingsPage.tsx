import { useState, useEffect } from 'react'
import { getStoredToken, setStoredToken, clearStoredToken } from '@/services/api'
import { useT } from '@/i18n'

interface StatusInfo {
  version: string
  helen_path: string
}

export function SettingsPage() {
  const [statusInfo, setStatusInfo] = useState<StatusInfo | null>(null)
  const [token, setToken] = useState<string>(() => getStoredToken())
  const [tokenInput, setTokenInput] = useState<string>('')
  const [tokenSaved, setTokenSaved] = useState(false)
  const t = useT()

  const loadStatus = async () => {
    try {
      const response = await fetch('/api/status')
      const data = await response.json()
      setStatusInfo({
        version: data.version || '',
        ...data.config,
      })
    } catch (error) {
      console.error('Failed to load status:', error)
    }
  }

  useEffect(() => {
    loadStatus()
  }, [])

  const handleSaveToken = () => {
    const trimmed = tokenInput.trim()
    if (!trimmed) return
    setStoredToken(trimmed)
    setToken(trimmed)
    setTokenInput('')
    setTokenSaved(true)
    setTimeout(() => setTokenSaved(false), 2000)
  }

  const handleClearToken = () => {
    clearStoredToken()
    setToken('')
  }

  return (
    <div className="p-6 overflow-y-auto h-full max-w-4xl">
      <h1 className="text-3xl font-bold mb-6">{t('settings.title')}</h1>

      {/* Access Token */}
      <section className="mb-6">
        <h2 className="text-xl font-semibold mb-4">{t('settings.token.section')}</h2>

        <div className="border rounded-lg p-4 bg-card space-y-3">
          {token ? (
            <div className="space-y-2">
              <p className="text-sm text-muted-foreground">
                {t('settings.token.current')}（<span className="font-mono">{token.slice(0, 8)}…</span>）
              </p>
              <div className="flex gap-2">
                <button
                  onClick={() => navigator.clipboard?.writeText(token)}
                  className="px-3 py-1.5 text-sm border rounded hover:bg-accent"
                >
                  {t('settings.token.copy')}
                </button>
                <button
                  onClick={handleClearToken}
                  className="px-3 py-1.5 text-sm border rounded hover:bg-destructive/10"
                >
                  {t('settings.token.clear')}
                </button>
              </div>
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">
              {t('settings.token.empty')}
            </p>
          )}

          <div className="flex gap-2 items-center">
            <input
              type="password"
              placeholder={t('settings.token.placeholder')}
              value={tokenInput}
              onChange={(e) => setTokenInput(e.target.value)}
              onKeyDown={(e) => { if (e.key === 'Enter') handleSaveToken() }}
              className="flex-1 px-3 py-1.5 border rounded font-mono text-sm"
            />
            <button
              onClick={handleSaveToken}
              disabled={!tokenInput.trim()}
              className="px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded hover:bg-primary/90 disabled:opacity-50"
            >
              {t('settings.token.save')}
            </button>
          </div>
          {tokenSaved && (
            <p className="text-xs text-green-600">{t('settings.token.saved')}</p>
          )}
          <p className="text-xs text-muted-foreground">
            {t('settings.token.hint')} <code className="font-mono">~/.helen/webui_token</code> {t('settings.token.hintFile')}
          </p>
        </div>
      </section>

      {/* System info */}
      <section>
        <h2 className="text-xl font-semibold mb-4">{t('settings.system.title')}</h2>

        <div className="border rounded-lg p-4 bg-card">
          <dl className="space-y-2 text-sm">
            <div className="flex justify-between">
              <dt className="text-muted-foreground">{t('settings.system.version')}</dt>
              <dd>{statusInfo?.version ? `v${statusInfo.version}` : t('settings.loading')}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-muted-foreground">{t('settings.system.helenPath')}</dt>
              <dd className="font-mono text-xs">{statusInfo?.helen_path ?? t('settings.loading')}</dd>
            </div>
            <div className="flex justify-between">
              <dt className="text-muted-foreground">{t('settings.system.backendApi')}</dt>
              <dd className="font-mono text-xs">{t('settings.system.backendApiValue')}</dd>
            </div>
          </dl>
        </div>
      </section>
    </div>
  )
}

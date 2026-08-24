import { useEffect, useState, useCallback, type ReactNode } from 'react'
import { getStoredToken, setStoredToken, onAuthRequired } from '@/services/api'
import { useT } from '@/i18n'

interface Props {
  children: ReactNode
}

/**
 * Auth gate: shows a token prompt on first 401 or missing token.
 * Once the user enters a correct token and stores it in localStorage,
 * child components render normally.
 */
export function AuthGate({ children }: Props) {
  const [token, setToken] = useState<string>(() => getStoredToken())
  const [prompting, setPrompting] = useState(false)
  const [probed, setProbed] = useState(false)
  const t = useT()

  const askForToken = useCallback(() => {
    setPrompting(true)
  }, [])

  useEffect(() => {
    const unsub = onAuthRequired(askForToken)
    return unsub
  }, [askForToken])

  // Initial startup: probe /api/status, if 401 prompt immediately
  useEffect(() => {
    if (token) {
      setProbed(true) // already have token, skip probe
      return
    }
    const probe = async () => {
      try {
        const resp = await fetch('/api/status')
        if (resp.status === 401 || resp.status === 403) {
          setPrompting(true)
        }
      } catch {
        // Network error (backend not running): don't prompt, let user see normal connection error
      } finally {
        setProbed(true)
      }
    }
    probe()
  }, [token])

  const handleSubmit = (value: string) => {
    const trimmed = value.trim()
    if (!trimmed) return
    setStoredToken(trimmed)
    setToken(trimmed)
    setPrompting(false)
    // Reload page so all cached requests carry the token
    window.location.reload()
  }

  const handleClear = () => {
    setStoredToken('')
    setToken('')
    setPrompting(false)
  }

  // No token and not prompting and probe hasn't completed: wait for probe
  if (!token && !prompting && !probed) {
    return null
  }

  if (prompting) {
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
        <form
          className="bg-card border rounded-lg shadow-lg p-6 w-96 space-y-4"
          onSubmit={(e) => {
            e.preventDefault()
            const input = (e.currentTarget.elements.namedItem('token') as HTMLInputElement).value
            handleSubmit(input)
          }}
        >
          <h2 className="text-lg font-semibold">{t('auth.title')}</h2>
          <p className="text-sm text-muted-foreground">
            {t('auth.description')} <code className="font-mono">~/.helen/webui_token</code> {t('auth.descriptionFile')}
          </p>
          <input
            name="token"
            type="password"
            autoFocus
            placeholder={t('auth.placeholder')}
            className="w-full px-3 py-2 border rounded font-mono text-sm"
          />
          <div className="flex gap-2 justify-end">
            {token && (
              <button
                type="button"
                onClick={handleClear}
                className="px-3 py-1.5 text-sm border rounded hover:bg-destructive/10"
              >
                {t('auth.clear')}
              </button>
            )}
            <button
              type="submit"
              className="px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded hover:bg-primary/90"
            >
              {t('auth.submit')}
            </button>
          </div>
        </form>
      </div>
    )
  }

  return <>{children}</>
}

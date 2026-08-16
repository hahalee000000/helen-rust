import { Component, ErrorInfo, ReactNode, useState, useCallback } from 'react'
import { AlertCircle, RefreshCw } from 'lucide-react'
import { useT } from '@/i18n'

interface Props {
  children: ReactNode
  fallback?: ReactNode
}

interface ErrorCatcherProps {
  children: ReactNode
  onError: (error: Error) => void
}

interface ErrorCatcherState {
  hasError: boolean
}

/**
 * Class-based error catcher — React error boundaries must be class components.
 * Forwards errors to the parent function component via callback.
 */
class ErrorCatcher extends Component<ErrorCatcherProps, ErrorCatcherState> {
  state: ErrorCatcherState = { hasError: false }

  static getDerivedStateFromError(): ErrorCatcherState {
    return { hasError: true }
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('ErrorBoundary caught an error:', error, errorInfo)
    this.props.onError(error)
  }

  render() {
    if (this.state.hasError) {
      return null // Parent function component handles the UI
    }
    return this.props.children
  }
}

/**
 * Error boundary — catches render errors in child tree.
 * Uses a class-based catcher internally but exposes a function component
 * API so we can use the i18n hook in the fallback UI.
 */
export function ErrorBoundary({ children, fallback }: Props) {
  const [error, setError] = useState<Error | null>(null)
  const t = useT()

  const handleReset = useCallback(() => {
    setError(null)
  }, [])

  if (error) {
    if (fallback) return fallback

    return (
      <div className="flex items-center justify-center h-full p-8">
        <div className="max-w-md text-center">
          <AlertCircle className="h-16 w-16 text-destructive mx-auto mb-4" />
          <h2 className="text-2xl font-bold mb-2">{t('error.title')}</h2>
          <p className="text-muted-foreground mb-4">
            {error.message || t('error.unknownMessage')}
          </p>
          <button
            onClick={handleReset}
            className="inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
          >
            <RefreshCw className="h-4 w-4" />
            {t('error.retry')}
          </button>
        </div>
      </div>
    )
  }

  return <ErrorCatcher onError={setError}>{children}</ErrorCatcher>
}

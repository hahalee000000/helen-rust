// Test utilities — custom render that wraps components with required providers
import { ReactElement } from 'react'
import { render, RenderOptions } from '@testing-library/react'
import { I18nProvider } from '@/i18n'

/**
 * Custom render that wraps the component with I18nProvider (and any other
 * providers needed for tests). Use this instead of the default `render` from
 * @testing-library/react.
 */
function AllProviders({ children }: { children: React.ReactNode }) {
  return <I18nProvider>{children}</I18nProvider>
}

function customRender(ui: ReactElement, options?: Omit<RenderOptions, 'wrapper'>) {
  return render(ui, { wrapper: AllProviders, ...options })
}

// Re-export everything from @testing-library/react
export * from '@testing-library/react'

// Override render with our custom version
export { customRender as render }

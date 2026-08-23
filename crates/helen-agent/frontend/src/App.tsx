import { Routes, Route, Navigate } from 'react-router-dom'
import { Layout } from '@/components/layout/Layout'
import { ErrorBoundary } from '@/components/ErrorBoundary'
import { AuthGate } from '@/components/AuthGate'
import { ChatPage } from '@/pages/ChatPage'
import { SettingsPage } from '@/pages/SettingsPage'

function App() {
  return (
    <AuthGate>
      <ErrorBoundary>
        <Layout>
          <Routes>
            <Route path="/" element={
              <ErrorBoundary fallback={<div className="p-8">聊天页面加载失败</div>}>
                <ChatPage />
              </ErrorBoundary>
            } />
            <Route path="/settings" element={
              <ErrorBoundary fallback={<div className="p-8">设置页面加载失败</div>}>
                <SettingsPage />
              </ErrorBoundary>
            } />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        </Layout>
      </ErrorBoundary>
    </AuthGate>
  )
}

export default App

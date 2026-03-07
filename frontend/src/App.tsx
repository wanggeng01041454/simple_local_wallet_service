import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom'
import { AppProvider, useApp } from './context/AppContext'
import HealthDot from './components/HealthDot'
import Setup from './pages/Setup'
import Unlock from './pages/Unlock'
import Dashboard from './pages/Dashboard'
import Wallets from './pages/Wallets'
import Settings from './pages/Settings'

/**
 * Routing rules:
 *   setup_required → everything goes to /setup
 *   locked         → everything goes to /unlock (including /setup)
 *   unlocked       → /setup and /unlock redirect to /dashboard; protected pages are accessible
 */
function AppRoutes() {
  const { status } = useApp()

  if (status === 'loading') return <div className="page-container loading">Loading...</div>

  // No wallet files yet — force /setup for every URL
  if (status === 'setup_required') {
    return (
      <Routes>
        <Route path="/setup" element={<Setup />} />
        <Route path="*" element={<Navigate to="/setup" replace />} />
      </Routes>
    )
  }

  // Wallet files exist but locked — force /unlock for every URL
  if (status === 'locked') {
    return (
      <Routes>
        <Route path="/unlock" element={<Unlock />} />
        <Route path="*" element={<Navigate to="/unlock" replace />} />
      </Routes>
    )
  }

  // Unlocked — full access; /setup and /unlock redirect to /dashboard
  return (
    <Routes>
      <Route path="/dashboard" element={<Dashboard />} />
      <Route path="/wallets" element={<Wallets />} />
      <Route path="/settings" element={<Settings />} />
      <Route path="/setup" element={<Navigate to="/dashboard" replace />} />
      <Route path="/unlock" element={<Navigate to="/dashboard" replace />} />
      <Route path="/" element={<Navigate to="/dashboard" replace />} />
      <Route path="*" element={<Navigate to="/dashboard" replace />} />
    </Routes>
  )
}

export default function App() {
  return (
    <BrowserRouter>
      <AppProvider>
        <div style={{ position: 'fixed', top: 12, right: 12 }}>
          <HealthDot />
        </div>
        <AppRoutes />
      </AppProvider>
    </BrowserRouter>
  )
}

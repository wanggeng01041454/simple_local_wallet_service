import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom'
import { AppProvider, useApp } from './context/AppContext'
import Setup from './pages/Setup'
import Unlock from './pages/Unlock'
import Dashboard from './pages/Dashboard'
import Wallets from './pages/Wallets'
import Settings from './pages/Settings'

function RequireUnlocked({ children }: { children: React.ReactNode }) {
  const { status } = useApp()
  if (status === 'loading') return <div>Loading...</div>
  if (status === 'setup_required') return <Navigate to="/setup" replace />
  if (status !== 'unlocked') return <Navigate to="/unlock" replace />
  return <>{children}</>
}

function AppRoutes() {
  const { status } = useApp()
  if (status === 'loading') return <div>Loading...</div>
  return (
    <Routes>
      <Route path="/setup" element={<Setup />} />
      <Route path="/unlock" element={<Unlock />} />
      <Route
        path="/dashboard"
        element={<RequireUnlocked><Dashboard /></RequireUnlocked>}
      />
      <Route
        path="/wallets"
        element={<RequireUnlocked><Wallets /></RequireUnlocked>}
      />
      <Route
        path="/settings"
        element={<RequireUnlocked><Settings /></RequireUnlocked>}
      />
      <Route
        path="/"
        element={
          status === 'setup_required' ? (
            <Navigate to="/setup" replace />
          ) : status === 'unlocked' ? (
            <Navigate to="/dashboard" replace />
          ) : (
            <Navigate to="/unlock" replace />
          )
        }
      />
      <Route
        path="*"
        element={
          status === 'unlocked'
            ? <Navigate to="/dashboard" replace />
            : <Navigate to="/unlock" replace />
        }
      />
    </Routes>
  )
}

export default function App() {
  return (
    <BrowserRouter>
      <AppProvider>
        <AppRoutes />
      </AppProvider>
    </BrowserRouter>
  )
}

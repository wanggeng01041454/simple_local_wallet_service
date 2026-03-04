import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom'
import { AppProvider, useApp } from './context/AppContext'
import Setup from './pages/Setup'
import Unlock from './pages/Unlock'
import Dashboard from './pages/Dashboard'
import Wallets from './pages/Wallets'
import Settings from './pages/Settings'

function AppRoutes() {
  const { status } = useApp()
  if (status === 'loading') return <div>Loading...</div>
  return (
    <Routes>
      <Route path="/setup" element={<Setup />} />
      <Route path="/unlock" element={<Unlock />} />
      <Route path="/dashboard" element={<Dashboard />} />
      <Route path="/wallets" element={<Wallets />} />
      <Route path="/settings" element={<Settings />} />
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

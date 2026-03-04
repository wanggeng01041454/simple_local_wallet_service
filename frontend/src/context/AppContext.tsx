import React, { createContext, useContext, useEffect, useState } from 'react'
import axios from 'axios'

export type AppStatus = 'loading' | 'setup_required' | 'locked' | 'unlocked'

interface AppContextValue {
  status: AppStatus
  refresh: () => Promise<void>
}

const AppContext = createContext<AppContextValue>({
  status: 'loading',
  refresh: async () => {},
})

export function AppProvider({ children }: { children: React.ReactNode }) {
  const [status, setStatus] = useState<AppStatus>('loading')

  const refresh = async () => {
    try {
      const resp = await axios.get('/api/admin/status')
      const { setup_required, unlocked } = resp.data
      if (setup_required) setStatus('setup_required')
      else if (unlocked) setStatus('unlocked')
      else setStatus('locked')
    } catch {
      setStatus('locked')
    }
  }

  useEffect(() => {
    refresh()
  }, [])

  return (
    <AppContext.Provider value={{ status, refresh }}>
      {children}
    </AppContext.Provider>
  )
}

export function useApp() {
  return useContext(AppContext)
}

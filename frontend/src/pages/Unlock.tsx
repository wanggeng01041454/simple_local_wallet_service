import { useState } from 'react'
import axios from 'axios'
import { useNavigate } from 'react-router-dom'
import { useApp } from '../context/AppContext'

export default function Unlock() {
  const navigate = useNavigate()
  const { refresh } = useApp()
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')

  const handleUnlock = async () => {
    try {
      await axios.post('/api/admin/unlock', { password })
      await refresh()
      navigate('/dashboard')
    } catch (e: any) {
      setError(e.response?.data?.error ?? 'Failed to unlock')
    }
  }

  return (
    <div>
      <h1>Unlock Wallet</h1>
      <label htmlFor="password">Password</label>
      <input
        id="password"
        type="password"
        value={password}
        onChange={e => setPassword(e.target.value)}
      />
      {error && <p role="alert">{error}</p>}
      <button onClick={handleUnlock}>Unlock</button>
    </div>
  )
}

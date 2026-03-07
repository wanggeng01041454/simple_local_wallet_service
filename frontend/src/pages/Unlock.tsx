import { useState } from 'react'
import axios from 'axios'
import { useNavigate } from 'react-router-dom'
import { useApp } from '../context/AppContext'

export default function Unlock() {
  const navigate = useNavigate()
  const { refresh } = useApp()
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)

  const handleUnlock = async () => {
    setError('')
    setLoading(true)
    try {
      await axios.post('/api/admin/unlock', { password })
      await refresh()
      navigate('/dashboard')
    } catch (e: any) {
      setError(e.response?.data?.error ?? 'Failed to unlock')
    } finally {
      setLoading(false)
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !loading) handleUnlock()
  }

  return (
    <div className="page-container">
      <h1>Unlock Wallet</h1>
      {error && <p className="alert-error" role="alert">{error}</p>}
      <div className="form-row">
        <label htmlFor="password">Password</label>
        <input
          id="password"
          type="password"
          value={password}
          onChange={e => setPassword(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={loading}
        />
      </div>
      <button className="btn-block" onClick={handleUnlock} disabled={loading}>
        {loading ? 'Unlocking...' : 'Unlock'}
      </button>
    </div>
  )
}

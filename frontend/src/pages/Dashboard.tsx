import { Link } from 'react-router-dom'
import axios from 'axios'
import { useNavigate } from 'react-router-dom'
import { useApp } from '../context/AppContext'

export default function Dashboard() {
  const navigate = useNavigate()
  const { refresh } = useApp()

  const handleLock = async () => {
    await axios.post('/api/admin/lock')
    await refresh()
    navigate('/unlock')
  }

  return (
    <div className="page-container">
      <h1>Dashboard</h1>
      <nav className="nav-bar">
        <Link to="/wallets">Wallets</Link>
        <Link to="/settings">Settings</Link>
      </nav>
      <div className="divider" />
      <button className="btn-secondary" onClick={handleLock}>Lock Wallet</button>
    </div>
  )
}

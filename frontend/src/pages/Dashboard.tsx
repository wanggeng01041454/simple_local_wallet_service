import { Link } from 'react-router-dom'

export default function Dashboard() {
  return (
    <div>
      <h1>Dashboard</h1>
      <nav>
        <Link to="/wallets">Wallets</Link>
        {' | '}
        <Link to="/settings">Settings</Link>
      </nav>
    </div>
  )
}

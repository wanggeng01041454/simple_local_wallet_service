import { useEffect, useState } from 'react'
import axios from 'axios'
import { Link } from 'react-router-dom'

const NETWORKS = ['solana', 'eth', 'bnb', 'arb'] as const

export default function Wallets() {
  const [addresses, setAddresses] = useState<Record<string, string>>({})
  const [balances, setBalances] = useState<Record<string, string>>({})

  useEffect(() => {
    NETWORKS.forEach(async network => {
      try {
        const addrResp = await axios.get(
          `http://localhost:9293/api/wallet/address?network=${network}`
        )
        setAddresses(prev => ({ ...prev, [network]: addrResp.data.address }))
        const balResp = await axios.get(
          `http://localhost:9293/api/wallet/balance?network=${network}`
        )
        setBalances(prev => ({
          ...prev,
          [network]: `${balResp.data.balance} ${balResp.data.unit}`,
        }))
      } catch {
        // locked or not set up — leave blank
      }
    })
  }, [])

  return (
    <div className="page-container">
      <nav className="nav-bar">
        <Link to="/dashboard">Dashboard</Link>
        <Link to="/settings">Settings</Link>
      </nav>
      <h1>Wallets</h1>
      {NETWORKS.map(network => (
        <div className="card" key={network}>
          <div className="card-title">{network.toUpperCase()}</div>
          <p className="card-text">
            <strong>Address:</strong> {addresses[network] ?? '—'}
          </p>
          <p className="card-text" style={{ marginTop: 4 }}>
            <strong>Balance:</strong> {balances[network] ?? '—'}
          </p>
        </div>
      ))}
    </div>
  )
}

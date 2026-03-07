import { useEffect, useState } from 'react'
import axios from 'axios'
import { Link } from 'react-router-dom'

const NETWORKS = ['solana', 'eth', 'bnb', 'arb', 'polygon'] as const

interface TokenBalance {
  token: string
  balance: string
}

export default function Wallets() {
  const [addresses, setAddresses] = useState<Record<string, string>>({})
  const [balances, setBalances] = useState<Record<string, TokenBalance[]>>({})

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
          [network]: balResp.data.balances ?? [],
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
          {(balances[network] ?? []).map(b => (
            <p className="card-text" style={{ marginTop: 4 }} key={b.token}>
              <strong>{b.token}:</strong> {b.balance}
            </p>
          ))}
          {!balances[network] && (
            <p className="card-text" style={{ marginTop: 4 }}>
              <strong>Balance:</strong> —
            </p>
          )}
        </div>
      ))}
    </div>
  )
}

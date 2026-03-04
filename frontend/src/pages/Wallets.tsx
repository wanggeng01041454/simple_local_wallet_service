import { useEffect, useState } from 'react'
import axios from 'axios'

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
    <div>
      <h1>Wallets</h1>
      {NETWORKS.map(network => (
        <div key={network}>
          <h2>{network.toUpperCase()}</h2>
          <p>Address: {addresses[network] ?? '—'}</p>
          <p>Balance: {balances[network] ?? '—'}</p>
        </div>
      ))}
    </div>
  )
}

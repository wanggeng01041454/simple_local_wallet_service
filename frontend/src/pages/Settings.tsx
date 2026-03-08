import { useEffect, useState } from 'react'
import axios from 'axios'
import { Link } from 'react-router-dom'

interface RpcSettings {
  solana: string
  eth: string
  bnb: string
  arb: string
  polygon: string
}

const EMPTY_RPC: RpcSettings = {
  solana: '',
  eth: '',
  bnb: '',
  arb: '',
  polygon: '',
}

export default function Settings() {
  const [rpc, setRpc] = useState<RpcSettings>(EMPTY_RPC)
  const [botToken, setBotToken] = useState('')
  const [chatId, setChatId] = useState('')
  const [password, setPassword] = useState('')
  const [message, setMessage] = useState('')

  useEffect(() => {
    axios.get('/api/admin/settings').then(resp => setRpc({ ...EMPTY_RPC, ...resp.data.rpc }))
  }, [])

  const saveRpc = async () => {
    await axios.post('/api/admin/settings/rpc', rpc)
    setMessage('RPC settings saved')
  }

  const saveTelegram = async () => {
    await axios.post('/api/admin/settings/telegram', {
      bot_token: botToken,
      chat_id: chatId,
      password,
    })
    setMessage('Telegram settings saved')
  }

  return (
    <div className="page-container">
      <nav className="nav-bar">
        <Link to="/dashboard">Dashboard</Link>
        <Link to="/wallets">Wallets</Link>
      </nav>
      <h1>Settings</h1>
      {message && <p className="alert-success">{message}</p>}

      <h2>RPC Endpoints</h2>
      {(['solana', 'eth', 'bnb', 'arb', 'polygon'] as const).map(net => (
        <div className="form-row" key={net}>
          <label htmlFor={`rpc-${net}`}>{net.toUpperCase()} RPC</label>
          <input
            id={`rpc-${net}`}
            value={rpc[net]}
            onChange={e => setRpc(prev => ({ ...prev, [net]: e.target.value }))}
          />
        </div>
      ))}
      <button onClick={saveRpc}>Save RPC</button>

      <div className="divider" />

      <h2>Telegram Bot</h2>
      <div className="form-row">
        <label htmlFor="bot-token">Bot Token</label>
        <input
          id="bot-token"
          value={botToken}
          onChange={e => setBotToken(e.target.value)}
        />
      </div>
      <div className="form-row">
        <label htmlFor="chat-id">Chat ID</label>
        <input id="chat-id" value={chatId} onChange={e => setChatId(e.target.value)} />
      </div>
      <div className="form-row">
        <label htmlFor="tg-password">Wallet Password (to encrypt)</label>
        <input
          id="tg-password"
          type="password"
          value={password}
          onChange={e => setPassword(e.target.value)}
        />
      </div>
      <button onClick={saveTelegram}>Save Telegram</button>
    </div>
  )
}

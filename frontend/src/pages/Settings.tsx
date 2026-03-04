import { useEffect, useState } from 'react'
import axios from 'axios'

interface RpcSettings {
  solana: string
  eth: string
  bnb: string
  arb: string
}

export default function Settings() {
  const [rpc, setRpc] = useState<RpcSettings>({ solana: '', eth: '', bnb: '', arb: '' })
  const [botToken, setBotToken] = useState('')
  const [chatId, setChatId] = useState('')
  const [password, setPassword] = useState('')
  const [message, setMessage] = useState('')

  useEffect(() => {
    axios.get('/api/admin/settings').then(resp => setRpc(resp.data.rpc))
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
    <div>
      <h1>Settings</h1>
      {message && <p>{message}</p>}

      <h2>RPC Endpoints</h2>
      {(['solana', 'eth', 'bnb', 'arb'] as const).map(net => (
        <div key={net}>
          <label htmlFor={`rpc-${net}`}>{net.toUpperCase()} RPC</label>
          <input
            id={`rpc-${net}`}
            value={rpc[net]}
            onChange={e => setRpc(prev => ({ ...prev, [net]: e.target.value }))}
          />
        </div>
      ))}
      <button onClick={saveRpc}>Save RPC</button>

      <h2>Telegram Bot</h2>
      <label htmlFor="bot-token">Bot Token</label>
      <input
        id="bot-token"
        value={botToken}
        onChange={e => setBotToken(e.target.value)}
      />
      <label htmlFor="chat-id">Chat ID</label>
      <input id="chat-id" value={chatId} onChange={e => setChatId(e.target.value)} />
      <label htmlFor="tg-password">Wallet Password (to encrypt)</label>
      <input
        id="tg-password"
        type="password"
        value={password}
        onChange={e => setPassword(e.target.value)}
      />
      <button onClick={saveTelegram}>Save Telegram</button>
    </div>
  )
}

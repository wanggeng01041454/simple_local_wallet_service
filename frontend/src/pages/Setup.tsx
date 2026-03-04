import { useState } from 'react'
import axios from 'axios'
import { useNavigate } from 'react-router-dom'

type Step = 'password' | 'wallets'
const NETWORKS = ['solana', 'eth', 'bnb', 'arb'] as const

export default function Setup() {
  const navigate = useNavigate()
  const [step, setStep] = useState<Step>('password')
  const [password, setPassword] = useState('')
  const [confirm, setConfirm] = useState('')
  const [error, setError] = useState('')
  const [createdWallets, setCreatedWallets] = useState<Record<string, string>>({})
  const [importing, setImporting] = useState<string | null>(null)
  const [importKey, setImportKey] = useState('')

  const handlePasswordNext = () => {
    if (password !== confirm) {
      setError('Passwords do not match')
      return
    }
    if (password.length < 8) {
      setError('Password must be at least 8 characters')
      return
    }
    setError('')
    setStep('wallets')
  }

  const generateWallet = async (network: string) => {
    try {
      const resp = await axios.post('/api/admin/setup/wallet', {
        password,
        network,
        action: 'generate',
      })
      setCreatedWallets(prev => ({ ...prev, [network]: resp.data.address }))
    } catch (e: any) {
      setError(e.response?.data?.error ?? 'Failed to generate wallet')
    }
  }

  const importWallet = async (network: string) => {
    try {
      const resp = await axios.post('/api/admin/setup/wallet', {
        password,
        network,
        action: 'import',
        private_key: importKey,
      })
      setCreatedWallets(prev => ({ ...prev, [network]: resp.data.address }))
      setImporting(null)
      setImportKey('')
    } catch (e: any) {
      setError(e.response?.data?.error ?? 'Failed to import wallet')
    }
  }

  const handleFinish = () => navigate('/unlock')

  if (step === 'password') {
    return (
      <div>
        <h1>Set Password</h1>
        <label htmlFor="password">Password</label>
        <input
          id="password"
          type="password"
          value={password}
          onChange={e => setPassword(e.target.value)}
        />
        <label htmlFor="confirm-password">Confirm Password</label>
        <input
          id="confirm-password"
          type="password"
          value={confirm}
          onChange={e => setConfirm(e.target.value)}
        />
        {error && <p role="alert">{error}</p>}
        <button onClick={handlePasswordNext}>Next</button>
      </div>
    )
  }

  return (
    <div>
      <h1>Import or Generate Wallets</h1>
      {error && <p role="alert">{error}</p>}
      {NETWORKS.map(network => (
        <div key={network}>
          <strong>{network.toUpperCase()}</strong>
          {createdWallets[network] ? (
            <span> {createdWallets[network]}</span>
          ) : (
            <>
              <button onClick={() => generateWallet(network)}>
                Generate {network.toUpperCase()}
              </button>
              <button onClick={() => setImporting(network)}>
                Import {network.toUpperCase()}
              </button>
              {importing === network && (
                <>
                  <input
                    value={importKey}
                    onChange={e => setImportKey(e.target.value)}
                    placeholder={
                      network === 'solana' ? 'Base58 private key' : 'Hex private key'
                    }
                  />
                  <button onClick={() => importWallet(network)}>Confirm Import</button>
                </>
              )}
            </>
          )}
        </div>
      ))}
      <button onClick={handleFinish} disabled={Object.keys(createdWallets).length === 0}>
        Finish Setup
      </button>
    </div>
  )
}

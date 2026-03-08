import { useState } from 'react'
import axios from 'axios'
import { useNavigate } from 'react-router-dom'
import { useApp } from '../context/AppContext'

type Step = 'password' | 'wallets'
const NETWORKS = ['solana', 'eth', 'bnb', 'arb', 'polygon'] as const

export default function Setup() {
  const navigate = useNavigate()
  const { refresh } = useApp()
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

  const handleFinish = async () => {
    await refresh()
    navigate('/unlock')
  }

  if (step === 'password') {
    return (
      <div className="page-container">
        <h1>Set Password</h1>
        {error && <p className="alert-error" role="alert">{error}</p>}
        <div className="form-row">
          <label htmlFor="password">Password</label>
          <input
            id="password"
            type="password"
            value={password}
            onChange={e => setPassword(e.target.value)}
          />
        </div>
        <div className="form-row">
          <label htmlFor="confirm-password">Confirm Password</label>
          <input
            id="confirm-password"
            type="password"
            value={confirm}
            onChange={e => setConfirm(e.target.value)}
          />
        </div>
        <button className="btn-block" onClick={handlePasswordNext}>Next</button>
      </div>
    )
  }

  return (
    <div className="page-container">
      <h1>Import or Generate Wallets</h1>
      {error && <p className="alert-error" role="alert">{error}</p>}
      {NETWORKS.map(network => (
        <div className="card" key={network}>
          <div className="card-title">{network.toUpperCase()}</div>
          {createdWallets[network] ? (
            <p className="card-text">{createdWallets[network]}</p>
          ) : (
            <>
              <div className="btn-row">
                <button onClick={() => generateWallet(network)}>Generate</button>
                <button
                  className="btn-secondary"
                  onClick={() => setImporting(importing === network ? null : network)}
                >
                  Import
                </button>
              </div>
              {importing === network && (
                <div style={{ marginTop: 10 }}>
                  <div className="form-row">
                    <input
                      value={importKey}
                      onChange={e => setImportKey(e.target.value)}
                      placeholder={
                        network === 'solana' ? 'Base58 private key' : 'Hex private key'
                      }
                    />
                  </div>
                  <button onClick={() => importWallet(network)}>Confirm Import</button>
                </div>
              )}
            </>
          )}
        </div>
      ))}
      <button
        className="btn-block"
        onClick={() => void handleFinish()}
        disabled={Object.keys(createdWallets).length === 0}
        style={{ marginTop: 8 }}
      >
        Finish Setup
      </button>
    </div>
  )
}

import { useEffect, useState } from 'react'
import axios from 'axios'

export default function HealthDot() {
  const [alive, setAlive] = useState(true)

  useEffect(() => {
    const check = () => {
      axios
        .get('/api/admin/health', { timeout: 3000 })
        .then(() => setAlive(true))
        .catch(() => setAlive(false))
    }
    check()
    const id = setInterval(check, 1000)
    return () => clearInterval(id)
  }, [])

  return (
    <span
      className={`health-dot ${alive ? 'health-alive' : 'health-dead'}`}
      title={alive ? 'Server alive' : 'Server unreachable'}
    />
  )
}

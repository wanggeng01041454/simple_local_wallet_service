import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { vi } from 'vitest'
import axios from 'axios'
import { MemoryRouter } from 'react-router-dom'
import Settings from './Settings'

vi.mock('axios')
const mockedAxios = axios as typeof axios & {
  get: ReturnType<typeof vi.fn>
  post: ReturnType<typeof vi.fn>
}

describe('Settings page', () => {
  beforeEach(() => {
    mockedAxios.get = vi.fn().mockResolvedValue({
      data: {
        rpc: {
          solana: 'https://api.mainnet-beta.solana.com',
          eth: 'https://eth.llamarpc.com',
          bnb: 'https://bsc-dataseed.binance.org',
          arb: 'https://arb1.arbitrum.io/rpc',
        },
      },
    })
  })

  test('loads and displays current RPC settings', async () => {
    render(
      <MemoryRouter>
        <Settings />
      </MemoryRouter>
    )
    await waitFor(() =>
      expect(screen.getByDisplayValue('https://eth.llamarpc.com')).toBeInTheDocument()
    )
  })

  test('saves RPC settings on submit', async () => {
    mockedAxios.post = vi.fn().mockResolvedValue({ data: { status: 'saved' } })
    render(
      <MemoryRouter>
        <Settings />
      </MemoryRouter>
    )
    await waitFor(() => screen.getByDisplayValue('https://eth.llamarpc.com'))
    fireEvent.click(screen.getByRole('button', { name: /save rpc/i }))
    await waitFor(() =>
      expect(mockedAxios.post).toHaveBeenCalledWith(
        '/api/admin/settings/rpc',
        expect.any(Object)
      )
    )
  })
})

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { vi } from 'vitest'
import axios from 'axios'
import { MemoryRouter } from 'react-router-dom'
import Setup from './Setup'

vi.mock('axios')
const mockedAxios = axios as typeof axios & { post: ReturnType<typeof vi.fn> }

describe('Setup wizard', () => {
  test('shows step 1 (set password) initially', () => {
    render(<MemoryRouter><Setup /></MemoryRouter>)
    expect(screen.getByText(/set password/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/^password/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/confirm password/i)).toBeInTheDocument()
  })

  test('shows error when passwords do not match', async () => {
    render(<MemoryRouter><Setup /></MemoryRouter>)
    await userEvent.type(screen.getByLabelText(/^password/i), 'abc123xx')
    await userEvent.type(screen.getByLabelText(/confirm password/i), 'different')
    fireEvent.click(screen.getByRole('button', { name: /next/i }))
    expect(screen.getByText(/passwords do not match/i)).toBeInTheDocument()
  })

  test('advances to step 2 after valid password', async () => {
    render(<MemoryRouter><Setup /></MemoryRouter>)
    await userEvent.type(screen.getByLabelText(/^password/i), 'StrongPass123!')
    await userEvent.type(screen.getByLabelText(/confirm password/i), 'StrongPass123!')
    fireEvent.click(screen.getByRole('button', { name: /next/i }))
    expect(screen.getByText(/import or generate/i)).toBeInTheDocument()
  })

  test('calls API to generate wallet on step 2', async () => {
    mockedAxios.post = vi.fn().mockResolvedValue({ data: { address: '0xABC' } })
    render(<MemoryRouter><Setup /></MemoryRouter>)
    await userEvent.type(screen.getByLabelText(/^password/i), 'StrongPass123!')
    await userEvent.type(screen.getByLabelText(/confirm password/i), 'StrongPass123!')
    fireEvent.click(screen.getByRole('button', { name: /next/i }))
    fireEvent.click(screen.getByRole('button', { name: /generate eth/i }))
    await waitFor(() =>
      expect(mockedAxios.post).toHaveBeenCalledWith(
        '/api/admin/setup/wallet',
        expect.objectContaining({ network: 'eth', action: 'generate' })
      )
    )
  })
})

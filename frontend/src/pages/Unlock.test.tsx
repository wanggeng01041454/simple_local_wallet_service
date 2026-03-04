import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { vi } from 'vitest'
import axios from 'axios'
import { MemoryRouter } from 'react-router-dom'
import Unlock from './Unlock'
import { AppProvider } from '../context/AppContext'

vi.mock('axios')
const mockedAxios = axios as typeof axios & {
  post: ReturnType<typeof vi.fn>
  get: ReturnType<typeof vi.fn>
}

describe('Unlock page', () => {
  test('renders password field and submit button', () => {
    mockedAxios.get = vi.fn().mockResolvedValue({
      data: { unlocked: false, setup_required: false },
    })
    render(
      <MemoryRouter>
        <AppProvider>
          <Unlock />
        </AppProvider>
      </MemoryRouter>
    )
    expect(screen.getByLabelText(/password/i)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /unlock/i })).toBeInTheDocument()
  })

  test('calls unlock API on submit', async () => {
    mockedAxios.post = vi.fn().mockResolvedValue({ data: { status: 'unlocked' } })
    mockedAxios.get = vi.fn().mockResolvedValue({
      data: { unlocked: true, setup_required: false },
    })
    render(
      <MemoryRouter>
        <AppProvider>
          <Unlock />
        </AppProvider>
      </MemoryRouter>
    )
    await userEvent.type(screen.getByLabelText(/password/i), 'my-password')
    fireEvent.click(screen.getByRole('button', { name: /unlock/i }))
    await waitFor(() =>
      expect(mockedAxios.post).toHaveBeenCalledWith('/api/admin/unlock', {
        password: 'my-password',
      })
    )
  })

  test('shows error on wrong password', async () => {
    mockedAxios.get = vi.fn().mockResolvedValue({
      data: { unlocked: false, setup_required: false },
    })
    mockedAxios.post = vi.fn().mockRejectedValue({
      response: { status: 401, data: { error: 'invalid password' } },
    })
    render(
      <MemoryRouter>
        <AppProvider>
          <Unlock />
        </AppProvider>
      </MemoryRouter>
    )
    await userEvent.type(screen.getByLabelText(/password/i), 'wrong')
    fireEvent.click(screen.getByRole('button', { name: /unlock/i }))
    await waitFor(() =>
      expect(screen.getByText(/invalid password/i)).toBeInTheDocument()
    )
  })
})

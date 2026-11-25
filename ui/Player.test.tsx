// @vitest-environment jsdom
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { Player } from './Player'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
const loaded = { title: 'Pattern', position: 2, duration: 16, paused: false,
  volume: 90, speed: 1, decoder: 'd3d11va', error: null }

describe('playback controls', () => {
  beforeEach(() => { vi.mocked(invoke).mockImplementation(async name => name === 'playback_state' ? loaded : undefined) })
  afterEach(() => { cleanup(); vi.resetAllMocks() })

  it('uses the backend timeline for keyboard and accessibility seeks', async () => {
    render(<Player />)
    await screen.findByText('Pattern')
    fireEvent.change(screen.getByRole('slider', { name: 'Seek' }), { target: { value: '8' } })
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('transport', { command: { kind: 'seek', value: 8 } }))
  })

  it('keeps transport disabled when initialization fails and shows the engine error', async () => {
    vi.mocked(invoke).mockResolvedValue({ ...loaded, title: null, duration: null, error: 'Missing playback runtime' })
    render(<Player />)
    await screen.findByText('Missing playback runtime')
    expect((screen.getByRole('slider', { name: 'Seek' }) as HTMLInputElement).disabled).toBe(true)
    expect((screen.getByRole('button', { name: 'Open' }) as HTMLButtonElement).disabled).toBe(false)
  })
})

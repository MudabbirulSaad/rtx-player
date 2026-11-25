import { useCallback, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Activity, FolderOpen, Maximize, Pause, Play, Volume2 } from 'lucide-react'
import { ExportPanel } from './ExportPanel'
import './player.css'

type Effects = { super_resolution: boolean; hdr: boolean }
type Playback = {
  title: string | null; position: number | null; duration: number | null
  source_fps: number | null; avsync: number | null; dropped: number | null
  rife_available: boolean; factor: number; filtered_fps: number | null
  requested_rtx: Effects; active_rtx: Effects; display_hdr: boolean | null; display_bits: number | null; hdr10_swapchain: boolean | null
  processed_width: number | null; processed_height: number | null
  source_transfer: string | null; output_transfer: string | null
  target_transfer: string | null; target_primaries: string | null; target_peak: number | null
  upscale: boolean; paused: boolean; volume: number; speed: number; decoder: string | null; error: string | null
}
type Command = { kind: 'rtx'; value: Effects } | { kind: 'upscale'; value: boolean } | { kind: 'toggle' | 'mute' } | { kind: 'volume' | 'seek' | 'speed' | 'interpolate'; value: number }

function clock(seconds: number) {
  const t = Math.floor(Math.max(0, seconds))
  const mins = Math.floor(t / 60)
  const sec = String(t % 60).padStart(2, '0')
  return mins < 60 ? `${mins}:${sec}` : `${Math.floor(mins / 60)}:${String(mins % 60).padStart(2, '0')}:${sec}`
}

export function Player() {
  const [state, setState] = useState<Playback | null>(null)
  const [issue, setIssue] = useState('')
  const [exporting, setExporting] = useState(false)
  const [stats, setStats] = useState(false)
  const [scrub, setScrub] = useState<number | null>(null)
  const [volume, setVolume] = useState<number | null>(null)
  const request = useCallback(async (name: string, args?: Record<string, unknown>) => {
    try { await invoke(name, args); setIssue('') } catch (error) { setIssue(String(error)) }
  }, [])
  const transport = useCallback((command: Command) => request('transport', { command }), [request])
  useEffect(() => {
    let active = true
    let timer: ReturnType<typeof setTimeout>
    async function update() {
      try {
        const next = await invoke<Playback>('playback_state')
        if (active) setState(next)
      } catch (error) { if (active) setIssue(String(error)) }
      if (active) timer = setTimeout(update, 250)
    }
    void update()
    return () => { active = false; clearTimeout(timer) }
  }, [])
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key.toLowerCase() === 'o') {
        e.preventDefault(); void request('open_media')
      } else if (e.key === ' ' && e.target === document.body && state?.title) {
        e.preventDefault(); void transport({ kind: 'toggle' })
      }
    }
    window.addEventListener('keydown', key)
    return () => window.removeEventListener('keydown', key)
  }, [request, transport, state?.title])

  useEffect(() => {
    if (scrub === null) return
    const timer = setTimeout(() => {
      void transport({kind: 'seek', value: scrub})
      setScrub(null)
    }, 120)
    return () => clearTimeout(timer)
  }, [scrub, transport])

  const playing = Boolean(state?.title)
  const effects = state?.requested_rtx ?? {super_resolution:false, hdr:false}
  const active = state?.active_rtx
  const sdrSource = ['bt.1886','srgb','gamma2.2','gamma2.4'].includes(state?.source_transfer ?? '')
  const hdrHint = !state?.display_hdr ? 'Enable Windows HDR on this display to convert SDR video.' :
    !sdrSource ? 'Conversion requires an SDR source.' : 'Convert SDR video to HDR using the NVIDIA driver.'
  const duration = state?.duration ?? 0
  const at = scrub ?? state?.position ?? 0
  const timing = state && [
    state.decoder || 'Decoder unavailable',
    `Source ${state.source_fps?.toFixed(2) ?? '—'} fps`,
    `Filter ${state.filtered_fps?.toFixed(2) ?? '—'} fps`,
    `Drops ${state.dropped ?? '—'}`,
    `A/V ${state.avsync == null ? '—' : Math.round(state.avsync*1000)} ms`,
  ].join(' · ')
  const output = state && [
    `Filter ${state.processed_width ?? '—'}×${state.processed_height ?? '—'} ${state.output_transfer ?? '—'}`,
    `Target ${state.target_transfer ?? '—'} ${state.target_primaries ?? '—'}` + (state.target_peak == null ? '' : ` ${Math.round(state.target_peak)} nits`),
    `Windows ${state.display_hdr == null ? 'unknown' : state.display_hdr ? 'HDR' : 'SDR'}` + (state.display_bits ? ` ${state.display_bits}-bit` : ''),
    ...(state.hdr10_swapchain ? ['HDR10 swap chain'] : []),
  ].join(' · ')
  if (exporting) return <ExportPanel close={()=>setExporting(false)}/>
  return <section className="shell flex flex-col justify-center gap-2 px-5" aria-label="Playback controls">
    <div className="flex items-center justify-between gap-3">
      {stats && timing ? <span className="metrics truncate" title={timing}>{timing}</span> :
      <span className="title truncate" title={state?.title ?? undefined}>{state?.title ?? 'Open a video'}</span>}
      <span className="time">{clock(at)} <span>/</span> {clock(duration)}</span>
    </div>
    <input type="range" className="timeline" aria-label="Seek" min={0} max={duration || 1} step={0.1}
      value={at} disabled={!duration} onChange={e => setScrub(Number(e.target.value))}
 />
    <div className="flex items-center gap-2">
      <button className="primary" title={state?.paused ? 'Play' : 'Pause'} disabled={!playing}
        aria-label={state?.paused ? 'Play' : 'Pause'} onClick={() => void transport({kind:'toggle'})}>
        {state?.paused || !playing ? <Play/> : <Pause/>}</button>
      <button onClick={() => void request('open_media')} title="Open file (Ctrl+O)"><FolderOpen/>Open</button>
      <button disabled={!playing} onClick={()=>setExporting(true)}>Export</button>
      <span className="error truncate" role="status" title={issue || state?.error || undefined}>{issue || state?.error}</span>
      <div className="ml-auto flex items-center gap-2">
        <button aria-label="Playback statistics" aria-pressed={stats} onClick={() => setStats(!stats)}><Activity/></button>
        <button aria-label="Toggle mute" disabled={!playing} onClick={() => void transport({kind:'mute'})}><Volume2/></button>
        <input className="volume" aria-label="Volume" type="range" min={0} max={100}
          disabled={!playing} value={volume ?? state?.volume ?? 100}
          onChange={e => { const value = Number(e.target.value); setVolume(value); void transport({kind:'volume',value}) }}
          onBlur={() => setVolume(null)} onPointerUp={() => setVolume(null)}/>
        <select aria-label="Playback speed" disabled={!playing} value={state?.speed ?? 1}
          onChange={e => void transport({kind:'speed', value:Number(e.target.value)})}>
          {[0.25,0.5,0.75,1,1.25,1.5,2,3,4].map(x => <option key={x} value={x}>{x}×</option>)}
        </select>
        <button aria-label="Toggle fullscreen" title="Fullscreen" onClick={() => void request('fullscreen')}><Maximize/></button>
      </div>
    </div>
    <div className="enhancements flex items-center gap-2" role="group" aria-label="Video enhancements">
      <button aria-pressed={Boolean(state?.upscale)} disabled={!playing} title="Anime4K neural upscaling for enlarged video" onClick={() => void transport({kind:'upscale',value:!state?.upscale})}>Anime4K</button>
      <button aria-label="RTX Super Resolution" aria-pressed={Boolean(active?.super_resolution)} disabled={!playing} title="Apply 2× NVIDIA video super resolution" onClick={() => void transport({kind:'rtx',value:{...effects,super_resolution:!effects.super_resolution}})}>RTX VSR</button>
      <button aria-pressed={Boolean(active?.hdr)} disabled={!playing || (!effects.hdr && (!sdrSource || !state?.display_hdr))} title={hdrHint} onClick={() => void transport({kind:'rtx',value:{...effects,hdr:!effects.hdr}})}>RTX HDR</button>
      <select aria-label="Frame interpolation" disabled={!playing || !state?.rife_available || !sdrSource} title="RIFE interpolation requires the optional provider and SDR video" value={state?.factor || 1} onChange={e => void transport({kind:'interpolate',value:Number(e.target.value)})}>
        <option value={1}>Original FPS</option>{[2,2.5,3,4,5,6].map(n => <option key={n} value={n}>RIFE {n}×</option>)}
      </select>
      {stats && output && <span className="metrics truncate" title={output}>{output}</span>}
    </div>
  </section>
}

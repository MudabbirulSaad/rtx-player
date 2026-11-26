import { useCallback, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Activity, FolderOpen, Maximize, Pause, Play, Volume2 } from 'lucide-react'
import { ExportPanel } from './ExportPanel'
import { LibraryPanel } from './LibraryPanel'
import { Enhancements } from './Enhancements'
import {Tracks, type TrackCommand,type TrackState} from './Tracks'
import {Settings,type Options} from './Settings'
import './player.css'

type Effects = { super_resolution: boolean; hdr: boolean }
type Playback = TrackState & {
  phase:string;safe_mode:boolean;options:Options;audio_devices:{name:string;description:string}[]
  fullscreen:boolean; queue_size:number; queue_index:number
  title: string | null; position: number | null; duration: number | null
  source_fps: number | null; avsync: number | null; dropped: number | null
  rife_available: boolean; rife_export_available:boolean; rife_problem: string | null; factor: number; filtered_fps: number | null
  requested_factor:number;adaptive:boolean;adaptation:string|null
  requested_rtx: Effects; active_rtx: Effects; display_hdr: boolean | null; display_bits: number | null; hdr10_swapchain: boolean | null
  processed_width: number | null; processed_height: number | null; processing_scale:number
  video_area_width:number|null;video_area_height:number|null
  source_transfer: string | null; output_transfer: string | null
  target_transfer: string | null; target_primaries: string | null; target_peak: number | null
  upscale: boolean; paused: boolean; volume: number; speed: number; decoder: string | null; error: string | null
}
type Command = TrackCommand | { kind: 'rtx'; value: Effects } | { kind: 'upscale'|'adaptive'; value: boolean } | { kind: 'toggle' | 'mute' | 'next' | 'previous' } | { kind: 'volume' | 'seek' | 'speed' | 'interpolate' | 'processingScale'; value: number }

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
  const [library, setLibrary] = useState(false)
  const [enhancements,setEnhancements]=useState(false)
  const [tracks,setTracks]=useState(false)
  const [settings,setSettings]=useState(false)
  const [stats, setStats] = useState(false)
  const [scrub, setScrub] = useState<number | null>(null)
  const [volume, setVolume] = useState<number | null>(null)
  const request = useCallback(async (name: string, args?: Record<string, unknown>) => {
    try { await invoke(name, args); setIssue('') } catch (error) { setIssue(String(error)) }
  }, [])
  const transport = useCallback((command: Command) => request('transport', { command }), [request])
  useEffect(()=>{
    document.documentElement.dataset.theme=state?.options?.theme || 'dark'
  },[state?.options?.theme])
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
      if (state?.fullscreen && e.key==='Escape') {
        e.preventDefault();void request('fullscreen')
      } else if(state?.fullscreen && e.key==='Tab') {
        void request('reveal_controls')
      } else if (e.ctrlKey && e.key.toLowerCase() === 'o') {
        e.preventDefault(); void request('open_media')
      } else if (e.key === ' ' && e.target === document.body && state?.title) {
        e.preventDefault(); void transport({ kind: 'toggle' })
      } else if(!settings&&!library&&!enhancements&&!tracks&&!exporting && state?.title &&
                !(e.target instanceof HTMLElement&&e.target.closest('input,select,textarea,[contenteditable=true]'))){
        if(e.key==='ArrowRight'||e.key==='ArrowLeft'){
          e.preventDefault();const jump=state.options?.seek_seconds??5
          void transport({kind:'seek',value:Math.max(0,Math.min(state.duration??Infinity,(state.position??0)+(e.key==='ArrowRight'?jump:-jump)))})
        }else if(e.key.toLowerCase()==='f'){e.preventDefault();void request('fullscreen')}
        else if(e.key.toLowerCase()==='m'){e.preventDefault();void transport({kind:'mute'})}
      }
    }
    window.addEventListener('keydown', key)
    return () => window.removeEventListener('keydown', key)
  }, [request, transport, state, settings,library,enhancements,tracks,exporting])

  useEffect(() => {
    if (scrub === null) return
    const timer = setTimeout(() => {
      void transport({kind: 'seek', value: scrub})
      setScrub(null)
    }, 120)
    return () => clearTimeout(timer)
  }, [scrub, transport])

  const playing = Boolean(state?.title) && state?.phase!=='error'
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
    ...(state.video_area_width && state.video_area_height ? [`Video area ${state.video_area_width}×${state.video_area_height}`] : []),
  ].join(' · ')
  if (library) return <LibraryPanel close={()=>setLibrary(false)}/>
  if(settings && state?.phase)return <Settings close={()=>setSettings(false)} initial={state.options} devices={state.audio_devices} safe={state.safe_mode}/>
  if(tracks&&state) return <Tracks state={state} close={()=>setTracks(false)} send={v=>void transport(v)} add={()=>void request('open_subtitle')} error={issue || state.error || ''}/>
  if (enhancements) return <Enhancements close={()=>setEnhancements(false)} issue={setIssue}
    adaptive={Boolean(state?.adaptive)} requested={state?.requested_factor||1} reason={state?.adaptation??null} changeAdaptive={v=>void transport({kind:'adaptive',value:v})} error={issue||state?.error||''}
    playing={playing && !state?.safe_mode} interpolationAvailable={playing && Boolean(state?.rife_available) && sdrSource} providerProblem={state?.rife_problem} hdrAvailable={playing && !state?.safe_mode && sdrSource && Boolean(state?.display_hdr)}
    factor={state?.factor || 1} scale={state?.processing_scale || 2} shader={Boolean(state?.upscale)} vsr={Boolean(active?.super_resolution)} hdr={Boolean(active?.hdr)}
    rate={v=>void transport({kind:'interpolate',value:v})} resize={v=>void transport({kind:'processingScale',value:v})}
    anime={()=>void transport({kind:'upscale',value:!state?.upscale})} nvidia={()=>void transport({kind:'rtx',value:{...effects,super_resolution:!effects.super_resolution}})}
    convert={()=>void transport({kind:'rtx',value:{...effects,hdr:!effects.hdr}})} timing={timing} output={output}/>
  if (exporting) return <ExportPanel close={()=>setExporting(false)} interpolationAvailable={Boolean(state?.rife_export_available) && sdrSource}/>
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
      <button onClick={()=>setLibrary(true)}>Library</button>
      <button disabled={!state?.phase} onClick={()=>setSettings(true)}>Settings</button>
      {state&&state.queue_size>1&&<><button aria-label="Previous in playlist" disabled={state.queue_index===0} onClick={()=>void transport({kind:'previous'})}>Previous</button>
        <button aria-label="Next in playlist" disabled={state.queue_index+1>=state.queue_size} onClick={()=>void transport({kind:'next'})}>Next</button></>}
      <button disabled={!playing} onClick={()=>setTracks(true)}>Tracks</button>
      <button disabled={!playing} onClick={()=>setExporting(true)}>Export</button>
      <span className="error truncate" role="status" title={issue || state?.error || state?.adaptation || undefined}>{issue || state?.error || state?.adaptation}</span>
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
      <button onClick={()=>setEnhancements(true)}>Enhancements</button>
      <button aria-pressed={Boolean(state?.upscale)} disabled={!playing || state?.safe_mode} title="Anime4K neural upscaling for enlarged video" onClick={() => void transport({kind:'upscale',value:!state?.upscale})}>Anime4K</button>
      <button aria-label="RTX Super Resolution" aria-pressed={Boolean(active?.super_resolution)} disabled={!playing || state?.safe_mode} title={`Apply ${state?.processing_scale || 2}× NVIDIA video super resolution`} onClick={() => void transport({kind:'rtx',value:{...effects,super_resolution:!effects.super_resolution}})}>RTX VSR</button>
      <button aria-pressed={Boolean(active?.hdr)} disabled={!playing || state?.safe_mode || (!effects.hdr && (!sdrSource || !state?.display_hdr))} title={hdrHint} onClick={() => void transport({kind:'rtx',value:{...effects,hdr:!effects.hdr}})}>RTX HDR</button>
      <select aria-label="Frame interpolation" disabled={!playing || !state?.rife_available || !sdrSource} title="RIFE interpolation requires the optional provider and SDR video" value={state?.factor || 1} onChange={e => void transport({kind:'interpolate',value:Number(e.target.value)})}>
        <option value={1}>Original FPS</option>{[2,2.5,3,4,5,6].map(n => <option key={n} value={n}>RIFE {n}×</option>)}
      </select>
      {stats && output && <span className="metrics truncate" title={output}>{output}</span>}
    </div>
  </section>
}

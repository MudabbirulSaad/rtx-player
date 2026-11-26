import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'

type Job = {state:'idle'|'cancelled'} | {state:'complete'; encoder:string} | {state:'running'; fraction:number} | {state:'error'; message:string}

export function ExportPanel({close,interpolationAvailable=false}: {close:()=>void;interpolationAvailable?:boolean}) {
  const [encoder,setEncoder] = useState('software')
  const [height,setHeight] = useState(0)
  const [quality,setQuality] = useState(22)
  const [denoise,setDenoise] = useState(false)
  const [anime,setAnime] = useState(false)
  const [hdr,setHdr] = useState(false)
  const [fps,setFps] = useState(0)
  const [bitrate,setBitrate] = useState('')
  const [job,setJob] = useState<Job>({state:'idle'})
  const [pending,setPending] = useState(false)
  const [error,setError] = useState('')
  useEffect(()=>{
    let active=true
    void invoke('expand_controls',{expanded:true}).catch(e=>{if(active)setError(String(e))})
    let timer:ReturnType<typeof setTimeout>
    async function poll() {
      try {
        const value=await invoke<Job>('export_status')
        if(active) setJob(value)
      } catch (e) { if(active) setError(String(e)) }
      if(active) timer=setTimeout(poll,400)
    }
    void poll()
    return ()=>{active=false;clearTimeout(timer);void invoke('expand_controls',{expanded:false}).catch(()=>{})}
  },[])
  async function start() {
    setPending(true)
    setError('')
    try {await invoke('start_export',{options:{encoder,height:height || null,quality,denoise,anime,hdr,target_fps:fps || null,bitrate_kbps:bitrate?Number(bitrate):null}})}
    catch(e) {setError(String(e))}
    finally {setPending(false)}
  }
  const busy=job.state==='running'
  const invalidRate=bitrate!=='' && (!Number.isInteger(Number(bitrate)) || Number(bitrate)<128 || Number(bitrate)>200000)
  return <main className="library enhance" aria-label="Export video" onKeyDown={e=>{if(e.key==='Escape')close()}}>
    <button onClick={close}>Back to player</button><header><h1>Export video</h1></header>
    <fieldset disabled={busy||pending}><legend>Encoding</legend>
      <select value={encoder} aria-label="Video encoder" onChange={e=>setEncoder(e.target.value)}>
        <option value="software">CPU</option><option value="nvenc">NVIDIA NVENC</option><option value="auto">NVENC with CPU fallback</option>
      </select>
      <label>Quality <input aria-label="Encoding quality" className="w-12" type="number" disabled={bitrate!==''} min="0" max="51" value={quality} onChange={e=>setQuality(Number(e.target.value))}/></label>
      <label>Target bitrate (kb/s) <input aria-label="Target bitrate" type="number" placeholder="Automatic" min="128" max="200000" value={bitrate} onChange={e=>setBitrate(e.target.value)}/></label>
    </fieldset>
    <fieldset disabled={busy||pending}><legend>Video processing</legend>
      <select value={height} aria-label="Output height" onChange={e=>setHeight(Number(e.target.value))}>
        <option value={0}>{anime?'2× source size':'Source size'}</option>{[480,720,1080,1440,2160].map(h=><option key={h} value={h}>{h}p</option>)}
      </select>
      <label><input type="checkbox" checked={denoise} onChange={e=>setDenoise(e.target.checked)}/> Denoise</label>
      <label><input type="checkbox" checked={anime} onChange={e=>setAnime(e.target.checked)}/> Anime4K upscaling</label>
      <label><input type="checkbox" checked={hdr} onChange={e=>setHdr(e.target.checked)}/> Convert SDR to HDR (BT.2020/PQ)</label>
      <label>Frame rate <select aria-label="Export frame rate" value={fps} disabled={!interpolationAvailable} onChange={e=>setFps(Number(e.target.value))}>
        <option value={0}>Original FPS</option>{[48,60,72,96,120,144].map(n=><option key={n} value={n}>RIFE {n} fps</option>)}
      </select></label>
      {!interpolationAvailable&&<p>Frame interpolation requires SDR video, RIFE and the optional export reader.</p>}
    </fieldset>
    <div className="flex items-center gap-3">
      {busy ? <button onClick={()=>void invoke('cancel_export').catch(e=>setError(String(e)))}>Cancel export</button> :
        <button className="primary" disabled={pending || quality<0 || quality>51 || invalidRate} onClick={()=>void start()}>Save as…</button>}
      <span role="status">{error || (job.state==='running' ? `Encoding ${Math.floor(job.fraction*100)}%` : job.state==='error' ? job.message : job.state==='complete' ? 'Export saved' : job.state==='cancelled' ? 'Export cancelled' : hdr?'10-bit HEVC and AAC audio':'H.264 video and AAC audio')}</span>
      {job.state==='complete'&&<span>{job.encoder}</span>}
    </div>
  </main>
}

import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'

type Job = {state:'idle'|'complete'|'cancelled'} | {state:'running'; fraction:number} | {state:'error'; message:string}

export function ExportPanel({close}: {close:()=>void}) {
  const [encoder,setEncoder] = useState('software')
  const [height,setHeight] = useState(0)
  const [quality,setQuality] = useState(22)
  const [denoise,setDenoise] = useState(false)
  const [job,setJob] = useState<Job>({state:'idle'})
  const [pending,setPending] = useState(false)
  const [error,setError] = useState('')
  useEffect(()=>{
    let active=true
    let timer:ReturnType<typeof setTimeout>
    async function poll() {
      try {
        const value=await invoke<Job>('export_status')
        if(active) setJob(value)
      } catch (e) { if(active) setError(String(e)) }
      if(active) timer=setTimeout(poll,400)
    }
    void poll()
    return ()=>{active=false;clearTimeout(timer)}
  },[])
  async function start() {
    setPending(true)
    setError('')
    try {await invoke('start_export',{options:{encoder,height:height || null,quality,denoise}})}
    catch(e) {setError(String(e))}
    finally {setPending(false)}
  }
  const busy=job.state==='running'
  return <section className="shell flex flex-col justify-center gap-2 px-5" aria-label="Export video">
    <div className="flex items-center gap-3"><strong>Export video</strong><button className="ml-auto" onClick={close}>Back to player</button></div>
    <fieldset disabled={busy||pending} className="flex items-center flex-wrap gap-2">
      <select value={encoder} aria-label="Video encoder" onChange={e=>setEncoder(e.target.value)}>
        <option value="software">H.264 · CPU</option><option value="nvenc">H.264 · NVENC</option>
      </select>
      <select value={height} aria-label="Output height" onChange={e=>setHeight(Number(e.target.value))}>
        <option value={0}>Source size</option>{[480,720,1080,1440,2160].map(h=><option key={h} value={h}>{h}p</option>)}
      </select>
      <label>Quality <input aria-label="Encoding quality" className="w-12" type="number" min="0" max="51" value={quality} onChange={e=>setQuality(Number(e.target.value))}/></label>
      <label><input type="checkbox" checked={denoise} onChange={e=>setDenoise(e.target.checked)}/> Denoise</label>
    </fieldset>
    <div className="flex items-center gap-3">
      {busy ? <button onClick={()=>void invoke('cancel_export').catch(e=>setError(String(e)))}>Cancel export</button> :
        <button className="primary" disabled={pending || quality<0 || quality>51} onClick={()=>void start()}>Save as…</button>}
      <span role="status">{error || (job.state==='running' ? `Encoding ${Math.floor(job.fraction*100)}%` : job.state==='error' ? job.message : job.state==='complete' ? 'Export saved' : job.state==='cancelled' ? 'Export cancelled' : 'H.264 video and AAC audio')}</span>
    </div>
  </section>
}

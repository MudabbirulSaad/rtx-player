import {useEffect, useRef,useState} from 'react'
import {ArrowLeft} from 'lucide-react'
import {invoke} from '@tauri-apps/api/core'

type Props = {
  close:()=>void; issue:(message:string)=>void
  playing:boolean; interpolationAvailable:boolean; hdrAvailable:boolean
  providerProblem?:string|null
  factor:number; scale:number; shader:boolean; vsr:boolean; hdr:boolean
  rate:(value:number)=>void; resize:(value:number)=>void
  anime:()=>void; nvidia:()=>void; convert:()=>void
  timing:string|null; output:string|null
  adaptive:boolean; requested:number; reason:string|null; changeAdaptive:(enabled:boolean)=>void; error:string
}
type GPU={name:string|null;compute:string|null;gpu_percent:number|null;used_memory:number|null;total_memory:number|null;temperature_c:number|null;power_w:number|null;decoder_percent:number|null}

export function Enhancements(props:Props) {
  const back=useRef<HTMLButtonElement>(null)
  const [gpu,setGpu]=useState<{devices:GPU[];driver:string|null;error:string|null}|null>(null)
  useEffect(()=>{
    let active=true
    let timer:ReturnType<typeof setTimeout>
    async function sample(){
      try{const next=await invoke<NonNullable<typeof gpu>>('gpu_stats');if(active)setGpu(next)}
      catch(e){if(active)props.issue(String(e))}
      if(active)timer=setTimeout(sample,2000)
    }
    void sample()
    return ()=>{active=false;clearTimeout(timer)}
  },[props.issue])
  useEffect(()=>{
    void invoke('expand_controls',{expanded:true}).then(()=>back.current?.focus()).catch(e=>props.issue(String(e)))
    return ()=>{void invoke('expand_controls',{expanded:false}).catch(()=>{})}
  },[props.issue])
  return <main className="library enhance" onKeyDown={e=>{if(e.key==='Escape')props.close()}}>
    <button ref={back} onClick={props.close}><ArrowLeft/>Back to player</button>
    <header><h1>Video enhancements</h1></header>
    <fieldset disabled={!props.playing}><legend>Spatial processing</legend>
      <button aria-pressed={props.shader} onClick={props.anime}>Anime4K</button>
      <button aria-pressed={props.vsr} onClick={props.nvidia}>RTX Super Resolution</button>
    </fieldset>
    <fieldset disabled={!props.playing}><legend>RTX scale</legend>
      {[1.5,2,3,4].map(v=><label className="rate" key={v}><input type="radio" name="rtx-scale" aria-label={`RTX scale ${v}×`}
        checked={props.scale===v} onChange={()=>props.resize(v)}/>{v}×</label>)}
    </fieldset>
    <fieldset disabled={!props.interpolationAvailable}><legend>Frame interpolation</legend>
      {[1,2,2.5,3,4,5,6].map(v=><label className="rate" key={v}><input type="radio" name="rife-rate"
        checked={props.factor===v} onChange={()=>props.rate(v)}/>{v===1?'Original FPS':`RIFE ${v}×`}</label>)}
    </fieldset>
    {!props.interpolationAvailable&&<p>{props.providerProblem || 'RIFE requires an SDR source.'}</p>}
    <section className="library-options"><label><input type="checkbox" checked={props.adaptive} onChange={e=>props.changeAdaptive(e.target.checked)}/>Reduce interpolation when playback falls behind</label>
      {props.requested!==props.factor&&<button onClick={()=>props.rate(props.requested)}>Retry requested {props.requested}×</button>}
    </section>
    {props.reason&&<p role="status">{props.reason}</p>}
    <fieldset><legend>SDR to HDR</legend>
      <button aria-pressed={props.hdr} disabled={!props.hdrAvailable} onClick={props.convert}>RTX HDR</button>
      {!props.hdrAvailable&&<span>Requires SDR video and an active Windows HDR display.</span>}
    </fieldset>
    {props.playing&&<section aria-label="Processing statistics"><h2>Playback statistics</h2><p>{props.timing}</p><p>{props.output}</p></section>}
    {gpu?.error&&<p>{gpu.error}</p>}
    {gpu?.devices.map((dev,i)=><section key={i} aria-label="GPU telemetry"><h2>{dev.name??`GPU ${i}`}</h2>
      <p>Driver {gpu.driver??'—'} · Compute capability {dev.compute??'—'}</p>
      <p>GPU {dev.gpu_percent??'—'}% · Decoder {dev.decoder_percent??'—'}% · Device memory {dev.used_memory==null?'—':(dev.used_memory/2**30).toFixed(1)} / {dev.total_memory==null?'—':(dev.total_memory/2**30).toFixed(1)} GiB · {dev.temperature_c??'—'} °C · {dev.power_w==null?'—':dev.power_w.toFixed(0)} W</p>
    </section>)}
    {props.error&&<p role="alert">{props.error}</p>}
  </main>
}

import {useEffect,useRef,useState} from 'react'
import {invoke} from '@tauri-apps/api/core'
import {ArrowLeft} from 'lucide-react'

export type Options={
  software_decode:boolean; adaptive:boolean; scale:number; audio_device:string
  subtitles:boolean; theme:'dark'|'light'|'system'; seek_seconds:number; adapter:string|null
}
export const defaults:Options={software_decode:false,adaptive:false,scale:2,audio_device:'auto',subtitles:true,theme:'dark',seek_seconds:5,adapter:null}
type Policy={kib:number;days:number}
type Device={name:string;description:string}

export function Settings({initial,devices,safe,rife,reader,recovered,close}:{initial:Options;devices:Device[];safe:boolean;rife:boolean;reader:boolean;recovered:boolean;close:()=>void}){
  const [options,setOptions]=useState(initial)
  const [adapters,setAdapters]=useState<string[]>([])
  const [policy,setPolicy]=useState<Policy|null>(null)
  const [message,setMessage]=useState('')
  const [busy,setBusy]=useState(false)
  const [setup,setSetup]=useState({running:false,message:''})
  const back=useRef<HTMLButtonElement>(null)
  useEffect(()=>{
    let live=true
    void invoke('expand_controls',{expanded:true}).then(()=>back.current?.focus()).catch(e=>{if(live)setMessage(String(e))})
    void invoke<Policy>('diagnostic_policy').then(p=>{if(live)setPolicy(p)}).catch(e=>{if(live)setMessage(String(e))})
    void invoke<{devices:{name:string|null}[]}>('gpu_stats').then(r=>{if(live)setAdapters(r.devices.flatMap(d=>d.name?[d.name]:[]))}).catch(()=>{})
    return ()=>{live=false;void invoke('expand_controls',{expanded:false}).catch(()=>{})}
  },[])
  useEffect(()=>{
    let live=true
    let timer:ReturnType<typeof setTimeout>
    async function update(){
      try{const status=await invoke<typeof setup>('provider_status');if(live && status)setSetup(status)}
      catch(e){if(live)setMessage(String(e))}
      if(live)timer=setTimeout(update,800)
    }
    void update()
    return ()=>{live=false;clearTimeout(timer)}
  },[])
  async function install(reader:boolean){
    try{await invoke('setup_provider',{reader});setSetup({running:true,message:'Starting setup'})}
    catch(e){setMessage(String(e))}
  }
  function change<K extends keyof Options>(key:K,value:Options[K]){setOptions({...options,[key]:value});setMessage('')}
  async function save(){
    setBusy(true)
    try{await invoke('set_player_options',{options});if(policy)await invoke('configure_diagnostics',policy);setMessage('Preferences saved')}
    catch(e){setMessage(String(e))}finally{setBusy(false)}
  }
  async function diagnostic(){
    try{if(await invoke<boolean>('export_diagnostics'))setMessage('Diagnostic report saved')}
    catch(e){setMessage(String(e))}
  }
  return <main className="library enhance" onKeyDown={e=>{if(e.key==='Escape')close()}}>
    <button ref={back} onClick={close}><ArrowLeft/>Back to player</button>
    <header><h1>Settings</h1></header>
    {recovered&&<p role="status">Your original library database is damaged and has been preserved. New settings and library entries use library-recovery.sqlite3.</p>}
    {safe&&<p role="status">Safe mode is active. Playback uses software decoding and disables enhancements.</p>}
    <fieldset><legend>Playback defaults</legend>
      <label><input type="checkbox" checked={options.software_decode} onChange={e=>change('software_decode',e.target.checked)}/> Use software decoding</label>
      <label><input type="checkbox" checked={options.subtitles} onChange={e=>change('subtitles',e.target.checked)}/> Show selected subtitles</label>
      <label>Audio output <select value={options.audio_device} onChange={e=>change('audio_device',e.target.value)}>
        <option value="auto">System default</option>{devices.filter(d=>d.name!=='auto').map(d=><option value={d.name} key={d.name}>{d.description}</option>)}</select></label>
      <label>Direct3D adapter after restart <select value={options.adapter??''} onChange={e=>change('adapter',e.target.value||null)}>
        <option value="">Automatic</option>{[...new Set(adapters)].map(a=><option key={a}>{a}</option>)}</select></label>
    </fieldset>
    <fieldset><legend>Enhancement defaults</legend>
      <label><input type="checkbox" checked={options.adaptive} onChange={e=>change('adaptive',e.target.checked)}/> Reduce interpolation on overload</label>
      <label>RTX scale <select value={options.scale} onChange={e=>change('scale',Number(e.target.value))}>{[1.5,2,3,4].map(s=><option value={s} key={s}>{s}×</option>)}</select></label>
    </fieldset>
    <fieldset><legend>Appearance and keyboard</legend>
      <label>Appearance <select value={options.theme} onChange={e=>change('theme',e.target.value as Options['theme'])}><option value="dark">Dark</option><option value="light">Light</option><option value="system">System</option></select></label>
      <label>Arrow key seek <select value={options.seek_seconds} onChange={e=>change('seek_seconds',Number(e.target.value))}>{[5,10,30].map(n=><option value={n} key={n}>{n} seconds</option>)}</select></label>
    </fieldset>
    <fieldset disabled={safe}><legend>Optional frame interpolation</legend>
      <p>RIFE downloads the model and runtime into the player folder. BestSource adds frame interpolation for export. Restart after installing each provider.</p>
      <button disabled={rife||setup.running} onClick={()=>void install(false)}>Install RIFE for playback</button>
      <button disabled={!rife||reader||setup.running} onClick={()=>void install(true)}>Install BestSource for export</button>
      {setup.running&&<button onClick={()=>void invoke('cancel_provider').catch(e=>setMessage(String(e)))}>Cancel setup</button>}
      {setup.message&&<p role="status">{setup.message}</p>}
    </fieldset>
    <fieldset><legend>Local diagnostics</legend>
      {policy&&<><label>Log size <select value={policy.kib} onChange={e=>setPolicy({...policy,kib:Number(e.target.value)})}>{[64,256,1024].map(k=><option key={k} value={k}>{k} KiB</option>)}</select></label>
      <label>Retention <select value={policy.days} onChange={e=>setPolicy({...policy,days:Number(e.target.value)})}>{[1,7,30].map(d=><option value={d} key={d}>{d} days</option>)}</select></label></>}
      <button onClick={()=>void diagnostic()}>Save diagnostic report</button>
      <p>Reports contain timing, renderer, GPU and recovery data. Media names and paths are excluded.</p>
    </fieldset>
    <div className="library-options"><button className="primary" disabled={busy} onClick={()=>void save()}>Save preferences</button>
      <button onClick={()=>{setOptions({...defaults});setPolicy({kib:256,days:7});setMessage('Default values selected')}}>Use defaults</button></div>
    {message&&<p role="status">{message}</p>}
  </main>
}

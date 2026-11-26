import {useEffect,useState} from 'react'
import {invoke} from '@tauri-apps/api/core'

function previewImage(bytes:ArrayBuffer):Promise<string>{
  return new Promise((resolve,reject)=>{
    const reader=new FileReader()
    reader.onload=()=>resolve(String(reader.result));reader.onerror=()=>reject(reader.error)
    reader.readAsDataURL(new Blob([bytes],{type:'image/jpeg'}))
  })
}
export function Timeline({at,duration,buffered,seek}:{at:number;duration:number;buffered:number;seek:(n:number)=>void}){
  const [hover,setHover]=useState<number|null>(null)
  const [image,setImage]=useState<{time:number;url:string}|null>(null)
  useEffect(()=>{
    if(hover===null||duration<=0)return
    let active=true
    const timer=setTimeout(async()=>{
      try{
        const url=await previewImage(await invoke<ArrayBuffer>('seek_preview',{seconds:hover}))
        if(active)setImage({time:hover,url})
      }catch{if(active)setImage(null)}
    },250)
    return ()=>{active=false;clearTimeout(timer)}
  },[hover,duration])
  const position=duration?100*at/duration:0
  return <div className="seek-bar" onPointerLeave={()=>setHover(null)}>
    {duration>0&&buffered>0&&<span className="buffered" aria-hidden="true" style={{left:`${position}%`,width:`${Math.max(0,Math.min(100-position,100*buffered/duration))}%`}}/>}
    <input type="range" className="timeline" aria-label="Seek" min={0} max={duration||1} step={0.1} value={at} disabled={!duration}
      onChange={e=>seek(Number(e.target.value))} onPointerMove={e=>{
        const box=e.currentTarget.getBoundingClientRect()
        if(duration>0&&box.width>0)setHover(Math.min(Math.max(0,duration-.05),Math.max(0,Math.floor((e.clientX-box.left)*duration/box.width))))
      }}/>
    {hover!==null&&image?.time===hover&&<aside className="seek-preview" style={{left:`${Math.min(88,Math.max(1,100*hover/duration))}%`}}>
      <img src={image.url} alt={`Preview at ${hover} seconds`}/><span>{Math.floor(hover/60)}:{String(Math.floor(hover%60)).padStart(2,'0')}</span>
    </aside>}
  </div>
}

import {useEffect,useRef} from 'react'
import {invoke} from '@tauri-apps/api/core'
import {ArrowLeft} from 'lucide-react'

type Track={id:number;kind:string;title:string|null;language:string|null;codec:string|null}
type Chapter={index:number;title:string|null;time:number}
export type TrackState={
  streams:{tracks:Track[];chapters:Chapter[]};audio_id:number|null;subtitle_id:number|null;chapter:number|null;audio_delay:number;subtitle_delay:number
}
export type TrackCommand={kind:'audioTrack'|'audioDelay'|'subtitleDelay'|'chapter';value:number}|{kind:'subtitleTrack';value:number|null}

type Props={state:TrackState;close:()=>void;send:(command:TrackCommand)=>void;add:()=>void;error:string}
export function Tracks({state,close,send,add,error}:Props){
  const back=useRef<HTMLButtonElement>(null)
  useEffect(()=>{
    void invoke('expand_controls',{expanded:true}).then(()=>back.current?.focus())
    return ()=>{void invoke('expand_controls',{expanded:false})}
  },[])
  const choices=(kind:string)=>state.streams.tracks.filter(t=>t.kind===kind)
  const title=(track:Track)=>[track.title || `Track ${track.id}`,track.language,track.codec].filter(Boolean).join(' · ')
  return <main className="library enhance" onKeyDown={e=>{if(e.key==='Escape')close()}}>
    <button ref={back} onClick={close}><ArrowLeft/>Back to player</button>
    <header><h1>Tracks and chapters</h1></header>
    <fieldset><legend>Audio</legend>{choices('audio').map(t=><label className="rate" key={t.id}>
      <input type="radio" name="audio-track" checked={t.id===state.audio_id} onChange={()=>send({kind:'audioTrack',value:t.id})}/>{title(t)}</label>)}
      <label>Audio delay <input type="number" aria-label="Audio delay in seconds" min={-10} max={10} step={0.1} value={state.audio_delay}
        onChange={e=>{if(e.target.value)send({kind:'audioDelay',value:Number(e.target.value)})}}/> s</label>
    </fieldset>
    <fieldset><legend>Subtitles</legend>
      <label className="rate"><input type="radio" name="sub-track" checked={state.subtitle_id==null} onChange={()=>send({kind:'subtitleTrack',value:null})}/>Off</label>
      {choices('sub').map(t=><label className="rate" key={t.id}><input type="radio" name="sub-track" checked={t.id===state.subtitle_id} onChange={()=>send({kind:'subtitleTrack',value:t.id})}/>{title(t)}</label>)}
      <button onClick={add}>Add subtitle file</button>
      <label>Subtitle delay <input type="number" aria-label="Subtitle delay in seconds" min={-60} max={60} step={0.1} value={state.subtitle_delay}
        onChange={e=>{if(e.target.value)send({kind:'subtitleDelay',value:Number(e.target.value)})}}/> s</label>
    </fieldset>
    {state.streams.chapters.length>0&&<fieldset><legend>Chapters</legend>{state.streams.chapters.map(ch=>
      <button key={ch.index} aria-current={state.chapter===ch.index?'true':undefined} onClick={()=>send({kind:'chapter',value:ch.index})}>
        {Math.floor(ch.time/60)}:{String(Math.floor(ch.time%60)).padStart(2,'0')} · {ch.title || `Chapter ${ch.index+1}`}
      </button>)}</fieldset>}
    {error&&<p role="alert">{error}</p>}
  </main>
}

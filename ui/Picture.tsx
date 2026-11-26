import {useState} from 'react'

export type PictureOptions={
  brightness:number; contrast:number; saturation:number; gamma:number; sharpness:number
  deband:boolean; deinterlace:string; aspect:string; crop:string
}
const original:PictureOptions={brightness:0,contrast:0,saturation:0,gamma:0,sharpness:0,deband:false,deinterlace:'no',aspect:'source',crop:'source'}
const sliders=[['brightness','Brightness'],['contrast','Contrast'],['saturation','Saturation'],['gamma','Gamma'],['sharpness','Sharpening']] as const
export function Picture({current,apply}:{current:PictureOptions;apply:(value:PictureOptions)=>void}){
  const [draft,setDraft]=useState(current)
  return <fieldset><legend>Picture adjustments</legend>
    {sliders.map(([key,text])=><label key={key}>{text} <input type="number" aria-label={text} min={key==='sharpness'?0:-100} max={100} value={draft[key]}
      onChange={e=>setDraft({...draft,[key]:Number(e.target.value)})}/></label>)}
    <label><input type="checkbox" checked={draft.deband} onChange={e=>setDraft({...draft,deband:e.target.checked})}/> Debanding</label>
    <label>Deinterlacing <select value={draft.deinterlace} onChange={e=>setDraft({...draft,deinterlace:e.target.value})}>
      <option value="no">Off</option><option value="auto">Auto</option><option value="yes">On</option></select></label>
    {(['aspect','crop'] as const).map(key=><label key={key}>{key==='aspect'?'Aspect ratio':'Center crop'} <select value={draft[key]} onChange={e=>setDraft({...draft,[key]:e.target.value})}>
      {['source','16:9','4:3','1:1','2.39:1'].map(v=><option key={v} value={v}>{v==='source'?'Source':v}</option>)}</select></label>)}
    <button onClick={()=>apply(draft)}>Apply picture adjustments</button>
    <button onClick={()=>{setDraft({...original});apply({...original})}}>Reset picture</button>
  </fieldset>
}

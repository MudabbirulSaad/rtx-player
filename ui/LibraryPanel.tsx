import {useEffect, useRef, useState} from 'react'
import {invoke} from '@tauri-apps/api/core'
import {ArrowLeft, Film, Search} from 'lucide-react'
import {Collections} from './Collections'

type Entry={id:number; title:string; position:number; duration:number}
type Preferences={remember:boolean; resume:boolean}

export function LibraryPanel({close}:{close:()=>void}) {
  const [search,setSearch]=useState('')
  const [items,setItems]=useState<Entry[]>([])
  const [prefs,setPrefs]=useState<Preferences|null>(null)
  const [revision,setRevision]=useState(0)
  const [confirm,setConfirm]=useState(false)
  const [collections,setCollections]=useState(false)
  const [error,setError]=useState('')
  const back=useRef<HTMLButtonElement>(null)
  useEffect(()=>{
    let active=true
    void invoke('expand_controls',{expanded:true}).then(()=>back.current?.focus()).catch(e=>{if(active)setError(String(e))})
    void invoke<Preferences>('library_preferences').then(p=>{if(active)setPrefs(p)}).catch(e=>{if(active)setError(String(e))})
    return ()=>{active=false;void invoke('expand_controls',{expanded:false}).catch(()=>{})}
  },[])
  useEffect(()=>{
    let active=true
    void invoke<Entry[]>('library_entries',{search}).then(rows=>{if(active)setItems(rows)}).catch(e=>{if(active)setError(String(e))})
    return ()=>{active=false}
  },[search,revision])
  async function preference(next:Preferences){
    try{await invoke('set_library_preferences',next);setPrefs(next);setRevision(v=>v+1);setError('')}
    catch(e){setError(String(e))}
  }
  async function clear(){
    try{await invoke('clear_history');setConfirm(false);setRevision(v=>v+1);setError('')}
    catch(e){setError(String(e))}
  }
  async function open(id:number){
    try{await invoke('open_recent',{id});close()}catch(e){setError(String(e))}
  }
  return <main className="library" onKeyDown={e=>{if(e.key==='Escape')close()}}>
    <button ref={back} onClick={close}><ArrowLeft/>Back to player</button>
    <header><h1>Library</h1>
      {!collections&&<label className="search"><Search/><input type="search" aria-label="Search recent videos" placeholder="Search recent videos" value={search} onChange={e=>setSearch(e.target.value)}/></label>}
    </header>
    <nav className="library-options" aria-label="Library views"><button aria-pressed={!collections} onClick={()=>setCollections(false)}>History</button><button aria-pressed={collections} onClick={()=>setCollections(true)}>Folders and playlists</button></nav>
    {collections?<Collections close={close}/>:<>
    <section aria-label="History preferences" className="library-options">
      <label><input type="checkbox" disabled={!prefs} checked={prefs?.remember??false} onChange={e=>prefs&&void preference({...prefs,remember:e.target.checked})}/> Remember recent videos</label>
      <label><input type="checkbox" disabled={!prefs?.remember} checked={prefs?.resume??true} onChange={e=>prefs&&void preference({...prefs,resume:e.target.checked})}/> Resume playback</label>
      {items.length>0 && <button onClick={()=>setConfirm(true)}>Clear history</button>}
    </section>
    {confirm&&<div className="confirm" role="group" aria-label="Confirm clearing history">Remove all recent entries?
      <button onClick={()=>void clear()}>Remove entries</button><button onClick={()=>setConfirm(false)}>Cancel</button></div>}
    {error&&<p role="alert">{error}</p>}
    <h2>Recent videos</h2>
    {items.length===0 ? <p>{search?'No matching videos.':prefs?.remember?'Open a video to add it to your history.':'Turn on history to keep a list of videos on this device.'}</p>:
      <ul>{items.map(item=><li key={item.id}><button onClick={()=>void open(item.id)}><Film/><strong>{item.title}</strong>
        {item.position>0 && <span>Resume at {Math.floor(item.position/60)}:{String(Math.floor(item.position%60)).padStart(2,'0')}</span>}</button></li>)}</ul>}
    </>}
  </main>
}

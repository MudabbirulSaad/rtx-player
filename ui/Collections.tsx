import {useEffect, useState} from 'react'
import {invoke} from '@tauri-apps/api/core'
import {FolderOpen, ListVideo, Play, X} from 'lucide-react'

type Collection={id:number;name:string;is_folder:boolean;count:number}
type Item={ordinal:number;title:string;bytes:number;duration:number}

export function Collections({close}:{close:()=>void}) {
  const [groups,setGroups]=useState<Collection[]>([])
  const [selected,setSelected]=useState<number|null>(null)
  const [items,setItems]=useState<Item[]>([])
  const [search,setSearch]=useState('')
  const [name,setName]=useState('')
  const [creating,setCreating]=useState(false)
  const [removing,setRemoving]=useState(false)
  const [revision,setRevision]=useState(0)
  const [busy,setBusy]=useState(false)
  const [error,setError]=useState('')
  const [previews,setPreviews]=useState<Record<string,string>>({})
  const group=groups.find(g=>g.id===selected)
  useEffect(()=>{
    let active=true
    void invoke<Collection[]>('list_collections').then(rows=>{if(active)setGroups(rows)}).catch(e=>{if(active)setError(String(e))})
    return ()=>{active=false}
  },[revision])
  useEffect(()=>{
    let active=true
    setItems([])
    if(selected!==null)void invoke<Item[]>('collection_items',{id:selected,search}).then(rows=>{if(active)setItems(rows)}).catch(e=>{if(active)setError(String(e))})
    return ()=>{active=false}
  },[selected,search,revision])
  async function edit(work:()=>Promise<void>) {
    setBusy(true)
    try {await work();setRevision(v=>v+1);setError('')} catch(e){setError(String(e))} finally{setBusy(false)}
  }
  function select(id:number|null){setSelected(id);setSearch('');setRemoving(false)}
  async function play(start:number) {
    try {await invoke('play_collection',{id:selected,start});close()}catch(e){setError(String(e))}
  }
  async function preview(ordinal:number) {
    const key=`${selected}/${ordinal}`
    setBusy(true)
    try {
      const image=await invoke<ArrayBuffer>('collection_preview',{id:selected,ordinal})
      const reader=new FileReader()
      reader.onload=()=>setPreviews(previous=>({...previous,[key]:String(reader.result)}))
      reader.readAsDataURL(new Blob([image],{type:'image/jpeg'}))
    } catch(e){setError(String(e))} finally{setBusy(false)}
  }
  return <section aria-label="Folders and playlists">
    {group ? <>
      <button onClick={()=>select(null)}>All collections</button><h2>{group.name}</h2>
      <div className="library-options">
        <button disabled={!group.count||busy} onClick={()=>void play(0)}><Play/>Play all</button>
        <button disabled={busy} onClick={()=>void edit(()=>invoke(group.is_folder?'refresh_folder':'add_playlist_files',{id:selected}))}>
          {group.is_folder?'Refresh folder':'Add videos'}
        </button>
        <button disabled={busy} onClick={()=>setRemoving(true)}>Remove collection</button>
      </div>
      {removing&&<div className="confirm" role="group" aria-label="Confirm collection removal">Remove this collection from the library?
        <button onClick={()=>void edit(async()=>{await invoke('remove_collection',{id:selected});select(null)})}>Remove</button>
        <button onClick={()=>setRemoving(false)}>Cancel</button></div>}
      <input type="search" aria-label="Search this collection" value={search} onChange={e=>setSearch(e.target.value)} placeholder="Search this collection"/>
      <ul className="collection-items">{items.map(item=><li key={item.ordinal}><button onClick={()=>void play(item.ordinal)}>{previews[`${selected}/${item.ordinal}`]?<img width="96" alt="" src={previews[`${selected}/${item.ordinal}`]}/>:<Play/>}<strong>{item.title}</strong><span>{(item.bytes/1048576).toFixed(1)} MiB{item.duration>0?` · ${Math.floor(item.duration/60)} min`:''}</span></button>
        <button disabled={busy} onClick={()=>void preview(item.ordinal)} aria-label={`Preview ${item.title}`}>Preview</button>
        {!group.is_folder&&<button title="Remove from playlist" aria-label={`Remove ${item.title} from playlist`} onClick={()=>void edit(()=>invoke('remove_playlist_item',{id:selected,ordinal:item.ordinal}))}><X/></button>}</li>)}</ul>
      {!items.length&&<p>{search?'No matching videos.':group.is_folder?'This folder has no supported videos.':'Add videos to start this playlist.'}</p>}
    </> : <>
      <div className="library-options"><button disabled={busy} onClick={()=>void edit(async()=>{const id=await invoke<number|null>('import_folder');if(id!==null)select(id)})}><FolderOpen/>Add folder</button>
        <button onClick={()=>setCreating(true)}><ListVideo/>New playlist</button>
        <button disabled={busy} onClick={()=>void edit(async()=>{await invoke('clear_previews');setPreviews({})})}>Clear cached previews</button></div>
      {creating&&<form onSubmit={e=>{e.preventDefault();void edit(async()=>{const id=await invoke<number>('create_playlist',{name:name.trim()});select(id);setCreating(false);setName('')})}}>
        <label>Playlist name <input autoFocus maxLength={120} value={name} onChange={e=>setName(e.target.value)}/></label><button disabled={busy||!name.trim()}>Create</button>
        <button type="button" onClick={()=>setCreating(false)}>Cancel</button></form>}
      <ul>{groups.map(g=><li key={g.id}><button onClick={()=>select(g.id)}>{g.is_folder?<FolderOpen/>:<ListVideo/>}<strong>{g.name}</strong><span>{g.count} videos</span></button></li>)}</ul>
    </>}
    {error&&<p role="alert">{error}</p>}
  </section>
}

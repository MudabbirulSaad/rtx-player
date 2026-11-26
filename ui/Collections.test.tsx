// @vitest-environment jsdom
import {render,screen,fireEvent,waitFor} from '@testing-library/react'
import {expect,it,vi} from 'vitest'
import {invoke} from '@tauri-apps/api/core'
import {Collections} from './Collections'

vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}))
it('plays the collection from the selected member in stored order',async()=>{
  const close=vi.fn()
  vi.mocked(invoke).mockImplementation(async name=>{
    if(name==='list_collections') return [{id:4,name:'Short films',count:2,is_folder:false}]
    if(name==='collection_items')return [{ordinal:0,title:'First',bytes:10,duration:0},{ordinal:1,title:'Second',bytes:12,duration:0}]
  })
  render(<Collections close={close}/>)
  fireEvent.click(await screen.findByText('Short films'))
  fireEvent.click(await screen.findByText('Second'))
  await waitFor(()=>expect(close).toHaveBeenCalled())
  expect(invoke).toHaveBeenCalledWith('play_collection',{id:4,start:1})
})

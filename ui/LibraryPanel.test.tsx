// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react'
import {afterEach,expect,it,vi} from 'vitest'
import {invoke} from '@tauri-apps/api/core'
import {LibraryPanel} from './LibraryPanel'
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}))
afterEach(()=>{cleanup();vi.resetAllMocks()})
it('adds an explicit opt-in before recording history',async()=>{
  vi.mocked(invoke).mockImplementation(async name=>name==='library_entries'?[]:name==='library_preferences'?{remember:false,resume:true}:undefined)
  render(<LibraryPanel close={()=>{}}/>)
  await waitFor(()=>expect((screen.getByLabelText('Remember recent videos') as HTMLInputElement).disabled).toBe(false))
  fireEvent.click(screen.getByLabelText('Remember recent videos'))
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith('set_library_preferences',{remember:true,resume:true}))
})
it('opens a stored entry by its database id',async()=>{
  const close=vi.fn()
  vi.mocked(invoke).mockImplementation(async name=>name==='library_entries'?[{id:3,title:'Cartoon',position:70,duration:180}]:name==='library_preferences'?{remember:true,resume:true}:undefined)
  render(<LibraryPanel close={close}/>)
  fireEvent.click(await screen.findByText('Cartoon'))
  await waitFor(()=>expect(close).toHaveBeenCalled())
  expect(invoke).toHaveBeenCalledWith('open_recent',{id:3})
})

// @vitest-environment jsdom
import {render, screen, fireEvent, waitFor, cleanup} from '@testing-library/react'
import {afterEach, expect, it, vi} from 'vitest'
import {invoke} from '@tauri-apps/api/core'
import {ExportPanel} from './ExportPanel'

vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}))
afterEach(()=>{cleanup();vi.resetAllMocks()})

it('exports with the selected resolution and denoiser',async()=>{
  vi.mocked(invoke).mockImplementation(async name=>name==='export_status'?{state:'idle'}:undefined)
  render(<ExportPanel close={()=>{}}/>)
  fireEvent.change(screen.getByLabelText('Output height'),{target:{value:'1080'}})
  fireEvent.click(screen.getByLabelText('Denoise'))
  fireEvent.click(screen.getByText('Save as…'))
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith('start_export',{options:{
    encoder:'software',height:1080,quality:22,denoise:true,anime:false,hdr:false,bitrate_kbps:null,
  }}))
})

it('shows job progress and sends cancellation to the worker',async()=>{
  vi.mocked(invoke).mockImplementation(async name=>name==='export_status'?{state:'running',fraction:0.32}:undefined)
  render(<ExportPanel close={()=>{}}/>)
  await screen.findByText('Encoding 32%')
  fireEvent.click(screen.getByText('Cancel export'))
  expect(invoke).toHaveBeenCalledWith('cancel_export')
})

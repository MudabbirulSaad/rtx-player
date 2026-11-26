# Architecture

The Windows host creates a child HWND for Direct3D video and a sibling WebView2 surface for the controls. The renderer presents video directly to the child window. React sends typed playback commands over Tauri IPC and polls a playback snapshot. Resizing, fullscreen and the expanded panels move the native surfaces together.

## Playback and processing

One Rust worker owns the libmpv handle, drains its event queue and serializes filter changes. A bounded channel limits queued commands. libmpv performs demuxing, decoding, A/V scheduling, subtitles and WASAPI audio. Playback opens each selected container with a specific demuxer. A playlist contains paths explicitly added to the local library.

The default path preserves the source frames. On compatible hardware libmpv uses D3D11VA. Selecting a processing resolution or RIFE switches to copy-back decoding. The optional input filter resizes or resamples cadence before VapourSynth calls the RIFE NCNN/Vulkan provider. Its scene-cut guard holds the original across a cut. The D3D11 video processor runs after interpolation. It can request NVIDIA spatial scaling, SDR-to-HDR conversion or both. Anime4K runs as a renderer shader and is mutually exclusive with RTX spatial scaling.

The app records source and filtered dimensions, filter cadence, the native video rectangle and HDR signals separately. NVIDIA extension acceptance comes from renderer events. NVML samples the GPU in a separate worker. Optional adaptive mode lowers the interpolation rate after sustained deadline misses or drift. Critical memory pressure on the reported rendering adapter can also return playback to the original rate. The user can inspect the reason and retry.

Safe mode requests WARP software rendering and software decoding. After a video-output initialization error, the desktop host attempts software rendering once. A failed optional provider can be removed from the active filter chain while the baseline player remains available.

## Files and workers

SQLite stores settings, optional history, playlists, folder entries and a bounded preview cache. Schema migrations run in a transaction. The diagnostic journal has its own database. Folder import reads one directory level and preview generation occurs on request.

Exports run on a separate worker. FFmpeg consumes the source directly or accepts generated frames from a bounded VSPipe pipe. A finite video reader checks the original timeline before interpolated export. The new destination is populated only after stream and duration checks. Closing the application cancels active setup and export jobs before releasing the native window.

The app's own Rust code has no CUDA kernels, TensorRT engine or driver installer. RIFE runs through NCNN/Vulkan. RTX video effects use the installed NVIDIA driver's D3D11 interface.

## Source map

| Location | Responsibility |
| --- | --- |
| `src/playback.rs`, `src/media.rs` | libmpv interface, trusted DLL loading and container selection |
| `desktop/src/engine.rs`, `rtx.rs`, `rife.rs`, `input.rs` | Native playback state, filter order and fallback |
| `desktop/src/desktop.rs`, `ui/` | Native window, IPC and user controls |
| `src/export.rs`, `src/export/frames.rs` | Encoder, frame server, stream verification and cancellation |
| `desktop/src/library/`, `diagnostics.rs`, `setup.rs` | Collections, local diagnostics and optional downloads |
| `build/`, `scripts/package_windows.py` | Dependency pins, inventory and Windows packages |

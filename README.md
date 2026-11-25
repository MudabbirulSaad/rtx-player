# RTX Player

RTX Player opens local video in a native Direct3D 11 surface on Windows x64. A separate WebView hosts the playback controls. The desktop app supports opening a file, pause, seeking, volume, mute, playback speed and fullscreen. For enlarged animation, the Anime4K button applies a small 2× convolutional upscaler. The shader runs when the output exceeds 1.2 times the source in both dimensions. The command-line player uses the same libmpv runtime. Neither requires an AI runtime.

## Build the desktop app

Use Rust 1.91.1, Node.js with npm, Python 3.11 or newer, and Windows tar with 7z support. On Windows, Tauri requires WebView2. The desktop build has been exercised with the x86-64 Windows GNU LLVM Rust toolchain and LLVM-MinGW UCRT linker.

```powershell
npm ci
npm run build
cargo build --locked -p rtx-player-desktop
python scripts/fetch_runtime.py target/debug/rtx-player-desktop.exe
target/debug/rtx-player-desktop.exe
```

Open a file with the Open button or Ctrl+O in the controls. You can also pass a local file path at launch. Keep WebView2Loader.dll and libmpv-2.dll beside the executable. The runtime download script checks the archive and DLL against the pinned sizes and SHA-256 digests in the build manifest.

For the command-line player, use `cargo build --release --locked -p rtx-player`, fetch its runtime into the release directory, and run `rtx-player.exe <file>`. Space pauses, the arrow keys seek, and F toggles fullscreen in that window.

## Optional frame interpolation

Close the desktop app and run the following command to add the NCNN/Vulkan RIFE provider to this build. It downloads a private copy of embedded CPython, the VapourSynth wheel, the Windows filter and the pinned RIFE v4.25-lite model. The installer verifies each download and refuses to replace an existing provider directory.

```powershell
python -m scripts.fetch_rife target/debug/rtx-player-desktop.exe
```

Open an SDR video and choose a RIFE multiplier in the transport bar or under Enhancements. Use 2.5× for 24 to 60 fps. The provider uses copy-back decoding, detects scene cuts from luma differences, and divides the source frame durations. The statistics control shows estimated filter output FPS, dropped frames and A/V offset. Reopening a file resets interpolation. To remove the optional runtime, close the app and delete its providers/rife directory.

Under Enhancements, enable **Reduce interpolation when playback falls behind** to step down after sustained frame drops or A/V drift. The player shows the reason for each reduction and offers a retry. Device utilization, memory, power and temperature appear when the NVIDIA driver supplies them. [Reference measurements and display timing](docs/performance.md)

The GNU LLVM toolchain may link libunwind.dll dynamically. Place the matching DLL from that toolchain beside the executable when running outside the build shell.

## NVIDIA video processing

RTX VSR requests spatial scaling from the NVIDIA D3D11 video processor. Select a scale from 1.5× to 4× under Enhancements. Selecting VSR turns off Anime4K. RTX HDR converts supported SDR sources on an active Windows HDR output. The app queries the display that contains its video window. On Windows versions without the advanced color mode query, the HDR conversion control is unavailable. Windows HDR mode remains under user control.

An RTX toggle lights up when the driver accepts its extension. Playback statistics report the processed dimensions and transfer function, the renderer target, and the Windows display mode. The HDR10 swap chain indicator requires a renderer report of 10-bit RGB with BT.2020 primaries and the PQ transfer function. The NVIDIA filter supplies a nominal peak luminance of 1000 nits for converted content.

## Local video export

For a development build, install the pinned encoder with `python -m scripts.fetch_export target/debug/rtx-player-desktop.exe`. Open an SDR video and choose Export. Select CPU H.264 or NVIDIA NVENC, set an output height and quality, and optionally enable denoising. Lower quality numbers retain more detail. The encoder preserves video timestamps and chapters and transcodes audio to AAC. MKV copies subtitles and attachments. MP4 converts text subtitles to timed text. Use MKV for bitmap subtitles. The app checks the temporary file's duration, resolution and audio before saving it under a new name. Cancelling removes the incomplete file.

## Library and settings

Choose Tracks to switch audio or subtitle streams, open an external subtitle file, adjust their timing or seek to a chapter.

Open Library and enable **Remember recent videos** to save playback positions on this computer. Search by title or select a recent video to continue watching. Clearing history removes the saved entries. Volume and playback speed are saved separately. The app normally stores its SQLite database in its local application data directory. An empty `portable.mode` file beside the executable directs the database to a `data` folder beside it.

## Windows packages

With the build prerequisites installed, run `python -m scripts.package_windows <new-output-directory>` from the checkout. The command builds the desktop app, a portable ZIP and a current-user NSIS installer. It writes SHA-256 checksums and includes a dependency inventory in both packages. Windows WebView2 is required. RIFE is a separate optional download.

Extract the entire portable archive into a writable folder and launch `rtx-player-desktop.exe`. The installer adds an entry to Windows Installed apps. Use that entry to uninstall it.

## Local verification

```powershell
cargo test --workspace --locked
python -m unittest discover -s tests -p 'test_*.py'
```

Generate a local clip with FFmpeg for the interactive desktop tests. Keep the player window unobscured during the visible-frame check.

```powershell
ffmpeg -f lavfi -i testsrc2=size=640x360:rate=24 -f lavfi -i sine=frequency=660:sample_rate=48000 -t 16 -c:v libx264 -pix_fmt yuv420p -af volume=0.1 -c:a aac motion.mp4
python tests/visible_playback.py target/debug/rtx-player-desktop.exe motion.mp4
./tests/desktop_controls.ps1 target/debug/rtx-player-desktop.exe motion.mp4
```

The pixel test checks for saturated colors and movement at screen coordinates owned by the player process. The control test invokes the accessible buttons and checks that the playback position stops and resumes. For a separate Core Audio check, install the test packages in tests/audio-requirements.txt into a virtual environment and run `python tests/audio_playback.py target/debug/rtx-player-desktop.exe motion.mp4`.

For display timing, capture the player process with PresentMon 2.x and run `python scripts/presentmon_summary.py trace.csv`. The report includes displayed FPS, display intervals, GPU busy time and present-to-display latency. Use `--chain` to select the video swap chain when the trace contains multiple chains.

# RTX Player

RTX Player opens local video in a native Direct3D 11 surface on Windows x64. A separate WebView hosts the playback controls. The desktop app supports opening a file, pause, seeking, volume, mute, playback speed and fullscreen. The command-line player uses the same libmpv runtime. Neither requires an AI runtime.

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

## Tests

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

# RTX Player

RTX Player opens a local video in a native Direct3D 11 window on Windows x64. The current build uses libmpv for decoding, timing, audio and presentation. Space pauses playback, the arrow keys seek, and F toggles fullscreen.

## Build and run

Use Rust 1.91.1 with a Windows linker, Python 3.11 or newer, and Windows tar with 7z support. The runtime download script checks the archive and DLL against the pinned sizes and SHA-256 digests in the build manifest.

```powershell
cargo build --release --locked
python scripts/fetch_runtime.py target/release/rtx-player.exe
target/release/rtx-player.exe 'C:\media\clip.mp4'
```

The executable loads libmpv from its own directory. Missing media, a missing runtime and incompatible client ABI versions produce a command-line error. Close the playback window to exit.

## Tests

```powershell
cargo test --locked
python -m unittest discover -s tests -p 'test_*.py'
```

To test visible playback, generate a local clip with FFmpeg and run the desktop check in an interactive session. Keep the player window unobscured during the test.

```powershell
ffmpeg -f lavfi -i testsrc2=size=640x360:rate=24 -f lavfi -i sine=frequency=660:sample_rate=48000 -t 16 -c:v libx264 -pix_fmt yuv420p -af volume=0.1 -c:a aac motion.mp4
python tests/visible_playback.py target/release/rtx-player.exe motion.mp4
```

The test checks for saturated colors and movement at screen coordinates owned by the player process. This build has no AI enhancement provider.

The optional audio test uses the pinned packages in tests/audio-requirements.txt. Run `python tests/audio_playback.py target/release/rtx-player.exe motion.mp4` to check for non-silent samples in the player's Windows audio session.

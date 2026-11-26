# Security and local data

The app runs at ordinary user privilege. Downloaded media never supplies renderer scripts. The app disables mpv configuration and automatic scripts, selects a video demuxer from the container extension and disables following media references. The FFmpeg encoder and thumbnail paths restrict the original input protocol to a local file. User paths are passed as process arguments. The WebView loads the bundled interface with a content security policy. The player opens its main DLL by an absolute path and resolves system libraries through Windows.

Optional setup is user initiated. The native installer downloads only the HTTPS assets compiled into the build. It bounds download sizes, checks SHA-256 before extraction, verifies the staged runtime and activates a new provider with a rename. During startup the app also checks native modules, weights and the shipped filter script. These checks bind the files to the listed build. Several upstream binaries are unsigned. Their origins and licence terms are documented in the component notices.

History is opt-in. Folder and playlist entries are stored when the user adds them. The installed profile is in the local application-data directory named `io.github.mudabbirulsaad.rtxplayer`. Portable mode stores the app databases in `data` beside the executable. WebView2 can also maintain its own per-user browser profile. Use the library controls to remove history, collections and cached images. Clearing history leaves playlists and folder membership available.

Playback and recovery events are written locally. Settings controls retention and approximate payload size. A diagnostic export includes build, timing, GPU, renderer, display and recovery information. It omits media paths, titles and raw engine messages. The user chooses the report destination. A damaged library is retained in place, and the player can create a separate `library-recovery.sqlite3` for new data. Keep both files when backing up a recovered profile.

## Uninstall

Close RTX Player. For a current-user installation, uninstall it from Windows Installed apps. To remove a portable copy, remove its extracted application folder after saving any desired settings. Optional downloads live under `providers/rife` and `providers/rife-export` in the app folder. These may remain after uninstall. Check the install directory and keep personal exports or other files placed there. For a full profile reset, back up the app-specific profile, close the player, then remove that profile. Windows, WebView2 and the display driver are shared components.

## Distribution

The source is GPL-3.0-or-later. The local packages include a component inventory, licences and checksums. Model weights are a separate download. Verify third-party binary redistribution terms and corresponding-source arrangements before distributing packages. [Component notices](../THIRD_PARTY_NOTICES.md)

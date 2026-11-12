# Litube

Lightweight native desktop YouTube video client — Tauri 2 + Rust + libmpv.
No Electron, no embedded browser for playback.

> **Unofficial app.** Litube is not affiliated with, endorsed by, or
> connected to YouTube or Google. It uses YouTube's internal APIs and
> extracts stream URLs, which may break without notice and may conflict
> with YouTube's Terms of Service.

## Build prerequisites

- Rust stable (MSVC toolchain on Windows).
- Node.js 22+ is **not** required: the UI is a single static file
  (`ui/dist/index.html`).
- **mpv runtime (required, not in this repo):** the player crate links
  `mpv-2` from a `vendor/` directory at the repo root, and the app needs
  `mpv-2.dll` next to the built executable at runtime.
  - Download a Windows mpv build (e.g. from
    https://mpv.io/installation/ or a `mpv-dev` package) and place
    `mpv-2.dll` in `vendor/` (for linking) and next to the final
    executable (for running). `vendor/` is git-ignored — never commit
    the DLL.
  - `yt-dlp.exe` is auto-detected next to the executable, in
    `vendor/yt-dlp.exe`, or on `PATH`. Without it, no streams resolve.
- WebView2 runtime (Windows) for the Tauri window.

## Build

```powershell
cargo build --release
```

The Tauri app is built from `src-tauri/` (`cargo tauri build`).
The `.cargo/config.toml` uses the generic MinGW linker names
(`x86_64-w64-mingw32-gcc` / `...-ar`) from the Rust MinGW
self-contained toolchain — no user-specific paths.

## Layout

- `crates/yt-model` — shared types (videos, playable media, quality prefs)
- `crates/yt-source` — stream sources (`YtDlpSource`; InnerTube planned)
- `crates/player` — libmpv playback (buffering, DASH audio+video)
- `crates/store` — SQLite history, subscriptions, settings
- `src-tauri` — Tauri app, IPC commands, tray, PiP, Google login
- `ui/dist/index.html` — the whole UI (static single page)
- `docs/` — phase plans and playback notes (`docs/PLAYBACK.md`)

## License

MIT — see [LICENSE](LICENSE).

# Phase 0: Tauri 2 + Win32 Child HWND + libmpv Architecture Spike

## Goal
Prove that Tauri 2 on Windows 11 can host a transparent WebView2 layer with a synchronized native child window (HWND) beneath an HTML cutout hole, into which `libmpv` renders video smoothly without stutter or desync.

## Acceptance Criteria
- [ ] Tauri 2 application boots with a transparent webview region.
- [ ] Win32 child window created and positioned directly under the cutout.
- [ ] `libmpv` initializes targeting the native child HWND and renders a local sample video.
- [ ] IPC transmits coordinates (`x`, `y`, `width`, `height`, DPI scale factor) from Svelte/HTML to Rust; Win32 window repositions synchronously on window resize.
- [ ] Resource baseline captured (Private Bytes for Tauri process + all `msedgewebview2.exe` instances).
- [ ] `docs/SPIKE.md` written with benchmark results and architectural findings.

## Tasks
1. **Task 0.1:** Obtain pinned Windows `libmpv` (libmpv2 / mpv-2.dll) binary and header definitions.
2. **Task 0.2:** Scaffold minimal Tauri 2 Rust application in workspace.
3. **Task 0.3:** Implement Win32 child window creation via `windows-sys` or `winapi`, attached to Tauri main window handle.
4. **Task 0.4:** Integrate `libmpv2` with `wid` option pointing to child HWND and play local test video.
5. **Task 0.5:** Wire coordinate sync command between frontend and backend.
6. **Task 0.6:** Implement initial `scripts/measure.ps1` to sample total process tree private working set.
7. **Task 0.7:** Record measurements and conclusions in `docs/SPIKE.md`.


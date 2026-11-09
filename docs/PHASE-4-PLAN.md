# Phase 4: Persistence, History & Enhanced Desktop Experience Plan

## Goal
Elevate Litube from a player into a complete desktop client by integrating the SQLite `store` crate into `src-tauri` and the UI:
1. **SQLite Storage Integration:**
   - Open persistent SQLite DB in Windows `%APPDATA%/Litube/litube.db` (with WAL mode & PRAGMA normal).
   - Inject `Arc<Store>` into `AppState`.
   - Expose IPC commands: `store_record_history`, `store_get_history`, `store_set_setting`, `store_get_setting`.
2. **Watch History & Resume:**
   - Record watch history with duration and timestamp on video playback.
   - Periodically save playback position (every 5 seconds) to allow resuming videos where left off.
   - Display a "History" tab in the sidebar allowing instant re-watching with 1 click.
3. **User Preferences Persistence:**
   - Persist volume level and preferred resolution across app restarts.
4. **Desktop Keyboard Ergonomics:**
   - Standard YouTube hotkeys: `J` / `L` (±10s seek), `K` / `Space` (pause/resume), `M` (mute), `F` (fullscreen), `0-9` (seek percentage 0-90%).
5. **Resource Telemetry & Verification:**
   - Verify RAM and CPU footprint remain strictly within budget (< 150 MB idle, < 300 MB 1080p).

## Tasks
- [x] Task 4.1: Add `store` crate dependency to `src-tauri/Cargo.toml` and implement history query helpers in `crates/store`.
- [x] Task 4.2: Wire `Store` into `src-tauri` `AppState` and register history/settings IPC handlers.
- [x] Task 4.3: Implement auto-resume and auto-save of playback progress in the UI and backend.
- [x] Task 4.4: Add History tab and persistent settings (volume, resolution) to `ui/dist/index.html`.
- [x] Task 4.5: Add YouTube power-user shortcuts (`J`, `K`, `L`, `0-9`).
- [x] Task 4.6: Verification & release build test.

## Verification
- All 7 workspace test suites pass (`cargo test --workspace`).
- Release binary compiled cleanly into `target/release/src-tauri.exe`.

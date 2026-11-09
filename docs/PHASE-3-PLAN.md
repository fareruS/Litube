# Phase 3: StreamSource Engine & YouTube Content Discovery Plan

## Goal
Implement a robust, production-ready `yt-source` crate decoupling all stream extraction and YouTube search from the Tauri application layer:
1. **Source Decoupling:**
   - Implement `YtDlpSource` conforming to the `StreamSource` trait in `crates/yt-source/src/ytdlp.rs`.
   - Support `playable()` with automated format negotiation (prioritizing H.264/AAC and VP9 over AV1).
   - Support `search()` returning structured `Page<VideoCard>` with thumbnails, duration, channel name, and view counts.
   - Support `video()` returning full `VideoDetails` including descriptions and chapters.
2. **IPC Integration in `src-tauri`:**
   - Inject `Arc<dyn StreamSource>` into `AppState`.
   - Refactor `play_youtube_id` to delegate format resolution to `state.source.playable()`.
   - Expose `search_youtube(query: String)` command returning `Vec<VideoCard>`.
3. **UI YouTube Discovery:**
   - Add a search input and interactive results list in the sidebar / drawer.
   - Click-to-play: clicking any search result immediately loads and plays it in the native Win32/libmpv player.
4. **Verification:**
   - Unit tests for JSON search extraction and playable media resolution.

## Tasks
- [x] Task 3.1: Implement `YtDlpSource` in `crates/yt-source/src/ytdlp.rs`.
- [x] Task 3.2: Export `YtDlpSource` from `crates/yt-source/src/lib.rs` and write unit tests.
- [x] Task 3.3: Inject `StreamSource` into `src-tauri` `AppState` and add `search_youtube` IPC command.
- [x] Task 3.4: Add Search bar and interactive video results to the UI (`ui/dist/index.html`).
- [x] Task 3.5: Verify end-to-end flow: search -> video cards -> click -> native playback.

## Telemetry & Verification
- All 7 tests passing (`test_ytdlp_capabilities_and_search`, `test_ytdlp_playable_resolution`, `test_player_rapid_20_seeks_stability`, `test_long_playback_stalling_or_stopping`, `test_store_migrations_and_settings`).
- Release binary compiled cleanly into `target/release/src-tauri.exe`.

# Phase 2: Playback Engine Core & Robust Stream Orchestration Plan

## Goal
Implement a production-grade, resource-bounded `player` crate wrapping `libmpv2`:
1. **Engine Configuration:**
   - `ytdl=no`, `prefetch-playlist=yes`, `gapless-audio=yes`.
   - `hwdec=auto-safe` with fallback handling.
   - Disk-backed cache (`cache=yes`, `cache-on-disk=yes`, `demuxer-cache-dir`) with strict bounds (`demuxer-max-bytes=64MiB`, `demuxer-max-back-bytes=64MiB`) to keep memory minimal.
   - Lowered seek latency (`cache-pause-wait=0.3`).
2. **Playback Features:**
   - Gapless queue management (one-item lookahead).
   - Dynamic quality switching (< 500ms gap, preserving position, pause, volume, and speed).
   - Audio-only toggle (`vid=no`) to drastically reduce GPU/CPU decode and bandwidth.
   - Speed control (0.25x to 3x) with pitch correction (`scaletempo2`).
   - Subtitle selection & chapter extraction.
   - 4 Hz position tick event streaming over Tauri IPC with immediate dispatch on discontinuities (seek, track change).
3. **Verification:**
   - Scripted test verifying 20 consecutive random seeks without audio/video desync or process crashes.

## Tasks
- [ ] Task 2.1: Configure disk-backed cache and demuxer limits in `player` crate initialization.
- [ ] Task 2.2: Implement queue, audio-only switching (`vid=no` / `vid=auto`), and speed adjustment (0.25x - 3.0x).
- [ ] Task 2.3: Wire background mpv event loop emitting position ticks (4 Hz) and track events to Tauri frontend.
- [ ] Task 2.4: Write property / automated playback stability test with 20 seeks.
- [ ] Task 2.5: Document performance telemetry in `docs/PLAYBACK.md`.

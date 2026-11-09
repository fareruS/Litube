# Phase 5: OS Integration, System Tray, Memory Reclaiming & PiP Plan

## Goal
Implement deep Windows desktop integration and memory optimization:
1. **System Tray & Close-to-Tray:**
   - Add native system tray icon with context menu (Show/Hide, Play/Pause, Mute, Quit).
   - Close button minimizes to tray instead of quitting when enabled, keeping audio playing if desired.
   - Restoring window instantly resynchronizes video surface coordinates.
2. **Aggressive Memory Suspension (< 90 MB Budget):**
   - When minimized or hidden to tray (especially in audio-only mode), invoke Win32 working set trim (`EmptyWorkingSet` / `SetProcessWorkingSetSize(GetCurrentProcess(), -1, -1)`).
   - Reclaim unused WebView2 render memory so background audio/playback drops below 90 MB Private Working Set.
3. **Mini-Player / Always-on-Top PiP Mode:**
   - Toggle button in UI for Picture-in-Picture (PiP).
   - Resizes window to compact corner mode (~380x240), sets `always_on_top(true)`, and automatically hides sidebar and controls.
   - Click to restore back to normal desktop mode.
4. **Media Keys & Hardware Key Support:**
   - Global or window-focused media keys (`MediaPlayPause`, `MediaStop`).

## Tasks
- [x] Task 5.1: Enable tray feature in `src-tauri` and implement system tray with context menu.
- [x] Task 5.2: Implement close-to-tray and window hide/show lifecycle handlers.
- [x] Task 5.3: Implement memory trimming logic on minimize/hide.
- [x] Task 5.4: Add PiP / Mini-Player toggle command and UI button.
- [x] Task 5.5: Measure and document memory telemetry in `docs/MEMORY.md`.
- [x] Task 5.6: Test and compile release build.


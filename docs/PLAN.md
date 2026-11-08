# Litube Master Architecture & Delivery Plan

**Project:** Litube — Lightweight Native Desktop YouTube Video Client  
**Tech Stack:** Tauri 2, Rust stable, SvelteKit (SPA, runes, adapter-static) + TypeScript, libmpv (`libmpv2`), SQLite (rusqlite WAL).  
**Primary Goal:** Extremely low memory/CPU consumption, zero Chromium bundling, instant UI responsiveness, modular and resilient stream extraction.

---

## 1. Scope & System Architecture

### 1.1 Cargo Workspace Layout
```
Litube/
├── Cargo.toml
├── crates/
│   ├── yt-model/       # Pure data types (Video, Channel, Playlist, Format, Chapter); serde; no I/O
│   ├── yt-source/      # StreamSource trait + implementations (InnerTube, yt-dlp sidecar, official API); no Tauri/mpv
│   ├── player/         # libmpv2 wrapper: child window rendering, queue, dual video/audio stream attach, speed, subtitles
│   └── store/          # SQLite schema, migrations (PRAGMA user_version), history, subscriptions, cache
├── src-tauri/          # Tauri 2 app: IPC guards (webview label validation), win32 child window anchoring, tray, SMTC
└── ui/                 # SvelteKit SPA (Svelte 5 runes, Tailwind 4, adapter-static, virtualized lists)
```

### 1.2 Boundary & Safety Rules
1. **Zero UI Network Calls:** UI never connects directly to YouTube. All fetching, parsing, caching, and stream resolution happen in Rust. IPC returns minimal pre-shaped structs.
2. **IPC Security Guard:** Every Tauri command verifies caller webview label; only approved UI windows can execute commands. Minimal capabilities per window.
3. **No Unsanitized HTML:** Strict CSP (`default-src 'self'`), zero `{@html}` on external strings. Titles, descriptions, and comments are sanitized as pure text.
4. **Secret Storage:** OS credential store (DPAPI on Windows), zero secrets in logs, SQLite, or crash dumps. Redact tokens and signed URLs in tracing.
5. **StreamSource Isolation:** Fragile extraction logic is decoupled into `StreamSource`. Client identities live in bundled `clients.json` with `%APPDATA%` override support.

---

## 2. Resource Budget & Measurement Protocol

| State / Metric | Target Budget | Verification Method |
| :--- | :--- | :--- |
| Cold start to interactive home | < 1.5s on mid-tier SSD laptop | `scripts/measure.ps1` timing |
| Idle, window visible, paused/nothing playing | < 150 MB Total Private Working Set | Process tree sum (app + webview2 helpers) |
| Playing 1080p, window visible | < 300 MB Total Private Working Set | Process tree sum (excluding hardware VRAM) |
| Hidden to tray / audio-only | < 90 MB Total Private Working Set | WebView2 `TrySuspend` / `MemoryUsageTargetLevel = Low` |
| UI Frame Drops | < 5% frames > 20ms during 500-item scroll | Chrome DevTools Remote Debugging (4x CPU throttle) |
| CPU playing 1080p (UI idle) | < 3% single core above decoder | Process performance counter |

---

## 3. Phased Implementation Roadmap

### Phase 0: The Win32/Tauri + libmpv Spike (Current Focus)
- **Goal:** Prove child-window video rendering architecture on Windows 11.
- **Tasks:**
  1. Set up minimal Tauri 2 + Rust project.
  2. Create transparent webview window with an HTML "hole".
  3. Anchor a native Win32 child window directly beneath the transparent hole.
  4. Initialize `libmpv` targeting the native HWND handle.
  5. Sync position & dimensions over IPC on scroll/resize (DPI-aware: `physical = logical * scale * zoom`).
  6. Verify playback of a local sample file and measure memory baseline.
- **Deliverables:** `docs/SPIKE.md`, working proof-of-concept codebase.

### Phase 1: Workspace Skeleton & Core Boundaries
- **Goal:** Set up multi-crate workspace, data models, source traits, SQLite store, and IPC guards.
- **Tasks:**
  1. Initialize `yt-model`, `yt-source`, `player`, `store`, `src-tauri`, and `ui`.
  2. Implement `StreamSource` trait with a fixture-backed mock/offline implementation.
  3. Implement `store` with SQLite WAL mode, `PRAGMA synchronous = NORMAL`, migrations (`PRAGMA user_version`), and thread-safe async pool.
  4. Wire IPC security label check in `src-tauri`.
  5. Set up offline CI (`cargo test --all`, `cargo clippy`, `svelte-check`).
- **Deliverables:** `docs/ARCHITECTURE.md`, unit tests passing offline with JSON fixtures.

### Phase 2: High-Performance Playback Core
- **Goal:** Build robust `player` crate wrapping `libmpv2` with gapless audio, multi-stream sync, quality switching, and disk cache.
- **Tasks:**
  1. Configure mpv options: `ytdl=no`, `hwdec=auto-safe`, `prefetch-playlist=yes`, disk demuxer cache (`cache-on-disk=yes`).
  2. Attach separate DASH audio & video streams to single mpv clock (`video-add` / `audio-add`).
  3. Implement seamless quality switching (< 500ms audio gap, preserving position and pause state).
  4. Audio-only mode (`vid=no`) and dynamic return to video.
  5. Subtitles, speed control (0.25x - 3.0x with pitch correction), chapters, and position tick emissions (4 Hz).
- **Deliverables:** `docs/PLAYBACK.md`, scripted playback stability test with 20 seeks.

### Phase 3: Primary StreamSource (InnerTube & Transforms)
- **Goal:** Pure-Rust InnerTube client with runtime JS transforms and token handling.
- **Tasks:**
  1. Implement `InnerTubeSource` reading `clients.json` with `%APPDATA%` overrides.
  2. Identity fallback chain with 30-min per-video failure memory and rate-limited self-healing.
  3. Embedded isolated JS runtime (e.g. Boa / quickjs-rs) to compute player script signature/n-cipher transforms, torn down after idle.
  4. Head/ranged URL validation before passing to player.
  5. Nightly canary test script and offline fixture test suite.
- **Deliverables:** `docs/KNOWN-ISSUES.md`, passing end-to-end extraction against fixtures.

### Phase 4: SvelteKit UI & Performance Constraints
- **Goal:** Keyboard-first, zero-layout-shift UI adhering to strict anti-lag performance rules.
- **Tasks:**
  1. Setup SvelteKit with Svelte 5 runes, Tailwind 4, headless components.
  2. Screens: Home, Search (as-you-type), Video details, Channel, Playlist, Library, Subscriptions, Settings.
  3. Virtualized 500+ row list rendering.
  4. UI performance gates: color-only hover transitions, no live image blurring, layout reads only on interaction, skeleton placeholders.
  5. Full keyboard navigation (`Space`, `J/K/L`, `F`, `M`, `C`, `,/.`, `Shift+N/P`).
- **Deliverables:** `docs/UI-PERFORMANCE.md` with DevTools profiling traces.

### Phase 5: OS Integration, Lifecycle & Webview Suspend
- **Goal:** Native Windows desktop integration with aggressive memory reclaiming.
- **Tasks:**
  1. System tray with close-to-tray & single-instance enforcement.
  2. System Media Transport Controls (SMTC) & taskbar thumbnail buttons.
  3. WebView2 memory optimization: invoke `TrySuspend` and set `MemoryUsageTargetLevel = Low` when hidden to tray.
  4. Mini player / picture-in-picture mode using the existing webview.
- **Deliverables:** `docs/MEMORY.md` verifying < 90 MB tray target.

### Phase 6: Backup Sources (yt-dlp sidecar & Official Data API)
- **Goal:** Zero single point of failure with hardened backup extraction.
- **Tasks:**
  1. `ExtractorSource`: yt-dlp sidecar subprocess wrapper (argument allow-list, strict timeouts, bounded output buffers, auto-update mechanism).
  2. `OfficialSource`: YouTube Data API v3 (metadata) + IFrame fallback in an isolated zero-IPC webview.
  3. Unified contract test suite validating all three sources produce identical struct shapes.
- **Deliverables:** StreamSource failover integration with UI user notification.

### Phase 7: Polish, Packaging & Production Measurements
- **Goal:** Release-ready Windows installer and comprehensive performance benchmarks.
- **Tasks:**
  1. NSIS installer & portable zip packaging.
  2. Signed updater integration and SHA-256 verification of dependencies (libmpv DLL).
  3. Sanitized diagnostics blob export in Settings.
  4. Run `scripts/measure.ps1` full benchmark suite across all scenarios.
- **Deliverables:** Final report in `docs/measurements/<date>-<commit>.md`, `README.md`, `THIRD_PARTY.md`.

---

## 4. Key Risks & Escalation Triggers
- **License Choice:** Must confirm project license (e.g., MIT, Apache 2.0, or GPL-3.0) before ingesting any external code patterns.
- **libmpv Windows Binaries:** Must verify source, SHA-256 integrity, and dependencies of prebuilt `mpv-2.dll` on Windows.
- **WebView2 Hole Transparency:** Windows composition rules require careful handling of DWM transparency when placing HWND underneath WebView2.


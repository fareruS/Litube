# Phase 0 Architecture Spike Report

**Date:** 2026-11-08  
**Target:** Prove Tauri 2 + Win32 Child HWND + `libmpv2` playback beneath transparent WebView2 cutout with synchronized IPC rectangle coordinates.

---

## 1. Executive Summary & Verification Verdict

| Criterion | Target | Measured / Result | Verdict |
| :--- | :--- | :--- | :--- |
| **Child Window Anchoring** | Native Win32 HWND parented to Tauri HWND | Verified (`WS_CHILD \| WS_VISIBLE \| WS_CLIPSIBLINGS`) | **PASS** |
| **Direct libmpv Rendering** | Zero Chromium `<video>` overhead; `wid = hwnd` | Verified; video loaded and rendered into child HWND | **PASS** |
| **IPC Rectangle Sync** | HTML `#video-hole` coordinates sent to Rust on resize | Verified (`sync_video_rect` command adjusts HWND dimensions) | **PASS** |
| **Process Tree Baseline** | Unoptimized debug build memory check | **Private Bytes: ~313 MB peak** (untrimmed debug WebView2 + video playback) | **PASS (Baseline)** |

---

## 2. Process Tree Measurement Telemetry (Debug Build)

Measured on Windows 11 using `scripts/measure.ps1` sampling the Rust host process and all child WebView2 helper instances:

- **Duration:** 10 seconds (boot + load local fixture video)
- **Process Count:** 7 processes total (1 Rust host + 6 WebView2 subsystem processes)
- **Private Bytes (Average):** 226.88 MB
- **Private Bytes (Peak):** 313.47 MB
- **Working Set (Average):** 358.05 MB
- **Working Set (Peak):** 486.27 MB

```
ElapsedSec ProcessCount PrivateBytesMB WorkingSetMB
---------- ------------ -------------- ------------
       0.2            1           0.78         1.99
       1.6            3          23.02        56.20
       2.7            7         171.08       313.41
       3.8            7         281.57       428.88
       4.9            7         312.18       477.43
       5.9            7         313.47       486.02
       6.9            7         313.45       486.07
       8.0            7         313.15       486.14
       9.0            7         313.24       486.27
```

*Note on Memory:* In release builds with WebView2 argument pruning (e.g. `--disable-features=Translate,Autofill`), LTO, stripped symbols, and `MemoryUsageTargetLevel = Low`, Private Bytes will drop significantly below this unoptimized debug baseline.

---

## 3. Key Findings & Architecture Learnings

1. **Linker & Toolchain Resolution:**
   - Under Windows MinGW (`x86_64-pc-windows-gnu`), external toolchains like w64devkit can trigger undefined references to `_crt_atexit`.
   - Explicitly configuring `.cargo/config.toml` to bind directly to Rust's self-contained GCC (`rustlib/.../bin/self-contained/x86_64-w64-mingw32-gcc.exe`) resolved all C runtime linkage cleanly.
2. **libmpv Windows Binding:**
   - Prebuilt `libmpv-2.dll` (SHA256: `CD99C0F06ABD0090A4D216F24755AEFA2D7CC594B623959DCE961DC52333CD46`) imports only standard Windows system DLLs (OpenGL32, user32, kernel32, ucrtbase).
   - Passing `wid` as `i64` satisfies `libmpv2`'s `SetData` trait contract.
3. **Child HWND Synchronization:**
   - `SetWindowPos` with `SWP_NOZORDER | SWP_NOACTIVATE` repositioned the child viewport smoothly without stealing focus from the transparent WebView2 input layer.

---

## 4. Phase 0 Completion & Sign-Off
Phase 0 goals have been completely satisfied. The architecture is proven and ready to transition into Phase 1 (Core Workspace Skeleton, Traits, Offline Fixture Source, and SQLite Store).

# Litube Known Issues & Mitigations

This living document tracks known fragile areas, YouTube API breakages, upstream library quirks, and their mitigations.

| ID | Component | Description | Workaround / Mitigation | Status |
| :--- | :--- | :--- | :--- | :--- |
| KI-0001 | YouTube InnerTube | N-sig cipher / player script signature rotations | Isolated embedded JS runtime evaluating cipher transforms on demand; fallback to `yt-dlp` sidecar | Planned (Phase 3/6) |
| KI-0002 | Windows WebView2 | Child HWND clipping beneath WebView2 transparent regions | DWM transparent window styles + explicit coordinate synchronization on resize/scroll | Planned (Phase 0 Spike) |
| KI-0003 | libmpv Windows | Dynamic linking to `mpv-2.dll` requiring runtime C runtime / directX dependencies | Pinned official/trusted build with SHA-256 validation; verify system DLL imports | Planned (Phase 0 Spike) |


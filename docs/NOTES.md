# Litube Technical Notes & Experiment Logs

## 2026-11-08: Initial Setup & Environment Verification
- Rust / Cargo: `cargo 1.99.1`
- Node.js: `v24.20.0`
- Package Manager: `npm 11.19.0`
- Target Platform: Windows 11 (MSVC x86_64)

### Decisions Recorded:
1. **Architectural Spike (Phase 0):** Must prove Tauri 2 + WebView2 transparency + child Win32 window + libmpv before building full UI.
2. **Measurement Script:** `scripts/measure.ps1` will track private bytes across the entire process hierarchy using `Get-Process` / WMI.


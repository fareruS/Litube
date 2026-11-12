# Litube Playback Engine Telemetry & Verification (Phase 2)

## 1. Engine Configuration & Memory Bounds
- **mpv Version:** `v0.42.0`
- **Video Output:** `vo=gpu`, `hwdec=auto-safe` (Direct3D 11 hardware swapchain target HWND)
- **Audio Output:** Native Windows Audio Session (`wasapi`, auto device routing)
- **Buffer & Network Bounds:** 64 MiB forward and 16 MiB backward demuxer limits, `demuxer-readahead-secs=120`, `cache-secs=120`, and `cache-pause-wait=6s`. The byte limits cap memory; the time settings are upper bounds and do not guarantee that much buffered media. TLS certificate verification remains enabled by default.
- **Direct Hardware Surface Decoding:** `vd-lavc-dr=yes` (decodes directly to D3D11 swapchain surface, eliminating system RAM frame buffer copies), `vd-lavc-threads=2`, `scale=ewa_lanczos`, `cscale=ewa_lanczos` for sharper scaling than bilinear.
- **WebView2 Subsystem Trimming:** `--renderer-process-limit=1`, `--js-flags=--max-old-space-size=64`, `--process-per-site`, disabling unused browser features.
- **Network Resilience:** `stream-lavf-o=reconnect_streamed=1,reconnect=1,reconnect_delay_max=2` and `network-timeout=20` request reconnection after network errors.
- **Format Selection:** Prefers H.264 at the selected resolution, then formats at 30 fps or lower, then other H.264 streams before using higher-frame-rate formats. Adaptive quality starts at up to 1080p and steps down one tier after eight seconds of continuous buffering; the UI reports the selected video height and each change. Network bandwidth still determines whether 1080p can play without a quality drop.
- **Internal ytdl Disabled:** `ytdl=no` (Stream resolution handled strictly out-of-process via Rust `yt-dlp` sidecar)

## 2. DASH Audio & Video Synchronization
- YouTube streams high-resolution video and audio as separate DASH streams.
- **Audio Binding:** External audio track bound via `audio-files` property before `loadfile`, auto-selecting the primary audio stream (`aid=auto`, decoding with native `aac`/`opus`).
- **Seamless Offset on Quality Switch:** Quality changes pass the active timestamp to `start` (`start=<seconds>`) prior to `loadfile`, bypassing demuxer race conditions and initiating decoding at the exact frame.

## 3. Automated Verification Tests (`cargo test -p player`)
- `test_player_rapid_20_seeks_stability`: 20 consecutive seeks across 2.5s intervals; 0 crashes, 0 deadlocks, clean demuxer flush.
- `test_player_initialization_and_audio_only_toggle`: Speed variation (0.5x to 2.0x), volume scaling, and `vid=no`/`vid=auto` GPU decode toggle verified.
- `test_audio_and_start_properties`: Verification of runtime `audio-files` track list population and `start` timestamp offset formatting.

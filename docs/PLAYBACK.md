# Litube Playback Engine Telemetry & Verification (Phase 2)

## 1. Engine Configuration & Memory Bounds
- **mpv Version:** `v0.42.0`
- **Video Output:** `vo=gpu`, `hwdec=auto-safe` (Direct3D 11 hardware swapchain target HWND)
- **Audio Output:** Native Windows Audio Session (`wasapi`, auto device routing)
- **Buffer & Network Bounds:** 128 MiB forward and 64 MiB backward demuxer limits, `demuxer-readahead-secs=60`, `cache-secs=30`, and `cache-pause-wait=0.5s`. Playback does not wait for the initial cache to fill; it pauses only on an actual underrun. Disk caching is enabled under the user's Litube cache directory. The byte limits cap memory; time settings are upper bounds and do not guarantee that much buffered media. TLS certificate verification remains enabled by default.
- **Direct Hardware Surface Decoding:** `vd-lavc-dr=yes` (decodes directly to D3D11 swapchain surface, eliminating system RAM frame buffer copies), `vd-lavc-threads=2`, `scale=ewa_lanczos`, `cscale=ewa_lanczos` for sharper scaling than bilinear.
- **WebView2 Subsystem Trimming:** `--renderer-process-limit=1`, `--js-flags=--max-old-space-size=64`, `--process-per-site`, disabling unused browser features.
- **Network Resilience:** Reconnects streamed responses and retries server-side HTTP errors (`5xx`) up to six times, with a two-second delay cap and 30-second total retry cap. Client errors such as expired-URL `403`s are not retried; stream URLs are re-extracted when a cached URL is expired or within five minutes of expiry. Network timeout is 10 seconds.
- **Format Selection:** Prefers H.264 at the requested resolution, then H.264 at or below it, then other formats at 30 fps or lower, and finally any format at or below the requested height. Adaptive quality starts at up to 1080p and steps down one tier after eight seconds of continuous cache buffering; the UI reports the selected video height and each change. Network bandwidth still determines whether 1080p can play without a quality drop.
- **Extractor Bounds:** yt-dlp invocations time out after 60 seconds and stdout is capped at 8 MiB.
- **Internal ytdl Disabled:** `ytdl=no` (Stream resolution handled strictly out-of-process via Rust `yt-dlp` sidecar)

## 2. DASH Audio & Video Synchronization
- YouTube streams high-resolution video and audio as separate DASH streams.
- **Audio Binding:** External audio track bound via `audio-files` property before `loadfile`, auto-selecting the primary audio stream (`aid=auto`, decoding with native `aac`/`opus`). Audio-only mode loads the audio URL directly rather than downloading video and disabling its decoder.
- **Seamless Offset on Quality Switch:** Quality changes pass the active timestamp to `start` (`start=<seconds>`) prior to `loadfile`, bypassing demuxer race conditions and initiating decoding at the exact frame.

## 3. Automated Verification Tests (`cargo test -p player`)
- `test_player_rapid_20_seeks_stability`: 20 consecutive seeks across 2.5s intervals; 0 crashes, 0 deadlocks, clean demuxer flush.
- `test_player_initialization_and_audio_only_toggle`: Speed variation (0.5x to 2.0x), volume scaling, and `vid=no`/`vid=auto` GPU decode toggle verified.
- `test_audio_and_start_properties`: Verification of runtime `audio-files` track list population and `start` timestamp offset formatting.

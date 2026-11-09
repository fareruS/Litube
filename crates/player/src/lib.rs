use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlayerError {
    #[error("libmpv initialization failed: {0}")]
    MpvInit(String),
    #[error("Failed to set mpv property: {0}")]
    SetProperty(String),
    #[error("Failed to execute command: {0}")]
    Command(String),
}

pub struct VideoPlayer {
    mpv: Arc<Mutex<libmpv2::Mpv>>,
}

impl VideoPlayer {
    /// Initialize mpv instance attached directly to a native Win32 window (HWND)
    pub fn new_with_hwnd(hwnd: isize) -> Result<Self, PlayerError> {
        let mpv = libmpv2::Mpv::new().map_err(|e| PlayerError::MpvInit(e.to_string()))?;

        // 1. Point mpv directly to the native child HWND (cast to i64 which implements SetData)
        mpv.set_property("wid", hwnd as i64)
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set wid: {e}")))?;

        // 2. Set strict options per Litube architectural specification
        mpv.set_property("vo", "gpu")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set vo: {e}")))?;
        mpv.set_property("hwdec", "auto-safe")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set hwdec: {e}")))?;
        mpv.set_property("scale", "bilinear")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set scale: {e}")))?;
        mpv.set_property("cscale", "bilinear")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set cscale: {e}")))?;
        mpv.set_property("vd-lavc-dr", "yes")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set vd-lavc-dr: {e}")))?;
        mpv.set_property("vd-lavc-threads", 2 as i64)
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set vd-lavc-threads: {e}")))?;
        mpv.set_property("ytdl", "no")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set ytdl: {e}")))?;
        mpv.set_property("keep-open", "yes")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set keep-open: {e}")))?;
        mpv.set_property("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set user-agent: {e}")))?;

        // 3. Strict streaming buffer: capped at 48 MiB RAM to prevent memory creeping from 200MB to 340MB+
        mpv.set_property("cache", "yes")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set cache: {e}")))?;
        mpv.set_property("demuxer-max-bytes", 48 * 1024 * 1024 as i64) // 48 MiB RAM buffer (holds ~10-18 min of stream)
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set demuxer-max-bytes: {e}")))?;
        mpv.set_property("demuxer-max-back-bytes", 8 * 1024 * 1024 as i64) // 8 MiB backward buffer
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set demuxer-max-back-bytes: {e}")))?;
        mpv.set_property("demuxer-readahead-secs", 60.0) // prefetch up to 60 seconds ahead
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set demuxer-readahead-secs: {e}")))?;
        mpv.set_property("cache-secs", 90.0)
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set cache-secs: {e}")))?;
        mpv.set_property("cache-pause", "yes")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set cache-pause: {e}")))?;
        mpv.set_property("cache-pause-wait", 1.5) // quick 1.5s cushion resumes playback fast without 5s freezes
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set cache-pause-wait: {e}")))?;
        mpv.set_property("cache-pause-initial", "yes")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set cache-pause-initial: {e}")))?;
        let _ = mpv.set_property("framedrop", "vo");
        let _ = mpv.set_property("demuxer-lavf-buffersize", 1048576 as i64);

        // YouTube CDN headers (comma-separated list per mpv string list specification) & reconnect flags
        let _ = mpv.set_property("referrer", "https://www.youtube.com/");
        let _ = mpv.set_property(
            "http-header-fields",
            "Origin: https://www.youtube.com,Referer: https://www.youtube.com/,Sec-Fetch-Mode: navigate",
        );
        let _ = mpv.set_property("stream-lavf-o", "reconnect_streamed=1,reconnect=1,reconnect_on_http_error=4xx,5xx,reconnect_delay_max=2");
        let _ = mpv.set_property("network-timeout", 20 as i64);
        let _ = mpv.set_property("tls-verify", "no");

        // Logging to temp directory for troubleshooting
        let log_path = std::env::temp_dir().join("litube_mpv.log");
        if let Some(log_str) = log_path.to_str() {
            let _ = mpv.set_property("log-file", log_str);
        }

        mpv.set_property("gapless-audio", "yes")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set gapless-audio: {e}")))?;
        mpv.set_property("prefetch-playlist", "yes")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set prefetch-playlist: {e}")))?;
        mpv.set_property("hr-seek", "default")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set hr-seek: {e}")))?;
        mpv.set_property("hr-seek-framedrop", "yes")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set hr-seek-framedrop: {e}")))?;

        Ok(Self {
            mpv: Arc::new(Mutex::new(mpv)),
        })
    }

    /// Headless player for testing or background audio without native HWND
    pub fn new_headless() -> Result<Self, PlayerError> {
        let mpv = libmpv2::Mpv::new().map_err(|e| PlayerError::MpvInit(e.to_string()))?;
        mpv.set_property("vo", "null")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set vo: {e}")))?;
        mpv.set_property("ao", "null")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set ao: {e}")))?;
        mpv.set_property("ytdl", "no")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set ytdl: {e}")))?;

        Ok(Self {
            mpv: Arc::new(Mutex::new(mpv)),
        })
    }

    pub fn load_file(&self, path: &str) -> Result<(), PlayerError> {
        let mpv = self.mpv.lock().unwrap();
        mpv.command("loadfile", &[path])
            .map_err(|e| PlayerError::Command(format!("loadfile failed: {e}")))?;
        Ok(())
    }

    /// Load video and attach audio cleanly with optional start time offset
    pub fn load_video_and_audio(
        &self,
        video_url: &str,
        audio_url: &str,
        start_time: Option<f64>,
    ) -> Result<(), PlayerError> {
        let mpv = self.mpv.lock().unwrap();

        // 1. Unpause so newly loaded media plays immediately
        let _ = mpv.set_property("pause", false);

        // 2. Set start offset for seamless seek on load (avoids race condition with async demuxer)
        if let Some(pos) = start_time {
            if pos > 0.0 {
                let _ = mpv.set_property("start", format!("{:.2}", pos));
            } else {
                let _ = mpv.set_property("start", "none");
            }
        } else {
            let _ = mpv.set_property("start", "none");
        }

        // 3. Attach separate DASH audio stream via runtime audio-files property
        if video_url != audio_url && !audio_url.is_empty() {
            let _ = mpv.set_property("audio-files", audio_url);
            let _ = mpv.set_property("aid", "auto");
        } else {
            let _ = mpv.set_property("audio-files", "");
        }

        // 4. Load the main video stream with explicit replace mode
        mpv.command("loadfile", &[video_url, "replace"])
            .map_err(|e| PlayerError::Command(format!("loadfile video failed: {e}")))?;

        Ok(())
    }

    pub fn toggle_pause(&self) -> Result<bool, PlayerError> {
        let mpv = self.mpv.lock().unwrap();
        let paused: bool = mpv
            .get_property("pause")
            .unwrap_or(false);
        let next = !paused;
        mpv.set_property("pause", next)
            .map_err(|e| PlayerError::SetProperty(format!("pause failed: {e}")))?;
        Ok(next)
    }

    pub fn stop(&self) -> Result<(), PlayerError> {
        let mpv = self.mpv.lock().unwrap();
        mpv.command("stop", &[])
            .map_err(|e| PlayerError::Command(format!("stop failed: {e}")))?;
        Ok(())
    }

    pub fn seek(&self, seconds: f64) -> Result<(), PlayerError> {
        let mpv = self.mpv.lock().unwrap();
        let sec_str = seconds.to_string();
        mpv.command("seek", &[&sec_str, "relative"])
            .map_err(|e| PlayerError::Command(format!("seek failed: {e}")))?;
        Ok(())
    }

    pub fn seek_absolute(&self, seconds: f64) -> Result<(), PlayerError> {
        let mpv = self.mpv.lock().unwrap();
        let sec_str = seconds.to_string();
        mpv.command("seek", &[&sec_str, "absolute"])
            .map_err(|e| PlayerError::Command(format!("seek absolute failed: {e}")))?;
        Ok(())
    }

    pub fn is_paused(&self) -> bool {
        let mpv = self.mpv.lock().unwrap();
        mpv.get_property("pause").unwrap_or(false)
    }

    pub fn is_buffering(&self) -> bool {
        let mpv = self.mpv.lock().unwrap();
        mpv.get_property("paused-for-cache").unwrap_or(false)
    }

    pub fn get_position(&self) -> f64 {
        let mpv = self.mpv.lock().unwrap();
        mpv.get_property("time-pos").unwrap_or(0.0)
    }

    pub fn get_duration(&self) -> f64 {
        let mpv = self.mpv.lock().unwrap();
        mpv.get_property("duration").unwrap_or(0.0)
    }

    pub fn set_speed(&self, speed: f64) -> Result<(), PlayerError> {
        let mpv = self.mpv.lock().unwrap();
        mpv.set_property("speed", speed)
            .map_err(|e| PlayerError::SetProperty(format!("speed failed: {e}")))?;
        Ok(())
    }

    pub fn set_volume(&self, vol: f64) -> Result<(), PlayerError> {
        let mpv = self.mpv.lock().unwrap();
        mpv.set_property("volume", vol)
            .map_err(|e| PlayerError::SetProperty(format!("volume failed: {e}")))?;
        Ok(())
    }

    pub fn get_volume(&self) -> f64 {
        let mpv = self.mpv.lock().unwrap();
        mpv.get_property("volume").unwrap_or(100.0)
    }

    /// Toggle audio-only mode to conserve GPU/CPU decode resources
    pub fn set_audio_only(&self, audio_only: bool) -> Result<(), PlayerError> {
        let mpv = self.mpv.lock().unwrap();
        let val = if audio_only { "no" } else { "auto" };
        mpv.set_property("vid", val)
            .map_err(|e| PlayerError::SetProperty(format!("vid property failed: {e}")))?;
        Ok(())
    }

    pub fn is_audio_only(&self) -> bool {
        let mpv = self.mpv.lock().unwrap();
        let vid: Result<String, _> = mpv.get_property("vid");
        match vid {
            Ok(v) => v == "no",
            Err(_) => false,
        }
    }

    pub fn get_audio_files(&self) -> String {
        let mpv = self.mpv.lock().unwrap();
        mpv.get_property("audio-files").unwrap_or_default()
    }
}

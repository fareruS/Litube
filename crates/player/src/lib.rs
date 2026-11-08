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
        mpv.set_property("gpu-context", "d3d11")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set gpu-context: {e}")))?;
        mpv.set_property("panscan", 1.0) // Fill the viewport completely without letterbox bars
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set panscan: {e}")))?;
        mpv.set_property("hwdec", "auto-safe")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set hwdec: {e}")))?;
        mpv.set_property("ytdl", "no")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set ytdl: {e}")))?;
        mpv.set_property("keep-open", "yes")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set keep-open: {e}")))?;
        mpv.set_property("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set user-agent: {e}")))?;

        // 3. Strict disk and RAM bounds: prevent unbounded streaming buffers in memory
        mpv.set_property("cache", "yes")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set cache: {e}")))?;
        mpv.set_property("demuxer-max-bytes", 32 * 1024 * 1024 as i64) // 32 MiB max RAM buffer
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set demuxer-max-bytes: {e}")))?;
        mpv.set_property("demuxer-max-back-bytes", 16 * 1024 * 1024 as i64) // 16 MiB backward buffer
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set demuxer-max-back-bytes: {e}")))?;
        mpv.set_property("cache-pause-wait", 0.3)
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set cache-pause-wait: {e}")))?;

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

    /// Load video and attach audio cleanly without invalid option syntax
    pub fn load_video_and_audio(&self, video_url: &str, audio_url: &str) -> Result<(), PlayerError> {
        let mpv = self.mpv.lock().unwrap();

        // 1. Set audio-file property for the next loaded file
        let _ = mpv.set_property("audio-file", audio_url);

        // 2. Load the main video stream
        mpv.command("loadfile", &[video_url])
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
}

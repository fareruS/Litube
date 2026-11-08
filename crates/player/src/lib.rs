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
        mpv.set_property("ytdl", "no")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set ytdl: {e}")))?;
        mpv.set_property("keep-open", "yes")
            .map_err(|e| PlayerError::SetProperty(format!("Failed to set keep-open: {e}")))?;

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

    /// Load video and attach audio via mpv's external-file mechanism
    pub fn load_video_and_audio(&self, video_url: &str, audio_url: &str) -> Result<(), PlayerError> {
        let mpv = self.mpv.lock().unwrap();
        
        // Pass external audio track directly as an option upon loading the file
        // This avoids calling audio-add while the demuxer is still opening
        let audio_opt = format!("audio-file={audio_url}");
        mpv.command("loadfile", &[video_url, "replace", &audio_opt])
            .map_err(|e| PlayerError::Command(format!("loadfile with audio failed: {e}")))?;
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

    pub fn seek(&self, seconds: f64) -> Result<(), PlayerError> {
        let mpv = self.mpv.lock().unwrap();
        let sec_str = seconds.to_string();
        mpv.command("seek", &[&sec_str, "relative"])
            .map_err(|e| PlayerError::Command(format!("seek failed: {e}")))?;
        Ok(())
    }
}

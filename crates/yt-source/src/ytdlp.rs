use crate::traits::{SourceError, StreamSource};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use yt_model::{
    Capabilities, ChannelId, ChannelPage, Page, PlayableMedia, PlaylistId, QualityPref,
    SearchQuery, VideoCard, VideoDetails, VideoId,
};

pub struct YtDlpSource {
    binary_path: PathBuf,
    search_cancel: std::sync::Arc<tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    stream_cancel: std::sync::Arc<tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    stream_cache: Arc<Mutex<HashMap<String, (PlayableMedia, Instant)>>>,
}

impl YtDlpSource {
    pub fn new<P: Into<PathBuf>>(binary_path: P) -> Self {
        Self {
            binary_path: binary_path.into(),
            search_cancel: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            stream_cancel: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            stream_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Auto-detect the yt-dlp executable next to the current binary or in vendor/
    pub fn auto_detect() -> Self {
        let binary_path = if let Ok(exe_path) = std::env::current_exe() {
            let exe_dir = exe_path.parent().unwrap();
            let candidate = exe_dir.join("yt-dlp.exe");
            if candidate.exists() {
                candidate
            } else {
                let root_candidate = std::path::Path::new("vendor/yt-dlp.exe").to_path_buf();
                if root_candidate.exists() {
                    root_candidate
                } else {
                    PathBuf::from("yt-dlp.exe")
                }
            }
        } else {
            PathBuf::from("vendor/yt-dlp.exe")
        };

        Self {
            binary_path,
            search_cancel: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            stream_cancel: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            stream_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn new_command(&self) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new(&self.binary_path);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW: completely hides terminal window
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // Attach cookies if user is signed in via imported cookies
        if let Ok(appdata) = std::env::var("APPDATA") {
            let cookies_file = std::path::PathBuf::from(appdata)
                .join("Litube")
                .join("cookies.txt");
            if cookies_file.exists() {
                if let Some(path_str) = cookies_file.to_str() {
                    cmd.args(["--cookies", path_str]);
                }
            }
        }

        cmd
    }

    async fn execute_isolated(
        cancel_lock: &std::sync::Arc<tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
        mut cmd: tokio::process::Command,
    ) -> Result<std::process::Output, SourceError> {
        // Safely cancel any previously running extractor in this queue without PID reuse risk
        {
            let mut lock = cancel_lock.lock().await;
            if let Some(old_tx) = lock.take() {
                let _ = old_tx.send(());
            }
        }

        let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel();
        {
            let mut lock = cancel_lock.lock().await;
            *lock = Some(cancel_tx);
        }

        cmd.kill_on_drop(true);
        let child = cmd
            .spawn()
            .map_err(|e| SourceError::Network(format!("Failed to spawn extractor: {e}")))?;

        tokio::select! {
            res = child.wait_with_output() => {
                {
                    let mut lock = cancel_lock.lock().await;
                    *lock = None;
                }
                let output = res.map_err(|e| SourceError::Network(format!("Extractor execution aborted: {e}")))?;
                Ok(output)
            }
            _ = &mut cancel_rx => {
                // child future dropped here; kill_on_drop(true) automatically terminates the process safely
                Err(SourceError::Network("Operation cancelled by newer request".into()))
            }
        }
    }
}

#[async_trait]
impl StreamSource for YtDlpSource {
    async fn search(&self, q: &SearchQuery) -> Result<Page<VideoCard>, SourceError> {
        let query_arg = format!("ytsearch15:{}", q.term);
        let mut cmd = self.new_command();
        cmd.args([
            "--dump-single-json",
            "--flat-playlist",
            "--skip-download",
            "--",
            &query_arg,
        ]);
        let output = Self::execute_isolated(&self.search_cancel, cmd).await?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(SourceError::ExtractionFailed(err.into_owned()));
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| SourceError::Parse(format!("Invalid search response JSON: {e}")))?;

        let mut cards = Vec::new();
        if let Some(entries) = json.get("entries").and_then(|v| v.as_array()) {
            for entry in entries {
                let id = entry.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                let ie_key = entry.get("ie_key").and_then(|v| v.as_str()).unwrap_or("");
                if id.is_empty()
                    || ie_key == "YoutubeTab"
                    || (id.starts_with("UC") && id.len() >= 24)
                {
                    continue;
                }
                let title = entry
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Untitled");
                let channel_id = entry
                    .get("channel_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let channel_title = entry
                    .get("channel")
                    .or_else(|| entry.get("uploader"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown Channel");
                let duration_seconds = entry.get("duration").and_then(|v| v.as_u64());
                let view_count = entry.get("view_count").and_then(|v| v.as_u64());

                // Find the best quality thumbnail URL
                let thumbnail_url = entry
                    .get("thumbnails")
                    .and_then(|v| v.as_array())
                    .and_then(|thumbs| thumbs.last())
                    .and_then(|t| t.get("url"))
                    .and_then(|u| u.as_str())
                    .unwrap_or("")
                    .to_string();

                cards.push(VideoCard {
                    id: VideoId(id.to_string()),
                    title: title.to_string(),
                    channel_id: ChannelId(channel_id.to_string()),
                    channel_title: channel_title.to_string(),
                    duration_seconds,
                    view_count,
                    thumbnail_url,
                });
            }
        }

        Ok(Page {
            items: cards,
            next_page_token: None,
        })
    }

    async fn video(&self, id: &VideoId) -> Result<VideoDetails, SourceError> {
        let url = format!("https://www.youtube.com/watch?v={}", id.0);
        let mut cmd = self.new_command();
        cmd.args(["--dump-single-json", "--skip-download", "--", &url]);
        let output = Self::execute_isolated(&self.stream_cancel, cmd).await?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(SourceError::ExtractionFailed(err.into_owned()));
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| SourceError::Parse(format!("Invalid video details JSON: {e}")))?;

        let title = json
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled")
            .to_string();
        let description = json
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let channel_id = json
            .get("channel_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let channel_title = json
            .get("channel")
            .or_else(|| json.get("uploader"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let duration_seconds = json.get("duration").and_then(|v| v.as_u64()).unwrap_or(0);

        Ok(VideoDetails {
            id: id.clone(),
            title,
            description,
            channel_id: ChannelId(channel_id),
            channel_title,
            duration_seconds,
            chapters: vec![],
            captions: vec![],
            related_videos: vec![],
        })
    }

    async fn channel(&self, _id: &ChannelId) -> Result<ChannelPage, SourceError> {
        Err(SourceError::Unsupported)
    }

    async fn playlist(
        &self,
        _id: &PlaylistId,
        _page_token: Option<String>,
    ) -> Result<Page<VideoCard>, SourceError> {
        Err(SourceError::Unsupported)
    }

    async fn playable(
        &self,
        id: &VideoId,
        pref: QualityPref,
    ) -> Result<PlayableMedia, SourceError> {
        let height = pref.max_height.unwrap_or(1080);
        let cache_key = format!("{}:{}", id.0, height);

        // 1. Instant cache hit: skip yt-dlp entirely if extracted within the last 4 hours
        {
            let mut cache = self.stream_cache.lock().unwrap();
            if let Some((media, fetched_at)) = cache.get(&cache_key) {
                if fetched_at.elapsed() < Duration::from_secs(14400) {
                    return Ok(media.clone());
                } else {
                    cache.remove(&cache_key);
                }
            }
        }

        // Prioritize format 18 (unthrottled ratebypass single stream) for <=360p; fallback to VP9 WebM and Opus audio
        let format_filter = if height <= 360 {
            "18/best[height<=360]/bestvideo[height<=360][vcodec^=vp9]+bestaudio[acodec^=opus]/bestvideo[height<=360]+bestaudio/best".to_string()
        } else {
            format!(
                "bestvideo[height<={height}][vcodec^=vp9]+bestaudio[acodec^=opus]/bestvideo[height<={height}][vcodec^=av01]+bestaudio/bestvideo[height<={height}]+bestaudio/best[height<={height}]/best"
            )
        };

        let url = format!("https://www.youtube.com/watch?v={}", id.0);
        let mut cmd = self.new_command();
        cmd.args([
            "--no-playlist",
            "--no-warnings",
            "--js-runtimes",
            "node",
            "--extractor-args",
            "youtube:player_client=android,web,tv",
            "-f",
            &format_filter,
            "-g",
            "--",
            &url,
        ]);
        let output = Self::execute_isolated(&self.stream_cancel, cmd).await?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(SourceError::ExtractionFailed(err.into_owned()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut lines = stdout.lines().filter(|l| l.starts_with("http"));
        let video_url = lines
            .next()
            .ok_or_else(|| SourceError::ExtractionFailed("No video URL returned".into()))?;
        let audio_url = lines.next().unwrap_or(video_url);

        let media = PlayableMedia {
            video_id: id.clone(),
            video_url: Some(video_url.to_string()),
            audio_url: audio_url.to_string(),
            expiry_timestamp: 0,
            headers: vec![],
        };

        // Cache the extracted media
        self.stream_cache
            .lock()
            .unwrap()
            .insert(cache_key, (media.clone(), Instant::now()));

        Ok(media)
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            can_search: true,
            can_stream_dash: true,
            can_stream_audio_only: false,
            supports_chapters: false,
            supports_subtitles: false,
        }
    }
}

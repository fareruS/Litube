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
    video_cancel: std::sync::Arc<tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    stream_cache: Arc<Mutex<HashMap<String, (PlayableMedia, Instant)>>>,
}

/// Hard timeout for a single yt-dlp invocation: without it a hung
/// extractor leaves the UI stuck on "Extracting..." forever.
const EXTRACTOR_TIMEOUT_SECS: u64 = 20;
/// Reject absurdly large `--dump-single-json` output instead of letting it
/// fill RAM (a single video's JSON is far below this).
const MAX_EXTRACTOR_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
/// Treat a stream URL as expired this long before its `expire=` timestamp,
/// so playback never starts on a URL that dies mid-load.
const EXPIRY_SAFETY_MARGIN_SECS: u64 = 300;

impl YtDlpSource {
    const PAGE_SIZE: u32 = 15;

    pub fn new<P: Into<PathBuf>>(binary_path: P) -> Self {
        Self {
            binary_path: binary_path.into(),
            search_cancel: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            stream_cancel: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            video_cancel: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
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
            video_cancel: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
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
                if output.stdout.len() > MAX_EXTRACTOR_OUTPUT_BYTES {
                    return Err(SourceError::Parse(format!(
                        "Extractor output too large ({} bytes, cap {} bytes)",
                        output.stdout.len(),
                        MAX_EXTRACTOR_OUTPUT_BYTES
                    )));
                }
                Ok(output)
            }
            _ = &mut cancel_rx => {
                // child future dropped here; kill_on_drop(true) automatically terminates the process safely
                Err(SourceError::Network("Operation cancelled by newer request".into()))
            }
            _ = tokio::time::sleep(Duration::from_secs(EXTRACTOR_TIMEOUT_SECS)) => {
                // child future dropped here; kill_on_drop(true) terminates the hung process
                Err(SourceError::Network(format!(
                    "Extractor timed out after {EXTRACTOR_TIMEOUT_SECS}s"
                )))
            }
        }
    }
}

#[async_trait]
impl StreamSource for YtDlpSource {
    async fn search(&self, q: &SearchQuery) -> Result<Page<VideoCard>, SourceError> {
        let (start, end) = page_bounds(q.page_token.as_deref())?;
        let query_arg = format!("ytsearch{end}:{}", q.term);
        let mut cmd = self.new_command();
        cmd.args([
            "--dump-single-json",
            "--flat-playlist",
            "--skip-download",
            "--playlist-items",
            &format!("{start}:{end}"),
            "--",
            &query_arg,
        ]);
        let output = Self::execute_isolated(&self.search_cancel, cmd).await?;

        if !output.status.success() {
            return Err(extraction_error(&output.stderr));
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| SourceError::Parse(format!("Invalid search response JSON: {e}")))?;

        let has_more = page_has_more(&json, Self::PAGE_SIZE);
        Ok(make_page(parse_video_cards(&json), end, has_more))
    }

    async fn recommendations(
        &self,
        page_token: Option<String>,
    ) -> Result<Page<VideoCard>, SourceError> {
        let (start, end) = page_bounds(page_token.as_deref())?;
        let mut cmd = self.new_command();
        cmd.args([
            "--dump-single-json",
            "--flat-playlist",
            "--skip-download",
            "--playlist-items",
            &format!("{start}:{end}"),
            "--",
            "https://www.youtube.com/feed/recommended",
        ]);
        let output = Self::execute_isolated(&self.search_cancel, cmd).await?;

        if !output.status.success() {
            return Err(extraction_error(&output.stderr));
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| {
            SourceError::Parse(format!("Invalid recommendation response JSON: {e}"))
        })?;
        let items = parse_video_cards(&json);
        if items.is_empty() {
            return Err(SourceError::ExtractionFailed(
                "YouTube returned no recommendations. Sign in to YouTube and try again.".into(),
            ));
        }

        let has_more = page_has_more(&json, Self::PAGE_SIZE);
        Ok(make_page(items, end, has_more))
    }

    async fn video(&self, id: &VideoId) -> Result<VideoDetails, SourceError> {
        let url = format!("https://www.youtube.com/watch?v={}", id.0);
        let mut cmd = self.new_command();
        cmd.args(["--dump-single-json", "--skip-download", "--", &url]);
        let output = Self::execute_isolated(&self.video_cancel, cmd).await?;

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

        // 1. Instant cache hit: skip yt-dlp entirely if extracted within the
        // last 4 hours AND the stream URL has not expired (with safety margin).
        // An expired cached entry is evicted instead of being handed to mpv,
        // which would otherwise retry a dead 403 URL forever.
        {
            let mut cache = self.stream_cache.lock().unwrap();
            if let Some((media, fetched_at)) = cache.get(&cache_key) {
                if fetched_at.elapsed() < Duration::from_secs(14400)
                    && !is_expired(media, EXPIRY_SAFETY_MARGIN_SECS)
                {
                    return Ok(media.clone());
                } else {
                    cache.remove(&cache_key);
                }
            }
        }

        let format_filter = video_format_filter(height);

        let url = format!("https://www.youtube.com/watch?v={}", id.0);
        let mut cmd = self.new_command();
        cmd.args([
            "--no-playlist",
            "--no-warnings",
            "--js-runtimes",
            "node",
            "-f",
            &format_filter,
            "--dump-single-json",
            "--skip-download",
            "--",
            &url,
        ]);
        let output = Self::execute_isolated(&self.stream_cancel, cmd).await?;

        if !output.status.success() {
            return Err(extraction_error(&output.stderr));
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| SourceError::Parse(format!("Invalid playable response JSON: {e}")))?;
        let media = parse_playable_media(&json, id)?;

        // Cache the extracted media only if it carries a usable expiry.
        // An entry without one could outlive its (unknown) URL lifetime and
        // be served to mpv as a dead 403 link.
        if !is_expired(&media, EXPIRY_SAFETY_MARGIN_SECS) {
            self.stream_cache
                .lock()
                .unwrap()
                .insert(cache_key, (media.clone(), Instant::now()));
        }

        Ok(media)
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            can_search: true,
            can_stream_dash: true,
            can_stream_audio_only: true,
            supports_chapters: false,
            supports_subtitles: false,
        }
    }
}

/// Parse the `expire=` query parameter of a googlevideo stream URL.
/// Returns the expiry as a unix timestamp, or `None` if absent/unparseable.
fn expire_from_url(url: &str) -> Option<i64> {
    let rest = url.split("expire=").nth(1)?;
    let value = rest.split('&').next()?;
    value.parse::<i64>().ok()
}

/// True when the media's stream URL is already expired, or will expire
/// within `margin_secs`. A media entry with no expiry (`0`) is treated as
/// expired: its URL lifetime is unknown, so it must be re-resolved.
fn is_expired(media: &PlayableMedia, margin_secs: u64) -> bool {
    if media.expiry_timestamp == 0 {
        return true;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    now.saturating_add(margin_secs) >= media.expiry_timestamp
}

fn extraction_error(stderr: &[u8]) -> SourceError {
    SourceError::ExtractionFailed(String::from_utf8_lossy(stderr).into_owned())
}

fn page_start(token: Option<&str>) -> Result<u32, SourceError> {
    let offset = token
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|_| SourceError::Parse("Invalid page token".into()))
        })
        .transpose()?
        .unwrap_or(0);
    if offset % YtDlpSource::PAGE_SIZE != 0 {
        return Err(SourceError::Parse("Invalid page token".into()));
    }
    offset
        .checked_add(1)
        .ok_or_else(|| SourceError::Parse("Page token is out of range".into()))
}

fn page_bounds(token: Option<&str>) -> Result<(u32, u32), SourceError> {
    let start = page_start(token)?;
    let end = start
        .checked_add(YtDlpSource::PAGE_SIZE - 1)
        .ok_or_else(|| SourceError::Parse("Page token is out of range".into()))?;
    Ok((start, end))
}

fn video_format_filter(max_height: u32) -> String {
    format!(
        "bestvideo[height={max_height}][vcodec^=avc1]+bestaudio/best[height={max_height}][vcodec^=avc1]/bestvideo[height<={max_height}][fps<=30]+bestaudio/best[height<={max_height}][fps<=30]/bestvideo[height<={max_height}][vcodec^=avc1]+bestaudio/best[height<={max_height}][vcodec^=avc1]/bestvideo[height<={max_height}]+bestaudio/best[height<={max_height}]"
    )
}

fn parse_playable_media(
    json: &serde_json::Value,
    id: &VideoId,
) -> Result<PlayableMedia, SourceError> {
    let formats = json
        .get("requested_formats")
        .and_then(|value| value.as_array());
    let video_format = formats.and_then(|formats| {
        formats.iter().find(|format| {
            format
                .get("vcodec")
                .and_then(|value| value.as_str())
                .is_some_and(|codec| codec != "none")
        })
    });
    let audio_format = formats.and_then(|formats| {
        formats.iter().find(|format| {
            format
                .get("acodec")
                .and_then(|value| value.as_str())
                .is_some_and(|codec| codec != "none")
        })
    });

    let video_url = video_format
        .and_then(|format| format.get("url"))
        .and_then(|value| value.as_str())
        .or_else(|| json.get("url").and_then(|value| value.as_str()))
        .ok_or_else(|| SourceError::ExtractionFailed("No video URL returned".into()))?;
    let audio_url = audio_format
        .and_then(|format| format.get("url"))
        .and_then(|value| value.as_str())
        .or_else(|| json.get("url").and_then(|value| value.as_str()))
        .unwrap_or(video_url);
    let video_height = video_format
        .and_then(|format| format.get("height"))
        .and_then(|value| value.as_u64())
        .or_else(|| json.get("height").and_then(|value| value.as_u64()))
        .and_then(|height| u32::try_from(height).ok());

    // Stream URLs die at their `expire=` timestamp. Take the earliest of
    // the video/audio pair — the media is only usable while both are alive.
    // `0` means no expiry could be read (unknown lifetime; callers treat
    // that as expired so the entry is never served from cache).
    let expiry_timestamp = [video_url, audio_url]
        .iter()
        .filter_map(|url| expire_from_url(url))
        .min()
        .unwrap_or(0)
        .max(0) as u64;

    Ok(PlayableMedia {
        video_id: id.clone(),
        video_url: Some(video_url.to_string()),
        audio_url: audio_url.to_string(),
        video_height,
        expiry_timestamp,
        headers: vec![],
    })
}

fn page_has_more(json: &serde_json::Value, page_size: u32) -> bool {
    json.get("entries")
        .and_then(|value| value.as_array())
        .is_some_and(|entries| entries.len() as u32 == page_size)
}

fn make_page(items: Vec<VideoCard>, end: u32, has_more: bool) -> Page<VideoCard> {
    let next_page_token = has_more.then(|| end.to_string());
    Page {
        items,
        next_page_token,
    }
}

fn parse_video_cards(json: &serde_json::Value) -> Vec<VideoCard> {
    json.get("entries")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let id = entry.get("id").and_then(|value| value.as_str())?;
            let ie_key = entry
                .get("ie_key")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            if id.is_empty() || ie_key == "YoutubeTab" || (id.starts_with("UC") && id.len() >= 24) {
                return None;
            }

            let thumbnail_url = entry
                .get("thumbnails")
                .and_then(|value| value.as_array())
                .and_then(|thumbnails| thumbnails.last())
                .and_then(|thumbnail| thumbnail.get("url"))
                .and_then(|url| url.as_str())
                .unwrap_or("")
                .to_string();
            Some(VideoCard {
                id: VideoId(id.to_string()),
                title: entry
                    .get("title")
                    .and_then(|value| value.as_str())
                    .unwrap_or("Untitled")
                    .to_string(),
                channel_id: ChannelId(
                    entry
                        .get("channel_id")
                        .and_then(|value| value.as_str())
                        .unwrap_or("")
                        .to_string(),
                ),
                channel_title: entry
                    .get("channel")
                    .or_else(|| entry.get("uploader"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("Unknown Channel")
                    .to_string(),
                duration_seconds: entry.get("duration").and_then(|value| value.as_u64()),
                view_count: entry.get("view_count").and_then(|value| value.as_u64()),
                thumbnail_url,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_tokens_are_offsets_in_multiples_of_page_size() {
        assert_eq!(page_start(None).unwrap(), 1);
        assert_eq!(page_start(Some("15")).unwrap(), 16);
        assert!(page_start(Some("16")).is_err());
        assert!(page_start(Some("invalid")).is_err());
    }

    #[test]
    fn quality_filter_prefers_requested_resolution_and_caps_frame_rate() {
        assert_eq!(
            video_format_filter(1080),
            "bestvideo[height=1080][vcodec^=avc1]+bestaudio/best[height=1080][vcodec^=avc1]/bestvideo[height<=1080][fps<=30]+bestaudio/best[height<=1080][fps<=30]/bestvideo[height<=1080][vcodec^=avc1]+bestaudio/best[height<=1080][vcodec^=avc1]/bestvideo[height<=1080]+bestaudio/best[height<=1080]"
        );
    }

    #[test]
    fn selected_format_metadata_reports_actual_video_height() {
        let json = serde_json::json!({
            "requested_formats": [
                { "url": "https://video", "vcodec": "av01", "acodec": "none", "height": 1080 },
                { "url": "https://audio", "vcodec": "none", "acodec": "opus" }
            ]
        });
        let media = parse_playable_media(&json, &VideoId("video-id".into())).unwrap();
        assert_eq!(media.video_url.as_deref(), Some("https://video"));
        assert_eq!(media.audio_url, "https://audio");
        assert_eq!(media.video_height, Some(1080));
    }

    #[test]
    fn expire_from_url_parses_googlevideo_timestamp() {
        assert_eq!(
            expire_from_url("https://rr1---sn-abc.googlevideo.com/videoplayback?expire=1700021600&ei=xyz"),
            Some(1_700_021_600)
        );
        assert_eq!(expire_from_url("https://example.com/no-expire"), None);
        assert_eq!(expire_from_url("https://example.com/?expire=notanumber"), None);
    }

    #[test]
    fn playable_media_expiry_uses_earliest_of_pair() {
        let json = serde_json::json!({
            "requested_formats": [
                { "url": "https://video?expire=1700021600", "vcodec": "avc1", "acodec": "none", "height": 720 },
                { "url": "https://audio?expire=1700018000", "vcodec": "none", "acodec": "opus" }
            ]
        });
        let media = parse_playable_media(&json, &VideoId("video-id".into())).unwrap();
        assert_eq!(media.expiry_timestamp, 1_700_018_000);
    }

    #[test]
    fn playable_media_without_expiry_reports_zero() {
        let json = serde_json::json!({
            "requested_formats": [
                { "url": "https://video", "vcodec": "avc1", "acodec": "none", "height": 720 },
                { "url": "https://audio", "vcodec": "none", "acodec": "opus" }
            ]
        });
        let media = parse_playable_media(&json, &VideoId("video-id".into())).unwrap();
        assert_eq!(media.expiry_timestamp, 0);
    }

    #[test]
    fn is_expired_treats_missing_expiry_and_near_expiry_as_expired() {
        let no_expiry = PlayableMedia {
            video_id: VideoId("id".into()),
            video_url: Some("https://video".into()),
            audio_url: "https://audio".into(),
            video_height: None,
            expiry_timestamp: 0,
            headers: vec![],
        };
        assert!(is_expired(&no_expiry, EXPIRY_SAFETY_MARGIN_SECS));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let far_future = PlayableMedia {
            expiry_timestamp: now + 3600,
            ..no_expiry.clone()
        };
        assert!(!is_expired(&far_future, EXPIRY_SAFETY_MARGIN_SECS));

        let about_to_expire = PlayableMedia {
            expiry_timestamp: now + 60,
            ..no_expiry
        };
        assert!(is_expired(&about_to_expire, EXPIRY_SAFETY_MARGIN_SECS));
    }

    #[test]
    fn full_page_has_next_token_and_short_page_ends() {
        let card = VideoCard {
            id: VideoId("id".into()),
            title: "title".into(),
            channel_id: ChannelId("channel".into()),
            channel_title: "channel".into(),
            duration_seconds: None,
            view_count: None,
            thumbnail_url: String::new(),
        };
        assert_eq!(
            make_page(vec![card.clone()], 15, false).next_page_token,
            None
        );
        assert_eq!(
            make_page(vec![card; 15], 15, true).next_page_token,
            Some("15".into())
        );
    }
}

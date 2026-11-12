use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VideoId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlaylistId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoCard {
    pub id: VideoId,
    pub title: String,
    pub channel_id: ChannelId,
    pub channel_title: String,
    pub duration_seconds: Option<u64>,
    pub view_count: Option<u64>,
    pub thumbnail_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub title: String,
    pub start_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptionTrack {
    pub language_code: String,
    pub name: String,
    pub url: String,
    pub is_auto_generated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoDetails {
    pub id: VideoId,
    pub title: String,
    pub description: String,
    pub channel_id: ChannelId,
    pub channel_title: String,
    pub duration_seconds: u64,
    pub chapters: Vec<Chapter>,
    pub captions: Vec<CaptionTrack>,
    pub related_videos: Vec<VideoCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelPage {
    pub id: ChannelId,
    pub title: String,
    pub description: Option<String>,
    pub avatar_url: String,
    pub subscriber_count: Option<String>,
    pub videos: Vec<VideoCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamFormat {
    pub itag: u32,
    pub url: String,
    pub mime_type: String,
    pub quality_label: Option<String>,
    pub bitrate: Option<u64>,
    pub is_audio_only: bool,
    pub is_video_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayableMedia {
    pub video_id: VideoId,
    pub video_url: Option<String>,
    pub audio_url: String,
    #[serde(default)]
    pub video_height: Option<u32>,
    pub expiry_timestamp: u64,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QualityPref {
    pub max_height: Option<u32>,
    pub audio_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub term: String,
    pub page_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Capabilities {
    pub can_search: bool,
    pub can_stream_dash: bool,
    pub can_stream_audio_only: bool,
    pub supports_chapters: bool,
    pub supports_subtitles: bool,
}

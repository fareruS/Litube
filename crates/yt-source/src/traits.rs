use async_trait::async_trait;
use thiserror::Error;
use yt_model::{
    Capabilities, ChannelId, ChannelPage, Page, PlayableMedia, PlaylistId, QualityPref,
    SearchQuery, VideoCard, VideoDetails, VideoId,
};

#[derive(Error, Debug)]
pub enum SourceError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Serialization / parsing error: {0}")]
    Parse(String),
    #[error("Resource not found: {0}")]
    NotFound(String),
    #[error("Stream extraction failed: {0}")]
    ExtractionFailed(String),
    #[error("Operation not supported by this stream source")]
    Unsupported,
}

#[async_trait]
pub trait StreamSource: Send + Sync {
    async fn search(&self, q: &SearchQuery) -> Result<Page<VideoCard>, SourceError>;
    async fn video(&self, id: &VideoId) -> Result<VideoDetails, SourceError>;
    async fn channel(&self, id: &ChannelId) -> Result<ChannelPage, SourceError>;
    async fn playlist(&self, id: &PlaylistId, page_token: Option<String>) -> Result<Page<VideoCard>, SourceError>;
    async fn playable(&self, id: &VideoId, pref: QualityPref) -> Result<PlayableMedia, SourceError>;
    fn capabilities(&self) -> Capabilities;
}


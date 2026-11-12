use crate::traits::{SourceError, StreamSource};
use async_trait::async_trait;
use yt_model::{
    Capabilities, ChannelId, ChannelPage, Page, PlayableMedia, PlaylistId, QualityPref,
    SearchQuery, VideoCard, VideoDetails, VideoId,
};

pub struct FixtureSource {
    fixtures_dir: std::path::PathBuf,
}

impl FixtureSource {
    pub fn new<P: Into<std::path::PathBuf>>(fixtures_dir: P) -> Self {
        Self {
            fixtures_dir: fixtures_dir.into(),
        }
    }
}

#[async_trait]
impl StreamSource for FixtureSource {
    async fn search(&self, _q: &SearchQuery) -> Result<Page<VideoCard>, SourceError> {
        let file = self.fixtures_dir.join("search_results.json");
        if !file.exists() {
            return Ok(Page {
                items: vec![VideoCard {
                    id: VideoId("fixture_01".into()),
                    title: "Offline Test Video".into(),
                    channel_id: ChannelId("chan_01".into()),
                    channel_title: "Offline Channel".into(),
                    duration_seconds: Some(60),
                    view_count: Some(1000),
                    thumbnail_url: "https://example.com/thumb.jpg".into(),
                }],
                next_page_token: None,
            });
        }
        let content = tokio::fs::read_to_string(&file)
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;
        let page: Page<VideoCard> =
            serde_json::from_str(&content).map_err(|e| SourceError::Parse(e.to_string()))?;
        Ok(page)
    }

    async fn recommendations(
        &self,
        page_token: Option<String>,
    ) -> Result<Page<VideoCard>, SourceError> {
        self.search(&SearchQuery {
            term: "recommendations".into(),
            page_token,
        })
        .await
    }

    async fn video(&self, id: &VideoId) -> Result<VideoDetails, SourceError> {
        Ok(VideoDetails {
            id: id.clone(),
            title: "Offline Test Video Details".into(),
            description: "A pure offline mock fixture video details description.".into(),
            channel_id: ChannelId("chan_01".into()),
            channel_title: "Offline Channel".into(),
            duration_seconds: 60,
            chapters: vec![],
            captions: vec![],
            related_videos: vec![],
        })
    }

    async fn channel(&self, id: &ChannelId) -> Result<ChannelPage, SourceError> {
        Ok(ChannelPage {
            id: id.clone(),
            title: "Offline Channel".into(),
            description: Some("Mock offline channel".into()),
            avatar_url: "https://example.com/avatar.jpg".into(),
            subscriber_count: Some("10K".into()),
            videos: vec![],
        })
    }

    async fn playlist(
        &self,
        _id: &PlaylistId,
        _page_token: Option<String>,
    ) -> Result<Page<VideoCard>, SourceError> {
        Ok(Page {
            items: vec![],
            next_page_token: None,
        })
    }

    async fn playable(
        &self,
        id: &VideoId,
        _pref: QualityPref,
    ) -> Result<PlayableMedia, SourceError> {
        Ok(PlayableMedia {
            video_id: id.clone(),
            video_url: Some("tests/fixtures/sample.mp4".into()),
            audio_url: "tests/fixtures/sample.mp4".into(),
            video_height: Some(480),
            expiry_timestamp: 4102444800, // Year 2100
            headers: vec![],
        })
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            can_search: true,
            can_stream_dash: false,
            can_stream_audio_only: true,
            supports_chapters: true,
            supports_subtitles: true,
        }
    }
}

use yt_model::{QualityPref, SearchQuery, VideoId};
use yt_source::{StreamSource, YtDlpSource};

#[test]
fn test_ytdlp_capabilities_match_implemented_features() {
    let caps = YtDlpSource::auto_detect().capabilities();
    assert!(caps.can_search);
    assert!(caps.can_stream_dash);
    assert!(!caps.can_stream_audio_only);
    assert!(!caps.supports_chapters);
    assert!(!caps.supports_subtitles);
}

#[tokio::test]
#[ignore = "requires yt-dlp and network access"]
async fn test_ytdlp_capabilities_and_search() {
    let source = YtDlpSource::auto_detect();
    let caps = source.capabilities();
    assert!(caps.can_search);
    assert!(caps.can_stream_dash);

    let page = source
        .search(&SearchQuery {
            term: "big buck bunny".into(),
            page_token: None,
        })
        .await
        .expect("Search should succeed");

    assert!(!page.items.is_empty(), "Search should return items");
    let first = &page.items[0];
    assert!(!first.id.0.is_empty(), "Video ID must not be empty");
    assert!(!first.title.is_empty(), "Title must not be empty");
}

#[tokio::test]
#[ignore = "requires yt-dlp and network access"]
async fn test_ytdlp_playable_resolution() {
    let source = YtDlpSource::auto_detect();
    let pref = QualityPref {
        max_height: Some(720),
        audio_only: false,
    };

    let media = source
        .playable(&VideoId("aqz-KE-bpKQ".into()), pref)
        .await
        .expect("Playable stream extraction should succeed");

    let video_url = media.video_url.expect("Video URL must be present");
    assert!(
        video_url.starts_with("http"),
        "Video URL must be valid HTTP"
    );
    assert!(
        media.audio_url.starts_with("http"),
        "Audio URL must be valid HTTP"
    );
}

#[tokio::test]
#[ignore = "requires yt-dlp and network access"]
async fn test_concurrent_search_and_playable_do_not_conflict() {
    let source = std::sync::Arc::new(YtDlpSource::auto_detect());
    let s1 = source.clone();
    let s2 = source.clone();

    // Spawn search and playable concurrently to ensure isolated PID tracking
    let search_task = tokio::spawn(async move {
        s1.search(&SearchQuery {
            term: "test".into(),
            page_token: None,
        })
        .await
    });

    let playable_task = tokio::spawn(async move {
        s2.playable(
            &VideoId("aqz-KE-bpKQ".into()),
            QualityPref {
                max_height: Some(360),
                audio_only: false,
            },
        )
        .await
    });

    let (search_res, playable_res) = tokio::join!(search_task, playable_task);
    search_res
        .expect("Search task should not panic")
        .expect("Search should succeed");
    playable_res
        .expect("Playback task should not panic")
        .expect("Stream extraction should succeed");
}

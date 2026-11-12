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
            term: "music".into(),
            page_token: None,
        })
        .await
        .expect("Search should succeed");

    assert!(!page.items.is_empty(), "Search should return items");
    let first = &page.items[0];
    assert!(!first.id.0.is_empty(), "Video ID must not be empty");
    assert!(!first.title.is_empty(), "Title must not be empty");

    let next_page_token = page
        .next_page_token
        .expect("Full page should expose a next token");
    let next_page = source
        .search(&SearchQuery {
            term: "music".into(),
            page_token: Some(next_page_token),
        })
        .await
        .expect("Next search page should succeed");
    assert!(
        !next_page.items.is_empty(),
        "Next search page should return items"
    );
    assert!(
        page.items
            .iter()
            .all(|item| !next_page.items.iter().any(|next| next.id == item.id)),
        "Pages should not repeat videos"
    );
}

#[tokio::test]
#[ignore = "requires yt-dlp and network access"]
async fn test_ytdlp_playable_resolution() {
    let source = YtDlpSource::auto_detect();
    let pref = QualityPref {
        max_height: Some(1080),
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
    assert!(
        media
            .video_height
            .is_some_and(|height| height > 0 && height <= 1080),
        "Selected video height should be reported and respect the 1080p cap"
    );
    eprintln!("yt-dlp selected {}p", media.video_height.unwrap());
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

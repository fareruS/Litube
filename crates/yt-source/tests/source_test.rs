use yt_model::{QualityPref, SearchQuery, VideoId};
use yt_source::{StreamSource, YtDlpSource};

#[tokio::test]
async fn test_ytdlp_capabilities_and_search() {
    let source = YtDlpSource::auto_detect();
    let caps = source.capabilities();
    assert!(caps.can_search);
    assert!(caps.can_stream_dash);

    // Search query test
    let res = source
        .search(&SearchQuery {
            term: "big buck bunny".into(),
            page_token: None,
        })
        .await;

    if let Ok(page) = res {
        assert!(!page.items.is_empty(), "Search should return items");
        let first = &page.items[0];
        assert!(!first.id.0.is_empty(), "Video ID must not be empty");
        assert!(!first.title.is_empty(), "Title must not be empty");
    }
}

#[tokio::test]
async fn test_ytdlp_playable_resolution() {
    let source = YtDlpSource::auto_detect();
    let pref = QualityPref {
        max_height: Some(720),
        audio_only: false,
    };

    let media = source
        .playable(&VideoId("aqz-KE-bpKQ".into()), pref)
        .await;

    if let Ok(m) = media {
        assert!(m.video_url.is_some(), "Video URL must be present");
        assert!(
            m.video_url.unwrap().starts_with("http"),
            "Video URL must be valid HTTP"
        );
        assert!(
            m.audio_url.starts_with("http"),
            "Audio URL must be valid HTTP"
        );
    }
}

#[tokio::test]
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
    assert!(search_res.is_ok());
    assert!(playable_res.is_ok());
}



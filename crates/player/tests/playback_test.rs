use player::VideoPlayer;

#[test]
fn test_player_initialization_and_audio_only_toggle() {
    let player = VideoPlayer::new_headless().expect("Headless player must initialize cleanly");

    // Verify speed control
    player
        .set_speed(1.25)
        .expect("Speed adjustment must succeed");
    player.set_speed(1.0).expect("Reset speed must succeed");

    // Verify volume control
    player
        .set_volume(80.0)
        .expect("Volume adjustment must succeed");

    // Verify audio-only toggle
    player
        .set_audio_only(true)
        .expect("Set audio-only must succeed");
    assert!(player.is_audio_only());

    player
        .set_audio_only(false)
        .expect("Disable audio-only must succeed");
    assert!(!player.is_audio_only());
}

#[test]
fn test_player_rapid_20_seeks_stability() {
    let player = VideoPlayer::new_headless().expect("Headless player must initialize cleanly");
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/sample.mp4");
    player
        .load_file(fixture.to_str().expect("Fixture path must be valid UTF-8"))
        .expect("Local fixture must load before seeking");

    // Seek within the local 60-second fixture to exercise the demuxer.
    for i in 1..=20 {
        let seek_pos = (i as f64) * 2.5;
        player
            .seek_absolute(seek_pos)
            .expect("Absolute seek should succeed");
        player.seek(-1.0).expect("Relative seek should succeed");
    }

    player.stop().expect("Player should stop cleanly");
}

#[test]
fn test_audio_and_start_properties() {
    let mpv = libmpv2::Mpv::new().unwrap();
    mpv.set_property("vo", "null").unwrap();
    mpv.set_property("ao", "null").unwrap();
    mpv.set_property("hr-seek", "yes").unwrap();
    mpv.set_property("hr-seek-framedrop", "yes").unwrap();

    let test_audio = "https://example.com/audio.webm";
    mpv.set_property("audio-files", test_audio).unwrap();
    let files: Result<String, _> = mpv.get_property("audio-files");
    assert_eq!(files.unwrap(), test_audio);

    // Verify start offset as string
    mpv.set_property("start", "42.50").unwrap();

    mpv.set_property("scale", "ewa_lanczos").unwrap();
    mpv.set_property("cscale", "ewa_lanczos").unwrap();
    mpv.set_property("vd-lavc-dr", "yes").unwrap();
    mpv.set_property("vd-lavc-threads", 2 as i64).unwrap();

    mpv.set_property("referrer", "https://www.youtube.com/")
        .unwrap();
    mpv.set_property("http-header-fields", "Origin: https://www.youtube.com\r\nReferer: https://www.youtube.com/\r\nSec-Fetch-Mode: no-cors").unwrap();

    let pfc: bool = mpv.get_property("paused-for-cache").unwrap_or(false);
    assert!(!pfc);

    mpv.set_property("hr-seek", "default").unwrap();
    let hr_seek: Result<String, _> = mpv.get_property("hr-seek");
    assert_eq!(hr_seek.unwrap(), "default");

    // Test switching video tracks and unpause behavior
    let player = VideoPlayer::new_headless().unwrap();
    player.toggle_pause().unwrap(); // Now paused
    assert!(player.is_paused());

    // Loading new video must unpause and load with replace
    player
        .load_video_and_audio(
            "https://example.com/vid1.mp4",
            "https://example.com/aud1.mp4",
            None,
        )
        .unwrap();
    assert!(!player.is_paused());

    // Switching to second video must also succeed
    player
        .load_video_and_audio(
            "https://example.com/vid2.mp4",
            "https://example.com/vid2.mp4",
            Some(10.0),
        )
        .unwrap();
    assert!(!player.is_paused());

    let aud = player.get_audio_files();
    assert_eq!(aud, "");
}

#[test]
#[ignore = "requires yt-dlp, Node.js, and network access"]
fn test_long_playback_stalling_or_stopping() {
    let mpv = libmpv2::Mpv::new().unwrap();
    mpv.set_property("vo", "null").unwrap();
    mpv.set_property("ao", "null").unwrap();
    mpv.set_property("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .unwrap();
    mpv.set_property("cache", "yes").unwrap();
    mpv.set_property("demuxer-max-bytes", 64 * 1024 * 1024 as i64)
        .unwrap();
    mpv.set_property("demuxer-max-back-bytes", 32 * 1024 * 1024 as i64)
        .unwrap();
    mpv.set_property("cache-pause-wait", 0.5).unwrap();
    mpv.set_property("hr-seek", "yes").unwrap();
    mpv.set_property("hr-seek-framedrop", "yes").unwrap();

    let sidecar_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/yt-dlp.exe");
    assert!(
        sidecar_path.exists(),
        "yt-dlp sidecar is missing: {}",
        sidecar_path.display()
    );

    let output = std::process::Command::new(sidecar_path)
        .args([
            "--js-runtimes",
            "node",
            "-f",
            "bestvideo[height<=720]+bestaudio/best[height<=720]",
            "-g",
            "https://www.youtube.com/watch?v=aqz-KE-bpKQ",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "yt-dlp failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines().filter(|l| l.starts_with("http"));
    let video_url = lines.next().expect("video url");
    let audio_url = lines.next().unwrap_or(video_url);

    mpv.set_property("audio-files", audio_url).unwrap();
    mpv.command("loadfile", &[video_url]).unwrap();

    for i in 1..=20 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let pos: Result<f64, _> = mpv.get_property("time-pos");
        let paused: Result<bool, _> = mpv.get_property("pause");
        let eof: Result<bool, _> = mpv.get_property("eof-reached");
        let idle: Result<bool, _> = mpv.get_property("idle-active");
        let core_idle: Result<bool, _> = mpv.get_property("core-idle");
        println!(
            "Sec {:.1}: pos={:?}, paused={:?}, eof={:?}, idle={:?}, core-idle={:?}",
            (i as f64) * 0.5,
            pos,
            paused,
            eof,
            idle,
            core_idle
        );
    }
}

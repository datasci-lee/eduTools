use edutools_core::music::{BreakMusicFade, BreakMusicUrl};

// Swift: BreakMusicTests.playlistURLRequiresHTTPSMusicHost()
#[test]
fn playlist_url_requires_https_music_host() {
    assert!(BreakMusicUrl::validated("https://music.youtube.com/playlist?list=valid").is_some());
    assert!(BreakMusicUrl::validated("https://music.youtube.com.evil.example/playlist").is_none());
    assert!(BreakMusicUrl::validated("http://music.youtube.com/playlist").is_none());
}

// Swift: BreakMusicTests.fadeIsLinearAndTracksEndDate()
#[test]
fn fade_is_linear_and_tracks_end_date() {
    let cases = [
        ("remaining equals duration", 60.0, 60.0, 0.8_f32),
        ("middle", 30.0, 60.0, 0.4_f32),
        ("finished", 0.0, 60.0, 0.0_f32),
        ("zero duration", 30.0, 0.0, 0.8_f32),
    ];

    for (name, remaining, duration, expected) in cases {
        let actual = BreakMusicFade::volume(0.8, 2_000_000.0, 2_000_000.0 + remaining, duration);
        assert!((actual - expected).abs() < 0.000_001, "case={name}");
    }
}

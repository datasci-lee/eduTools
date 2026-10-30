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
    let original_volume = 0.8_f32;
    let now = 2_000_000.0;
    let during_fade = BreakMusicFade::volume(original_volume, now, now + 30.0, 60.0);
    let after_extension = BreakMusicFade::volume(original_volume, now, now + 120.0, 60.0);

    assert!((during_fade - 0.4).abs() < 0.000_001);
    assert!((after_extension - original_volume).abs() < 0.000_001);
}

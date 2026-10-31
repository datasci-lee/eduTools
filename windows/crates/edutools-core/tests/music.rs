use edutools_core::music::{
    collect_music_tracks, BreakMusicFade, BreakMusicUrl, MusicDirectoryEntry, MusicDirectoryReader,
    MusicOrder, MusicRepeat, MusicTrack, Playlist, PlaylistRng, MAX_MUSIC_TRACKS,
};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

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

#[derive(Default)]
struct FakeReader {
    entries: BTreeMap<PathBuf, Vec<MusicDirectoryEntry>>,
    failures: BTreeSet<PathBuf>,
}

impl FakeReader {
    fn add(&mut self, directory: &str, name: &str, is_directory: bool) {
        let path = Path::new(directory).join(name);
        self.entries
            .entry(PathBuf::from(directory))
            .or_default()
            .push(MusicDirectoryEntry {
                file_name: OsString::from(name),
                path,
                is_directory,
                is_file: !is_directory,
            });
    }
}

impl MusicDirectoryReader for FakeReader {
    fn read_directory(&self, path: &Path) -> io::Result<Vec<MusicDirectoryEntry>> {
        if self.failures.contains(path) {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"));
        }
        Ok(self.entries.get(path).cloned().unwrap_or_default())
    }
}

// Swift: LocalMusicTests.collectionFiltersDepthCapsAndNaturalSorts()
#[test]
fn collection_filters_depth_caps_and_natural_sorts() {
    let mut reader = FakeReader::default();
    for (directory, name, is_directory) in [
        ("root", ".hidden", true),
        ("root", ".hidden.mp3", false),
        ("root", "notes.txt", false),
        ("root", "10.mp3", false),
        ("root", "2.MP3", false),
        ("root", "level1", true),
        ("root/level1", "song.m4a", false),
        ("root/level1", "level2", true),
        ("root/level1/level2", "deep.aac", false),
        ("root/level1/level2", "level3", true),
        ("root/level1/level2/level3", "edge.wav", false),
        ("root/level1/level2/level3", "level4", true),
        ("root/level1/level2/level3/level4", "too-deep.flac", false),
    ] {
        reader.add(directory, name, is_directory);
    }
    reader.add("root/.hidden", "inside.mp3", false);
    let collection = collect_music_tracks(Path::new("root"), &reader);
    assert_eq!(collection.failure_reason, None);
    assert_eq!(
        collection
            .tracks
            .iter()
            .map(|track| track.relative_path.as_str())
            .collect::<Vec<_>>(),
        [
            "2.MP3",
            "10.mp3",
            "level1/level2/deep.aac",
            "level1/level2/level3/edge.wav",
            "level1/song.m4a",
        ]
    );

    let mut capped = FakeReader::default();
    for index in 0..=MAX_MUSIC_TRACKS {
        capped.add("root", &format!("{index}.mp3"), false);
    }
    let collection = collect_music_tracks(Path::new("root"), &capped);
    assert_eq!(collection.tracks.len(), MAX_MUSIC_TRACKS);
    assert_eq!(collection.tracks[2].relative_path, "2.mp3");
    assert_eq!(collection.tracks[10].relative_path, "10.mp3");
    assert_eq!(collection.tracks.last().unwrap().relative_path, "499.mp3");

    let mut unreadable = FakeReader::default();
    unreadable.failures.insert(PathBuf::from("root"));
    let collection = collect_music_tracks(Path::new("root"), &unreadable);
    assert!(collection.tracks.is_empty());
    assert!(collection.failure_reason.unwrap().contains("denied"));
}

struct ZeroRng;

impl PlaylistRng for ZeroRng {
    fn next_u64(&mut self) -> u64 {
        0
    }
}

fn tracks(names: &[&str]) -> Vec<MusicTrack> {
    names
        .iter()
        .map(|name| MusicTrack {
            path: PathBuf::from(name),
            relative_path: (*name).to_owned(),
        })
        .collect()
}

// Swift: LocalMusicTests.playlistWrapRepeatShuffleAndFailureState()
#[test]
fn playlist_wrap_repeat_shuffle_and_failure_state() {
    let mut named = Playlist::new(
        tracks(&["10.mp3", "2.mp3"]),
        MusicOrder::Name,
        MusicRepeat::All,
        ZeroRng,
    );
    assert_eq!(named.current().unwrap().relative_path, "2.mp3");
    assert_eq!(named.advance(false).unwrap().relative_path, "10.mp3");
    assert_eq!(named.advance(false).unwrap().relative_path, "2.mp3");

    let mut repeated = Playlist::new(
        tracks(&["1.mp3", "2.mp3"]),
        MusicOrder::Name,
        MusicRepeat::One,
        ZeroRng,
    );
    assert_eq!(repeated.advance(false).unwrap().relative_path, "1.mp3");
    assert_eq!(repeated.advance(true).unwrap().relative_path, "2.mp3");
    repeated.report_failure();
    assert!(!repeated.exhausted());
    repeated.report_success();
    assert_eq!(repeated.consecutive_failures(), 0);
    repeated.report_failure();
    repeated.report_failure();
    assert!(repeated.exhausted());

    let mut shuffled = Playlist::new(
        tracks(&["1.mp3", "2.mp3"]),
        MusicOrder::Shuffle,
        MusicRepeat::All,
        ZeroRng,
    );
    for _ in 1..shuffled.track_count() {
        shuffled.advance(false);
    }
    let previous = shuffled.current().unwrap().relative_path.clone();
    let wrapped = shuffled.advance(false).unwrap().relative_path.clone();
    assert_ne!(wrapped, previous);

    let empty = Playlist::new(Vec::new(), MusicOrder::Name, MusicRepeat::All, ZeroRng);
    assert!(empty.current().is_none());
}

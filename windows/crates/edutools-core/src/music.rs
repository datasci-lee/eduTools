//! Break-music URL validation, local-folder playlists, and end-date-driven fading.

use std::cmp::Ordering;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

// Swift: BreakMusicURL
pub struct BreakMusicUrl;

impl BreakMusicUrl {
    pub const REQUIRED_PREFIX: &'static str = "https://music.youtube.com/";

    // Swift: BreakMusicURL.validated(_:)
    pub fn validated(value: &str) -> Option<&str> {
        if !value.starts_with(Self::REQUIRED_PREFIX) || value.chars().any(char::is_control) {
            return None;
        }

        // The exact, lowercase prefix makes the scheme HTTPS, the authority exactly
        // music.youtube.com, and user/password components impossible without a URL crate.
        Some(value)
    }
}

// Swift: BreakMusicFade
pub struct BreakMusicFade;

impl BreakMusicFade {
    // Swift: BreakMusicFade.volume(originalVolume:now:endDate:duration:)
    pub fn volume(original_volume: f32, now: f64, end_date: f64, duration: f64) -> f32 {
        if duration <= 0.0 {
            return original_volume;
        }
        let remaining = end_date - now;
        if remaining >= duration {
            return original_volume;
        }
        if remaining <= 0.0 {
            return 0.0;
        }
        original_volume * (remaining / duration) as f32
    }
}

pub const MAX_MUSIC_TRACKS: usize = 500;
pub const MAX_MUSIC_FOLDER_DEPTH: usize = 3;
const MUSIC_EXTENSIONS: &[&str] = &["mp3", "m4a", "aac", "wav", "flac"];

// Swift: LocalMusicTrack
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MusicTrack {
    pub path: PathBuf,
    pub relative_path: String,
}

// Swift: LocalMusicDirectoryEntry
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MusicDirectoryEntry {
    pub file_name: OsString,
    pub path: PathBuf,
    pub is_directory: bool,
    pub is_file: bool,
}

pub trait MusicDirectoryReader {
    fn read_directory(&self, path: &Path) -> io::Result<Vec<MusicDirectoryEntry>>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FileSystemMusicDirectoryReader;

impl MusicDirectoryReader for FileSystemMusicDirectoryReader {
    fn read_directory(&self, path: &Path) -> io::Result<Vec<MusicDirectoryEntry>> {
        std::fs::read_dir(path)?
            .map(|entry| {
                let entry = entry?;
                let file_type = entry.file_type()?;
                Ok(MusicDirectoryEntry {
                    file_name: entry.file_name(),
                    path: entry.path(),
                    is_directory: file_type.is_dir(),
                    is_file: file_type.is_file(),
                })
            })
            .collect()
    }
}

// Swift: LocalMusicCollection
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MusicCollection {
    pub tracks: Vec<MusicTrack>,
    pub failure_reason: Option<String>,
}

// Swift: LocalMusicCollector.collect(root:reader:)
pub fn collect_music_tracks(root: &Path, reader: &impl MusicDirectoryReader) -> MusicCollection {
    let mut tracks = Vec::new();
    if let Err(error) = collect_directory(root, Path::new(""), 0, reader, &mut tracks) {
        return MusicCollection {
            tracks: Vec::new(),
            failure_reason: Some(error),
        };
    }
    tracks.sort_by(|left, right| natural_path_cmp(&left.relative_path, &right.relative_path));
    tracks.truncate(MAX_MUSIC_TRACKS);
    MusicCollection {
        tracks,
        failure_reason: None,
    }
}

fn collect_directory(
    directory: &Path,
    relative_directory: &Path,
    depth: usize,
    reader: &impl MusicDirectoryReader,
    tracks: &mut Vec<MusicTrack>,
) -> Result<(), String> {
    let mut entries = reader
        .read_directory(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?;
    entries.sort_by(|left, right| {
        natural_path_cmp(
            &left.file_name.to_string_lossy(),
            &right.file_name.to_string_lossy(),
        )
    });
    for entry in entries {
        let name = entry.file_name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let relative_path = relative_directory.join(&entry.file_name);
        if entry.is_directory && depth < MAX_MUSIC_FOLDER_DEPTH {
            collect_directory(&entry.path, &relative_path, depth + 1, reader, tracks)?;
        } else if entry.is_file && has_music_extension(&entry.path) {
            tracks.push(MusicTrack {
                path: entry.path,
                relative_path: relative_path.to_string_lossy().into_owned(),
            });
        }
    }
    Ok(())
}

fn has_music_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            MUSIC_EXTENSIONS
                .iter()
                .any(|allowed| extension.eq_ignore_ascii_case(allowed))
        })
}

// Swift: LocalMusicNameComparator.compare(_:_:)
pub fn natural_path_cmp(left: &str, right: &str) -> Ordering {
    let left_original = left;
    let right_original = right;
    let left = left_original.to_lowercase();
    let right = right_original.to_lowercase();
    let mut left_index = 0;
    let mut right_index = 0;
    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    while left_index < left_bytes.len() && right_index < right_bytes.len() {
        let left_digit = left_bytes[left_index].is_ascii_digit();
        let right_digit = right_bytes[right_index].is_ascii_digit();
        if left_digit && right_digit {
            let left_end = digit_end(left_bytes, left_index);
            let right_end = digit_end(right_bytes, right_index);
            let left_number = trim_leading_zeroes(&left_bytes[left_index..left_end]);
            let right_number = trim_leading_zeroes(&right_bytes[right_index..right_end]);
            match left_number
                .len()
                .cmp(&right_number.len())
                .then_with(|| left_number.cmp(right_number))
            {
                Ordering::Equal => {
                    left_index = left_end;
                    right_index = right_end;
                }
                ordering => return ordering,
            }
        } else {
            match left_bytes[left_index].cmp(&right_bytes[right_index]) {
                Ordering::Equal => {
                    left_index += 1;
                    right_index += 1;
                }
                ordering => return ordering,
            }
        }
    }
    left_bytes
        .len()
        .cmp(&right_bytes.len())
        .then_with(|| left.cmp(&right))
        .then_with(|| left_original.cmp(right_original))
}

fn digit_end(value: &[u8], start: usize) -> usize {
    value[start..]
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .map_or(value.len(), |offset| start + offset)
}

fn trim_leading_zeroes(value: &[u8]) -> &[u8] {
    let first_nonzero = value
        .iter()
        .position(|byte| *byte != b'0')
        .unwrap_or(value.len().saturating_sub(1));
    &value[first_nonzero..]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MusicOrder {
    Name,
    Shuffle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MusicRepeat {
    All,
    One,
}

pub trait PlaylistRng {
    fn next_u64(&mut self) -> u64;
}

#[derive(Clone, Copy, Debug)]
pub struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    pub fn seeded(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }
}

impl PlaylistRng for XorShift64 {
    fn next_u64(&mut self) -> u64 {
        let mut value = self.state;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.state = value;
        value
    }
}

// Swift: LocalMusicPlaylist
pub struct Playlist<R: PlaylistRng> {
    tracks: Vec<MusicTrack>,
    indices: Vec<usize>,
    position: Option<usize>,
    order: MusicOrder,
    repeat: MusicRepeat,
    rng: R,
    consecutive_failures: usize,
    exhausted: bool,
}

impl<R: PlaylistRng> Playlist<R> {
    pub fn new(
        mut tracks: Vec<MusicTrack>,
        order: MusicOrder,
        repeat: MusicRepeat,
        rng: R,
    ) -> Self {
        tracks.sort_by(|left, right| natural_path_cmp(&left.relative_path, &right.relative_path));
        let mut playlist = Self {
            indices: (0..tracks.len()).collect(),
            position: (!tracks.is_empty()).then_some(0),
            tracks,
            order,
            repeat,
            rng,
            consecutive_failures: 0,
            exhausted: false,
        };
        if order == MusicOrder::Shuffle {
            playlist.shuffle_indices();
        }
        playlist
    }

    pub fn current(&self) -> Option<&MusicTrack> {
        self.position
            .and_then(|position| self.indices.get(position))
            .and_then(|index| self.tracks.get(*index))
    }

    pub fn advance(&mut self, user_skip: bool) -> Option<&MusicTrack> {
        let Some(position) = self.position else {
            return None;
        };
        if self.repeat == MusicRepeat::One && !user_skip {
            return self.current();
        }
        if position + 1 < self.indices.len() {
            self.position = Some(position + 1);
            return self.current();
        }
        let previous = self.indices[position];
        if self.order == MusicOrder::Shuffle {
            self.shuffle_indices();
            if self.indices.len() > 1 && self.indices.first() == Some(&previous) {
                self.indices.swap(0, 1);
            }
        }
        self.position = Some(0);
        self.current()
    }

    pub fn report_failure(&mut self) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        self.exhausted = !self.tracks.is_empty() && self.consecutive_failures >= self.tracks.len();
    }

    pub fn report_success(&mut self) {
        self.consecutive_failures = 0;
        self.exhausted = false;
    }

    pub fn consecutive_failures(&self) -> usize {
        self.consecutive_failures
    }

    pub fn exhausted(&self) -> bool {
        self.exhausted
    }

    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }

    fn shuffle_indices(&mut self) {
        for upper in (1..self.indices.len()).rev() {
            let selected = (self.rng.next_u64() as usize) % (upper + 1);
            self.indices.swap(upper, selected);
        }
    }
}

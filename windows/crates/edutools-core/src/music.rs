//! Break-music URL validation and end-date-driven linear fading.

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

//! Command-line contract of `annota` (capability `app-shell`).

/// Capture modes accepted by `--capture`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    Region,
    Window,
    Screen,
}

impl CaptureMode {
    pub const VALID: &'static str = "region, window, screen";

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "region" => Some(Self::Region),
            "window" => Some(Self::Window),
            "screen" => Some(Self::Screen),
            _ => None,
        }
    }
}

/// Allowed range for `--delay` in seconds.
pub const DELAY_RANGE: std::ops::RangeInclusive<i32> = 0..=60;

/// Exit status for invalid command-line usage.
pub const EXIT_USAGE: i32 = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_modes_parse() {
        assert_eq!(CaptureMode::parse("region"), Some(CaptureMode::Region));
        assert_eq!(CaptureMode::parse("window"), Some(CaptureMode::Window));
        assert_eq!(CaptureMode::parse("screen"), Some(CaptureMode::Screen));
    }

    #[test]
    fn invalid_mode_is_rejected() {
        assert_eq!(CaptureMode::parse("banana"), None);
        assert_eq!(CaptureMode::parse(""), None);
    }

    #[test]
    fn delay_limits() {
        assert!(DELAY_RANGE.contains(&0));
        assert!(DELAY_RANGE.contains(&60));
        assert!(!DELAY_RANGE.contains(&61));
        assert!(!DELAY_RANGE.contains(&-1));
    }
}

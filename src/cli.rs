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

    pub const ALL: [CaptureMode; 3] = [Self::Region, Self::Window, Self::Screen];

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "region" => Some(Self::Region),
            "window" => Some(Self::Window),
            "screen" => Some(Self::Screen),
            _ => None,
        }
    }

    /// The command-line spelling, inverse of [`CaptureMode::parse`].
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Region => "region",
            Self::Window => "window",
            Self::Screen => "screen",
        }
    }

    /// Index in the GSettings enum `io.github.trosinde.Annota.CaptureMode`.
    pub fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Window,
            2 => Self::Screen,
            _ => Self::Region,
        }
    }

    pub fn index(self) -> i32 {
        match self {
            Self::Region => 0,
            Self::Window => 1,
            Self::Screen => 2,
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
    fn modes_round_trip() {
        for mode in CaptureMode::ALL {
            assert_eq!(CaptureMode::parse(mode.as_str()), Some(mode));
            assert_eq!(CaptureMode::from_index(mode.index()), mode);
        }
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

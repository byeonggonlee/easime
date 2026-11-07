//! Key repeat timing.
//!
//! **OS-specific: Wayland.** An input method here gets the keys from a grab and has
//! to repeat them itself (or let the compositor do it). On Windows and macOS the OS
//! repeats keys before the input method sees them, so this is expected not to be
//! needed there (not yet verified).

use std::time::Duration;

/// Repeat parameters from `wl_keyboard.repeat_info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepeatInfo {
    /// Repeats per second. Zero means "the compositor repeats for you".
    pub rate: i32,
    /// Milliseconds a key must be held before repeating starts.
    pub delay: i32,
}

impl Default for RepeatInfo {
    /// The usual desktop default, used until the compositor tells us otherwise.
    fn default() -> Self {
        Self {
            rate: 25,
            delay: 600,
        }
    }
}

impl RepeatInfo {
    /// `(initial delay, interval)` for our own timer, or `None` when we must
    /// not repeat ourselves (rate 0, or nonsensical values).
    pub fn timings(self) -> Option<(Duration, Duration)> {
        if self.rate <= 0 || self.delay < 0 {
            return None;
        }
        let interval = Duration::from_secs_f64(1.0 / f64::from(self.rate));
        Some((Duration::from_millis(self.delay as u64), interval))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_delay_and_interval() {
        let (delay, interval) = RepeatInfo {
            rate: 25,
            delay: 600,
        }
        .timings()
        .unwrap();
        assert_eq!(delay, Duration::from_millis(600));
        assert_eq!(interval, Duration::from_millis(40));
    }

    #[test]
    fn rate_zero_means_compositor_repeats() {
        assert_eq!(
            RepeatInfo {
                rate: 0,
                delay: 600
            }
            .timings(),
            None
        );
    }

    #[test]
    fn rejects_negative_values() {
        assert_eq!(
            RepeatInfo {
                rate: -1,
                delay: 600
            }
            .timings(),
            None
        );
        assert_eq!(
            RepeatInfo {
                rate: 25,
                delay: -1
            }
            .timings(),
            None
        );
    }

    #[test]
    fn zero_delay_is_allowed() {
        let (delay, _) = RepeatInfo { rate: 30, delay: 0 }.timings().unwrap();
        assert_eq!(delay, Duration::ZERO);
    }
}

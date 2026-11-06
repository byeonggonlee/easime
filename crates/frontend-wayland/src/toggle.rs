//! Language toggle key logic.
//!
//! * The Hangul key toggles as soon as it is pressed.
//! * Right Alt toggles only when it is *tapped* (pressed and released with no
//!   other key in between). If another key is pressed while it is held, it was
//!   being used as a modifier, so the held-back press is handed back to be
//!   forwarded to the application and the toggle is cancelled.

use crate::translate::{KEY_ALT_R, KEY_HANGUL};

/// A key press that was held back and may have to be forwarded later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingPress {
    pub serial: u32,
    pub time: u32,
    pub key: u32,
}

/// What the caller should do with the key event it just reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Not a toggle key: process it normally.
    Pass,
    /// Hold the event back; nothing to do now.
    Swallow,
    /// Switch language and consume the event.
    Toggle,
}

#[derive(Debug, Default)]
pub struct ToggleTracker {
    pending: Option<PendingPress>,
}

impl ToggleTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Report a key press. If a held-back press is returned, forward it to the
    /// application *before* acting on the decision.
    pub fn on_press(
        &mut self,
        serial: u32,
        time: u32,
        key: u32,
        keysym: u32,
    ) -> (Option<PendingPress>, Decision) {
        match keysym {
            KEY_HANGUL => (self.pending.take(), Decision::Toggle),
            KEY_ALT_R if self.pending.is_none() => {
                self.pending = Some(PendingPress { serial, time, key });
                (None, Decision::Swallow)
            }
            KEY_ALT_R => (None, Decision::Swallow),
            _ => (self.pending.take(), Decision::Pass),
        }
    }

    /// Report a key release.
    pub fn on_release(&mut self, key: u32) -> Decision {
        match self.pending {
            Some(p) if p.key == key => {
                self.pending = None;
                Decision::Toggle
            }
            _ => Decision::Pass,
        }
    }

    /// True if `key` is a press currently held back.
    pub fn holds(&self, key: u32) -> bool {
        self.pending.is_some_and(|p| p.key == key)
    }

    /// Forget any held-back press (e.g. on focus change).
    pub fn clear(&mut self) {
        self.pending = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALT_R_CODE: u32 = 100;
    const KEY_A_CODE: u32 = 30;
    const KEYSYM_A: u32 = 0x61;

    #[test]
    fn hangul_key_toggles_on_press() {
        let mut t = ToggleTracker::new();
        assert_eq!(t.on_press(1, 10, 122, KEY_HANGUL), (None, Decision::Toggle));
    }

    #[test]
    fn tapping_right_alt_toggles_on_release() {
        let mut t = ToggleTracker::new();
        assert_eq!(
            t.on_press(1, 10, ALT_R_CODE, KEY_ALT_R),
            (None, Decision::Swallow)
        );
        assert!(t.holds(ALT_R_CODE));
        assert_eq!(t.on_release(ALT_R_CODE), Decision::Toggle);
        assert!(!t.holds(ALT_R_CODE));
    }

    #[test]
    fn right_alt_used_as_modifier_is_forwarded_not_toggled() {
        let mut t = ToggleTracker::new();
        t.on_press(1, 10, ALT_R_CODE, KEY_ALT_R);
        let (pending, decision) = t.on_press(2, 20, KEY_A_CODE, KEYSYM_A);
        assert_eq!(
            pending,
            Some(PendingPress {
                serial: 1,
                time: 10,
                key: ALT_R_CODE
            })
        );
        assert_eq!(decision, Decision::Pass);
        // The Alt release is now an ordinary release to forward.
        assert_eq!(t.on_release(ALT_R_CODE), Decision::Pass);
    }

    #[test]
    fn ordinary_keys_pass() {
        let mut t = ToggleTracker::new();
        assert_eq!(
            t.on_press(1, 10, KEY_A_CODE, KEYSYM_A),
            (None, Decision::Pass)
        );
        assert_eq!(t.on_release(KEY_A_CODE), Decision::Pass);
    }

    #[test]
    fn altgr_is_not_a_toggle() {
        let mut t = ToggleTracker::new();
        // ISO_Level3_Shift
        assert_eq!(t.on_press(1, 10, 100, 0xfe03), (None, Decision::Pass));
    }

    #[test]
    fn clear_forgets_pending() {
        let mut t = ToggleTracker::new();
        t.on_press(1, 10, ALT_R_CODE, KEY_ALT_R);
        t.clear();
        assert_eq!(t.on_release(ALT_R_CODE), Decision::Pass);
    }
}

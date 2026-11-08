//! Which presses of the switch key switch language.
//!
//! * The Hangul key switches as soon as it is pressed.
//! * Right Alt switches only when it is *tapped* (pressed and released with no
//!   other key in between). If another key is pressed while it is held, it was
//!   being used as a modifier, so the held-back press is handed back to be
//!   forwarded to the application and the switch is cancelled.
//!
//! The rules are the same on every OS. What differs is how a frontend *knows*
//! which key it is looking at (a keysym on Wayland, a virtual-key code on
//! Windows, a key code on macOS), so the frontend classifies each press as a
//! [`KeyRole`] and the tracker works with that. A held-back press is returned
//! with whatever the frontend needs to forward it later (the `token`: the
//! Wayland serial and timestamp, for example).

use easime_config::ToggleKey;

/// What a key is, as far as switching language is concerned. Decided by the
/// frontend from its own key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRole {
    /// The dedicated Hangul (Korean/English) key.
    HangulKey,
    /// The right Alt key. Not AltGr: on layouts where right Alt makes a third
    /// level (AltGr) the frontend reports `Other`.
    RightAlt,
    /// Any other key.
    Other,
}

/// A key press that was held back and may have to be forwarded later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Held<K, T> {
    pub key: K,
    pub token: T,
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

/// Tracks the switch key. `K` identifies a physical key so that its release can
/// be matched with its press; `T` is the frontend's own data for forwarding a
/// held-back press.
#[derive(Debug)]
pub struct ToggleTracker<K, T> {
    setting: ToggleKey,
    pending: Option<Held<K, T>>,
}

impl<K, T> Default for ToggleTracker<K, T> {
    fn default() -> Self {
        Self {
            setting: ToggleKey::default(),
            pending: None,
        }
    }
}

impl<K: Copy + PartialEq, T: Copy> ToggleTracker<K, T> {
    /// Both switch keys enabled (the default setting).
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_setting(setting: ToggleKey) -> Self {
        Self {
            setting,
            pending: None,
        }
    }

    /// Report a key press. If a held-back press is returned, forward it to the
    /// application *before* acting on the decision.
    pub fn on_press(&mut self, key: K, role: KeyRole, token: T) -> (Option<Held<K, T>>, Decision) {
        match role {
            KeyRole::HangulKey if self.setting.uses_hangul_key() => {
                (self.pending.take(), Decision::Toggle)
            }
            KeyRole::RightAlt if self.setting.uses_right_alt() => {
                if self.pending.is_none() {
                    self.pending = Some(Held { key, token });
                }
                (None, Decision::Swallow)
            }
            _ => (self.pending.take(), Decision::Pass),
        }
    }

    /// Report a key release.
    pub fn on_release(&mut self, key: K) -> Decision {
        match self.pending {
            Some(p) if p.key == key => {
                self.pending = None;
                Decision::Toggle
            }
            _ => Decision::Pass,
        }
    }

    /// True if `key` is a press currently held back.
    pub fn holds(&self, key: K) -> bool {
        self.pending.is_some_and(|p| p.key == key)
    }

    /// Take any held-back press and forget it.
    pub fn take_pending(&mut self) -> Option<Held<K, T>> {
        self.pending.take()
    }

    /// Update the switch-key setting without losing a held-back press.
    pub fn set_setting(&mut self, setting: ToggleKey) {
        self.setting = setting;
    }

    /// Forget any held-back press (e.g. on focus change).
    pub fn clear(&mut self) {
        self.pending = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Key identity and token as a frontend might choose them.
    type Tracker = ToggleTracker<u32, (u32, u32)>;

    const ALT_R: u32 = 100;
    const HANGUL: u32 = 122;
    const A: u32 = 30;

    #[test]
    fn hangul_key_toggles_on_press() {
        let mut t = Tracker::new();
        assert_eq!(
            t.on_press(HANGUL, KeyRole::HangulKey, (1, 10)),
            (None, Decision::Toggle)
        );
    }

    #[test]
    fn tapping_right_alt_toggles_on_release() {
        let mut t = Tracker::new();
        assert_eq!(
            t.on_press(ALT_R, KeyRole::RightAlt, (1, 10)),
            (None, Decision::Swallow)
        );
        assert!(t.holds(ALT_R));
        assert_eq!(t.on_release(ALT_R), Decision::Toggle);
        assert!(!t.holds(ALT_R));
    }

    #[test]
    fn right_alt_used_as_modifier_is_forwarded_not_toggled() {
        let mut t = Tracker::new();
        t.on_press(ALT_R, KeyRole::RightAlt, (1, 10));
        let (held, decision) = t.on_press(A, KeyRole::Other, (2, 20));
        assert_eq!(
            held,
            Some(Held {
                key: ALT_R,
                token: (1, 10)
            })
        );
        assert_eq!(decision, Decision::Pass);
        // The Alt release is now an ordinary release to forward.
        assert_eq!(t.on_release(ALT_R), Decision::Pass);
    }

    #[test]
    fn ordinary_keys_pass() {
        let mut t = Tracker::new();
        assert_eq!(
            t.on_press(A, KeyRole::Other, (1, 10)),
            (None, Decision::Pass)
        );
        assert_eq!(t.on_release(A), Decision::Pass);
    }

    #[test]
    fn hangul_only_setting_leaves_right_alt_alone() {
        let mut t = Tracker::with_setting(ToggleKey::Hangul);
        assert_eq!(
            t.on_press(ALT_R, KeyRole::RightAlt, (1, 10)),
            (None, Decision::Pass)
        );
        assert_eq!(t.on_release(ALT_R), Decision::Pass);
        assert_eq!(
            t.on_press(HANGUL, KeyRole::HangulKey, (2, 20)),
            (None, Decision::Toggle)
        );
    }

    #[test]
    fn right_alt_only_setting_leaves_hangul_key_alone() {
        let mut t = Tracker::with_setting(ToggleKey::RightAlt);
        assert_eq!(
            t.on_press(HANGUL, KeyRole::HangulKey, (1, 10)),
            (None, Decision::Pass)
        );
        assert_eq!(
            t.on_press(ALT_R, KeyRole::RightAlt, (2, 20)),
            (None, Decision::Swallow)
        );
        assert_eq!(t.on_release(ALT_R), Decision::Toggle);
    }

    #[test]
    fn pressing_right_alt_again_while_held_keeps_the_first_press() {
        // Some systems repeat a held modifier; the first press is the one that
        // may have to be forwarded.
        let mut t = Tracker::new();
        t.on_press(ALT_R, KeyRole::RightAlt, (1, 10));
        t.on_press(ALT_R, KeyRole::RightAlt, (3, 30));
        let (held, _) = t.on_press(A, KeyRole::Other, (4, 40));
        assert_eq!(held.map(|h| h.token), Some((1, 10)));
    }

    #[test]
    fn clear_forgets_pending() {
        let mut t = Tracker::new();
        t.on_press(ALT_R, KeyRole::RightAlt, (1, 10));
        t.clear();
        assert_eq!(t.on_release(ALT_R), Decision::Pass);
    }

    #[test]
    fn the_key_and_token_types_belong_to_the_frontend() {
        // A frontend with string key names and no token at all.
        let mut t: ToggleTracker<&str, ()> = ToggleTracker::new();
        t.on_press("AltRight", KeyRole::RightAlt, ());
        assert!(t.holds("AltRight"));
        assert_eq!(t.on_release("AltRight"), Decision::Toggle);
    }
}

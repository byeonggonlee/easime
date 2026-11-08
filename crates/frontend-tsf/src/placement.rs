//! Where the small mode box goes, from the rectangle of the text cursor. Plain
//! numbers, so every OS tests it, although only Windows uses it today.
//!
//! The box goes just below the cursor (like the Linux box, which KWin places
//! under the cursor rectangle). Where there is no room below, it goes above,
//! and it never leaves the screen's work area. It must not cover the character
//! being typed, so it never overlaps the cursor rectangle unless the screen is
//! too small for anything else.

/// A rectangle in screen coordinates: `right` and `bottom` are one past the last pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(self) -> i32 {
        self.right - self.left
    }

    pub fn height(self) -> i32 {
        self.bottom - self.top
    }
}

/// The gap between the cursor and the box.
const GAP: i32 = 2;

/// The top-left corner of a `size` by `size` box for the cursor rectangle
/// `caret`, inside `work` (the usable area of the monitor the cursor is on).
pub fn box_position(caret: Rect, size: i32, work: Rect) -> (i32, i32) {
    window_position(caret, size, size, work)
}

/// The top-left corner of a window of `width` × `height` for the text cursor at
/// `caret`, by the same rule as the box: just below the cursor, above it when
/// there is no room below, and always on the screen (`work`).
pub fn window_position(caret: Rect, width: i32, height: i32, work: Rect) -> (i32, i32) {
    let below = caret.bottom + GAP;
    let above = caret.top - GAP - height;
    let top = if below + height <= work.bottom {
        below
    } else if above >= work.top {
        above
    } else {
        let room_below = work.bottom - caret.bottom;
        let room_above = caret.top - work.top;
        if room_below >= room_above {
            work.bottom - height
        } else {
            work.top
        }
    };
    let left = caret.left.min(work.right - width).max(work.left);
    (
        left,
        top.clamp(work.top, (work.bottom - height).max(work.top)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_window_keeps_its_left_on_the_screen_and_goes_above_when_it_must() {
        let screen = Rect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        let near_right = Rect {
            left: 1800,
            top: 100,
            right: 1802,
            bottom: 120,
        };
        assert_eq!(window_position(near_right, 400, 300, screen), (1520, 122));
        let near_bottom = Rect {
            left: 100,
            top: 900,
            right: 102,
            bottom: 920,
        };
        assert_eq!(window_position(near_bottom, 400, 300, screen), (100, 598));
    }

    const SCREEN: Rect = Rect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1040,
    };

    fn caret(left: i32, top: i32) -> Rect {
        Rect {
            left,
            top,
            right: left + 2,
            bottom: top + 20,
        }
    }

    #[test]
    fn the_box_goes_just_below_the_cursor() {
        assert_eq!(box_position(caret(300, 400), 48, SCREEN), (300, 422));
    }

    #[test]
    fn near_the_bottom_it_goes_above_and_does_not_cover_the_cursor() {
        let (x, y) = box_position(caret(300, 1000), 48, SCREEN);
        assert_eq!(x, 300);
        assert_eq!(y, 1000 - 2 - 48);
        assert!(y + 48 <= 1000, "ends above the cursor");
    }

    #[test]
    fn it_stays_inside_the_screen_on_the_right_and_left() {
        assert_eq!(box_position(caret(1915, 100), 48, SCREEN).0, 1920 - 48);
        assert_eq!(box_position(caret(-30, 100), 48, SCREEN).0, 0);
    }

    #[test]
    fn a_work_area_that_does_not_start_at_zero_is_respected() {
        let second = Rect {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1080,
        };
        let (x, y) = box_position(caret(-100, 500), 48, second);
        assert_eq!((x, y), (-100, 522));
        assert!(x >= second.left && x + 48 <= second.right);
    }

    #[test]
    fn a_tiny_screen_still_gets_a_position_on_it() {
        let tiny = Rect {
            left: 0,
            top: 0,
            right: 100,
            bottom: 60,
        };
        let (x, y) = box_position(caret(50, 20), 48, tiny);
        assert!(x >= 0 && x + 48 <= 100 && y >= 0 && y + 48 <= 60, "{x},{y}");
    }

    #[test]
    fn rect_sizes() {
        assert_eq!((caret(5, 5).width(), caret(5, 5).height()), (2, 20));
    }
}

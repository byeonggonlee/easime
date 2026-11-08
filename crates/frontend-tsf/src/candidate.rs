//! The candidate window (Windows): the page of [`crate::page`] next to the text
//! cursor while a conversion (Hanja, kanji) is going on.
//!
//! Like the mode box ([`crate::indicator`]) it is a window of the
//! application's own thread that never takes the focus. It is drawn with GDI
//! in the dark colours of the Linux window, with the Windows font of the
//! language being converted, because the same Han character has a different
//! shape in Japanese and in Korean. The chosen candidate is marked by a sign
//! (`>`) as well as its background. Its size follows the screen's scale; a
//! long candidate makes it wider up to a limit, past which the text ends in an
//! ellipsis, so what is cut off is seen to be.

use crate::indicator::{dpi, face_name, own_module, work_area_of};
use crate::page::Page;
use crate::placement::{self, Rect};
use ime_core::Language;
use std::cell::RefCell;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateSolidBrush,
    DEFAULT_CHARSET, DT_END_ELLIPSIS, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteObject,
    DrawTextW, EndPaint, FF_DONTCARE, FW_NORMAL, FillRect, GetDC, GetTextExtentPoint32W, HDC,
    HFONT, HGDIOBJ, InvalidateRect, OUT_DEFAULT_PRECIS, PAINTSTRUCT, ReleaseDC, SelectObject,
    SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetForegroundWindow,
    GetWindowRect, HWND_TOPMOST, MA_NOACTIVATE, RegisterClassExW, SW_HIDE, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, SetWindowPos, ShowWindow, UnregisterClassW, WM_MOUSEACTIVATE, WM_PAINT,
    WNDCLASSEXW, WS_BORDER, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, w};

/// The window class, which the tests look for.
pub const CLASS_NAME: &str = "EasimeCandidates";

// Sizes at 96 dpi, as in the Linux window.
const MIN_WIDTH: i32 = 400;
const MAX_WIDTH: i32 = 960;
const MARGIN: i32 = 12;
const HEADER: i32 = 34;
const ROW: i32 = 34;
const FOOTER: i32 = 30;
/// Where the candidate starts, after the number.
const CANDIDATE_LEFT: i32 = 52;
const TEXT_SIZE: i32 = 17;
const CANDIDATE_SIZE: i32 = 23;
const FOOTER_SIZE: i32 = 14;

const BACKGROUND: COLORREF = COLORREF(0x0026_2220);
const TEXT: COLORREF = COLORREF(0x00F2_F2F2);
const SELECTED: COLORREF = COLORREF(0x0075_5535);

/// What the window procedure draws.
struct Shown {
    page: Page,
    language: Language,
    scale: f32,
}

thread_local! {
    static SHOWN: RefCell<Option<Shown>> = const { RefCell::new(None) };
}

fn scaled(value: i32, scale: f32) -> i32 {
    (value as f32 * scale).round() as i32
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// A font of the language at `height` pixels. Deleted by the caller.
fn font(language: Language, height: i32) -> HFONT {
    // SAFETY: plain numbers and a static name.
    unsafe {
        CreateFontW(
            -height,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            FF_DONTCARE.0 as u32,
            face_name(language),
        )
    }
}

/// The width of `text` in `font` on `dc`.
fn measure(dc: HDC, font: HFONT, text: &str) -> i32 {
    let mut size = SIZE::default();
    // SAFETY: a live device context and font; the text lives for the call.
    unsafe {
        let previous = SelectObject(dc, HGDIOBJ(font.0));
        let _ = GetTextExtentPoint32W(dc, &wide(text), &mut size);
        SelectObject(dc, previous);
    }
    size.cx
}

/// Draw `text` in `area`, on one line, cut with an ellipsis if it is too long.
fn draw(dc: HDC, font: HFONT, text: &str, mut area: RECT) {
    let mut text = wide(text);
    // SAFETY: a live device context and font; the text and area live for the call.
    unsafe {
        let previous = SelectObject(dc, HGDIOBJ(font.0));
        DrawTextW(
            dc,
            &mut text,
            &mut area,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        SelectObject(dc, previous);
    }
}

fn paint(dc: HDC, window: HWND, shown: &Shown) {
    let s = |value: i32| scaled(value, shown.scale);
    let mut client = RECT::default();
    // SAFETY: our own window and device context.
    unsafe {
        let _ = GetClientRect(window, &mut client);
        let background = CreateSolidBrush(BACKGROUND);
        FillRect(dc, &client, background);
        let _ = DeleteObject(HGDIOBJ(background.0));
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, TEXT);
    }
    let width = client.right;
    let text_font = font(shown.language, s(TEXT_SIZE));
    let candidate_font = font(shown.language, s(CANDIDATE_SIZE));
    let footer_font = font(shown.language, s(FOOTER_SIZE));
    let line = |top: i32, height: i32, left: i32, right: i32| RECT {
        left,
        top,
        right,
        bottom: top + height,
    };

    draw(
        dc,
        text_font,
        &shown.page.reading,
        line(0, s(HEADER), s(MARGIN), width - s(MARGIN)),
    );
    for (index, row) in shown.page.rows.iter().enumerate() {
        let top = s(HEADER) + index as i32 * s(ROW);
        if row.selected {
            // SAFETY: as above.
            unsafe {
                let brush = CreateSolidBrush(SELECTED);
                FillRect(dc, &line(top, s(ROW), s(4), width - s(4)), brush);
                let _ = DeleteObject(HGDIOBJ(brush.0));
            }
        }
        draw(
            dc,
            text_font,
            &row.label(),
            line(top, s(ROW), s(10), s(CANDIDATE_LEFT)),
        );
        let candidate_right = match &row.annotation {
            None => width - s(MARGIN),
            Some(_) => {
                (s(CANDIDATE_LEFT) + measure(dc, candidate_font, &row.text)).min(width - s(MARGIN))
            }
        };
        draw(
            dc,
            candidate_font,
            &row.text,
            line(top, s(ROW), s(CANDIDATE_LEFT), candidate_right),
        );
        if let Some(annotation) = &row.annotation {
            let left = (candidate_right + s(MARGIN)).max(s(100));
            draw(
                dc,
                text_font,
                annotation,
                line(top, s(ROW), left, width - s(MARGIN)),
            );
        }
    }
    let footer_top = s(HEADER) + shown.page.rows.len() as i32 * s(ROW);
    draw(
        dc,
        footer_font,
        &shown.page.footer,
        line(footer_top, s(FOOTER), s(MARGIN), width - s(MARGIN)),
    );
    for font in [text_font, candidate_font, footer_font] {
        // SAFETY: fonts made above, no longer selected.
        unsafe {
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
    }
}

unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            let mut paint_info = PAINTSTRUCT::default();
            // SAFETY: the standard paint sequence on this window.
            unsafe {
                let dc = BeginPaint(window, &mut paint_info);
                SHOWN.with(|shown| {
                    if let Some(shown) = shown.borrow().as_ref() {
                        paint(dc, window, shown);
                    }
                });
                let _ = EndPaint(window, &paint_info);
            }
            LRESULT(0)
        }
        // A click must not take the focus from the text being converted.
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        // SAFETY: the default processing of a window of ours.
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

/// Somewhere to put the window when the application says nothing about its
/// cursor: near the top left of the window in front, so the candidates are
/// seen rather than chosen blind.
pub fn fallback_anchor() -> Option<Rect> {
    let mut area = RECT::default();
    // SAFETY: plain queries.
    unsafe {
        let window = GetForegroundWindow();
        if window.is_invalid() {
            return None;
        }
        GetWindowRect(window, &mut area).ok()?;
    }
    Some(Rect {
        left: area.left + 16,
        top: area.top + 16,
        right: area.left + 18,
        bottom: area.top + 36,
    })
}

/// The candidate window of one thread.
#[derive(Default)]
pub struct CandidateWindow {
    window: Option<HWND>,
}

impl CandidateWindow {
    /// Show `page` (candidates of `language`) next to `caret`. Returns where
    /// the window went, or `None` if it could not be shown.
    pub fn show(&mut self, page: &Page, language: Language, caret: Rect) -> Option<Rect> {
        let window = self.window()?;
        let scale = dpi() as f32 / 96.0;
        let s = |value: i32| scaled(value, scale);

        // As wide as the widest line needs, within the limits.
        // SAFETY: the window's own device context, released below.
        let dc = unsafe { GetDC(Some(window)) };
        let (text_font, candidate_font, footer_font) = (
            font(language, s(TEXT_SIZE)),
            font(language, s(CANDIDATE_SIZE)),
            font(language, s(FOOTER_SIZE)),
        );
        let mut widest =
            measure(dc, text_font, &page.reading).max(measure(dc, footer_font, &page.footer));
        for row in &page.rows {
            let mut needed = s(CANDIDATE_LEFT) + measure(dc, candidate_font, &row.text);
            if let Some(annotation) = &row.annotation {
                needed = needed.max(s(100) - s(MARGIN)) + s(MARGIN);
                needed += measure(dc, text_font, annotation);
            }
            widest = widest.max(needed);
        }
        // SAFETY: fonts made above; the device context taken above.
        unsafe {
            for font in [text_font, candidate_font, footer_font] {
                let _ = DeleteObject(HGDIOBJ(font.0));
            }
            ReleaseDC(Some(window), dc);
        }
        let width = (widest + 2 * s(MARGIN)).clamp(s(MIN_WIDTH), s(MAX_WIDTH));
        let height = s(HEADER) + page.rows.len() as i32 * s(ROW) + s(FOOTER);

        SHOWN.with(|shown| {
            *shown.borrow_mut() = Some(Shown {
                page: page.clone(),
                language,
                scale,
            });
        });
        let (x, y) = placement::window_position(caret, width, height, work_area_of(caret));
        // SAFETY: our own window; plain numbers.
        unsafe {
            SetWindowPos(
                window,
                Some(HWND_TOPMOST),
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
            .ok()?;
            // Repaint with the new page even if the window was showing.
            let _ = InvalidateRect(Some(window), None, true);
        }
        Some(Rect {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        })
    }

    /// Hide it (the conversion is over).
    pub fn hide(&mut self) {
        if let Some(window) = self.window {
            // SAFETY: our own window.
            unsafe {
                let _ = ShowWindow(window, SW_HIDE);
            }
        }
        SHOWN.with(|shown| shown.borrow_mut().take());
    }

    /// Destroy the window on the thread that made it (TSF `Deactivate`, drop).
    pub fn destroy(&mut self) {
        if let Some(window) = self.window.take() {
            // SAFETY: our own window, on the thread that made it.
            unsafe {
                let _ = DestroyWindow(window);
            }
        }
        SHOWN.with(|shown| shown.borrow_mut().take());
    }

    fn window(&mut self) -> Option<HWND> {
        if self.window.is_none() {
            self.window = create_window();
        }
        self.window
    }
}

impl Drop for CandidateWindow {
    fn drop(&mut self) {
        self.destroy();
        // As for the mode box: the class points into this DLL; it fails while
        // another thread's window still uses it, and that thread takes it then.
        let class: Vec<u16> = CLASS_NAME.encode_utf16().chain(Some(0)).collect();
        if let Some(instance) = own_module() {
            // SAFETY: the class name lives for the call.
            unsafe {
                let _ = UnregisterClassW(PCWSTR(class.as_ptr()), Some(instance.into()));
            }
        }
    }
}

fn create_window() -> Option<HWND> {
    let class: Vec<u16> = CLASS_NAME.encode_utf16().chain(Some(0)).collect();
    // SAFETY: registering and creating a window of our own class; the strings
    // live for the calls. Registering twice fails harmlessly: the class exists.
    unsafe {
        let instance = own_module()?;
        let description = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(procedure),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class.as_ptr()),
            ..WNDCLASSEXW::default()
        };
        RegisterClassExW(&description);
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            PCWSTR(class.as_ptr()),
            w!("easime"),
            WS_POPUP | WS_BORDER,
            0,
            0,
            MIN_WIDTH,
            HEADER + ROW + FOOTER,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .ok()
    }
}

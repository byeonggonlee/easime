//! The small box that says which language is on, next to the text cursor, for a
//! moment after the user switches (the same decision as on Linux: it is shown
//! only when the user switches, never because a text field got the focus, and it
//! never takes the focus).
//!
//! It is a window of the application's own thread, because that is where the
//! text service runs and where the application's coordinates (and its idea of
//! the screen's scale) are the ones the cursor rectangle is in. The window has
//! no activation, ignores the mouse, and is hidden by a timer; the application's
//! message loop runs both. Where the box goes is [`crate::placement`].

use crate::placement::{self, Rect};
use ime_core::Language;
use ime_session::indicator::language_glyph;
use std::cell::RefCell;
use windows::Win32::Foundation::{COLORREF, HMODULE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, ClientToScreen, CreateFontW, CreateRoundRectRgn, CreateSolidBrush, DT_CENTER,
    DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawTextW, EndPaint, FF_DONTCARE, FW_BOLD, FillRect,
    GET_DEVICE_CAPS_INDEX, GetDC, GetDeviceCaps, GetMonitorInfoW, HGDIOBJ,
    MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, PAINTSTRUCT, ReleaseDC, SelectObject,
    SetBkMode, SetTextColor, SetWindowRgn, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleHandleExW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GUITHREADINFO, GetGUIThreadInfo, HWND_TOPMOST,
    KillTimer, LWA_ALPHA, RegisterClassExW, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SetLayeredWindowAttributes, SetTimer, SetWindowPos, ShowWindow, UnregisterClassW, WM_PAINT,
    WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

/// The window class, which the tests look for.
pub const CLASS_NAME: &str = "EasimeIndicator";

/// The box's size at 96 dpi, as on Linux.
const SIZE: i32 = 48;
const BACKGROUND: COLORREF = COLORREF(0x0026_2220);
const FOREGROUND: COLORREF = COLORREF(0x00F2_F2F2);
/// Slightly see-through, like the Linux box (0xE0 of 0xFF).
const ALPHA: u8 = 0xE0;
const TIMER: usize = 1;

/// What the window procedure draws.
struct Face {
    glyph: String,
    language: Language,
    /// Pixel height of the glyph.
    glyph_height: i32,
}

thread_local! {
    static FACE: RefCell<Face> = const {
        RefCell::new(Face {
            glyph: String::new(),
            language: Language::English,
            glyph_height: 24,
        })
    };
}

/// A font that has the glyph of the language, by the names Windows ships with.
fn face_name(language: Language) -> PCWSTR {
    match language {
        Language::Korean => w!("Malgun Gothic"),
        Language::Japanese => w!("Yu Gothic UI"),
        Language::ChineseSimplified => w!("Microsoft YaHei UI"),
        Language::ChineseTraditional => w!("Microsoft JhengHei UI"),
        Language::English => w!("Segoe UI"),
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
            let mut paint = PAINTSTRUCT::default();
            // SAFETY: the standard paint sequence on this window.
            unsafe {
                let dc = BeginPaint(window, &mut paint);
                let brush = CreateSolidBrush(BACKGROUND);
                FillRect(dc, &paint.rcPaint, brush);
                let _ = DeleteObject(HGDIOBJ(brush.0));
                FACE.with(|face| {
                    let face = face.borrow();
                    let font = CreateFontW(
                        -face.glyph_height,
                        0,
                        0,
                        0,
                        FW_BOLD.0 as i32,
                        0,
                        0,
                        0,
                        windows::Win32::Graphics::Gdi::DEFAULT_CHARSET,
                        windows::Win32::Graphics::Gdi::OUT_DEFAULT_PRECIS,
                        windows::Win32::Graphics::Gdi::CLIP_DEFAULT_PRECIS,
                        windows::Win32::Graphics::Gdi::CLEARTYPE_QUALITY,
                        FF_DONTCARE.0 as u32,
                        face_name(face.language),
                    );
                    let previous = SelectObject(dc, HGDIOBJ(font.0));
                    SetBkMode(dc, TRANSPARENT);
                    SetTextColor(dc, FOREGROUND);
                    let mut text: Vec<u16> = face.glyph.encode_utf16().collect();
                    let mut area = RECT::default();
                    let _ =
                        windows::Win32::UI::WindowsAndMessaging::GetClientRect(window, &mut area);
                    DrawTextW(
                        dc,
                        &mut text,
                        &mut area,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );
                    SelectObject(dc, previous);
                    let _ = DeleteObject(HGDIOBJ(font.0));
                });
                let _ = EndPaint(window, &paint);
            }
            LRESULT(0)
        }
        WM_TIMER => {
            // SAFETY: this window's own timer.
            unsafe {
                let _ = KillTimer(Some(window), TIMER);
                let _ = ShowWindow(window, SW_HIDE);
            }
            LRESULT(0)
        }
        // SAFETY: the default processing of a window of ours.
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

/// The box of one thread.
#[derive(Default)]
pub struct Indicator {
    window: Option<HWND>,
}

impl Indicator {
    /// Show `language`'s glyph with its top-left near `caret`, for `milliseconds`.
    /// Returns where the box went, or `None` if no window could be made.
    pub fn show(&mut self, language: Language, caret: Rect, milliseconds: u32) -> Option<Rect> {
        let window = self.window()?;
        let scale = dpi() as f32 / 96.0;
        let size = (SIZE as f32 * scale).round() as i32;
        FACE.with(|face| {
            *face.borrow_mut() = Face {
                glyph: language_glyph(language).to_string(),
                language,
                glyph_height: (size as f32 * 0.52).round() as i32,
            };
        });
        let work = work_area_of(caret);
        let (x, y) = placement::box_position(caret, size, work);
        // SAFETY: our own window; plain numbers.
        unsafe {
            let corner = (size as f32 * 0.25).round() as i32;
            let _ = SetWindowRgn(
                window,
                Some(CreateRoundRectRgn(0, 0, size + 1, size + 1, corner, corner)),
                true,
            );
            let _ = SetWindowPos(
                window,
                Some(HWND_TOPMOST),
                x,
                y,
                size,
                size,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            // Repaint with the new glyph even if the box was still showing.
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(window), None, true);
            SetTimer(Some(window), TIMER, milliseconds.max(100), None);
        }
        Some(Rect {
            left: x,
            top: y,
            right: x + size,
            bottom: y + size,
        })
    }

    /// Hide it now.
    pub fn hide(&mut self) {
        if let Some(window) = self.window {
            // SAFETY: our own window.
            unsafe {
                let _ = KillTimer(Some(window), TIMER);
                let _ = ShowWindow(window, SW_HIDE);
            }
        }
    }

    fn window(&mut self) -> Option<HWND> {
        if self.window.is_none() {
            self.window = create_window();
        }
        self.window
    }
}

impl Drop for Indicator {
    fn drop(&mut self) {
        if let Some(window) = self.window.take() {
            // SAFETY: our own window, on the thread that made it (the service
            // is released on its thread).
            unsafe {
                let _ = DestroyWindow(window);
            }
        }
        // The class points into this DLL, which may be unloaded after the last
        // service goes: take it away. It fails while another thread's window
        // still uses it, and that thread's drop takes it then.
        let class = CLASS_NAME
            .encode_utf16()
            .chain(Some(0))
            .collect::<Vec<u16>>();
        if let Some(instance) = own_module() {
            // SAFETY: the class name lives for the call.
            unsafe {
                let _ = UnregisterClassW(PCWSTR(class.as_ptr()), Some(instance.into()));
            }
        }
    }
}

/// This DLL's own module handle (a handle found from an address in it).
fn own_module() -> Option<HMODULE> {
    let mut module = HMODULE::default();
    // SAFETY: an address inside this DLL; the reference count is left alone.
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(procedure as *const () as *const u16),
            &mut module,
        )
    }
    .ok()?;
    Some(module)
}

fn create_window() -> Option<HWND> {
    let class = CLASS_NAME
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<u16>>();
    // SAFETY: registering and creating a window of our own class; the strings
    // live for the calls. Registering twice fails harmlessly: the class exists.
    unsafe {
        // This DLL's own handle (its file name differs: the installer renames it).
        let instance = own_module()?;
        let description = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(procedure),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class.as_ptr()),
            ..WNDCLASSEXW::default()
        };
        RegisterClassExW(&description);
        let window = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT,
            PCWSTR(class.as_ptr()),
            w!("easime"),
            WS_POPUP,
            0,
            0,
            SIZE,
            SIZE,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .ok()?;
        let _ = SetLayeredWindowAttributes(window, COLORREF(0), ALPHA, LWA_ALPHA);
        Some(window)
    }
}

/// The screen's scale in dots per inch (96 is 100%), as this process sees it.
fn dpi() -> i32 {
    // SAFETY: the screen's device context, released at once.
    unsafe {
        let dc = GetDC(None);
        let dpi = GetDeviceCaps(Some(dc), GET_DEVICE_CAPS_INDEX(88)); // LOGPIXELSX
        ReleaseDC(None, dc);
        if dpi > 0 { dpi } else { 96 }
    }
}

/// The usable area of the monitor `caret` is on.
fn work_area_of(caret: Rect) -> Rect {
    // SAFETY: plain numbers and a struct we size ourselves.
    unsafe {
        let monitor = MonitorFromPoint(
            POINT {
                x: caret.left,
                y: caret.bottom,
            },
            MONITOR_DEFAULTTONEAREST,
        );
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..MONITORINFO::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            return Rect {
                left: info.rcWork.left,
                top: info.rcWork.top,
                right: info.rcWork.right,
                bottom: info.rcWork.bottom,
            };
        }
    }
    // No answer: a generous area, so only the cursor decides.
    Rect {
        left: i32::MIN / 2,
        top: i32::MIN / 2,
        right: i32::MAX / 2,
        bottom: i32::MAX / 2,
    }
}

/// The text cursor of the foreground window, when it keeps a system caret.
/// Many applications draw their own and have none; then there is no answer.
pub fn system_caret() -> Option<Rect> {
    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..GUITHREADINFO::default()
    };
    // SAFETY: the struct is sized and lives for the call.
    unsafe { GetGUIThreadInfo(0, &mut info) }.ok()?;
    if info.hwndCaret.0.is_null() {
        return None;
    }
    let mut corner = POINT {
        x: info.rcCaret.left,
        y: info.rcCaret.top,
    };
    let mut far = POINT {
        x: info.rcCaret.right,
        y: info.rcCaret.bottom,
    };
    // SAFETY: the window handle came from the system just now.
    unsafe {
        if !ClientToScreen(info.hwndCaret, &mut corner).as_bool()
            || !ClientToScreen(info.hwndCaret, &mut far).as_bool()
        {
            return None;
        }
    }
    Some(Rect {
        left: corner.x,
        top: corner.y,
        right: far.x.max(corner.x + 1),
        bottom: far.y.max(corner.y + 1),
    })
}

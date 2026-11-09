//! The mode button: easime's language, always visible in the notification area
//! of the taskbar, with a menu for the options.
//!
//! This is the usual place Windows users look to see what an input method is
//! doing (the Microsoft Korean input method shows its 가/A there). It is a TSF
//! *language bar item*: a button that TSF shows on the language bar and, because
//! of its style (`TF_LBI_STYLE_SHOWNINTRAY`) and the category the text service
//! registers (`GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT`, [`crate::registration`]), in
//! the tray. It is the "input mode" item (`GUID_LBI_INPUTMODE`).
//!
//! * The icon is the language's letter (`A`, `한`, `あ`) on a dark rounded
//!   badge, like the tray icon of the Linux daemon. The tooltip names the language.
//! * A left click goes to the next language; the menu (right click) lists the
//!   languages and opens the settings app.
//! * What a click or a menu choice does is the service's business
//!   ([`Request`]): the button only says what the user asked for.
//!
//! The mode is the mode of the program that has the focus (each program has its
//! own, see `docs/platforms.md`), so the button shows the language of the
//! program the user is typing in.
//!
//! Everything runs on the service's thread. No cell is borrowed while TSF is
//! called.

use crate::diag;
use crate::indicator::{dpi, face_name};
use crate::profiles::CLSID;
use crate::registration::guid;
use crate::service::Alive;
use ime_core::Language;
use ime_session::indicator::{language_glyph, language_native_name};
use std::cell::RefCell;
use std::ffi::c_void;
use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG, E_POINTER, HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    ANTIALIASED_QUALITY, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLIP_DEFAULT_PRECIS, CreateBitmap,
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DEFAULT_CHARSET, DEFAULT_PITCH,
    DIB_RGB_COLORS, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject,
    DrawTextW, FF_DONTCARE, FW_BOLD, GdiFlush, HBITMAP, HGDIOBJ, OUT_DEFAULT_PRECIS, SelectObject,
    SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::TextServices::{
    GUID_LBI_INPUTMODE, ITfLangBarItem, ITfLangBarItem_Impl, ITfLangBarItemButton,
    ITfLangBarItemButton_Impl, ITfLangBarItemMgr, ITfLangBarItemSink, ITfMenu, TF_LANGBARITEMINFO,
    TF_LBI_CLK_LEFT, TF_LBI_ICON, TF_LBI_STYLE_BTN_BUTTON, TF_LBI_STYLE_SHOWNINTRAY, TF_LBI_TEXT,
    TF_LBI_TOOLTIP, TF_LBMENUF_RADIOCHECKED, TF_LBMENUF_SEPARATOR, TfLBIClick,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, DestroyIcon, HICON, ICONINFO, SW_SHOWNORMAL,
};
use windows::core::{BOOL, BSTR, GUID, IUnknown, Interface, PCWSTR, Ref, Result, implement, w};

/// What the user asked of the service through the button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Go to the next language of the cycle (a left click).
    Next,
    /// Go to this language (a menu choice).
    Select(Language),
    /// Open the settings app (a menu choice).
    Settings,
}

/// The id of the menu entry that opens the settings; the languages count up from 1.
const SETTINGS_ID: u32 = 1000;

/// The cookie of the one sink TSF gives us.
const SINK_COOKIE: u32 = 1;

/// The button's face: what it shows and offers.
struct Face {
    language: Language,
    /// The languages of the cycle, English first.
    cycle: Vec<Language>,
}

/// The language bar item.
#[implement(ITfLangBarItemButton, windows::Win32::UI::TextServices::ITfSource)]
pub struct ModeButton {
    face: RefCell<Face>,
    /// TSF's sink, told when the icon or the tooltip changed.
    sink: RefCell<Option<ITfLangBarItemSink>>,
    /// Where requests go.
    handler: Box<dyn Fn(Request)>,
    /// The text of the "settings" menu entry, in the user's language.
    settings_text: String,
    _alive: Alive,
}

impl ModeButton {
    /// A button showing `language`, with the languages of the cycle.
    pub fn new(
        language: Language,
        cycle: Vec<Language>,
        settings_text: String,
        handler: Box<dyn Fn(Request)>,
    ) -> Self {
        Self {
            face: RefCell::new(Face { language, cycle }),
            sink: RefCell::new(None),
            handler,
            settings_text,
            _alive: Alive::new(),
        }
    }
}

impl ModeButton {
    /// The button shows `language` of `cycle` from now on; TSF is told, so the
    /// tray icon follows.
    pub fn set(&self, language: Language, cycle: Vec<Language>) {
        let changed = {
            let mut face = self.face.borrow_mut();
            let changed = face.language != language || face.cycle != cycle;
            face.language = language;
            face.cycle = cycle;
            changed
        };
        if !changed {
            return;
        }
        // Outside the borrow: TSF asks for the new icon during the call.
        let sink = self.sink.borrow().clone();
        if let Some(sink) = sink {
            // SAFETY: the sink is alive for the call.
            if let Err(e) = unsafe { sink.OnUpdate(TF_LBI_ICON | TF_LBI_TEXT | TF_LBI_TOOLTIP) } {
                diag::log(&format!(
                    "language bar: could not tell TSF of the change: {e}"
                ));
            }
        }
    }

    fn language(&self) -> Language {
        self.face.borrow().language
    }
}

/// `text` in a fixed-size UTF-16 field, cut to fit and ending in NUL.
fn field<const N: usize>(text: &str) -> [u16; N] {
    let mut out = [0u16; N];
    for (slot, unit) in out.iter_mut().take(N - 1).zip(text.encode_utf16()) {
        *slot = unit;
    }
    out
}

impl ITfLangBarItem_Impl for ModeButton_Impl {
    // The signature, raw pointer included, is COM's: TSF owns the structure.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn GetInfo(&self, info: *mut TF_LANGBARITEMINFO) -> Result<()> {
        if info.is_null() {
            return Err(E_POINTER.into());
        }
        // SAFETY: TSF passes a structure to fill.
        unsafe {
            *info = TF_LANGBARITEMINFO {
                clsidService: guid(CLSID),
                guidItem: GUID_LBI_INPUTMODE,
                dwStyle: TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY,
                ulSort: 0,
                szDescription: field::<32>("easime"),
            };
        }
        Ok(())
    }

    fn GetStatus(&self) -> Result<u32> {
        Ok(0)
    }

    fn Show(&self, _show: BOOL) -> Result<()> {
        Ok(())
    }

    fn GetTooltipString(&self) -> Result<BSTR> {
        Ok(BSTR::from(format!(
            "easime: {}",
            language_native_name(self.language())
        )))
    }
}

impl ITfLangBarItemButton_Impl for ModeButton_Impl {
    fn OnClick(&self, click: TfLBIClick, _point: &POINT, _area: *const RECT) -> Result<()> {
        // A right click opens the menu on its own (`InitMenu`).
        if click == TF_LBI_CLK_LEFT {
            (self.handler)(Request::Next);
        }
        Ok(())
    }

    fn InitMenu(&self, menu: Ref<ITfMenu>) -> Result<()> {
        let menu = menu.ok()?;
        let (current, cycle) = {
            let face = self.face.borrow();
            (face.language, face.cycle.clone())
        };
        for (index, language) in cycle.iter().enumerate() {
            let name: Vec<u16> = language_native_name(*language).encode_utf16().collect();
            let flags = if *language == current {
                TF_LBMENUF_RADIOCHECKED
            } else {
                0
            };
            // SAFETY: the menu and the text live for the call; no sub menu.
            unsafe {
                menu.AddMenuItem(
                    1 + index as u32,
                    flags,
                    HBITMAP::default(),
                    HBITMAP::default(),
                    &name,
                    std::ptr::null_mut(),
                )?;
            }
        }
        let none: [u16; 0] = [];
        let text: Vec<u16> = self.settings_text.encode_utf16().collect();
        // SAFETY: as above.
        unsafe {
            menu.AddMenuItem(
                0,
                TF_LBMENUF_SEPARATOR,
                HBITMAP::default(),
                HBITMAP::default(),
                &none,
                std::ptr::null_mut(),
            )?;
            menu.AddMenuItem(
                SETTINGS_ID,
                0,
                HBITMAP::default(),
                HBITMAP::default(),
                &text,
                std::ptr::null_mut(),
            )?;
        }
        Ok(())
    }

    fn OnMenuSelect(&self, id: u32) -> Result<()> {
        if id == SETTINGS_ID {
            (self.handler)(Request::Settings);
            return Ok(());
        }
        let language = {
            let face = self.face.borrow();
            id.checked_sub(1)
                .and_then(|index| face.cycle.get(index as usize).copied())
        };
        match language {
            Some(language) => {
                (self.handler)(Request::Select(language));
                Ok(())
            }
            None => Err(E_INVALIDARG.into()),
        }
    }

    fn GetIcon(&self) -> Result<HICON> {
        render_icon(self.language()).ok_or_else(|| E_FAIL.into())
    }

    fn GetText(&self) -> Result<BSTR> {
        Ok(BSTR::from(language_glyph(self.language()).to_string()))
    }
}

impl windows::Win32::UI::TextServices::ITfSource_Impl for ModeButton_Impl {
    // The signature, raw pointer included, is COM's: TSF owns the GUID.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn AdviseSink(&self, iid: *const GUID, sink: Ref<IUnknown>) -> Result<u32> {
        if iid.is_null() {
            return Err(E_POINTER.into());
        }
        // SAFETY: TSF passes a GUID.
        if unsafe { *iid } != ITfLangBarItemSink::IID {
            return Err(windows::Win32::System::Ole::CONNECT_E_CANNOTCONNECT.into());
        }
        let sink: ITfLangBarItemSink = sink.ok()?.cast()?;
        *self.sink.borrow_mut() = Some(sink);
        Ok(SINK_COOKIE)
    }

    fn UnadviseSink(&self, cookie: u32) -> Result<()> {
        if cookie != SINK_COOKIE {
            return Err(windows::Win32::System::Ole::CONNECT_E_NOCONNECTION.into());
        }
        *self.sink.borrow_mut() = None;
        Ok(())
    }
}

/// The button on TSF's language bar, for as long as the service is active.
pub struct LangBar {
    manager: ITfLangBarItemMgr,
    item: ITfLangBarItem,
    button: windows::core::ComObject<ModeButton>,
}

impl LangBar {
    /// Put a button showing `language` on the language bar. `manager` is TSF's
    /// manager of the thread.
    pub fn add(
        manager: ITfLangBarItemMgr,
        language: Language,
        cycle: Vec<Language>,
        settings_text: String,
        handler: Box<dyn Fn(Request)>,
    ) -> Result<Self> {
        let button =
            windows::core::ComObject::new(ModeButton::new(language, cycle, settings_text, handler));
        let item: ITfLangBarItem = button.cast()?;
        // SAFETY: the manager and the item are alive for the call.
        unsafe { manager.AddItem(&item) }?;
        Ok(Self {
            manager,
            item,
            button,
        })
    }

    /// The button, to change what it shows (hold no borrow of the owner while
    /// doing so: TSF is called).
    pub fn button(&self) -> windows::core::ComObject<ModeButton> {
        self.button.clone()
    }

    /// Take the button off the language bar.
    pub fn remove(self) {
        // SAFETY: the manager and the item are alive for the call.
        if let Err(e) = unsafe { self.manager.RemoveItem(&self.item) } {
            diag::log(&format!("language bar: could not remove the button: {e}"));
        }
    }
}

/// The settings app, next to the text service: `settings\easime-settings.exe`
/// in the install folder. `None` when it is not there (a build without it).
pub fn settings_app() -> Option<std::path::PathBuf> {
    let dll = crate::exports::dll_path()?;
    let exe = dll.parent()?.join("settings").join("easime-settings.exe");
    exe.is_file().then_some(exe)
}

/// Open the settings app. Returns whether it was started.
pub fn open_settings() -> bool {
    let Some(exe) = settings_app() else {
        diag::log("language bar: the settings app is not installed next to the text service");
        return false;
    };
    let path: Vec<u16> = exe
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // SAFETY: the strings are NUL-terminated and live for the call.
    let result = unsafe {
        ShellExecuteW(
            Some(HWND::default()),
            w!("open"),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecute reports success with a value above 32.
    let started = result.0 as isize > 32;
    if !started {
        diag::log(&format!(
            "language bar: could not start the settings app (code {})",
            result.0 as isize
        ));
    }
    started
}

/// The icon's side in pixels: the small icon of the display's scale.
pub(crate) fn icon_size() -> i32 {
    (16.0 * dpi() as f32 / 96.0).round().clamp(16.0, 48.0) as i32
}

const BADGE: [u8; 3] = [0x20, 0x22, 0x26];

/// Whether pixel (`x`, `y`) of a `size` square is inside the rounded badge
/// (corner radius a fifth of the side).
fn in_badge(x: i32, y: i32, size: i32) -> bool {
    let radius = (size as f32 * 0.2).max(2.0);
    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
    let s = size as f32;
    let cx = fx.clamp(radius, s - radius);
    let cy = fy.clamp(radius, s - radius);
    let (dx, dy) = (fx - cx, fy - cy);
    dx * dx + dy * dy <= radius * radius
}

/// Mix the badge colour with white by `coverage` (0 to 255), as BGRA with full opacity.
fn badge_pixel(coverage: u8) -> u32 {
    let mix = |badge: u8| -> u32 {
        let (b, c) = (badge as u32, coverage as u32);
        (b * (255 - c) + 255 * c) / 255
    };
    0xFF00_0000 | (mix(BADGE[0]) << 16) | (mix(BADGE[1]) << 8) | mix(BADGE[2])
}

/// The icon for `language`: its letter in white on the dark badge, with the
/// alpha the taskbar needs. `None` if Windows would not make the pieces. The
/// caller owns the icon.
pub(crate) fn render_icon(language: Language) -> Option<HICON> {
    let size = icon_size();
    let glyph: Vec<u16> = language_glyph(language)
        .to_string()
        .encode_utf16()
        .collect();
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..BITMAPINFOHEADER::default()
        },
        ..BITMAPINFO::default()
    };
    // SAFETY: plain GDI calls on objects made here and released below; the
    // bitmap's memory is `size * size` 32-bit pixels, which is what is read
    // and written, and it is only touched while the bitmap is alive.
    unsafe {
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return None;
        }
        let mut bits: *mut c_void = std::ptr::null_mut();
        let color = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(color.0));
            let _ = DeleteDC(dc);
            return None;
        }
        let previous = SelectObject(dc, HGDIOBJ(color.0));

        // White letter on black first: the grey it leaves is how much of each
        // pixel the letter covers.
        let height = (size as f32 * 0.8).round() as i32;
        let font = CreateFontW(
            -height,
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            ANTIALIASED_QUALITY,
            DEFAULT_PITCH.0 as u32 | FF_DONTCARE.0 as u32,
            face_name(language),
        );
        let old_font = SelectObject(dc, HGDIOBJ(font.0));
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, windows::Win32::Foundation::COLORREF(0x00FF_FFFF));
        let mut area = RECT {
            left: 0,
            top: 0,
            right: size,
            bottom: size,
        };
        let mut text = glyph;
        DrawTextW(
            dc,
            &mut text,
            &mut area,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        let _ = GdiFlush();

        let pixels = std::slice::from_raw_parts_mut(bits as *mut u32, (size * size) as usize);
        for y in 0..size {
            for x in 0..size {
                let slot = &mut pixels[(y * size + x) as usize];
                let word = *slot;
                let coverage = (word & 0xFF)
                    .max((word >> 8) & 0xFF)
                    .max((word >> 16) & 0xFF) as u8;
                *slot = if in_badge(x, y, size) {
                    badge_pixel(coverage)
                } else {
                    0
                };
            }
        }

        SelectObject(dc, old_font);
        let _ = DeleteObject(HGDIOBJ(font.0));
        SelectObject(dc, previous);
        let mask = CreateBitmap(size, size, 1, 1, None);
        let icon = CreateIconIndirect(&ICONINFO {
            fIcon: BOOL(1),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        })
        .ok();
        let _ = DeleteObject(HGDIOBJ(mask.0));
        let _ = DeleteObject(HGDIOBJ(color.0));
        let _ = DeleteDC(dc);
        icon
    }
}

/// Free an icon [`render_icon`] made.
#[allow(dead_code)]
pub(crate) fn destroy_icon(icon: HICON) {
    // SAFETY: an icon made by `CreateIconIndirect`, no longer used.
    unsafe {
        let _ = DestroyIcon(icon);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_badge_is_rounded() {
        let size = 16;
        assert!(!in_badge(0, 0, size), "the corner is cut off");
        assert!(in_badge(size / 2, size / 2, size));
        assert!(in_badge(size / 2, 0, size), "the middle of an edge stays");
    }

    #[test]
    fn a_letter_is_white_on_the_badge() {
        assert_eq!(badge_pixel(0), 0xFF20_2226, "no letter: the badge colour");
        assert_eq!(badge_pixel(255), 0xFFFF_FFFF, "all letter: white");
    }

    #[test]
    fn a_field_is_cut_and_ends_in_nul() {
        let text: [u16; 4] = field("easime");
        assert_eq!(text, ['e' as u16, 'a' as u16, 's' as u16, 0]);
        let short: [u16; 8] = field("ab");
        assert_eq!(&short[..3], &['a' as u16, 'b' as u16, 0]);
    }
}

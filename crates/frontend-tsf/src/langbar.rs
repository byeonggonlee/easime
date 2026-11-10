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
//! * A left click does what the plain switch key does; the menu (right click) lists the
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
use windows::Win32::Foundation::{
    E_FAIL, E_INVALIDARG, E_POINTER, HWND, LPARAM, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    ANTIALIASED_QUALITY, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLIP_DEFAULT_PRECIS, CreateBitmap,
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DEFAULT_CHARSET, DEFAULT_PITCH,
    DIB_RGB_COLORS, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject,
    DrawTextW, FF_DONTCARE, FW_BOLD, GdiFlush, HBITMAP, HDC, HGDIOBJ, OUT_DEFAULT_PRECIS,
    SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::TextServices::{
    GUID_LBI_INPUTMODE, ITfLangBarItem, ITfLangBarItem_Impl, ITfLangBarItemButton,
    ITfLangBarItemButton_Impl, ITfLangBarItemMgr, ITfLangBarItemSink, ITfMenu, TF_LANGBARITEMINFO,
    TF_LBI_CLK_LEFT, TF_LBI_CLK_RIGHT, TF_LBI_ICON, TF_LBI_STYLE_BTN_BUTTON,
    TF_LBI_STYLE_SHOWNINTRAY, TF_LBI_TEXT, TF_LBI_TOOLTIP, TF_LBMENUF_RADIOCHECKED,
    TF_LBMENUF_SEPARATOR, TfLBIClick,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DestroyIcon, DestroyMenu,
    DestroyWindow, HICON, ICONINFO, MF_CHECKED, MF_SEPARATOR, MF_STRING, MF_UNCHECKED,
    PostMessageW, SW_SHOWNORMAL, SetForegroundWindow, TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TrackPopupMenu, WM_NULL, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{BOOL, BSTR, GUID, IUnknown, Interface, PCWSTR, Ref, Result, implement, w};

/// What the user asked of the service through the button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Do what the plain switch key does (a left click).
    Toggle,
    /// Go to this language (a menu choice).
    Select(Language),
    /// Open the settings app (a menu choice).
    Settings,
}

/// The id of the menu entry that opens the settings; the languages count up from 1.
const SETTINGS_ID: u32 = 1000;

/// One line of the button's menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: u32,
    pub text: String,
    pub kind: Kind,
}

/// What a line of the menu is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A choice; `checked` marks the language that is on.
    Item { checked: bool },
    /// The line between the languages and the settings.
    Separator,
}

/// The lines of the menu: the languages of the cycle (the one that is on
/// marked), a separator, the settings. The same lines are given to TSF
/// (`InitMenu`) and shown by the button itself on a right click.
pub fn menu_entries(current: Language, cycle: &[Language], settings_text: &str) -> Vec<Entry> {
    let mut lines: Vec<Entry> = cycle
        .iter()
        .enumerate()
        .map(|(index, language)| Entry {
            id: 1 + index as u32,
            text: language_native_name(*language).to_string(),
            kind: Kind::Item {
                checked: *language == current,
            },
        })
        .collect();
    lines.push(Entry {
        id: 0,
        text: String::new(),
        kind: Kind::Separator,
    });
    lines.push(Entry {
        id: SETTINGS_ID,
        text: settings_text.to_string(),
        kind: Kind::Item { checked: false },
    });
    lines
}

/// What choosing the line `id` of the menu asks for. `None` for an id that is
/// no line of it.
pub fn request_for(id: u32, cycle: &[Language]) -> Option<Request> {
    if id == SETTINGS_ID {
        return Some(Request::Settings);
    }
    let index = id.checked_sub(1)?;
    cycle.get(index as usize).copied().map(Request::Select)
}

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

    /// The lines of the menu for what the button shows now.
    fn entries(&self) -> Vec<Entry> {
        let face = self.face.borrow();
        menu_entries(face.language, &face.cycle, &self.settings_text)
    }

    /// Show the menu at `at` (screen coordinates) and do what the user picks.
    /// Nothing is borrowed while the menu is open: TSF may call the button.
    fn pop_up(&self, at: POINT) {
        let entries = self.entries();
        let cycle = self.face.borrow().cycle.clone();
        let Some(id) = track_popup(&entries, at) else {
            return;
        };
        if let Some(request) = request_for(id, &cycle) {
            (self.handler)(request);
        }
    }
}

/// Show `entries` as a pop-up menu with its bottom edge at `at` (the button is
/// in the taskbar; Windows moves the menu where it fits), wait for the user,
/// and return the id they chose, `None` if they chose nothing.
fn track_popup(entries: &[Entry], at: POINT) -> Option<u32> {
    // SAFETY: a window and a menu of ours, made and destroyed here; the texts
    // live for the calls that read them.
    unsafe {
        // The menu belongs to a window, and closes when the user clicks away
        // only if that window is the foreground one.
        let owner = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            w!("STATIC"),
            w!("easime"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            None,
            None,
        )
        .ok()?;
        let Ok(menu) = CreatePopupMenu() else {
            let _ = DestroyWindow(owner);
            return None;
        };
        for entry in entries {
            let added = match entry.kind {
                Kind::Separator => AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()),
                Kind::Item { checked } => {
                    let text: Vec<u16> = entry.text.encode_utf16().chain(Some(0)).collect();
                    let flags = MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED };
                    AppendMenuW(menu, flags, entry.id as usize, PCWSTR(text.as_ptr()))
                }
            };
            if let Err(e) = added {
                diag::log(&format!("language bar: could not add a menu line: {e}"));
            }
        }
        let _ = SetForegroundWindow(owner);
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            at.x,
            at.y,
            None,
            owner,
            None,
        );
        // Without this a menu opened from the taskbar may close at once the
        // second time (Windows' documented remedy for notification menus).
        let _ = PostMessageW(Some(owner), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        let _ = DestroyWindow(owner);
        u32::try_from(chosen.0).ok().filter(|id| *id > 0)
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
    fn OnClick(&self, click: TfLBIClick, point: &POINT, _area: *const RECT) -> Result<()> {
        // Said in the log, so that a PC where a click does not do what it should
        // can tell whether Windows hands the click to us at all.
        diag::log(&format!(
            "language bar: click {} at ({}, {})",
            if click == TF_LBI_CLK_LEFT {
                "left"
            } else if click == TF_LBI_CLK_RIGHT {
                "right"
            } else {
                "other"
            },
            point.x,
            point.y
        ));
        if click == TF_LBI_CLK_LEFT {
            (self.handler)(Request::Toggle);
        } else if click == TF_LBI_CLK_RIGHT {
            // A push button has no menu of Windows' own: the menu is ours to show.
            self.pop_up(*point);
        }
        Ok(())
    }

    fn InitMenu(&self, menu: Ref<ITfMenu>) -> Result<()> {
        let menu = menu.ok()?;
        let entries = self.entries();
        for entry in &entries {
            let (flags, text) = match entry.kind {
                Kind::Item { checked: true } => (TF_LBMENUF_RADIOCHECKED, entry.text.as_str()),
                Kind::Item { checked: false } => (0, entry.text.as_str()),
                Kind::Separator => (TF_LBMENUF_SEPARATOR, ""),
            };
            let text: Vec<u16> = text.encode_utf16().collect();
            // SAFETY: the menu and the text live for the call; no sub menu.
            unsafe {
                menu.AddMenuItem(
                    entry.id,
                    flags,
                    HBITMAP::default(),
                    HBITMAP::default(),
                    &text,
                    std::ptr::null_mut(),
                )?;
            }
        }
        Ok(())
    }

    fn OnMenuSelect(&self, id: u32) -> Result<()> {
        let cycle = self.face.borrow().cycle.clone();
        match request_for(id, &cycle) {
            Some(request) => {
                (self.handler)(request);
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

/// How much of the icon's side the letter's ink takes at most, tall and wide:
/// the letter is the icon, the badge only its ground.
const INK_HEIGHT: f32 = 0.8;
const INK_WIDTH: f32 = 0.88;

/// A pixel the letter covers by at least this much (of 255) is ink.
const INK_LEVEL: u8 = 64;

/// The smallest box (left, top, right, bottom, all inclusive) around the ink of
/// `coverage`, a `size` square of 0 to 255 per pixel. `None` if there is none.
fn ink_bounds(coverage: &[u8], size: i32) -> Option<(i32, i32, i32, i32)> {
    let mut found: Option<(i32, i32, i32, i32)> = None;
    for y in 0..size {
        for x in 0..size {
            if coverage[(y * size + x) as usize] < INK_LEVEL {
                continue;
            }
            found = Some(match found {
                None => (x, y, x, y),
                Some((left, top, right, bottom)) => {
                    (left.min(x), top.min(y), right.max(x), bottom.max(y))
                }
            });
        }
    }
    found
}

/// The font height (in pixels) that would make a letter of `em` pixels, whose
/// ink is the box `ink`, as big as the icon of side `size` lets it be.
fn fitted_em(ink: (i32, i32, i32, i32), em: i32, size: i32) -> i32 {
    let width = (ink.2 - ink.0 + 1) as f32;
    let height = (ink.3 - ink.1 + 1) as f32;
    let scale = (INK_HEIGHT * size as f32 / height).min(INK_WIDTH * size as f32 / width);
    ((em as f32 * scale).round() as i32).clamp(size / 2, size * 2)
}

/// How far to move a letter whose ink is the box `ink` to put the ink in the
/// middle of the icon of side `size`.
fn centring(ink: (i32, i32, i32, i32), size: i32) -> (i32, i32) {
    // Twice the middle of the ink, so that odd widths keep their half pixel.
    let (twice_x, twice_y) = (ink.0 + ink.2 + 1, ink.1 + ink.3 + 1);
    ((size - twice_x) / 2, (size - twice_y) / 2)
}

/// A `size` square top-down 32-bit bitmap in a memory device context of its
/// own, and the pointer to its `size * size` pixels. The caller frees both
/// with [`release_canvas`].
///
/// # Safety
/// Plain GDI calls; the pointer is good for as long as the bitmap lives.
unsafe fn canvas(size: i32) -> Option<(HDC, HBITMAP, *mut u32)> {
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
    // SAFETY: as the caller promises; what is made here is released on failure.
    unsafe {
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return None;
        }
        let mut bits: *mut c_void = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(dc);
            return None;
        };
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            return None;
        }
        Some((dc, bitmap, bits as *mut u32))
    }
}

/// Free what [`canvas`] made.
///
/// # Safety
/// The bitmap must not be selected into another device context.
unsafe fn release_canvas(dc: HDC, bitmap: HBITMAP) {
    // SAFETY: objects made by `canvas`, no longer used.
    unsafe {
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
    }
}

/// Draw `glyph` in white on black, in a font `em` pixels high, into the bitmap
/// at `bits` (selected into `dc`), moved by (`dx`, `dy`), and say how much each
/// pixel is covered (0 to 255): the grey the letter leaves.
///
/// # Safety
/// `bits` must point to `size * size` pixels of the bitmap selected into `dc`.
unsafe fn draw_letter(
    dc: HDC,
    bits: *mut u32,
    size: i32,
    language: Language,
    glyph: &[u16],
    (em, dx, dy): (i32, i32, i32),
) -> Vec<u8> {
    // SAFETY: as the caller promises; the font is made and freed here.
    unsafe {
        let pixels = std::slice::from_raw_parts_mut(bits, (size * size) as usize);
        pixels.fill(0);
        let font = CreateFontW(
            -em,
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
            left: dx,
            top: dy,
            right: size + dx,
            bottom: size + dy,
        };
        let mut text = glyph.to_vec();
        DrawTextW(
            dc,
            &mut text,
            &mut area,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        let _ = GdiFlush();
        SelectObject(dc, old_font);
        let _ = DeleteObject(HGDIOBJ(font.0));
        pixels
            .iter()
            .map(|word| {
                (word & 0xFF)
                    .max((word >> 8) & 0xFF)
                    .max((word >> 16) & 0xFF) as u8
            })
            .collect()
    }
}

/// How much of each pixel of an icon of side `size` the letter of `language`
/// covers. The letter is drawn three times: a first time to see how much room
/// its ink takes, a second time at the size that fills the icon, a third time
/// moved to the middle (the font's own line box does not put the ink there). So
/// `A`, `한` and `あ` all fill the badge. `None` if Windows would not make the
/// pieces.
fn glyph_coverage(language: Language, size: i32) -> Option<Vec<u8>> {
    let glyph: Vec<u16> = language_glyph(language)
        .to_string()
        .encode_utf16()
        .collect();
    // SAFETY: a canvas made here and freed below; `draw_letter` gets the
    // pointer of the bitmap that is selected into the same device context.
    unsafe {
        let (dc, bitmap, bits) = canvas(size)?;
        let previous = SelectObject(dc, HGDIOBJ(bitmap.0));
        let draw =
            |placing: (i32, i32, i32)| draw_letter(dc, bits, size, language, &glyph, placing);
        // The first draw is a little smaller than the icon, so that the glyph's
        // box is not cut off at the edges where the ink is measured.
        let first_em = size * 3 / 4;
        let first = draw((first_em, 0, 0));
        let coverage = match ink_bounds(&first, size) {
            None => first,
            Some(ink) => {
                let em = fitted_em(ink, first_em, size);
                let second = draw((em, 0, 0));
                match ink_bounds(&second, size) {
                    None => second,
                    Some(ink) => {
                        let (dx, dy) = centring(ink, size);
                        draw((em, dx, dy))
                    }
                }
            }
        };
        SelectObject(dc, previous);
        release_canvas(dc, bitmap);
        Some(coverage)
    }
}

/// The icon for `language`: its letter in white on the dark badge, as large as
/// the badge allows, with the alpha the taskbar needs. `None` if Windows would
/// not make the pieces. The caller owns the icon.
pub(crate) fn render_icon(language: Language) -> Option<HICON> {
    let size = icon_size();
    let coverage = glyph_coverage(language, size)?;
    // SAFETY: a canvas made here, written within its `size * size` pixels, and
    // freed below once the icon has its own copy.
    unsafe {
        let (dc, color, bits) = canvas(size)?;
        let pixels = std::slice::from_raw_parts_mut(bits, (size * size) as usize);
        for y in 0..size {
            for x in 0..size {
                let at = (y * size + x) as usize;
                pixels[at] = if in_badge(x, y, size) {
                    badge_pixel(coverage[at])
                } else {
                    0
                };
            }
        }
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
        release_canvas(dc, color);
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

    const CYCLE: [Language; 3] = [Language::English, Language::Korean, Language::Japanese];

    #[test]
    fn the_menu_lists_the_languages_then_the_settings() {
        let lines = menu_entries(Language::Korean, &CYCLE, "easime settings");
        let kinds: Vec<Kind> = lines.iter().map(|line| line.kind).collect();
        assert_eq!(
            kinds,
            [
                Kind::Item { checked: false },
                Kind::Item { checked: true },
                Kind::Item { checked: false },
                Kind::Separator,
                Kind::Item { checked: false },
            ],
            "the language that is on is marked"
        );
        assert_eq!(lines[1].text, "한국어");
        assert_eq!(lines[4].text, "easime settings");
        assert_eq!(lines[4].id, SETTINGS_ID);
    }

    #[test]
    fn a_choice_is_a_language_or_the_settings() {
        assert_eq!(
            request_for(1, &CYCLE),
            Some(Request::Select(Language::English))
        );
        assert_eq!(
            request_for(3, &CYCLE),
            Some(Request::Select(Language::Japanese))
        );
        assert_eq!(request_for(SETTINGS_ID, &CYCLE), Some(Request::Settings));
        assert_eq!(request_for(0, &CYCLE), None, "the separator is no choice");
        assert_eq!(request_for(4, &CYCLE), None, "past the languages");
    }

    #[test]
    fn every_line_the_menu_offers_can_be_chosen() {
        for line in menu_entries(Language::English, &CYCLE, "settings") {
            match line.kind {
                Kind::Item { .. } => assert!(
                    request_for(line.id, &CYCLE).is_some(),
                    "{:?} asks for nothing",
                    line.text
                ),
                Kind::Separator => assert_eq!(request_for(line.id, &CYCLE), None),
            }
        }
    }

    #[test]
    fn the_ink_of_a_letter_is_measured() {
        let mut coverage = vec![0u8; 16 * 16];
        assert_eq!(ink_bounds(&coverage, 16), None, "nothing drawn");
        coverage[(3 * 16 + 5) as usize] = 255;
        coverage[(9 * 16 + 11) as usize] = 200;
        coverage[(16 + 1) as usize] = INK_LEVEL - 1; // too faint to count
        assert_eq!(ink_bounds(&coverage, 16), Some((5, 3, 11, 9)));
    }

    #[test]
    fn a_small_letter_grows_and_a_wide_one_shrinks() {
        // 8 by 8 ink at 12 pixels in a 16 icon: far from filling it.
        let grown = fitted_em((4, 4, 11, 11), 12, 16);
        assert!(grown > 12, "grows, got {grown}");
        // 16 wide ink in a 16 icon is too wide; it must shrink.
        let shrunk = fitted_em((0, 2, 15, 13), 16, 16);
        assert!(shrunk < 16, "shrinks, got {shrunk}");
        // Never absurd, even for a speck.
        assert!(fitted_em((7, 7, 7, 7), 12, 16) <= 32);
    }

    #[test]
    fn ink_is_moved_to_the_middle() {
        // Ink on the left and top: move right and down.
        let (dx, dy) = centring((0, 0, 7, 7), 16);
        assert_eq!((dx, dy), (4, 4));
        // Ink already in the middle stays.
        assert_eq!(centring((4, 4, 11, 11), 16), (0, 0));
    }

    #[test]
    fn each_letter_fills_the_badge_and_sits_in_the_middle() {
        for language in [Language::English, Language::Korean, Language::Japanese] {
            for size in [16, 20, 24, 32] {
                let coverage = glyph_coverage(language, size)
                    .unwrap_or_else(|| panic!("{language:?}: the letter is drawn"));
                let ink = ink_bounds(&coverage, size)
                    .unwrap_or_else(|| panic!("{language:?} at {size}: the letter leaves ink"));
                let (width, height) = (ink.2 - ink.0 + 1, ink.3 - ink.1 + 1);
                let longest = width.max(height) as f32;
                assert!(
                    longest >= 0.7 * size as f32,
                    "{language:?} at {size}: the letter is {width}x{height}, too small"
                );
                assert!(
                    width as f32 <= 0.95 * size as f32 && height as f32 <= 0.95 * size as f32,
                    "{language:?} at {size}: the letter is {width}x{height}, too big"
                );
                // Centred within a pixel.
                let off_x = (ink.0 + ink.2 + 1 - size).abs();
                let off_y = (ink.1 + ink.3 + 1 - size).abs();
                assert!(
                    off_x <= 2 && off_y <= 2,
                    "{language:?} at {size}: the ink {ink:?} is off the middle"
                );
            }
        }
    }

    #[test]
    fn the_icon_is_made() {
        for language in [Language::English, Language::Korean, Language::Japanese] {
            let icon = render_icon(language).expect("an icon");
            destroy_icon(icon);
        }
    }

    #[test]
    fn a_field_is_cut_and_ends_in_nul() {
        let text: [u16; 4] = field("easime");
        assert_eq!(text, ['e' as u16, 'a' as u16, 's' as u16, 0]);
        let short: [u16; 8] = field("ab");
        assert_eq!(&short[..3], &['a' as u16, 'b' as u16, 0]);
    }
}

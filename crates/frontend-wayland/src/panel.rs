//! The indicator box on screen: a `wl_surface` with the `zwp_input_panel_v1`
//! role, filled from `wl_shm` buffers made by [`crate::indicator`].
//!
//! What KWin does with such a surface (read from its source,
//! `src/inputpanelv1window.cpp`, `src/inputmethod.cpp` and
//! `src/wayland/inputmethod_v1.cpp`, as of Plasma 6.x):
//!
//! * `get_input_panel_surface` gives the surface the role
//!   `input_panel_surface_v1` and creates an `InputPanelV1Window`. KWin keeps
//!   only ONE such window: a second panel surface replaces (destroys) the
//!   first. So we create the surface once and reuse it.
//! * The window is positioned only after the client picks a mode:
//!   * `set_overlay_panel`: placed at the left edge of the focused text
//!     input's cursor rectangle, directly below it (flipped above it if it
//!     would leave the screen, clamped into the work area). The rectangle
//!     comes from text-input-v2/v3 `set_cursor_rectangle`; KWin repositions
//!     whenever it changes. Overlay panels are shown as soon as the surface is
//!     mapped, whether or not anyone asked for an on-screen keyboard.
//!   * `set_toplevel`: "virtual keyboard" mode: stuck to the bottom centre of
//!     the screen and shown only while KWin wants a keyboard. Not what we
//!     want.
//! * Nothing is visible before the first commit with a buffer attached
//!   (`mapped`). Attaching a null buffer and committing unmaps the window
//!   (`setHidden(true)`), and attaching a buffer again shows it again, so the
//!   surface can be reused for every flash of the box.
//! * The window never accepts keyboard focus (`acceptsFocus() == false`), so
//!   showing it does not steal typing. It does take pointer input unless the
//!   surface's input region is empty, so we set an empty input region.
//! * Size is whatever the surface size is (buffer size / buffer scale); there
//!   are no extra constraints. We use scale 1.
//! * Caveat: without a context (no focused text field) KWin has no cursor
//!   rectangle and would put the box in the top-left corner, so we unmap the
//!   box when the field loses focus. Text-input-v1 clients send no cursor
//!   rectangle at all, so for them the box lands at the top-left of the
//!   window.
//!
//! **OS-specific: Wayland.** The input panel surface is a KWin feature; other OSes
//! show the box and the candidate list with their own windows.

use crate::app::App;
use crate::indicator::{BOX_SIZE, Bitmap, glyph_for, render};
use ime_core::{CandidateList, Language};
use rustix::fs::{MemfdFlags, ftruncate, memfd_create};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::os::fd::AsFd;
use std::sync::OnceLock;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Proxy, QueueHandle};

use crate::protocol::client::zwp_input_panel_surface_v1::ZwpInputPanelSurfaceV1;
use crate::protocol::client::zwp_input_panel_v1::ZwpInputPanelV1;

const LANGUAGES: [Language; 5] = [
    Language::English,
    Language::Korean,
    Language::Japanese,
    Language::ChineseSimplified,
    Language::ChineseTraditional,
];

/// The rendered boxes, one per language glyph. Fonts are scanned once per
/// process; this takes tens of milliseconds in release builds.
fn bitmaps() -> &'static HashMap<char, Bitmap> {
    static BITMAPS: OnceLock<HashMap<char, Bitmap>> = OnceLock::new();
    BITMAPS.get_or_init(|| {
        let fonts = crate::font::Fonts::load_system();
        let mut map = HashMap::new();
        for language in LANGUAGES {
            let glyph = glyph_for(language);
            if map.contains_key(&glyph) {
                continue; // the two Chinese scripts share a letter
            }
            let font = fonts.font_for(glyph);
            if font.is_none() {
                crate::log!("no font has {glyph:?}; its indicator box will be empty");
            }
            map.insert(glyph, render(glyph, font.as_ref(), BOX_SIZE));
        }
        map
    })
}

/// Put `bitmap` into a new `wl_shm` pool and make a buffer from it.
fn make_buffer(
    shm: &WlShm,
    bitmap: &Bitmap,
    qh: &QueueHandle<App>,
) -> Result<WlBuffer, std::io::Error> {
    let fd = memfd_create("easime-indicator", MemfdFlags::CLOEXEC)?;
    let len = bitmap.data.len();
    ftruncate(&fd, len as u64)?;
    let mut file = File::from(fd);
    file.write_all(&bitmap.data)?;
    let pool = shm.create_pool(file.as_fd(), len as i32, qh, ());
    let buffer = pool.create_buffer(
        0,
        bitmap.width as i32,
        bitmap.height as i32,
        bitmap.stride as i32,
        wl_shm::Format::Argb8888,
        qh,
        (),
    );
    // The buffer keeps the memory alive after the pool object is gone.
    pool.destroy();
    Ok(buffer)
}

struct PanelSurface {
    surface: WlSurface,
    // Kept so the role object lives as long as the surface.
    _role: ZwpInputPanelSurfaceV1,
}

/// The indicator box. Create it only if the compositor offers all of
/// `wl_compositor`, `wl_shm` and `zwp_input_panel_v1`.
pub struct Panel {
    compositor: WlCompositor,
    shm: WlShm,
    input_panel: ZwpInputPanelV1,
    qh: QueueHandle<App>,
    surface: Option<PanelSurface>,
    buffers: HashMap<char, WlBuffer>,
    mapped: bool,
    painter: crate::candidate::Painter,
    candidate_buffer: Option<WlBuffer>,
}

impl Panel {
    pub fn new(
        compositor: WlCompositor,
        shm: WlShm,
        input_panel: ZwpInputPanelV1,
        qh: QueueHandle<App>,
    ) -> Self {
        Self {
            compositor,
            shm,
            input_panel,
            qh,
            surface: None,
            buffers: HashMap::new(),
            mapped: false,
            painter: crate::candidate::Painter::new(),
            candidate_buffer: None,
        }
    }

    /// Render the boxes now (scans fonts the first time) instead of at the
    /// first flash.
    pub fn prepare(&self) {
        let _ = bitmaps();
    }

    /// Create the surface with its role, once.
    fn ensure_surface(&mut self) -> &PanelSurface {
        self.surface.get_or_insert_with(|| {
            let qh = &self.qh;
            let surface = self.compositor.create_surface(qh, ());
            let role = self.input_panel.get_input_panel_surface(&surface, qh, ());
            // Placed below the text cursor; see the module comment.
            role.set_overlay_panel();
            // Clicks must go to the application below, not to the box.
            let empty = self.compositor.create_region(qh, ());
            surface.set_input_region(Some(&empty));
            empty.destroy();
            PanelSurface {
                surface,
                _role: role,
            }
        })
    }

    /// Show the box for `language` (replacing whatever is shown).
    pub fn show(&mut self, language: Language) {
        let glyph = glyph_for(language);
        let Some(bitmap) = bitmaps().get(&glyph) else {
            return;
        };
        if !self.buffers.contains_key(&glyph) {
            match make_buffer(&self.shm, bitmap, &self.qh) {
                Ok(buffer) => {
                    self.buffers.insert(glyph, buffer);
                }
                Err(e) => {
                    crate::log!("indicator buffer failed: {e}");
                    return;
                }
            }
        }
        let buffer = self.buffers[&glyph].clone();
        let surface = &self.ensure_surface().surface;
        surface.attach(Some(&buffer), 0, 0);
        if surface.version() >= 4 {
            surface.damage_buffer(0, 0, bitmap.width as i32, bitmap.height as i32);
        } else {
            surface.damage(0, 0, bitmap.width as i32, bitmap.height as i32);
        }
        surface.commit();
        self.mapped = true;
    }

    /// Whether a candidate list can be displayed with readable text.
    pub fn can_show_candidates(&self) -> bool {
        self.painter.available()
    }

    /// Show `list` with the text being converted in its header. `language`
    /// chooses the fonts: the same Han character is drawn differently in
    /// Japanese and Korean.
    pub fn show_candidates(
        &mut self,
        list: &CandidateList,
        reading: &str,
        language: Language,
    ) -> bool {
        let Some(bitmap) = self.painter.render(list, reading, language) else {
            return false;
        };
        let buffer = match make_buffer(&self.shm, &bitmap, &self.qh) {
            Ok(buffer) => buffer,
            Err(error) => {
                crate::log!("candidate buffer failed: {error}");
                return false;
            }
        };
        let surface = &self.ensure_surface().surface;
        surface.attach(Some(&buffer), 0, 0);
        if surface.version() >= 4 {
            surface.damage_buffer(0, 0, bitmap.width as i32, bitmap.height as i32);
        } else {
            surface.damage(0, 0, bitmap.width as i32, bitmap.height as i32);
        }
        surface.commit();
        if let Some(previous) = self.candidate_buffer.replace(buffer) {
            previous.destroy();
        }
        self.mapped = true;
        true
    }

    /// Unmap the shared overlay without destroying its role.
    pub fn hide(&mut self) {
        if !self.mapped {
            return;
        }
        self.mapped = false;
        if let Some(panel) = &self.surface {
            panel.surface.attach(None, 0, 0);
            panel.surface.commit();
        }
    }
}

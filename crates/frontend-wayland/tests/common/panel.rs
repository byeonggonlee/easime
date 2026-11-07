//! The part of the fake KWin that the indicator box talks to: `wl_compositor`,
//! `wl_shm` and `zwp_input_panel_v1`. Everything the client does with them is
//! recorded as [`PanelEv`] in `State::panel_log`, and the pixels of every
//! committed buffer are read back so tests can look at them.

use std::fs::File;
use std::os::unix::fs::FileExt;
use std::sync::{Arc, Mutex};

use wayland_server::protocol::wl_buffer::{self, WlBuffer};
use wayland_server::protocol::wl_compositor::{self, WlCompositor};
use wayland_server::protocol::wl_region::{self, WlRegion};
use wayland_server::protocol::wl_shm::{self, WlShm};
use wayland_server::protocol::wl_shm_pool::{self, WlShmPool};
use wayland_server::protocol::wl_surface::{self, WlSurface};
use wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource};

use super::State;
use super::protocol::zwp_input_panel_surface_v1::{self, ZwpInputPanelSurfaceV1};
use super::protocol::zwp_input_panel_v1::{self, ZwpInputPanelV1};

/// A snapshot of the buffer attached to a surface at `wl_surface.commit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shot {
    pub width: i32,
    pub height: i32,
    pub stride: i32,
    /// `wl_shm` format code (0 is ARGB8888).
    pub format: u32,
    /// The pixel bytes, `stride * height` of them, read from the pool.
    pub pixels: Vec<u8>,
}

impl Shot {
    /// Number of pixels whose alpha byte (ARGB8888, little endian) is not 0.
    pub fn visible_pixels(&self) -> usize {
        self.pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] != 0)
            .count()
    }

    /// Pixels brighter than the dark background of the indicator box, which
    /// is where the glyph is.
    pub fn bright_pixels(&self) -> usize {
        self.pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] != 0 && p[0].max(p[1]).max(p[2]) > 0x70)
            .count()
    }

    /// A hash of the pixels, to tell two glyphs apart.
    pub fn digest(&self) -> u64 {
        self.pixels.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
            (h ^ b as u64).wrapping_mul(0x0100_0000_01b3)
        })
    }
}

/// What the client did with surfaces, shm and the input panel, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelEv {
    SurfaceCreated,
    SurfaceDestroyed,
    RegionCreated,
    /// `wl_surface.set_input_region`; `true` if the region is empty.
    SetInputRegion {
        empty: bool,
    },
    ShmPool {
        size: i32,
    },
    ShmBuffer {
        width: i32,
        height: i32,
        stride: i32,
        format: u32,
    },
    BufferDestroyed,
    /// `zwp_input_panel_v1.get_input_panel_surface`.
    GetPanelSurface,
    SetOverlayPanel,
    SetToplevel,
    /// `wl_surface.commit`. `attached_null` is true if a null buffer was
    /// attached; `buffer` is the attached buffer's content, if any.
    Commit {
        attached_null: bool,
        buffer: Option<Shot>,
    },
}

/// Which optional globals the fake compositor offers.
#[derive(Debug, Clone, Copy)]
pub struct Globals {
    pub compositor: bool,
    pub shm: bool,
    pub input_panel: bool,
}

impl Default for Globals {
    fn default() -> Self {
        Self {
            compositor: true,
            shm: true,
            input_panel: true,
        }
    }
}

/// Memory shared by a client through `wl_shm.create_pool`.
pub struct PoolData {
    file: File,
    size: Mutex<i32>,
}

pub struct BufferData {
    pool: Arc<PoolData>,
    offset: i32,
    width: i32,
    height: i32,
    stride: i32,
    format: u32,
}

#[derive(Default)]
pub struct SurfaceData {
    /// `Some(None)` is a null attach.
    pending: Mutex<Option<Option<Arc<BufferData>>>>,
}

#[derive(Default)]
pub struct RegionData {
    rects: Mutex<usize>,
}

fn read_shot(b: &BufferData) -> Shot {
    let len = (b.stride * b.height) as usize;
    let mut pixels = vec![0u8; len];
    let size = *b.pool.size.lock().unwrap();
    assert!(
        b.offset >= 0 && b.offset as i64 + len as i64 <= size as i64,
        "buffer lies outside its pool"
    );
    b.pool
        .file
        .read_exact_at(&mut pixels, b.offset as u64)
        .expect("read the shm pool");
    Shot {
        width: b.width,
        height: b.height,
        stride: b.stride,
        format: b.format,
        pixels,
    }
}

impl GlobalDispatch<WlCompositor, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<WlCompositor>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<WlCompositor, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &WlCompositor,
        request: wl_compositor::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_compositor::Request::CreateSurface { id } => {
                data_init.init(id, SurfaceData::default());
                state.panel_log.push(PanelEv::SurfaceCreated);
            }
            wl_compositor::Request::CreateRegion { id } => {
                data_init.init(id, RegionData::default());
                state.panel_log.push(PanelEv::RegionCreated);
            }
            other => panic!("unexpected wl_compositor request: {other:?}"),
        }
    }
}

impl Dispatch<WlRegion, RegionData> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlRegion,
        request: wl_region::Request,
        data: &RegionData,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_region::Request::Add { .. } => *data.rects.lock().unwrap() += 1,
            wl_region::Request::Subtract { .. } | wl_region::Request::Destroy => {}
            other => panic!("unexpected wl_region request: {other:?}"),
        }
    }
}

impl Dispatch<WlSurface, SurfaceData> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &WlSurface,
        request: wl_surface::Request,
        data: &SurfaceData,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_surface::Request::Attach { buffer, .. } => {
                let attached = buffer.map(|b| b.data::<Arc<BufferData>>().expect("buffer").clone());
                *data.pending.lock().unwrap() = Some(attached);
            }
            wl_surface::Request::SetInputRegion { region } => {
                let empty = region.is_none_or(|r| {
                    *r.data::<RegionData>()
                        .expect("region data")
                        .rects
                        .lock()
                        .unwrap()
                        == 0
                });
                state.panel_log.push(PanelEv::SetInputRegion { empty });
            }
            wl_surface::Request::Commit => {
                let pending = data.pending.lock().unwrap().take();
                let (attached_null, buffer) = match pending {
                    None => (false, None),
                    Some(None) => (true, None),
                    Some(Some(b)) => (false, Some(read_shot(&b))),
                };
                if attached_null || buffer.is_some() {
                    state.panel_mapped = buffer.is_some();
                }
                state.panel_log.push(PanelEv::Commit {
                    attached_null,
                    buffer,
                });
            }
            wl_surface::Request::Destroy => state.panel_log.push(PanelEv::SurfaceDestroyed),
            // Damage, frame callbacks, scale, transform: not interesting.
            _ => {}
        }
    }
}

impl GlobalDispatch<WlShm, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<WlShm>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        let shm = data_init.init(resource, ());
        shm.format(wl_shm::Format::Argb8888);
        shm.format(wl_shm::Format::Xrgb8888);
    }
}

impl Dispatch<WlShm, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &WlShm,
        request: wl_shm::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_shm::Request::CreatePool { id, fd, size } => {
                data_init.init(
                    id,
                    Arc::new(PoolData {
                        file: File::from(fd),
                        size: Mutex::new(size),
                    }),
                );
                state.panel_log.push(PanelEv::ShmPool { size });
            }
            other => panic!("unexpected wl_shm request: {other:?}"),
        }
    }
}

impl Dispatch<WlShmPool, Arc<PoolData>> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &WlShmPool,
        request: wl_shm_pool::Request,
        pool: &Arc<PoolData>,
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_shm_pool::Request::CreateBuffer {
                id,
                offset,
                width,
                height,
                stride,
                format,
            } => {
                let format = match format {
                    wayland_server::WEnum::Value(f) => f as u32,
                    wayland_server::WEnum::Unknown(v) => v,
                };
                data_init.init(
                    id,
                    Arc::new(BufferData {
                        pool: pool.clone(),
                        offset,
                        width,
                        height,
                        stride,
                        format,
                    }),
                );
                state.panel_log.push(PanelEv::ShmBuffer {
                    width,
                    height,
                    stride,
                    format,
                });
            }
            wl_shm_pool::Request::Resize { size } => *pool.size.lock().unwrap() = size,
            wl_shm_pool::Request::Destroy => {}
            other => panic!("unexpected wl_shm_pool request: {other:?}"),
        }
    }
}

impl Dispatch<WlBuffer, Arc<BufferData>> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &WlBuffer,
        request: wl_buffer::Request,
        _: &Arc<BufferData>,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_buffer::Request::Destroy => state.panel_log.push(PanelEv::BufferDestroyed),
            other => panic!("unexpected wl_buffer request: {other:?}"),
        }
    }
}

impl GlobalDispatch<ZwpInputPanelV1, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<ZwpInputPanelV1>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ZwpInputPanelV1, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &ZwpInputPanelV1,
        request: zwp_input_panel_v1::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwp_input_panel_v1::Request::GetInputPanelSurface { id, .. } => {
                data_init.init(id, ());
                state.panel_log.push(PanelEv::GetPanelSurface);
            }
        }
    }
}

impl Dispatch<ZwpInputPanelSurfaceV1, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &ZwpInputPanelSurfaceV1,
        request: zwp_input_panel_surface_v1::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwp_input_panel_surface_v1::Request::SetToplevel { .. } => {
                state.panel_log.push(PanelEv::SetToplevel)
            }
            zwp_input_panel_surface_v1::Request::SetOverlayPanel => {
                state.panel_log.push(PanelEv::SetOverlayPanel)
            }
        }
    }
}

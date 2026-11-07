//! A fake KWin: a minimal Wayland server that speaks `zwp_input_method_v1`
//! over a socketpair, so the real client in `frontend_wayland` can be driven
//! end to end without a compositor.
//!
//! The compositor runs on the test thread; the client runs on a background
//! thread. Every wait has a deadline so a hung test fails instead of hanging.

#![allow(dead_code)]

use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, poll};
use rustix::time::Timespec;
use wayland_client::Connection;
use wayland_server::protocol::wl_keyboard::{self, WlKeyboard};
use wayland_server::{Client, DataInit, Dispatch, Display, DisplayHandle, GlobalDispatch, New};

use self::protocol::zwp_input_method_context_v1::{self, ZwpInputMethodContextV1};
use self::protocol::zwp_input_method_v1::{self, ZwpInputMethodV1};

/// Server-side bindings for the shipped protocol XML (generated here so the
/// crate itself does not need server code).
pub mod protocol {
    #![allow(
        clippy::all,
        non_upper_case_globals,
        non_camel_case_types,
        missing_docs
    )]

    use wayland_server;
    use wayland_server::protocol::*;

    pub mod __interfaces {
        use wayland_server::backend as wayland_backend;
        use wayland_server::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/input-method-unstable-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_server_code!("protocols/input-method-unstable-v1.xml");
}

/// How long any single wait may take before the test fails.
const DEADLINE: Duration = Duration::from_secs(5);

/// A request the client sent, in arrival order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Req {
    GrabKeyboard,
    /// `wl_keyboard.release`.
    KeyboardRelease,
    /// `zwp_input_method_context_v1.destroy`.
    ContextDestroy,
    CommitString {
        serial: u32,
        text: String,
    },
    PreeditString {
        serial: u32,
        text: String,
        commit: String,
    },
    PreeditStyling {
        index: u32,
        length: u32,
        style: u32,
    },
    PreeditCursor {
        index: i32,
    },
    Key {
        serial: u32,
        time: u32,
        key: u32,
        state: u32,
    },
    Modifiers {
        serial: u32,
        depressed: u32,
        latched: u32,
        locked: u32,
        group: u32,
    },
}

/// Compositor-side state, mutated by the request handlers.
#[derive(Default)]
pub struct State {
    input_method: Option<ZwpInputMethodV1>,
    context: Option<ZwpInputMethodContextV1>,
    keyboard: Option<WlKeyboard>,
    /// Requests not yet taken by the test.
    log: Vec<Req>,
    /// Serials of barrier `modifiers` requests that have come back.
    barriers_seen: Vec<u32>,
}

/// Serials at or above this value mark barrier `modifiers` events.
const BARRIER_BASE: u32 = 0xB000_0000;

struct NoClientData;
impl wayland_server::backend::ClientData for NoClientData {}

impl GlobalDispatch<ZwpInputMethodV1, ()> for State {
    fn bind(
        state: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<ZwpInputMethodV1>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        state.input_method = Some(data_init.init(resource, ()));
    }
}

impl Dispatch<ZwpInputMethodV1, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &ZwpInputMethodV1,
        request: zwp_input_method_v1::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        // The interface has no requests.
        match request {}
    }
}

impl Dispatch<ZwpInputMethodContextV1, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &ZwpInputMethodContextV1,
        request: zwp_input_method_context_v1::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        use zwp_input_method_context_v1::Request as R;
        let req = match request {
            R::Destroy => {
                state.context = None;
                Req::ContextDestroy
            }
            R::GrabKeyboard { keyboard } => {
                state.keyboard = Some(data_init.init(keyboard, ()));
                Req::GrabKeyboard
            }
            R::CommitString { serial, text } => Req::CommitString { serial, text },
            R::PreeditString {
                serial,
                text,
                commit,
            } => Req::PreeditString {
                serial,
                text,
                commit,
            },
            R::PreeditStyling {
                index,
                length,
                style,
            } => Req::PreeditStyling {
                index,
                length,
                style,
            },
            R::PreeditCursor { index } => Req::PreeditCursor { index },
            R::Key {
                serial,
                time,
                key,
                state,
            } => Req::Key {
                serial,
                time,
                key,
                state,
            },
            R::Modifiers {
                serial,
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
            } => {
                if serial >= BARRIER_BASE {
                    state.barriers_seen.push(serial);
                    return;
                }
                Req::Modifiers {
                    serial,
                    depressed: mods_depressed,
                    latched: mods_latched,
                    locked: mods_locked,
                    group,
                }
            }
            other => panic!("unexpected request from the client: {other:?}"),
        };
        state.log.push(req);
    }
}

impl Dispatch<WlKeyboard, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &WlKeyboard,
        request: wl_keyboard::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_keyboard::Request::Release => {
                state.keyboard = None;
                state.log.push(Req::KeyboardRelease);
            }
            other => panic!("unexpected wl_keyboard request: {other:?}"),
        }
    }
}

type ClientResult = Result<std::convert::Infallible, frontend_wayland::RunError>;

/// The fake compositor plus the client under test.
pub struct Fake {
    display: Display<State>,
    pub state: State,
    client: Option<JoinHandle<()>>,
    client_done: Receiver<ClientResult>,
}

impl Fake {
    /// Start the client on a background thread and wait until it has bound
    /// `zwp_input_method_v1`.
    pub fn start() -> Self {
        let (server_end, client_end) = UnixStream::pair().expect("socketpair");
        let display = Display::<State>::new().expect("display");
        display
            .handle()
            .create_global::<State, ZwpInputMethodV1, ()>(1, ());
        display
            .handle()
            .insert_client(server_end, Arc::new(NoClientData))
            .expect("insert client");

        let (tx, client_done) = channel();
        let client = std::thread::spawn(move || {
            let result = match Connection::from_socket(client_end) {
                Ok(connection) => frontend_wayland::run_on(&connection),
                Err(e) => Err(frontend_wayland::RunError::Startup(e.to_string())),
            };
            let _ = tx.send(result);
        });

        let mut fake = Self {
            display,
            state: State::default(),
            client: Some(client),
            client_done,
        };
        fake.wait_until("the client to bind zwp_input_method_v1", |s| {
            s.input_method.is_some()
        });
        fake
    }

    /// Serve the client until `done` holds, failing after the deadline or if
    /// the client stops.
    pub fn wait_until(&mut self, what: &str, done: impl Fn(&State) -> bool) {
        let deadline = Instant::now() + DEADLINE;
        loop {
            self.display
                .dispatch_clients(&mut self.state)
                .expect("dispatch");
            self.display.flush_clients().expect("flush");
            if done(&self.state) {
                return;
            }
            match self.client_done.try_recv() {
                Ok(result) => panic!("client stopped while waiting for {what}: {result:?}"),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => panic!("client thread died waiting for {what}"),
            }
            let now = Instant::now();
            if now >= deadline {
                panic!(
                    "timed out waiting for {what}; requests so far: {:#?}",
                    self.state.log
                );
            }
            let remaining = (deadline - now).min(Duration::from_millis(20));
            let timeout = Timespec {
                tv_sec: 0,
                tv_nsec: remaining.as_nanos() as _,
            };
            let mut fds = [PollFd::new(&self.display, PollFlags::IN)];
            let _ = poll(&mut fds, Some(&timeout));
        }
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        // Closing our end of the socket makes the client's event loop fail,
        // which ends its thread.
        let client = self.client.take();
        let display = std::mem::replace(
            &mut self.display,
            Display::new().expect("replacement display"),
        );
        drop(display);
        if let Some(client) = client
            && !std::thread::panicking()
        {
            let _ = client.join();
        }
    }
}

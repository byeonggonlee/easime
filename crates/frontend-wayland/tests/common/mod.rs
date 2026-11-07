//! A fake KWin: a minimal Wayland server that speaks `zwp_input_method_v1`
//! over a socketpair, so the real client in `frontend_wayland` can be driven
//! end to end without a compositor.
//!
//! The compositor runs on the test thread; the client runs on a background
//! thread. Every wait has a deadline so a hung test fails instead of hanging.

#![allow(dead_code)]

use std::fs::File;
use std::io::Write;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use easime_config::Config;
use rustix::event::{PollFd, PollFlags, poll};
use rustix::fs::{MemfdFlags, memfd_create};
use rustix::time::Timespec;
use wayland_backend::protocol::{Argument, Message};
use wayland_backend::smallvec::smallvec;
use wayland_client::Connection;
use wayland_server::protocol::wl_keyboard::{self, WlKeyboard};
use wayland_server::{
    Client, DataInit, Dispatch, Display, DisplayHandle, GlobalDispatch, New, Resource,
};
use xkbcommon::xkb;

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
    keymap: xkb::Keymap,
    /// Last modifier masks sent: depressed, latched, locked, group.
    mods: [u32; 4],
    /// Serial of `wl_keyboard` events (key, modifiers).
    keyboard_serial: u32,
    /// Serial of the latest `commit_state`.
    pub commit_serial: u32,
    time: u32,
    barriers: u32,
}

impl Fake {
    /// Start the client on a background thread and wait until it has bound
    /// `zwp_input_method_v1`.
    pub fn start() -> Self {
        Self::start_with(Config::default())
    }

    /// Like [`Fake::start`], with the given settings.
    pub fn start_with(config: Config) -> Self {
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
                Ok(connection) => frontend_wayland::run_with(&connection, &config),
                Err(e) => Err(frontend_wayland::RunError::Startup(e.to_string())),
            };
            let _ = tx.send(result);
        });

        let mut fake = Self {
            display,
            state: State::default(),
            client: Some(client),
            client_done,
            keymap: us_keymap(),
            mods: [0; 4],
            keyboard_serial: 1000,
            commit_serial: 0,
            time: 0,
            barriers: 0,
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

/// The plain US keymap, compiled from the system's xkeyboard-config.
fn us_keymap() -> xkb::Keymap {
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    xkb::Keymap::new_from_names(
        &context,
        "",
        "",
        "us",
        "",
        None,
        xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .expect("compile the US keymap (is xkeyboard-config installed?)")
}

/// Put `text` plus a terminating NUL into a fresh memfd, as KWin does for the
/// keymap. Returns the file and the size to announce (including the NUL).
fn keymap_fd(text: &str) -> (File, u32) {
    let fd = memfd_create("easime-test-keymap", MemfdFlags::CLOEXEC).expect("memfd_create");
    let mut file = File::from(fd);
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    file.write_all(&bytes).expect("write keymap");
    (file, bytes.len() as u32)
}

/// Opcode of the `wl_keyboard.key` event.
const EVT_KEY_OPCODE: u16 = 3;

impl Fake {
    /// Send `activate` for a new input method context (version 10, like a
    /// current KWin, so its keyboard may use `release` and key state 2) and
    /// wait for the client to grab the keyboard. Then deliver the keymap,
    /// `repeat_info` and an initial `modifiers` event.
    pub fn activate(&mut self) {
        self.activate_with(25, 600);
    }

    /// Like [`Fake::activate`], with the given `repeat_info` (rate per second,
    /// delay in ms). A rate of 0 means the compositor repeats keys itself.
    pub fn activate_with(&mut self, rate: i32, delay: i32) {
        let input_method = self.state.input_method.clone().expect("input method bound");
        let handle = self.display.handle();
        let client = input_method.client().expect("client alive");
        let context = client
            .create_resource::<ZwpInputMethodContextV1, (), State>(&handle, 10, ())
            .expect("create context");
        input_method.activate(&context);
        self.commit_serial += 1;
        context.commit_state(self.commit_serial);
        self.state.context = Some(context);
        self.wait_until("grab_keyboard", |s| s.keyboard.is_some());
        let _ = self.take_requests();

        let text = self.keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1);
        let (file, size) = keymap_fd(&text);
        let keyboard = self.keyboard().clone();
        keyboard.keymap(wl_keyboard::KeymapFormat::XkbV1, file.as_fd(), size);
        keyboard.repeat_info(rate, delay);
        self.mods = [0; 4];
        self.barrier();
    }

    fn keyboard(&self) -> &WlKeyboard {
        self.state.keyboard.as_ref().expect("keyboard grabbed")
    }

    fn next_serial(&mut self) -> u32 {
        self.keyboard_serial += 1;
        self.keyboard_serial
    }

    /// Send a `wl_keyboard.key` event with a raw state value (0 released,
    /// 1 pressed, 2 repeated). Returns its serial.
    pub fn key_raw(&mut self, evdev: u32, state: u32) -> u32 {
        let serial = self.next_serial();
        self.time += 10;
        // `repeated` is newer than the generated enum, so build the event by
        // hand.
        let message = Message {
            sender_id: self.keyboard().id(),
            opcode: EVT_KEY_OPCODE,
            args: smallvec![
                Argument::Uint(serial),
                Argument::Uint(self.time),
                Argument::Uint(evdev),
                Argument::Uint(state),
            ],
        };
        self.display
            .handle()
            .backend_handle()
            .send_event(message)
            .expect("send key");
        serial
    }

    pub fn press(&mut self, evdev: u32) -> u32 {
        self.key_raw(evdev, 1)
    }

    pub fn release(&mut self, evdev: u32) -> u32 {
        self.key_raw(evdev, 0)
    }

    pub fn repeat(&mut self, evdev: u32) -> u32 {
        self.key_raw(evdev, 2)
    }

    /// Press and release each key in turn.
    pub fn tap_all(&mut self, evdev: &[u32]) {
        for &key in evdev {
            self.press(key);
            self.release(key);
        }
    }

    /// Send a `wl_keyboard.modifiers` event. Returns its serial.
    pub fn modifiers(&mut self, depressed: u32, latched: u32, locked: u32, group: u32) -> u32 {
        let serial = self.next_serial();
        self.mods = [depressed, latched, locked, group];
        self.keyboard()
            .modifiers(serial, depressed, latched, locked, group);
        serial
    }

    /// Modifier mask bit of an xkb modifier name, e.g. `xkb::MOD_NAME_SHIFT`.
    pub fn mod_mask(&self, name: &str) -> u32 {
        let index = self.keymap.mod_get_index(name);
        assert_ne!(index, xkb::MOD_INVALID, "no modifier {name}");
        1 << index
    }

    /// Evdev code of the first key that produces `keysym` with no modifiers.
    pub fn evdev_for_keysym(&self, keysym: u32) -> u32 {
        let state = xkb::State::new(&self.keymap);
        let mut found = None;
        self.keymap.key_for_each(|_, keycode| {
            if found.is_none() && state.key_get_one_sym(keycode).raw() == keysym {
                found = Some(keycode.raw() - 8);
            }
        });
        found.unwrap_or_else(|| panic!("no key produces keysym {keysym:#x}"))
    }

    /// Wait until the client has handled everything sent so far. It sends a
    /// `modifiers` event that repeats the current state, and waits for the
    /// client to forward it back (the echo is not recorded).
    pub fn barrier(&mut self) {
        self.barriers += 1;
        let serial = BARRIER_BASE + self.barriers;
        let [d, l, k, g] = self.mods;
        self.keyboard().modifiers(serial, d, l, k, g);
        self.wait_until("the barrier to come back", |s| {
            s.barriers_seen.contains(&serial)
        });
    }

    /// Serve the client until at least `count` `commit_string` requests have
    /// arrived since the requests were last taken.
    pub fn wait_for_commits(&mut self, count: usize) {
        self.wait_until("enough commit_string requests", |s| {
            s.log
                .iter()
                .filter(|r| matches!(r, Req::CommitString { .. }))
                .count()
                >= count
        });
    }

    /// Take the requests recorded so far without waiting.
    pub fn take_requests(&mut self) -> Vec<Req> {
        std::mem::take(&mut self.state.log)
    }

    /// Wait for the client to catch up, then take its requests.
    pub fn settle(&mut self) -> Vec<Req> {
        self.barrier();
        self.take_requests()
    }

    /// Send `commit_state` with a new serial. Returns it.
    pub fn commit_state(&mut self) -> u32 {
        self.commit_serial += 1;
        let context = self.state.context.as_ref().expect("active context");
        context.commit_state(self.commit_serial);
        self.commit_serial
    }

    /// Send `deactivate` and wait until the client has destroyed the context.
    pub fn deactivate(&mut self) {
        let input_method = self.state.input_method.clone().expect("input method");
        let context = self.state.context.clone().expect("active context");
        input_method.deactivate(&context);
        self.wait_until("the context to be destroyed", |s| {
            s.log.contains(&Req::ContextDestroy)
        });
    }
}

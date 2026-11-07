//! The input method client: talks `zwp_input_method_v1` with KWin.
//!
//! Flow (see the protocol XML for details):
//! 1. KWin sends `activate` when a text field gets focus, handing us a
//!    `zwp_input_method_context_v1`.
//! 2. We `grab_keyboard`, so every key goes to us first.
//! 3. For each key we either consume it (engine composes, we send
//!    `commit_string` / `preedit_string`) or forward it back with `key`.

use crate::log;
use crate::protocol::client::zwp_input_method_context_v1::{self, ZwpInputMethodContextV1};
use crate::protocol::client::zwp_input_method_v1::{self, ZwpInputMethodV1};
use crate::repeat::RepeatInfo;
use crate::timer::Timer;
use crate::toggle::{Decision, ToggleTracker};
use crate::translate::{case_from_shift, is_modifier_keysym, translate};
use easime_config::{BackspaceUnit, Config};
use engine_hangul::{BackspaceMode, HangulEngine};
use engine_japanese::JapaneseEngine;
use ime_core::{EngineOutput, InputEngine, KeyEvent, Language, LanguageSwitcher, Modifiers};
use rustix::event::{PollFd, PollFlags, poll};
use rustix::io::Errno;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::ErrorKind;
use std::os::unix::fs::FileExt;
use wayland_client::backend::WaylandError;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_keyboard::{self, KeyState, KeymapFormat, WlKeyboard};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum, event_created_child};
use xkbcommon::xkb;

/// Preedit style value for "underline" (see text-input-v1 `preedit_style`).
const PREEDIT_STYLE_UNDERLINE: u32 = 5;

/// evdev keycodes are offset by 8 in XKB.
const EVDEV_OFFSET: u32 = 8;

/// Upper bound on repeats replayed after one timer wakeup.
const MAX_REPEATS_PER_WAKEUP: u64 = 8;

/// What happened to a key. `Repeated` (wl_keyboard key state 2, since
/// version 10) is sent by KWin when it repeats keys on the client's behalf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyAction {
    Pressed,
    Released,
    Repeated,
}

impl KeyAction {
    fn from_wire(state: WEnum<KeyState>) -> Option<Self> {
        match state {
            WEnum::Value(KeyState::Pressed) => Some(Self::Pressed),
            WEnum::Value(KeyState::Released) => Some(Self::Released),
            // The generated enum predates `repeated`, so it arrives as raw 2.
            WEnum::Unknown(2) => Some(Self::Repeated),
            _ => None,
        }
    }
}

struct Xkb {
    keymap: xkb::Keymap,
    state: xkb::State,
}

/// A key whose press we consumed, so its release must be dropped too.
#[derive(Debug, Clone, Copy)]
struct Swallowed {
    /// The engine input to replay when the key repeats. `None` for keys that
    /// must not repeat (the language toggle).
    event: Option<KeyEvent>,
    /// Serial and time of the original press, reused if the key is later
    /// handed over to the application.
    serial: u32,
    time: u32,
}

struct App {
    /// One engine per enabled language. English has none: keys pass through.
    engines: HashMap<Language, Box<dyn InputEngine>>,
    /// Which language is active and what the toggle key switches to next.
    switcher: LanguageSwitcher,
    context: Option<ZwpInputMethodContextV1>,
    keyboard: Option<WlKeyboard>,
    /// Serial of the latest `commit_state`, used for text requests.
    commit_serial: u32,
    xkb_context: xkb::Context,
    xkb: Option<Xkb>,
    swallowed: HashMap<u32, Swallowed>,
    /// From `wl_keyboard.repeat_info`.
    repeat_info: RepeatInfo,
    /// The consumed key our own timer is currently repeating.
    repeating: Option<u32>,
    toggle: ToggleTracker,
    /// Fires when a consumed key should repeat.
    timer: Timer,
}

impl App {
    fn new(config: &Config) -> std::io::Result<Self> {
        let backspace = match config.korean.backspace {
            BackspaceUnit::Jamo => BackspaceMode::Jamo,
            BackspaceUnit::Syllable => BackspaceMode::Syllable,
        };
        let mut engines: HashMap<Language, Box<dyn InputEngine>> = HashMap::new();
        for &language in &config.languages.enabled {
            match language {
                Language::Korean => {
                    engines.insert(
                        language,
                        Box::new(HangulEngine::with_backspace_mode(backspace)),
                    );
                }
                Language::Japanese => {
                    engines.insert(language, Box::new(JapaneseEngine::new()));
                }
                Language::English | Language::Chinese => {
                    log!("language {language:?} is not available yet, ignoring");
                }
            }
        }
        // Only languages that have an engine take part in the cycle.
        let usable: Vec<Language> = config
            .languages
            .enabled
            .iter()
            .copied()
            .filter(|l| engines.contains_key(l))
            .collect();
        Ok(Self {
            engines,
            switcher: LanguageSwitcher::new(&usable),
            context: None,
            keyboard: None,
            commit_serial: 0,
            xkb_context: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            xkb: None,
            swallowed: HashMap::new(),
            repeat_info: RepeatInfo::default(),
            repeating: None,
            toggle: ToggleTracker::with_setting(config.shortcuts.toggle_key),
            timer: Timer::new()?,
        })
    }

    /// The engine of the active language, or `None` for English.
    fn engine(&mut self) -> Option<&mut (dyn InputEngine + 'static)> {
        let language = self.switcher.current();
        self.engines.get_mut(&language).map(|e| e.as_mut())
    }

    /// The repeat timer fired: replay the repeating key once per expiration.
    fn on_timer(&mut self) {
        let expirations = self.timer.take_expirations().unwrap_or(0);
        // A long stall must not turn into a burst of hundreds of characters.
        for _ in 0..expirations.min(MAX_REPEATS_PER_WAKEUP) {
            match self.repeating {
                Some(key) => self.repeat_key(key),
                None => break,
            }
        }
    }

    /// Start repeating a consumed key with our own timer, if the compositor
    /// wants us to (rate > 0) and the key repeats at all.
    fn start_repeat(&mut self, key: u32) {
        self.stop_repeat();
        let repeats = self.xkb.as_ref().is_some_and(|xkb| {
            xkb.keymap
                .key_repeats(xkb::Keycode::new(key + EVDEV_OFFSET))
        });
        let Some((delay, interval)) = self.repeat_info.timings() else {
            return;
        };
        if repeats && self.timer.arm(delay, interval).is_ok() {
            self.repeating = Some(key);
        }
    }

    fn stop_repeat(&mut self) {
        if self.repeating.take().is_some() {
            let _ = self.timer.disarm();
        }
    }

    /// Replay a consumed key. If the engine no longer wants it (for example
    /// Backspace once the syllable is gone), hand the key to the application,
    /// which then repeats it by itself.
    fn repeat_key(&mut self, key: u32) {
        let Some(sw) = self.swallowed.get(&key).copied() else {
            return;
        };
        let Some(event) = sw.event else {
            return;
        };
        let Some(engine) = self.engine() else {
            return;
        };
        let output = engine.process_key(event);
        let handled = output.handled;
        self.apply(output);
        if !handled {
            self.swallowed.remove(&key);
            self.stop_repeat();
            self.forward_key(sw.serial, sw.time, key, KeyAction::Pressed);
        }
    }

    fn activate(&mut self, context: ZwpInputMethodContextV1, qh: &QueueHandle<App>) {
        log!("activate");
        self.release_focus();
        self.keyboard = Some(context.grab_keyboard(qh, ()));
        self.context = Some(context);
        for engine in self.engines.values_mut() {
            engine.reset();
        }
        self.swallowed.clear();
        self.toggle.clear();
        self.stop_repeat();
    }

    fn deactivate(&mut self) {
        log!("deactivate");
        self.release_focus();
    }

    /// Finish any composition and drop the current context and keyboard grab.
    fn release_focus(&mut self) {
        self.stop_repeat();
        let leftover = self.engine().map(|e| e.flush()).unwrap_or_default();
        // Release the grab first. The proxy reports version 1 (it inherits the
        // context's), but the compositor's wl_keyboard is newer and the client
        // library does not enforce `since` on requests.
        if let Some(keyboard) = self.keyboard.take() {
            keyboard.release();
        }
        if let Some(context) = self.context.take() {
            // KWin usually ignores text after `deactivate`; the `commit` argument
            // of the last `preedit_string` is what keeps the text. This is a
            // harmless fallback for compositors that do honour it.
            if !leftover.is_empty() {
                context.commit_string(self.commit_serial, leftover);
            }
            context.destroy();
        }
    }

    fn set_keymap(&mut self, file: File, size: u32) {
        let mut buf = vec![0u8; size as usize];
        if let Err(e) = file.read_exact_at(&mut buf, 0) {
            log!("keymap read failed: {e}");
            return;
        }
        // The keymap is a NUL-terminated string.
        while buf.last() == Some(&0) {
            buf.pop();
        }
        let Ok(text) = String::from_utf8(buf) else {
            log!("keymap is not valid UTF-8");
            return;
        };
        let keymap = xkb::Keymap::new_from_string(
            &self.xkb_context,
            text,
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        );
        match keymap {
            Some(keymap) => {
                log!("keymap loaded");
                self.xkb = Some(Xkb {
                    state: xkb::State::new(&keymap),
                    keymap,
                });
            }
            None => log!("keymap compile failed"),
        }
    }

    fn on_modifiers(&mut self, serial: u32, depressed: u32, latched: u32, locked: u32, group: u32) {
        if let Some(xkb) = &mut self.xkb {
            xkb.state
                .update_mask(depressed, latched, locked, 0, 0, group);
        }
        // Applications must see the modifier state for keys we forward.
        if let Some(context) = &self.context {
            context.modifiers(serial, depressed, latched, locked, group);
        }
    }

    /// True if a Shift key is physically held (Caps Lock does not count).
    fn shift_held(&self) -> bool {
        self.xkb.as_ref().is_some_and(|xkb| {
            xkb.state.mod_name_is_active(
                xkb::MOD_NAME_SHIFT,
                xkb::STATE_MODS_DEPRESSED | xkb::STATE_MODS_LATCHED,
            )
        })
    }

    fn current_modifiers(&self) -> Modifiers {
        let Some(xkb) = &self.xkb else {
            return Modifiers::NONE;
        };
        let active = |name| {
            xkb.state
                .mod_name_is_active(name, xkb::STATE_MODS_EFFECTIVE)
        };
        Modifiers {
            ctrl: active(xkb::MOD_NAME_CTRL),
            alt: active(xkb::MOD_NAME_ALT),
            meta: active(xkb::MOD_NAME_LOGO),
        }
    }

    fn on_key(&mut self, serial: u32, time: u32, key: u32, action: KeyAction) {
        match action {
            KeyAction::Released => {
                if self.toggle.on_release(key) == Decision::Toggle {
                    self.toggle_language();
                } else if self.swallowed.remove(&key).is_none() {
                    self.forward_key(serial, time, key, KeyAction::Released);
                }
                if self.repeating == Some(key) {
                    self.stop_repeat();
                }
                return;
            }
            KeyAction::Repeated => {
                // The compositor repeats for us only when our own timer is off.
                if self.swallowed.contains_key(&key) {
                    if self.repeating != Some(key) {
                        self.repeat_key(key);
                    }
                } else if !self.toggle.holds(key) {
                    self.forward_key(serial, time, key, KeyAction::Repeated);
                }
                return;
            }
            KeyAction::Pressed => {
                // The most recently pressed key is the one that repeats.
                self.stop_repeat();
            }
        }

        let keysym = match &self.xkb {
            Some(xkb) => xkb
                .state
                .key_get_one_sym(xkb::Keycode::new(key + EVDEV_OFFSET))
                .raw(),
            None => {
                self.forward_key(serial, time, key, KeyAction::Pressed);
                return;
            }
        };

        let (held_back, decision) = self.toggle.on_press(serial, time, key, keysym);
        if let Some(p) = held_back {
            self.forward_key(p.serial, p.time, p.key, KeyAction::Pressed);
        }
        match decision {
            Decision::Toggle => {
                self.toggle_language();
                self.swallowed.insert(
                    key,
                    Swallowed {
                        event: None,
                        serial,
                        time,
                    },
                );
                return;
            }
            Decision::Swallow => return,
            Decision::Pass => {}
        }
        if is_modifier_keysym(keysym) {
            self.forward_key(serial, time, key, KeyAction::Pressed);
            return;
        }

        let keysym = case_from_shift(keysym, self.shift_held());
        let event = translate(keysym, self.current_modifiers());
        let Some(engine) = self.engine() else {
            // English: the application gets the key untouched.
            self.forward_key(serial, time, key, KeyAction::Pressed);
            return;
        };
        let output = engine.process_key(event);
        let handled = output.handled;
        self.apply(output);
        if handled {
            self.swallowed.insert(
                key,
                Swallowed {
                    event: Some(event),
                    serial,
                    time,
                },
            );
            self.start_repeat(key);
        } else {
            self.forward_key(serial, time, key, KeyAction::Pressed);
        }
    }

    fn toggle_language(&mut self) {
        // Finish the text in progress before switching.
        let leftover = self.engine().map(|e| e.flush()).unwrap_or_default();
        let language = self.switcher.advance();
        log!("language -> {language:?}");
        self.apply(EngineOutput {
            commit: leftover,
            ..EngineOutput::default()
        });
    }

    fn forward_key(&self, serial: u32, time: u32, key: u32, action: KeyAction) {
        if let Some(context) = &self.context {
            let state = match action {
                KeyAction::Pressed => u32::from(KeyState::Pressed),
                KeyAction::Released => u32::from(KeyState::Released),
                KeyAction::Repeated => 2,
            };
            context.key(serial, time, key, state);
        }
    }

    /// Send the engine's result to the application.
    fn apply(&self, output: EngineOutput) {
        let Some(context) = &self.context else {
            return;
        };
        let serial = self.commit_serial;
        if !output.commit.is_empty() {
            context.commit_string(serial, output.commit);
        }
        // Styling and cursor must precede the `preedit_string` they apply to.
        let text = output.preedit.text;
        if !text.is_empty() {
            context.preedit_styling(0, text.len() as u32, PREEDIT_STYLE_UNDERLINE);
        }
        context.preedit_cursor(text.len() as i32);
        context.preedit_string(serial, text.clone(), text);
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for App {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as wayland_client::Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpInputMethodV1, ()> for App {
    fn event(
        app: &mut Self,
        _: &ZwpInputMethodV1,
        event: zwp_input_method_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            zwp_input_method_v1::Event::Activate { id } => app.activate(id, qh),
            zwp_input_method_v1::Event::Deactivate { .. } => app.deactivate(),
        }
    }

    event_created_child!(App, ZwpInputMethodV1, [
        zwp_input_method_v1::EVT_ACTIVATE_OPCODE => (ZwpInputMethodContextV1, ()),
    ]);
}

impl Dispatch<ZwpInputMethodContextV1, ()> for App {
    fn event(
        app: &mut Self,
        _: &ZwpInputMethodContextV1,
        event: zwp_input_method_context_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_input_method_context_v1::Event::CommitState { serial } => {
                app.commit_serial = serial;
            }
            zwp_input_method_context_v1::Event::Reset => {
                // The application discarded its text state.
                if let Some(engine) = app.engine() {
                    engine.reset();
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlKeyboard, ()> for App {
    fn event(
        app: &mut Self,
        _: &WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                if format == WEnum::Value(KeymapFormat::XkbV1) {
                    app.set_keymap(File::from(fd), size);
                }
            }
            wl_keyboard::Event::Key {
                serial,
                time,
                key,
                state,
            } => {
                if let Some(action) = KeyAction::from_wire(state) {
                    app.on_key(serial, time, key, action);
                }
            }
            wl_keyboard::Event::Modifiers {
                serial,
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
            } => app.on_modifiers(serial, mods_depressed, mods_latched, mods_locked, group),
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                log!("repeat_info rate={rate} delay={delay}");
                app.repeat_info = RepeatInfo { rate, delay };
            }
            _ => {}
        }
    }
}

/// Why [`run`] stopped.
#[derive(Debug)]
pub enum RunError {
    /// Could not start (no compositor, or no input method interface because
    /// easime was not launched by KWin). Restarting would not help.
    Startup(String),
    /// Failed after startup (e.g. the connection dropped). The process should
    /// die abnormally so KWin starts it again.
    Runtime(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Startup(m) | RunError::Runtime(m) => f.write_str(m),
        }
    }
}

impl Error for RunError {}

/// Connect to the compositor and serve input until the connection closes.
///
/// Fails with [`RunError::Startup`] if the compositor does not offer
/// `zwp_input_method_v1`; KWin only does so to the program configured as the
/// virtual keyboard.
pub fn run() -> Result<std::convert::Infallible, RunError> {
    log::init();
    log!("easime starting");
    let config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            log!("settings not loaded, using defaults: {e}");
            Config::default()
        }
    };
    let connection = Connection::connect_to_env().map_err(|e| RunError::Startup(e.to_string()))?;
    run_with(&connection, &config)
}

/// Like [`run`], but on an already established connection with default
/// settings (used by tests with a fake compositor).
pub fn run_on(connection: &Connection) -> Result<std::convert::Infallible, RunError> {
    run_with(connection, &Config::default())
}

/// Serve input on `connection` using the given settings.
pub fn run_with(
    connection: &Connection,
    config: &Config,
) -> Result<std::convert::Infallible, RunError> {
    let startup = |e: &dyn std::fmt::Display| RunError::Startup(e.to_string());
    let (globals, mut queue) = registry_queue_init::<App>(connection).map_err(|e| startup(&e))?;
    let qh = queue.handle();
    let _input_method: ZwpInputMethodV1 = globals.bind(&qh, 1..=1, ()).map_err(|e| {
        RunError::Startup(format!(
            "zwp_input_method_v1 is not available ({e}); start easime via \
             System Settings > Keyboard > Virtual Keyboard"
        ))
    })?;
    log!("bound zwp_input_method_v1");

    let mut app = App::new(config).map_err(|e| startup(&e))?;
    event_loop(&mut queue, &mut app)
}

/// Dispatch Wayland events and timer expirations until something fails.
fn event_loop(
    queue: &mut EventQueue<App>,
    app: &mut App,
) -> Result<std::convert::Infallible, RunError> {
    let runtime = |e: &dyn std::fmt::Display| RunError::Runtime(e.to_string());
    loop {
        queue.dispatch_pending(app).map_err(|e| runtime(&e))?;

        // Push our requests out; if the socket is full, also wait for POLLOUT.
        let want_write = match queue.flush() {
            Ok(()) => false,
            Err(WaylandError::Io(e)) if e.kind() == ErrorKind::WouldBlock => true,
            Err(e) => return Err(runtime(&e)),
        };

        // `None` means events are already queued; dispatch them first.
        let Some(guard) = queue.prepare_read() else {
            continue;
        };

        let mut wayland_flags = PollFlags::IN;
        if want_write {
            wayland_flags |= PollFlags::OUT;
        }
        let (wayland_ready, timer_ready) = {
            let wayland_fd = guard.connection_fd();
            let timer_fd = app.timer.as_fd();
            let mut fds = [
                PollFd::new(&wayland_fd, wayland_flags),
                PollFd::new(&timer_fd, PollFlags::IN),
            ];
            match poll(&mut fds, None) {
                Ok(_) => {}
                Err(Errno::INTR) => continue,
                Err(e) => return Err(runtime(&e)),
            }
            let ready = PollFlags::IN | PollFlags::ERR | PollFlags::HUP;
            (
                fds[0].revents().intersects(ready),
                fds[1].revents().intersects(PollFlags::IN),
            )
        };

        if wayland_ready {
            match guard.read() {
                Ok(_) => {}
                Err(WaylandError::Io(e)) if e.kind() == ErrorKind::WouldBlock => {}
                Err(e) => return Err(runtime(&e)),
            }
        } else {
            drop(guard);
        }
        if timer_ready {
            app.on_timer();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_key_states() {
        assert_eq!(
            KeyAction::from_wire(WEnum::Value(KeyState::Pressed)),
            Some(KeyAction::Pressed)
        );
        assert_eq!(
            KeyAction::from_wire(WEnum::Value(KeyState::Released)),
            Some(KeyAction::Released)
        );
        // `repeated` is raw value 2 in the wire protocol.
        assert_eq!(
            KeyAction::from_wire(WEnum::Unknown(2)),
            Some(KeyAction::Repeated)
        );
        assert_eq!(KeyAction::from_wire(WEnum::Unknown(9)), None);
    }
}

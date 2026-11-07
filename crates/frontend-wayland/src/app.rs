//! The input method client: talks `zwp_input_method_v1` with KWin.
//!
//! Flow (see the protocol XML for details):
//! 1. KWin sends `activate` when a text field gets focus, handing us a
//!    `zwp_input_method_context_v1`.
//! 2. We `grab_keyboard`, so every key goes to us first.
//! 3. For each key we either consume it (engine composes, we send
//!    `commit_string` / `preedit_string`) or forward it back with `key`.

use crate::log;
use crate::panel::Panel;
use crate::protocol::client::zwp_input_method_context_v1::{self, ZwpInputMethodContextV1};
use crate::protocol::client::zwp_input_method_v1::{self, ZwpInputMethodV1};
use crate::protocol::client::zwp_input_panel_surface_v1::ZwpInputPanelSurfaceV1;
use crate::protocol::client::zwp_input_panel_v1::ZwpInputPanelV1;
use crate::repeat::RepeatInfo;
use crate::timer::Timer;
use crate::toggle::{Decision, ToggleTracker};
use crate::translate::{case_from_shift, is_modifier_keysym, translate};
use crate::watch::ConfigWatcher;
use easime_config::{BackspaceUnit, Config};
use engine_hangul::{BackspaceMode, HangulEngine};
use engine_japanese::{Dictionary, JapaneseEngine};
use ime_core::{EngineOutput, InputEngine, Key, KeyEvent, Language, LanguageSwitcher, Modifiers};
use rustix::event::{PollFd, PollFlags, poll};
use rustix::io::Errno;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::ErrorKind;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wayland_client::backend::WaylandError;
use wayland_client::globals::{GlobalList, GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_keyboard::{self, KeyState, KeymapFormat, WlKeyboard};
use wayland_client::protocol::wl_region::WlRegion;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::WlSurface;
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
            // wl_keyboard v10 `repeated`.
            WEnum::Value(KeyState::Repeated) => Some(Self::Repeated),
            // Defensive: the same value as raw wire data.
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

pub(crate) struct App {
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
    /// Where settings are re-read from when the file changes, if watched.
    settings: Option<Settings>,
    /// The language indicator box; `None` if the compositor cannot show it.
    panel: Option<Panel>,
    indicator: IndicatorSettings,
    /// Fires when the indicator box has been shown long enough.
    indicator_timer: Timer,
    candidates_visible: bool,
    /// Kept for engines built when the settings change.
    dictionary: JapaneseDictionary,
}

/// The `[appearance]` settings the indicator box needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IndicatorSettings {
    enabled: bool,
    duration: Duration,
}

impl IndicatorSettings {
    fn from_config(config: &Config) -> Self {
        Self {
            enabled: config.appearance.indicator_box,
            duration: Duration::from_millis(u64::from(config.appearance.indicator_ms)),
        }
    }
}

/// A settings file we reload when it changes.
struct Settings {
    path: PathBuf,
    watcher: ConfigWatcher,
}

/// The Japanese dictionary, shared by every Japanese engine we build.
///
/// Settings changes rebuild the engines, but the dictionary (tens of
/// megabytes, about 0.1 s to open) is only loaded once, when the first
/// Japanese engine is needed.
pub(crate) struct JapaneseDictionary {
    loaded: Option<Arc<dyn Dictionary>>,
}

impl JapaneseDictionary {
    /// Load the installed dictionaries when first needed.
    pub(crate) fn installed() -> Self {
        Self { loaded: None }
    }

    /// Use `dictionary` and never look at the disk (tests).
    pub(crate) fn fixed(dictionary: Arc<dyn Dictionary>) -> Self {
        Self {
            loaded: Some(dictionary),
        }
    }

    fn get(&mut self) -> Arc<dyn Dictionary> {
        self.loaded.get_or_insert_with(load_installed).clone()
    }
}

/// Open the dictionaries in the user and system directories and say in the
/// log what was found: with no dictionary conversion only offers kana, and
/// the log is where that can be told apart from "the word is not in it".
fn load_installed() -> Arc<dyn Dictionary> {
    let started = Instant::now();
    let dirs = engine_japanese::loader::default_dictionary_dirs();
    let report = engine_japanese::load_dictionaries(&dirs);
    for (path, why) in &report.failed {
        log!(
            "japanese dictionary: could not load {}: {why}",
            path.display()
        );
    }
    if report.skipped_lines > 0 {
        log!(
            "japanese dictionary: skipped {} malformed lines in text files",
            report.skipped_lines
        );
    }
    if report.loaded.is_empty() {
        let searched: Vec<_> = dirs.iter().map(|d| d.display().to_string()).collect();
        log!(
            "japanese dictionary: none loaded (looked in {}); conversion offers only kana",
            searched.join(", ")
        );
    } else {
        let files: Vec<_> = report
            .loaded
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        log!(
            "japanese dictionary: loaded {} in {} ms",
            files.join(", "),
            started.elapsed().as_millis()
        );
    }
    Arc::new(report.dictionary)
}

/// Engines for the enabled languages and the cycle over them.
///
/// Only languages that have an engine take part in the cycle.
fn build_engines(
    config: &Config,
    dictionary: &mut JapaneseDictionary,
) -> (HashMap<Language, Box<dyn InputEngine>>, LanguageSwitcher) {
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
                engines.insert(
                    language,
                    Box::new(JapaneseEngine::with_dictionary(Box::new(dictionary.get()))),
                );
            }
            Language::English | Language::Chinese => {
                log!("language {language:?} is not available yet, ignoring");
            }
        }
    }
    let usable: Vec<Language> = config
        .languages
        .enabled
        .iter()
        .copied()
        .filter(|l| engines.contains_key(l))
        .collect();
    (engines, LanguageSwitcher::new(&usable))
}

impl App {
    fn new(config: &Config, mut dictionary: JapaneseDictionary) -> std::io::Result<Self> {
        let (engines, switcher) = build_engines(config, &mut dictionary);
        Ok(Self {
            engines,
            switcher,
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
            settings: None,
            panel: None,
            indicator: IndicatorSettings::from_config(config),
            indicator_timer: Timer::new()?,
            candidates_visible: false,
            dictionary,
        })
    }

    /// Switch to new settings while running. Text in progress is committed
    /// first, and the current language is kept if it is still enabled.
    fn apply_config(&mut self, config: &Config) {
        log!("applying new settings");
        let leftover = self.engine().map(|e| e.flush()).unwrap_or_default();
        let previous = self.switcher.current();
        self.stop_repeat();

        let (engines, mut switcher) = build_engines(config, &mut self.dictionary);
        switcher.select(previous);
        self.engines = engines;
        self.switcher = switcher;
        self.toggle = ToggleTracker::with_setting(config.shortcuts.toggle_key);
        self.set_indicator(IndicatorSettings::from_config(config));

        self.apply(EngineOutput {
            commit: leftover,
            ..EngineOutput::default()
        });
    }

    /// Take new indicator settings: turning it off hides a visible box.
    fn set_indicator(&mut self, settings: IndicatorSettings) {
        self.indicator = settings;
        if !settings.enabled {
            self.hide_indicator();
        } else if let Some(panel) = &self.panel {
            panel.prepare();
        }
    }

    /// Flash the box for the current language next to the text cursor, if
    /// that is enabled, possible, and a text field has focus.
    fn show_indicator(&mut self) {
        if !self.indicator.enabled || self.context.is_none() || self.candidates_visible {
            return;
        }
        let Some(panel) = &mut self.panel else {
            return;
        };
        panel.show(self.switcher.current());
        // A zero duration would disarm the timer; show it for a moment anyway.
        let duration = self.indicator.duration.max(Duration::from_millis(1));
        if self.indicator_timer.arm(duration, Duration::ZERO).is_err() {
            // Without a timer the box could never go away.
            panel.hide();
        }
    }

    fn hide_indicator(&mut self) {
        let _ = self.indicator_timer.disarm();
        if !self.candidates_visible
            && let Some(panel) = &mut self.panel
        {
            panel.hide();
        }
    }

    /// The indicator timer fired.
    fn on_indicator_timer(&mut self) {
        if self.indicator_timer.take_expirations().unwrap_or(0) > 0 {
            self.hide_indicator();
        }
    }

    /// The settings file changed: reload it and apply the result.
    fn on_settings_event(&mut self) {
        let Some(settings) = &self.settings else {
            return;
        };
        match settings.watcher.take_changed() {
            Ok(true) => {}
            Ok(false) => return,
            Err(e) => {
                log!("settings watcher failed: {e}");
                return;
            }
        }
        match Config::load_from(&settings.path) {
            Ok(config) => self.apply_config(&config),
            // Keep what we have: a half-edited file must not break typing.
            Err(e) => log!("ignoring unreadable settings: {e}"),
        }
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
        // Without a focused field KWin has no cursor to put the box next to.
        self.hide_indicator();
    }

    /// Finish any composition and drop the current context and keyboard grab.
    fn release_focus(&mut self) {
        self.candidates_visible = false;
        self.hide_indicator();
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
        if event.key == Key::Hanja && !self.panel.as_ref().is_some_and(Panel::can_show_candidates) {
            // Keep the composition intact when conversion cannot be shown.
            self.forward_key(serial, time, key, KeyAction::Pressed);
            return;
        }
        let was_converting = self.candidates_visible;
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
                    event: if event.key == Key::Hanja
                        || (was_converting
                            && matches!(event.key, Key::Enter | Key::Escape | Key::Char('1'..='9')))
                    {
                        None
                    } else {
                        Some(event)
                    },
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
        self.show_indicator();
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
    fn apply(&mut self, mut output: EngineOutput) {
        if let Some(list) = output.candidates.as_ref() {
            let language = self.switcher.current();
            // Korean keeps the Hangul on screen while converting, so the preedit
            // is the reading; Japanese shows the chosen candidate there.
            let reading = if list.reading.is_empty() {
                output.preedit.text.as_str()
            } else {
                list.reading.as_str()
            };
            let shown = self
                .panel
                .as_mut()
                .is_some_and(|panel| panel.show_candidates(list, reading, language));
            if shown {
                let _ = self.indicator_timer.disarm();
                self.candidates_visible = true;
            } else {
                // Never allow a selection that the user could not see.
                if let Some(engine) = self.engine() {
                    output = engine.process_key(KeyEvent::new(Key::Escape));
                }
                self.candidates_visible = false;
                if let Some(panel) = &mut self.panel {
                    panel.hide();
                }
            }
        } else if self.candidates_visible {
            self.candidates_visible = false;
            if let Some(panel) = &mut self.panel {
                panel.hide();
            }
        }
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

/// Objects of the indicator box that never send events we care about.
macro_rules! ignore_events {
    ($($interface:ty),* $(,)?) => {$(
        impl Dispatch<$interface, ()> for App {
            fn event(
                _: &mut Self,
                _: &$interface,
                _: <$interface as wayland_client::Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        }
    )*};
}

ignore_events!(
    WlCompositor,
    WlShm,
    WlShmPool,
    WlBuffer,
    WlSurface,
    WlRegion,
    ZwpInputPanelV1,
    ZwpInputPanelSurfaceV1,
);

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
                app.candidates_visible = false;
                app.hide_indicator();
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
    run_watching(
        &connection,
        &config,
        easime_config::config_path().as_deref(),
    )
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
    run_watching(connection, config, None)
}

/// Like [`run_with`], and also reload settings from `settings_path` whenever
/// that file changes.
pub fn run_watching(
    connection: &Connection,
    config: &Config,
    settings_path: Option<&Path>,
) -> Result<std::convert::Infallible, RunError> {
    serve(
        connection,
        config,
        settings_path,
        JapaneseDictionary::installed(),
    )
}

/// Like [`run_watching`], but Japanese conversion uses `dictionary` instead of
/// the files installed on this machine (used by tests, so that they do not
/// depend on what happens to be installed).
pub fn run_with_dictionary(
    connection: &Connection,
    config: &Config,
    settings_path: Option<&Path>,
    dictionary: Arc<dyn Dictionary>,
) -> Result<std::convert::Infallible, RunError> {
    serve(
        connection,
        config,
        settings_path,
        JapaneseDictionary::fixed(dictionary),
    )
}

fn serve(
    connection: &Connection,
    config: &Config,
    settings_path: Option<&Path>,
    dictionary: JapaneseDictionary,
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

    let mut app = App::new(config, dictionary).map_err(|e| startup(&e))?;
    app.panel = bind_panel(&globals, &qh);
    if app.indicator.enabled
        && let Some(panel) = &app.panel
    {
        panel.prepare();
    }
    if let Some(path) = settings_path {
        match ConfigWatcher::new(path) {
            Ok(watcher) => {
                app.settings = Some(Settings {
                    path: path.to_path_buf(),
                    watcher,
                });
            }
            Err(e) => log!("not watching settings ({e}); changes need a restart"),
        }
    }
    event_loop(&mut queue, &mut app)
}

/// The indicator box, if the compositor offers everything it needs. The
/// daemon works without it.
fn bind_panel(globals: &GlobalList, qh: &QueueHandle<App>) -> Option<Panel> {
    let compositor: Result<WlCompositor, _> = globals.bind(qh, 1..=4, ());
    let shm: Result<WlShm, _> = globals.bind(qh, 1..=1, ());
    let input_panel: Result<ZwpInputPanelV1, _> = globals.bind(qh, 1..=1, ());
    match (compositor, shm, input_panel) {
        (Ok(compositor), Ok(shm), Ok(input_panel)) => {
            log!("bound zwp_input_panel_v1, wl_compositor and wl_shm");
            Some(Panel::new(compositor, shm, input_panel, qh.clone()))
        }
        (c, s, i) => {
            log!(
                "no indicator box: wl_compositor {}, wl_shm {}, zwp_input_panel_v1 {}",
                if c.is_ok() { "ok" } else { "missing" },
                if s.is_ok() { "ok" } else { "missing" },
                if i.is_ok() { "ok" } else { "missing" },
            );
            None
        }
    }
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
        let (wayland_ready, timer_ready, indicator_ready, settings_ready) = {
            let wayland_fd = guard.connection_fd();
            let timer_fd = app.timer.as_fd();
            let indicator_fd = app.indicator_timer.as_fd();
            let mut fds = vec![
                PollFd::new(&wayland_fd, wayland_flags),
                PollFd::new(&timer_fd, PollFlags::IN),
                PollFd::new(&indicator_fd, PollFlags::IN),
            ];
            let settings_fd = app.settings.as_ref().map(|s| s.watcher.as_fd());
            if let Some(fd) = &settings_fd {
                fds.push(PollFd::new(fd, PollFlags::IN));
            }
            match poll(&mut fds, None) {
                Ok(_) => {}
                Err(Errno::INTR) => continue,
                Err(e) => return Err(runtime(&e)),
            }
            let ready = PollFlags::IN | PollFlags::ERR | PollFlags::HUP;
            (
                fds[0].revents().intersects(ready),
                fds[1].revents().intersects(PollFlags::IN),
                fds[2].revents().intersects(PollFlags::IN),
                fds.get(3)
                    .is_some_and(|f| f.revents().intersects(PollFlags::IN)),
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
        if indicator_ready {
            app.on_indicator_timer();
        }
        if settings_ready {
            app.on_settings_event();
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
        // `repeated` is what the generated enum produces for wire value 2.
        assert_eq!(
            KeyAction::from_wire(WEnum::Value(KeyState::Repeated)),
            Some(KeyAction::Repeated)
        );
        assert_eq!(
            KeyAction::from_wire(WEnum::Unknown(2)),
            Some(KeyAction::Repeated)
        );
        assert_eq!(KeyAction::from_wire(WEnum::Unknown(9)), None);
    }
}

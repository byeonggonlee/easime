//! The text service itself: the COM object Windows creates in every
//! application that lets the user type, and the factory that makes it.
//!
//! **What it does.** Windows asks about every key twice, `OnTestKeyDown` ("would
//! you take it?") and `OnKeyDown` (take it). Both go to the [`Typist`] (the part
//! with no Windows API in it, [`crate::typing`]), which decides. What comes back
//! is an edit of the document: finished text, and the text being composed. The
//! composition is a TSF composition: its text is in the document, and the
//! application shows it as composing. The document is only touched inside an
//! *edit session* that TSF grants (`RequestEditSession`), which is what
//! [`EditSession`] is.
//!
//! The composition is underlined through TSF display attributes
//! ([`crate::display`]); the service is their provider. Candidates go to its
//! window, or to an application that draws them itself ([`crate::element`]).
//!
//! The language is also a button in the notification area of the taskbar
//! ([`crate::langbar`]); a click there is not a key, so the service handles it
//! itself (`tray_request`).
//!
//! **What it does not do yet:** share the mode between programs. See
//! `docs/platforms.md`.
//!
//! One service lives per thread of an application, so each has its own typist,
//! and its mode (English or Korean) is its own. Everything here runs on that
//! one thread, so plain cells are enough; no cell is borrowed while a call goes
//! out to the application, because the application may call back in.
//!
//! A panic must never take the application down with it: every entry point
//! catches it, writes it to the log and says "not eaten".

use crate::candidate;
use crate::diag;
use crate::display;
use crate::element::Candidates;
use crate::indicator::{self, Indicator};
use crate::install;
use crate::keys::{self, KeyInfo, KeyboardState};
use crate::langbar::{self, LangBar, Request};
use crate::marks::Mark;
use crate::page;
use crate::placement::Rect;
use crate::profiles;
use crate::typing::{Outcome, Step, Typist, plan};
use easime_config::{Appearance, Config};
use engine_japanese::loader::Access;
use ime_session::{ConfigReloader, JapaneseDictionary, Session};
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{
    CLASS_E_NOAGGREGATION, E_FAIL, E_INVALIDARG, E_POINTER, HWND, LPARAM, WPARAM,
};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardState;
use windows::Win32::UI::TextServices::{
    GUID_COMPARTMENT_EMPTYCONTEXT, GUID_COMPARTMENT_KEYBOARD_DISABLED, IEnumTfDisplayAttributeInfo,
    ITfCompartmentMgr, ITfComposition, ITfCompositionSink, ITfCompositionSink_Impl, ITfContext,
    ITfContextComposition, ITfContextView, ITfDisplayAttributeInfo, ITfDisplayAttributeProvider,
    ITfDisplayAttributeProvider_Impl, ITfDocumentMgr, ITfEditSession, ITfEditSession_Impl,
    ITfFnConfigure, ITfFnConfigure_Impl, ITfFunction_Impl, ITfInsertAtSelection, ITfKeyEventSink,
    ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfLangBarItemMgr, ITfSource, ITfTextInputProcessor,
    ITfTextInputProcessor_Impl, ITfTextInputProcessorEx, ITfTextInputProcessorEx_Impl,
    ITfThreadMgr, ITfThreadMgrEventSink, ITfThreadMgrEventSink_Impl, ITfThreadMgrEx,
    ITfUIElementMgr, TF_AE_NONE, TF_ANCHOR_END, TF_CONTEXT_EDIT_CONTEXT_FLAGS,
    TF_DEFAULT_SELECTION, TF_E_SYNCHRONOUS, TF_ES_ASYNCDONTCARE, TF_ES_READ, TF_ES_READWRITE,
    TF_ES_SYNC, TF_IAS_QUERYONLY, TF_SELECTION, TF_SELECTIONSTYLE, TF_TMF_UIELEMENTENABLEDONLY,
};
use windows::core::{BOOL, BSTR, GUID, IUnknown, IUnknownImpl, Interface, Ref, Result, implement};

/// How many objects of this DLL are alive (text services and their factories),
/// for `DllCanUnloadNow`.
static IN_USE: AtomicIsize = AtomicIsize::new(0);
/// How many `LockServer(TRUE)` calls are not yet undone.
static LOCKS: AtomicIsize = AtomicIsize::new(0);

/// Whether nothing of this DLL is in use: Windows may unload it only then.
pub fn unloadable() -> bool {
    IN_USE.load(Ordering::SeqCst) == 0 && LOCKS.load(Ordering::SeqCst) == 0
}

/// Counts an object of this DLL as alive for as long as it lives (a field of
/// the COM objects TSF may keep, such as the display attributes).
pub(crate) struct Alive;

impl Alive {
    pub(crate) fn new() -> Self {
        IN_USE.fetch_add(1, Ordering::SeqCst);
        Alive
    }
}

impl Drop for Alive {
    fn drop(&mut self) {
        IN_USE.fetch_sub(1, Ordering::SeqCst);
    }
}

/// How often the settings file is looked at for changes.
const CONFIG_POLL: Duration = Duration::from_secs(2);

/// Run `f`; if it panics, log that and answer `fallback` instead of letting the
/// panic cross into the application.
fn guarded<T>(what: &str, fallback: T, f: impl FnOnce() -> T) -> T {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(_) => {
            diag::log(&format!("panic in {what}; the key is passed on"));
            fallback
        }
    }
}

fn keyboard_state() -> KeyboardState {
    let mut state = [0u8; 256];
    // SAFETY: the buffer is the 256 bytes the call wants. A failure leaves it
    // zeroed: no modifier held.
    let _ = unsafe { GetKeyboardState(&mut state) };
    state
}

/// Whether the application asks text services to leave this context's keys
/// alone: a password field (keyboard disabled) or a place that takes no text,
/// such as a web page itself (an empty context, where letters are shortcuts).
/// TSF leaves honoring this to the text service.
fn keys_not_ours(context: Option<&ITfContext>) -> bool {
    why_not_ours(context).is_some()
}

/// Why the application asks text services to leave this context's keys alone,
/// in words for the log; `None` if it does not.
fn why_not_ours(context: Option<&ITfContext>) -> Option<&'static str> {
    let context = context?;
    let compartments = context.cast::<ITfCompartmentMgr>().ok()?;
    [
        (GUID_COMPARTMENT_KEYBOARD_DISABLED, "keyboard disabled"),
        (GUID_COMPARTMENT_EMPTYCONTEXT, "empty context"),
    ]
    .iter()
    .find(|(which, _)| {
        // SAFETY: the compartment manager is alive; the GUID lives for the call.
        unsafe { compartments.GetCompartment(which) }
            .and_then(|compartment| unsafe { compartment.GetValue() })
            .ok()
            .and_then(|value| i32::try_from(&value).ok())
            .is_some_and(|value| value != 0)
    })
    .map(|(_, why)| *why)
}

/// What TSF gave the service when it was activated.
struct Activated {
    thread_manager: ITfThreadMgr,
    client_id: u32,
    /// Where the thread sink was registered, to take it away again.
    thread_sink_cookie: u32,
}

/// The composition that is going on, and the context it is in.
struct Composing {
    context: ITfContext,
    composition: ITfComposition,
}

/// A syllable the engine still holds as its composition while the document has
/// it as finished text, because the application ended the composition (Excel's
/// cell editor does after every key). The next key takes it up again if the text
/// is still just before the cursor of the same field: the new text then replaces
/// it, instead of standing next to it.
struct Orphan {
    context: ITfContext,
    text: String,
}

/// The text service of one thread of one application.
#[implement(
    ITfTextInputProcessorEx,
    ITfKeyEventSink,
    ITfCompositionSink,
    ITfThreadMgrEventSink,
    ITfDisplayAttributeProvider,
    ITfFnConfigure
)]
pub struct TextService {
    activated: RefCell<Option<Activated>>,
    typist: RefCell<Option<Typist>>,
    reloader: RefCell<Option<ConfigReloader>>,
    /// The box that says which language is on, and whether and how long to show it.
    indicator: RefCell<Indicator>,
    /// The candidates while a conversion is going on.
    candidates: Candidates,
    /// The application shows text service windows only as TSF UI elements
    /// (some full-screen programs): the mode box, which is not one, stays away.
    ui_elements_only: Cell<bool>,
    appearance: RefCell<Appearance>,
    /// Whether the log already says that no cursor position could be found.
    told_no_caret: Cell<bool>,
    /// What the log last said about whether the keys of the text field are
    /// ours: `None` = nothing yet, `Some(None)` = ours, `Some(Some(why))` = the
    /// application asked us to stay out of the way.
    told_keys: Cell<Option<Option<&'static str>>>,
    /// The syllable the application ended the composition of, if the engine
    /// still holds it.
    orphan: RefCell<Option<Orphan>>,
    /// Shared with the edit sessions, which start and end compositions.
    composing: Rc<RefCell<Option<Composing>>>,
    /// TSF's numbers for the underlines, once asked for (`None` if TSF would
    /// not give them: the composition is then not underlined).
    atoms: Cell<Option<Option<[u32; 3]>>>,
    /// The button in the notification area ([`crate::langbar`]).
    langbar: RefCell<Option<LangBar>>,
    /// The service as the sink of its own compositions, kept from the
    /// activation on: what starts outside a key (the language bar) edits the
    /// document too, and the edit needs the sink.
    self_sink: RefCell<Option<ITfCompositionSink>>,
}

impl TextService {
    pub fn new() -> Self {
        IN_USE.fetch_add(1, Ordering::SeqCst);
        Self {
            activated: RefCell::new(None),
            typist: RefCell::new(None),
            reloader: RefCell::new(None),
            indicator: RefCell::new(Indicator::default()),
            candidates: Candidates::default(),
            ui_elements_only: Cell::new(false),
            appearance: RefCell::new(Appearance::default()),
            told_no_caret: Cell::new(false),
            told_keys: Cell::new(None),
            orphan: RefCell::new(None),
            composing: Rc::new(RefCell::new(None)),
            atoms: Cell::new(None),
            langbar: RefCell::new(None),
            self_sink: RefCell::new(None),
        }
    }
}

impl Default for TextService {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TextService {
    fn drop(&mut self) {
        IN_USE.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The language bar menu's entry that opens the settings, in the language of
/// the user's Windows (Korean or English).
fn langbar_settings_text() -> String {
    let korean = easime_config::system_language_tags()
        .first()
        .is_some_and(|tag| tag.to_ascii_lowercase().starts_with("ko"));
    if korean {
        "easime 설정…".to_owned()
    } else {
        "easime settings…".to_owned()
    }
}

/// The settings the cycle runs with (see [`profiles::effective_config`]).
fn cycle_config(config: &Config) -> Config {
    profiles::effective_config(
        config,
        &easime_config::system_languages(),
        &profiles::TYPEABLE,
        profiles::HOME,
    )
}

/// The file name of the program the service runs in, for the log (which of the
/// programs that share the log a line is about).
fn program_name() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "an unknown program".to_owned())
}

/// The input method this thread starts with: the user's settings, with the
/// languages of the one easime profile as the cycle (English, then each language).
fn make_typist() -> (Typist, Option<ConfigReloader>, Appearance) {
    let config = Config::load().unwrap_or_else(|why| {
        diag::log(&format!("settings not read, using the defaults: {why}"));
        Config::default()
    });
    let cycle = cycle_config(&config);
    diag::log(&format!(
        "start in {} (build {}): languages {:?}, switch key {:?}",
        program_name(),
        env!("EASIME_COMMIT"),
        cycle.languages.enabled.clone().unwrap_or_default(),
        config.shortcuts.toggle_key
    ));
    // Opened when a word is first converted, mapped so that every application
    // using it shares one copy, from the folder next to this DLL.
    let dictionary = JapaneseDictionary::on_first_lookup(
        install::dictionary_dirs(
            engine_japanese::loader::user_dictionary_dir(),
            crate::exports::dll_path().as_deref(),
        ),
        Access::Map,
        diag::log,
    );
    let session = Session::new(&cycle, dictionary, Box::new(diag::log));
    let reloader = easime_config::config_path().map(|path| ConfigReloader::new(path, CONFIG_POLL));
    let mut typist = Typist::new(session, config.shortcuts.toggle_key);
    // The service shows candidates itself (`candidate`); a conversion whose
    // window cannot be shown is taken back (`show_candidates`).
    typist.set_candidate_window(true);
    (typist, reloader, config.appearance)
}

impl TextService_Impl {
    /// Start serving: take the key sink and the thread sink, and remember what
    /// TSF gave us.
    fn activate(&self, thread_manager: &ITfThreadMgr, client_id: u32) -> Result<()> {
        if self.activated.borrow().is_some() {
            return Ok(());
        }
        let sink: ITfKeyEventSink = self.to_interface();
        let keystrokes: ITfKeystrokeMgr = thread_manager.cast()?;
        // SAFETY: both interfaces are alive for the call.
        unsafe { keystrokes.AdviseKeyEventSink(client_id, &sink, true) }
            .map_err(|e| windows::core::Error::new(e.code(), format!("AdviseKeyEventSink: {e}")))?;

        let thread_sink: ITfThreadMgrEventSink = self.to_interface();
        let advised = thread_manager.cast::<ITfSource>().and_then(|source| {
            // SAFETY: the sink is alive for the call.
            unsafe { source.AdviseSink(&ITfThreadMgrEventSink::IID, &thread_sink) }
        });
        let cookie = match advised {
            Ok(cookie) => cookie,
            Err(e) => {
                // Activation fails: TSF will not call Deactivate, so the key
                // sink must not stay behind holding this service.
                // SAFETY: the interface is alive for the call.
                let _ = unsafe { keystrokes.UnadviseKeyEventSink(client_id) };
                return Err(windows::core::Error::new(
                    e.code(),
                    format!("AdviseSink(thread): {e}"),
                ));
            }
        };

        // SAFETY: the interface is alive for the call.
        let flags = thread_manager
            .cast::<ITfThreadMgrEx>()
            .and_then(|manager| unsafe { manager.GetActiveFlags() })
            .unwrap_or(0);
        let elements_only = flags & TF_TMF_UIELEMENTENABLEDONLY != 0;
        if elements_only {
            diag::log("this program shows text service windows only as UI elements: no mode box");
        }
        self.ui_elements_only.set(elements_only);

        let (typist, reloader, appearance) = make_typist();
        *self.typist.borrow_mut() = Some(typist);
        *self.reloader.borrow_mut() = reloader;
        *self.appearance.borrow_mut() = appearance;
        *self.activated.borrow_mut() = Some(Activated {
            thread_manager: thread_manager.clone(),
            client_id,
            thread_sink_cookie: cookie,
        });
        *self.self_sink.borrow_mut() = Some(self.to_interface());
        self.add_langbar(thread_manager);
        Ok(())
    }

    /// Put the button on the language bar (and so in the notification area).
    /// Without it the service works as before, so a failure is only logged.
    fn add_langbar(&self, thread_manager: &ITfThreadMgr) {
        let Ok(manager) = thread_manager.cast::<ITfLangBarItemMgr>() else {
            diag::log("language bar: TSF has no manager for it, no tray button");
            return;
        };
        let state = self
            .typist
            .borrow()
            .as_ref()
            .map(|typist| (typist.language(), typist.cycle().to_vec()));
        let Some((language, cycle)) = state else {
            return;
        };
        // The button asks this service for what the user chose. The loop (the
        // service holds the button, the button the service) is cut in
        // `Deactivate`.
        let service = self.to_object();
        let handler = Box::new(move |request: Request| service.get().tray_request(request));
        match LangBar::add(manager, language, cycle, langbar_settings_text(), handler) {
            Ok(bar) => *self.langbar.borrow_mut() = Some(bar),
            Err(e) => diag::log(&format!("language bar: could not add the button: {e}")),
        }
    }
}

/// What does not need the COM object itself: the rest of the service's work,
/// which the language bar button also calls (`tray_request`).
impl TextService {
    fn client_id(&self) -> Option<u32> {
        self.activated.borrow().as_ref().map(|a| a.client_id)
    }

    /// TSF's manager of UI elements, to hand candidates to an application
    /// that draws them itself.
    fn elements(&self) -> Option<ITfUIElementMgr> {
        self.activated
            .borrow()
            .as_ref()
            .and_then(|a| a.thread_manager.cast().ok())
    }

    /// Whether the document has text of ours that is not finished: a composition,
    /// or a syllable the application ended the composition of.
    fn unfinished(&self) -> bool {
        self.composing.borrow().is_some() || self.orphan.borrow().is_some()
    }

    /// Drop the engine's copy of the composition and whatever waits to be taken
    /// up again; the document keeps the text it shows.
    fn forget_engine(&self) {
        if let Ok(mut orphan) = self.orphan.try_borrow_mut() {
            orphan.take();
        }
        if let Ok(mut typist) = self.typist.try_borrow_mut()
            && let Some(typist) = typist.as_mut()
        {
            typist.forget();
        }
    }

    /// After the document was edited: if the engine shows a composition that no
    /// composition of the document carries any more (the application ended it),
    /// remember it as the one to take up again. Conversions are not taken up.
    fn sync_orphan(&self, context: &ITfContext) {
        let carried = self.composing.try_borrow().map_or(true, |c| c.is_some());
        let shown = self
            .typist
            .try_borrow()
            .ok()
            .and_then(|typist| {
                typist
                    .as_ref()
                    .filter(|typist| typist.candidates().is_none())
                    .map(|typist| typist.shown().to_owned())
            })
            .filter(|text| !text.is_empty());
        if let Ok(mut orphan) = self.orphan.try_borrow_mut() {
            *orphan = match shown {
                Some(text) if !carried => Some(Orphan {
                    context: context.clone(),
                    text,
                }),
                _ => None,
            };
        }
    }

    /// Before the engine is asked anything: a syllable waiting to be taken up
    /// again is taken up only if it is still just before the cursor of this
    /// field. If the user moved on, the engine forgets it, so that no syllable
    /// appears out of nothing where they went.
    fn settle_orphan(&self, context: &ITfContext) {
        let waiting = self
            .orphan
            .borrow()
            .as_ref()
            .map(|orphan| (orphan.context.clone(), orphan.text.clone()));
        let Some((field, text)) = waiting else {
            return;
        };
        let verdict = if field == *context {
            self.text_before_cursor_is(context, &text)
        } else {
            Err("another text field".to_owned())
        };
        if let Err(why) = verdict {
            diag::log(&format!("an ended syllable is not taken up again: {why}"));
            self.forget_engine();
        }
    }

    /// Whether the text just before the cursor of `context` is `text` and nothing
    /// is selected; if not, why not (for the log).
    fn text_before_cursor_is(
        &self,
        context: &ITfContext,
        text: &str,
    ) -> std::result::Result<(), String> {
        let Some(client_id) = self.client_id() else {
            return Err("the service is not active".to_owned());
        };
        let verdict = Rc::new(RefCell::new(String::from("the read did not run")));
        let session: ITfEditSession = BeforeCursor {
            context: context.clone(),
            expected: text.encode_utf16().collect(),
            verdict: verdict.clone(),
        }
        .into();
        let flags = TF_CONTEXT_EDIT_CONTEXT_FLAGS(TF_ES_SYNC.0 | TF_ES_READ.0);
        // SAFETY: the context and the session are alive for the call; a read
        // lock is granted at once or the answer is "no".
        let asked = unsafe { context.RequestEditSession(client_id, &session, flags) };
        match asked {
            Ok(result) if result.is_ok() => {}
            Ok(result) => {
                return Err(format!(
                    "the document did not give a read lock ({result:?}); {}",
                    verdict.borrow()
                ));
            }
            Err(e) => return Err(format!("the read failed: {e}")),
        }
        let verdict = verdict.borrow().clone();
        if verdict == "ok" {
            Ok(())
        } else {
            Err(verdict)
        }
    }

    /// Make `outcome`'s edit in the document of `context`. Returns false if
    /// the document could not be edited (the failure is in the log).
    fn perform(&self, context: &ITfContext, outcome: &Outcome) -> bool {
        let mut edited = true;
        if let Some(edit) = &outcome.edit {
            let steps = plan(self.unfinished(), edit);
            let parts = self.parts(&edit.marks);
            if !steps.is_empty()
                && let Err(e) = self.run(context, steps, parts, true)
            {
                diag::log(&format!("could not edit the document: {e}"));
                edited = false;
            }
        }
        // After the edit, so the box sits next to where the cursor is now.
        if let Some(language) = outcome.switched_to {
            diag::log(&format!("language -> {language:?}"));
            self.show_indicator(context, language);
            self.update_langbar();
        }
        edited
    }

    /// The button shows the language and the cycle the typist has now.
    fn update_langbar(&self) {
        let state = self
            .typist
            .borrow()
            .as_ref()
            .map(|typist| (typist.language(), typist.cycle().to_vec()));
        // Outside every borrow of ours: TSF asks the button for its icon now.
        let button = self.langbar.borrow().as_ref().map(LangBar::button);
        if let (Some((language, cycle)), Some(button)) = (state, button) {
            button.get().set(language, cycle);
        }
    }

    /// Say in the log, when it changes, whether the application lets easime have
    /// the keys of the text field in focus. No key is named: it is the answer to
    /// "do the keys reach easime here at all, and if not why", the first thing to
    /// know when typing does nothing in some program.
    fn note_keys(&self, why: Option<&'static str>) {
        let now = Some(why);
        let before = self.told_keys.replace(now);
        if before == now {
            return;
        }
        match (before, why) {
            (None, None) => diag::log("keys: the first key reached easime in this program"),
            (_, Some(why)) => diag::log(&format!(
                "keys: the application asks easime to stay out of the way here ({why})"
            )),
            (Some(Some(_)), None) | (Some(None), None) => {
                diag::log("keys: easime has the keys of this text field")
            }
        }
    }

    /// What the user asked of the button: another language (the text being
    /// composed is finished first, as with the switch key) or the settings app.
    fn tray_request(&self, request: Request) {
        guarded("language bar", (), || {
            if request == Request::Settings {
                langbar::open_settings();
                return;
            }
            if self.orphan.borrow().is_some()
                && let Some(focused) = self.focused_context()
            {
                self.settle_orphan(&focused);
            }
            let outcome = {
                let mut typist = self.typist.borrow_mut();
                let Some(typist) = typist.as_mut() else {
                    return;
                };
                match request {
                    Request::Toggle => typist.toggle_language(),
                    Request::Select(language) => typist.select_language(language),
                    Request::Settings => return,
                }
            };
            if outcome.switched_to.is_none() {
                return;
            }
            // The text of the switch is finished where the composition is, else
            // in the document that has the focus; with neither there is nothing
            // to finish and only the language changes.
            let context = self
                .composing
                .borrow()
                .as_ref()
                .map(|composing| composing.context.clone())
                .or_else(|| self.focused_context());
            match context {
                Some(context) => {
                    self.perform(&context, &outcome);
                }
                None => {
                    if let Some(language) = outcome.switched_to {
                        diag::log(&format!("language -> {language:?} (from the language bar)"));
                    }
                    self.update_langbar();
                }
            }
        });
    }

    /// The text field that has the focus, if the application tells.
    fn focused_context(&self) -> Option<ITfContext> {
        let thread_manager = self.activated.borrow().as_ref()?.thread_manager.clone();
        // SAFETY: the interfaces are alive for the calls.
        let document = unsafe { thread_manager.GetFocus() }.ok()?;
        unsafe { document.GetTop() }.ok()
    }

    /// The service as the sink of its compositions (set from the activation on).
    fn composition_sink(&self) -> Result<ITfCompositionSink> {
        self.self_sink.borrow().clone().ok_or_else(|| E_FAIL.into())
    }

    /// Say which language is on, next to the text cursor, if the user wants it.
    /// Only a switch the user made comes here, never a focus change.
    fn show_indicator(&self, context: &ITfContext, language: ime_core::Language) {
        let appearance = self.appearance.borrow().clone();
        if !appearance.indicator_box || self.ui_elements_only.get() {
            return;
        }
        let caret = self.caret_of(context).or_else(indicator::system_caret);
        let Some(caret) = caret else {
            // Not at some other place: the language is still in the tray and
            // the settings. Say so once in the log instead of every switch.
            if !self.told_no_caret.replace(true) {
                diag::log("mode box: this program gives no text cursor position, so none is shown");
            }
            return;
        };
        let shown = self
            .indicator
            .borrow_mut()
            .show(language, caret, appearance.indicator_ms);
        diag::log_key(|| format!("mode box {language:?} at {shown:?} for {caret:?}"));
    }

    /// Show the candidates the last key left, or end them. If they cannot be
    /// shown, the conversion is taken back: a candidate the user did not see
    /// is never chosen.
    fn show_candidates(&self, context: &ITfContext) {
        let wanted = self.typist.borrow().as_ref().and_then(|typist| {
            let list = typist.candidates()?;
            let page = page::page(list, typist.shown())?;
            Some((list.clone(), page, typist.language()))
        });
        let elements = self.elements();
        let Some((list, page, language)) = wanted else {
            self.candidates.end(elements.as_ref());
            return;
        };
        let shown = self.candidates.show(
            elements.as_ref(),
            // SAFETY: the context is alive for the call.
            || unsafe { context.GetDocumentMgr() }.ok(),
            &list,
            page,
            language,
            || self.caret_of(context).or_else(candidate::fallback_anchor),
        );
        if shown {
            return;
        }
        diag::log("candidates could not be shown; the conversion is taken back");
        let edit = self
            .typist
            .borrow_mut()
            .as_mut()
            .and_then(Typist::candidates_not_shown);
        if let Some(edit) = edit {
            let parts = self.parts(&edit.marks);
            if let Err(e) = self.run(context, plan(self.unfinished(), &edit), parts, true) {
                diag::log(&format!("could not edit the document: {e}"));
            }
        }
        self.candidates.end(elements.as_ref());
    }

    /// Where the text cursor is on the screen, as the application's document says.
    fn caret_of(&self, context: &ITfContext) -> Option<Rect> {
        let client_id = self.client_id()?;
        let found = Rc::new(Cell::new(None));
        let session: ITfEditSession = CaretQuery {
            context: context.clone(),
            found: found.clone(),
        }
        .into();
        // SAFETY: the context and the session are alive for the call.
        let result = unsafe {
            context.RequestEditSession(
                client_id,
                &session,
                TF_CONTEXT_EDIT_CONTEXT_FLAGS(TF_ES_SYNC.0 | TF_ES_READ.0),
            )
        };
        if let Err(e) = result {
            diag::log_key(|| format!("cursor position: {e}"));
        }
        found.get()
    }

    /// The underlines of a composition, with TSF's numbers for them.
    fn parts(&self, marks: &[(Range<usize>, Mark)]) -> Vec<(Range<usize>, u32)> {
        if marks.is_empty() {
            return Vec::new();
        }
        let atoms = self.atoms.get().unwrap_or_else(|| {
            let atoms = display::atoms()
                .inspect_err(|e| diag::log(&format!("no underlines for the composition: {e}")))
                .ok();
            self.atoms.set(Some(atoms));
            atoms
        });
        let Some(atoms) = atoms else {
            return Vec::new();
        };
        marks
            .iter()
            .map(|(part, mark)| (part.clone(), display::atom_of(&atoms, *mark)))
            .collect()
    }

    /// Run `steps` in an edit session of `context`. A key wants its text now
    /// (`sync`); a composition that ends because the focus moved does not mind
    /// when.
    fn run(
        &self,
        context: &ITfContext,
        steps: Vec<Step>,
        parts: Vec<(Range<usize>, u32)>,
        sync: bool,
    ) -> Result<()> {
        let result = self.run_session(context, steps, parts, sync);
        // Whatever the edit left of the composition, the service now knows.
        self.sync_orphan(context);
        result
    }

    fn run_session(
        &self,
        context: &ITfContext,
        steps: Vec<Step>,
        parts: Vec<(Range<usize>, u32)>,
        sync: bool,
    ) -> Result<()> {
        let Some(client_id) = self.client_id() else {
            return Ok(());
        };
        // How much text the application left unowned just before the cursor,
        // for the new text to take the place of.
        let adopt = self
            .orphan
            .try_borrow()
            .ok()
            .and_then(|orphan| {
                orphan
                    .as_ref()
                    .filter(|orphan| orphan.context == *context)
                    .map(|orphan| orphan.text.encode_utf16().count())
            })
            .unwrap_or(0);
        let session: ITfEditSession = EditSession {
            context: context.clone(),
            sink: self.composition_sink()?,
            composing: self.composing.clone(),
            steps: steps.clone(),
            parts: parts.clone(),
            adopt: Cell::new(adopt),
        }
        .into();
        let write = TF_ES_READWRITE.0;
        let first = if sync {
            TF_ES_SYNC.0 | write
        } else {
            TF_ES_ASYNCDONTCARE.0 | write
        };
        // SAFETY: the context and the session are alive for the call. The
        // session runs inside it (sync) or later (async), on this thread.
        let mut result = unsafe {
            context.RequestEditSession(client_id, &session, TF_CONTEXT_EDIT_CONTEXT_FLAGS(first))
        }?;
        if result == TF_E_SYNCHRONOUS {
            // The application could not grant the lock now: have the text put
            // in when it can.
            let session: ITfEditSession = EditSession {
                context: context.clone(),
                sink: self.composition_sink()?,
                composing: self.composing.clone(),
                steps,
                parts,
                adopt: Cell::new(adopt),
            }
            .into();
            // SAFETY: as above.
            result = unsafe {
                context.RequestEditSession(
                    client_id,
                    &session,
                    TF_CONTEXT_EDIT_CONTEXT_FLAGS(TF_ES_ASYNCDONTCARE.0 | write),
                )
            }?;
        }
        result.ok()
    }

    /// The composition is over without a key (the focus moved on, the service
    /// stops): end it in the document, where it is, and forget the engine's
    /// copy. The text stays as it is shown.
    fn end_composition(&self) {
        let taken = self.composing.borrow().as_ref().map(|c| c.context.clone());
        if let Some(context) = taken
            && let Err(e) = self.run(&context, vec![Step::End], Vec::new(), false)
        {
            diag::log(&format!("could not end the composition: {e}"));
            // Do not keep a composition we cannot reach.
            self.composing.borrow_mut().take();
        }
        self.forget_engine();
        self.candidates.end(self.elements().as_ref());
    }

    /// A composition left in another text field (the focus moved without
    /// telling us) is finished there before a key of this one is handled, so
    /// no edit of this field ever touches the other field's composition.
    fn leave_other_context(&self, context: &ITfContext) {
        let elsewhere = self
            .composing
            .borrow()
            .as_ref()
            .is_some_and(|c| c.context != *context);
        if elsewhere {
            self.end_composition();
        }
    }

    /// Settings changed on disk: use them. What is being composed is finished
    /// first.
    fn reload_settings(&self, context: Option<&ITfContext>) {
        let changed = self
            .reloader
            .borrow_mut()
            .as_mut()
            .and_then(|reloader| reloader.poll(Instant::now()));
        let Some(result) = changed else {
            return;
        };
        match result {
            Ok(config) => {
                diag::log("settings changed, using them");
                if let Some(context) = context {
                    self.settle_orphan(context);
                }
                *self.appearance.borrow_mut() = config.appearance.clone();
                let edit = self
                    .typist
                    .borrow_mut()
                    .as_mut()
                    .and_then(|typist| typist.apply_config(&cycle_config(&config)));
                if let (Some(edit), Some(context)) = (edit, context) {
                    let steps = plan(self.unfinished(), &edit);
                    if let Err(e) = self.run(context, steps, Vec::new(), true) {
                        diag::log(&format!("could not edit the document: {e}"));
                    }
                }
                // The languages of the cycle may have changed.
                self.update_langbar();
            }
            Err(why) => diag::log(&format!("settings changed but not understood, kept: {why}")),
        }
    }
}

impl ITfTextInputProcessor_Impl for TextService_Impl {
    fn Activate(&self, thread_manager: Ref<ITfThreadMgr>, client_id: u32) -> Result<()> {
        let thread_manager = thread_manager.ok()?;
        self.activate(thread_manager, client_id)
    }

    fn Deactivate(&self) -> Result<()> {
        guarded("Deactivate", Ok(()), || {
            self.end_composition();
            self.indicator.borrow_mut().destroy();
            self.candidates.destroy(self.elements().as_ref());
            let bar = self.langbar.borrow_mut().take();
            if let Some(bar) = bar {
                bar.remove();
            }
            // Every step is tried even if one fails: a sink left behind would
            // keep this service alive and keep being called.
            let taken = self.activated.borrow_mut().take();
            if let Some(activated) = taken {
                if let Ok(keystrokes) = activated.thread_manager.cast::<ITfKeystrokeMgr>() {
                    // SAFETY: the interface is alive for the call.
                    let _ = unsafe { keystrokes.UnadviseKeyEventSink(activated.client_id) };
                }
                if let Ok(source) = activated.thread_manager.cast::<ITfSource>() {
                    // SAFETY: as above.
                    let _ = unsafe { source.UnadviseSink(activated.thread_sink_cookie) };
                }
            }
            *self.typist.borrow_mut() = None;
            *self.reloader.borrow_mut() = None;
            *self.self_sink.borrow_mut() = None;
            Ok(())
        })
    }
}

impl ITfTextInputProcessorEx_Impl for TextService_Impl {
    fn ActivateEx(
        &self,
        thread_manager: Ref<ITfThreadMgr>,
        client_id: u32,
        _flags: u32,
    ) -> Result<()> {
        let thread_manager = thread_manager.ok()?;
        self.activate(thread_manager, client_id)
    }
}

impl ITfKeyEventSink_Impl for TextService_Impl {
    fn OnSetFocus(&self, _foreground: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        context: Ref<ITfContext>,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Result<BOOL> {
        guarded("OnTestKeyDown", Ok(false.into()), || {
            let why = why_not_ours(context.as_ref());
            self.note_keys(why);
            if why.is_some() {
                return Ok(false.into());
            }
            let key = KeyInfo::from_message(wparam.0 as u16, lparam.0);
            let wanted = match self.typist.borrow().as_ref() {
                Some(typist) => typist.test_key_down(key, &keyboard_state()),
                None => false,
            };
            diag::log_key(|| format!("test key down {key:?} -> {wanted}"));
            Ok(wanted.into())
        })
    }

    fn OnTestKeyUp(
        &self,
        context: Ref<ITfContext>,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Result<BOOL> {
        guarded("OnTestKeyUp", Ok(false.into()), || {
            if keys_not_ours(context.as_ref()) {
                return Ok(false.into());
            }
            let key = KeyInfo::from_message(wparam.0 as u16, lparam.0);
            let wanted = match self.typist.borrow().as_ref() {
                Some(typist) => typist.test_key_up(key),
                None => false,
            };
            Ok(wanted.into())
        })
    }

    fn OnKeyDown(&self, context: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        guarded("OnKeyDown", Ok(false.into()), || {
            let Some(context) = context.as_ref() else {
                return Ok(false.into());
            };
            if keys_not_ours(Some(context)) {
                return Ok(false.into());
            }
            let key = KeyInfo::from_message(wparam.0 as u16, lparam.0);
            // First the other field, then new settings (which may finish the
            // composition of this one).
            self.leave_other_context(context);
            self.settle_orphan(context);
            self.reload_settings(Some(context));
            let outcome = match self.typist.borrow_mut().as_mut() {
                Some(typist) => typist.key_down(key, &keyboard_state()),
                None => return Ok(false.into()),
            };
            diag::log_key(|| format!("key down {key:?} -> {outcome:?}"));
            // The user has seen which language it is and goes on: the box next
            // to the text cursor does not wait for its time. The switch keys
            // and the modifiers do not count (`starts_typing`).
            if keys::starts_typing(key) {
                self.indicator.borrow_mut().hide();
            }
            if !self.perform(context, &outcome) && outcome.eaten {
                // The document did not take the text (it may refuse
                // compositions): give the key to the application rather than
                // lose it, and start afresh.
                self.forget_engine();
                self.candidates.end(self.elements().as_ref());
                return Ok(false.into());
            }
            self.show_candidates(context);
            Ok(outcome.eaten.into())
        })
    }

    fn OnKeyUp(&self, context: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        guarded("OnKeyUp", Ok(false.into()), || {
            let Some(context) = context.as_ref() else {
                return Ok(false.into());
            };
            if keys_not_ours(Some(context)) {
                return Ok(false.into());
            }
            let key = KeyInfo::from_message(wparam.0 as u16, lparam.0);
            self.leave_other_context(context);
            self.settle_orphan(context);
            let outcome = match self.typist.borrow_mut().as_mut() {
                Some(typist) => typist.key_up(key),
                None => return Ok(false.into()),
            };
            diag::log_key(|| format!("key up {key:?} -> {outcome:?}"));
            self.perform(context, &outcome);
            // A switch on the release finishes a conversion too.
            self.show_candidates(context);
            Ok(outcome.eaten.into())
        })
    }

    fn OnPreservedKey(&self, _context: Ref<ITfContext>, _guid: *const GUID) -> Result<BOOL> {
        Ok(false.into())
    }
}

/// Windows' own places for a text service's options (the keyboard's options in
/// the Settings app, the old "Text Services and Input Languages" panel) make
/// the service by its class id, ask it for `ITfFnConfigure` and call `Show`.
/// The options of easime are its settings app.
impl ITfFunction_Impl for TextService_Impl {
    fn GetDisplayName(&self) -> Result<BSTR> {
        Ok(BSTR::from("easime"))
    }
}

impl ITfFnConfigure_Impl for TextService_Impl {
    fn Show(&self, _parent: HWND, _langid: u16, _profile: *const GUID) -> Result<()> {
        guarded("Show", Err(E_FAIL.into()), || {
            diag::log("options asked for by Windows: opening the settings app");
            if langbar::open_settings() {
                Ok(())
            } else {
                Err(E_FAIL.into())
            }
        })
    }
}

impl ITfCompositionSink_Impl for TextService_Impl {
    /// The application ended our composition (the user clicked elsewhere, the
    /// field went away): its text stays as shown, so only the engine's copy
    /// has to go.
    fn OnCompositionTerminated(&self, write: u32, composition: Ref<ITfComposition>) -> Result<()> {
        guarded("OnCompositionTerminated", Ok(()), || {
            // Not by us (our own end of a composition does not come here): the
            // application did it.
            let ended = self
                .composing
                .try_borrow_mut()
                .ok()
                .and_then(|mut composing| composing.take());
            if let Some(ended) = &ended {
                // What stays in the document is finished text: no underline.
                let composition = composition.ok().unwrap_or(&ended.composition);
                if let Err(e) = display::unmark(&ended.context, composition, write) {
                    diag::log_key(|| format!("underline not taken off: {e}"));
                }
            }
            // The engine may be holding the syllable still. The text in the
            // document is the same, and the next key can take it up again (Excel
            // ends the composition after every key; forgetting the syllable
            // there made every jamo stand alone). A conversion is not taken up.
            let holding = self
                .typist
                .try_borrow()
                .ok()
                .and_then(|typist| {
                    typist
                        .as_ref()
                        .filter(|typist| typist.candidates().is_none())
                        .map(|typist| typist.shown().to_owned())
                })
                .filter(|text| !text.is_empty());
            let what = match (&ended, &holding) {
                (Some(_), Some(text)) => format!(
                    "its syllable ({} characters) is kept to be taken up again",
                    text.chars().count()
                ),
                (None, _) => "not a composition easime knew of: the engine forgets".to_owned(),
                (Some(_), None) => "nothing held, or a conversion: the engine forgets".to_owned(),
            };
            diag::log(&format!("the application ended the composition: {what}"));
            match (ended, holding) {
                (Some(ended), Some(text)) => {
                    if let Ok(mut orphan) = self.orphan.try_borrow_mut() {
                        *orphan = Some(Orphan {
                            context: ended.context,
                            text,
                        });
                    }
                }
                _ => self.forget_engine(),
            }
            let elements = self.activated.try_borrow().ok().and_then(|activated| {
                activated
                    .as_ref()
                    .and_then(|a| a.thread_manager.cast::<ITfUIElementMgr>().ok())
            });
            self.candidates.end(elements.as_ref());
            Ok(())
        })
    }
}

impl ITfDisplayAttributeProvider_Impl for TextService_Impl {
    fn EnumDisplayAttributeInfo(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        guarded(
            "EnumDisplayAttributeInfo",
            Err(windows::Win32::Foundation::E_FAIL.into()),
            || Ok(display::AttributeList::new().into()),
        )
    }

    // The signature, raw pointer included, is COM's: the caller owns it.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn GetDisplayAttributeInfo(&self, id: *const GUID) -> Result<ITfDisplayAttributeInfo> {
        guarded(
            "GetDisplayAttributeInfo",
            Err(windows::Win32::Foundation::E_FAIL.into()),
            || {
                // SAFETY: TSF passes a GUID that lives for the call, or null.
                let id = unsafe { id.as_ref() }.ok_or(E_POINTER)?;
                display::info(id)
            },
        )
    }
}

impl ITfThreadMgrEventSink_Impl for TextService_Impl {
    fn OnInitDocumentMgr(&self, _document: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    fn OnUninitDocumentMgr(&self, _document: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    /// Another text field has the focus: the composition of the old one ends
    /// where it is.
    fn OnSetFocus(
        &self,
        _focus: Ref<ITfDocumentMgr>,
        _previous: Ref<ITfDocumentMgr>,
    ) -> Result<()> {
        guarded("OnSetFocus", Ok(()), || {
            self.end_composition();
            self.reload_settings(None);
            Ok(())
        })
    }

    fn OnPushContext(&self, _context: Ref<ITfContext>) -> Result<()> {
        Ok(())
    }

    fn OnPopContext(&self, _context: Ref<ITfContext>) -> Result<()> {
        Ok(())
    }
}

/// The operations on the document, run where TSF allows them.
#[implement(ITfEditSession)]
struct EditSession {
    context: ITfContext,
    sink: ITfCompositionSink,
    composing: Rc<RefCell<Option<Composing>>>,
    steps: Vec<Step>,
    /// The underlines of the composition after the steps (UTF-16 ranges and
    /// TSF's numbers for the attributes).
    parts: Vec<(Range<usize>, u32)>,
    /// How many UTF-16 units just before the cursor the application left as
    /// finished text of an ended composition: the first text written, when no
    /// composition is going on, takes their place.
    adopt: Cell<usize>,
}

impl ITfEditSession_Impl for EditSession_Impl {
    fn DoEditSession(&self, cookie: u32) -> Result<()> {
        guarded::<Result<()>>(
            "DoEditSession",
            Err(windows::Win32::Foundation::E_FAIL.into()),
            || {
                for step in &self.steps {
                    match step {
                        Step::Text(text) => self.write(cookie, text)?,
                        Step::End => self.end(cookie)?,
                    }
                }
                // The text is in; an underline that cannot be drawn is not
                // worth failing the edit for.
                let composing = self
                    .composing
                    .borrow()
                    .as_ref()
                    .map(|c| c.composition.clone());
                if let Some(composition) = composing
                    && !self.parts.is_empty()
                    && let Err(e) = display::mark(&self.context, &composition, cookie, &self.parts)
                {
                    diag::log_key(|| format!("composition not underlined: {e}"));
                }
                Ok(())
            },
        )
    }
}

impl EditSession_Impl {
    /// Make `text` the text of the composition, starting one at the cursor if
    /// there is none, and put the cursor after it.
    fn write(&self, cookie: u32, text: &str) -> Result<()> {
        let existing = self
            .composing
            .borrow()
            .as_ref()
            .map(|c| c.composition.clone());
        let composition = match existing {
            Some(composition) => composition,
            None => {
                let insert: ITfInsertAtSelection = self.context.cast()?;
                // SAFETY: the context is alive; `QUERYONLY` inserts nothing.
                let at_cursor =
                    unsafe { insert.InsertTextAtSelection(cookie, TF_IAS_QUERYONLY, &[]) }?;
                let adopted = self.adopt.take() as i32;
                if adopted > 0 {
                    // Cover the text the application left behind, so that the
                    // new text takes its place instead of standing next to it.
                    let mut moved = 0;
                    // SAFETY: the range is alive; the counter lives for the call.
                    let shifted = unsafe {
                        at_cursor.ShiftStart(cookie, -adopted, &mut moved, std::ptr::null())
                    };
                    if shifted.is_err() || moved != -adopted {
                        diag::log(&format!(
                            "could not cover the ended syllable to replace it (moved {moved} of {adopted}: {shifted:?})"
                        ));
                        if moved != 0 {
                            let mut back = 0;
                            // SAFETY: as above; the cursor is put back.
                            let _ = unsafe {
                                at_cursor.ShiftStart(cookie, -moved, &mut back, std::ptr::null())
                            };
                        }
                    }
                }
                let composer: ITfContextComposition = self.context.cast()?;
                // SAFETY: the range and the sink are alive for the call.
                let composition =
                    unsafe { composer.StartComposition(cookie, &at_cursor, &self.sink) }?;
                *self.composing.borrow_mut() = Some(Composing {
                    context: self.context.clone(),
                    composition: composition.clone(),
                });
                composition
            }
        };
        // SAFETY: the composition is alive.
        let range = unsafe { composition.GetRange() }?;
        let wide: Vec<u16> = text.encode_utf16().collect();
        // SAFETY: the range is alive; `wide` lives for the call.
        unsafe { range.SetText(cookie, 0, &wide) }?;
        // SAFETY: as above.
        unsafe { range.Collapse(cookie, TF_ANCHOR_END) }?;
        let mut selection = TF_SELECTION {
            range: ManuallyDrop::new(Some(range)),
            style: TF_SELECTIONSTYLE {
                ase: TF_AE_NONE,
                fInterimChar: false.into(),
            },
        };
        // SAFETY: the selection holds a live range; TSF takes its own reference.
        let moved = unsafe {
            self.context
                .SetSelection(cookie, std::slice::from_ref(&selection))
        };
        // SAFETY: the range was put there above and is released once.
        unsafe { ManuallyDrop::drop(&mut selection.range) };
        // The text is written; a cursor that could not be moved must not stop
        // the steps after this one (ending the composition, for one).
        if let Err(e) = moved {
            diag::log(&format!("could not move the cursor after the text: {e}"));
        }
        Ok(())
    }

    /// End the composition; its text stays in the document.
    fn end(&self, cookie: u32) -> Result<()> {
        let taken = self.composing.borrow_mut().take();
        if let Some(composing) = taken {
            // What stays in the document is finished text: no underline.
            if let Err(e) = display::unmark(&composing.context, &composing.composition, cookie) {
                diag::log_key(|| format!("underline not taken off: {e}"));
            }
            // SAFETY: the composition is alive.
            unsafe { composing.composition.EndComposition(cookie) }?;
        }
        Ok(())
    }
}

/// Reads whether the text just before the cursor is `expected` (and nothing is
/// selected), and leaves the answer, in words for the log, in `verdict`: "ok" or
/// what was different.
#[implement(ITfEditSession)]
struct BeforeCursor {
    context: ITfContext,
    expected: Vec<u16>,
    verdict: Rc<RefCell<String>>,
}

impl ITfEditSession_Impl for BeforeCursor_Impl {
    fn DoEditSession(&self, cookie: u32) -> Result<()> {
        guarded::<Result<()>>(
            "BeforeCursor",
            Err(windows::Win32::Foundation::E_FAIL.into()),
            || {
                let verdict = self
                    .look(cookie)
                    .unwrap_or_else(|e| format!("could not read the document: {e}"));
                *self.verdict.borrow_mut() = verdict;
                Ok(())
            },
        )
    }
}

impl BeforeCursor_Impl {
    fn look(&self, cookie: u32) -> Result<String> {
        let mut selection = [TF_SELECTION::default()];
        let mut fetched = 0;
        // SAFETY: the buffer holds the one selection asked for.
        unsafe {
            self.context
                .GetSelection(cookie, TF_DEFAULT_SELECTION, &mut selection, &mut fetched)
        }?;
        // SAFETY: the range was handed to us by GetSelection; taken once.
        let Some(range) = (unsafe { ManuallyDrop::take(&mut selection[0].range) }) else {
            return Ok("the document has no selection".to_owned());
        };
        // SAFETY: the range is alive for the calls.
        if !unsafe { range.IsEmpty(cookie) }?.as_bool() {
            return Ok("something is selected".to_owned());
        }
        let wanted = self.expected.len() as i32;
        let mut moved = 0;
        // SAFETY: as above; the counter lives for the call.
        unsafe { range.ShiftStart(cookie, -wanted, &mut moved, std::ptr::null()) }?;
        if moved != -wanted {
            return Ok(format!(
                "the cursor cannot go back {wanted} characters (went back {})",
                -moved
            ));
        }
        let mut text = vec![0u16; self.expected.len()];
        let mut got = 0;
        // SAFETY: the buffer and the counter live for the call.
        unsafe { range.GetText(cookie, 0, &mut text, &mut got) }?;
        if got as usize != text.len() {
            return Ok(format!(
                "read {got} of {wanted} characters before the cursor"
            ));
        }
        Ok(if text == self.expected {
            "ok".to_owned()
        } else {
            "the text before the cursor is another text".to_owned()
        })
    }
}

/// Asks the document where its cursor is on the screen.
#[implement(ITfEditSession)]
struct CaretQuery {
    context: ITfContext,
    found: Rc<Cell<Option<Rect>>>,
}

impl ITfEditSession_Impl for CaretQuery_Impl {
    fn DoEditSession(&self, cookie: u32) -> Result<()> {
        guarded::<Result<()>>(
            "CaretQuery",
            Err(windows::Win32::Foundation::E_FAIL.into()),
            || {
                let mut selection = [TF_SELECTION::default()];
                let mut fetched = 0;
                // SAFETY: the buffer holds the one selection asked for.
                unsafe {
                    self.context.GetSelection(
                        cookie,
                        TF_DEFAULT_SELECTION,
                        &mut selection,
                        &mut fetched,
                    )
                }?;
                // SAFETY: the range was handed to us by GetSelection; taken once.
                let Some(range) = (unsafe { ManuallyDrop::take(&mut selection[0].range) }) else {
                    return Ok(());
                };
                // SAFETY: the context is alive.
                let view: ITfContextView = unsafe { self.context.GetActiveView() }?;
                let mut area = windows::Win32::Foundation::RECT::default();
                let mut clipped = BOOL::default();
                // SAFETY: the range, the rectangle and the flag live for the call.
                unsafe { view.GetTextExt(cookie, &range, &mut area, &mut clipped) }?;
                if area.left == 0 && area.top == 0 && area.right == 0 && area.bottom == 0 {
                    return Ok(());
                }
                self.found.set(Some(Rect {
                    left: area.left,
                    top: area.top,
                    right: area.right.max(area.left + 1),
                    bottom: area.bottom.max(area.top + 1),
                }));
                Ok(())
            },
        )
    }
}

/// Makes text services. Counted like the services: COM may hold a factory
/// without locking the server, and the DLL must not go away under it.
#[implement(IClassFactory)]
pub struct ClassFactory;

impl ClassFactory {
    fn new() -> Self {
        IN_USE.fetch_add(1, Ordering::SeqCst);
        ClassFactory
    }
}

impl Drop for ClassFactory {
    fn drop(&mut self) {
        IN_USE.fetch_sub(1, Ordering::SeqCst);
    }
}

impl IClassFactory_Impl for ClassFactory_Impl {
    // The signature, raw pointers included, is COM's: the caller owns them.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        interface: *const GUID,
        object: *mut *mut c_void,
    ) -> Result<()> {
        if object.is_null() || interface.is_null() {
            return Err(E_POINTER.into());
        }
        // SAFETY: the caller passes a pointer to write the result to.
        unsafe { *object = std::ptr::null_mut() };
        if outer.as_ref().is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let service: ITfTextInputProcessorEx = TextService::new().into();
        // SAFETY: `interface` and `object` are the caller's, checked above.
        unsafe { service.query(interface, object).ok() }
    }

    fn LockServer(&self, lock: BOOL) -> Result<()> {
        if lock.as_bool() {
            LOCKS.fetch_add(1, Ordering::SeqCst);
        } else if LOCKS.fetch_sub(1, Ordering::SeqCst) <= 0 {
            // More unlocks than locks: a bug of the caller. Keep the count sane.
            LOCKS.fetch_add(1, Ordering::SeqCst);
            return Err(E_INVALIDARG.into());
        }
        Ok(())
    }
}

/// The factory, as `DllGetClassObject` hands it out.
pub fn class_factory() -> IClassFactory {
    ClassFactory::new().into()
}

/// What `ITfTextInputProcessor` a created service is. Kept for tests that make
/// one without going through COM's class lookup.
pub fn new_text_service() -> ITfTextInputProcessor {
    let service: ITfTextInputProcessorEx = TextService::new().into();
    service
        .cast()
        .expect("the service implements the base interface")
}

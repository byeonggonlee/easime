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
//! **What it does not do yet:** show a persistent taskbar/language-bar
//! indicator, underline the composition (no display attributes), and
//! candidates for conversion. See `docs/platforms.md`.
//!
//! One service lives per thread of an application, so each has its own typist,
//! and its mode (English or Korean) is its own. Everything here runs on that
//! one thread, so plain cells are enough; no cell is borrowed while a call goes
//! out to the application, because the application may call back in.
//!
//! A panic must never take the application down with it: every entry point
//! catches it, writes it to the log and says "not eaten".

use crate::diag;
use crate::indicator::{self, Indicator};
use crate::keys::{KeyInfo, KeyboardState};
use crate::placement::Rect;
use crate::profiles;
use crate::typing::{Outcome, Step, Typist, plan};
use easime_config::{Appearance, Config};
use ime_session::{ConfigReloader, JapaneseDictionary, Session};
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{CLASS_E_NOAGGREGATION, E_INVALIDARG, E_POINTER, LPARAM, WPARAM};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardState;
use windows::Win32::UI::TextServices::{
    GUID_COMPARTMENT_EMPTYCONTEXT, GUID_COMPARTMENT_KEYBOARD_DISABLED, ITfCompartmentMgr,
    ITfComposition, ITfCompositionSink, ITfCompositionSink_Impl, ITfContext, ITfContextComposition,
    ITfContextView, ITfDocumentMgr, ITfEditSession, ITfEditSession_Impl, ITfInsertAtSelection,
    ITfKeyEventSink, ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfSource, ITfTextInputProcessor,
    ITfTextInputProcessor_Impl, ITfTextInputProcessorEx, ITfTextInputProcessorEx_Impl,
    ITfThreadMgr, ITfThreadMgrEventSink, ITfThreadMgrEventSink_Impl, TF_AE_NONE, TF_ANCHOR_END,
    TF_CONTEXT_EDIT_CONTEXT_FLAGS, TF_DEFAULT_SELECTION, TF_E_SYNCHRONOUS, TF_ES_ASYNCDONTCARE,
    TF_ES_READ, TF_ES_READWRITE, TF_ES_SYNC, TF_IAS_QUERYONLY, TF_SELECTION, TF_SELECTIONSTYLE,
};
use windows::core::{BOOL, GUID, IUnknown, IUnknownImpl, Interface, Ref, Result, implement};

/// How many objects of this DLL are alive (text services and their factories),
/// for `DllCanUnloadNow`.
static IN_USE: AtomicIsize = AtomicIsize::new(0);
/// How many `LockServer(TRUE)` calls are not yet undone.
static LOCKS: AtomicIsize = AtomicIsize::new(0);

/// Whether nothing of this DLL is in use: Windows may unload it only then.
pub fn unloadable() -> bool {
    IN_USE.load(Ordering::SeqCst) == 0 && LOCKS.load(Ordering::SeqCst) == 0
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
    let Some(context) = context else {
        return false;
    };
    let Ok(compartments) = context.cast::<ITfCompartmentMgr>() else {
        return false;
    };
    [
        GUID_COMPARTMENT_KEYBOARD_DISABLED,
        GUID_COMPARTMENT_EMPTYCONTEXT,
    ]
    .iter()
    .any(|which| {
        // SAFETY: the compartment manager is alive; the GUID lives for the call.
        unsafe { compartments.GetCompartment(which) }
            .and_then(|compartment| unsafe { compartment.GetValue() })
            .ok()
            .and_then(|value| i32::try_from(&value).ok())
            .is_some_and(|value| value != 0)
    })
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

/// The text service of one thread of one application.
#[implement(
    ITfTextInputProcessorEx,
    ITfKeyEventSink,
    ITfCompositionSink,
    ITfThreadMgrEventSink
)]
pub struct TextService {
    activated: RefCell<Option<Activated>>,
    typist: RefCell<Option<Typist>>,
    reloader: RefCell<Option<ConfigReloader>>,
    /// The box that says which language is on, and whether and how long to show it.
    indicator: RefCell<Indicator>,
    appearance: RefCell<Appearance>,
    /// Whether the log already says that no cursor position could be found.
    told_no_caret: Cell<bool>,
    /// Shared with the edit sessions, which start and end compositions.
    composing: Rc<RefCell<Option<Composing>>>,
}

impl TextService {
    pub fn new() -> Self {
        IN_USE.fetch_add(1, Ordering::SeqCst);
        Self {
            activated: RefCell::new(None),
            typist: RefCell::new(None),
            reloader: RefCell::new(None),
            indicator: RefCell::new(Indicator::default()),
            appearance: RefCell::new(Appearance::default()),
            told_no_caret: Cell::new(false),
            composing: Rc::new(RefCell::new(None)),
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

/// The settings the cycle runs with (see [`profiles::effective_config`]).
fn cycle_config(config: &Config) -> Config {
    profiles::effective_config(
        config,
        &easime_config::system_languages(),
        &profiles::TYPEABLE,
        profiles::HOME,
    )
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
        "start: languages {:?}, switch key {:?}",
        cycle.languages.enabled.clone().unwrap_or_default(),
        config.shortcuts.toggle_key
    ));
    let session = Session::new(&cycle, JapaneseDictionary::installed(), Box::new(diag::log));
    let reloader = easime_config::config_path().map(|path| ConfigReloader::new(path, CONFIG_POLL));
    (
        Typist::new(session, config.shortcuts.toggle_key),
        reloader,
        config.appearance,
    )
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

        let (typist, reloader, appearance) = make_typist();
        *self.typist.borrow_mut() = Some(typist);
        *self.reloader.borrow_mut() = reloader;
        *self.appearance.borrow_mut() = appearance;
        *self.activated.borrow_mut() = Some(Activated {
            thread_manager: thread_manager.clone(),
            client_id,
            thread_sink_cookie: cookie,
        });
        Ok(())
    }

    fn client_id(&self) -> Option<u32> {
        self.activated.borrow().as_ref().map(|a| a.client_id)
    }

    /// Make `outcome`'s edit in the document of `context`. Returns false if
    /// the document could not be edited (the failure is in the log).
    fn perform(&self, context: &ITfContext, outcome: &Outcome) -> bool {
        let mut edited = true;
        if let Some(edit) = &outcome.edit {
            let composing = self.composing.borrow().is_some();
            let steps = plan(composing, edit);
            if !steps.is_empty()
                && let Err(e) = self.run(context, steps, true)
            {
                diag::log(&format!("could not edit the document: {e}"));
                edited = false;
            }
        }
        // After the edit, so the box sits next to where the cursor is now.
        if let Some(language) = outcome.switched_to {
            diag::log(&format!("language -> {language:?}"));
            self.show_indicator(context, language);
        }
        edited
    }

    /// Say which language is on, next to the text cursor, if the user wants it.
    /// Only a switch the user made comes here, never a focus change.
    fn show_indicator(&self, context: &ITfContext, language: ime_core::Language) {
        let appearance = self.appearance.borrow().clone();
        if !appearance.indicator_box {
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

    /// Run `steps` in an edit session of `context`. A key wants its text now
    /// (`sync`); a composition that ends because the focus moved does not mind
    /// when.
    fn run(&self, context: &ITfContext, steps: Vec<Step>, sync: bool) -> Result<()> {
        let Some(client_id) = self.client_id() else {
            return Ok(());
        };
        let session: ITfEditSession = EditSession {
            context: context.clone(),
            sink: self.to_interface(),
            composing: self.composing.clone(),
            steps: steps.clone(),
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
                sink: self.to_interface(),
                composing: self.composing.clone(),
                steps,
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
            && let Err(e) = self.run(&context, vec![Step::End], false)
        {
            diag::log(&format!("could not end the composition: {e}"));
            // Do not keep a composition we cannot reach.
            self.composing.borrow_mut().take();
        }
        if let Some(typist) = self.typist.borrow_mut().as_mut() {
            typist.forget();
        }
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
                *self.appearance.borrow_mut() = config.appearance.clone();
                let edit = self
                    .typist
                    .borrow_mut()
                    .as_mut()
                    .and_then(|typist| typist.apply_config(&cycle_config(&config)));
                if let (Some(edit), Some(context)) = (edit, context) {
                    let steps = plan(self.composing.borrow().is_some(), &edit);
                    if let Err(e) = self.run(context, steps, true) {
                        diag::log(&format!("could not edit the document: {e}"));
                    }
                }
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
            if keys_not_ours(context.as_ref()) {
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
            self.reload_settings(Some(context));
            let outcome = match self.typist.borrow_mut().as_mut() {
                Some(typist) => typist.key_down(key, &keyboard_state()),
                None => return Ok(false.into()),
            };
            diag::log_key(|| format!("key down {key:?} -> {outcome:?}"));
            if !self.perform(context, &outcome) && outcome.eaten {
                // The document did not take the text (it may refuse
                // compositions): give the key to the application rather than
                // lose it, and start afresh.
                if let Some(typist) = self.typist.borrow_mut().as_mut() {
                    typist.forget();
                }
                return Ok(false.into());
            }
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
            let outcome = match self.typist.borrow_mut().as_mut() {
                Some(typist) => typist.key_up(key),
                None => return Ok(false.into()),
            };
            diag::log_key(|| format!("key up {key:?} -> {outcome:?}"));
            self.perform(context, &outcome);
            Ok(outcome.eaten.into())
        })
    }

    fn OnPreservedKey(&self, _context: Ref<ITfContext>, _guid: *const GUID) -> Result<BOOL> {
        Ok(false.into())
    }
}

impl ITfCompositionSink_Impl for TextService_Impl {
    /// The application ended our composition (the user clicked elsewhere, the
    /// field went away): its text stays as shown, so only the engine's copy
    /// has to go.
    fn OnCompositionTerminated(
        &self,
        _write: u32,
        _composition: Ref<ITfComposition>,
    ) -> Result<()> {
        guarded("OnCompositionTerminated", Ok(()), || {
            if let Ok(mut composing) = self.composing.try_borrow_mut() {
                composing.take();
            }
            if let Ok(mut typist) = self.typist.try_borrow_mut()
                && let Some(typist) = typist.as_mut()
            {
                typist.forget();
            }
            Ok(())
        })
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
            // SAFETY: the composition is alive.
            unsafe { composing.composition.EndComposition(cookie) }?;
        }
        Ok(())
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

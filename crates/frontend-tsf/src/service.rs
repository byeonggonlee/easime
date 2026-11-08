//! The text service itself: the COM object Windows creates in every
//! application that lets the user type, and the factory that makes it.
//!
//! **Step one**: the service activates, registers a key event sink with TSF,
//! and lets every key through (it "eats" nothing). So a registered easime does
//! not type yet and cannot get in the way; what is proven is that Windows can
//! create it, activate it and call its sink. Typing comes next.
//!
//! TSF asks the sink twice per key press, `OnTestKeyDown` ("would you take
//! it?", no side effects) and `OnKeyDown` (do it). That shapes how the shared
//! `Session` is used later (docs/platforms.md).

use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::Win32::Foundation::{CLASS_E_NOAGGREGATION, E_INVALIDARG, E_POINTER, LPARAM, WPARAM};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows::Win32::UI::TextServices::{
    ITfContext, ITfKeyEventSink, ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfTextInputProcessor,
    ITfTextInputProcessor_Impl, ITfTextInputProcessorEx, ITfTextInputProcessorEx_Impl,
    ITfThreadMgr,
};
use windows::core::{BOOL, GUID, IUnknown, IUnknownImpl, Interface, Ref, Result, implement};

/// How many text services and factory locks are alive, for
/// `DllCanUnloadNow`: Windows may only unload the DLL when this is zero.
static IN_USE: AtomicIsize = AtomicIsize::new(0);

/// Whether nothing of this DLL is in use.
pub fn unloadable() -> bool {
    IN_USE.load(Ordering::SeqCst) == 0
}

/// What TSF gave the service when it was activated.
struct Activated {
    thread_manager: ITfThreadMgr,
    client_id: u32,
}

/// The text service of one thread of one application.
#[implement(ITfTextInputProcessorEx, ITfKeyEventSink)]
pub struct TextService {
    /// Windows calls into the service from the thread it is activated on, so
    /// a plain cell is enough.
    activated: RefCell<Option<Activated>>,
}

impl TextService {
    pub fn new() -> Self {
        IN_USE.fetch_add(1, Ordering::SeqCst);
        Self {
            activated: RefCell::new(None),
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

impl TextService_Impl {
    /// Start serving: hand TSF the key event sink, and remember what it gave us.
    fn activate(&self, thread_manager: &ITfThreadMgr, client_id: u32) -> Result<()> {
        let sink: ITfKeyEventSink = self.to_interface();
        let keystrokes: ITfKeystrokeMgr = thread_manager.cast()?;
        // SAFETY: both interfaces are alive for the call.
        unsafe { keystrokes.AdviseKeyEventSink(client_id, &sink, true) }
            .map_err(|e| windows::core::Error::new(e.code(), format!("AdviseKeyEventSink: {e}")))?;
        *self.activated.borrow_mut() = Some(Activated {
            thread_manager: thread_manager.clone(),
            client_id,
        });
        Ok(())
    }
}

impl ITfTextInputProcessor_Impl for TextService_Impl {
    fn Activate(&self, thread_manager: Ref<ITfThreadMgr>, client_id: u32) -> Result<()> {
        let thread_manager = thread_manager.ok()?;
        self.activate(thread_manager, client_id)
    }

    fn Deactivate(&self) -> Result<()> {
        if let Some(activated) = self.activated.borrow_mut().take() {
            let keystrokes: ITfKeystrokeMgr = activated.thread_manager.cast()?;
            // SAFETY: the interface is alive for the call.
            unsafe { keystrokes.UnadviseKeyEventSink(activated.client_id)? };
        }
        Ok(())
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

    // Step one takes no key: every answer is "not eaten".
    fn OnTestKeyDown(
        &self,
        _context: Ref<ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(false.into())
    }

    fn OnTestKeyUp(
        &self,
        _context: Ref<ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(false.into())
    }

    fn OnKeyDown(
        &self,
        _context: Ref<ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(false.into())
    }

    fn OnKeyUp(&self, _context: Ref<ITfContext>, _wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        Ok(false.into())
    }

    fn OnPreservedKey(&self, _context: Ref<ITfContext>, _guid: *const GUID) -> Result<BOOL> {
        Ok(false.into())
    }
}

/// Makes text services.
#[implement(IClassFactory)]
pub struct ClassFactory;

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
            IN_USE.fetch_add(1, Ordering::SeqCst);
        } else if IN_USE.fetch_sub(1, Ordering::SeqCst) <= 0 {
            // More unlocks than locks: a bug of the caller. Keep the count sane.
            IN_USE.fetch_add(1, Ordering::SeqCst);
            return Err(E_INVALIDARG.into());
        }
        Ok(())
    }
}

/// The factory, as `DllGetClassObject` hands it out.
pub fn class_factory() -> IClassFactory {
    ClassFactory.into()
}

/// What `ITfTextInputProcessor` a created service is. Kept for tests that make
/// one without going through COM's class lookup.
pub fn new_text_service() -> ITfTextInputProcessor {
    let service: ITfTextInputProcessorEx = TextService::new().into();
    service
        .cast()
        .expect("the service implements the base interface")
}

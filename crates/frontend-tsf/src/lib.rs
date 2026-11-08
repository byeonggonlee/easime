//! The Windows frontend: easime as a TSF (Text Services Framework) text
//! service.
//!
//! **The part that needs no Windows API**, checked on every OS: turning what
//! Windows reports about a key press into the OS-independent
//! [`ime_core::KeyEvent`] and the switch-key role ([`keys`]); the single TSF
//! profile registered with Windows and the default language cycle ([`profiles`]);
//! what happens to a key, which keys the service takes, what text and
//! composition result, when the language switches ([`typing`]); and where the
//! installed files are, next to the DLL ([`install`]).
//!
//! **The part that talks to Windows** (Windows only): registering the COM class
//! and the profile ([`registration`], run by `regsvr32` and the installer),
//! switching easime on for the installing user ([`setup`]), and the text
//! service itself, which writes what [`typing`] decides into TSF compositions
//! ([`service`]).
//!
//! Switching a language shows a small box next to the text cursor ([`indicator`];
//! where it goes is the portable rule in [`placement`]).
//!
//! The composition is underlined: which part is which is the portable rule in
//! [`marks`], and the Windows side is [`display`].
//!
//! **Not here yet:** a candidate window (so no conversion), and sharing the mode
//! between programs.
//! Only Korean can be typed ([`profiles::TYPEABLE`]). It has been tested against
//! a simulated TSF application in CI, not on a real PC. See `docs/platforms.md`,
//! "Windows", and `docs/status.md`.

pub mod install;
pub mod keys;
pub mod marks;
pub mod page;
pub mod placement;
pub mod profiles;
pub mod typing;

#[cfg(windows)]
pub mod diag;
#[cfg(windows)]
pub mod display;
#[cfg(windows)]
pub mod indicator;
#[cfg(windows)]
pub mod registration;
#[cfg(windows)]
pub mod service;
#[cfg(windows)]
pub mod setup;

/// The DLL's entry points. Windows loads the DLL, asks it to register itself
/// (`regsvr32`, the installer) and later to unregister, and asks for the text
/// service's class factory. The installer also runs two steps through
/// `rundll32` (add or remove a language for the installing user).
#[cfg(windows)]
mod exports {
    use crate::setup::{self, Step};
    use std::ffi::{CStr, c_char, c_void};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use windows::Win32::Foundation::{
        CLASS_E_CLASSNOTAVAILABLE, E_FAIL, E_POINTER, HINSTANCE, HMODULE, HWND, S_FALSE, S_OK, TRUE,
    };
    use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
    use windows::core::{BOOL, GUID, HRESULT, Interface};

    /// This DLL's module handle, saved when it is loaded.
    static MODULE: AtomicIsize = AtomicIsize::new(0);

    const DLL_PROCESS_ATTACH: u32 = 1;

    #[unsafe(no_mangle)]
    pub extern "system" fn DllMain(module: HINSTANCE, reason: u32, _reserved: *mut c_void) -> BOOL {
        if reason == DLL_PROCESS_ATTACH {
            MODULE.store(module.0 as isize, Ordering::SeqCst);
        }
        TRUE
    }

    /// Where this DLL is, as Windows loaded it.
    /// Where this DLL is (the install folder holds its other files).
    pub(crate) fn dll_path() -> Option<PathBuf> {
        let module = HMODULE(MODULE.load(Ordering::SeqCst) as *mut c_void);
        let mut buffer = vec![0u16; 1024];
        // SAFETY: `buffer` is writable for its whole length.
        let length = unsafe { GetModuleFileNameW(Some(module), &mut buffer) } as usize;
        (length > 0 && length < buffer.len())
            .then(|| PathBuf::from(String::from_utf16_lossy(&buffer[..length])))
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn DllRegisterServer() -> HRESULT {
        match dll_path().map(|path| crate::registration::register(&path)) {
            Some(Ok(())) => S_OK,
            Some(Err(e)) => e.code(),
            None => E_FAIL,
        }
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn DllUnregisterServer() -> HRESULT {
        match crate::registration::unregister() {
            Ok(()) => S_OK,
            Err(e) => e.code(),
        }
    }

    /// What `rundll32` passes: a NUL-terminated ANSI command line.
    ///
    /// # Safety
    /// `command_line` is null or points to a NUL-terminated string.
    unsafe fn command_line_text(command_line: *const c_char) -> String {
        if command_line.is_null() {
            return String::new();
        }
        // SAFETY: promised by the caller.
        unsafe { CStr::from_ptr(command_line) }
            .to_string_lossy()
            .into_owned()
    }

    /// `rundll32 easime_tsf.dll,EnableForCurrentUser`: switch easime on in
    /// the input methods of the current user, under their first language (no
    /// language is added to their list).
    ///
    /// # Safety
    /// Called by `rundll32`, which passes a NUL-terminated command line.
    #[unsafe(no_mangle)]
    pub unsafe extern "system" fn EnableForCurrentUser(
        _window: HWND,
        _instance: HINSTANCE,
        command_line: *const c_char,
        _show: i32,
    ) {
        // SAFETY: rundll32 passes a NUL-terminated string.
        setup::run(Step::Enable, &unsafe { command_line_text(command_line) });
    }

    /// `rundll32 easime_tsf.dll,DisableForCurrentUser`: the reverse, under
    /// whichever languages easime is.
    ///
    /// # Safety
    /// Called by `rundll32`, which passes a NUL-terminated command line.
    #[unsafe(no_mangle)]
    pub unsafe extern "system" fn DisableForCurrentUser(
        _window: HWND,
        _instance: HINSTANCE,
        command_line: *const c_char,
        _show: i32,
    ) {
        // SAFETY: rundll32 passes a NUL-terminated string.
        setup::run(Step::Disable, &unsafe { command_line_text(command_line) });
    }

    /// Windows may unload the DLL only when no text service or lock is alive.
    #[unsafe(no_mangle)]
    pub extern "system" fn DllCanUnloadNow() -> HRESULT {
        if crate::service::unloadable() {
            S_OK
        } else {
            S_FALSE
        }
    }

    /// Hand out the factory for easime's own class, and refuse any other.
    #[unsafe(no_mangle)]
    pub extern "system" fn DllGetClassObject(
        class: *const GUID,
        interface: *const GUID,
        object: *mut *mut c_void,
    ) -> HRESULT {
        if class.is_null() || interface.is_null() || object.is_null() {
            return E_POINTER;
        }
        // SAFETY: the caller passes valid pointers; checked for null above.
        unsafe {
            *object = std::ptr::null_mut();
            if *class != crate::registration::guid(crate::profiles::CLSID) {
                return CLASS_E_CLASSNOTAVAILABLE;
            }
            crate::service::class_factory().query(interface, object)
        }
    }
}

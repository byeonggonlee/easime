//! The Windows frontend: easime as a TSF (Text Services Framework) text
//! service.
//!
//! **What is here today** is the part that needs no Windows API and is checked
//! on every OS: turning what Windows reports about a key press into the
//! OS-independent [`ime_core::KeyEvent`] and the switch-key role the shared
//! `ime-session` wants ([`keys`]), and the list of what is registered with
//! Windows, one profile per language ([`profiles`]). **What is not here yet** is everything that
//! talks to TSF (the COM text service, composition, the candidate window,
//! registration); there is no Windows input method until that exists. See
//! `docs/platforms.md`, "Windows".

pub mod keys;
pub mod profiles;

#[cfg(windows)]
pub mod registration;
#[cfg(windows)]
pub mod service;

/// The DLL's entry points. Windows loads the DLL, asks it to register itself
/// (`regsvr32`, the installer) and later to unregister. The text service
/// itself (the class factory) does not exist yet, so asking for the class is
/// refused.
#[cfg(windows)]
mod exports {
    use std::ffi::c_void;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use windows::Win32::Foundation::{
        CLASS_E_CLASSNOTAVAILABLE, E_FAIL, E_POINTER, HINSTANCE, HMODULE, S_FALSE, S_OK, TRUE,
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
    fn dll_path() -> Option<PathBuf> {
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

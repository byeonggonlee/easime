//! Registering the text service with Windows, and taking it away again.
//!
//! Two things make an input method known to Windows: the COM class (a registry
//! key under `HKLM\SOFTWARE\Classes\CLSID` that says which DLL implements it)
//! and its TSF profiles (what the language settings list, one per language, see
//! [`crate::profiles`]). Both are written by `DllRegisterServer`, which the
//! installer runs (`regsvr32`), and removed by `DllUnregisterServer`.
//!
//! Both need administrator rights (they are machine-wide). Doing either twice
//! is fine, so a repair or an upgrade over an old install works.

use crate::profiles::{self, CLSID};
use std::path::Path;
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteTreeW, RegOpenKeyExW, RegSetValueExW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, GUID_TFCAT_TIP_KEYBOARD, ITfCategoryMgr,
    ITfInputProcessorProfileMgr, TF_INPUTPROCESSORPROFILE, TF_PROFILETYPE_INPUTPROCESSOR,
};
use windows::core::{BOOL, Error, GUID, PCWSTR, Result, s, w};

/// `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}` as a GUID. The ids in
/// [`crate::profiles`] are constants checked by tests, so a bad one is a bug.
pub fn guid(text: &str) -> GUID {
    let hex: String = text.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    GUID::from_u128(u128::from_str_radix(&hex, 16).expect("a GUID is 32 hex digits"))
}

/// UTF-16 with a closing NUL, for the registry.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

/// COM for the current thread, for as long as the value lives.
struct Com(bool);

impl Com {
    fn start() -> Self {
        // Fails with "changed mode" when the thread already has COM in another
        // mode; then it is already usable and must not be shut down by us.
        // SAFETY: plain COM initialization of this thread.
        Self(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful CoInitializeEx of `start`.
            unsafe { CoUninitialize() };
        }
    }
}

fn class_key() -> String {
    format!("SOFTWARE\\Classes\\CLSID\\{CLSID}")
}

fn set_default(key: HKEY, value: &str) -> Result<()> {
    let data = wide(value);
    // SAFETY: `data` is a NUL-terminated UTF-16 string that outlives the call.
    unsafe {
        RegSetValueExW(
            key,
            PCWSTR::null(),
            None,
            REG_SZ,
            Some(std::slice::from_raw_parts(
                data.as_ptr().cast::<u8>(),
                data.len() * 2,
            )),
        )
        .ok()
    }
}

fn set_named(key: HKEY, name: &str, value: &str) -> Result<()> {
    let (name, data) = (wide(name), wide(value));
    // SAFETY: both are NUL-terminated UTF-16 strings that outlive the call.
    unsafe {
        RegSetValueExW(
            key,
            PCWSTR(name.as_ptr()),
            None,
            REG_SZ,
            Some(std::slice::from_raw_parts(
                data.as_ptr().cast::<u8>(),
                data.len() * 2,
            )),
        )
        .ok()
    }
}

/// Create (or open) a key below `HKLM`.
fn create_key(path: &str) -> Result<HKEY> {
    let path = wide(path);
    let mut key = HKEY::default();
    // SAFETY: `path` is NUL-terminated; `key` receives the opened key.
    unsafe {
        RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(path.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            None,
        )
        .ok()?;
    }
    Ok(key)
}

/// Write the COM class: which DLL is the text service. TSF needs a model that
/// works for apartment threads, so `Both`.
pub fn register_class(dll: &Path) -> Result<()> {
    let class = create_key(&class_key())?;
    let result = set_default(class, "easime");
    // SAFETY: closing the key opened above.
    unsafe {
        let _ = RegCloseKey(class);
    }
    result?;
    let server = create_key(&format!("{}\\InprocServer32", class_key()))?;
    let result = set_default(server, &dll.to_string_lossy())
        .and_then(|()| set_named(server, "ThreadingModel", "Both"));
    // SAFETY: closing the key opened above.
    unsafe {
        let _ = RegCloseKey(server);
    }
    result
}

/// Remove the COM class. Not being there is not an error.
pub fn unregister_class() -> Result<()> {
    let path = wide(&class_key());
    // SAFETY: `path` is NUL-terminated.
    let result = unsafe { RegDeleteTreeW(HKEY_LOCAL_MACHINE, PCWSTR(path.as_ptr())) };
    if result == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    result.ok()
}

/// Whether a key below `HKLM` exists.
fn key_exists(path: &str) -> bool {
    let path = wide(path);
    let mut key = HKEY::default();
    // SAFETY: `path` is NUL-terminated; `key` receives the opened key.
    let opened = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(path.as_ptr()),
            None,
            KEY_READ,
            &mut key,
        )
    };
    if opened.is_ok() {
        // SAFETY: closing the key opened above.
        unsafe {
            let _ = RegCloseKey(key);
        }
        true
    } else {
        false
    }
}

/// Whether the COM class is registered.
pub fn class_registered() -> bool {
    key_exists(&class_key())
}

fn profile_manager() -> Result<ITfInputProcessorProfileMgr> {
    // SAFETY: an in-process COM class of Windows itself.
    unsafe { CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER) }
}

/// Add the profiles to the language settings, and the text service to the
/// keyboard category.
pub fn register_profiles(dll: &Path) -> Result<()> {
    let clsid = guid(CLSID);
    let manager = profile_manager()?;
    let icon: Vec<u16> = dll.to_string_lossy().encode_utf16().collect();
    for profile in profiles::registered() {
        let name: Vec<u16> = profile.name().encode_utf16().collect();
        // SAFETY: the slices and GUID pointers live for the call.
        unsafe {
            manager.RegisterProfile(
                &clsid,
                profile.lang_id,
                &guid(profile.guid),
                &name,
                &icon,
                0,
                HKL::default(),
                0,
                true,
                0,
            )?;
        }
    }
    // SAFETY: an in-process COM class of Windows itself.
    let categories: ITfCategoryMgr =
        unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)? };
    // SAFETY: GUID pointers live for the call.
    unsafe { categories.RegisterCategory(&clsid, &GUID_TFCAT_TIP_KEYBOARD, &clsid) }
}

/// Take the profiles and the category away. Profiles that are not there are
/// skipped: removal must work after a half-finished install too.
pub fn unregister_profiles() -> Result<()> {
    let clsid = guid(CLSID);
    let manager = profile_manager()?;
    for profile in profiles::ALL {
        // SAFETY: GUID pointers live for the call. An error means "was not
        // registered", which is what removal wants anyway.
        let _ =
            unsafe { manager.UnregisterProfile(&clsid, profile.lang_id, &guid(profile.guid), 0) };
    }
    // SAFETY: an in-process COM class of Windows itself.
    let categories: ITfCategoryMgr =
        unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)? };
    // SAFETY: GUID pointers live for the call.
    let _ = unsafe { categories.UnregisterCategory(&clsid, &GUID_TFCAT_TIP_KEYBOARD, &clsid) };
    Ok(())
}

/// Ask Windows for this profile of the text service. The error says why it is
/// not listed.
pub fn profile_lookup(profile: &profiles::Profile) -> Result<()> {
    let _com = Com::start();
    let manager = profile_manager()?;
    let mut found = TF_INPUTPROCESSORPROFILE::default();
    // SAFETY: the GUID pointers and `found` live for the call.
    unsafe {
        manager.GetProfile(
            TF_PROFILETYPE_INPUTPROCESSOR,
            profile.lang_id,
            &guid(CLSID),
            &guid(profile.guid),
            HKL::default(),
            &mut found,
        )
    }
}

/// Where TSF keeps the machine-wide registration of a profile.
fn profile_key(profile: &profiles::Profile) -> String {
    format!(
        "SOFTWARE\\Microsoft\\CTF\\TIP\\{CLSID}\\LanguageProfile\\0x{:08x}\\{}",
        profile.lang_id, profile.guid
    )
}

/// Whether TSF has this profile of the text service registered (machine-wide,
/// in its own part of the registry), whatever the current user has enabled.
pub fn profile_registered(profile: &profiles::Profile) -> bool {
    key_exists(&profile_key(profile))
}

/// `InstallLayoutOrTip` of `input.dll`, the call Windows' own setup and every IME
/// installer use to add or remove an input method for the *current user*.
type InstallLayoutOrTip = unsafe extern "system" fn(*const u16, u32) -> BOOL;
const ILOT_UNINSTALL: u32 = 0x0000_0001;

fn install_layout_or_tip(spec: &str, flags: u32) -> Result<()> {
    // SAFETY: loading a system DLL by name.
    let library = unsafe { LoadLibraryW(w!("input.dll")) }?;
    // SAFETY: the name is a NUL-terminated string; the function has this signature.
    let function = unsafe { GetProcAddress(library, s!("InstallLayoutOrTip")) }
        .ok_or_else(Error::from_thread)?;
    // SAFETY: input.dll exports `BOOL InstallLayoutOrTip(LPCWSTR, DWORD)`.
    let function: InstallLayoutOrTip = unsafe { std::mem::transmute(function) };
    let spec = wide(spec);
    // SAFETY: `spec` is NUL-terminated and lives for the call.
    if unsafe { function(spec.as_ptr(), flags) }.as_bool() {
        Ok(())
    } else {
        Err(Error::from_thread())
    }
}

/// Add easime's profile for `language` to the current user's input methods
/// (this also adds the language to the user's list if it is not there). Needs
/// no administrator rights; the machine-wide registration must have happened.
pub fn enable_for_current_user(language: ime_core::Language) -> Result<()> {
    let profile = profiles::profile_of(language)
        .ok_or_else(|| Error::from(windows::Win32::Foundation::E_INVALIDARG))?;
    let _com = Com::start();
    install_layout_or_tip(&profiles::tip_spec(profile), 0)
}

/// Take it away again from the current user's input methods. (The language
/// itself stays in the user's list.)
pub fn disable_for_current_user(language: ime_core::Language) -> Result<()> {
    let profile = profiles::profile_of(language)
        .ok_or_else(|| Error::from(windows::Win32::Foundation::E_INVALIDARG))?;
    let _com = Com::start();
    install_layout_or_tip(&profiles::tip_spec(profile), ILOT_UNINSTALL)
}

/// Everything `DllRegisterServer` does, for the DLL at `dll`.
pub fn register(dll: &Path) -> Result<()> {
    let _com = Com::start();
    register_class(dll)?;
    if let Err(e) = register_profiles(dll) {
        // Leave nothing half-registered.
        let _ = unregister_profiles();
        let _ = unregister_class();
        return Err(e);
    }
    Ok(())
}

/// Everything `DllUnregisterServer` does.
pub fn unregister() -> Result<()> {
    let _com = Com::start();
    let profiles = unregister_profiles();
    let class = unregister_class();
    profiles.and(class)
}

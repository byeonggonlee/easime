//! Registering the text service with Windows, and taking it away again.
//!
//! Two things make an input method known to Windows: the COM class (a registry
//! key under `HKLM\SOFTWARE\Classes\CLSID` that says which DLL implements it)
//! and its TSF profile (what the language settings list, see
//! [`crate::profiles`]). Both are written by `DllRegisterServer`, which the
//! installer runs (`regsvr32`), and removed by `DllUnregisterServer`.
//!
//! Both need administrator rights (they are machine-wide). Doing either twice
//! is fine, so a repair or an upgrade over an old install works.

use crate::profiles::{self, CLSID};
use std::path::Path;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, FreeLibrary};
use windows::Win32::Globalization::{GetUserDefaultUILanguage, LocaleNameToLCID};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_OPTION_NON_VOLATILE,
    REG_SZ, RRF_RT_REG_MULTI_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegEnumKeyExW,
    RegEnumValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_TIP_KEYBOARD, GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT, ITfCategoryMgr,
    ITfInputProcessorProfileMgr, TF_INPUTPROCESSORPROFILE, TF_PROFILETYPE_INPUTPROCESSOR,
};
use windows::core::{BOOL, Error, GUID, PCWSTR, PWSTR, Result, s, w};

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

/// The TSF categories the text service is registered in:
///
/// * a keyboard text service (what the language settings list);
/// * one that works in Store apps ("immersive" mode). Without it Windows does
///   not use the input method there. In such apps the service may not be able
///   to read the settings file or write its log (they live in the user's own
///   folders); it then types with the default settings, as when the file is
///   missing;
/// * a display attribute provider: it says how its composition is underlined
///   ([`crate::display`]).
///
/// Not `SECUREMODE` (the sign-in screen and UAC prompts): a text service there
/// must not touch user files or show its own windows, which this one does.
pub const CATEGORIES: [GUID; 3] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
];

fn category_manager() -> Result<ITfCategoryMgr> {
    // SAFETY: an in-process COM class of Windows itself.
    unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER) }
}

/// The categories TSF has the text service in.
pub fn categories() -> Result<Vec<GUID>> {
    let _com = Com::start();
    let clsid = guid(CLSID);
    // SAFETY: the GUID pointer lives for the call.
    let list = unsafe { category_manager()?.EnumCategoriesInItem(&clsid) }?;
    let mut found = Vec::new();
    loop {
        let mut one = [GUID::zeroed()];
        let mut fetched = 0;
        // SAFETY: `one` and `fetched` live for the call.
        let next = unsafe { list.Next(&mut one, Some(&mut fetched)) };
        if next.is_err() || fetched == 0 {
            return Ok(found);
        }
        found.push(one[0]);
    }
}

fn profile_manager() -> Result<ITfInputProcessorProfileMgr> {
    // SAFETY: an in-process COM class of Windows itself.
    unsafe { CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER) }
}

/// Add the profile to the language settings (for every language), and the
/// text service to its categories. What an earlier build registered goes first,
/// so an upgrade leaves one easime.
pub fn register_profiles(dll: &Path) -> Result<()> {
    let clsid = guid(CLSID);
    let manager = profile_manager()?;
    for earlier in &profiles::EARLIER {
        // SAFETY: GUID pointers live for the call; "not there" is fine.
        let _ =
            unsafe { manager.UnregisterProfile(&clsid, earlier.lang_id, &guid(earlier.guid), 0) };
    }
    let icon: Vec<u16> = dll.to_string_lossy().encode_utf16().collect();
    let profile = profiles::REGISTERED;
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
    let categories = category_manager()?;
    for category in &CATEGORIES {
        // SAFETY: GUID pointers live for the call.
        unsafe { categories.RegisterCategory(&clsid, category, &clsid) }?;
    }
    Ok(())
}

/// Take the profiles and the category away. Profiles that are not there are
/// skipped: removal must work after a half-finished install too.
pub fn unregister_profiles() -> Result<()> {
    let clsid = guid(CLSID);
    let manager = profile_manager()?;
    for profile in profiles::EARLIER.iter().chain([&profiles::REGISTERED]) {
        // SAFETY: GUID pointers live for the call. An error means "was not
        // registered", which is what removal wants anyway.
        let _ =
            unsafe { manager.UnregisterProfile(&clsid, profile.lang_id, &guid(profile.guid), 0) };
    }
    let categories = category_manager()?;
    for category in &CATEGORIES {
        // SAFETY: GUID pointers live for the call; "not there" is fine.
        let _ = unsafe { categories.UnregisterCategory(&clsid, category, &clsid) };
    }
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
    let result = (|| {
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
    })();
    // SAFETY: balances the successful LoadLibraryW above.
    unsafe {
        let _ = FreeLibrary(library);
    }
    result
}

const USER_PROFILE: &str = "Control Panel\\International\\User Profile";

/// UTF-16 without the closing NUL, as a string.
fn text_of(units: &[u16]) -> String {
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

/// The names of the keys (`values: false`) or values (`true`) of the current
/// user's key at `path`.
fn user_names(path: &str, values: bool) -> Vec<String> {
    let path = wide(path);
    let mut key = HKEY::default();
    // SAFETY: `path` is NUL-terminated; `key` receives the opened key.
    let opened = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path.as_ptr()),
            None,
            KEY_READ,
            &mut key,
        )
    };
    if opened.is_err() {
        return Vec::new();
    }
    let mut names = Vec::new();
    for index in 0.. {
        let mut buffer = [0u16; 512];
        let mut length = buffer.len() as u32;
        let name = Some(PWSTR(buffer.as_mut_ptr()));
        // SAFETY: the buffer and its length live for the call.
        let next = unsafe {
            if values {
                RegEnumValueW(key, index, name, &mut length, None, None, None, None)
            } else {
                RegEnumKeyExW(key, index, name, &mut length, None, None, None, None)
            }
        };
        if next.is_err() {
            break;
        }
        names.push(text_of(&buffer[..length as usize]));
    }
    // SAFETY: closing the key opened above.
    unsafe {
        let _ = RegCloseKey(key);
    }
    names
}

/// The current user's languages, in the order of their language settings
/// (`Languages`, a list of language tags such as `en-US`, `ko`).
fn user_language_tags() -> Vec<String> {
    let (key, value) = (wide(USER_PROFILE), wide("Languages"));
    let mut buffer = vec![0u16; 4096];
    let mut bytes = (buffer.len() * 2) as u32;
    // SAFETY: the buffer holds `bytes` bytes; the names are NUL-terminated.
    let read = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_MULTI_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    if read.is_err() {
        return Vec::new();
    }
    buffer.truncate(bytes as usize / 2);
    buffer
        .split(|&unit| unit == 0)
        .filter(|tag| !tag.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}

/// The language id Windows has for a language tag, if any.
fn lang_id_of(tag: &str) -> Option<u16> {
    let tag = wide(tag);
    // SAFETY: `tag` is NUL-terminated.
    let lcid = unsafe { LocaleNameToLCID(PCWSTR(tag.as_ptr()), 0) };
    // The low word of a locale id is its language id.
    Some((lcid & 0xFFFF) as u16).filter(|&id| id != 0)
}

/// easime's entries in the current user's input methods, as specs
/// (`0409:{class}{profile}`), whichever language they are under.
fn user_entries() -> Vec<String> {
    user_names(USER_PROFILE, false)
        .into_iter()
        .flat_map(|language| user_names(&format!("{USER_PROFILE}\\{language}"), true))
        .filter(|entry| profiles::is_ours(entry))
        .collect()
}

/// Switch easime on in the current user's input methods, under the first of
/// their languages ([`profiles::language_to_switch_on_under`]), so no language
/// is added to their list. Entries an earlier build made (under Korean) are
/// taken away first. Returns the language it is switched on under. Needs no
/// administrator rights; the machine-wide registration must have happened.
pub fn enable_for_current_user() -> Result<u16> {
    let _com = Com::start();
    let languages: Vec<Option<u16>> = user_language_tags().iter().map(|t| lang_id_of(t)).collect();
    // SAFETY: a plain query.
    let display = Some(unsafe { GetUserDefaultUILanguage() });
    let language = profiles::language_to_switch_on_under(&languages, display);
    let wanted = profiles::tip_spec(language, &profiles::REGISTERED);
    for entry in user_entries() {
        if !entry.eq_ignore_ascii_case(&wanted) {
            let _ = install_layout_or_tip(&entry, ILOT_UNINSTALL);
        }
    }
    install_layout_or_tip(&wanted, 0)?;
    Ok(language)
}

/// Take easime away from the current user's input methods, under whichever
/// languages it is (the languages themselves stay). Returns how many entries
/// were taken away.
pub fn disable_for_current_user() -> Result<usize> {
    let _com = Com::start();
    let entries = user_entries();
    for entry in &entries {
        install_layout_or_tip(entry, ILOT_UNINSTALL)?;
    }
    Ok(entries.len())
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

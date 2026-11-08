//! An experiment, not a check of easime (docs/windows-plan-2026-11-08.md, step 2):
//! can one input method be offered under **every** language of Windows?
//!
//! The documented way is the older `ITfInputProcessorProfiles::AddLanguageProfile`
//! with the language `-1` ("the text service is added to all languages"); the
//! newer `ITfInputProcessorProfileMgr::RegisterProfile`, which easime uses,
//! documents no such value. Each way is tried with a throwaway class and profile
//! (nothing of easime's), next to a registration under Japanese alone for
//! comparison, and what Windows then says is printed: what it wrote to the
//! registry, which languages list the profile, and whether the current user can
//! switch it on under a language the way the installer does
//! (`InstallLayoutOrTip`). Everything is taken away again.
//!
//! It changes the machine and the current user's language list, so it is
//! ignored by default; the CI's Windows runner runs it with
//! `cargo test -p frontend-tsf --test languages -- --ignored --nocapture`.
//! Whether Windows' Settings window shows the profile cannot be seen from here:
//! that is for a real PC (docs/windows-testing.md).
#![cfg(windows)]

use frontend_tsf::registration::guid;
use windows::Win32::Foundation::FreeLibrary;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_OPTION_NON_VOLATILE,
    REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegDeleteValueW, RegEnumKeyExW,
    RegEnumValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, GUID_TFCAT_TIP_KEYBOARD, ITfCategoryMgr,
    ITfInputProcessorProfileMgr, ITfInputProcessorProfiles, TF_INPUTPROCESSORPROFILE,
    TF_IPP_FLAG_ENABLED, TF_LANGUAGEPROFILE, TF_PROFILETYPE_INPUTPROCESSOR,
};
use windows::core::{BOOL, Error, GUID, PCWSTR, PWSTR, Result, s, w};

/// "Every language": the documented `-1` of `AddLanguageProfile`, as a LANGID.
const EVERY_LANGUAGE: u16 = 0xFFFF;
const JAPANESE: u16 = 0x0411;

/// The languages whose lists are looked at: English (what a CI runner has),
/// Japanese and Korean (easime's), and German (one easime has nothing to do with).
const LANGUAGES: [(u16, &str); 4] = [
    (0x0409, "en-US"),
    (JAPANESE, "ja-JP"),
    (0x0412, "ko-KR"),
    (0x0407, "de-DE"),
];

/// The language parts of `InstallLayoutOrTip` specs to try.
const SWITCH_ON_UNDER: [&str; 3] = ["0409", "0411", "FFFF"];

const USER_PROFILE: &str = "Control Panel\\International\\User Profile";

#[derive(Clone, Copy)]
enum Way {
    OldApiEveryLanguage,
    NewApiEveryLanguage,
    NewApiJapanese,
}

struct Case {
    way: Way,
    name: &'static str,
    clsid: &'static str,
    profile: &'static str,
}

const CASES: [Case; 3] = [
    Case {
        way: Way::OldApiEveryLanguage,
        name: "AddLanguageProfile(-1)",
        clsid: "{5209916B-7A70-4CF9-B157-B10D3FECD2DC}",
        profile: "{1F0571A3-9B88-48B6-B13F-7C45D44CBAE8}",
    },
    Case {
        way: Way::NewApiEveryLanguage,
        name: "RegisterProfile(0xFFFF)",
        clsid: "{4AA3A5A9-949B-4C41-A541-2A491D10E3C9}",
        profile: "{1584F0F6-1991-4BC8-856B-63D13C19FA89}",
    },
    Case {
        way: Way::NewApiJapanese,
        name: "RegisterProfile(ja-JP)",
        clsid: "{E0416917-AC43-42ED-BC05-7029A8EF4604}",
        profile: "{FAB3FAD8-D0AC-47CE-BAD2-9DA635782FD2}",
    },
];

fn report(case: &str, what: &str, result: impl std::fmt::Display) {
    println!("EXPERIMENT {case} | {what}: {result}");
}

fn outcome(result: &Result<()>) -> String {
    match result {
        Ok(()) => "ok".to_string(),
        Err(e) => failure(e),
    }
}

fn failure(e: &Error) -> String {
    format!("error 0x{:08X} ({})", e.code().0 as u32, e.message())
}

/// UTF-16 with a closing NUL.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

/// UTF-16 without a NUL, as TSF takes descriptions.
fn text(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn open(
    root: HKEY,
    path: &str,
    access: windows::Win32::System::Registry::REG_SAM_FLAGS,
) -> Option<HKEY> {
    let path = wide(path);
    let mut key = HKEY::default();
    // SAFETY: `path` is NUL-terminated; `key` receives the opened key.
    let opened = unsafe { RegOpenKeyExW(root, PCWSTR(path.as_ptr()), None, access, &mut key) };
    opened.is_ok().then_some(key)
}

fn close(key: HKEY) {
    // SAFETY: a key opened by `open` or created below.
    unsafe {
        let _ = RegCloseKey(key);
    }
}

/// The names of the keys below `path`.
fn subkeys(root: HKEY, path: &str) -> Vec<String> {
    let Some(key) = open(root, path, KEY_READ) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for index in 0.. {
        let mut buffer = [0u16; 256];
        let mut length = buffer.len() as u32;
        // SAFETY: the buffer and its length live for the call.
        let next = unsafe {
            RegEnumKeyExW(
                key,
                index,
                Some(PWSTR(buffer.as_mut_ptr())),
                &mut length,
                None,
                None,
                None,
                None,
            )
        };
        if next.is_err() {
            break;
        }
        names.push(String::from_utf16_lossy(&buffer[..length as usize]));
    }
    close(key);
    names
}

/// The names of the values of the key at `path`.
fn value_names(root: HKEY, path: &str) -> Vec<String> {
    let Some(key) = open(root, path, KEY_READ) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for index in 0.. {
        let mut buffer = [0u16; 512];
        let mut length = buffer.len() as u32;
        // SAFETY: the buffer and its length live for the call.
        let next = unsafe {
            RegEnumValueW(
                key,
                index,
                Some(PWSTR(buffer.as_mut_ptr())),
                &mut length,
                None,
                None,
                None,
                None,
            )
        };
        if next.is_err() {
            break;
        }
        names.push(String::from_utf16_lossy(&buffer[..length as usize]));
    }
    close(key);
    names
}

fn tip_key(clsid: &str) -> String {
    format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{clsid}")
}

/// What TSF wrote for the class: `<language key>/<profile>` pairs.
fn registered_profiles(clsid: &str) -> Vec<String> {
    let languages = format!("{}\\LanguageProfile", tip_key(clsid));
    subkeys(HKEY_LOCAL_MACHINE, &languages)
        .into_iter()
        .flat_map(|language| {
            subkeys(HKEY_LOCAL_MACHINE, &format!("{languages}\\{language}"))
                .into_iter()
                .map(move |profile| format!("{language}/{profile}"))
        })
        .collect()
}

/// The current user's languages (the keys of their language settings).
fn user_languages() -> Vec<String> {
    subkeys(HKEY_CURRENT_USER, USER_PROFILE)
}

/// Where the current user has the class switched on: `<language>: <spec>`.
fn user_tips(clsid: &str) -> Vec<String> {
    user_languages()
        .into_iter()
        .flat_map(|language| {
            value_names(HKEY_CURRENT_USER, &format!("{USER_PROFILE}\\{language}"))
                .into_iter()
                .filter(|name| name.to_uppercase().contains(clsid))
                .map(move |name| format!("{language}: {name}"))
        })
        .collect()
}

/// Take any switched-on entry of the class out of the user's settings.
fn forget_user_tips(clsid: &str) {
    for language in user_languages() {
        let path = format!("{USER_PROFILE}\\{language}");
        let names: Vec<String> = value_names(HKEY_CURRENT_USER, &path)
            .into_iter()
            .filter(|name| name.to_uppercase().contains(clsid))
            .collect();
        if names.is_empty() {
            continue;
        }
        if let Some(key) = open(HKEY_CURRENT_USER, &path, KEY_WRITE) {
            for name in names {
                let name = wide(&name);
                // SAFETY: an open key and a NUL-terminated name.
                unsafe {
                    let _ = RegDeleteValueW(key, PCWSTR(name.as_ptr()));
                }
            }
            close(key);
        }
    }
}

/// A COM class entry, as a real input method has (nothing loads it here).
fn create_class(clsid: &str, dll: &str) {
    let path = wide(&format!(
        "SOFTWARE\\Classes\\CLSID\\{clsid}\\InprocServer32"
    ));
    let mut key = HKEY::default();
    // SAFETY: `path` is NUL-terminated; `key` receives the created key.
    let created = unsafe {
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
    };
    assert!(
        created.is_ok(),
        "creating a class needs administrator rights"
    );
    let data = wide(dll);
    // SAFETY: `data` is a NUL-terminated UTF-16 string that outlives the call.
    let set = unsafe {
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
    };
    assert!(set.is_ok());
    close(key);
}

fn delete_tree(path: &str) {
    let path = wide(path);
    // SAFETY: a NUL-terminated path below HKLM; a missing key is fine.
    unsafe {
        let _ = RegDeleteTreeW(HKEY_LOCAL_MACHINE, PCWSTR(path.as_ptr()));
    }
}

/// `BOOL InstallLayoutOrTip(LPCWSTR, DWORD)` of `input.dll` (what the installer uses).
type InstallLayoutOrTip = unsafe extern "system" fn(*const u16, u32) -> BOOL;
const ILOT_UNINSTALL: u32 = 0x0000_0001;

fn install_layout_or_tip(spec: &str, flags: u32) -> Result<()> {
    // SAFETY: loading a system DLL by name.
    let library = unsafe { LoadLibraryW(w!("input.dll")) }?;
    let result = (|| {
        // SAFETY: the name is NUL-terminated; the function has this signature.
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
    // SAFETY: balances the LoadLibraryW above.
    unsafe {
        let _ = FreeLibrary(library);
    }
    result
}

/// Whether the newer interface lists the class for `langid`, and as what.
fn listed_by_manager(manager: &ITfInputProcessorProfileMgr, langid: u16, clsid: &GUID) -> String {
    // SAFETY: a plain call on a live interface.
    let list = match unsafe { manager.EnumProfiles(langid) } {
        Ok(list) => list,
        Err(e) => return format!("EnumProfiles {}", failure(&e)),
    };
    let mut found = Vec::new();
    loop {
        let mut one = [TF_INPUTPROCESSORPROFILE::default()];
        let mut fetched = 0;
        // SAFETY: `one` and `fetched` live for the call.
        if unsafe { list.Next(&mut one, &mut fetched) }.is_err() || fetched == 0 {
            break;
        }
        if one[0].clsid == *clsid {
            found.push(format!(
                "langid=0x{:04X} enabled={}",
                one[0].langid,
                one[0].dwFlags & TF_IPP_FLAG_ENABLED != 0
            ));
        }
    }
    if found.is_empty() {
        "no".to_string()
    } else {
        format!("yes ({})", found.join("; "))
    }
}

/// Whether the older interface lists the class for `langid`.
fn listed_by_profiles(profiles: &ITfInputProcessorProfiles, langid: u16, clsid: &GUID) -> String {
    // SAFETY: a plain call on a live interface.
    let list = match unsafe { profiles.EnumLanguageProfiles(langid) } {
        Ok(list) => list,
        Err(e) => return format!("EnumLanguageProfiles {}", failure(&e)),
    };
    let mut found = Vec::new();
    loop {
        let mut one = [TF_LANGUAGEPROFILE::default()];
        let mut fetched = 0;
        // SAFETY: `one` and `fetched` live for the call.
        if unsafe { list.Next(&mut one, &mut fetched) }.is_err() || fetched == 0 {
            break;
        }
        if one[0].clsid == *clsid {
            found.push(format!("langid=0x{:04X}", one[0].langid));
        }
    }
    if found.is_empty() {
        "no".to_string()
    } else {
        format!("yes ({})", found.join("; "))
    }
}

struct Tsf {
    profiles: ITfInputProcessorProfiles,
    manager: ITfInputProcessorProfileMgr,
    categories: ITfCategoryMgr,
}

impl Tsf {
    fn new() -> Self {
        // SAFETY: in-process COM classes of Windows itself.
        unsafe {
            Self {
                profiles: CoCreateInstance(
                    &CLSID_TF_InputProcessorProfiles,
                    None,
                    CLSCTX_INPROC_SERVER,
                )
                .unwrap(),
                manager: CoCreateInstance(
                    &CLSID_TF_InputProcessorProfiles,
                    None,
                    CLSCTX_INPROC_SERVER,
                )
                .unwrap(),
                categories: CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)
                    .unwrap(),
            }
        }
    }

    fn register(&self, case: &Case, icon: &[u16]) -> Result<()> {
        let (clsid, profile) = (guid(case.clsid), guid(case.profile));
        let name = text("easime registration experiment");
        // SAFETY: the GUIDs and slices live for the calls.
        unsafe {
            match case.way {
                Way::OldApiEveryLanguage => {
                    self.profiles.Register(&clsid)?;
                    self.profiles.AddLanguageProfile(
                        &clsid,
                        EVERY_LANGUAGE,
                        &profile,
                        &name,
                        icon,
                        0,
                    )
                }
                Way::NewApiEveryLanguage => self.manager.RegisterProfile(
                    &clsid,
                    EVERY_LANGUAGE,
                    &profile,
                    &name,
                    icon,
                    0,
                    HKL::default(),
                    0,
                    true,
                    0,
                ),
                Way::NewApiJapanese => self.manager.RegisterProfile(
                    &clsid,
                    JAPANESE,
                    &profile,
                    &name,
                    icon,
                    0,
                    HKL::default(),
                    0,
                    true,
                    0,
                ),
            }
        }
    }

    /// Take everything of the case away, whichever way it was registered.
    fn clean(&self, case: &Case) {
        let (clsid, profile) = (guid(case.clsid), guid(case.profile));
        for spec in SWITCH_ON_UNDER {
            let _ = install_layout_or_tip(
                &format!("{spec}:{}{}", case.clsid, case.profile),
                ILOT_UNINSTALL,
            );
        }
        forget_user_tips(case.clsid);
        // SAFETY: GUID pointers live for the calls; errors mean "not there".
        unsafe {
            for langid in [EVERY_LANGUAGE, JAPANESE] {
                let _ = self.manager.UnregisterProfile(&clsid, langid, &profile, 0);
            }
            let _ = self
                .categories
                .UnregisterCategory(&clsid, &GUID_TFCAT_TIP_KEYBOARD, &clsid);
            let _ = self.profiles.Unregister(&clsid);
        }
        delete_tree(&tip_key(case.clsid));
        delete_tree(&format!("SOFTWARE\\Classes\\CLSID\\{}", case.clsid));
    }
}

#[test]
#[ignore = "an experiment that changes the machine: the CI runs it with --ignored --nocapture"]
fn which_registration_offers_one_input_method_under_every_language() {
    // SAFETY: COM for this thread, for the rest of the test.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let tsf = Tsf::new();
    // Any file will do as "the DLL": nothing loads it.
    let dll = std::env::current_exe()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let icon = text(&dll);
    report("-", "user languages before", user_languages().join(", "));

    for case in &CASES {
        let (clsid, profile) = (guid(case.clsid), guid(case.profile));
        tsf.clean(case); // a clean start, in case an earlier run stopped half way
        create_class(case.clsid, &dll);

        report(case.name, "register", outcome(&tsf.register(case, &icon)));
        // SAFETY: GUID pointers live for the call.
        let category = unsafe {
            tsf.categories
                .RegisterCategory(&clsid, &GUID_TFCAT_TIP_KEYBOARD, &clsid)
        };
        report(case.name, "keyboard category", outcome(&category));
        report(
            case.name,
            "registry (language/profile)",
            registered_profiles(case.clsid).join(", "),
        );

        for (langid, tag) in LANGUAGES {
            report(
                case.name,
                &format!("{tag} EnumProfiles lists it"),
                listed_by_manager(&tsf.manager, langid, &clsid),
            );
            report(
                case.name,
                &format!("{tag} EnumLanguageProfiles lists it"),
                listed_by_profiles(&tsf.profiles, langid, &clsid),
            );
            let mut found = TF_INPUTPROCESSORPROFILE::default();
            // SAFETY: the GUIDs and `found` live for the call.
            let got = unsafe {
                tsf.manager.GetProfile(
                    TF_PROFILETYPE_INPUTPROCESSOR,
                    langid,
                    &clsid,
                    &profile,
                    HKL::default(),
                    &mut found,
                )
            };
            let got = match got {
                Ok(()) => format!(
                    "ok (langid=0x{:04X}, enabled={})",
                    found.langid,
                    found.dwFlags & TF_IPP_FLAG_ENABLED != 0
                ),
                Err(e) => failure(&e),
            };
            report(case.name, &format!("{tag} GetProfile"), got);
        }

        // Switching it on for this user the way the installer does, under a
        // language, then off again.
        for language in SWITCH_ON_UNDER {
            let spec = format!("{language}:{}{}", case.clsid, case.profile);
            let on = install_layout_or_tip(&spec, 0);
            report(
                case.name,
                &format!("switch on {language}"),
                format!("{} -> [{}]", outcome(&on), user_tips(case.clsid).join(", ")),
            );
            let off = install_layout_or_tip(&spec, ILOT_UNINSTALL);
            report(
                case.name,
                &format!("switch off {language}"),
                format!(
                    "{} -> [{}]",
                    outcome(&off),
                    user_tips(case.clsid).join(", ")
                ),
            );
        }

        tsf.clean(case);
        assert!(
            registered_profiles(case.clsid).is_empty(),
            "{}: the registration is gone",
            case.name
        );
        assert!(
            user_tips(case.clsid).is_empty(),
            "{}: nothing is left in the user's settings",
            case.name
        );
    }
    report("-", "user languages after", user_languages().join(", "));
}

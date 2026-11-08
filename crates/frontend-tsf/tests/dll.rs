//! The built DLL, the way Windows uses it: `regsvr32` registers it through its
//! exported `DllRegisterServer`, COM creates the text service from the DLL it
//! finds in the registry (`DllGetClassObject`), and `regsvr32 /u` removes it.
//!
//! The DLL is built by `cargo build -p frontend-tsf` (the CI does that first and
//! sets `EASIME_REQUIRE_DLL`); without it this test says so and skips.
#![cfg(windows)]

use frontend_tsf::{profiles, registration};
use std::path::PathBuf;
use std::process::Command;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::{
    CLSID_TF_InputProcessorProfiles, CLSID_TF_ThreadMgr, ITfInputProcessorProfileMgr,
    ITfKeystrokeMgr, ITfTextInputProcessor, ITfThreadMgr, TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE,
    TF_IPPMF_ENABLEPROFILE, TF_IPPMF_FORPROCESS, TF_PROFILETYPE_INPUTPROCESSOR,
};
use windows::core::Interface;

/// `target/<profile>/frontend_tsf.dll`, next to the test's own folder.
fn dll() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dll = exe.parent()?.parent()?.join("frontend_tsf.dll");
    dll.is_file().then_some(dll)
}

fn regsvr32(dll: &PathBuf, unregister: bool) -> bool {
    let mut command = Command::new("regsvr32.exe");
    command.arg("/s");
    if unregister {
        command.arg("/u");
    }
    command
        .arg(dll)
        .status()
        .is_ok_and(|status| status.success())
}

#[test]
fn regsvr32_registers_the_dll_and_com_creates_the_service_from_it() {
    let Some(dll) = dll() else {
        assert!(
            std::env::var_os("EASIME_REQUIRE_DLL").is_none(),
            "the DLL was not built: run `cargo build -p frontend-tsf` first"
        );
        eprintln!("SKIPPED: frontend_tsf.dll is not built (cargo build -p frontend-tsf)");
        return;
    };
    // A clean start.
    registration::unregister().unwrap();

    assert!(regsvr32(&dll, false), "regsvr32 could not register the DLL");
    assert!(
        registration::class_registered(),
        "the COM class is registered"
    );

    // COM finds the DLL in the registry, loads it and asks for the service.
    // SAFETY: plain COM initialization of this test's thread.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .unwrap();
    let clsid = registration::guid(profiles::CLSID);
    // SAFETY: the class was registered above, for this DLL.
    let service: ITfTextInputProcessor =
        unsafe { CoCreateInstance(&clsid, None, CLSCTX_INPROC_SERVER) }
            .expect("COM creates the text service from the registered DLL");
    drop(service);

    // And TSF itself activates it: activating the profile for this process makes
    // TSF load the DLL, create the service and activate it on a thread manager,
    // which then reports easime's key event sink as the foreground one.
    activates_through_tsf(&clsid);

    assert!(
        regsvr32(&dll, true),
        "regsvr32 could not unregister the DLL"
    );
    // Not checked: whether COM can still make the service. This process loaded
    // the DLL and COM keeps the class it got from it until the process ends.
    assert!(!registration::class_registered(), "the COM class is gone");
}

/// Have TSF activate easime's first profile in this process, and look at what
/// the thread manager says.
fn activates_through_tsf(clsid: &windows::core::GUID) {
    let profile = &profiles::registered()[0];
    // SAFETY: Windows' own in-process COM classes.
    let thread_manager: ITfThreadMgr =
        unsafe { CoCreateInstance(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER) }.unwrap();
    unsafe { thread_manager.Activate() }.expect("thread manager activation");
    let manager: ITfInputProcessorProfileMgr =
        unsafe { CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER) }
            .unwrap();
    // For this process only: nothing is changed in the user's settings.
    unsafe {
        manager.ActivateProfile(
            TF_PROFILETYPE_INPUTPROCESSOR,
            profile.lang_id,
            clsid,
            &registration::guid(profile.guid),
            HKL::default(),
            TF_IPPMF_FORPROCESS | TF_IPPMF_ENABLEPROFILE | TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE,
        )
    }
    .unwrap_or_else(|e| panic!("TSF could not activate {}: {e}", profile.name()));
    let keystrokes: ITfKeystrokeMgr = thread_manager.cast().unwrap();
    assert_eq!(
        unsafe { keystrokes.GetForeground() }.unwrap(),
        *clsid,
        "TSF reports easime's sink as the foreground key event sink"
    );
    unsafe { thread_manager.Deactivate() }.expect("thread manager deactivation");
}

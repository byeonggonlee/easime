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
use windows::Win32::UI::TextServices::ITfTextInputProcessor;

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

    assert!(
        regsvr32(&dll, true),
        "regsvr32 could not unregister the DLL"
    );
    assert!(!registration::class_registered(), "the COM class is gone");
    // SAFETY: as above; the class is no longer registered.
    let gone =
        unsafe { CoCreateInstance::<_, ITfTextInputProcessor>(&clsid, None, CLSCTX_INPROC_SERVER) };
    assert!(gone.is_err(), "COM no longer knows the class");
}

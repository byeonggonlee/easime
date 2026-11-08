//! The text service's factory, in this process: it makes services, counts them
//! while they live, and lets the DLL be unloaded when they are gone. (That TSF
//! itself activates the service is checked with the built DLL, in `dll.rs`.)
//! One test only: the count is process-wide, which a second test running at the
//! same time would disturb.
#![cfg(windows)]

use frontend_tsf::service;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::UI::TextServices::{ITfTextInputProcessor, ITfTextInputProcessorEx};
use windows::core::Interface;

#[test]
fn the_factory_makes_counted_services_that_can_be_released() {
    // SAFETY: plain COM initialization of this test's thread.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .unwrap();
    assert!(service::unloadable(), "nothing of the service exists yet");

    let factory = service::class_factory();
    // SAFETY: an ordinary COM call on a live factory.
    let extended: ITfTextInputProcessorEx = unsafe { factory.CreateInstance(None) }.unwrap();
    assert!(!service::unloadable(), "a service is alive");

    // The same object answers for the base interface too.
    let base: ITfTextInputProcessor = extended.cast().unwrap();
    // Deactivating a service that was never activated is harmless.
    unsafe { base.Deactivate() }.expect("deactivating an inactive service");

    // A lock on the factory also keeps the DLL in memory.
    drop(base);
    drop(extended);
    assert!(service::unloadable(), "the services are gone");
    unsafe { factory.LockServer(true) }.unwrap();
    assert!(!service::unloadable(), "a locked factory is in use");
    unsafe { factory.LockServer(false) }.unwrap();
    assert!(service::unloadable(), "unlocked again");
    assert!(
        unsafe { factory.LockServer(false) }.is_err(),
        "more unlocks than locks is refused"
    );
    assert!(service::unloadable(), "and does not break the count");
}

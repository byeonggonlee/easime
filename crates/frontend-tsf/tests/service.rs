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
    assert!(
        !service::unloadable(),
        "the factory itself keeps the DLL: COM may hold it without a lock"
    );
    // SAFETY: an ordinary COM call on a live factory.
    let extended: ITfTextInputProcessorEx = unsafe { factory.CreateInstance(None) }.unwrap();

    // The same object answers for the base interface too.
    let base: ITfTextInputProcessor = extended.cast().unwrap();
    // Deactivating a service that was never activated is harmless.
    unsafe { base.Deactivate() }.expect("deactivating an inactive service");
    drop(base);
    drop(extended);
    assert!(!service::unloadable(), "the factory is still there");

    // A lock on the factory also keeps the DLL in memory, after the factory.
    unsafe { factory.LockServer(true) }.unwrap();
    let locker = factory.clone();
    drop(factory);
    assert!(!service::unloadable(), "a locked server is in use");
    unsafe { locker.LockServer(false) }.unwrap();
    assert!(
        unsafe { locker.LockServer(false) }.is_err(),
        "more unlocks than locks is refused"
    );
    drop(locker);
    assert!(
        service::unloadable(),
        "no service, no factory, no lock: the DLL may go"
    );
}

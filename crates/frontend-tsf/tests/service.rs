//! The text service with the real Windows thread manager: it can be created by
//! its factory, TSF accepts its key event sink, and it goes away cleanly.
//! One test only: the service counts itself in a process-wide number, which a
//! second test running at the same time would disturb.
#![cfg(windows)]

use frontend_tsf::{profiles, registration, service};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::UI::TextServices::{
    CLSID_TF_ThreadMgr, ITfClientId, ITfKeystrokeMgr, ITfTextInputProcessorEx, ITfThreadMgr,
};
use windows::core::Interface;

#[test]
fn the_service_activates_with_the_real_thread_manager_and_goes_away() {
    // SAFETY: plain COM initialization of this test's thread.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .unwrap();
    let clsid = registration::guid(profiles::CLSID);
    assert!(service::unloadable(), "nothing of the service exists yet");

    // The factory makes services, and they are counted while they live.
    let factory = service::class_factory();
    // SAFETY: an ordinary COM call on a live factory.
    let tip: ITfTextInputProcessorEx = unsafe { factory.CreateInstance(None) }.unwrap();
    assert!(!service::unloadable(), "a service is alive");

    // Activate it the way TSF does, with a real thread manager.
    // SAFETY: Windows' own in-process COM class.
    let thread_manager: ITfThreadMgr =
        unsafe { CoCreateInstance(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER) }.unwrap();
    // The thread manager has to be active, as it is inside an application.
    let application_id = unsafe { thread_manager.Activate() }.expect("thread manager activation");
    let client_id = unsafe {
        thread_manager
            .cast::<ITfClientId>()
            .unwrap()
            .GetClientId(&clsid)
    }
    .expect("the thread manager gives the service a client id");
    eprintln!("application id {application_id}, easime's client id {client_id}");
    unsafe { tip.ActivateEx(&thread_manager, client_id, 0) }.expect("activation");

    // TSF now has its key event sink as the foreground one.
    let keystrokes: ITfKeystrokeMgr = thread_manager.cast().unwrap();
    assert_eq!(
        unsafe { keystrokes.GetForeground() }.unwrap(),
        clsid,
        "TSF reports easime's sink as the foreground key event sink"
    );

    // Deactivating takes the sink away again.
    unsafe { tip.Deactivate() }.expect("deactivation");
    let foreground = unsafe { keystrokes.GetForeground() };
    assert!(
        foreground.map(|guid| guid != clsid).unwrap_or(true),
        "the sink is gone after deactivation"
    );

    // Everything released: the DLL could be unloaded.
    drop(keystrokes);
    unsafe { thread_manager.Deactivate() }.expect("thread manager deactivation");
    drop(thread_manager);
    drop(tip);
    drop(factory);
    assert!(service::unloadable(), "nothing of the service is left");
}

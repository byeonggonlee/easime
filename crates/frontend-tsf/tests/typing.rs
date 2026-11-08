//! Typing through the real TSF: an application is simulated by a thread manager
//! with a document (a text store that holds a string), easime's DLL is
//! registered and activated for this process like Windows does it, and keys are
//! sent through `ITfKeystrokeMgr` the way TSF-aware applications do (ask with
//! `TestKeyDown`, act with `KeyDown`). What lands in the document is what a
//! user would see.
//!
//! The DLL is built by `cargo build -p frontend-tsf` (the CI does that first and
//! sets `EASIME_REQUIRE_DLL`); without it this test says so and skips.
#![cfg(windows)]

use frontend_tsf::{profiles, registration};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use windows::Win32::Foundation::{
    E_FAIL, E_NOTIMPL, E_UNEXPECTED, HWND, LPARAM, POINT, RECT, S_OK, WPARAM,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, FORMATETC,
    IDataObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Input::KeyboardAndMouse::{HKL, SetFocus};
use windows::Win32::UI::TextServices::{
    CLSID_TF_InputProcessorProfiles, CLSID_TF_ThreadMgr, GUID_COMPARTMENT_EMPTYCONTEXT,
    GUID_COMPARTMENT_KEYBOARD_DISABLED, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,
    IEnumITfCompositionView, ITextStoreACP, ITextStoreACP_Impl, ITextStoreACPSink,
    ITfCompartmentMgr, ITfContext, ITfContextComposition, ITfInputProcessorProfileMgr,
    ITfKeystrokeMgr, ITfThreadMgr, TEXT_STORE_LOCK_FLAGS, TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE,
    TF_IPPMF_ENABLEPROFILE, TF_IPPMF_FORPROCESS, TF_PROFILETYPE_INPUTPROCESSOR, TS_AE_END,
    TS_ATTRVAL, TS_E_SYNCHRONOUS, TS_LF_SYNC, TS_RT_PLAIN, TS_RUNINFO, TS_S_ASYNC,
    TS_SELECTION_ACP, TS_SELECTIONSTYLE, TS_STATUS, TS_TEXTCHANGE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, FindWindowW, GetWindowRect, IsWindowVisible,
    MSG, PM_REMOVE, PeekMessageW, SetForegroundWindow, TranslateMessage, WINDOW_EX_STYLE,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};
use windows::core::{
    BOOL, GUID, HRESULT, IUnknown, Interface, PCWSTR, PWSTR, Ref, Result, implement, w,
};

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

/// What the application's text field holds: UTF-16 text and a selection, plus
/// a note of every call TSF made, to read when something goes wrong.
#[derive(Default)]
struct Field {
    text: Vec<u16>,
    selection: (i32, i32),
    trace: Vec<String>,
}

impl Field {
    fn string(&self) -> String {
        String::from_utf16_lossy(&self.text)
    }

    /// The application inserts text at the cursor itself (a key it was given).
    fn insert(&mut self, text: &str) {
        let wide: Vec<u16> = text.encode_utf16().collect();
        let (start, end) = self.selection;
        self.text
            .splice(start as usize..end as usize, wide.iter().copied());
        let at = start + wide.len() as i32;
        self.selection = (at, at);
    }

    /// The application deletes the character before the cursor.
    fn backspace(&mut self) {
        let (start, end) = self.selection;
        if start == end && start > 0 {
            self.text.remove(start as usize - 1);
            self.selection = (start - 1, start - 1);
        } else if start != end {
            self.text.drain(start as usize..end as usize);
            self.selection = (start, start);
        }
    }
}

/// A text store (what an application gives TSF to edit its text) over a
/// [`Field`]: the smallest one that lets TSF compose in it. Calls TSF should
/// not need answer "not implemented".
#[implement(ITextStoreACP)]
struct TextStore {
    field: Rc<RefCell<Field>>,
    sink: RefCell<Option<ITextStoreACPSink>>,
    locked: Cell<bool>,
}

impl TextStore {
    fn note(&self, call: impl FnOnce() -> String) {
        self.field.borrow_mut().trace.push(call());
    }
}

#[allow(non_snake_case)]
impl ITextStoreACP_Impl for TextStore_Impl {
    fn AdviseSink(&self, riid: *const GUID, punk: Ref<IUnknown>, mask: u32) -> Result<()> {
        self.note(|| format!("AdviseSink mask={mask:#x}"));
        // SAFETY: TSF passes a valid IID.
        if unsafe { *riid } != ITextStoreACPSink::IID {
            return Err(windows::Win32::Foundation::E_INVALIDARG.into());
        }
        *self.sink.borrow_mut() = Some(punk.ok()?.cast()?);
        Ok(())
    }

    fn UnadviseSink(&self, _punk: Ref<IUnknown>) -> Result<()> {
        self.note(|| "UnadviseSink".into());
        *self.sink.borrow_mut() = None;
        Ok(())
    }

    fn RequestLock(&self, flags: u32) -> Result<HRESULT> {
        self.note(|| format!("RequestLock {flags:#x}"));
        let sink = self.sink.borrow().clone().ok_or(E_UNEXPECTED)?;
        if self.locked.get() {
            // Already locked: only an asynchronous request can wait.
            return Ok(if flags & TS_LF_SYNC != 0 {
                TS_E_SYNCHRONOUS
            } else {
                TS_S_ASYNC
            });
        }
        self.locked.set(true);
        // SAFETY: the sink is alive.
        let granted = unsafe { sink.OnLockGranted(TEXT_STORE_LOCK_FLAGS(flags & !TS_LF_SYNC)) };
        self.locked.set(false);
        Ok(match granted {
            Ok(()) => S_OK,
            Err(e) => e.code(),
        })
    }

    fn GetStatus(&self) -> Result<TS_STATUS> {
        Ok(TS_STATUS::default())
    }

    fn QueryInsert(
        &self,
        start: i32,
        end: i32,
        _cch: u32,
        result_start: *mut i32,
        result_end: *mut i32,
    ) -> Result<()> {
        // SAFETY: TSF passes valid pointers.
        unsafe {
            *result_start = start;
            *result_end = end;
        }
        Ok(())
    }

    fn GetSelection(
        &self,
        _index: u32,
        count: u32,
        selection: *mut TS_SELECTION_ACP,
        fetched: *mut u32,
    ) -> Result<()> {
        let (start, end) = self.field.borrow().selection;
        self.note(|| format!("GetSelection -> {start},{end}"));
        // SAFETY: TSF passes room for `count` selections.
        unsafe {
            if count >= 1 {
                *selection = TS_SELECTION_ACP {
                    acpStart: start,
                    acpEnd: end,
                    style: TS_SELECTIONSTYLE {
                        ase: TS_AE_END,
                        fInterimChar: false.into(),
                    },
                };
                *fetched = 1;
            } else {
                *fetched = 0;
            }
        }
        Ok(())
    }

    fn SetSelection(&self, count: u32, selection: *const TS_SELECTION_ACP) -> Result<()> {
        if count >= 1 {
            // SAFETY: TSF passes `count` selections.
            let selection = unsafe { *selection };
            self.note(|| format!("SetSelection {},{}", selection.acpStart, selection.acpEnd));
            self.field.borrow_mut().selection = (selection.acpStart, selection.acpEnd);
        }
        Ok(())
    }

    fn GetText(
        &self,
        start: i32,
        end: i32,
        plain: PWSTR,
        plain_wanted: u32,
        plain_returned: *mut u32,
        runs: *mut TS_RUNINFO,
        runs_wanted: u32,
        runs_returned: *mut u32,
        next: *mut i32,
    ) -> Result<()> {
        let field = self.field.borrow();
        let length = field.text.len();
        let end = if end < 0 {
            length
        } else {
            (end as usize).min(length)
        };
        let start = (start.max(0) as usize).min(end);
        let count = (end - start).min(plain_wanted as usize);
        // SAFETY: TSF passes buffers of the sizes it says.
        unsafe {
            if plain_wanted > 0 && !plain.is_null() {
                std::ptr::copy_nonoverlapping(field.text[start..].as_ptr(), plain.0, count);
            }
            *plain_returned = count as u32;
            if runs_wanted > 0 && !runs.is_null() {
                *runs = TS_RUNINFO {
                    uCount: count as u32,
                    r#type: TS_RT_PLAIN,
                };
                *runs_returned = 1;
            } else {
                *runs_returned = 0;
            }
            *next = (start + count) as i32;
        }
        Ok(())
    }

    fn SetText(
        &self,
        _flags: u32,
        start: i32,
        end: i32,
        text: &PCWSTR,
        cch: u32,
    ) -> Result<TS_TEXTCHANGE> {
        // SAFETY: TSF passes `cch` UTF-16 units.
        let wide = unsafe { std::slice::from_raw_parts(text.0, cch as usize) };
        let mut field = self.field.borrow_mut();
        field.trace.push(format!(
            "SetText {start}..{end} {:?}",
            String::from_utf16_lossy(wide)
        ));
        field
            .text
            .splice(start as usize..end as usize, wide.iter().copied());
        let new_end = start + cch as i32;
        let (selection_start, selection_end) = field.selection;
        // Keep the cursor where it was relative to the text after the change.
        let shift = |position: i32| {
            if position >= end {
                position + cch as i32 - (end - start)
            } else if position > start {
                new_end
            } else {
                position
            }
        };
        field.selection = (shift(selection_start), shift(selection_end));
        Ok(TS_TEXTCHANGE {
            acpStart: start,
            acpOldEnd: end,
            acpNewEnd: new_end,
        })
    }

    fn GetFormattedText(&self, _start: i32, _end: i32) -> Result<IDataObject> {
        Err(E_NOTIMPL.into())
    }

    fn GetEmbedded(
        &self,
        _pos: i32,
        _service: *const GUID,
        _riid: *const GUID,
    ) -> Result<IUnknown> {
        Err(E_NOTIMPL.into())
    }

    fn QueryInsertEmbedded(
        &self,
        _service: *const GUID,
        _format: *const FORMATETC,
    ) -> Result<BOOL> {
        Ok(false.into())
    }

    fn InsertEmbedded(
        &self,
        _flags: u32,
        _start: i32,
        _end: i32,
        _object: Ref<IDataObject>,
    ) -> Result<TS_TEXTCHANGE> {
        Err(E_NOTIMPL.into())
    }

    fn InsertTextAtSelection(
        &self,
        flags: u32,
        text: &PCWSTR,
        cch: u32,
        start: *mut i32,
        end: *mut i32,
        change: *mut TS_TEXTCHANGE,
    ) -> Result<()> {
        let (selection_start, selection_end) = self.field.borrow().selection;
        self.note(|| format!("InsertTextAtSelection flags={flags:#x} cch={cch}"));
        if flags & windows::Win32::UI::TextServices::TS_IAS_QUERYONLY != 0 {
            // SAFETY: TSF passes valid pointers.
            unsafe {
                *start = selection_start;
                *end = selection_end;
            }
            return Ok(());
        }
        // SAFETY: TSF passes `cch` UTF-16 units.
        let wide = unsafe { std::slice::from_raw_parts(text.0, cch as usize) };
        let mut field = self.field.borrow_mut();
        field.text.splice(
            selection_start as usize..selection_end as usize,
            wide.iter().copied(),
        );
        let new_end = selection_start + cch as i32;
        field.selection = (new_end, new_end);
        // SAFETY: TSF passes valid pointers (`change` may be null).
        unsafe {
            *start = selection_start;
            *end = new_end;
            if !change.is_null() {
                *change = TS_TEXTCHANGE {
                    acpStart: selection_start,
                    acpOldEnd: selection_end,
                    acpNewEnd: new_end,
                };
            }
        }
        Ok(())
    }

    fn InsertEmbeddedAtSelection(
        &self,
        _flags: u32,
        _object: Ref<IDataObject>,
        _start: *mut i32,
        _end: *mut i32,
        _change: *mut TS_TEXTCHANGE,
    ) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn RequestSupportedAttrs(&self, _flags: u32, _count: u32, _filter: *const GUID) -> Result<()> {
        Ok(())
    }

    fn RequestAttrsAtPosition(
        &self,
        _pos: i32,
        _count: u32,
        _filter: *const GUID,
        _flags: u32,
    ) -> Result<()> {
        Ok(())
    }

    fn RequestAttrsTransitioningAtPosition(
        &self,
        _pos: i32,
        _count: u32,
        _filter: *const GUID,
        _flags: u32,
    ) -> Result<()> {
        Ok(())
    }

    fn FindNextAttrTransition(
        &self,
        _start: i32,
        _halt: i32,
        _count: u32,
        _filter: *const GUID,
        _flags: u32,
        _next: *mut i32,
        _found: *mut BOOL,
        _offset: *mut i32,
    ) -> Result<()> {
        // SAFETY: TSF passes valid pointers.
        unsafe { *_found = false.into() };
        Ok(())
    }

    fn RetrieveRequestedAttrs(
        &self,
        _count: u32,
        _values: *mut TS_ATTRVAL,
        fetched: *mut u32,
    ) -> Result<()> {
        // SAFETY: TSF passes a valid pointer.
        unsafe { *fetched = 0 };
        Ok(())
    }

    fn GetEndACP(&self) -> Result<i32> {
        Ok(self.field.borrow().text.len() as i32)
    }

    fn GetActiveView(&self) -> Result<u32> {
        Ok(0)
    }

    fn GetACPFromPoint(&self, _view: u32, _point: *const POINT, _flags: u32) -> Result<i32> {
        Err(E_NOTIMPL.into())
    }

    fn GetTextExt(
        &self,
        _view: u32,
        _start: i32,
        _end: i32,
        rect: *mut RECT,
        clipped: *mut BOOL,
    ) -> Result<()> {
        // The text cursor is a thin rectangle at a place of this test's choosing.
        // SAFETY: TSF passes valid pointers.
        unsafe {
            *rect = CURSOR;
            *clipped = false.into();
        }
        Ok(())
    }

    fn GetScreenExt(&self, _view: u32) -> Result<RECT> {
        Err(E_FAIL.into())
    }

    fn GetWnd(&self, _view: u32) -> Result<HWND> {
        Ok(HWND::default())
    }
}

/// A profile that exists only for this test, under US English: a CI runner has
/// no Korean language set up, and TSF refuses to activate a profile of a
/// language the machine does not have. The service behind it is easime's, and
/// with a profile that is none of easime's it types Korean (the first language
/// it can type).
const TEST_PROFILE: &str = "{3C7E2D91-5B4A-4F08-A6D3-9E1B7C5A2F64}";
const ENGLISH_US: u16 = 0x0409;

/// Removes the test profile again, whatever happens.
struct TestProfile(ITfInputProcessorProfileMgr);

impl Drop for TestProfile {
    fn drop(&mut self) {
        let clsid = registration::guid(profiles::CLSID);
        // SAFETY: GUID pointers live for the call; an error means "not there".
        let _ = unsafe {
            self.0
                .UnregisterProfile(&clsid, ENGLISH_US, &registration::guid(TEST_PROFILE), 0)
        };
    }
}

/// The application, as far as the keyboard goes: it asks TSF about every key
/// and gives the key to itself when TSF does not take it.
struct Application {
    keystrokes: ITfKeystrokeMgr,
    field: Rc<RefCell<Field>>,
}

impl Application {
    /// One key press and release. `scan` is the physical key (US layout).
    /// `typed` is what the application itself would insert for the key.
    /// Returns whether TSF took the key.
    fn press(&self, vk: u16, scan: u8, typed: Option<&str>) -> bool {
        let down = LPARAM(((scan as isize) << 16) | 1);
        let up = LPARAM(((scan as isize) << 16) | 1 | (1 << 30) | (1 << 31));
        let wparam = WPARAM(usize::from(vk));
        // SAFETY: the keystroke manager is alive; the values are plain numbers.
        let wanted = unsafe { self.keystrokes.TestKeyDown(wparam, down) }
            .expect("TestKeyDown")
            .as_bool();
        let taken = wanted
            && unsafe { self.keystrokes.KeyDown(wparam, down) }
                .expect("KeyDown")
                .as_bool();
        if !taken {
            match typed {
                Some("\u{8}") => self.field.borrow_mut().backspace(),
                Some(text) => self.field.borrow_mut().insert(text),
                None => {}
            }
        }
        // SAFETY: as above.
        let wants_release = unsafe { self.keystrokes.TestKeyUp(wparam, up) }
            .expect("TestKeyUp")
            .as_bool();
        if wants_release {
            let _ = unsafe { self.keystrokes.KeyUp(wparam, up) }.expect("KeyUp");
        }
        taken
    }

    fn letters(&self, text: &str) {
        for c in text.chars() {
            let (scan, vk) = letter(c);
            self.press(vk, scan, Some(&c.to_string()));
        }
    }

    fn text(&self) -> String {
        self.field.borrow().string()
    }
}

/// The scan code and virtual key of a lower-case letter on a US keyboard.
fn letter(c: char) -> (u8, u16) {
    let (row, first) = ["qwertyuiop", "asdfghjkl", "zxcvbnm"]
        .into_iter()
        .zip([0x10u8, 0x1E, 0x2C])
        .find(|(row, _)| row.contains(c))
        .expect("a lower-case letter");
    let scan = first + row.find(c).unwrap() as u8;
    (scan, u16::from(c.to_ascii_uppercase() as u8))
}

/// Where the fake document says its text cursor is on the screen.
const CURSOR: RECT = RECT {
    left: 120,
    top: 200,
    right: 122,
    bottom: 220,
};

/// The mode box, if it is on the screen: its rectangle.
fn mode_box() -> Option<RECT> {
    // SAFETY: looking a window of our own class up by name.
    unsafe {
        let window = FindWindowW(w!("EasimeIndicator"), None).ok()?;
        if !IsWindowVisible(window).as_bool() {
            return None;
        }
        let mut area = RECT::default();
        GetWindowRect(window, &mut area).ok()?;
        Some(area)
    }
}

const VK_BACK: u16 = 0x08;
const VK_SPACE: u16 = 0x20;
const VK_HANGUL: u16 = 0x15;

/// Let the thread's windows handle their messages for a while: TSF learns about
/// focus through them.
fn pump(milliseconds: u64) {
    let end = std::time::Instant::now() + std::time::Duration::from_millis(milliseconds);
    while std::time::Instant::now() < end {
        let mut message = MSG::default();
        // SAFETY: plain message loop of this thread.
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// A window of this thread that has the focus, as an application's would, so
/// that TSF counts the thread as focused and sends keys to its text services.
fn focused_window() -> HWND {
    // SAFETY: "STATIC" is a window class every process has; the rest are plain
    // numbers and strings.
    unsafe {
        let instance = GetModuleHandleW(None).unwrap();
        let window = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("easime typing test"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            0,
            0,
            300,
            120,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .expect("a window");
        let _ = SetForegroundWindow(window);
        let _ = SetFocus(Some(window));
        window
    }
}

/// How many compositions the context has going on.
fn compositions(context: &ITfContext) -> usize {
    let composer: ITfContextComposition = context.cast().unwrap();
    // SAFETY: the context is alive.
    let list: IEnumITfCompositionView = unsafe { composer.EnumCompositions() }.unwrap();
    let mut count = 0;
    loop {
        let mut view = [None];
        let mut fetched = 0;
        // SAFETY: `view` has room for one item.
        let _ = unsafe { list.Next(&mut view, &mut fetched) };
        if fetched == 0 {
            return count;
        }
        count += 1;
    }
}

#[test]
fn hangul_is_typed_through_tsf_into_a_document() {
    let Some(dll) = dll() else {
        assert!(
            std::env::var_os("EASIME_REQUIRE_DLL").is_none(),
            "the DLL was not built: run `cargo build -p frontend-tsf` first"
        );
        eprintln!("SKIPPED: frontend_tsf.dll is not built (cargo build -p frontend-tsf)");
        return;
    };
    // The DLL logs every key, and where.
    // SAFETY: nothing else reads the environment at the same time in this test.
    unsafe { std::env::set_var("EASIME_LOG_KEYS", "1") };
    registration::unregister().unwrap();
    assert!(regsvr32(&dll, false), "regsvr32 could not register the DLL");
    let result = std::panic::catch_unwind(type_through_tsf);
    regsvr32(&dll, true);
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

fn type_through_tsf() {
    let clsid = registration::guid(profiles::CLSID);
    // SAFETY: plain COM initialization of this test's thread.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .unwrap();
    // SAFETY: Windows' own in-process COM classes.
    let thread_manager: ITfThreadMgr =
        unsafe { CoCreateInstance(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER) }.unwrap();
    let client = unsafe { thread_manager.Activate() }.expect("thread manager activation");

    // The application's document.
    let field = Rc::new(RefCell::new(Field::default()));
    let store: ITextStoreACP = TextStore {
        field: field.clone(),
        sink: RefCell::new(None),
        locked: Cell::new(false),
    }
    .into();
    let document = unsafe { thread_manager.CreateDocumentMgr() }.unwrap();
    let mut context = None;
    let mut edit_cookie = 0;
    let store_unknown: IUnknown = store.cast().unwrap();
    // SAFETY: the out pointers live for the call.
    unsafe { document.CreateContext(client, 0, &store_unknown, &mut context, &mut edit_cookie) }
        .expect("a context over the text store");
    let context: ITfContext = context.expect("the context");
    unsafe { document.Push(&context) }.expect("pushing the context");
    // The way an application does it: a window with the focus, tied to the document.
    let window = focused_window();
    pump(200);
    let _ = unsafe { thread_manager.AssociateFocus(window, &document) };
    unsafe { thread_manager.SetFocus(&document) }.expect("focusing the document");
    pump(200);

    // easime's text service, activated by TSF for this process only.
    let manager: ITfInputProcessorProfileMgr =
        unsafe { CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER) }
            .unwrap();
    let test_profile = registration::guid(TEST_PROFILE);
    let name: Vec<u16> = "easime typing test".encode_utf16().collect();
    // SAFETY: the slices and GUID pointers live for the call.
    unsafe {
        manager.RegisterProfile(
            &clsid,
            ENGLISH_US,
            &test_profile,
            &name,
            &[],
            0,
            HKL::default(),
            0,
            true,
            0,
        )
    }
    .expect("registering the test profile");
    let _cleanup = TestProfile(manager.clone());
    unsafe {
        manager.ActivateProfile(
            TF_PROFILETYPE_INPUTPROCESSOR,
            ENGLISH_US,
            &clsid,
            &test_profile,
            HKL::default(),
            TF_IPPMF_FORPROCESS | TF_IPPMF_ENABLEPROFILE | TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE,
        )
    }
    .unwrap_or_else(|e| panic!("TSF could not activate easime's service: {e}"));
    let keystrokes: ITfKeystrokeMgr = thread_manager.cast().unwrap();
    pump(100);
    // What TSF says about the setup: to read when no key arrives.
    eprintln!("thread has focus: {:?}", unsafe {
        thread_manager.IsThreadFocus()
    });
    eprintln!("foreground key sink: {:?}", unsafe {
        keystrokes.GetForeground()
    });
    eprintln!(
        "focused document is ours: {:?}",
        unsafe { thread_manager.GetFocus() }.map(|focus| focus == document)
    );
    let compartments: ITfCompartmentMgr = thread_manager.cast().unwrap();
    if let Ok(open) = unsafe { compartments.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) } {
        eprintln!(
            "keyboard open/close: {:?}",
            unsafe { open.GetValue() }.map(|v| i32::try_from(&v).ok())
        );
    }
    let application = Application {
        keystrokes,
        field: field.clone(),
    };

    // Run the scenario; whatever happens, show what TSF asked of the document.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scenario(&application, &context, client);
    }));
    eprintln!("---- what TSF did to the text store:");
    for call in &field.borrow().trace {
        eprintln!("  {call}");
    }
    eprintln!("---- final text: {:?}", application.text());
    // SAFETY: the thread manager and the window are alive.
    unsafe { thread_manager.Deactivate() }.expect("thread manager deactivation");
    let _ = unsafe { DestroyWindow(window) };
    if let Some(log) = easime_config::log_path()
        && let Ok(text) = std::fs::read_to_string(log)
    {
        eprintln!("---- easime.log (last lines):");
        for line in text
            .lines()
            .rev()
            .take(60)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            eprintln!("  {line}");
        }
    }
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}

fn scenario(app: &Application, context: &ITfContext, client: u32) {
    // Hangul: gksrmf is 한글, and only the last syllable is still being composed.
    app.letters("g");
    assert_eq!(app.text(), "ㅎ", "the first jamo shows at once");
    assert_eq!(compositions(context), 1, "and starts a composition");
    app.letters("ksrmf");
    assert_eq!(app.text(), "한글");
    assert_eq!(compositions(context), 1);

    // Space finishes the syllable and then belongs to the application.
    let taken = app.press(VK_SPACE, 0x39, Some(" "));
    assert!(!taken, "the application gets the space itself");
    assert_eq!(app.text(), "한글 ");
    assert_eq!(
        compositions(context),
        0,
        "nothing is being composed any more"
    );

    // Backspace takes back jamo of the syllable, then the application's own.
    app.letters("gks");
    assert_eq!(app.text(), "한글 한");
    assert!(app.press(VK_BACK, 0x0E, Some("\u{8}")));
    assert_eq!(app.text(), "한글 하");
    assert!(app.press(VK_BACK, 0x0E, Some("\u{8}")));
    assert_eq!(app.text(), "한글 ㅎ");
    assert!(app.press(VK_BACK, 0x0E, Some("\u{8}")));
    assert_eq!(app.text(), "한글 ");
    assert_eq!(compositions(context), 0);
    assert!(
        !app.press(VK_BACK, 0x0E, Some("\u{8}")),
        "nothing to compose: the application deletes the space"
    );
    assert_eq!(app.text(), "한글");

    assert!(mode_box().is_none(), "no box before the user switches");

    // The Hangul key switches to English: the keys go to the application as typed.
    assert!(app.press(VK_HANGUL, 0x72, None), "the Hangul key is taken");
    // The box says so, just under the text cursor, and goes away by itself.
    let shown = mode_box().expect("the mode box is on the screen after a switch");
    assert_eq!(shown.left, CURSOR.left, "starts at the cursor");
    assert_eq!(shown.top, CURSOR.bottom + 2, "just below the cursor");
    assert!(shown.right - shown.left >= 48 && shown.right - shown.left == shown.bottom - shown.top);
    pump(1500);
    assert!(mode_box().is_none(), "and goes away after its time");
    app.letters("gk");
    assert_eq!(app.text(), "한글gk");
    assert!(app.press(VK_HANGUL, 0x72, None));
    app.letters("gk");
    assert_eq!(app.text(), "한글gk하");

    // The Hangul key in the middle of a syllable finishes it first.
    assert!(app.press(VK_HANGUL, 0x72, None));
    assert_eq!(app.text(), "한글gk하");
    assert_eq!(compositions(context), 0);

    // A password field: the application disables the keyboard for text
    // services, and easime leaves every key to it, even in Korean mode.
    assert!(app.press(VK_HANGUL, 0x72, None), "back to Korean");
    set_compartment(context, client, &GUID_COMPARTMENT_KEYBOARD_DISABLED, 1);
    app.letters("gk");
    assert_eq!(app.text(), "한글gk하gk", "typed as the keys are");
    assert_eq!(compositions(context), 0);
    assert!(
        !app.press(VK_HANGUL, 0x72, None),
        "not even the Hangul key is taken there"
    );
    set_compartment(context, client, &GUID_COMPARTMENT_KEYBOARD_DISABLED, 0);
    app.letters("gk");
    assert_eq!(app.text(), "한글gk하gk하", "Korean again outside it");

    // A place that takes no text (a web page itself): letters are shortcuts.
    assert!(
        !app.press(VK_SPACE, 0x39, Some(" ")),
        "the syllable is finished and the space is the application's"
    );
    set_compartment(context, client, &GUID_COMPARTMENT_EMPTYCONTEXT, 1);
    app.letters("k");
    assert_eq!(app.text(), "한글gk하gk하 k");
    set_compartment(context, client, &GUID_COMPARTMENT_EMPTYCONTEXT, 0);
}

/// What an application sets on its own context, as a number.
fn set_compartment(context: &ITfContext, client: u32, which: &GUID, value: i32) {
    let compartments: ITfCompartmentMgr = context.cast().unwrap();
    // SAFETY: the manager is alive; the GUID and the value live for the calls.
    unsafe {
        let compartment = compartments.GetCompartment(which).unwrap();
        compartment
            .SetValue(client, &VARIANT::from(value))
            .expect("setting a compartment of our own context");
    }
}

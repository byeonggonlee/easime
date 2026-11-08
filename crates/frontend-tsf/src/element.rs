//! Candidates for applications that draw them themselves: TSF's *UI element*
//! for a candidate list (`ITfCandidateListUIElement`).
//!
//! Before the service shows its candidate window ([`crate::candidate`]), it
//! tells TSF that a candidate list begins (`BeginUIElement`). An application
//! that draws candidates itself (a full-screen game, where a window of ours
//! would not be seen) answers "do not show it" and reads the list from the
//! element; each change is announced (`UpdateUIElement`), and so is the end
//! (`EndUIElement`). Every other application gets easime's window. The
//! application may also ask for the window later (`Show`).
//!
//! The pages are the ones the number keys pick from (nine, on every OS), so an
//! application cannot set its own (`SetPageIndex`). Choosing with the mouse in
//! such an application (`ITfCandidateListUIElementBehavior`) is not offered
//! yet: the keys choose.
//!
//! Everything runs on the service's thread. No cell is borrowed while TSF is
//! called, because the application reads the element during those calls.

use crate::candidate::CandidateWindow;
use crate::diag;
use crate::page::{self, Changes, Page};
use crate::placement::Rect;
use crate::service::Alive;
use ime_core::{CandidateList, Language};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG, E_NOTIMPL};
use windows::Win32::UI::TextServices::{
    ITfCandidateListUIElement, ITfCandidateListUIElement_Impl, ITfDocumentMgr, ITfUIElement_Impl,
    ITfUIElementMgr, TF_CLUIE_COUNT, TF_CLUIE_CURRENTPAGE, TF_CLUIE_DOCUMENTMGR,
    TF_CLUIE_PAGEINDEX, TF_CLUIE_SELECTION, TF_CLUIE_STRING,
};
use windows::core::{BOOL, BSTR, Error, GUID, Result, implement};

/// Names easime's candidate list among an application's UI elements.
pub const ELEMENT_GUID: GUID = GUID::from_u128(0xB7D99758_921F_4B1F_B963_ACB42AD0FD9A);

/// What the element tells the application, and the window easime may show.
#[derive(Default)]
struct Shared {
    list: Option<CandidateList>,
    /// The document of the conversion.
    document: Option<ITfDocumentMgr>,
    /// What the last announcement changed (`TF_CLUIE_*`).
    updated: u32,
    /// Whether easime shows the candidates (or the application draws them).
    ours: bool,
    /// What the window shows, to show it again if the application asks.
    last: Option<(Page, Language, Rect)>,
    window: CandidateWindow,
}

/// The candidates of one text service: its window, and the UI element that
/// lets an application draw them instead.
pub struct Candidates {
    shared: Rc<RefCell<Shared>>,
    element: ITfCandidateListUIElement,
    /// TSF's number for the list while it is up.
    id: Cell<Option<u32>>,
}

impl Default for Candidates {
    fn default() -> Self {
        let shared = Rc::new(RefCell::new(Shared::default()));
        let element = Element {
            shared: shared.clone(),
            _alive: Alive::new(),
        }
        .into();
        Self {
            shared,
            element,
            id: Cell::new(None),
        }
    }
}

impl Candidates {
    /// Show `list`, whose page on the screen is `page` (candidates of
    /// `language`). `manager` is TSF's, to ask whether the application draws
    /// them; `document` is where the conversion is. `caret` is where the text
    /// cursor is on the screen. Returns whether the candidates are shown, by
    /// easime or by the application.
    pub fn show(
        &self,
        manager: Option<&ITfUIElementMgr>,
        document: impl FnOnce() -> Option<ITfDocumentMgr>,
        list: &CandidateList,
        page: Page,
        language: Language,
        caret: impl FnOnce() -> Option<Rect>,
    ) -> bool {
        let changes = {
            let mut shared = self.shared.borrow_mut();
            let changes = match (&shared.list, self.id.get()) {
                (Some(before), Some(_)) => page::changes(before, list),
                _ => Changes::ALL,
            };
            shared.list = Some(list.clone());
            shared.updated = flags(changes);
            changes
        };
        match (manager, self.id.get()) {
            (Some(manager), Some(id)) => {
                if changes.any() {
                    // SAFETY: the manager is alive for the call.
                    if let Err(e) = unsafe { manager.UpdateUIElement(id) } {
                        diag::log_key(|| format!("candidate list update: {e}"));
                    }
                }
            }
            (Some(manager), None) => {
                let document = document();
                {
                    let mut shared = self.shared.borrow_mut();
                    shared.document = document;
                    shared.updated |= TF_CLUIE_DOCUMENTMGR;
                }
                let mut show = BOOL::from(true);
                let mut id = 0;
                // SAFETY: the manager and the element are alive; the out
                // values live for the call.
                let begun = unsafe { manager.BeginUIElement(&self.element, &mut show, &mut id) };
                let ours = match begun {
                    Ok(()) => {
                        self.id.set(Some(id));
                        show.as_bool()
                    }
                    Err(e) => {
                        diag::log_key(|| format!("candidate list not announced: {e}"));
                        true
                    }
                };
                if !ours {
                    diag::log("this program draws the candidates itself");
                }
                self.shared.borrow_mut().ours = ours;
            }
            (None, _) => self.shared.borrow_mut().ours = true,
        }
        // Asked with no cell borrowed: it reads the application's document.
        let caret = caret();
        let mut shared = self.shared.borrow_mut();
        if !shared.ours {
            shared.window.hide();
            shared.last = caret.map(|caret| (page, language, caret));
            return true;
        }
        let Some(caret) = caret else {
            return false;
        };
        let shown = shared.window.show(&page, language, caret);
        if shown.is_some() {
            diag::log_key(|| format!("candidates at {shown:?}"));
        }
        shared.last = shown.map(|_| (page, language, caret));
        shown.is_some()
    }

    /// The candidates are over: the window goes away, and the application
    /// is told.
    pub fn end(&self, manager: Option<&ITfUIElementMgr>) {
        if let Ok(mut shared) = self.shared.try_borrow_mut() {
            shared.window.hide();
            shared.list = None;
            shared.last = None;
            shared.document = None;
        }
        if let (Some(manager), Some(id)) = (manager, self.id.take()) {
            // SAFETY: the manager is alive for the call.
            if let Err(e) = unsafe { manager.EndUIElement(id) } {
                diag::log_key(|| format!("candidate list end: {e}"));
            }
        }
    }

    /// The service stops: as [`Candidates::end`], and the window is destroyed.
    pub fn destroy(&self, manager: Option<&ITfUIElementMgr>) {
        self.end(manager);
        if let Ok(mut shared) = self.shared.try_borrow_mut() {
            shared.window.destroy();
        }
    }
}

/// TSF's flags for `changes`.
fn flags(changes: Changes) -> u32 {
    [
        (changes.count, TF_CLUIE_COUNT),
        (changes.strings, TF_CLUIE_STRING),
        (changes.selection, TF_CLUIE_SELECTION),
        (changes.page, TF_CLUIE_CURRENTPAGE),
        (changes.pages, TF_CLUIE_PAGEINDEX),
    ]
    .into_iter()
    .filter(|(changed, _)| *changed)
    .fold(0, |all, (_, flag)| all | flag)
}

/// The candidate list as TSF hands it to the application.
#[implement(ITfCandidateListUIElement)]
struct Element {
    shared: Rc<RefCell<Shared>>,
    _alive: Alive,
}

impl Element {
    /// What is shared, or a failure if the service is using it right now.
    fn with<T>(&self, f: impl FnOnce(&Shared) -> Result<T>) -> Result<T> {
        let shared = self.shared.try_borrow().map_err(|_| Error::from(E_FAIL))?;
        f(&shared)
    }
}

impl ITfUIElement_Impl for Element_Impl {
    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from("easime candidates"))
    }

    fn GetGUID(&self) -> Result<GUID> {
        Ok(ELEMENT_GUID)
    }

    fn Show(&self, show: BOOL) -> Result<()> {
        let mut shared = self
            .shared
            .try_borrow_mut()
            .map_err(|_| Error::from(E_FAIL))?;
        shared.ours = show.as_bool();
        match shared.last.clone() {
            Some((page, language, caret)) if shared.ours => {
                shared.window.show(&page, language, caret);
            }
            _ => shared.window.hide(),
        }
        Ok(())
    }

    fn IsShown(&self) -> Result<BOOL> {
        self.with(|shared| Ok((shared.ours && shared.list.is_some()).into()))
    }
}

impl ITfCandidateListUIElement_Impl for Element_Impl {
    fn GetUpdatedFlags(&self) -> Result<u32> {
        self.with(|shared| Ok(shared.updated))
    }

    fn GetDocumentMgr(&self) -> Result<ITfDocumentMgr> {
        self.with(|shared| shared.document.clone().ok_or_else(|| E_FAIL.into()))
    }

    fn GetCount(&self) -> Result<u32> {
        self.with(|shared| Ok(shared.list.as_ref().map_or(0, |l| l.items.len() as u32)))
    }

    fn GetSelection(&self) -> Result<u32> {
        self.with(|shared| Ok(shared.list.as_ref().map_or(0, |l| l.selected as u32)))
    }

    fn GetString(&self, index: u32) -> Result<BSTR> {
        self.with(|shared| {
            shared
                .list
                .as_ref()
                .and_then(|list| list.items.get(index as usize))
                .map(|text| BSTR::from(text.as_str()))
                .ok_or_else(|| E_INVALIDARG.into())
        })
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn GetPageIndex(&self, index: *mut u32, size: u32, count: *mut u32) -> Result<()> {
        if count.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let starts = self.with(|shared| Ok(shared.list.as_ref().map(page::page_starts)))?;
        let starts = starts.unwrap_or_default();
        // SAFETY: the caller gives room for the count, and for `size` numbers
        // at `index` when it is not null.
        unsafe {
            *count = starts.len() as u32;
            if !index.is_null() {
                for (i, start) in starts.iter().take(size as usize).enumerate() {
                    *index.add(i) = *start;
                }
            }
        }
        Ok(())
    }

    fn SetPageIndex(&self, _index: *const u32, _count: u32) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn GetCurrentPage(&self) -> Result<u32> {
        self.with(|shared| Ok(shared.list.as_ref().map_or(0, page::current_page)))
    }
}

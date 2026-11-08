//! The composition's underlines ("display attributes"), the Windows side of
//! [`crate::marks`]. The text service is a display attribute provider: TSF
//! asks it what each of its attributes looks like ([`AttributeInfo`]), the
//! service marks the parts of its composition with them ([`mark`]), and the
//! application draws the lines in its own colours.

use crate::marks::Mark;
use crate::registration::guid;
use crate::service::Alive;
use std::cell::Cell;
use std::ops::Range;
use windows::Win32::Foundation::{E_INVALIDARG, E_NOTIMPL, E_POINTER, S_FALSE};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, GUID_PROP_ATTRIBUTE, IEnumTfDisplayAttributeInfo,
    IEnumTfDisplayAttributeInfo_Impl, ITfCategoryMgr, ITfComposition, ITfContext,
    ITfDisplayAttributeInfo, ITfDisplayAttributeInfo_Impl, TF_ANCHOR_START, TF_ATTR_INPUT,
    TF_ATTR_TARGET_CONVERTED, TF_DA_COLOR, TF_DISPLAYATTRIBUTE, TF_LS_DOT, TF_LS_SOLID,
};
use windows::core::{BSTR, Error, GUID, Result, implement};

/// What a mark looks like. Only the line: the colours stay the application's.
pub fn look(mark: Mark) -> TF_DISPLAYATTRIBUTE {
    TF_DISPLAYATTRIBUTE {
        crText: TF_DA_COLOR::default(),
        crBk: TF_DA_COLOR::default(),
        lsStyle: if mark.dotted() {
            TF_LS_DOT
        } else {
            TF_LS_SOLID
        },
        fBoldLine: mark.bold().into(),
        crLine: TF_DA_COLOR::default(),
        bAttr: if mark.converting() {
            TF_ATTR_TARGET_CONVERTED
        } else {
            TF_ATTR_INPUT
        },
    }
}

/// The attribute with this id, for `GetDisplayAttributeInfo`.
pub fn info(id: &GUID) -> Result<ITfDisplayAttributeInfo> {
    Mark::ALL
        .into_iter()
        .find(|mark| guid(mark.guid()) == *id)
        .map(|mark| AttributeInfo::new(mark).into())
        .ok_or_else(|| E_INVALIDARG.into())
}

/// One attribute, as TSF and applications read it.
#[implement(ITfDisplayAttributeInfo)]
pub struct AttributeInfo {
    mark: Mark,
    _alive: Alive,
}

impl AttributeInfo {
    fn new(mark: Mark) -> Self {
        Self {
            mark,
            _alive: Alive::new(),
        }
    }
}

impl ITfDisplayAttributeInfo_Impl for AttributeInfo_Impl {
    fn GetGUID(&self) -> Result<GUID> {
        Ok(guid(self.mark.guid()))
    }

    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from(self.mark.description()))
    }

    // The signature, raw pointer included, is COM's: the caller owns it.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn GetAttributeInfo(&self, attribute: *mut TF_DISPLAYATTRIBUTE) -> Result<()> {
        // SAFETY: TSF passes an attribute to fill in, or null.
        let attribute = unsafe { attribute.as_mut() }.ok_or(Error::from(E_POINTER))?;
        *attribute = look(self.mark);
        Ok(())
    }

    /// The look is fixed (no setting changes it yet).
    fn SetAttributeInfo(&self, _attribute: *const TF_DISPLAYATTRIBUTE) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn Reset(&self) -> Result<()> {
        Ok(())
    }
}

/// Every attribute, in turn, for `EnumDisplayAttributeInfo`.
#[implement(IEnumTfDisplayAttributeInfo)]
pub struct AttributeList {
    next: Cell<usize>,
    _alive: Alive,
}

impl AttributeList {
    pub fn new() -> Self {
        Self::from(0)
    }

    fn from(next: usize) -> Self {
        Self {
            next: Cell::new(next),
            _alive: Alive::new(),
        }
    }
}

impl Default for AttributeList {
    fn default() -> Self {
        Self::new()
    }
}

impl IEnumTfDisplayAttributeInfo_Impl for AttributeList_Impl {
    fn Clone(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(AttributeList::from(self.next.get()).into())
    }

    // The signature, raw pointers included, is COM's: the caller owns them.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn Next(
        &self,
        count: u32,
        out: *mut Option<ITfDisplayAttributeInfo>,
        fetched: *mut u32,
    ) -> Result<()> {
        if out.is_null() {
            return Err(E_POINTER.into());
        }
        let mut given = 0usize;
        while given < count as usize
            && let Some(&mark) = Mark::ALL.get(self.next.get())
        {
            let attribute: ITfDisplayAttributeInfo = AttributeInfo::new(mark).into();
            // SAFETY: TSF passes room for `count` interfaces.
            unsafe { out.add(given).write(Some(attribute)) };
            given += 1;
            self.next.set(self.next.get() + 1);
        }
        if !fetched.is_null() {
            // SAFETY: checked for null; TSF passes a place for the count.
            unsafe { fetched.write(given as u32) };
        }
        // Fewer than asked for: S_FALSE, as every enumerator says it.
        if given < count as usize {
            Err(S_FALSE.into())
        } else {
            Ok(())
        }
    }

    fn Reset(&self) -> Result<()> {
        self.next.set(0);
        Ok(())
    }

    fn Skip(&self, count: u32) -> Result<()> {
        let next = self.next.get() + count as usize;
        self.next.set(next.min(Mark::ALL.len()));
        if next > Mark::ALL.len() {
            Err(S_FALSE.into())
        } else {
            Ok(())
        }
    }
}

/// TSF's short numbers ("atoms") for the attributes, as the property values.
pub fn atoms() -> Result<[u32; 3]> {
    // SAFETY: an in-process COM class of Windows itself.
    let categories: ITfCategoryMgr =
        unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER) }?;
    let mut atoms = [0; 3];
    for (atom, mark) in atoms.iter_mut().zip(Mark::ALL) {
        // SAFETY: the GUID lives for the call.
        *atom = unsafe { categories.RegisterGUID(&guid(mark.guid())) }?;
    }
    Ok(atoms)
}

/// The atom of `mark` among `atoms` (in the order of [`Mark::ALL`]).
pub fn atom_of(atoms: &[u32; 3], mark: Mark) -> u32 {
    let index = Mark::ALL.iter().position(|&m| m == mark).unwrap_or(0);
    atoms[index]
}

/// Mark the parts of the composition (`parts`: UTF-16 ranges with the atom of
/// their attribute), replacing what was marked before.
pub fn mark(
    context: &ITfContext,
    composition: &ITfComposition,
    cookie: u32,
    parts: &[(Range<usize>, u32)],
) -> Result<()> {
    // SAFETY: the context and composition are alive, inside an edit session.
    let property = unsafe { context.GetProperty(&GUID_PROP_ATTRIBUTE) }?;
    let range = unsafe { composition.GetRange() }?;
    unsafe { property.Clear(cookie, &range) }?;
    for (part, atom) in parts {
        // SAFETY: as above; the new range is ours.
        let span = unsafe { range.Clone() }?;
        let mut moved = 0;
        unsafe {
            span.Collapse(cookie, TF_ANCHOR_START)?;
            span.ShiftEnd(cookie, part.end as i32, &mut moved, std::ptr::null())?;
            span.ShiftStart(cookie, part.start as i32, &mut moved, std::ptr::null())?;
        }
        let value = VARIANT::from(*atom as i32);
        // SAFETY: the range and the value live for the call.
        unsafe { property.SetValue(cookie, &span, &value) }?;
    }
    Ok(())
}

/// Take the marks off the composition: its text is finished.
pub fn unmark(context: &ITfContext, composition: &ITfComposition, cookie: u32) -> Result<()> {
    // SAFETY: the context and composition are alive, inside an edit session.
    let property = unsafe { context.GetProperty(&GUID_PROP_ATTRIBUTE) }?;
    let range = unsafe { composition.GetRange() }?;
    unsafe { property.Clear(cookie, &range) }
}

//! Display attribute for text being composed: a solid underline, so the
//! bopomofo in the document reads as "not committed yet".

use std::cell::Cell;

use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::UI::TextServices::{
    IEnumTfDisplayAttributeInfo, IEnumTfDisplayAttributeInfo_Impl, ITfDisplayAttributeInfo,
    ITfDisplayAttributeInfo_Impl, TF_ATTR_INPUT, TF_DISPLAYATTRIBUTE, TF_LS_SOLID,
};
use windows_core::{implement, Result, BSTR, GUID};

use crate::globals::{guard, GUID_DISPLAY_ATTRIBUTE_INPUT};

#[implement(ITfDisplayAttributeInfo)]
pub struct InputAttribute;

impl ITfDisplayAttributeInfo_Impl for InputAttribute_Impl {
    fn GetGUID(&self) -> Result<GUID> {
        Ok(GUID_DISPLAY_ATTRIBUTE_INPUT)
    }

    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from("輕注音 組字中"))
    }

    fn GetAttributeInfo(&self, pda: *mut TF_DISPLAYATTRIBUTE) -> Result<()> {
        guard("GetAttributeInfo", Err(E_INVALIDARG.into()), || {
            if pda.is_null() {
                return Err(E_INVALIDARG.into());
            }
            unsafe {
                *pda = TF_DISPLAYATTRIBUTE {
                    lsStyle: TF_LS_SOLID,
                    bAttr: TF_ATTR_INPUT,
                    ..Default::default()
                };
            }
            Ok(())
        })
    }

    fn SetAttributeInfo(&self, _pda: *const TF_DISPLAYATTRIBUTE) -> Result<()> {
        Ok(())
    }

    fn Reset(&self) -> Result<()> {
        Ok(())
    }
}

/// Enumerates our single display attribute.
#[implement(IEnumTfDisplayAttributeInfo)]
pub struct Attributes {
    next: Cell<u32>,
}

impl Attributes {
    pub fn new() -> Self {
        Attributes { next: Cell::new(0) }
    }
}

impl IEnumTfDisplayAttributeInfo_Impl for Attributes_Impl {
    fn Clone(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(Attributes {
            next: Cell::new(self.next.get()),
        }
        .into())
    }

    fn Next(
        &self,
        ulcount: u32,
        rginfo: *mut Option<ITfDisplayAttributeInfo>,
        pcfetched: *mut u32,
    ) -> Result<()> {
        guard(
            "EnumDisplayAttributes::Next",
            Err(E_INVALIDARG.into()),
            || {
                if rginfo.is_null() {
                    return Err(E_INVALIDARG.into());
                }
                let mut fetched = 0;
                if ulcount > 0 && self.next.get() == 0 {
                    unsafe { *rginfo = Some(InputAttribute.into()) };
                    self.next.set(1);
                    fetched = 1;
                }
                if !pcfetched.is_null() {
                    unsafe { *pcfetched = fetched };
                }
                // Callers read the count; fewer than asked means the end.
                Ok(())
            },
        )
    }

    fn Reset(&self) -> Result<()> {
        self.next.set(0);
        Ok(())
    }

    fn Skip(&self, ulcount: u32) -> Result<()> {
        self.next.set(self.next.get().saturating_add(ulcount));
        Ok(())
    }
}

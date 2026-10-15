//! The 輕 icon on the taskbar: the language bar's input-mode button
//! (GUID_LBI_INPUTMODE), which the input indicator shows instead of the
//! profile icon. Like the system tray icons it is white on a dark taskbar
//! and black on a light one; clicking it switches Chinese/English.

use std::cell::{Cell, RefCell};

use windows::Win32::Foundation::{E_INVALIDARG, POINT, RECT};
use windows::Win32::System::Ole::{
    CONNECT_E_ADVISELIMIT, CONNECT_E_CANNOTCONNECT, CONNECT_E_NOCONNECTION,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetSystemMetricsForDpi};
use windows::Win32::UI::TextServices::{
    ITfCompartmentMgr, ITfLangBarItem, ITfLangBarItemButton, ITfLangBarItemButton_Impl,
    ITfLangBarItemSink, ITfLangBarItem_Impl, ITfMenu, ITfSource, ITfSource_Impl, ITfThreadMgr,
    TfLBIClick, GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION, GUID_LBI_INPUTMODE,
    TF_CONVERSIONMODE_NATIVE, TF_LANGBARITEMINFO, TF_LBI_ICON, TF_LBI_STYLE_BTN_BUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::{
    LoadImageW, HICON, IMAGE_ICON, LR_LOADFROMFILE, SM_CXSMICON,
};
use windows_core::{implement, w, IUnknown, Interface, Ref, Result, BOOL, BSTR, GUID};

use crate::globals::{self, guard, install_dir, wide_nul, CLSID_TEXT_SERVICE, DISPLAY_NAME};
use crate::log;

/// The only sink cookie we hand out (TSF advises one sink per item).
const COOKIE: u32 = 1;

/// Whether the taskbar is light. Unset means the classic dark taskbar.
fn light_taskbar() -> bool {
    globals::personalize_flag(w!("SystemUsesLightTheme")) == Some(1)
}

#[implement(ITfLangBarItemButton, ITfSource)]
pub struct InputModeButton {
    thread_mgr: ITfThreadMgr,
    client: u32,
    sink: RefCell<Option<ITfLangBarItemSink>>,
    /// The taskbar theme the icon was last drawn for.
    light: Cell<bool>,
}

impl InputModeButton {
    pub fn new(thread_mgr: ITfThreadMgr, client: u32) -> Self {
        globals::add_ref();
        InputModeButton {
            thread_mgr,
            client,
            sink: RefCell::new(None),
            light: Cell::new(light_taskbar()),
        }
    }

    /// Ask the taskbar for a new icon if its theme changed since.
    pub fn refresh(&self) {
        let light = light_taskbar();
        if light == self.light.replace(light) {
            return;
        }
        let sink = self.sink.borrow().clone();
        if let Some(sink) = sink {
            log::debug(|| format!("taskbar theme changed (light={light})"));
            let _ = unsafe { sink.OnUpdate(TF_LBI_ICON) };
        }
    }

    fn icon(&self) -> Result<HICON> {
        let light = light_taskbar();
        self.light.set(light);
        let name = if light {
            "lightbopomofo-input-light.ico"
        } else {
            "lightbopomofo-input.ico"
        };
        let path = install_dir().ok_or(E_INVALIDARG)?.join(name);
        let size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem()) };
        let path = wide_nul(&path.to_string_lossy());
        // The caller owns the icon and destroys it.
        let handle = unsafe {
            LoadImageW(
                None,
                windows_core::PCWSTR(path.as_ptr()),
                IMAGE_ICON,
                size,
                size,
                LR_LOADFROMFILE,
            )?
        };
        Ok(HICON(handle.0))
    }

    /// Flip Chinese/English through the input-mode compartment; the text
    /// service follows it like any other change of the indicator.
    fn toggle(&self) -> Result<()> {
        unsafe {
            let compartment = self
                .thread_mgr
                .cast::<ITfCompartmentMgr>()?
                .GetCompartment(&GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION)?;
            let mode = i32::try_from(&compartment.GetValue()?)
                .unwrap_or(TF_CONVERSIONMODE_NATIVE as i32) as u32;
            compartment.SetValue(
                self.client,
                &VARIANT::from((mode ^ TF_CONVERSIONMODE_NATIVE) as i32),
            )
        }
    }
}

impl Drop for InputModeButton {
    fn drop(&mut self) {
        globals::release();
    }
}

impl ITfLangBarItem_Impl for InputModeButton_Impl {
    fn GetInfo(&self, info: *mut TF_LANGBARITEMINFO) -> Result<()> {
        if info.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let mut description = [0u16; 32];
        for (slot, unit) in description.iter_mut().zip(DISPLAY_NAME.encode_utf16()) {
            *slot = unit;
        }
        unsafe {
            *info = TF_LANGBARITEMINFO {
                clsidService: CLSID_TEXT_SERVICE,
                guidItem: GUID_LBI_INPUTMODE,
                dwStyle: TF_LBI_STYLE_BTN_BUTTON,
                ulSort: 0,
                szDescription: description,
            };
        }
        Ok(())
    }

    fn GetStatus(&self) -> Result<u32> {
        Ok(0)
    }

    fn Show(&self, _show: BOOL) -> Result<()> {
        Ok(())
    }

    fn GetTooltipString(&self) -> Result<BSTR> {
        Ok(BSTR::from(DISPLAY_NAME))
    }
}

impl ITfLangBarItemButton_Impl for InputModeButton_Impl {
    fn OnClick(&self, _click: TfLBIClick, _pt: &POINT, _area: *const RECT) -> Result<()> {
        guard("LangBar::OnClick", Ok(()), || {
            self.toggle()
                .inspect_err(|e| log::error(&format!("toggle failed: {e}")))
        })
    }

    fn InitMenu(&self, _menu: Ref<ITfMenu>) -> Result<()> {
        Ok(())
    }

    fn OnMenuSelect(&self, _id: u32) -> Result<()> {
        Ok(())
    }

    fn GetIcon(&self) -> Result<HICON> {
        guard("LangBar::GetIcon", Err(E_INVALIDARG.into()), || {
            self.icon()
                .inspect_err(|e| log::error(&format!("icon failed: {e}")))
        })
    }

    fn GetText(&self) -> Result<BSTR> {
        Ok(BSTR::from(DISPLAY_NAME))
    }
}

impl ITfSource_Impl for InputModeButton_Impl {
    fn AdviseSink(&self, riid: *const GUID, punk: Ref<IUnknown>) -> Result<u32> {
        guard(
            "LangBar::AdviseSink",
            Err(CONNECT_E_CANNOTCONNECT.into()),
            || {
                if riid.is_null() || unsafe { *riid } != ITfLangBarItemSink::IID {
                    return Err(CONNECT_E_CANNOTCONNECT.into());
                }
                let mut sink = self.sink.borrow_mut();
                if sink.is_some() {
                    return Err(CONNECT_E_ADVISELIMIT.into());
                }
                *sink = Some(punk.ok()?.cast()?);
                Ok(COOKIE)
            },
        )
    }

    fn UnadviseSink(&self, cookie: u32) -> Result<()> {
        guard("LangBar::UnadviseSink", Ok(()), || {
            let mut sink = self.sink.borrow_mut();
            if cookie != COOKIE || sink.take().is_none() {
                return Err(CONNECT_E_NOCONNECTION.into());
            }
            Ok(())
        })
    }
}

// Interfaces implemented above, named so the compiler checks they exist.
#[allow(dead_code)]
fn _interfaces(_: ITfLangBarItem, _: ITfLangBarItemButton, _: ITfSource) {}

//! 輕注音 for Windows: a Text Services Framework (TSF) input method DLL.
//! See docs/WINDOWS_PLAN.md. Everything but key translation is Windows-only.
//!
//! Exports the standard COM server entry points; the installer registers the
//! DLL with regsvr32 (DllRegisterServer), and TSF creates the text service
//! through DllGetClassObject in every application that uses it.

pub mod keys;

#[cfg(windows)]
mod candidates;
#[cfg(windows)]
mod display;
#[cfg(windows)]
mod edit;
#[cfg(windows)]
mod globals;
#[cfg(windows)]
mod langbar;
#[cfg(windows)]
mod log;
#[cfg(windows)]
mod register;
#[cfg(windows)]
mod service;

#[cfg(windows)]
mod exports {
    use std::ffi::c_void;

    use windows::Win32::Foundation::{
        CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_POINTER, HMODULE, S_FALSE, S_OK,
    };
    use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
    use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
    use windows_core::{implement, IUnknown, Interface, Ref, Result, BOOL, GUID, HRESULT};

    use crate::globals::{self, guard, CLSID_TEXT_SERVICE};
    use crate::service::TextService;
    use crate::{log, register};

    #[implement(IClassFactory)]
    struct Factory;

    impl IClassFactory_Impl for Factory_Impl {
        fn CreateInstance(
            &self,
            outer: Ref<IUnknown>,
            iid: *const GUID,
            object: *mut *mut c_void,
        ) -> Result<()> {
            guard("CreateInstance", Err(E_FAIL.into()), || {
                if object.is_null() || iid.is_null() {
                    return Err(E_POINTER.into());
                }
                unsafe { *object = std::ptr::null_mut() };
                if !outer.is_null() {
                    return Err(CLASS_E_NOAGGREGATION.into());
                }
                let service: IUnknown = TextService::new().into();
                unsafe { service.query(iid, object).ok() }
            })
        }

        fn LockServer(&self, lock: BOOL) -> Result<()> {
            if lock.as_bool() {
                globals::add_ref();
            } else {
                globals::release();
            }
            Ok(())
        }
    }

    #[no_mangle]
    extern "system" fn DllMain(module: HMODULE, reason: u32, _reserved: *mut c_void) -> BOOL {
        if reason == DLL_PROCESS_ATTACH {
            globals::set_module(module);
        }
        BOOL(1)
    }

    /// # Safety
    /// Called by COM with valid pointers.
    #[no_mangle]
    unsafe extern "system" fn DllGetClassObject(
        clsid: *const GUID,
        iid: *const GUID,
        object: *mut *mut c_void,
    ) -> HRESULT {
        guard("DllGetClassObject", E_FAIL, || {
            if clsid.is_null() || iid.is_null() || object.is_null() {
                return E_POINTER;
            }
            *object = std::ptr::null_mut();
            if *clsid != CLSID_TEXT_SERVICE {
                return CLASS_E_CLASSNOTAVAILABLE;
            }
            let factory: IClassFactory = Factory.into();
            factory.query(iid, object)
        })
    }

    #[no_mangle]
    extern "system" fn DllCanUnloadNow() -> HRESULT {
        if globals::can_unload() {
            S_OK
        } else {
            S_FALSE
        }
    }

    #[no_mangle]
    extern "system" fn DllRegisterServer() -> HRESULT {
        guard("DllRegisterServer", E_FAIL, || match register::register() {
            Ok(()) => {
                log::info("registered");
                S_OK
            }
            Err(e) => {
                log::error(&format!("register failed: {e}"));
                e.code()
            }
        })
    }

    #[no_mangle]
    extern "system" fn DllUnregisterServer() -> HRESULT {
        guard(
            "DllUnregisterServer",
            E_FAIL,
            || match register::unregister() {
                Ok(()) => S_OK,
                Err(e) => e.code(),
            },
        )
    }
}

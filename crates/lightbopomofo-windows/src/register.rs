//! `DllRegisterServer` / `DllUnregisterServer`: the COM class in the
//! registry, the zh-TW language profile and the TSF categories that make the
//! input method available everywhere (including Start search, Settings and
//! Store apps). Run by the installer through regsvr32, as administrator.

use windows::Win32::Foundation::{ERROR_SUCCESS, E_FAIL};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY, HKEY_CLASSES_ROOT,
    KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
};
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, ITfCategoryMgr,
    ITfInputProcessorProfileMgr, GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT, GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT, GUID_TFCAT_TIP_KEYBOARD,
};
use windows_core::{Result, GUID, PCWSTR};

use crate::globals::{
    dll_path, install_dir, wide, wide_nul, CLSID_TEXT_SERVICE, DISPLAY_NAME, GUID_PROFILE,
    LANGID_ZH_TW,
};

const CATEGORIES: [GUID; 5] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
];

fn clsid_key() -> String {
    format!("CLSID\\{{{:?}}}", CLSID_TEXT_SERVICE)
}

fn set_string(key: HKEY, name: Option<&str>, value: &str) -> Result<()> {
    let name = name.map(wide_nul);
    let name_ptr = name.as_ref().map_or(PCWSTR::null(), |n| PCWSTR(n.as_ptr()));
    let data = wide_nul(value);
    let bytes = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2) };
    unsafe { RegSetValueExW(key, name_ptr, None, REG_SZ, Some(bytes)) }.ok()
}

fn create_key(parent: HKEY, path: &str) -> Result<HKEY> {
    let path = wide_nul(path);
    let mut key = HKEY::default();
    unsafe {
        RegCreateKeyExW(
            parent,
            PCWSTR(path.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            None,
        )
    }
    .ok()?;
    Ok(key)
}

fn register_com_class() -> Result<()> {
    let dll = dll_path().ok_or_else(|| windows_core::Error::from(E_FAIL))?;
    let class = create_key(HKEY_CLASSES_ROOT, &clsid_key())?;
    let result = (|| {
        set_string(class, None, DISPLAY_NAME)?;
        let server = create_key(class, "InprocServer32")?;
        let r = set_string(server, None, &dll.to_string_lossy())
            .and_then(|_| set_string(server, Some("ThreadingModel"), "Apartment"));
        unsafe {
            let _ = RegCloseKey(server);
        }
        r
    })();
    unsafe {
        let _ = RegCloseKey(class);
    }
    result
}

fn register_profile() -> Result<()> {
    let icon = install_dir()
        .map(|d| {
            d.join("lightbopomofo-input.ico")
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_default();
    unsafe {
        let profiles: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
        profiles.RegisterProfile(
            &CLSID_TEXT_SERVICE,
            LANGID_ZH_TW,
            &GUID_PROFILE,
            &wide(DISPLAY_NAME),
            &wide(&icon),
            0,
            HKL::default(),
            0,
            true,
            0,
        )?;
        let categories: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        for category in CATEGORIES {
            categories.RegisterCategory(&CLSID_TEXT_SERVICE, &category, &CLSID_TEXT_SERVICE)?;
        }
    }
    Ok(())
}

pub fn register() -> Result<()> {
    register_com_class()?;
    register_profile()
}

/// Best effort: remove whatever is there.
pub fn unregister() -> Result<()> {
    unsafe {
        if let Ok(categories) =
            CoCreateInstance::<_, ITfCategoryMgr>(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)
        {
            for category in CATEGORIES {
                let _ = categories.UnregisterCategory(
                    &CLSID_TEXT_SERVICE,
                    &category,
                    &CLSID_TEXT_SERVICE,
                );
            }
        }
        if let Ok(profiles) = CoCreateInstance::<_, ITfInputProcessorProfileMgr>(
            &CLSID_TF_InputProcessorProfiles,
            None,
            CLSCTX_INPROC_SERVER,
        ) {
            let _ = profiles.UnregisterProfile(&CLSID_TEXT_SERVICE, LANGID_ZH_TW, &GUID_PROFILE, 0);
        }
        let key = wide_nul(&clsid_key());
        let status = RegDeleteTreeW(HKEY_CLASSES_ROOT, PCWSTR(key.as_ptr()));
        if status != ERROR_SUCCESS && status.0 != 2 {
            // 2 = not found: already gone.
            return status.ok();
        }
    }
    Ok(())
}

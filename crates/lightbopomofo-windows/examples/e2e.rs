//! End-to-end check on Windows (run by windows/e2e.ps1 after the DLL is
//! installed and registered): activate 輕注音 in this process — as choosing
//! it with Win+Space would — type into an Edit control with real key
//! presses (SendInput) and read back what arrived. Exit code 0 = all passed.

#[cfg(not(windows))]
fn main() {
    eprintln!("e2e runs on Windows only");
}

#[cfg(windows)]
fn main() {
    std::process::exit(imp::run());
}

#[cfg(windows)]
mod imp {
    use std::time::{Duration, Instant};

    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, SetFocus, HKL, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
        KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_ESCAPE, VK_OEM_COMMA, VK_SHIFT, VK_SPACE,
    };
    use windows::Win32::UI::TextServices::{
        CLSID_TF_InputProcessorProfiles, ITfInputProcessorProfileMgr,
        TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE, TF_IPPMF_FORPROCESS, TF_PROFILETYPE_INPUTPROCESSOR,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetWindowTextW, PeekMessageW, SendMessageW,
        SetForegroundWindow, ShowWindow, TranslateMessage, MSG, PM_REMOVE, SW_SHOW,
        WINDOW_EX_STYLE, WINDOW_STYLE, WM_SETTEXT, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    };
    use windows_core::{w, GUID};

    // Must match src/globals.rs.
    const CLSID_TEXT_SERVICE: GUID = GUID::from_u128(0x0d5e0d6e_3056_4888_9151_8701b588ad0c);
    const GUID_PROFILE: GUID = GUID::from_u128(0x31638591_41d6_4dc9_b7d8_c2f29953f24b);

    /// Process window messages for a while (TSF handles keys in the pump).
    fn pump(ms: u64) {
        let until = Instant::now() + Duration::from_millis(ms);
        let mut msg = MSG::default();
        while Instant::now() < until {
            unsafe {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    ..Default::default()
                },
            },
        }
    }

    /// Press and release `vk`, with Shift held when `shift`.
    fn tap(vk: VIRTUAL_KEY, shift: bool) {
        let mut inputs = Vec::new();
        if shift {
            inputs.push(key(VK_SHIFT, false));
        }
        inputs.push(key(vk, false));
        inputs.push(key(vk, true));
        if shift {
            inputs.push(key(VK_SHIFT, true));
        }
        unsafe {
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
        pump(120);
    }

    /// Keys to press: virtual key and whether Shift is held.
    type Keys = Vec<(VIRTUAL_KEY, bool)>;

    fn letters(s: &str) -> Keys {
        s.chars()
            .map(|c| (VIRTUAL_KEY(c.to_ascii_uppercase() as u16), false))
            .collect()
    }

    fn text(hwnd: HWND) -> String {
        let mut buf = [0u16; 1024];
        let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    pub fn run() -> i32 {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let instance = GetModuleHandleW(None).unwrap_or_default();
            let Ok(edit) = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EDIT"),
                w!(""),
                WINDOW_STYLE(WS_OVERLAPPEDWINDOW.0 | WS_VISIBLE.0),
                100,
                100,
                640,
                240,
                None,
                None,
                Some(instance.into()),
                None,
            ) else {
                println!("FAIL create window");
                return 1;
            };
            let _ = ShowWindow(edit, SW_SHOW);
            let foreground = SetForegroundWindow(edit).as_bool();
            let _ = SetFocus(Some(edit));
            println!(
                "{} foreground window",
                if foreground { "PASS" } else { "NOTE no" }
            );
            pump(500);

            let activated = CoCreateInstance::<_, ITfInputProcessorProfileMgr>(
                &CLSID_TF_InputProcessorProfiles,
                None,
                CLSCTX_INPROC_SERVER,
            )
            .and_then(|profiles| {
                profiles.ActivateProfile(
                    TF_PROFILETYPE_INPUTPROCESSOR,
                    0x0404,
                    &CLSID_TEXT_SERVICE,
                    &GUID_PROFILE,
                    HKL::default(),
                    TF_IPPMF_FORPROCESS | TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE,
                )
            });
            match &activated {
                Ok(()) => println!("PASS activate 輕注音"),
                Err(e) => println!("FAIL activate 輕注音: {e}"),
            }
            pump(800);

            let mut cases: Vec<(&str, Keys, &str)> = Vec::new();
            let mut keys = letters("sca");
            keys.push((VK_SPACE, false));
            cases.push(("ㄋㄏㄇ + Space → 你好嗎", keys, "你好嗎"));
            let mut keys = letters("eyzz");
            keys.push((VK_SPACE, false));
            cases.push(("ㄍㄗㄈㄈ + Space → 工作方法", keys, "工作方法"));
            let mut keys = letters("sca");
            keys.push((VK_OEM_COMMA, true));
            cases.push(("ㄋㄏㄇ + Shift+, → 你好嗎，", keys, "你好嗎，"));
            let mut keys = letters("sc");
            keys.push((VK_ESCAPE, false));
            cases.push(("ㄋㄏ + Esc → (nothing)", keys, ""));

            let mut failed = activated.is_err() as i32;
            for (name, keys, expected) in cases {
                let empty = [0u16];
                SendMessageW(
                    edit,
                    WM_SETTEXT,
                    Some(WPARAM(0)),
                    Some(LPARAM(empty.as_ptr() as isize)),
                );
                pump(100);
                for (vk, shift) in keys {
                    tap(vk, shift);
                }
                pump(400);
                let got = text(edit);
                if got == expected {
                    println!("PASS {name}");
                } else {
                    println!("FAIL {name}: got '{got}'");
                    failed += 1;
                }
            }
            failed
        }
    }
}

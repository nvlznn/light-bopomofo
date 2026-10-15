//! End-to-end check on Windows (run by windows/e2e.ps1 after installing):
//! activate 輕注音 in this process — as choosing it with Win+Space would —
//! type into an Edit control with real key presses (SendInput) and check
//! what arrives, step by step.
//!
//!   e2e.exe              run every scenario; exit code = number of failures
//!   e2e.exe --hold SECS  activate and keep the input method loaded (for the
//!                        update-while-in-use test)

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
        GetKeyState, SendInput, SetFocus, HKL, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
        KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_BACK, VK_CAPITAL, VK_CONTROL,
        VK_ESCAPE, VK_OEM_COMMA, VK_OEM_PERIOD, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_SPACE,
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

    #[derive(Clone, Copy)]
    enum Mod {
        None,
        Shift,
        Ctrl,
    }

    enum Step {
        /// 大千 keys typed as letters/digits (e.g. "sca" = ㄋㄏㄇ).
        Type(&'static str),
        Key(VIRTUAL_KEY, Mod),
        /// Shift pressed and released alone (中/英 switch).
        ShiftTap,
        Expect(&'static str),
        ExpectPrefix(&'static str),
        Save,
        ExpectSaved,
    }
    use Step::*;

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

    fn input(vk: VIRTUAL_KEY, up: bool) -> INPUT {
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

    fn send(inputs: &[INPUT]) {
        unsafe {
            SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
        }
        pump(120);
    }

    fn press(vk: VIRTUAL_KEY, m: Mod) {
        let modifier = match m {
            Mod::None => None,
            Mod::Shift => Some(VK_SHIFT),
            Mod::Ctrl => Some(VK_CONTROL),
        };
        let mut inputs = Vec::new();
        inputs.extend(modifier.map(|k| input(k, false)));
        inputs.push(input(vk, false));
        inputs.push(input(vk, true));
        inputs.extend(modifier.map(|k| input(k, true)));
        send(&inputs);
    }

    fn type_keys(s: &str) {
        for c in s.chars() {
            let vk = match c {
                ',' => VK_OEM_COMMA,
                '.' => VK_OEM_PERIOD,
                c => VIRTUAL_KEY(c.to_ascii_uppercase() as u16),
            };
            press(vk, Mod::None);
        }
    }

    fn text(hwnd: HWND) -> String {
        let mut buf = [0u16; 1024];
        let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    fn clear(hwnd: HWND) {
        let empty = [0u16];
        unsafe {
            SendMessageW(
                hwnd,
                WM_SETTEXT,
                Some(WPARAM(0)),
                Some(LPARAM(empty.as_ptr() as isize)),
            );
        }
        pump(100);
    }

    fn caps_on() -> bool {
        (unsafe { GetKeyState(VK_CAPITAL.0 as i32) }) & 1 != 0
    }

    fn space() -> Step {
        Key(VK_SPACE, Mod::None)
    }

    fn right() -> Step {
        Key(VK_RIGHT, Mod::None)
    }

    fn scenarios() -> Vec<(&'static str, Vec<Step>)> {
        vec![
            (
                "ㄋㄏㄇ shows inline, Space → 你好嗎",
                vec![Type("sca"), Expect("ㄋㄏㄇ"), space(), Expect("你好嗎")],
            ),
            (
                "whole conversion ㄍㄗㄈㄈ → 工作方法",
                vec![Type("eyzz"), space(), Expect("工作方法")],
            ),
            (
                "choose the front part 工作, the rest stays",
                vec![
                    Type("eyzz"),
                    right(),
                    right(),
                    right(),
                    right(),
                    space(),
                    Expect("工作ㄈㄈ"),
                    space(),
                    Expect("工作方法"),
                ],
            ),
            (
                "Shift+, commits then ，",
                vec![
                    Type("sca"),
                    Key(VK_OEM_COMMA, Mod::Shift),
                    Expect("你好嗎，"),
                ],
            ),
            (
                "Shift+. alone → 。",
                vec![Key(VK_OEM_PERIOD, Mod::Shift), Expect("。")],
            ),
            (
                "Esc drops the composition",
                vec![Type("sc"), Key(VK_ESCAPE, Mod::None), Expect("")],
            ),
            (
                "Backspace removes symbols, then passes through",
                vec![
                    Type("sca"),
                    Key(VK_BACK, Mod::None),
                    Expect("ㄋㄏ"),
                    Key(VK_BACK, Mod::None),
                    Key(VK_BACK, Mod::None),
                    Expect(""),
                ],
            ),
            ("tone ㄇㄚˇ → 馬", vec![Type("a83"), space(), Expect("馬")]),
            (
                "Enter commits",
                vec![Type("sca"), Key(VK_RETURN, Mod::None), Expect("你好嗎")],
            ),
            (
                "lone Shift switches to English and back",
                vec![
                    ShiftTap,
                    Type("abc"),
                    Expect("abc"),
                    ShiftTap,
                    Type("sca"),
                    space(),
                    Expect("abc你好嗎"),
                ],
            ),
            (
                "Caps Lock types English",
                vec![
                    Key(VK_CAPITAL, Mod::None),
                    Type("ab"),
                    Expect("AB"),
                    Key(VK_CAPITAL, Mod::None),
                ],
            ),
            (
                "Ctrl+A while composing commits first",
                vec![
                    Type("sca"),
                    Key(VIRTUAL_KEY(b'A' as u16), Mod::Ctrl),
                    Expect("你好嗎"),
                ],
            ),
            (
                "a composition chosen in pieces is learned",
                vec![
                    Type("wfcc"),
                    right(),
                    right(),
                    space(),
                    ExpectPrefix("天氣"),
                    space(),
                    ExpectPrefix("天氣"),
                    Save,
                ],
            ),
            (
                "…and comes back first next time",
                vec![Type("wfcc"), space(), ExpectSaved],
            ),
        ]
    }

    fn activate() -> windows_core::Result<()> {
        unsafe {
            let profiles: ITfInputProcessorProfileMgr =
                CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
            profiles.ActivateProfile(
                TF_PROFILETYPE_INPUTPROCESSOR,
                0x0404,
                &CLSID_TEXT_SERVICE,
                &GUID_PROFILE,
                HKL::default(),
                TF_IPPMF_FORPROCESS | TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE,
            )
        }
    }

    fn check(name: &str, edit: HWND, want: &str, prefix: bool) -> bool {
        pump(200);
        let got = text(edit);
        let ok = if prefix {
            got.starts_with(want)
        } else {
            got == want
        };
        if !ok {
            println!(
                "FAIL {name}: expected '{want}{}', got '{got}'",
                if prefix { "…" } else { "" }
            );
        }
        ok
    }

    pub fn run() -> i32 {
        let args: Vec<String> = std::env::args().collect();
        let hold = (args.get(1).map(String::as_str) == Some("--hold")).then(|| {
            args.get(2)
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60)
        });

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
                if foreground { "PASS" } else { "NOTE not" }
            );
            pump(500);
            if caps_on() {
                press(VK_CAPITAL, Mod::None); // start from a known state
            }

            let mut failed = 0;
            match activate() {
                Ok(()) => println!("PASS activate 輕注音"),
                Err(e) => {
                    println!("FAIL activate 輕注音: {e}");
                    failed += 1;
                }
            }
            pump(800);

            if let Some(seconds) = hold {
                println!("holding the input method for {seconds} s");
                pump(seconds * 1000);
                return failed;
            }

            let mut saved = String::new();
            for (name, steps) in scenarios() {
                clear(edit);
                let mut ok = true;
                for step in steps {
                    ok = match step {
                        Type(keys) => {
                            type_keys(keys);
                            true
                        }
                        Key(vk, m) => {
                            press(vk, m);
                            true
                        }
                        ShiftTap => {
                            send(&[input(VK_SHIFT, false), input(VK_SHIFT, true)]);
                            true
                        }
                        Save => {
                            saved = text(edit);
                            true
                        }
                        Expect(want) => check(name, edit, want, false),
                        ExpectPrefix(want) => check(name, edit, want, true),
                        ExpectSaved => {
                            !saved.is_empty() && check(name, edit, &saved.clone(), false)
                        }
                    };
                    if !ok {
                        break;
                    }
                }
                // Leave nothing composing and Caps Lock off for the next one.
                press(VK_ESCAPE, Mod::None);
                if caps_on() {
                    press(VK_CAPITAL, Mod::None);
                }
                if ok {
                    println!("PASS {name}");
                } else {
                    failed += 1;
                }
            }
            failed
        }
    }
}

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
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, SendInput, SetFocus, HKL, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
        KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_BACK, VK_CAPITAL, VK_CONTROL,
        VK_ESCAPE, VK_OEM_COMMA, VK_OEM_PERIOD, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_SPACE,
    };
    use windows::Win32::UI::TextServices::{
        CLSID_TF_InputProcessorProfiles, CLSID_TF_ThreadMgr, ITfInputProcessorProfileMgr,
        ITfKeystrokeMgr, ITfThreadMgr, TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE, TF_IPPMF_FORPROCESS,
        TF_PROFILETYPE_INPUTPROCESSOR,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, DispatchMessageW, GetWindowTextW, PeekMessageW,
        SendMessageW, SetForegroundWindow, ShowWindow, TranslateMessage, MSG, PM_REMOVE, SW_SHOW,
        WINDOW_EX_STYLE, WINDOW_STYLE, WM_KEYDOWN, WM_KEYUP, WM_SETTEXT, WS_OVERLAPPEDWINDOW,
        WS_VISIBLE,
    };
    use windows_core::{w, Interface, GUID};

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
        /// The document's text (committed text, plus any composition in
        /// TSF-aware controls).
        Expect(&'static str),
        /// The composition itself visible in the document: checked only in
        /// TSF-aware controls (a plain Edit shows it in a separate window).
        ExpectInline(&'static str),
        ExpectPrefix(&'static str),
        Save,
        ExpectSaved,
    }
    use Step::*;

    thread_local! {
        /// Set once this thread runs TSF itself, as TSF-aware applications
        /// (Word, browsers) do: their message loop hands keys to TSF first.
        static KEYSTROKES: std::cell::RefCell<Option<ITfKeystrokeMgr>> =
            const { std::cell::RefCell::new(None) };
    }

    /// Whether TSF took the key (so it must not reach the control).
    fn tsf_takes(msg: &MSG) -> bool {
        KEYSTROKES.with(|k| {
            let Some(keys) = k.borrow().clone() else {
                return false;
            };
            unsafe {
                match msg.message {
                    WM_KEYDOWN => {
                        keys.TestKeyDown(msg.wParam, msg.lParam)
                            .is_ok_and(|e| e.as_bool())
                            && keys
                                .KeyDown(msg.wParam, msg.lParam)
                                .is_ok_and(|e| e.as_bool())
                    }
                    WM_KEYUP => {
                        keys.TestKeyUp(msg.wParam, msg.lParam)
                            .is_ok_and(|e| e.as_bool())
                            && keys
                                .KeyUp(msg.wParam, msg.lParam)
                                .is_ok_and(|e| e.as_bool())
                    }
                    _ => false,
                }
            }
        })
    }

    fn pump(ms: u64) {
        let until = Instant::now() + Duration::from_millis(ms);
        let mut msg = MSG::default();
        while Instant::now() < until {
            unsafe {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    if tsf_takes(&msg) {
                        continue;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Run TSF on this thread like a TSF-aware application.
    fn become_tsf_aware() -> windows_core::Result<ITfThreadMgr> {
        unsafe {
            let thread_mgr: ITfThreadMgr =
                CoCreateInstance(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER)?;
            thread_mgr.Activate()?;
            let keys: ITfKeystrokeMgr = thread_mgr.cast()?;
            KEYSTROKES.with(|k| *k.borrow_mut() = Some(keys));
            Ok(thread_mgr)
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
                vec![
                    Type("sca"),
                    ExpectInline("ㄋㄏㄇ"),
                    space(),
                    Expect("你好嗎"),
                ],
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
                    ExpectInline("工作ㄈㄈ"),
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
                    ExpectInline("ㄋㄏ"),
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

    fn open_window(class: windows_core::PCWSTR) -> Option<HWND> {
        unsafe {
            let instance = GetModuleHandleW(None).unwrap_or_default();
            let edit = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
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
            )
            .ok()?;
            let _ = ShowWindow(edit, SW_SHOW);
            let foreground = SetForegroundWindow(edit).as_bool();
            let _ = SetFocus(Some(edit));
            if !foreground {
                println!("NOTE window not in the foreground");
            }
            pump(500);
            Some(edit)
        }
    }

    /// Run every scenario in `edit`; returns the number that failed.
    fn run_scenarios(label: &str, edit: HWND, inline: bool) -> i32 {
        let mut failed = 0;
        let mut saved = String::new();
        for (name, steps) in scenarios() {
            let name = format!("[{label}] {name}");
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
                    Expect(want) => check(&name, edit, want, false),
                    ExpectInline(want) => !inline || check(&name, edit, want, false),
                    ExpectPrefix(want) => check(&name, edit, want, true),
                    ExpectSaved => !saved.is_empty() && check(&name, edit, &saved.clone(), false),
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

    pub fn run() -> i32 {
        let args: Vec<String> = std::env::args().collect();
        let hold = (args.get(1).map(String::as_str) == Some("--hold")).then(|| {
            args.get(2)
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60)
        });

        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let _ = LoadLibraryW(w!("Msftedit.dll")); // RICHEDIT50W
        }
        if caps_on() {
            press(VK_CAPITAL, Mod::None); // start from a known state
        }
        // A TSF-aware control (like Word or a browser: the composition is in
        // the document) and a plain Edit (older apps: Windows shows the
        // composition in its own window, only committed text reaches it).
        let controls = [
            ("Edit, no TSF", w!("EDIT"), false),
            ("RichEdit, TSF-aware", w!("RICHEDIT50W"), true),
        ];
        // Kept alive while the TSF-aware control is tested.
        let mut _keep_tsf_alive = None;
        let mut failed = 0;
        let mut activated = false;
        for (label, class, inline) in controls {
            let Some(edit) = open_window(class) else {
                println!("FAIL [{label}] create window");
                failed += 1;
                continue;
            };
            if inline {
                match become_tsf_aware() {
                    Ok(t) => _keep_tsf_alive = Some(t),
                    Err(e) => println!("NOTE could not run TSF on this thread: {e}"),
                }
                const EM_SETEDITSTYLE: u32 = 0x0400 + 204;
                const SES_USECTF: usize = 0x0001_0000;
                unsafe {
                    SendMessageW(
                        edit,
                        EM_SETEDITSTYLE,
                        Some(WPARAM(SES_USECTF)),
                        Some(LPARAM(SES_USECTF as isize)),
                    );
                }
            }
            if !activated {
                activated = true;
                match activate() {
                    Ok(()) => println!("PASS activate 輕注音"),
                    Err(e) => {
                        println!("FAIL activate 輕注音: {e}");
                        return failed + 1;
                    }
                }
                pump(800);
                if let Some(seconds) = hold {
                    println!("holding the input method for {seconds} s");
                    pump(seconds * 1000);
                    return failed;
                }
            }
            failed += run_scenarios(label, edit, inline);
            unsafe {
                let _ = DestroyWindow(edit);
            }
            pump(300);
        }
        failed
    }
}

//! The window: status of the input method, a key guide, and the history
//! (search, sort, delete, clear). Native Win32 controls, per-monitor DPI.

use std::cell::RefCell;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use lightbopomofo_core::history::HistoryEntry;
use lightbopomofo_core::History;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, DeleteObject, GetSysColorBrush, COLOR_WINDOW, FW_SEMIBOLD, HFONT, HGDIOBJ,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CLASSES_ROOT,
    HKEY_CURRENT_USER, KEY_READ,
};
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, ICC_LISTVIEW_CLASSES, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX,
    LVCF_TEXT, LVCF_WIDTH, LVCOLUMNW, LVIF_TEXT, LVITEMW, LVM_DELETEALLITEMS, LVM_GETNEXTITEM,
    LVM_INSERTCOLUMNW, LVM_INSERTITEMW, LVM_SETCOLUMNWIDTH, LVM_SETEXTENDEDLISTVIEWSTYLE,
    LVM_SETITEMTEXTW, LVNI_SELECTED, LVN_COLUMNCLICK, LVN_KEYDOWN, LVS_EX_DOUBLEBUFFER,
    LVS_EX_FULLROWSELECT, LVS_REPORT, LVS_SHOWSELALWAYS, NMHDR, NMLISTVIEW, NMLVKEYDOWN,
    WC_LISTVIEWW,
};
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, SetProcessDpiAwarenessContext, SystemParametersInfoForDpi,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_DELETE;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::TextServices::{
    CLSID_TF_InputProcessorProfiles, ITfInputProcessorProfiles,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW,
    IsDialogMessageW, LoadIconW, MessageBoxW, PostQuitMessage, RegisterClassExW, SendMessageW,
    SetWindowPos, SetWindowTextW, ShowWindow, TranslateMessage, BN_CLICKED, BS_PUSHBUTTON,
    CW_USEDEFAULT, EN_CHANGE, ES_AUTOHSCROLL, HMENU, IDYES, MB_ICONWARNING, MB_YESNO, MINMAXINFO,
    MSG, NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS, SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE,
    SW_SHOW, SW_SHOWNORMAL, WINDOW_EX_STYLE, WINDOW_STYLE, WM_ACTIVATE, WM_COMMAND, WM_CREATE,
    WM_DESTROY, WM_DPICHANGED, WM_GETMINMAXINFO, WM_NOTIFY, WM_SETFONT, WM_SIZE, WNDCLASSEXW,
    WS_BORDER, WS_CHILD, WS_CLIPCHILDREN, WS_EX_CLIENTEDGE, WS_OVERLAPPEDWINDOW, WS_TABSTOP,
    WS_VISIBLE, WS_VSCROLL,
};
use windows_core::{s, w, BOOL, GUID, PCWSTR, PWSTR};

// Must match crates/lightbopomofo-windows/src/globals.rs.
const CLSID_TEXT_SERVICE: GUID = GUID::from_u128(0x0d5e0d6e_3056_4888_9151_8701b588ad0c);
const GUID_PROFILE: GUID = GUID::from_u128(0x31638591_41d6_4dc9_b7d8_c2f29953f24b);
const LANGID_ZH_TW: u16 = 0x0404;
const VERSION: &str = env!("CARGO_PKG_VERSION");

const ID_PRIMARY: i32 = 101;
const ID_SEARCH: i32 = 102;
const ID_LIST: i32 = 103;
const ID_DELETE: i32 = 104;
const ID_CLEAR: i32 = 105;
const ID_SITE: i32 = 106;
const ID_ISSUES: i32 = 107;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

// ---------------------------------------------------------------------------
// The input method's state on this machine.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    NotInstalled,
    NotEnabled,
    Ready,
}

fn registered() -> bool {
    let path = wide(&format!(
        "CLSID\\{{{:?}}}\\InprocServer32",
        CLSID_TEXT_SERVICE
    ));
    let mut key = HKEY::default();
    let ok = unsafe {
        RegOpenKeyExW(
            HKEY_CLASSES_ROOT,
            PCWSTR(path.as_ptr()),
            None,
            KEY_READ,
            &mut key,
        )
    }
    .is_ok();
    if ok {
        unsafe {
            let _ = RegCloseKey(key);
        }
    }
    ok
}

fn enabled() -> bool {
    unsafe {
        CoCreateInstance::<_, ITfInputProcessorProfiles>(
            &CLSID_TF_InputProcessorProfiles,
            None,
            CLSCTX_INPROC_SERVER,
        )
        .and_then(|p| p.IsEnabledLanguageProfile(&CLSID_TEXT_SERVICE, LANGID_ZH_TW, &GUID_PROFILE))
        .is_ok_and(|b| b.as_bool())
    }
}

/// The `0404:{CLSID}{PROFILE}` name Windows uses for this input method.
fn tip_spec() -> String {
    format!(
        "{LANGID_ZH_TW:04X}:{{{:?}}}{{{:?}}}",
        CLSID_TEXT_SERVICE, GUID_PROFILE
    )
}

/// Whether 輕注音 is in the user's input methods: Windows 10/11 keep that
/// list per language under HKCU\Control Panel\International\User Profile,
/// as `0404:{CLSID}{PROFILE}` values. `None` when that key doesn't exist.
fn in_user_list() -> Option<bool> {
    let value = wide(&tip_spec());
    let mut root = HKEY::default();
    unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Control Panel\\International\\User Profile"),
            None,
            KEY_READ,
            &mut root,
        )
        .ok()
        .ok()?;
    }
    let mut found = false;
    for index in 0.. {
        let mut name = [0u16; 256];
        let mut len = name.len() as u32;
        let more = unsafe {
            RegEnumKeyExW(
                root,
                index,
                Some(PWSTR(name.as_mut_ptr())),
                &mut len,
                None,
                None,
                None,
                None,
            )
        };
        if more.is_err() {
            break;
        }
        let mut language = name[..len as usize].to_vec();
        language.push(0);
        let mut key = HKEY::default();
        unsafe {
            if RegOpenKeyExW(root, PCWSTR(language.as_ptr()), None, KEY_READ, &mut key).is_ok() {
                found =
                    RegQueryValueExW(key, PCWSTR(value.as_ptr()), None, None, None, None).is_ok();
                let _ = RegCloseKey(key);
            }
        }
        if found {
            break;
        }
    }
    unsafe {
        let _ = RegCloseKey(root);
    }
    Some(found)
}

fn status() -> Status {
    if !registered() {
        Status::NotInstalled
    } else if in_user_list().unwrap_or_else(enabled) {
        Status::Ready
    } else {
        Status::NotEnabled
    }
}

/// Add 輕注音 to (or remove it from) the user's input methods, the way
/// Settings does, through input.dll's InstallLayoutOrTip.
fn install_layout_or_tip(enable: bool) -> bool {
    const ILOT_UNINSTALL: u32 = 0x1;
    unsafe {
        let Ok(input) = LoadLibraryW(w!("input.dll")) else {
            return false;
        };
        let Some(function) = GetProcAddress(input, s!("InstallLayoutOrTip")) else {
            return false;
        };
        let install: unsafe extern "system" fn(PCWSTR, u32) -> BOOL = std::mem::transmute(function);
        let spec = wide(&tip_spec());
        install(
            PCWSTR(spec.as_ptr()),
            if enable { 0 } else { ILOT_UNINSTALL },
        )
        .as_bool()
    }
}

fn open(target: PCWSTR) {
    unsafe {
        ShellExecuteW(None, w!("open"), target, None, None, SW_SHOWNORMAL);
    }
}

// ---------------------------------------------------------------------------
// History.
// ---------------------------------------------------------------------------

fn history_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("LightBopomofo\\history.tsv"))
}

fn ago(unix: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let s = now.saturating_sub(unix);
    match s {
        0..60 => "剛剛".into(),
        60..3600 => format!("{} 分鐘前", s / 60),
        3600..86400 => format!("{} 小時前", s / 3600),
        _ => format!("{} 天前", s / 86400),
    }
}

// ---------------------------------------------------------------------------
// The window.
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Ui {
    hwnd: HWND,
    title: HWND,
    detail: HWND,
    primary: HWND,
    guide: HWND,
    history_label: HWND,
    search: HWND,
    count: HWND,
    list: HWND,
    delete: HWND,
    clear: HWND,
    site: HWND,
    issues: HWND,
    font: Option<HFONT>,
    bold: Option<HFONT>,
    dpi: u32,
    status: Option<Status>,
    history: Option<History>,
    entries: Vec<HistoryEntry>,
    /// Indices into `entries`, filtered and sorted as shown.
    rows: Vec<usize>,
    sort: (usize, bool),
}

thread_local! {
    static UI: RefCell<Ui> = RefCell::new(Ui { sort: (3, false), ..Ui::default() });
}

fn scale(px: i32, dpi: u32) -> i32 {
    px * dpi.max(96) as i32 / 96
}

fn child(parent: HWND, class: PCWSTR, text: &str, style: u32, ex: u32, id: i32) -> HWND {
    let text = wide(text);
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(ex),
            class,
            PCWSTR(text.as_ptr()),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | style),
            0,
            0,
            0,
            0,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            GetModuleHandleW(None).ok().map(Into::into),
            None,
        )
        .unwrap_or_default()
    }
}

fn set_text(hwnd: HWND, text: &str) {
    let text = wide(text);
    unsafe {
        let _ = SetWindowTextW(hwnd, PCWSTR(text.as_ptr()));
    }
}

fn send(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    unsafe { SendMessageW(hwnd, msg, Some(WPARAM(wparam)), Some(LPARAM(lparam))).0 }
}

fn fonts(ui: &mut Ui) {
    for f in [ui.font.take(), ui.bold.take()].into_iter().flatten() {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(f.0));
        }
    }
    let mut metrics = NONCLIENTMETRICSW {
        cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
        ..Default::default()
    };
    unsafe {
        let _ = SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS.0,
            metrics.cbSize,
            Some(&mut metrics as *mut _ as *mut _),
            0,
            ui.dpi,
        );
        let mut normal = metrics.lfMessageFont;
        let face = wide("Microsoft JhengHei UI");
        normal.lfFaceName = [0; 32];
        normal.lfFaceName[..face.len()].copy_from_slice(&face);
        let mut bold = normal;
        bold.lfHeight = normal.lfHeight * 13 / 10;
        bold.lfWeight = FW_SEMIBOLD.0 as i32;
        ui.font = Some(CreateFontIndirectW(&normal));
        ui.bold = Some(CreateFontIndirectW(&bold));
    }
    let controls = [
        ui.detail,
        ui.primary,
        ui.guide,
        ui.history_label,
        ui.search,
        ui.count,
        ui.list,
        ui.delete,
        ui.clear,
        ui.site,
        ui.issues,
    ];
    for c in controls {
        send(c, WM_SETFONT, ui.font.map_or(0, |f| f.0 as usize), 1);
    }
    send(ui.title, WM_SETFONT, ui.bold.map_or(0, |f| f.0 as usize), 1);
}

fn layout(ui: &Ui) {
    let mut rect = RECT::default();
    unsafe {
        let _ = GetClientRect(ui.hwnd, &mut rect);
    }
    let d = ui.dpi;
    let (w, h) = (rect.right, rect.bottom);
    let m = scale(20, d);
    let place = |hwnd: HWND, x: i32, y: i32, cx: i32, cy: i32| unsafe {
        let _ = SetWindowPos(
            hwnd,
            None,
            x,
            y,
            cx.max(0),
            cy.max(0),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    };
    let inner = w - 2 * m;
    place(ui.title, m, m, inner, scale(30, d));
    place(ui.detail, m, m + scale(34, d), inner, scale(40, d));
    place(ui.primary, m, m + scale(78, d), scale(180, d), scale(32, d));
    place(ui.guide, m, m + scale(124, d), inner, scale(110, d));
    let hy = m + scale(244, d);
    place(
        ui.history_label,
        m,
        hy + scale(4, d),
        scale(120, d),
        scale(24, d),
    );
    place(
        ui.count,
        w - m - scale(320, d),
        hy + scale(4, d),
        scale(80, d),
        scale(24, d),
    );
    place(
        ui.search,
        w - m - scale(230, d),
        hy,
        scale(230, d),
        scale(28, d),
    );
    let ly = hy + scale(36, d);
    let bottom = h - m - scale(32, d) - scale(12, d);
    place(ui.list, m, ly, inner, bottom - ly);
    let by = h - m - scale(32, d);
    place(ui.delete, m, by, scale(120, d), scale(32, d));
    place(ui.clear, m + scale(128, d), by, scale(120, d), scale(32, d));
    place(
        ui.issues,
        w - m - scale(100, d),
        by,
        scale(100, d),
        scale(32, d),
    );
    place(
        ui.site,
        w - m - scale(208, d),
        by,
        scale(100, d),
        scale(32, d),
    );
    let col = |i: usize, width: i32| send(ui.list, LVM_SETCOLUMNWIDTH, i, scale(width, d) as isize);
    let rest = (inner - scale(24, d)) as f32;
    col(0, (rest * 0.30 * 96.0 / d as f32) as i32);
    col(1, (rest * 0.38 * 96.0 / d as f32) as i32);
    col(2, (rest * 0.12 * 96.0 / d as f32) as i32);
    col(3, (rest * 0.20 * 96.0 / d as f32) as i32);
}

fn show_status(ui: &mut Ui) {
    let s = status();
    ui.status = Some(s);
    let (title, detail, button) = match s {
        Status::NotInstalled => (
            "輸入法沒有安裝".to_string(),
            "請重新執行輕注音的安裝程式。",
            None,
        ),
        Status::NotEnabled => (
            "輕注音已安裝，還沒加入輸入法".to_string(),
            "按下面的按鈕把它加入你的輸入法清單。",
            Some("加入輸入法"),
        ),
        Status::Ready => (
            format!("輕注音 {VERSION} 可以使用了"),
            "按 Windows 鍵 + 空白鍵 切換到「輕注音」就可以打字；按一下 Shift 切換中／英。",
            Some("開啟語言設定"),
        ),
    };
    set_text(ui.title, &title);
    set_text(ui.detail, detail);
    match button {
        Some(label) => unsafe {
            set_text(ui.primary, label);
            let _ = ShowWindow(ui.primary, SW_SHOW);
        },
        None => unsafe {
            let _ = ShowWindow(ui.primary, SW_HIDE);
        },
    }
}

fn search_text(ui: &Ui) -> String {
    let mut buf = [0u16; 256];
    let n = unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowTextW(ui.search, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
        .trim()
        .to_string()
}

fn reload_history(ui: &mut Ui) {
    if ui.history.is_none() {
        ui.history = history_path().map(History::load);
    }
    if let Some(h) = ui.history.as_mut() {
        h.reload_if_changed();
        ui.entries = h.entries();
    }
    fill_list(ui);
}

fn fill_list(ui: &mut Ui) {
    let query = search_text(ui);
    let mut rows: Vec<usize> = (0..ui.entries.len())
        .filter(|&i| {
            let e = &ui.entries[i];
            query.is_empty() || e.typed.contains(&query) || e.text.contains(&query)
        })
        .collect();
    let (column, ascending) = ui.sort;
    let entries = &ui.entries;
    rows.sort_by(|&a, &b| {
        let (a, b) = (&entries[a], &entries[b]);
        let order = match column {
            0 => a.typed.cmp(&b.typed),
            1 => a.text.cmp(&b.text),
            2 => a.count.cmp(&b.count),
            _ => a.last_used.cmp(&b.last_used),
        };
        if ascending {
            order
        } else {
            order.reverse()
        }
    });
    send(ui.list, LVM_DELETEALLITEMS, 0, 0);
    for (row, &i) in rows.iter().enumerate() {
        let e = &ui.entries[i];
        let cells = [
            e.typed.clone(),
            e.text.clone(),
            e.count.to_string(),
            ago(e.last_used),
        ];
        for (col, cell) in cells.iter().enumerate() {
            let mut text = wide(cell);
            let mut item = LVITEMW {
                mask: LVIF_TEXT,
                iItem: row as i32,
                iSubItem: col as i32,
                pszText: windows_core::PWSTR(text.as_mut_ptr()),
                ..Default::default()
            };
            let msg = if col == 0 {
                LVM_INSERTITEMW
            } else {
                LVM_SETITEMTEXTW
            };
            let wparam = if col == 0 { 0 } else { row };
            send(ui.list, msg, wparam, &mut item as *mut _ as isize);
        }
    }
    set_text(ui.count, &format!("{} 筆", ui.entries.len()));
    ui.rows = rows;
}

fn selected_rows(ui: &Ui) -> Vec<usize> {
    let mut out = Vec::new();
    let mut i = -1isize;
    loop {
        i = send(ui.list, LVM_GETNEXTITEM, i as usize, LVNI_SELECTED as isize);
        if i < 0 {
            break;
        }
        if let Some(&row) = ui.rows.get(i as usize) {
            out.push(row);
        }
    }
    out
}

fn delete_selected(ui: &mut Ui) {
    let doomed: Vec<(String, String)> = selected_rows(ui)
        .into_iter()
        .map(|i| (ui.entries[i].typed.clone(), ui.entries[i].text.clone()))
        .collect();
    if let Some(h) = ui.history.as_mut() {
        for (typed, text) in doomed {
            let _ = h.forget(&typed, &text);
        }
    }
    reload_history(ui);
}

fn clear_all(ui: &mut Ui) {
    let answer = unsafe {
        MessageBoxW(
            Some(ui.hwnd),
            w!("輕注音會忘記所有你選過的字，排序回到預設。這個動作無法復原。"),
            w!("清除全部選字紀錄？"),
            MB_YESNO | MB_ICONWARNING,
        )
    };
    if answer == IDYES {
        if let Some(h) = ui.history.as_mut() {
            let _ = h.clear();
        }
        reload_history(ui);
    }
}

fn create(hwnd: HWND) {
    UI.with(|cell| {
        let mut ui = cell.borrow_mut();
        ui.hwnd = hwnd;
        ui.dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
        let st = w!("STATIC");
        let bt = w!("BUTTON");
        ui.title = child(hwnd, st, "", 0, 0, 0);
        ui.detail = child(hwnd, st, "", 0, 0, 0);
        ui.primary = child(
            hwnd,
            bt,
            "",
            BS_PUSHBUTTON as u32 | WS_TABSTOP.0,
            0,
            ID_PRIMARY,
        );
        ui.guide = child(
            hwnd,
            st,
            "打法：每個字只打第一個注音，例如 ㄋㄏㄇ → 你好嗎、ㄍㄗㄈㄈ → 工作方法。\n\
             空白鍵／Enter 送出　← → 或 Tab 選字（可以只選前段，剩下的接著選）\n\
             3 4 6 7 聲調（可省略）　Backspace 刪除　Esc 清空\n\
             Shift + , . / 1 ; 打出 ，。？！：　單按 Shift 切換中／英",
            0,
            0,
            0,
        );
        ui.history_label = child(hwnd, st, "選字紀錄", 0, 0, 0);
        ui.count = child(hwnd, st, "", 0, 0, 0);
        ui.search = child(
            hwnd,
            w!("EDIT"),
            "",
            WS_TABSTOP.0 | WS_BORDER.0 | ES_AUTOHSCROLL as u32,
            0,
            ID_SEARCH,
        );
        let cue = wide("搜尋注音或文字");
        send(
            ui.search,
            windows::Win32::UI::Controls::EM_SETCUEBANNER,
            1,
            cue.as_ptr() as isize,
        );
        ui.list = child(
            hwnd,
            WC_LISTVIEWW,
            "",
            LVS_REPORT | LVS_SHOWSELALWAYS | WS_TABSTOP.0 | WS_VSCROLL.0,
            WS_EX_CLIENTEDGE.0,
            ID_LIST,
        );
        send(
            ui.list,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            0,
            (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as isize,
        );
        for (i, name) in ["注音", "選的字", "次數", "最後使用"].iter().enumerate() {
            let mut text = wide(name);
            let column = LVCOLUMNW {
                mask: LVCF_TEXT | LVCF_WIDTH,
                cx: 100,
                pszText: windows_core::PWSTR(text.as_mut_ptr()),
                ..Default::default()
            };
            send(ui.list, LVM_INSERTCOLUMNW, i, &column as *const _ as isize);
        }
        ui.delete = child(
            hwnd,
            bt,
            "刪除所選",
            BS_PUSHBUTTON as u32 | WS_TABSTOP.0,
            0,
            ID_DELETE,
        );
        ui.clear = child(
            hwnd,
            bt,
            "全部清除…",
            BS_PUSHBUTTON as u32 | WS_TABSTOP.0,
            0,
            ID_CLEAR,
        );
        ui.site = child(
            hwnd,
            bt,
            "官網",
            BS_PUSHBUTTON as u32 | WS_TABSTOP.0,
            0,
            ID_SITE,
        );
        ui.issues = child(
            hwnd,
            bt,
            "回報問題",
            BS_PUSHBUTTON as u32 | WS_TABSTOP.0,
            0,
            ID_ISSUES,
        );
        fonts(&mut ui);
        layout(&ui);
        show_status(&mut ui);
        reload_history(&mut ui);
    });
}

fn command(id: i32, code: u32) {
    UI.with(|cell| {
        let mut ui = cell.borrow_mut();
        match (id, code) {
            (ID_PRIMARY, BN_CLICKED) => match ui.status {
                Some(Status::NotEnabled) => {
                    install_layout_or_tip(true);
                    show_status(&mut ui);
                }
                _ => open(w!("ms-settings:regionlanguage")),
            },
            (ID_SEARCH, EN_CHANGE) => fill_list(&mut ui),
            (ID_DELETE, BN_CLICKED) => delete_selected(&mut ui),
            (ID_CLEAR, BN_CLICKED) => clear_all(&mut ui),
            (ID_SITE, BN_CLICKED) => open(w!("https://lightbopomofo.noky.dev")),
            (ID_ISSUES, BN_CLICKED) => open(w!("https://github.com/nvlznn/light-bopomofo/issues")),
            _ => {}
        }
    });
}

fn notify(header: *const NMHDR) {
    if header.is_null() {
        return;
    }
    let code = unsafe { (*header).code };
    UI.with(|cell| {
        let mut ui = cell.borrow_mut();
        if unsafe { (*header).hwndFrom } != ui.list {
            return;
        }
        if code == LVN_COLUMNCLICK {
            let column = unsafe { (*(header as *const NMLISTVIEW)).iSubItem } as usize;
            ui.sort = if ui.sort.0 == column {
                (column, !ui.sort.1)
            } else {
                (column, column <= 1)
            };
            fill_list(&mut ui);
        } else if code == LVN_KEYDOWN {
            let key = unsafe { (*(header as *const NMLVKEYDOWN)).wVKey };
            if key == VK_DELETE.0 {
                delete_selected(&mut ui);
            }
        }
    });
}

extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            create(hwnd);
            LRESULT(0)
        }
        WM_SIZE => {
            UI.with(|cell| {
                if let Ok(ui) = cell.try_borrow() {
                    if ui.hwnd == hwnd {
                        layout(&ui);
                    }
                }
            });
            LRESULT(0)
        }
        WM_DPICHANGED => {
            UI.with(|cell| {
                let mut ui = cell.borrow_mut();
                ui.dpi = (wparam.0 & 0xffff) as u32;
                fonts(&mut ui);
            });
            let suggested = unsafe { &*(lparam.0 as *const RECT) };
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            LRESULT(0)
        }
        WM_GETMINMAXINFO => {
            let dpi = UI
                .with(|cell| cell.try_borrow().map(|u| u.dpi).unwrap_or(96))
                .max(96);
            let info = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
            info.ptMinTrackSize.x = scale(640, dpi);
            info.ptMinTrackSize.y = scale(600, dpi);
            LRESULT(0)
        }
        WM_ACTIVATE => {
            if wparam.0 & 0xffff != 0 {
                UI.with(|cell| {
                    if let Ok(mut ui) = cell.try_borrow_mut() {
                        if ui.hwnd == hwnd {
                            show_status(&mut ui);
                            reload_history(&mut ui);
                        }
                    }
                });
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_COMMAND => {
            command(
                (wparam.0 & 0xffff) as i32,
                ((wparam.0 >> 16) & 0xffff) as u32,
            );
            LRESULT(0)
        }
        WM_NOTIFY => {
            notify(lparam.0 as *const NMHDR);
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--enable") => std::process::exit(if install_layout_or_tip(true) { 0 } else { 1 }),
        Some("--disable") => std::process::exit(if install_layout_or_tip(false) { 0 } else { 1 }),
        _ => {}
    }
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let controls = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES | ICC_STANDARD_CLASSES,
        };
        let _ = InitCommonControlsEx(&controls);

        let instance = GetModuleHandleW(None).unwrap_or_default();
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            // Resource 1 is the icon (app.rc).
            hIcon: LoadIconW(
                Some(instance.into()),
                PCWSTR(std::ptr::without_provenance(1)),
            )
            .unwrap_or_default(),
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
            lpszClassName: w!("LightBopomofoSettings"),
            ..Default::default()
        };
        RegisterClassExW(&class);
        let Ok(hwnd) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("LightBopomofoSettings"),
            w!("輕注音"),
            WINDOW_STYLE(WS_OVERLAPPEDWINDOW.0 | WS_CLIPCHILDREN.0),
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            760,
            680,
            None,
            None,
            Some(instance.into()),
            None,
        ) else {
            return;
        };
        // Size for the monitor's DPI now that the window exists.
        let dpi = GetDpiForWindow(hwnd).max(96);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            scale(760, dpi),
            scale(680, dpi),
            SWP_NOZORDER | windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE,
        );
        let _ = ShowWindow(hwnd, SW_SHOWNORMAL);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if !IsDialogMessageW(hwnd, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        let _ = DestroyWindow(hwnd);
    }
}

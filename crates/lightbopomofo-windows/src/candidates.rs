//! The candidate strip near the caret: up to 9 per page, numbered, the
//! selection highlighted. A topmost tool window that never takes focus,
//! drawn with GDI; follows the light/dark app theme and the monitor's DPI.
//!
//! One window per UI thread (TSF calls arrive on the application's thread).

use std::cell::RefCell;
use std::sync::OnceLock;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, EndPaint, FillRect, GetDC,
    GetMonitorInfoW, GetTextExtentPoint32W, InvalidateRect, MonitorFromRect, ReleaseDC, RoundRect,
    SelectObject, SetBkMode, SetTextColor, TextOutW, CLEARTYPE_QUALITY, DEFAULT_CHARSET, FW_NORMAL,
    HDC, HFONT, HGDIOBJ, MONITORINFO, MONITOR_DEFAULTTONEAREST, OUT_DEFAULT_PRECIS, PAINTSTRUCT,
    PS_SOLID, TRANSPARENT,
};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, RegisterClassExW, SetWindowPos,
    ShowWindow, CS_DROPSHADOW, HWND_TOPMOST, MA_NOACTIVATE, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SW_HIDE, WM_ERASEBKGND, WM_MOUSEACTIVATE, WM_PAINT, WNDCLASSEXW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows_core::{w, PCWSTR};

use crate::globals::{guard, module, wide, wide_nul};

const PER_PAGE: usize = 9;
const CLASS: PCWSTR = w!("LightBopomofoCandidates");

#[derive(Default)]
struct Panel {
    hwnd: Option<HWND>,
    items: Vec<String>,
    selected: usize,
    pages: usize,
    page: usize,
    /// Layout in pixels for the current DPI: (x of each cell, width of each cell).
    cells: Vec<(i32, i32)>,
    size: (i32, i32),
    dpi: u32,
    last_origin: Option<POINT>,
}

thread_local! {
    static PANEL: RefCell<Panel> = RefCell::new(Panel::default());
}

struct Theme {
    background: COLORREF,
    border: COLORREF,
    text: COLORREF,
    index: COLORREF,
    selected: COLORREF,
    selected_text: COLORREF,
}

fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16)
}

fn dark_mode() -> bool {
    let mut value = 1u32;
    let mut size = 4u32;
    let ok = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut size),
        )
    }
    .is_ok();
    ok && value == 0
}

fn theme() -> Theme {
    if dark_mode() {
        Theme {
            background: rgb(0x2B, 0x2B, 0x2B),
            border: rgb(0x45, 0x45, 0x45),
            text: rgb(0xF2, 0xF2, 0xF2),
            index: rgb(0x9A, 0x9A, 0x9A),
            selected: rgb(0x4C, 0xC2, 0xFF),
            selected_text: rgb(0x00, 0x00, 0x00),
        }
    } else {
        Theme {
            background: rgb(0xFF, 0xFF, 0xFF),
            border: rgb(0xD1, 0xD1, 0xD1),
            text: rgb(0x1A, 0x1A, 0x1A),
            index: rgb(0x7A, 0x7A, 0x7A),
            selected: rgb(0x00, 0x67, 0xC0),
            selected_text: rgb(0xFF, 0xFF, 0xFF),
        }
    }
}

fn scale(px: i32, dpi: u32) -> i32 {
    px * dpi as i32 / 96
}

fn font(size: i32, dpi: u32) -> HFONT {
    unsafe {
        CreateFontW(
            -scale(size, dpi),
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            Default::default(),
            CLEARTYPE_QUALITY,
            0,
            w!("Microsoft JhengHei UI"),
        )
    }
}

fn text_width(dc: HDC, text: &str) -> i32 {
    let w = wide(text);
    let mut size = Default::default();
    unsafe {
        let _ = GetTextExtentPoint32W(dc, &w, &mut size);
    }
    size.cx
}

const TEXT_PT: i32 = 18;
const INDEX_PT: i32 = 11;
const PAD: i32 = 6;
const CELL_PAD: i32 = 8;
const HEIGHT: i32 = 36;

fn page_label(p: &Panel) -> Option<String> {
    (p.pages > 1).then(|| format!("{}/{}", p.page + 1, p.pages))
}

fn layout(p: &mut Panel, hwnd: HWND) {
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    p.dpi = dpi;
    unsafe {
        let dc = GetDC(Some(hwnd));
        let big = font(TEXT_PT, dpi);
        let small = font(INDEX_PT, dpi);
        let mut x = scale(PAD, dpi);
        p.cells.clear();
        let start = p.page * PER_PAGE;
        for (i, item) in p.items[start..].iter().take(PER_PAGE).enumerate() {
            let old = SelectObject(dc, HGDIOBJ(small.0));
            let index = text_width(dc, &(i + 1).to_string());
            SelectObject(dc, HGDIOBJ(big.0));
            let word = text_width(dc, item);
            SelectObject(dc, old);
            let width = scale(CELL_PAD * 2 + 3, dpi) + index + word;
            p.cells.push((x, width));
            x += width;
        }
        if let Some(label) = page_label(p) {
            let old = SelectObject(dc, HGDIOBJ(small.0));
            x += text_width(dc, &label) + scale(CELL_PAD * 2, dpi);
            SelectObject(dc, old);
        }
        p.size = (x + scale(PAD, dpi), scale(HEIGHT, dpi));
        let _ = DeleteObject(HGDIOBJ(big.0));
        let _ = DeleteObject(HGDIOBJ(small.0));
        ReleaseDC(Some(hwnd), dc);
    }
}

fn paint(hwnd: HWND) {
    PANEL.with(|panel| {
        let p = panel.borrow();
        let t = theme();
        unsafe {
            let mut ps = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut ps);
            let (w, h) = p.size;
            let dpi = p.dpi.max(96);

            let bg = CreateSolidBrush(t.background);
            FillRect(
                dc,
                &RECT {
                    left: 0,
                    top: 0,
                    right: w,
                    bottom: h,
                },
                bg,
            );
            let _ = DeleteObject(HGDIOBJ(bg.0));
            let pen = CreatePen(PS_SOLID, 1, t.border);
            let old_pen = SelectObject(dc, HGDIOBJ(pen.0));
            let hollow = windows::Win32::Graphics::Gdi::GetStockObject(
                windows::Win32::Graphics::Gdi::NULL_BRUSH,
            );
            let old_brush = SelectObject(dc, hollow);
            let radius = scale(8, dpi);
            let _ = RoundRect(dc, 0, 0, w, h, radius, radius);
            SelectObject(dc, old_brush);
            SelectObject(dc, old_pen);
            let _ = DeleteObject(HGDIOBJ(pen.0));

            SetBkMode(dc, TRANSPARENT);
            let big = font(TEXT_PT, dpi);
            let small = font(INDEX_PT, dpi);
            let start = p.page * PER_PAGE;
            for (i, (item, &(x, cw))) in p.items[start..].iter().zip(&p.cells).enumerate() {
                let selected = start + i == p.selected;
                if selected {
                    let brush = CreateSolidBrush(t.selected);
                    let inset = scale(4, dpi);
                    let cell = RECT {
                        left: x,
                        top: inset,
                        right: x + cw,
                        bottom: h - inset,
                    };
                    let old = SelectObject(dc, HGDIOBJ(brush.0));
                    let pen = CreatePen(PS_SOLID, 1, t.selected);
                    let old_pen = SelectObject(dc, HGDIOBJ(pen.0));
                    let r = scale(6, dpi);
                    let _ = RoundRect(dc, cell.left, cell.top, cell.right, cell.bottom, r, r);
                    SelectObject(dc, old_pen);
                    SelectObject(dc, old);
                    let _ = DeleteObject(HGDIOBJ(pen.0));
                    let _ = DeleteObject(HGDIOBJ(brush.0));
                }
                let mut cx = x + scale(CELL_PAD, dpi);
                let index = wide(&(i + 1).to_string());
                let old = SelectObject(dc, HGDIOBJ(small.0));
                SetTextColor(dc, if selected { t.selected_text } else { t.index });
                let mut size = Default::default();
                let _ = GetTextExtentPoint32W(dc, &index, &mut size);
                let _ = TextOutW(dc, cx, (h - size.cy) / 2, &index);
                cx += size.cx + scale(3, dpi);
                SelectObject(dc, HGDIOBJ(big.0));
                SetTextColor(dc, if selected { t.selected_text } else { t.text });
                let word = wide(item);
                let _ = GetTextExtentPoint32W(dc, &word, &mut size);
                let _ = TextOutW(dc, cx, (h - size.cy) / 2, &word);
                SelectObject(dc, old);
            }
            if let Some(label) = page_label(&p) {
                let x = p.cells.last().map_or(0, |&(x, w)| x + w) + scale(CELL_PAD, dpi);
                let label = wide(&label);
                let old = SelectObject(dc, HGDIOBJ(small.0));
                SetTextColor(dc, t.index);
                let mut size = Default::default();
                let _ = GetTextExtentPoint32W(dc, &label, &mut size);
                let _ = TextOutW(dc, x, (h - size.cy) / 2, &label);
                SelectObject(dc, old);
            }
            let _ = DeleteObject(HGDIOBJ(big.0));
            let _ = DeleteObject(HGDIOBJ(small.0));
            let _ = EndPaint(hwnd, &ps);
        }
    });
}

extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    guard("candidate window", LRESULT(0), || match msg {
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    })
}

fn register_class() -> bool {
    static REGISTERED: OnceLock<bool> = OnceLock::new();
    *REGISTERED.get_or_init(|| {
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_DROPSHADOW,
            lpfnWndProc: Some(window_proc),
            hInstance: module().into(),
            lpszClassName: CLASS,
            ..Default::default()
        };
        unsafe { RegisterClassExW(&class) != 0 }
    })
}

fn window(p: &mut Panel) -> Option<HWND> {
    if let Some(hwnd) = p.hwnd {
        return Some(hwnd);
    }
    if !register_class() {
        return None;
    }
    let title = wide_nul("");
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
            CLASS,
            PCWSTR(title.as_ptr()),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(module().into()),
            None,
        )
    }
    .ok()?;
    unsafe {
        let round = DWMWCP_ROUND;
        // Windows 11 rounds the corners; earlier versions ignore this.
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &round as *const _ as *const _,
            std::mem::size_of_val(&round) as u32,
        );
    }
    p.hwnd = Some(hwnd);
    Some(hwnd)
}

/// Where to put a window of `size` for a caret at `caret` (screen pixels):
/// below the text, or above it when there is no room; inside the monitor.
fn origin(caret: Option<RECT>, size: (i32, i32), last: Option<POINT>) -> POINT {
    let caret = match caret {
        Some(rect) => rect,
        None => {
            if let Some(last) = last {
                return last;
            }
            let mut cursor = POINT::default();
            unsafe {
                let _ = GetCursorPos(&mut cursor);
            }
            RECT {
                left: cursor.x,
                top: cursor.y,
                right: cursor.x,
                bottom: cursor.y + 20,
            }
        }
    };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let work = unsafe {
        let monitor = MonitorFromRect(&caret, MONITOR_DEFAULTTONEAREST);
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            info.rcWork
        } else {
            RECT {
                left: i32::MIN / 2,
                top: i32::MIN / 2,
                right: i32::MAX / 2,
                bottom: i32::MAX / 2,
            }
        }
    };
    let mut y = caret.bottom + 4;
    if y + size.1 > work.bottom {
        y = caret.top - size.1 - 4;
    }
    let x = caret.left.min(work.right - size.0).max(work.left);
    POINT {
        x,
        y: y.max(work.top),
    }
}

/// Show `items` with `selected` highlighted near `caret` (screen pixels).
pub fn show(items: Vec<String>, selected: usize, caret: Option<RECT>) {
    if items.is_empty() {
        return hide();
    }
    PANEL.with(|panel| {
        let mut p = panel.borrow_mut();
        let Some(hwnd) = window(&mut p) else { return };
        p.pages = items.len().div_ceil(PER_PAGE);
        p.page = selected / PER_PAGE;
        p.items = items;
        p.selected = selected;
        layout(&mut p, hwnd);
        let at = origin(caret, p.size, p.last_origin);
        if caret.is_some() {
            p.last_origin = Some(at);
        }
        let (w, h) = p.size;
        drop(p);
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                at.x,
                at.y,
                w,
                h,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            let _ = InvalidateRect(Some(hwnd), None, true);
        }
    });
}

pub fn hide() {
    PANEL.with(|panel| {
        if let Some(hwnd) = panel.borrow().hwnd {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
    });
}

/// Destroy this thread's window (when the input method is deactivated).
pub fn destroy() {
    PANEL.with(|panel| {
        if let Some(hwnd) = panel.borrow_mut().hwnd.take() {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    });
}

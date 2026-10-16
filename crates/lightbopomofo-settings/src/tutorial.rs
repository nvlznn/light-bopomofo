//! The typing tutorial, shown before the app's window on first launch and
//! from its 「打字教學」 button: five pages on a frosted card, practising with
//! the real engine (what it learns stays in memory). Drawn with Direct2D.
//!
//! Mirrors macos/App/OnboardingView.swift (same pages, look and copy).

#[path = "../../lightbopomofo-windows/src/keys.rs"]
#[allow(dead_code)]
mod keys;

use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::time::Instant;

use lightbopomofo_core::composer::PAGE_SIZE;
use lightbopomofo_core::tutorial::{Practice, LESSONS};
use lightbopomofo_core::{dachen, Engine, History, Key, Lexicon};
use windows::Win32::Foundation::{
    COLORREF, D2DERR_RECREATE_TARGET, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_BORDER_MODE_HARD, D2D1_COLOR_F,
    D2D1_COMPOSITE_MODE_SOURCE_OVER, D2D1_GRADIENT_STOP, D2D1_PIXEL_FORMAT, D2D_RECT_F,
    D2D_SIZE_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    CLSID_D2D1GaussianBlur, D2D1CreateFactory, ID2D1Bitmap, ID2D1Brush, ID2D1DeviceContext,
    ID2D1Factory, ID2D1Geometry, ID2D1HwndRenderTarget, ID2D1RenderTarget,
    ID2D1SolidColorBrush, ID2D1StrokeStyle, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_CAP_STYLE_ROUND,
    D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ELLIPSE,
    D2D1_EXTEND_MODE_CLAMP, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_GAMMA_2_2,
    D2D1_GAUSSIANBLUR_PROP_BORDER_MODE, D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_INTERPOLATION_MODE_LINEAR, D2D1_LAYER_OPTIONS_NONE,
    D2D1_LAYER_PARAMETERS, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_LINE_JOIN_ROUND,
    D2D1_PRESENT_OPTIONS_NONE, D2D1_PROPERTY_TYPE_ENUM, D2D1_PROPERTY_TYPE_FLOAT,
    D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
    D2D1_STROKE_STYLE_PROPERTIES, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout,
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_LINE_SPACING_METHOD_UNIFORM, DWRITE_TEXT_METRICS,
};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetMonitorInfoW, InvalidateRect, MonitorFromWindow, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, PAINTSTRUCT,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapPaletteTypeMedianCut, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows::Win32::UI::Input::Ime::{ImmAssociateContext, HIMC};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetKeyState, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW,
    IsWindow, KillTimer, LoadCursorW, LoadIconW, PostQuitMessage, RegisterClassExW,
    SetCursor, SetForegroundWindow, SetTimer, SetWindowPos, ShowWindow, TranslateMessage,
    CW_USEDEFAULT, HTCLIENT, IDC_ARROW, IDC_HAND, MINMAXINFO, MSG, SWP_NOACTIVATE,
    SWP_NOZORDER, SW_SHOWNORMAL, WINDOW_EX_STYLE, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND,
    WM_GETMINMAXINFO, WM_KEYDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_QUIT, WM_SETCURSOR,
    WM_SIZE, WM_TIMER, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};
use windows_core::{w, Interface, PCWSTR};
use windows_numerics::{Matrix3x2, Vector2};

use crate::app::{self, Status};
use crate::update;
use keys::{vk, Modifiers};

const CARD_W: f32 = 440.0;
const CARD_H: f32 = 620.0;
const CARD_RADIUS: f32 = 38.0;
const PAD_X: f32 = 30.0;
const INNER_W: f32 = CARD_W - 2.0 * PAD_X;
/// The lessons, then the page about switching to 輕注音.
const STEPS: usize = LESSONS.len() + 1;
const FONT: PCWSTR = w!("Microsoft JhengHei UI");

const CREAM: (f32, f32, f32) = (0.97, 0.91, 0.74);
/// The sky at the top of the picture: the title bar, and the backdrop if the
/// picture cannot be shown.
const SKY: (f32, f32, f32) = (0.47, 0.70, 0.82);
const WHITE: (f32, f32, f32) = (1.0, 1.0, 1.0);
const BLACK: (f32, f32, f32) = (0.0, 0.0, 0.0);

/// The backdrop: the sky over a hill (shared with macOS).
const PICTURE: &[u8] = include_bytes!("../../../assets/tutorial-background.jpg");
/// Where the window sits along the picture's height when it is cropped.
const FOCUS: f32 = 0.6;

/// Title, tilted key by the title, and the line under it, per lesson.
const COPY: [(&str, &str, &str); 5] = [
    ("每個字\n只打第一個注音", "ㄋㄏㄇ", "打 ㄋㄏㄇ，按 Enter 送出「你好嗎」。"),
    ("按 Tab 或 → 選字", "Tab / →", "打 ㄇㄊ，按 Tab 或 → 選「明天」。Shift + Tab 或 ← 往回。"),
    ("找不到？\n按 ] 翻頁", "[ ]", "打 ㄏㄏ，到下一頁找「還好」。[ 回上一頁。"),
    ("整句不對？\n分段選", "分段", "打 ㄇㄇㄒㄒㄋ，先選「媽媽」，再選「謝謝你」。"),
    ("輕注音\n記住你的選擇", "記住了", "再打一次 ㄇㄇㄒㄒㄋ，「媽媽謝謝你」已經排在第一個。"),
];

const TIMER: usize = 1;

fn color((r, g, b): (f32, f32, f32), a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    }
}

fn rounded(r: D2D_RECT_F, radius: f32) -> D2D1_ROUNDED_RECT {
    D2D1_ROUNDED_RECT {
        rect: r,
        radiusX: radius,
        radiusY: radius,
    }
}

fn contains(r: &D2D_RECT_F, (x, y): (f32, f32)) -> bool {
    x >= r.left && x <= r.right && y >= r.top && y <= r.bottom
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// A value that glides to its target.
#[derive(Default)]
struct Smooth {
    value: f32,
    target: f32,
}

impl Smooth {
    fn set(&mut self, target: f32) {
        self.target = target;
    }

    fn snap(&mut self, target: f32) {
        self.value = target;
        self.target = target;
    }

    /// Advances by `dt` seconds; returns whether it is still moving.
    fn tick(&mut self, dt: f32, speed: f32) -> bool {
        self.value += (self.target - self.value) * (1.0 - (-dt * speed).exp());
        if (self.target - self.value).abs() < 0.05 {
            self.value = self.target;
        }
        self.value != self.target
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Hit {
    Back,
    Close,
    Continue,
    Enable,
}

/// The 大千 key that types `symbol`, as printed on the keycap.
fn key_for(symbol: char) -> Option<char> {
    "1qaz2wsxedcrfv5tgbyhnujm8ik,9ol.0p;/-7634"
        .chars()
        .find(|&k| dachen::symbol(k) == Some(symbol))
        .map(|k| k.to_ascii_uppercase())
}

fn keycap_labels(key: &Key) -> (String, Option<String>) {
    match key {
        Key::Symbol(s) => (s.to_string(), key_for(*s).map(String::from)),
        Key::Next => ("Tab".into(), Some("或 →".into())),
        Key::Previous => ("⇧Tab".into(), Some("或 ←".into())),
        Key::PageNext => ("]".into(), None),
        Key::PagePrevious => ("[".into(), None),
        Key::Commit => ("⏎".into(), Some("Enter".into())),
        Key::Escape => ("Esc".into(), None),
        _ => (String::new(), None),
    }
}

// ---------------------------------------------------------------------------
// Drawing resources.
// ---------------------------------------------------------------------------

struct Target {
    rt: ID2D1HwndRenderTarget,
    brush: ID2D1SolidColorBrush,
    round: ID2D1StrokeStyle,
    /// The picture, decoded for this target.
    picture: Option<ID2D1Bitmap>,
    /// The scene with the card's shadow, and the scene blurred for the glass.
    backdrop: Option<(ID2D1Bitmap, ID2D1Bitmap)>,
}

/// Decodes the backdrop picture (JPEG, built into the exe) for `rt`.
fn decode_picture(rt: &ID2D1RenderTarget) -> windows_core::Result<ID2D1Bitmap> {
    unsafe {
        let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream = wic.CreateStream()?;
        stream.InitializeFromMemory(PICTURE)?;
        let decoder = wic.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)?;
        let frame = decoder.GetFrame(0)?;
        let converter = wic.CreateFormatConverter()?;
        converter.Initialize(
            &frame,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeMedianCut,
        )?;
        rt.CreateBitmapFromWicBitmap(&converter, None)
    }
}

struct Gfx {
    factory: ID2D1Factory,
    dwrite: IDWriteFactory,
    formats: HashMap<(u32, i32), IDWriteTextFormat>,
    target: Option<Target>,
}

impl Gfx {
    fn new() -> windows_core::Result<Self> {
        unsafe {
            Ok(Gfx {
                factory: D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?,
                dwrite: DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?,
                formats: HashMap::new(),
                target: None,
            })
        }
    }

    fn format(&mut self, size: f32, weight: DWRITE_FONT_WEIGHT) -> Option<IDWriteTextFormat> {
        let key = ((size * 10.0) as u32, weight.0);
        if let Some(f) = self.formats.get(&key) {
            return Some(f.clone());
        }
        let format = unsafe {
            self.dwrite.CreateTextFormat(
                FONT,
                None,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!("zh-tw"),
            )
        }
        .ok()?;
        self.formats.insert(key, format.clone());
        Some(format)
    }

    fn layout(
        &mut self,
        text: &str,
        size: f32,
        weight: DWRITE_FONT_WEIGHT,
        max_width: f32,
    ) -> Option<(IDWriteTextLayout, f32, f32)> {
        let format = self.format(size, weight)?;
        let text: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            let layout = self
                .dwrite
                .CreateTextLayout(&text, &format, max_width, 10_000.0)
                .ok()?;
            let mut m = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut m).ok()?;
            Some((layout, m.widthIncludingTrailingWhitespace, m.height))
        }
    }

    fn measure(&mut self, text: &str, size: f32, weight: DWRITE_FONT_WEIGHT) -> (f32, f32) {
        self.layout(text, size, weight, 10_000.0)
            .map_or((0.0, 0.0), |(_, w, h)| (w, h))
    }
}

/// Draws in the current transform with one brush, fading by `alpha`.
struct Painter<'a> {
    gfx: &'a mut Gfx,
    rt: ID2D1RenderTarget,
    brush: ID2D1SolidColorBrush,
    round: ID2D1StrokeStyle,
    alpha: f32,
}

impl Painter<'_> {
    fn paint(&self, rgb: (f32, f32, f32), a: f32) -> &ID2D1SolidColorBrush {
        unsafe { self.brush.SetColor(&color(rgb, a * self.alpha)) };
        &self.brush
    }

    fn fill(&self, r: D2D_RECT_F, radius: f32, rgb: (f32, f32, f32), a: f32) {
        unsafe { self.rt.FillRoundedRectangle(&rounded(r, radius), self.paint(rgb, a)) };
    }

    fn stroke(&self, r: D2D_RECT_F, radius: f32, rgb: (f32, f32, f32), a: f32) {
        // Inset by half the line so it sits inside, like SwiftUI's strokeBorder.
        let r = D2D_RECT_F {
            left: r.left + 0.5,
            top: r.top + 0.5,
            right: r.right - 0.5,
            bottom: r.bottom - 0.5,
        };
        unsafe {
            self.rt
                .DrawRoundedRectangle(&rounded(r, radius - 0.5), self.paint(rgb, a), 1.0, None)
        };
    }

    fn line(&self, a: (f32, f32), b: (f32, f32), rgb: (f32, f32, f32), alpha: f32, width: f32) {
        unsafe {
            self.rt.DrawLine(
                Vector2::new(a.0, a.1),
                Vector2::new(b.0, b.1),
                self.paint(rgb, alpha),
                width,
                &self.round,
            )
        };
    }

    /// Draws `text` with its top-left at (x, y); returns its size.
    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        text: &str,
        size: f32,
        weight: DWRITE_FONT_WEIGHT,
        rgb: (f32, f32, f32),
        a: f32,
        x: f32,
        y: f32,
        max_width: f32,
    ) -> (f32, f32) {
        let Some((layout, w, h)) = self.gfx.layout(text, size, weight, max_width) else {
            return (0.0, 0.0);
        };
        unsafe {
            self.rt.DrawTextLayout(
                Vector2::new(x, y),
                &layout,
                self.paint(rgb, a),
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            )
        };
        (w, h)
    }

    fn measure(&mut self, text: &str, size: f32, weight: DWRITE_FONT_WEIGHT) -> (f32, f32) {
        self.gfx.measure(text, size, weight)
    }

    fn transform(&self) -> Matrix3x2 {
        let mut m = Matrix3x2::identity();
        unsafe { self.rt.GetTransform(&mut m) };
        m
    }

    fn set_transform(&self, m: &Matrix3x2) {
        unsafe { self.rt.SetTransform(m) };
    }
}

// ---------------------------------------------------------------------------
// State.
// ---------------------------------------------------------------------------

struct Tutor {
    hwnd: HWND,
    owner: Option<HWND>,
    dpi: u32,
    gfx: Gfx,
    step: usize,
    /// +1 going forward, -1 going back (for the transition).
    direction: f32,
    shown_at: Instant,
    engine: Option<Engine>,
    practice: Option<Practice>,
    wrong_at: Option<Instant>,
    status: Status,
    last_frame: Instant,
    pill: Smooth,
    ready: Smooth,
    selection_x: Smooth,
    selection_w: Smooth,
    scroll: Smooth,
    /// The page of candidates shown last, to snap instead of sliding across pages.
    strip_page: Option<usize>,
    /// Card-space rectangles that respond to clicks, from the last frame.
    hits: Vec<(Hit, D2D_RECT_F)>,
    card: Matrix3x2,
    hover: Option<Hit>,
}

thread_local! {
    static TUTOR: RefCell<Option<Tutor>> = const { RefCell::new(None) };
}

fn lexicon_path() -> Option<std::path::PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("lexicon.bin"))
}

fn new_engine() -> Option<Engine> {
    let lexicon = Lexicon::load(lexicon_path()?).ok()?;
    Some(Engine::new(lexicon, History::in_memory()))
}

impl Tutor {
    fn new(hwnd: HWND, owner: Option<HWND>, gfx: Gfx) -> Self {
        let mut tutor = Tutor {
            hwnd,
            owner,
            dpi: unsafe { GetDpiForWindow(hwnd) }.max(96),
            gfx,
            step: 0,
            direction: 1.0,
            shown_at: Instant::now(),
            engine: None,
            practice: None,
            wrong_at: None,
            status: Status::Ready,
            last_frame: Instant::now(),
            pill: Smooth::default(),
            ready: Smooth::default(),
            selection_x: Smooth::default(),
            selection_w: Smooth::default(),
            scroll: Smooth::default(),
            strip_page: None,
            hits: Vec::new(),
            card: Matrix3x2::identity(),
            hover: None,
        };
        tutor.start();
        tutor.pill.snap(tutor.pill_target());
        tutor
    }

    fn is_final(&self) -> bool {
        self.step == STEPS - 1
    }

    fn pill_target(&self) -> f32 {
        (self.step + 1) as f32 / STEPS as f32
    }

    fn go(&mut self, step: usize) {
        if step >= STEPS || step == self.step {
            return;
        }
        self.direction = if step > self.step { 1.0 } else { -1.0 };
        self.step = step;
        self.start();
        self.pill.set(self.pill_target());
        self.invalidate();
    }

    fn start(&mut self) {
        self.shown_at = Instant::now();
        self.wrong_at = None;
        self.strip_page = None;
        if self.step >= LESSONS.len() {
            self.practice = None;
            self.status = app::status();
            return;
        }
        let lesson = LESSONS[self.step];
        let is_last = self.step == LESSONS.len() - 1;
        // Each lesson starts from a fresh memory, except the last, which
        // shows what the one before taught.
        if !is_last || self.engine.is_none() {
            self.engine = new_engine();
        }
        let Some(engine) = self.engine.as_mut() else {
            // No lexicon to practise with: only the last page makes sense.
            self.step = STEPS - 1;
            self.practice = None;
            self.status = app::status();
            return;
        };
        let first = engine.query_str(lesson.symbols).first().map(|c| c.text.clone());
        if is_last && first.as_deref() != Some(lesson.target) {
            engine.learn_str(lesson.symbols, lesson.target); // the lesson before was skipped
        }
        self.practice = Some(Practice::new(lesson, engine));
    }

    fn invalidate(&self) {
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    /// Returns true when the tutorial should close.
    fn key(&mut self, code: u16) -> bool {
        let pressed = |k: u16| unsafe { GetKeyState(k as i32) } < 0;
        let m = Modifiers {
            shift: pressed(vk::SHIFT),
            ctrl: pressed(vk::CONTROL),
            alt: pressed(vk::MENU),
            win: pressed(vk::LWIN) || pressed(vk::RWIN),
        };
        if self.is_final() {
            return code == vk::RETURN && !m.ctrl && !m.alt;
        }
        let (Some(practice), Some(engine)) = (self.practice.as_mut(), self.engine.as_mut()) else {
            return false;
        };
        let Some(key) = keys::translate(code, m, !practice.composer().is_empty()) else {
            return false;
        };
        if practice.is_done() {
            if key == Key::Commit {
                self.go(self.step + 1);
            }
            return false;
        }
        practice.handle(engine, key);
        if practice.went_wrong() && self.wrong_at.is_none() {
            self.wrong_at = Some(Instant::now());
        }
        self.invalidate();
        false
    }

    /// Returns true when the tutorial should close.
    fn click(&mut self, hit: Hit) -> bool {
        match hit {
            Hit::Back => {
                if self.step > 0 {
                    self.go(self.step - 1);
                }
                false
            }
            Hit::Close => true,
            Hit::Continue => {
                if self.is_final() {
                    return true;
                }
                self.go(self.step + 1);
                false
            }
            Hit::Enable => {
                app::install_layout_or_tip(true);
                self.status = app::status();
                self.invalidate();
                false
            }
        }
    }

    fn hit_at(&self, x: i32, y: i32) -> Option<Hit> {
        let scale = self.dpi as f32 / 96.0;
        let (x, y) = (x as f32 / scale, y as f32 / scale);
        // Window DIPs → card space (the card transform is scale + translation).
        let m = &self.card;
        let p = ((x - m.M31) / m.M11, (y - m.M32) / m.M22);
        self.hits
            .iter()
            .find(|(_, r)| contains(r, p))
            .map(|(hit, _)| *hit)
    }

    /// The timer: restart after a wrong word, and redraw.
    fn tick(&mut self) {
        if let Some(at) = self.wrong_at {
            if at.elapsed().as_secs_f32() > 1.1 {
                self.wrong_at = None;
                if let Some(p) = self.practice.as_mut() {
                    if p.went_wrong() {
                        p.restart();
                    }
                }
            }
        }
        self.invalidate();
    }

    // -- Drawing ------------------------------------------------------------

    fn client_dips(&self) -> (f32, f32) {
        let mut r = RECT::default();
        unsafe {
            let _ = GetClientRect(self.hwnd, &mut r);
        }
        let scale = self.dpi as f32 / 96.0;
        (r.right as f32 / scale, r.bottom as f32 / scale)
    }

    fn card_transform(&self) -> Matrix3x2 {
        let (w, h) = self.client_dips();
        // Small screens: the whole card shrinks rather than being cut.
        let s = ((h - 40.0) / CARD_H).min((w - 40.0) / CARD_W).clamp(0.5, 1.0);
        Matrix3x2::translation(-CARD_W / 2.0, -CARD_H / 2.0)
            * Matrix3x2::scale(s, s)
            * Matrix3x2::translation((w / 2.0).round(), (h / 2.0).round())
    }

    fn ensure_target(&mut self) -> windows_core::Result<()> {
        if self.gfx.target.is_some() {
            return Ok(());
        }
        let mut r = RECT::default();
        unsafe {
            let _ = GetClientRect(self.hwnd, &mut r);
        }
        let dpi = self.dpi as f32;
        unsafe {
            let rt = self.gfx.factory.CreateHwndRenderTarget(
                &D2D1_RENDER_TARGET_PROPERTIES {
                    r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    dpiX: dpi,
                    dpiY: dpi,
                    usage: D2D1_RENDER_TARGET_USAGE_NONE,
                    ..Default::default()
                },
                &D2D1_HWND_RENDER_TARGET_PROPERTIES {
                    hwnd: self.hwnd,
                    pixelSize: D2D_SIZE_U {
                        width: r.right.max(1) as u32,
                        height: r.bottom.max(1) as u32,
                    },
                    presentOptions: D2D1_PRESENT_OPTIONS_NONE,
                },
            )?;
            rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            let brush = rt.CreateSolidColorBrush(&color(WHITE, 1.0), None)?;
            let round = self.gfx.factory.CreateStrokeStyle(
                &D2D1_STROKE_STYLE_PROPERTIES {
                    startCap: D2D1_CAP_STYLE_ROUND,
                    endCap: D2D1_CAP_STYLE_ROUND,
                    lineJoin: D2D1_LINE_JOIN_ROUND,
                    ..Default::default()
                },
                None,
            )?;
            let picture = rt.cast().ok().and_then(|rt| decode_picture(&rt).ok());
            self.gfx.target = Some(Target {
                rt,
                brush,
                round,
                picture,
                backdrop: None,
            });
        }
        Ok(())
    }

    fn resized(&mut self) {
        let mut r = RECT::default();
        unsafe {
            let _ = GetClientRect(self.hwnd, &mut r);
        }
        if let Some(t) = self.gfx.target.as_mut() {
            unsafe {
                let _ = t.rt.Resize(&D2D_SIZE_U {
                    width: r.right.max(1) as u32,
                    height: r.bottom.max(1) as u32,
                });
            }
            t.backdrop = None;
        }
        self.invalidate();
    }

    fn dpi_changed(&mut self, dpi: u32) {
        self.dpi = dpi.max(96);
        if let Some(t) = self.gfx.target.as_mut() {
            unsafe { t.rt.SetDpi(self.dpi as f32, self.dpi as f32) };
            t.backdrop = None;
        }
    }

    fn paint(&mut self) {
        if self.ensure_target().is_err() {
            return;
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        self.card = self.card_transform();
        let (w, h) = self.client_dips();
        let target = self.gfx.target.as_ref().expect("target");
        let rt: ID2D1RenderTarget = target.rt.cast().expect("render target");
        let brush = target.brush.clone();
        let round = target.round.clone();
        unsafe { rt.BeginDraw() };
        if self.gfx.target.as_ref().is_some_and(|t| t.backdrop.is_none()) {
            let picture = self.gfx.target.as_ref().and_then(|t| t.picture.clone());
            let backdrop = build_backdrop(&rt, picture.as_ref(), (w, h), &self.card).ok();
            if let Some(t) = self.gfx.target.as_mut() {
                t.backdrop = backdrop;
            }
        }
        unsafe { rt.SetTransform(&Matrix3x2::identity()) };
        if let Some((scene, glass)) = self.gfx.target.as_ref().and_then(|t| t.backdrop.clone()) {
            draw_glass(&self.gfx.factory, &rt, &brush, &scene, &glass, (w, h), &self.card);
        } else {
            unsafe { rt.Clear(Some(&color(SKY, 1.0))) };
        }
        let animating = self.draw_card(&rt, brush, round, dt);
        let result = unsafe { rt.EndDraw(None, None) };
        if let Err(e) = result {
            if e.code() == D2DERR_RECREATE_TARGET {
                self.gfx.target = None;
                self.invalidate();
            }
        }
        // Fast while something moves; otherwise only the caret blinks.
        unsafe {
            SetTimer(Some(self.hwnd), TIMER, if animating { 16 } else { 275 }, None);
        }
    }

    /// The card's contents, in card space. Returns whether anything moves.
    fn draw_card(
        &mut self,
        rt: &ID2D1RenderTarget,
        brush: ID2D1SolidColorBrush,
        round: ID2D1StrokeStyle,
        dt: f32,
    ) -> bool {
        let card = self.card;
        let mut animating = self.pill.tick(dt, 14.0);
        let ready = self.practice.as_ref().is_none_or(|p| p.is_done());
        self.ready.set(if ready { 1.0 } else { 0.0 });
        animating |= self.ready.tick(dt, 12.0);
        let since = self.shown_at.elapsed().as_secs_f32();
        let appear = ease_out(since / 0.38);
        animating |= since < 0.38;

        let is_final = self.is_final();
        let mut hits = Vec::new();
        let mut p = Painter {
            gfx: &mut self.gfx,
            rt: rt.clone(),
            brush,
            round,
            alpha: 1.0,
        };
        p.set_transform(&card);

        // Header: back, progress, close.
        if self.step > 0 {
            let hot = self.hover == Some(Hit::Back);
            let a = if hot { 1.0 } else { 0.9 };
            p.line((38.0, 41.0), (53.0, 41.0), WHITE, a, 2.0);
            p.line((38.0, 41.0), (44.0, 35.0), WHITE, a, 2.0);
            p.line((38.0, 41.0), (44.0, 47.0), WHITE, a, 2.0);
            hits.push((Hit::Back, rect(26.0, 22.0, 38.0, 38.0)));
        }
        let a = if self.hover == Some(Hit::Close) { 1.0 } else { 0.9 };
        p.line((388.0, 35.0), (400.0, 47.0), WHITE, a, 2.0);
        p.line((400.0, 35.0), (388.0, 47.0), WHITE, a, 2.0);
        hits.push((Hit::Close, rect(376.0, 22.0, 38.0, 38.0)));
        draw_progress(&mut p, self.step, self.pill.value);

        // The page, sliding in.
        p.alpha = appear;
        p.set_transform(&(Matrix3x2::translation(36.0 * self.direction * (1.0 - appear), 0.0) * card));
        if let Some(practice) = self.practice.as_ref() {
            let (title, chip, subtitle) = COPY[self.step];
            let mut y = draw_heading(&mut p, title, chip, subtitle);
            let shake = self.wrong_at.map_or(0.0, |at| {
                let t = (at.elapsed().as_secs_f32() / 0.45).min(1.0);
                8.0 * (t * std::f32::consts::PI * 4.0).sin() * (1.0 - t)
            });
            animating |= shake != 0.0 || self.wrong_at.is_some();
            y += 28.0;
            draw_field(&mut p, practice, y, shake);
            y += 74.0 + 12.0;
            animating |= draw_strip(
                &mut p,
                practice,
                y,
                dt,
                [&mut self.selection_x, &mut self.selection_w, &mut self.scroll],
                &mut self.strip_page,
            );
            y += 42.0 + 22.0;
            draw_keys(&mut p, practice, y);
        } else {
            let y = draw_final(&mut p, self.status, self.hover == Some(Hit::Enable));
            if let Some(button) = y {
                hits.push((Hit::Enable, button));
            }
        }

        // Continue.
        p.alpha = 1.0;
        p.set_transform(&card);
        let button = rect(PAD_X, CARD_H - 30.0 - 54.0, INNER_W, 54.0);
        let r = self.ready.value;
        let hot = if self.hover == Some(Hit::Continue) { 0.04 } else { 0.0 };
        p.fill(button, 27.0, WHITE, lerp(0.08 + hot, 1.0, r));
        p.stroke(button, 27.0, WHITE, 0.14 * (1.0 - r));
        let label = if is_final { "開始使用" } else { "繼續" };
        let (lw, lh) = p.measure(label, 16.0, DWRITE_FONT_WEIGHT_SEMI_BOLD);
        let ink = (lerp(1.0, 0.0, r), lerp(1.0, 0.0, r), lerp(1.0, 0.0, r));
        p.text(
            label,
            16.0,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            ink,
            lerp(0.55, 1.0, r),
            PAD_X + (INNER_W - lw) / 2.0,
            button.top + (54.0 - lh) / 2.0,
            INNER_W,
        );
        hits.push((Hit::Continue, button));
        self.hits = hits;
        animating
    }
}

// ---------------------------------------------------------------------------
// The backdrop and the glass.
// ---------------------------------------------------------------------------

fn gradient_stops(
    rt: &ID2D1RenderTarget,
    stops: &[(f32, D2D1_COLOR_F)],
) -> windows_core::Result<windows::Win32::Graphics::Direct2D::ID2D1GradientStopCollection> {
    let stops: Vec<D2D1_GRADIENT_STOP> = stops
        .iter()
        .map(|&(position, color)| D2D1_GRADIENT_STOP { position, color })
        .collect();
    unsafe { rt.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP) }
}

fn linear(
    rt: &ID2D1RenderTarget,
    from: (f32, f32),
    to: (f32, f32),
    stops: &[(f32, D2D1_COLOR_F)],
) -> windows_core::Result<ID2D1Brush> {
    let stops = gradient_stops(rt, stops)?;
    let brush = unsafe {
        rt.CreateLinearGradientBrush(
            &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                startPoint: Vector2::new(from.0, from.1),
                endPoint: Vector2::new(to.0, to.1),
            },
            None,
            &stops,
        )?
    };
    brush.cast()
}

/// The picture filling the window: cropped to keep the clouds, the stars and
/// the top of the hill in view.
fn draw_scene(rt: &ID2D1RenderTarget, picture: Option<&ID2D1Bitmap>, (w, h): (f32, f32)) {
    unsafe {
        let Some(picture) = picture else {
            rt.Clear(Some(&color(SKY, 1.0)));
            return;
        };
        let size = picture.GetPixelSize();
        let (pw, ph) = (size.width.max(1) as f32, size.height.max(1) as f32);
        let scale = (w / pw).max(h / ph);
        let (dw, dh) = (pw * scale, ph * scale);
        let x = (w - dw) / 2.0;
        let y = (h - dh) / 2.0 - (dh - h) * (FOCUS - 0.5);
        rt.DrawBitmap(
            picture,
            Some(&rect(x, y, dw, dh)),
            1.0,
            D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
            None,
        );
    }
}

fn blurred(
    rt: &ID2D1RenderTarget,
    input: &ID2D1Bitmap,
    size: D2D_SIZE_F,
    sigma: f32,
) -> windows_core::Result<ID2D1Bitmap> {
    unsafe {
        let dc: ID2D1DeviceContext = rt.cast()?;
        let blur = dc.CreateEffect(&CLSID_D2D1GaussianBlur)?;
        blur.SetInput(0, input, true);
        blur.SetValue(
            D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION.0 as u32,
            D2D1_PROPERTY_TYPE_FLOAT,
            &sigma.to_le_bytes(),
        )?;
        blur.SetValue(
            D2D1_GAUSSIANBLUR_PROP_BORDER_MODE.0 as u32,
            D2D1_PROPERTY_TYPE_ENUM,
            &(D2D1_BORDER_MODE_HARD.0 as u32).to_le_bytes(),
        )?;
        let out = rt.CreateCompatibleRenderTarget(
            Some(&size),
            None,
            None,
            D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
        )?;
        out.BeginDraw();
        let out_dc: ID2D1DeviceContext = out.cast()?;
        out_dc.DrawImage(
            &blur.GetOutput()?,
            None,
            None,
            D2D1_INTERPOLATION_MODE_LINEAR,
            D2D1_COMPOSITE_MODE_SOURCE_OVER,
        );
        out.EndDraw(None, None)?;
        out.GetBitmap()
    }
}

/// Renders the scene once per size: (scene with the card's shadow, the scene
/// blurred for the glass).
fn build_backdrop(
    rt: &ID2D1RenderTarget,
    picture: Option<&ID2D1Bitmap>,
    (w, h): (f32, f32),
    card: &Matrix3x2,
) -> windows_core::Result<(ID2D1Bitmap, ID2D1Bitmap)> {
    let size = D2D_SIZE_F { width: w.max(1.0), height: h.max(1.0) };
    let compatible = || unsafe {
        rt.CreateCompatibleRenderTarget(
            Some(&size),
            None,
            None,
            D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
        )
    };
    let scene_rt = compatible()?;
    unsafe { scene_rt.BeginDraw() };
    draw_scene(&scene_rt.cast()?, picture, (w, h));
    unsafe { scene_rt.EndDraw(None, None)? };
    let scene = unsafe { scene_rt.GetBitmap()? };
    let glass = blurred(rt, &scene, size, 30.0)?;

    // The card's shadow, blurred, under where the card goes.
    let shadow_rt = compatible()?;
    unsafe {
        shadow_rt.BeginDraw();
        shadow_rt.SetTransform(&(Matrix3x2::translation(0.0, 24.0) * card));
        let brush = shadow_rt.CreateSolidColorBrush(&color(BLACK, 0.3), None)?;
        shadow_rt.FillRoundedRectangle(&rounded(rect(0.0, 0.0, CARD_W, CARD_H), CARD_RADIUS), &brush);
        shadow_rt.EndDraw(None, None)?;
    }
    let shadow = blurred(rt, &unsafe { shadow_rt.GetBitmap()? }, size, 22.0)?;
    let final_rt = compatible()?;
    unsafe {
        final_rt.BeginDraw();
        final_rt.DrawBitmap(&scene, None, 1.0, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, None);
        final_rt.DrawBitmap(&shadow, None, 1.0, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, None);
        final_rt.EndDraw(None, None)?;
        Ok((final_rt.GetBitmap()?, glass))
    }
}

/// The backdrop, then the card: blurred backdrop clipped to it, tinted, lit
/// at the edge.
fn draw_glass(
    factory: &ID2D1Factory,
    rt: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    scene: &ID2D1Bitmap,
    glass: &ID2D1Bitmap,
    (w, h): (f32, f32),
    card: &Matrix3x2,
) {
    let all = rect(0.0, 0.0, w, h);
    unsafe {
        rt.DrawBitmap(scene, Some(&all), 1.0, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, None);
        let shape = rounded(rect(0.0, 0.0, CARD_W, CARD_H), CARD_RADIUS);
        let Ok(geometry) = factory.CreateRoundedRectangleGeometry(&shape) else {
            return;
        };
        let mask: Option<ID2D1Geometry> = geometry.cast().ok();
        let params = D2D1_LAYER_PARAMETERS {
            contentBounds: rect(-1e6, -1e6, 2e6, 2e6),
            geometricMask: ManuallyDrop::new(mask),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: *card,
            opacity: 1.0,
            opacityBrush: ManuallyDrop::new(None),
            layerOptions: D2D1_LAYER_OPTIONS_NONE,
        };
        rt.PushLayer(&params, None);
        rt.DrawBitmap(glass, Some(&all), 1.0, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, None);
        brush.SetColor(&color(BLACK, 0.32));
        rt.FillRectangle(&all, brush);
        rt.SetTransform(card);
        if let Ok(sheen) = linear(
            rt,
            (0.0, 0.0),
            (CARD_W, CARD_H),
            &[(0.0, color(WHITE, 0.10)), (0.5, color(WHITE, 0.0)), (1.0, color(WHITE, 0.03))],
        ) {
            rt.FillRectangle(&rect(0.0, 0.0, CARD_W, CARD_H), &sheen);
        }
        rt.PopLayer();
        let _ = ManuallyDrop::into_inner(params.geometricMask);
        if let Ok(edge) = linear(
            rt,
            (0.0, 0.0),
            (CARD_W, CARD_H),
            &[(0.0, color(WHITE, 0.45)), (0.5, color(WHITE, 0.08)), (1.0, color(WHITE, 0.22))],
        ) {
            let r = rounded(rect(0.5, 0.5, CARD_W - 1.0, CARD_H - 1.0), CARD_RADIUS - 0.5);
            rt.DrawRoundedRectangle(&r, &edge, 1.0, None);
        }
    }
}

// ---------------------------------------------------------------------------
// The card's parts (card space: 0…440 × 0…620).
// ---------------------------------------------------------------------------

/// A thin track with the step count riding on it in a white pill.
fn draw_progress(p: &mut Painter, step: usize, fraction: f32) {
    let (x0, x1, y) = (78.0, 362.0, 41.0);
    let width = x1 - x0;
    let pill = 46.0;
    let x = (x0 + width * fraction).clamp(x0 + pill / 2.0, x1 - pill / 2.0);
    p.fill(rect(x0, y - 2.0, width, 4.0), 2.0, WHITE, 0.16);
    if let Ok(fill) = linear(
        &p.rt,
        (x0, 0.0),
        (x, 0.0),
        &[(0.0, color(CREAM, 0.35)), (1.0, color(CREAM, 1.0))],
    ) {
        unsafe {
            p.rt.FillRoundedRectangle(&rounded(rect(x0, y - 2.0, x - x0, 4.0), 2.0), &fill)
        };
    }
    let pill_rect = rect(x - pill / 2.0, y - 12.0, pill, 24.0);
    p.fill(
        D2D_RECT_F { top: pill_rect.top + 2.0, bottom: pill_rect.bottom + 3.0, ..pill_rect },
        12.0,
        BLACK,
        0.12,
    );
    p.fill(pill_rect, 12.0, WHITE, 1.0);
    let label = format!("{}/{}", step + 1, STEPS);
    let (lw, lh) = p.measure(&label, 12.0, DWRITE_FONT_WEIGHT_SEMI_BOLD);
    p.text(
        &label,
        12.0,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        BLACK,
        1.0,
        x - lw / 2.0,
        y - lh / 2.0,
        pill,
    );
}

/// Title (with its tilted glass key) and the line under it; returns the
/// bottom.
fn draw_heading(p: &mut Painter, title: &str, chip: &str, subtitle: &str) -> f32 {
    let top = 26.0 + 30.0 + 34.0;
    let title_h = match p.gfx.layout(title, 31.0, DWRITE_FONT_WEIGHT_BOLD, INNER_W) {
        Some((layout, _, _)) => unsafe {
            let _ = layout.SetLineSpacing(DWRITE_LINE_SPACING_METHOD_UNIFORM, 44.0, 35.0);
            let mut m = DWRITE_TEXT_METRICS::default();
            let h = layout.GetMetrics(&mut m).map_or(44.0, |()| m.height);
            p.rt.DrawTextLayout(
                Vector2::new(PAD_X, top),
                &layout,
                p.paint(CREAM, 1.0),
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
            h
        },
        None => 44.0,
    };
    // The glass key, tilted, at the title's top right.
    let (cw, ch) = p.measure(chip, 17.0, DWRITE_FONT_WEIGHT_SEMI_BOLD);
    let (w, h) = (cw + 32.0, ch + 18.0);
    let x = PAD_X + INNER_W - w - 6.0;
    let y = top - 6.0;
    let base = p.transform();
    p.set_transform(&(Matrix3x2::rotation_around(-9.0, Vector2::new(x + w / 2.0, y + h / 2.0)) * base));
    p.fill(rect(x, y + 6.0, w, h), h / 2.0, BLACK, 0.18);
    if let Ok(fill) = linear(&p.rt, (0.0, y), (0.0, y + h), &[(0.0, color(WHITE, 0.32)), (1.0, color(WHITE, 0.06))]) {
        unsafe { p.rt.FillRoundedRectangle(&rounded(rect(x, y, w, h), h / 2.0), &fill) };
    }
    if let Ok(edge) = linear(&p.rt, (x, y), (x + w, y + h), &[(0.0, color(WHITE, 0.7)), (1.0, color(WHITE, 0.1))]) {
        let r = rounded(rect(x + 0.5, y + 0.5, w - 1.0, h - 1.0), h / 2.0 - 0.5);
        unsafe { p.rt.DrawRoundedRectangle(&r, &edge, 1.0, None) };
    }
    p.text(chip, 17.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, WHITE, 1.0, x + 16.0, y + 9.0, w);
    p.set_transform(&base);

    let (_, sub_h) = p.text(
        subtitle,
        14.5,
        DWRITE_FONT_WEIGHT_NORMAL,
        WHITE,
        0.68,
        PAD_X,
        top + title_h + 12.0,
        INNER_W,
    );
    top + title_h + 12.0 + sub_h
}

/// What has been typed: committed text, then the bopomofo being composed.
fn draw_field(p: &mut Painter, practice: &Practice, y: f32, shake: f32) {
    let done = practice.is_done();
    let field = rect(PAD_X + shake, y, INNER_W, 74.0);
    p.fill(field, 22.0, WHITE, if done { 0.1 } else { 0.06 });
    if done {
        p.stroke(field, 22.0, CREAM, 0.55);
    } else {
        p.stroke(field, 22.0, WHITE, 0.12);
    }
    let (size, weight) = (28.0, DWRITE_FONT_WEIGHT_SEMI_BOLD);
    let mut x = field.left + 20.0;
    let (_, line_h) = p.measure("你", size, weight);
    let ty = y + (74.0 - line_h) / 2.0;
    let committed = practice.committed();
    if !committed.is_empty() {
        x += p.text(committed, size, weight, WHITE, 1.0, x, ty, 400.0).0;
    }
    let composing = practice.composer().composing_text();
    if !composing.is_empty() {
        let (cw, _) = p.text(&composing, size, weight, CREAM, 1.0, x, ty, 400.0);
        p.line((x, ty + line_h - 2.0), (x + cw, ty + line_h - 2.0), CREAM, 0.6, 1.2);
        x += cw;
    }
    if done {
        let (cx, cy) = (field.right - 20.0 - 11.0, y + 37.0);
        unsafe {
            p.rt.FillEllipse(
                &D2D1_ELLIPSE { point: Vector2::new(cx, cy), radiusX: 11.0, radiusY: 11.0 },
                p.paint(CREAM, 1.0),
            )
        };
        let ink = (0.25, 0.18, 0.12);
        p.line((cx - 5.0, cy), (cx - 1.5, cy + 3.5), ink, 1.0, 2.2);
        p.line((cx - 1.5, cy + 3.5), (cx + 5.0, cy - 3.5), ink, 1.0, 2.2);
    } else {
        // The caret blinks.
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
        let on = now.map_or(0, |d| d.as_millis()) / 550 % 2 == 0;
        p.fill(rect(x + 2.0, y + 22.0, 2.0, 30.0), 1.0, CREAM, if on { 1.0 } else { 0.15 });
    }
}

/// The current page of candidates, as in the real candidate window. Returns
/// whether the selection is still moving.
fn draw_strip(
    p: &mut Painter,
    practice: &Practice,
    y: f32,
    dt: f32,
    [sel_x, sel_w, scroll]: [&mut Smooth; 3],
    shown_page: &mut Option<usize>,
) -> bool {
    let composer = practice.composer();
    if composer.is_empty() {
        *shown_page = None;
        return false;
    }
    let candidates = composer.candidates();
    let selected = composer.selected();
    let page = selected / PAGE_SIZE;
    let start = page * PAGE_SIZE;
    let end = (start + PAGE_SIZE).min(candidates.len());
    let pages = candidates.len().div_ceil(PAGE_SIZE);

    let label = (pages > 1).then(|| format!("{}/{}", page + 1, pages));
    let label_w = label
        .as_ref()
        .map_or(0.0, |l| p.measure(l, 11.0, DWRITE_FONT_WEIGHT_SEMI_BOLD).0 + 8.0);
    let view_w = INNER_W - label_w;

    // Lay out the cells.
    let mut cells = Vec::new();
    let mut x = 4.0;
    for (i, c) in candidates[start..end].iter().enumerate() {
        let (nw, _) = p.measure(&(i + 1).to_string(), 11.0, DWRITE_FONT_WEIGHT_SEMI_BOLD);
        let (tw, _) = p.measure(&c.text, 17.0, DWRITE_FONT_WEIGHT_SEMI_BOLD);
        let w = 12.0 + nw + 5.0 + tw + 12.0;
        cells.push((x, w, nw));
        x += w + 2.0;
    }
    let total = x + 2.0;
    let (cell_x, cell_w, _) = cells[selected - start];
    let want_scroll = (cell_x + cell_w / 2.0 - view_w / 2.0).clamp(0.0, (total - view_w).max(0.0));
    if *shown_page != Some(page) {
        sel_x.snap(cell_x);
        sel_w.snap(cell_w);
        scroll.snap(want_scroll);
        *shown_page = Some(page);
    } else {
        sel_x.set(cell_x);
        sel_w.set(cell_w);
        scroll.set(want_scroll);
    }
    let moving = sel_x.tick(dt, 18.0) | sel_w.tick(dt, 18.0) | scroll.tick(dt, 14.0);

    // Clip to the view, fading out at the right edge.
    let view = rect(PAD_X, y, view_w, 42.0);
    let fade = linear(
        &p.rt,
        (view.left, 0.0),
        (view.right, 0.0),
        &[(0.0, color(BLACK, 1.0)), (0.86, color(BLACK, 1.0)), (1.0, color(BLACK, 0.0))],
    )
    .ok();
    let params = D2D1_LAYER_PARAMETERS {
        contentBounds: view,
        geometricMask: ManuallyDrop::new(None),
        maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
        maskTransform: Matrix3x2::identity(),
        opacity: 1.0,
        opacityBrush: ManuallyDrop::new(if total > view_w { fade } else { None }),
        layerOptions: D2D1_LAYER_OPTIONS_NONE,
    };
    unsafe { p.rt.PushLayer(&params, None) };
    let origin = PAD_X - scroll.value;
    p.fill(rect(origin + sel_x.value, y + 2.0, sel_w.value, 38.0), 19.0, WHITE, 1.0);
    for (i, c) in candidates[start..end].iter().enumerate() {
        let (cx, _, nw) = cells[i];
        let is_selected = start + i == selected;
        let ink = if is_selected { BLACK } else { WHITE };
        let (_, th) = p.measure(&c.text, 17.0, DWRITE_FONT_WEIGHT_SEMI_BOLD);
        let ty = y + (42.0 - th) / 2.0;
        p.text(&(i + 1).to_string(), 11.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, ink, 0.5, origin + cx + 12.0, ty + 5.0, 40.0);
        p.text(&c.text, 17.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, ink, 1.0, origin + cx + 12.0 + nw + 5.0, ty, 300.0);
    }
    unsafe { p.rt.PopLayer() };
    let _ = ManuallyDrop::into_inner(params.opacityBrush);
    if let Some(label) = label {
        let (_, lh) = p.measure(&label, 11.0, DWRITE_FONT_WEIGHT_SEMI_BOLD);
        p.text(&label, 11.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, WHITE, 0.5, PAD_X + view_w + 8.0, y + (42.0 - lh) / 2.0, 60.0);
    }
    moving
}

#[derive(Clone, Copy, PartialEq)]
enum Cap {
    Done,
    Next,
    Waiting,
}

/// One keycap, with how many times to press it when more than once;
/// returns its width.
fn draw_keycap(p: &mut Painter, key: &Key, state: Cap, count: usize, x: f32, y: f32) -> f32 {
    let (label, hint) = keycap_labels(key);
    let times = (count > 1).then(|| format!("×{count}"));
    let (tw, _) = times
        .as_ref()
        .map_or((0.0, 0.0), |t| p.measure(t, 11.0, DWRITE_FONT_WEIGHT_BOLD));
    let (label_w, lh) = p.measure(&label, 16.0, DWRITE_FONT_WEIGHT_SEMI_BOLD);
    let lw = label_w + if times.is_some() { tw + 2.0 } else { 0.0 };
    let hint_size = hint.as_ref().map(|h| p.measure(h, 9.5, DWRITE_FONT_WEIGHT_SEMI_BOLD));
    let w = lw.max(hint_size.map_or(0.0, |s| s.0)).max(30.0) + 6.0;
    let (w, h) = (w.max(40.0), 46.0);
    let base = p.transform();
    if state == Cap::Next {
        p.set_transform(&(Matrix3x2::scale_around(1.06, 1.06, Vector2::new(x + w / 2.0, y + h / 2.0)) * base));
        for (grow, a) in [(9.0, 0.04), (6.0, 0.06), (3.0, 0.09)] {
            p.fill(rect(x - grow, y - grow, w + 2.0 * grow, h + 2.0 * grow), 12.0 + grow, WHITE, a);
        }
    }
    let cap = rect(x, y, w, h);
    let fill = match state {
        Cap::Next => 1.0,
        Cap::Done => 0.03,
        Cap::Waiting => 0.09,
    };
    p.fill(cap, 12.0, WHITE, fill);
    if state == Cap::Waiting {
        p.stroke(cap, 12.0, WHITE, 0.16);
    }
    let ink = if state == Cap::Next { BLACK } else { WHITE };
    let a = if state == Cap::Done { 0.35 } else { 1.0 };
    let content_h = lh + hint_size.map_or(0.0, |s| s.1 + 1.0);
    let top = y + (h - content_h) / 2.0;
    p.text(&label, 16.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, ink, a, x + (w - lw) / 2.0, top, w);
    if let Some(times) = times {
        // On the label's baseline, after it.
        let (_, th) = p.measure(&times, 11.0, DWRITE_FONT_WEIGHT_BOLD);
        let tx = x + (w - lw) / 2.0 + label_w + 2.0;
        p.text(&times, 11.0, DWRITE_FONT_WEIGHT_BOLD, ink, a, tx, top + lh - th - 1.0, w);
    }
    if let (Some(hint), Some((hw, _))) = (hint, hint_size) {
        p.text(&hint, 9.5, DWRITE_FONT_WEIGHT_SEMI_BOLD, ink, a * 0.55, x + (w - hw) / 2.0, top + lh + 1.0, w);
    }
    p.set_transform(&base);
    w
}

/// The plan with repeats of a key (Tab Tab Tab) folded into one:
/// (key, where it starts, how many).
fn runs(plan: &[Key]) -> Vec<(&Key, usize, usize)> {
    let mut runs: Vec<(&Key, usize, usize)> = Vec::new();
    for (i, key) in plan.iter().enumerate() {
        match runs.last_mut() {
            Some((last, _, count)) if *last == key && !matches!(key, Key::Symbol(_)) => *count += 1,
            _ => runs.push((key, i, 1)),
        }
    }
    runs
}

/// The keys of the lesson, the next one lit, and a line about where things are.
fn draw_keys(p: &mut Painter, practice: &Practice, y: f32) {
    let progress = practice.progress();
    let mut x = PAD_X;
    for (key, start, count) in runs(practice.plan()) {
        let end = start + count;
        let state = match progress {
            Some(n) if end <= n => Cap::Done,
            Some(n) if start <= n => Cap::Next,
            _ => Cap::Waiting,
        };
        let left = if state == Cap::Done { count } else { end - progress.unwrap_or(start).max(start) };
        x += draw_keycap(p, key, state, left, x, y) + 6.0;
    }
    let line_y = y + 46.0 + 14.0;
    let note = |p: &mut Painter, text: &str, x: f32| {
        let (_, h) = p.measure(text, 13.5, DWRITE_FONT_WEIGHT_SEMI_BOLD);
        p.text(text, 13.5, DWRITE_FONT_WEIGHT_SEMI_BOLD, WHITE, 0.62, x, line_y + (30.0 - h) / 2.0, INNER_W);
    };
    if practice.is_done() {
        note(p, "完成。按 Enter 繼續", PAD_X);
    } else if practice.went_wrong() {
        note(p, &format!("不是「{}」，再試一次", practice.lesson().target), PAD_X);
    } else if progress.is_none() {
        let w = draw_keycap(p, &Key::Escape, Cap::Next, 1, PAD_X, line_y - 8.0);
        note(p, "重來", PAD_X + w + 10.0);
    }
}

/// The last page: switching to 輕注音. Returns the 「加入輸入法」 button, if any.
fn draw_final(p: &mut Painter, status: Status, hot: bool) -> Option<D2D_RECT_F> {
    let subtitle = match status {
        Status::Ready => "按 Windows 鍵 + 空白鍵切換到「輕注音」，任何程式都能這樣打；單按 Shift 切換中／英。",
        Status::NotEnabled => "先把「輕注音」加入輸入法，再按 Windows 鍵 + 空白鍵切換。",
        Status::NotInstalled => "輸入法沒有安裝，請重新執行輕注音的安裝程式。",
    };
    let mut y = draw_heading(p, "開始打字吧", "輕", subtitle) + 28.0;
    for (icon, name, selected) in [("ENG", "英文", false), ("中", "輕注音", true)] {
        let row = rect(PAD_X, y, INNER_W, 60.0);
        if selected {
            p.fill(row, 20.0, BLACK, 0.72);
        } else {
            p.fill(row, 20.0, WHITE, 0.06);
            p.stroke(row, 20.0, WHITE, 0.1);
        }
        let (iw, ih) = p.measure(icon, 12.0, DWRITE_FONT_WEIGHT_BOLD);
        let badge = rect(PAD_X + 20.0, y + 19.0, iw.max(14.0) + 12.0, 22.0);
        p.stroke(badge, 5.0, WHITE, 0.7);
        p.text(icon, 12.0, DWRITE_FONT_WEIGHT_BOLD, WHITE, 1.0, badge.left + (badge.right - badge.left - iw) / 2.0, y + 30.0 - ih / 2.0, 60.0);
        let (_, nh) = p.measure(name, 15.5, DWRITE_FONT_WEIGHT_SEMI_BOLD);
        p.text(name, 15.5, DWRITE_FONT_WEIGHT_SEMI_BOLD, WHITE, 1.0, badge.right + 14.0, y + 30.0 - nh / 2.0, 200.0);
        if selected {
            let (cx, cy) = (row.right - 26.0, y + 30.0);
            p.line((cx - 6.0, cy), (cx - 2.0, cy + 4.0), WHITE, 1.0, 2.0);
            p.line((cx - 2.0, cy + 4.0), (cx + 6.0, cy - 4.0), WHITE, 1.0, 2.0);
        }
        y += 70.0;
    }
    if status != Status::NotEnabled {
        return None;
    }
    let button = rect(PAD_X, y + 8.0, INNER_W, 44.0);
    p.fill(button, 22.0, WHITE, if hot { 0.18 } else { 0.12 });
    p.stroke(button, 22.0, WHITE, 0.2);
    let (lw, lh) = p.measure("加入輸入法", 14.0, DWRITE_FONT_WEIGHT_SEMI_BOLD);
    p.text("加入輸入法", 14.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, WHITE, 1.0, PAD_X + (INNER_W - lw) / 2.0, button.top + (44.0 - lh) / 2.0, INNER_W);
    Some(button)
}

// ---------------------------------------------------------------------------
// The window.
// ---------------------------------------------------------------------------

/// What a message asks of the window once the state is put down.
enum After {
    Nothing,
    Close,
    Move(RECT),
}

fn with_tutor<R>(f: impl FnOnce(&mut Tutor) -> R) -> Option<R> {
    TUTOR.with(|cell| cell.try_borrow_mut().ok()?.as_mut().map(f))
}

fn finish(hwnd: HWND) {
    // Give the owner back its input first, so it — not another app — is
    // activated when this window goes.
    let owner = with_tutor(|t| t.owner).flatten();
    unsafe {
        if let Some(owner) = owner {
            let _ = EnableWindow(owner, true);
            let _ = SetForegroundWindow(owner);
        }
        let _ = DestroyWindow(hwnd);
    }
}

extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let after = match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            unsafe { BeginPaint(hwnd, &mut ps) };
            with_tutor(|t| t.paint());
            unsafe {
                let _ = EndPaint(hwnd, &ps);
            }
            return LRESULT(0);
        }
        WM_ERASEBKGND => return LRESULT(1),
        WM_TIMER => {
            with_tutor(|t| t.tick());
            After::Nothing
        }
        WM_SIZE => {
            with_tutor(|t| t.resized());
            After::Nothing
        }
        WM_DPICHANGED => {
            with_tutor(|t| t.dpi_changed((wparam.0 & 0xffff) as u32));
            After::Move(unsafe { *(lparam.0 as *const RECT) })
        }
        WM_GETMINMAXINFO => {
            let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
            let mut r = RECT { left: 0, top: 0, right: dips(480, dpi), bottom: dips(600, dpi) };
            unsafe {
                let _ = AdjustWindowRectExForDpi(&mut r, WS_OVERLAPPEDWINDOW, false, WINDOW_EX_STYLE(0), dpi);
                let info = &mut *(lparam.0 as *mut MINMAXINFO);
                info.ptMinTrackSize.x = r.right - r.left;
                info.ptMinTrackSize.y = r.bottom - r.top;
            }
            return LRESULT(0);
        }
        WM_KEYDOWN => {
            let close = with_tutor(|t| t.key(wparam.0 as u16)).unwrap_or(false);
            if close {
                After::Close
            } else {
                After::Nothing
            }
        }
        WM_MOUSEMOVE => {
            let (x, y) = point(lparam);
            with_tutor(|t| {
                let hover = t.hit_at(x, y);
                if hover != t.hover {
                    t.hover = hover;
                    t.invalidate();
                }
            });
            After::Nothing
        }
        WM_SETCURSOR if (lparam.0 & 0xffff) as u32 == HTCLIENT => {
            let hand = with_tutor(|t| t.hover.is_some()).unwrap_or(false);
            unsafe {
                let cursor = LoadCursorW(None, if hand { IDC_HAND } else { IDC_ARROW });
                SetCursor(cursor.ok());
            }
            return LRESULT(1);
        }
        WM_LBUTTONUP => {
            let (x, y) = point(lparam);
            let close = with_tutor(|t| t.hit_at(x, y).is_some_and(|hit| t.click(hit))).unwrap_or(false);
            if close {
                After::Close
            } else {
                After::Nothing
            }
        }
        WM_DESTROY => {
            unsafe {
                let _ = KillTimer(Some(hwnd), TIMER);
            }
            TUTOR.with(|cell| {
                if let Ok(mut t) = cell.try_borrow_mut() {
                    *t = None;
                }
            });
            return LRESULT(0);
        }
        _ => return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    };
    match after {
        After::Nothing => {}
        After::Close => finish(hwnd),
        After::Move(r) => unsafe {
            let _ = SetWindowPos(
                hwnd,
                None,
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        },
    }
    LRESULT(0)
}

fn point(lparam: LPARAM) -> (i32, i32) {
    ((lparam.0 & 0xffff) as i16 as i32, ((lparam.0 >> 16) & 0xffff) as i16 as i32)
}

fn dips(px: i32, dpi: u32) -> i32 {
    px * dpi as i32 / 96
}

/// Shows the tutorial and returns when it is closed; `owner` (the app's
/// window, if open) is disabled meanwhile. Marks the tutorial as seen.
pub fn run(owner: Option<HWND>) {
    let Ok(gfx) = Gfx::new() else {
        update::set_tutorial_done(); // no Direct2D: nothing to show
        return;
    };
    unsafe {
        let instance = GetModuleHandleW(None).unwrap_or_default();
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            // Resource 1 is the icon (app.rc).
            hIcon: LoadIconW(Some(instance.into()), PCWSTR(std::ptr::without_provenance(1)))
                .unwrap_or_default(),
            lpszClassName: w!("LightBopomofoTutorial"),
            ..Default::default()
        };
        RegisterClassExW(&class); // fails harmlessly the second time
        let Ok(hwnd) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("LightBopomofoTutorial"),
            w!("輕注音"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            900,
            760,
            owner,
            None,
            Some(instance.into()),
            None,
        ) else {
            return;
        };
        // Keys go to the tutorial, not to an input method.
        ImmAssociateContext(hwnd, HIMC::default());
        // A title bar in the sky's colour, dark text on it (Windows 11;
        // ignored before).
        let to_ref = |(r, g, b): (f32, f32, f32)| {
            COLORREF((r * 255.0) as u32 | ((g * 255.0) as u32) << 8 | ((b * 255.0) as u32) << 16)
        };
        let caption = to_ref(SKY);
        let ink = to_ref((0.08, 0.18, 0.26));
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_CAPTION_COLOR, (&caption as *const COLORREF).cast(), 4);
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, (&caption as *const COLORREF).cast(), 4);
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_TEXT_COLOR, (&ink as *const COLORREF).cast(), 4);

        // 900 × 760 DIPs of client area, within the screen, centred.
        let dpi = GetDpiForWindow(hwnd).max(96);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info);
        let work = info.rcWork;
        let mut r = RECT { left: 0, top: 0, right: dips(900, dpi), bottom: dips(760, dpi) };
        let _ = AdjustWindowRectExForDpi(&mut r, WS_OVERLAPPEDWINDOW, false, WINDOW_EX_STYLE(0), dpi);
        let width = (r.right - r.left).min(work.right - work.left);
        let height = (r.bottom - r.top).min(work.bottom - work.top);
        let _ = SetWindowPos(
            hwnd,
            None,
            work.left + (work.right - work.left - width) / 2,
            work.top + (work.bottom - work.top - height) / 2,
            width,
            height,
            SWP_NOZORDER,
        );

        TUTOR.with(|cell| *cell.borrow_mut() = Some(Tutor::new(hwnd, owner, gfx)));
        if let Some(owner) = owner {
            let _ = EnableWindow(owner, false);
        }
        let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));

        let mut msg = MSG::default();
        while IsWindow(Some(hwnd)).as_bool() {
            if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
                // WM_QUIT: hand it on to the outer loop.
                PostQuitMessage(msg.wParam.0 as i32);
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if msg.message == WM_QUIT {
            finish(hwnd);
        }
        if let Some(owner) = owner {
            let _ = EnableWindow(owner, true);
        }
    }
    update::set_tutorial_done();
}

//! Slint front-end glue: owns the document, translates pointer/keyboard input into
//! document operations and pushes the composite + overlays to the window.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use image::DynamicImage;
use slint::{
    Color, ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel,
};
use studio_core::redact::{RedactStyle, preselect_sensitive, select_by_keyword};
use studio_core::stickers::{self, CANVAS_PRESETS, StickerCategory, StickerStyle};
use studio_core::{
    BatchOptions, BlendMode, C2paAddOptions, C2paManifestDraft, C2paRemovalOptions, ColorLook,
    ComposeLayer, ConvertOptions, FrameCardOptions, GifOptions, PanoramaMode, PanoramaOptions,
    ShapeKind, SupportedFormat, TrimMode,
};

use crate::doc::{Doc, IRect, Selection, Snapshot, Tool, ToolSettings};
use crate::{Args, fonts, shell};

slint::include_modules!();

const PALETTE: [[u8; 3]; 28] = [
    [0, 0, 0],
    [64, 64, 64],
    [128, 128, 128],
    [255, 255, 255],
    [136, 0, 21],
    [237, 28, 36],
    [255, 127, 39],
    [255, 242, 0],
    [34, 177, 76],
    [0, 162, 232],
    [63, 72, 204],
    [163, 73, 164],
    [185, 122, 87],
    [255, 174, 201],
    [255, 201, 14],
    [239, 228, 176],
    [181, 230, 29],
    [153, 217, 234],
    [112, 146, 190],
    [200, 191, 231],
    [229, 57, 53],
    [30, 136, 229],
    [67, 160, 71],
    [251, 192, 45],
    [142, 36, 170],
    [0, 137, 123],
    [255, 112, 67],
    [96, 125, 139],
];

const TOOL_LABELS: [(&str, &str); 20] = [
    ("選取/移動", "Select/move"),
    ("鉛筆", "Pencil"),
    ("筆刷", "Brush"),
    ("螢光筆", "Highlighter"),
    ("噴槍", "Airbrush"),
    ("書法筆", "Calligraphy"),
    ("橡皮擦", "Eraser"),
    ("油漆桶", "Fill"),
    ("滴管", "Colour picker"),
    ("文字", "Text"),
    ("馬賽克筆", "Mosaic brush"),
    ("裁切", "Crop"),
    ("平移", "Pan"),
    ("形狀", "Shapes"),
    ("打碼框", "Redact boxes"),
    ("打碼筆刷", "Redact brush"),
    ("膠帶貼紙筆刷", "Tape sticker brush"),
    ("漸層", "Gradient"),
    ("魔術棒", "Magic wand"),
    ("仿製印章", "Clone stamp"),
];

#[derive(Debug, Clone, Copy)]
enum Drag {
    None,
    Paint {
        last: (f32, f32),
        secondary: bool,
    },
    Select {
        start: (f32, f32),
    },
    Shape {
        start: (f32, f32),
        now: (f32, f32),
    },
    Move {
        last: (f32, f32),
    },
    Resize {
        aspect: f32,
    },
    Pan {
        mouse: (f32, f32),
        pan: (f32, f32),
    },
    RedactBox {
        start: (f32, f32),
        now: (f32, f32),
    },
    BoxMove {
        i: usize,
        last: (f32, f32),
        moved: bool,
    },
    BoxResize {
        i: usize,
    },
    /// Canvas edge handle: 0 = right edge, 1 = bottom edge, 2 = corner.
    CanvasEdge {
        kind: u8,
        now: (f32, f32),
    },
    Gradient {
        start: (f32, f32),
        now: (f32, f32),
        secondary: bool,
    },
    Clone {
        offset: (f32, f32),
        last: (f32, f32),
    },
}

enum Job {
    Detect(
        u64,
        Vec<studio_core::redact::RedactBox>,
        Receiver<Result<Vec<studio_core::redact::RedactBox>, String>>,
    ),
    Batch(Receiver<BatchMsg>),
}

enum BatchMsg {
    Progress(usize, usize, bool),
    Done(String, bool, Option<PathBuf>),
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Settings {
    #[serde(default)]
    lang: Option<String>,
    #[serde(default)]
    keep_metadata: bool,
    #[serde(default)]
    show_grid: bool,
    #[serde(default)]
    toolbar_mode: i32,
    #[serde(default)]
    toolbar_x: Option<f32>,
    #[serde(default)]
    toolbar_y: Option<f32>,
    #[serde(default)]
    recent: Vec<PathBuf>,
    #[serde(default)]
    show_rulers: bool,
    #[serde(default)]
    hide_status: bool,
    #[serde(default)]
    export_format: Option<String>,
    #[serde(default)]
    export_quality: Option<u8>,
    #[serde(default)]
    webp_lossless: Option<bool>,
    #[serde(default)]
    alpha_bg: Option<[u8; 4]>,
    #[serde(default)]
    alpha_bg_mode: i32,
    #[serde(default)]
    last_save_dir: Option<PathBuf>,
    #[serde(default)]
    inspector_collapsed: bool,
}

fn settings_path() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    dirs::config_dir().map(|d| d.join("pixel-studio").join("settings.json"))
}

fn load_settings() -> Settings {
    settings_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub struct State {
    doc: Doc,
    tools: ToolSettings,
    tool: Tool,
    zoom: f32,
    pan: (f32, f32),
    buf: SharedPixelBuffer<Rgba8Pixel>,
    zh: bool,
    view_mode: bool,
    show_grid: bool,
    keep_metadata: bool,
    folder: Vec<PathBuf>,
    folder_idx: Option<usize>,
    drag: Drag,
    gesture: Option<Snapshot>,
    pointer_pos: Option<(f32, f32)>,
    sticker_style: StickerStyle,
    thumbs: HashMap<String, Image>,
    job: Option<Job>,
    batch_job: Option<Job>,
    gif_frames: Vec<PathBuf>,
    c2pa_files: Vec<PathBuf>,
    edit_session: Option<(usize, String)>,
    font_paths: Vec<PathBuf>,
    c2pa_info: studio_core::C2paInfo,
    /// Format/quality were confirmed for quick save of this document.
    quick_ok: bool,
    /// (mode, x, y) of the quick-action toolbar.
    toolbar: (i32, f32, f32),
    recent: Vec<PathBuf>,
    rulers: bool,
    status_bar: bool,
    convert_input: Option<PathBuf>,
    convert_input_has_alpha: Option<bool>,
    last_save_dir: Option<PathBuf>,
    save_dialog_mode: i32,
    alpha_bg: [u8; 4],
    alpha_bg_mode: i32,
    export_format: SupportedFormat,
    export_quality: u8,
    webp_lossless: bool,
    batch_format: SupportedFormat,
    batch_quality: u8,
    batch_webp_lossless: bool,
    batch_alpha_bg: [u8; 4],
    batch_alpha_bg_mode: i32,
    hist_revision: Option<u64>,
    /// Right inspector shrunk to its icon rail (persisted).
    inspector_collapsed: bool,
    /// Focus mode: tool column and inspector hidden (transient, never persisted).
    panels_hidden: bool,
    /// The view was at "fit" when the panel layout changed; re-fit on the next canvas resize.
    refit_pending: bool,
}

type S = Rc<RefCell<State>>;

fn tl(zh: bool, a: &str, b: &str) -> String {
    if zh { a.to_owned() } else { b.to_owned() }
}

fn to_color(c: [u8; 4]) -> Color {
    Color::from_argb_u8(c[3], c[0], c[1], c[2])
}
fn from_color(c: Color) -> [u8; 4] {
    [c.red(), c.green(), c.blue(), c.alpha()]
}

fn detect_zh(settings: &Settings) -> bool {
    if let Ok(l) = std::env::var("PIXEL_STUDIO_LANG") {
        return !l.to_lowercase().starts_with("en");
    }
    if let Some(l) = &settings.lang {
        return l != "en";
    }
    sys_locale::get_locale()
        .map(|l| l.to_lowercase().starts_with("zh"))
        .unwrap_or(false)
}

const FORMATS: &[SupportedFormat] = &SupportedFormat::ALL;

fn format_at(idx: i32) -> SupportedFormat {
    FORMATS[idx.clamp(0, FORMATS.len() as i32 - 1) as usize]
}

fn format_index(fmt: SupportedFormat) -> i32 {
    FORMATS.iter().position(|f| *f == fmt).unwrap_or(0) as i32
}

fn parse_hex_rgba(hex: &str) -> Option<[u8; 4]> {
    let v = hex.trim().trim_start_matches('#');
    if v.len() != 6 && v.len() != 8 {
        return None;
    }
    let p = |i: usize| v.get(i..i + 2).and_then(|x| u8::from_str_radix(x, 16).ok());
    Some([p(0)?, p(2)?, p(4)?, if v.len() == 8 { p(6)? } else { 255 }])
}

fn format_rgba_hex(c: [u8; 4]) -> String {
    if c[3] == 255 {
        format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
    } else {
        format!("#{:02X}{:02X}{:02X}{:02X}", c[0], c[1], c[2], c[3])
    }
}

fn resolve_alpha_bg(mode: i32, color1: [u8; 4], color2: [u8; 4], custom: [u8; 4]) -> [u8; 4] {
    match mode {
        0 => [255, 255, 255, 255],
        1 => [0, 0, 0, 255],
        2 => color1,
        3 => color2,
        _ => custom,
    }
}

fn suggested_save_dir(source_path: Option<&Path>, last_dir: Option<&Path>) -> Option<PathBuf> {
    source_path
        .and_then(Path::parent)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| last_dir.filter(|p| !p.as_os_str().is_empty()))
        .map(Path::to_path_buf)
}

fn blank(w: u32, h: u32) -> DynamicImage {
    DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(w, h, image::Rgba([255; 4])))
}

fn initial_state(settings: &Settings, zh: bool, view_mode: bool) -> State {
    let mut doc = Doc::new(1080, 720);
    doc.font_bytes = fonts::load_system_font_bytes();
    State {
        doc,
        tools: ToolSettings::default(),
        tool: Tool::Brush,
        zoom: 1.0,
        pan: (20.0, 20.0),
        buf: SharedPixelBuffer::new(1, 1),
        zh,
        view_mode,
        show_grid: settings.show_grid,
        keep_metadata: settings.keep_metadata,
        folder: Vec::new(),
        folder_idx: None,
        drag: Drag::None,
        gesture: None,
        pointer_pos: None,
        sticker_style: StickerStyle::default(),
        thumbs: HashMap::new(),
        job: None,
        batch_job: None,
        gif_frames: Vec::new(),
        c2pa_files: Vec::new(),
        edit_session: None,
        font_paths: fonts::list_system_font_paths(),
        c2pa_info: Default::default(),
        quick_ok: false,
        toolbar: (
            settings.toolbar_mode.clamp(0, 2),
            settings.toolbar_x.unwrap_or(12.0),
            settings.toolbar_y.unwrap_or(12.0),
        ),
        recent: settings
            .recent
            .iter()
            .filter(|p| p.exists())
            .take(MAX_RECENT)
            .cloned()
            .collect(),
        rulers: settings.show_rulers,
        status_bar: !settings.hide_status,
        convert_input: None,
        convert_input_has_alpha: None,
        last_save_dir: settings.last_save_dir.clone(),
        save_dialog_mode: 0,
        alpha_bg: settings.alpha_bg.unwrap_or([255, 255, 255, 255]),
        alpha_bg_mode: settings.alpha_bg_mode.clamp(0, 4),
        export_format: settings
            .export_format
            .as_deref()
            .and_then(SupportedFormat::from_extension)
            .unwrap_or(SupportedFormat::Png),
        export_quality: settings.export_quality.unwrap_or(90).clamp(1, 100),
        webp_lossless: settings.webp_lossless.unwrap_or(true),
        batch_format: SupportedFormat::Webp,
        batch_quality: 90,
        batch_webp_lossless: false,
        batch_alpha_bg: [255, 255, 255, 255],
        batch_alpha_bg_mode: 0,
        hist_revision: None,
        inspector_collapsed: settings.inspector_collapsed,
        panels_hidden: false,
        refit_pending: false,
    }
}

pub fn run(args: Args) -> Result<(), anyhow::Error> {
    let ui = AppWindow::new()?;
    let settings = load_settings();
    let zh = detect_zh(&settings);
    let state = Rc::new(RefCell::new(initial_state(&settings, zh, args.view)));
    {
        let s = state.borrow();
        ui.set_tb_mode(s.toolbar.0);
        ui.set_tb_x(s.toolbar.1);
        ui.set_tb_y(s.toolbar.2);
        ui.set_c2pa_action("".into());
        ui.set_show_rulers(s.rulers);
        ui.set_show_status(s.status_bar);
        ui.set_inspector_collapsed(s.inspector_collapsed);
        set_recent_model(&ui, &s);
    }
    ui.set_ui_font(fonts::ui_font_family().into());
    if let Ok(m) = std::env::var("PIXEL_STUDIO_MENU") {
        ui.set_menu_open(m.parse().unwrap_or(-1));
    }
    if let Ok(d) = std::env::var("PIXEL_STUDIO_DIALOG") {
        match d.as_str() {
            "about" => ui.set_about_open(true),
            "keys" => ui.set_keys_open(true),
            "resize" => ui.set_rs_open(true),
            _ => {}
        }
    }
    if let Ok(m) = std::env::var("PIXEL_STUDIO_C2PA_DIALOG") {
        ui.set_c2pa_open(m == "1");
    }

    T::get(&ui).set_zh(zh);
    ui.set_language(if zh { 0 } else { 1 });
    ui.set_version(env!("CARGO_PKG_VERSION").into());
    ui.set_view_mode(args.view);
    ui.set_show_grid(settings.show_grid);
    ui.set_keep_metadata(settings.keep_metadata);
    if zh {
        ui.set_style_text("標題文字".into());
    } else {
        ui.set_style_text("Title text".into());
    }
    ui.set_palette(ModelRc::new(VecModel::from(
        PALETTE
            .iter()
            .map(|c| Color::from_rgb_u8(c[0], c[1], c[2]))
            .collect::<Vec<_>>(),
    )));
    ui.set_format_names(ModelRc::new(VecModel::from(
        FORMATS
            .iter()
            .map(|f| SharedString::from(f.label()))
            .collect::<Vec<_>>(),
    )));
    init_export_ui(&ui, &state.borrow());
    ui.set_rs_w(1080);
    ui.set_rs_h(720);
    set_language_models(&ui, &state.borrow());
    wire(&ui, &state);

    if let Some(file) = args.file {
        open_path(&ui, &state, &file);
    }
    if std::env::var("PIXEL_STUDIO_DIALOG").is_ok_and(|d| d == "quicksave") {
        open_save_modal(&ui, &state, 0);
    }
    if let Ok(t) = std::env::var("PIXEL_STUDIO_TAB").map(|t| t.parse::<i32>().unwrap_or(0)) {
        ui.set_tab(t);
    }
    update_stickers(&ui, &state);

    // Fit once the window has its real size.
    {
        let weak = ui.as_weak();
        let st = state.clone();
        slint::Timer::single_shot(Duration::from_millis(80), move || {
            if let Some(ui) = weak.upgrade() {
                fit(&ui, &st);
                refresh(&ui, &st);
                // CI hook: auto-detect text so screenshots show the redaction overlay.
                if std::env::var_os("PIXEL_STUDIO_AUTODETECT").is_some() {
                    action(&ui, &st, "detect");
                }
                if std::env::var("PIXEL_STUDIO_DIALOG").is_ok_and(|d| d == "props") {
                    action(&ui, &st, "properties");
                }
            }
        });
    }
    let poll = slint::Timer::default();
    {
        let weak = ui.as_weak();
        let st = state.clone();
        poll.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(120),
            move || {
                if let Some(ui) = weak.upgrade() {
                    poll_jobs(&ui, &st);
                }
            },
        );
    }
    // Files dropped onto the window are opened.
    {
        use slint::winit_030::{EventResult, WinitWindowAccessor, winit::event::WindowEvent};
        let weak = ui.as_weak();
        let st = state.clone();
        ui.window().on_winit_window_event(move |_, ev| {
            if let WindowEvent::DroppedFile(p) = ev {
                let p = p.clone();
                let st = st.clone();
                let weak = weak.clone();
                slint::Timer::single_shot(Duration::from_millis(1), move || {
                    if let Some(ui) = weak.upgrade() {
                        open_path(&ui, &st, &p);
                    }
                });
            }
            EventResult::Propagate
        });
    }
    refresh(&ui, &state);
    {
        let st = state.clone();
        ui.window().on_close_requested(move || {
            if confirm_discard(&st) {
                slint::CloseRequestResponse::HideWindow
            } else {
                slint::CloseRequestResponse::KeepWindowShown
            }
        });
    }
    ui.run()?;
    sync_state_from_export_ui(&ui, &mut state.borrow_mut());
    save_settings(&state.borrow());
    Ok(())
}

fn save_settings(s: &State) {
    if let Some(p) = settings_path() {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let st = Settings {
            lang: Some(if s.zh { "zh-TW".into() } else { "en".into() }),
            keep_metadata: s.keep_metadata,
            show_grid: s.show_grid,
            toolbar_mode: s.toolbar.0,
            toolbar_x: Some(s.toolbar.1),
            toolbar_y: Some(s.toolbar.2),
            recent: s.recent.clone(),
            show_rulers: s.rulers,
            hide_status: !s.status_bar,
            export_format: Some(s.export_format.extension().to_owned()),
            export_quality: Some(s.export_quality),
            webp_lossless: Some(s.webp_lossless),
            alpha_bg: Some(s.alpha_bg),
            alpha_bg_mode: s.alpha_bg_mode,
            last_save_dir: s.last_save_dir.clone(),
            inspector_collapsed: s.inspector_collapsed,
        };
        let _ = std::fs::write(p, serde_json::to_string_pretty(&st).unwrap_or_default());
    }
}

const MAX_RECENT: usize = 8;

fn set_recent_model(ui: &AppWindow, s: &State) {
    ui.set_recent_files(ModelRc::new(VecModel::from(
        s.recent
            .iter()
            .map(|p| SharedString::from(p.display().to_string()))
            .collect::<Vec<_>>(),
    )));
}

/// Move `path` to the front of the recent-files list (deduplicated, capped).
fn push_recent(ui: &AppWindow, st: &S, path: &Path) {
    let mut s = st.borrow_mut();
    let p = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    s.recent.retain(|r| r != &p);
    s.recent.insert(0, p);
    s.recent.truncate(MAX_RECENT);
    set_recent_model(ui, &s);
    save_settings(&s);
}

/// Ruler ticks for one axis: (screen offset, label, major) every "nice" step.
fn ruler_ticks(pan: f32, zoom: f32, extent: f32, start: f32) -> Vec<Tick> {
    let steps = [
        1.0f32, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0,
    ];
    let major = steps
        .iter()
        .copied()
        .find(|s| s * zoom >= 64.0)
        .unwrap_or(10000.0);
    let minor = if major >= 5.0 { major / 5.0 } else { major };
    let first = ((start - pan) / zoom / minor).floor() as i64;
    let last = ((extent - pan) / zoom / minor).ceil() as i64;
    let mut out = Vec::new();
    for k in first..=last {
        let v = k as f32 * minor;
        let pos = pan + v * zoom;
        if pos < start || pos > extent || out.len() > 600 {
            continue;
        }
        let is_major = (v / major).fract().abs() < 1e-4;
        out.push(Tick {
            pos,
            label: if is_major {
                format!("{}", v as i64).into()
            } else {
                SharedString::new()
            },
            major: is_major,
        });
    }
    out
}

fn color_look_label(zh: bool, look: ColorLook) -> String {
    if !zh {
        return look.name().to_owned();
    }
    match look {
        ColorLook::Vivid => "鮮豔生動 (Vivid)",
        ColorLook::CinematicTealOrange => "電影青橙 (Teal & Orange)",
        ColorLook::MatteFilm => "霧面底片 (Matte Film)",
        ColorLook::MoodyBw => "深沉黑白 (Moody B&W)",
        ColorLook::WarmGolden => "暖陽金黃 (Warm Golden)",
        ColorLook::CrispCool => "清透冷調 (Crisp Cool)",
        ColorLook::HighKey => "日系高調 (High Key)",
        ColorLook::Cyberpunk => "賽博龐克 (Cyberpunk)",
    }
    .to_owned()
}

fn set_language_models(ui: &AppWindow, s: &State) {
    let zh = s.zh;
    ui.set_tools(ModelRc::new(VecModel::from(
        TOOL_LABELS
            .iter()
            .enumerate()
            .map(|(i, (a, b))| ToolItem {
                zh: (*a).into(),
                en: (*b).into(),
                group: i as i32,
            })
            .collect::<Vec<_>>(),
    )));
    let mut cats = vec![SharedString::from(tl(zh, "全部分類", "All categories"))];
    cats.extend(
        StickerCategory::ALL
            .iter()
            .map(|c| SharedString::from(c.label(zh))),
    );
    ui.set_sticker_categories(ModelRc::new(VecModel::from(cats)));
    ui.set_preset_names(ModelRc::new(VecModel::from(
        CANVAS_PRESETS
            .iter()
            .map(|(n, w, h)| SharedString::from(format!("{n}  {w}×{h}")))
            .collect::<Vec<_>>(),
    )));
    let mut fnames = vec![SharedString::from(tl(zh, "預設字型", "Default font"))];
    fnames.extend(
        s.font_paths
            .iter()
            .map(|p| SharedString::from(fonts::display_font_name(p))),
    );
    ui.set_font_names(ModelRc::new(VecModel::from(fnames)));
    ui.set_blend_mode_names(ModelRc::new(VecModel::from(
        BlendMode::ALL
            .iter()
            .map(|m| SharedString::from(if zh { m.label_zh() } else { m.label_en() }))
            .collect::<Vec<_>>(),
    )));
    ui.set_pro_look_names(ModelRc::new(VecModel::from(
        ColorLook::all()
            .iter()
            .map(|&l| SharedString::from(color_look_label(zh, l)))
            .collect::<Vec<_>>(),
    )));
}

// ------------------------------------------------------------------- rendering

fn maybe_refresh_histogram(ui: &AppWindow, s: &mut State) {
    let rev = s.doc.revision();
    let visible = matches!(ui.get_tab(), 2 | 5) && !s.inspector_collapsed && !s.panels_hidden;
    let initial = ui.get_histogram_img().size().width == 0;
    if !initial && (!visible || s.hist_revision == Some(rev)) {
        return;
    }
    let hist = s.doc.compute_histogram();
    let rgba = studio_core::render_histogram_image(&hist, 320, 96).to_rgba8();
    let buf = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        rgba.as_raw(),
        rgba.width(),
        rgba.height(),
    );
    ui.set_histogram_img(Image::from_rgba8(buf));
    let stats = if s.zh {
        format!(
            "平均亮度 {:.1} · 陰影裁切 {:.1}% · 高光裁切 {:.1}%",
            hist.mean_luma, hist.shadow_clip_pct, hist.highlight_clip_pct
        )
    } else {
        format!(
            "Mean {:.1} · Shadow clip {:.1}% · Highlight clip {:.1}%",
            hist.mean_luma, hist.shadow_clip_pct, hist.highlight_clip_pct
        )
    };
    ui.set_histogram_stats(stats.into());
    s.hist_revision = Some(rev);
}

/// Push dirty pixels + overlays to the window.
fn refresh(ui: &AppWindow, st: &S) {
    let mut guard = st.borrow_mut();
    let s = &mut *guard;
    let (w, h) = (s.doc.width(), s.doc.height());
    if s.doc.resized || s.doc.dirty.is_some() {
        s.hist_revision = None;
    }
    if s.doc.resized || s.buf.width() != w || s.buf.height() != h {
        s.buf = SharedPixelBuffer::new(w, h);
        s.doc.dirty = Some(IRect::full(w, h));
        s.doc.resized = false;
        ui.set_img_w(w as i32);
        ui.set_img_h(h as i32);
    }
    if let Some(r) = s.doc.dirty.take() {
        // Drop the window's reference first so the buffer is updated in place
        // (no copy-on-write of the whole canvas), then recomposite only `r`.
        ui.set_canvas(Image::default());
        let bytes = s.buf.make_mut_bytes();
        s.doc.composite_into(bytes, r);
        ui.set_canvas(Image::from_rgba8(s.buf.clone()));
    }
    sync_view(ui, s);
    maybe_refresh_histogram(ui, s);
}

fn sync_view(ui: &AppWindow, s: &State) {
    ui.set_canvas_cursor(canvas_cursor(
        &s.doc,
        if s.view_mode { Tool::Hand } else { s.tool },
        s.drag,
        s.pan,
        s.zoom,
        s.pointer_pos,
    ));
    ui.set_zoom(s.zoom);
    ui.set_pan_x(s.pan.0);
    ui.set_pan_y(s.pan.1);
    ui.set_pixelated(s.zoom >= 2.0);
    ui.set_can_undo(s.doc.can_undo());
    ui.set_can_redo(s.doc.can_redo());
    ui.set_view_mode(s.view_mode);
    ui.set_grid_on(s.show_grid);
    ui.set_has_layer(s.doc.selected.is_some());
    ui.set_show_edge_handles(s.tool == Tool::Select && !s.view_mode);
    if s.rulers {
        let (cw, ch) = (ui.get_canvas_w(), ui.get_canvas_h());
        ui.set_ruler_x(ModelRc::new(VecModel::from(ruler_ticks(
            s.pan.0, s.zoom, cw, 18.0,
        ))));
        ui.set_ruler_y(ModelRc::new(VecModel::from(ruler_ticks(
            s.pan.1, s.zoom, ch, 18.0,
        ))));
    }
    match s.doc.selection {
        Some(sel) => {
            ui.set_has_selection(true);
            ui.set_sel_x(sel.x);
            ui.set_sel_y(sel.y);
            ui.set_sel_w(sel.w);
            ui.set_sel_h(sel.h);
            ui.set_sel_circle(sel.circle);
        }
        None => ui.set_has_selection(false),
    }
    let path = if s.doc.selection.is_some() && !s.doc.selection_path.is_empty() {
        let mut path = String::new();
        for (i, &(x, y)) in s.doc.selection_path.iter().enumerate() {
            path.push_str(&format!(
                "{} {} {} ",
                if i == 0 { "M" } else { "L" },
                s.pan.0 + x * s.zoom,
                s.pan.1 + y * s.zoom
            ));
        }
        path.push('Z');
        path
    } else {
        String::new()
    };
    ui.set_selection_path(path.into());
    let pv = match s.drag {
        Drag::Shape { start, now }
        | Drag::RedactBox { start, now }
        | Drag::Gradient { start, now, .. } => Some((start, now)),
        Drag::CanvasEdge { now, .. } => Some(((0.0, 0.0), now)),
        _ => None,
    };
    ui.set_has_preview(pv.is_some());
    if let Some((a, b)) = pv {
        ui.set_pv_x(a.0.min(b.0));
        ui.set_pv_y(a.1.min(b.1));
        ui.set_pv_w((a.0 - b.0).abs());
        ui.set_pv_h((a.1 - b.1).abs());
    }
    let pv_line = match s.drag {
        Drag::Gradient { start, now, .. } => format!(
            "M {:.1} {:.1} L {:.1} {:.1}",
            s.pan.0 + start.0 * s.zoom,
            s.pan.1 + start.1 * s.zoom,
            s.pan.0 + now.0 * s.zoom,
            s.pan.1 + now.1 * s.zoom
        ),
        _ => String::new(),
    };
    ui.set_pv_line_path(pv_line.into());
    match s.tools.clone_source {
        Some((cx, cy)) => {
            ui.set_has_clone_source(true);
            ui.set_clone_src_x(cx);
            ui.set_clone_src_y(cy);
        }
        None => ui.set_has_clone_source(false),
    }
    match s.doc.selected.and_then(|i| s.doc.layer_bounds(i)) {
        Some((x, y, w, h)) => {
            ui.set_has_layer_box(true);
            ui.set_lb_x(x);
            ui.set_lb_y(y);
            ui.set_lb_w(w);
            ui.set_lb_h(h);
            ui.set_lb_handle(matches!(
                s.doc.selected.and_then(|i| s.doc.layers.get(i)),
                Some(ComposeLayer::Image(_))
            ));
        }
        None => ui.set_has_layer_box(false),
    }
    ui.set_show_boxes(!s.doc.boxes.is_empty());
    ui.set_focused_box(s.doc.focused_box.map(|i| i as i32).unwrap_or(-1));
    let items: Vec<BoxItem> = s
        .doc
        .boxes
        .iter()
        .map(|b| BoxItem {
            x: b.x as f32,
            y: b.y as f32,
            w: b.width as f32,
            h: b.height as f32,
            selected: b.selected,
            label: b.kind.map(|k| k.label()).unwrap_or("").into(),
        })
        .collect();
    ui.set_boxes(ModelRc::new(VecModel::from(items)));
    let list: Vec<BoxItem> = s
        .doc
        .boxes
        .iter()
        .map(|b| {
            let t = if b.text.is_empty() {
                format!("({}×{})", b.width, b.height)
            } else {
                b.text.clone()
            };
            BoxItem {
                selected: b.selected,
                label: match b.kind {
                    Some(k) => format!("[{}] {t}", k.label()),
                    None => t,
                }
                .into(),
                ..Default::default()
            }
        })
        .collect();
    ui.set_box_list(ModelRc::new(VecModel::from(list)));
    // pixel grid: only the lines that are visible
    let grid_on = s.show_grid && s.zoom >= 8.0;
    ui.set_show_grid_overlay(grid_on);
    if grid_on {
        let (cw, ch) = (ui.get_canvas_w(), ui.get_canvas_h());
        let z = s.zoom;
        let (iw, ih) = (s.doc.width() as f32, s.doc.height() as f32);
        let x0 = ((-s.pan.0) / z).floor().max(0.0);
        let x1 = ((cw - s.pan.0) / z).ceil().min(iw);
        let y0 = ((-s.pan.1) / z).floor().max(0.0);
        let y1 = ((ch - s.pan.1) / z).ceil().min(ih);
        let (top, bot) = ((s.pan.1 + y0 * z).max(0.0), (s.pan.1 + y1 * z).min(ch));
        let (left, right) = ((s.pan.0 + x0 * z).max(0.0), (s.pan.0 + x1 * z).min(cw));
        let mut p = String::new();
        let mut x = x0;
        while x <= x1 {
            let sx = s.pan.0 + x * z;
            p += &format!("M {sx:.1} {top:.1} L {sx:.1} {bot:.1} ");
            x += 1.0;
        }
        let mut y = y0;
        while y <= y1 {
            let sy = s.pan.1 + y * z;
            p += &format!("M {left:.1} {sy:.1} L {right:.1} {sy:.1} ");
            y += 1.0;
        }
        ui.set_grid_path(p.into());
    }
    let rows: Vec<LayerRow> = (0..s.doc.layers.len())
        .rev()
        .map(|i| {
            let l = &s.doc.layers[i];
            LayerRow {
                name: l.name().into(),
                kind: layer_kind(l),
                visible: l.is_visible(),
                selected: s.doc.selected == Some(i),
            }
        })
        .collect();
    ui.set_layers(ModelRc::new(VecModel::from(rows)));
    ui.set_sel_kind(
        s.doc
            .selected
            .and_then(|i| s.doc.layers.get(i))
            .map(layer_kind)
            .unwrap_or(-1),
    );
    let name = s
        .doc
        .path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| tl(s.zh, "未命名", "Untitled"));
    ui.set_title_file(name.clone().into());
    ui.set_doc_info(
        format!(
            "{}  {}×{}  {}{}",
            name,
            s.doc.width(),
            s.doc.height(),
            s.doc.layers.len(),
            tl(s.zh, " 圖層", " layers")
        )
        .into(),
    );
}

fn layer_kind(l: &ComposeLayer) -> i32 {
    match l {
        ComposeLayer::Text(_) => 0,
        ComposeLayer::Image(i) if i.sticker.is_some() => 3,
        ComposeLayer::Image(_) => 1,
        ComposeLayer::Shape(_) => 2,
    }
}

/// Zoom and pan that centre a `w`×`h` image in a `cw`×`ch` canvas with a 20px margin.
fn fit_for(cw: f32, ch: f32, w: f32, h: f32) -> (f32, (f32, f32)) {
    let (cw, ch) = (cw.max(100.0), ch.max(100.0));
    let z = ((cw - 40.0) / w).min((ch - 40.0) / h).clamp(0.02, 16.0);
    (z, ((cw - w * z) / 2.0, (ch - h * z) / 2.0))
}

/// Whether `zoom`/`pan` is (still) the fitted view for this canvas size.
fn is_at_fit(zoom: f32, pan: (f32, f32), cw: f32, ch: f32, w: f32, h: f32) -> bool {
    let (z, p) = fit_for(cw, ch, w, h);
    (zoom - z).abs() <= z * 1e-4 && (pan.0 - p.0).abs() < 0.5 && (pan.1 - p.1).abs() < 0.5
}

fn fit(ui: &AppWindow, st: &S) {
    let mut s = st.borrow_mut();
    let (w, h) = (s.doc.width() as f32, s.doc.height() as f32);
    (s.zoom, s.pan) = fit_for(ui.get_canvas_w(), ui.get_canvas_h(), w, h);
}

/// Apply a side-panel layout. A fitted view is re-fitted once the canvas reports its new size;
/// manually zoomed views are left alone.
fn set_panel_layout(ui: &AppWindow, st: &S, collapsed: bool, hidden: bool) {
    let mut s = st.borrow_mut();
    if s.inspector_collapsed == collapsed && s.panels_hidden == hidden {
        return;
    }
    let (w, h) = (s.doc.width() as f32, s.doc.height() as f32);
    s.refit_pending =
        !s.view_mode && is_at_fit(s.zoom, s.pan, ui.get_canvas_w(), ui.get_canvas_h(), w, h);
    let persist = s.inspector_collapsed != collapsed;
    s.inspector_collapsed = collapsed;
    s.panels_hidden = hidden;
    ui.set_inspector_collapsed(collapsed);
    ui.set_panels_hidden(hidden);
    if persist {
        save_settings(&s);
    }
    if !collapsed && !hidden {
        maybe_refresh_histogram(ui, &mut s);
    }
}

fn zoom_at(st: &S, factor: f32, at: (f32, f32)) {
    let mut s = st.borrow_mut();
    let nz = (s.zoom * factor).clamp(0.02, 64.0);
    let k = nz / s.zoom;
    s.pan = (at.0 - (at.0 - s.pan.0) * k, at.1 - (at.1 - s.pan.1) * k);
    s.zoom = nz;
}

// ------------------------------------------------------------------- file ops

fn open_image_filter(mut d: rfd::FileDialog, zh: bool) -> rfd::FileDialog {
    for (label, exts) in SupportedFormat::open_filter_entries(zh) {
        d = d.add_filter(label, exts);
    }
    d
}

fn save_image_filters(
    mut d: rfd::FileDialog,
    preferred: SupportedFormat,
    zh: bool,
    c2pa_only: bool,
) -> rfd::FileDialog {
    for (label, exts) in SupportedFormat::save_filter_entries(preferred, zh, c2pa_only) {
        d = d.add_filter(label, exts);
    }
    d
}

fn suggested_save_dialog(
    preferred_fmt: SupportedFormat,
    source_path: Option<&Path>,
    last_dir: Option<&Path>,
) -> rfd::FileDialog {
    let name = studio_core::naming::suggested_output_name(source_path, preferred_fmt);
    let mut d = rfd::FileDialog::new().set_file_name(name);
    if let Some(dir) = suggested_save_dir(source_path, last_dir)
        && dir.is_dir()
    {
        d = d.set_directory(dir);
    }
    d
}

#[derive(Clone, Copy)]
enum FormatPanel {
    Export,
    Batch,
}

impl FormatPanel {
    fn format_idx(self, ui: &AppWindow) -> i32 {
        match self {
            Self::Export => ui.get_export_format(),
            Self::Batch => ui.get_batch_format(),
        }
    }

    fn quality_val(self, ui: &AppWindow) -> f32 {
        match self {
            Self::Export => ui.get_quality(),
            Self::Batch => ui.get_batch_quality(),
        }
    }

    fn set_quality_val(self, ui: &AppWindow, q: f32) {
        match self {
            Self::Export => ui.set_quality(q),
            Self::Batch => ui.set_batch_quality(q),
        }
    }

    fn lossless_val(self, ui: &AppWindow) -> bool {
        match self {
            Self::Export => ui.get_webp_lossless(),
            Self::Batch => ui.get_batch_webp_lossless(),
        }
    }

    fn set_lossless_val(self, ui: &AppWindow, v: bool) {
        match self {
            Self::Export => ui.set_webp_lossless(v),
            Self::Batch => ui.set_batch_webp_lossless(v),
        }
    }

    fn alpha_mode_val(self, ui: &AppWindow) -> i32 {
        match self {
            Self::Export => ui.get_alpha_bg_mode(),
            Self::Batch => ui.get_batch_alpha_bg_mode(),
        }
    }

    fn set_alpha_mode_val(self, ui: &AppWindow, mode: i32) {
        match self {
            Self::Export => ui.set_alpha_bg_mode(mode),
            Self::Batch => ui.set_batch_alpha_bg_mode(mode),
        }
    }

    fn alpha_hex_val(self, ui: &AppWindow) -> String {
        match self {
            Self::Export => ui.get_alpha_bg_hex().into(),
            Self::Batch => ui.get_batch_alpha_bg_hex().into(),
        }
    }

    fn set_alpha_hex_val(self, ui: &AppWindow, hex: &str) {
        match self {
            Self::Export => ui.set_alpha_bg_hex(hex.into()),
            Self::Batch => ui.set_batch_alpha_bg_hex(hex.into()),
        }
    }

    fn set_alpha_custom_val(self, ui: &AppWindow, color: Color) {
        match self {
            Self::Export => ui.set_alpha_bg_custom(color),
            Self::Batch => ui.set_batch_alpha_bg_custom(color),
        }
    }

    fn state_mut(
        self,
        s: &mut State,
    ) -> (
        &mut SupportedFormat,
        &mut u8,
        &mut bool,
        &mut i32,
        &mut [u8; 4],
    ) {
        match self {
            Self::Export => (
                &mut s.export_format,
                &mut s.export_quality,
                &mut s.webp_lossless,
                &mut s.alpha_bg_mode,
                &mut s.alpha_bg,
            ),
            Self::Batch => (
                &mut s.batch_format,
                &mut s.batch_quality,
                &mut s.batch_webp_lossless,
                &mut s.batch_alpha_bg_mode,
                &mut s.batch_alpha_bg,
            ),
        }
    }

    fn state_ref(self, s: &State) -> (SupportedFormat, u8, bool, i32, [u8; 4]) {
        match self {
            Self::Export => (
                s.export_format,
                s.export_quality,
                s.webp_lossless,
                s.alpha_bg_mode,
                s.alpha_bg,
            ),
            Self::Batch => (
                s.batch_format,
                s.batch_quality,
                s.batch_webp_lossless,
                s.batch_alpha_bg_mode,
                s.batch_alpha_bg,
            ),
        }
    }

    fn init_ui(self, ui: &AppWindow, s: &State) {
        let (fmt, _q, lossless, alpha_mode, alpha_bg) = self.state_ref(s);
        match self {
            Self::Export => {
                ui.set_export_format(format_index(fmt));
                ui.set_keep_metadata(s.keep_metadata);
            }
            Self::Batch => {
                ui.set_batch_format(format_index(fmt));
            }
        }
        self.set_lossless_val(ui, lossless);
        self.set_alpha_mode_val(ui, alpha_mode);
        let eff_bg = resolve_alpha_bg(alpha_mode, s.tools.color1, s.tools.color2, alpha_bg);
        self.set_alpha_custom_val(ui, to_color(eff_bg));
        self.set_alpha_hex_val(ui, &format_rgba_hex(eff_bg));
        self.sync_ui(ui, s);
    }

    fn sync_state_from_ui(self, ui: &AppWindow, s: &mut State) {
        let new_fmt = format_at(self.format_idx(ui));
        let slider_q = self.quality_val(ui).round().clamp(1.0, 100.0) as u8;
        let ui_lossless = self.lossless_val(ui);
        let ui_alpha_mode = self.alpha_mode_val(ui).clamp(0, 4);
        let ui_hex = self.alpha_hex_val(ui);

        let (stored_fmt, stored_q, stored_lossless, stored_alpha_mode, stored_alpha_bg) =
            self.state_mut(s);
        let prev_fmt = *stored_fmt;
        // Preserve lossy quality when the slider was only showing 100 because WebP lossless locked it.
        let was_locked_lossless = prev_fmt == SupportedFormat::Webp && *stored_lossless;
        if prev_fmt == SupportedFormat::Webp {
            *stored_lossless = ui_lossless;
        }
        if !was_locked_lossless {
            if new_fmt == prev_fmt && new_fmt == SupportedFormat::Webp && slider_q >= 100 {
                *stored_lossless = true;
            } else {
                *stored_q = slider_q;
            }
        }
        *stored_fmt = new_fmt;
        *stored_alpha_mode = ui_alpha_mode;
        if ui_alpha_mode == 4
            && let Some(c) = parse_hex_rgba(&ui_hex)
        {
            *stored_alpha_bg = c;
        }
        if matches!(self, Self::Export) {
            s.keep_metadata = ui.get_keep_metadata();
        }
    }

    fn effective_alpha_bg(self, ui: &AppWindow, s: &State) -> [u8; 4] {
        let mode = self.alpha_mode_val(ui).clamp(0, 4);
        let (_, _, _, _, saved_bg) = self.state_ref(s);
        let custom = if mode == 4 {
            let hex = self.alpha_hex_val(ui);
            parse_hex_rgba(&hex).unwrap_or(saved_bg)
        } else {
            saved_bg
        };
        resolve_alpha_bg(mode, s.tools.color1, s.tools.color2, custom)
    }

    fn sync_ui(self, ui: &AppWindow, s: &State) {
        let fmt = format_at(self.format_idx(ui));
        let (_, stored_q, stored_lossless, _, _) = self.state_ref(s);
        let displayed_q = if fmt == SupportedFormat::Webp && stored_lossless {
            100.0
        } else if fmt == SupportedFormat::Webp {
            stored_q.min(99) as f32
        } else {
            stored_q as f32
        };
        self.set_lossless_val(ui, stored_lossless);
        self.set_quality_val(ui, displayed_q);
        let eff_bg = self.effective_alpha_bg(ui, s);
        self.set_alpha_custom_val(ui, to_color(eff_bg));

        match self {
            Self::Export => {
                ui.set_export_supports_quality(fmt.supports_quality());
                ui.set_export_supports_alpha(fmt.supports_alpha());
                ui.set_export_supports_c2pa(fmt.supports_c2pa());
                ui.set_export_is_webp(fmt == SupportedFormat::Webp);
                ui.set_export_format_hint(format_capability_hint(fmt, s.zh).into());
                let alpha_present = if s.save_dialog_mode == 3 && s.convert_input.is_some() {
                    s.convert_input_has_alpha.unwrap_or(false)
                } else {
                    s.doc.has_alpha_pixels()
                };
                ui.set_export_alpha_present(alpha_present);

                if s.save_dialog_mode == 0 {
                    let locked_path = s
                        .doc
                        .path
                        .as_ref()
                        .filter(|p| SupportedFormat::from_path(p).is_some());
                    ui.set_qs_format_locked(locked_path.is_some());
                    ui.set_qs_prompts_file(locked_path.is_none());
                    let target_text = match locked_path {
                        Some(p) => {
                            format!("{} {}", tl(s.zh, "覆寫：", "Overwrite:"), p.display())
                        }
                        None => format!(
                            "{} {}",
                            tl(s.zh, "新檔名：", "New file:"),
                            studio_core::naming::suggested_output_name(s.doc.path.as_deref(), fmt)
                        ),
                    };
                    ui.set_qs_target(target_text.into());
                    ui.set_export_target_preview(SharedString::new());
                } else {
                    ui.set_qs_format_locked(false);
                    ui.set_qs_prompts_file(true);
                    ui.set_qs_target(SharedString::new());
                    let preview_src = if s.save_dialog_mode == 3 {
                        s.convert_input.as_deref().or(s.doc.path.as_deref())
                    } else {
                        s.doc.path.as_deref()
                    };
                    let preview = studio_core::naming::suggested_output_name(preview_src, fmt);
                    ui.set_export_target_preview(preview.into());
                }
            }
            Self::Batch => {
                ui.set_batch_supports_quality(fmt.supports_quality());
                ui.set_batch_supports_alpha(fmt.supports_alpha());
                ui.set_batch_is_webp(fmt == SupportedFormat::Webp);
                ui.set_batch_format_hint(format_capability_hint(fmt, s.zh).into());
            }
        }
    }

    fn handle_quality_edited(self, ui: &AppWindow, s: &mut State) {
        let fmt = format_at(self.format_idx(ui));
        let v = self.quality_val(ui).round().clamp(1.0, 100.0) as u8;
        let (_, stored_q, stored_lossless, _, _) = self.state_mut(s);
        if fmt == SupportedFormat::Webp {
            if v >= 100 {
                *stored_lossless = true;
                *stored_q = (*stored_q).clamp(1, 99);
                self.set_lossless_val(ui, true);
            } else {
                *stored_lossless = false;
                *stored_q = v;
                self.set_lossless_val(ui, false);
            }
        } else {
            *stored_q = v;
        }
    }

    fn handle_lossless_toggled(self, ui: &AppWindow, s: &mut State) {
        let fmt = format_at(self.format_idx(ui));
        let lossless = self.lossless_val(ui);
        let (_, stored_q, stored_lossless, _, _) = self.state_mut(s);
        *stored_lossless = lossless;
        if fmt == SupportedFormat::Webp {
            if lossless {
                *stored_q = (*stored_q).clamp(1, 99);
                self.set_quality_val(ui, 100.0);
            } else {
                *stored_q = (*stored_q).clamp(1, 99);
                self.set_quality_val(ui, *stored_q as f32);
            }
        }
    }

    fn handle_alpha_changed(self, ui: &AppWindow, s: &mut State) {
        let c1 = s.tools.color1;
        let c2 = s.tools.color2;
        let mode = self.alpha_mode_val(ui).clamp(0, 4);
        let hex = self.alpha_hex_val(ui);
        let (_, _, _, stored_mode, stored_bg) = self.state_mut(s);
        let prev_mode = *stored_mode;
        *stored_mode = mode;
        if mode == 4 {
            if prev_mode == 4 {
                if let Some(c) = parse_hex_rgba(&hex) {
                    *stored_bg = c;
                }
            } else {
                let prev_eff = resolve_alpha_bg(prev_mode, c1, c2, *stored_bg);
                if hex.eq_ignore_ascii_case(&format_rgba_hex(prev_eff)) {
                    self.set_alpha_hex_val(ui, &format_rgba_hex(*stored_bg));
                } else if let Some(c) = parse_hex_rgba(&hex) {
                    *stored_bg = c;
                }
            }
        } else {
            let eff = resolve_alpha_bg(mode, c1, c2, *stored_bg);
            self.set_alpha_hex_val(ui, &format_rgba_hex(eff));
        }
        let eff = self.effective_alpha_bg(ui, s);
        self.set_alpha_custom_val(ui, to_color(eff));
    }

    fn handle_alpha_hex(self, ui: &AppWindow, s: &mut State) {
        let hex = self.alpha_hex_val(ui);
        if let Some(c) = parse_hex_rgba(&hex) {
            let (_, _, _, stored_mode, stored_bg) = self.state_mut(s);
            *stored_bg = c;
            *stored_mode = 4;
            self.set_alpha_mode_val(ui, 4);
            self.set_alpha_custom_val(ui, to_color(c));
        }
    }

    fn options_for_format(
        self,
        ui: &AppWindow,
        s: &State,
        format: SupportedFormat,
    ) -> ConvertOptions {
        let q = self.quality_val(ui).round().clamp(1.0, 100.0) as u8;
        let lossless = self.lossless_val(ui);
        ConvertOptions {
            format,
            quality: studio_core::effective_quality(format, q, lossless),
            background: self.effective_alpha_bg(ui, s),
        }
    }
}

fn init_export_ui(ui: &AppWindow, s: &State) {
    FormatPanel::Export.init_ui(ui, s);
    FormatPanel::Batch.init_ui(ui, s);
}

fn sync_state_from_export_ui(ui: &AppWindow, s: &mut State) {
    FormatPanel::Export.sync_state_from_ui(ui, s);
}

fn sync_export_format_ui(ui: &AppWindow, s: &State) {
    FormatPanel::Export.sync_ui(ui, s);
}

fn format_capability_hint(fmt: SupportedFormat, zh: bool) -> String {
    match fmt {
        SupportedFormat::Jpeg => tl(
            zh,
            "有損壓縮 · 不支援透明 · 支援品質 / EXIF / C2PA",
            "Lossy · No alpha · Supports Quality / EXIF / C2PA",
        ),
        SupportedFormat::Jfif => tl(
            zh,
            "JFIF (JPEG) 有損壓縮 · 不支援透明 · 支援品質 / EXIF / C2PA",
            "JFIF (JPEG) lossy · No alpha · Supports Quality / EXIF / C2PA",
        ),
        SupportedFormat::Png => tl(
            zh,
            "無損壓縮 · 保留透明 · 支援 EXIF / C2PA",
            "Lossless · Alpha preserved · Supports EXIF / C2PA",
        ),
        SupportedFormat::Webp => tl(
            zh,
            "無損 / 有損 · 保留透明 · 支援品質 / EXIF / C2PA",
            "Lossless / lossy · Alpha preserved · Supports Quality / EXIF / C2PA",
        ),
        SupportedFormat::Bmp => tl(
            zh,
            "無壓縮點陣圖 · 不支援透明 · 混合至背景色",
            "Uncompressed bitmap · No alpha · Flattens to background",
        ),
        SupportedFormat::Tiff => tl(
            zh,
            "無損壓縮 · 保留透明 · 支援 C2PA",
            "Lossless · Alpha preserved · Supports C2PA",
        ),
        SupportedFormat::Gif => tl(
            zh,
            "單張 GIF（256 色調色盤）· 支援透明",
            "Single-frame GIF (256 colors) · Alpha supported",
        ),
        SupportedFormat::Ico => tl(
            zh,
            "Windows 圖示（自動等比縮放 ≤ 256×256 px）· 保留透明",
            "Windows icon (auto-scaled ≤ 256×256 px) · Alpha preserved",
        ),
        SupportedFormat::Tga => tl(
            zh,
            "Truevision TGA（RLE 無損）· 保留透明",
            "Truevision TGA (RLE lossless) · Alpha preserved",
        ),
        SupportedFormat::Qoi => tl(
            zh,
            "Quite OK Image（高速無損）· 保留透明",
            "Quite OK Image (fast lossless) · Alpha preserved",
        ),
    }
}

fn sync_batch_format_ui(ui: &AppWindow, s: &State) {
    FormatPanel::Batch.sync_ui(ui, s);
}

fn refresh_convert_source_ui(ui: &AppWindow, s: &mut State) {
    if let Some(path) = s.convert_input.clone() {
        let label = match studio_core::inspect_image(&path) {
            Ok(info) => {
                s.convert_input_has_alpha = Some(matches!(
                    info.color_type.as_str(),
                    "La8" | "Rgba8" | "La16" | "Rgba16" | "Rgba32F"
                ));
                format!(
                    "{} ({} · {}×{} · {:.1} KB)",
                    path.display(),
                    info.format.label(),
                    info.width,
                    info.height,
                    info.file_size as f64 / 1024.0
                )
            }
            Err(_) => {
                s.convert_input_has_alpha = None;
                path.display().to_string()
            }
        };
        ui.set_convert_src_label(label.into());
        ui.set_convert_src_has_file(true);
    } else {
        s.convert_input_has_alpha = None;
        let edited = if s.doc.is_modified() {
            tl(s.zh, "（已編輯，未存檔）", " (edited)")
        } else {
            String::new()
        };
        let label = format!(
            "{} ({}×{} px){}",
            tl(s.zh, "目前畫布影像", "Current canvas image"),
            s.doc.width(),
            s.doc.height(),
            edited
        );
        ui.set_convert_src_label(label.into());
        ui.set_convert_src_has_file(false);
    }
}

fn open_save_modal(ui: &AppWindow, st: &S, mode: i32) {
    {
        let mut s = st.borrow_mut();
        s.save_dialog_mode = mode;
        ui.set_save_dialog_mode(mode);
        let path = s.doc.path.clone();
        if matches!(mode, 0 | 1)
            && let Some(f) = path.as_deref().and_then(SupportedFormat::from_path)
        {
            s.export_format = f;
            ui.set_export_format(format_index(f));
        }
        if mode == 3 {
            s.convert_input = if !s.doc.is_modified()
                && path
                    .as_deref()
                    .and_then(SupportedFormat::from_path)
                    .is_some()
            {
                path.clone()
            } else {
                None
            };
            refresh_convert_source_ui(ui, &mut s);
        }
        sync_export_format_ui(ui, &s);
    }
    ui.set_qs_open(true);
}

fn open_path(ui: &AppWindow, st: &S, path: &Path) {
    if !confirm_discard(st) {
        return;
    }
    let zh = st.borrow().zh;
    if path.extension().is_some_and(|e| e == "pixel-project") {
        let r = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|j| st.borrow_mut().doc.load_project_json(&j));
        let opened = r.is_ok();
        ui.set_status(
            match r {
                Ok(()) => format!(
                    "{} {}",
                    tl(zh, "已開啟專案：", "Project opened:"),
                    path.display()
                ),
                Err(e) => format!("{} {e}", tl(zh, "專案開啟失敗：", "Project open failed:")),
            }
            .into(),
        );
        if opened {
            reset_document_ui(ui, st);
            load_c2pa_state(ui, st, None);
            push_recent(ui, st, path);
            fit(ui, st);
            refresh(ui, st);
        }
        return;
    }
    match studio_core::load_dynamic_image(path) {
        Ok(img) => {
            {
                let mut s = st.borrow_mut();
                s.doc.load(img, Some(path.to_path_buf()));
                s.doc.source_metadata = studio_core::metadata::read_metadata(path);
                if let Some(dir) = path.parent() {
                    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
                        .map(|rd| {
                            rd.filter_map(|e| e.ok().map(|e| e.path()))
                                .filter(|p| SupportedFormat::from_path(p).is_some())
                                .collect()
                        })
                        .unwrap_or_default();
                    files.sort();
                    s.folder_idx = files.iter().position(|p| p == path);
                    s.folder = files;
                }
                let n = s.folder.len();
                let i = s.folder_idx.map(|i| i + 1).unwrap_or(0);
                ui.set_status(format!("{}  ({i}/{n})", path.display()).into());
                ui.set_rs_w(s.doc.width() as i32);
                ui.set_rs_h(s.doc.height() as i32);
            }
            reset_document_ui(ui, st);
            push_recent(ui, st, path);
            load_c2pa_state(ui, st, Some(path));
            fit(ui, st);
            refresh(ui, st);
        }
        Err(e) => ui.set_status(format!("{} {e}", tl(zh, "無法開啟：", "Cannot open:")).into()),
    }
}

fn format_options(ui: &AppWindow, s: &State, format: SupportedFormat) -> ConvertOptions {
    FormatPanel::Export.options_for_format(ui, s, format)
}

fn batch_convert_options(ui: &AppWindow, s: &State) -> ConvertOptions {
    FormatPanel::Batch.options_for_format(ui, s, format_at(ui.get_batch_format()))
}

fn copy_to_staged(src: &Path, staged: &Path) -> anyhow::Result<()> {
    let mut reader = std::fs::File::open(src)?;
    let mut writer = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(staged)?;
    std::io::copy(&mut reader, &mut writer)?;
    Ok(())
}

fn save_to(
    ui: &AppWindow,
    st: &S,
    out: &Path,
    explicit_format: SupportedFormat,
    update_doc: bool,
) -> bool {
    let (img, source_meta, keep, zh, opts, apply_c2pa, skip_c2pa_note, draft) = {
        let mut s = st.borrow_mut();
        sync_state_from_export_ui(ui, &mut s);
        let opts = format_options(ui, &s, explicit_format);
        let c2pa_req = ui.get_c2pa_on_save();
        (
            s.doc.flattened(),
            s.doc.source_metadata.clone(),
            ui.get_keep_metadata(),
            s.zh,
            opts,
            c2pa_req && explicit_format.supports_c2pa(),
            c2pa_req && !explicit_format.supports_c2pa(),
            c2pa_draft(ui),
        )
    };
    let keep_meta = keep && explicit_format.supports_exif_icc();
    let r = if apply_c2pa {
        save_with_c2pa(
            &img,
            out,
            opts,
            &draft,
            source_meta.as_ref().filter(|_| keep_meta),
        )
    } else if keep_meta && let Some(meta) = &source_meta {
        studio_core::atomic_file::write_with(out, |staged| -> anyhow::Result<()> {
            let ext = out.extension().and_then(|e| e.to_str()).unwrap_or("tmp");
            let tmp = staged.with_extension(format!("meta.{ext}"));
            let inner = (|| -> anyhow::Result<()> {
                studio_core::save_dynamic_image(&img, &tmp, opts)?;
                studio_core::metadata::apply_metadata(meta, &tmp)?;
                copy_to_staged(&tmp, staged)
            })();
            let _ = std::fs::remove_file(tmp);
            inner
        })
    } else {
        studio_core::save_dynamic_image(&img, out, opts)
            .map(|_| ())
            .map_err(anyhow::Error::from)
    };
    match r {
        Ok(()) => {
            let size_kb = std::fs::metadata(out)
                .map(|m| m.len() as f64 / 1024.0)
                .unwrap_or(0.0);
            let mut msg = format!(
                "{} {} ({} · {}×{}, {:.1} KB)",
                if update_doc {
                    tl(zh, "已儲存：", "Saved:")
                } else {
                    tl(zh, "已匯出：", "Exported:")
                },
                out.display(),
                explicit_format.label(),
                img.width(),
                img.height(),
                size_kb
            );
            if skip_c2pa_note {
                msg.push_str(&tl(
                    zh,
                    "（此格式不支援 C2PA，已略過簽署）",
                    " (C2PA unsupported for this format; skipped)",
                ));
            }
            ui.set_status(msg.into());
            st.borrow_mut().last_save_dir = out
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map(Path::to_path_buf);
            push_recent(ui, st, out);
            if update_doc {
                {
                    let mut s = st.borrow_mut();
                    s.doc.path = Some(out.to_path_buf());
                    s.doc.source_metadata = studio_core::metadata::read_metadata(out);
                    s.edit_session = None;
                    // A flattened export does not preserve editable layers.
                    if s.doc.layers.is_empty() {
                        s.doc.mark_saved();
                    }
                    s.c2pa_info = studio_core::inspect_c2pa(out).unwrap_or_default();
                }
                {
                    let s = st.borrow();
                    ui.set_has_source(true);
                    ui.set_c2pa_present(s.c2pa_info.present);
                    ui.set_c2pa_raw(s.c2pa_info.raw_json.clone().unwrap_or_default().into());
                }
                refresh_info(ui, st);
                refresh(ui, st);
            }
            true
        }
        Err(e) => {
            ui.set_status(format!("{} {e}", tl(zh, "儲存失敗：", "Save failed:")).into());
            false
        }
    }
}

fn convert_file_to(
    ui: &AppWindow,
    st: &S,
    src: &Path,
    out: &Path,
    actual_fmt: SupportedFormat,
) -> bool {
    let (keep, zh, opts, apply_c2pa, skip_c2pa_note, draft) = {
        let mut s = st.borrow_mut();
        sync_state_from_export_ui(ui, &mut s);
        let opts = format_options(ui, &s, actual_fmt);
        let c2pa_req = ui.get_c2pa_on_save();
        (
            ui.get_keep_metadata(),
            s.zh,
            opts,
            c2pa_req && actual_fmt.supports_c2pa(),
            c2pa_req && !actual_fmt.supports_c2pa(),
            c2pa_draft(ui),
        )
    };
    let keep_meta = keep && actual_fmt.supports_exif_icc();
    let res: anyhow::Result<(Option<SupportedFormat>, u32, u32, u64)> = if apply_c2pa {
        let mut meta = None;
        studio_core::atomic_file::write_with(out, |staged| -> anyhow::Result<()> {
            let ext = out.extension().and_then(|e| e.to_str()).unwrap_or("tmp");
            let tmp = staged.with_extension(format!("convert_src.{ext}"));
            let signed = staged.with_extension(format!("convert_c2pa.{ext}"));
            let inner = (|| -> anyhow::Result<()> {
                let conv = studio_core::convert_image_with_options(src, &tmp, opts, keep_meta)?;
                studio_core::write_c2pa_manifest(&tmp, &signed, &draft)?;
                copy_to_staged(&signed, staged)?;
                let size = std::fs::metadata(&signed)?.len();
                meta = Some((conv.source_format, conv.width, conv.height, size));
                Ok(())
            })();
            let _ = std::fs::remove_file(tmp);
            let _ = std::fs::remove_file(signed);
            inner
        })
        .and_then(|()| {
            meta.ok_or_else(|| anyhow::anyhow!("missing conversion metadata after C2PA signing"))
        })
    } else {
        studio_core::convert_image_with_options(src, out, opts, keep_meta)
            .map(|conv| (conv.source_format, conv.width, conv.height, conv.file_size))
            .map_err(anyhow::Error::from)
    };

    match res {
        Ok((src_fmt, w, h, file_size)) => {
            st.borrow_mut().last_save_dir = out
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map(Path::to_path_buf);
            push_recent(ui, st, out);
            let src_label = src_fmt.map(|f| f.label()).unwrap_or("IMAGE");
            let kb = file_size as f64 / 1024.0;
            let mut msg = format!(
                "{} {} → {} ({}×{}, {:.1} KB) — {}",
                tl(zh, "已轉換：", "Converted:"),
                src_label,
                actual_fmt.label(),
                w,
                h,
                kb,
                out.display()
            );
            if skip_c2pa_note {
                msg.push_str(&tl(
                    zh,
                    "（此格式不支援 C2PA，已略過簽署）",
                    " (C2PA unsupported for this format; skipped)",
                ));
            }
            ui.set_status(msg.into());
            true
        }
        Err(e) => {
            ui.set_status(format!("{} {e}", tl(zh, "轉換失敗：", "Conversion failed:")).into());
            false
        }
    }
}

fn c2pa_draft(ui: &AppWindow) -> C2paManifestDraft {
    let action: String = ui.get_c2pa_action().into();
    C2paManifestDraft {
        title: ui.get_c2pa_title().into(),
        creator: ui.get_c2pa_creator().into(),
        action,
    }
}

/// Inspect C2PA of a freshly opened file and pre-fill the manifest draft (as in v0.2).
fn load_c2pa_state(ui: &AppWindow, st: &S, path: Option<&Path>) {
    let info = path
        .and_then(|p| studio_core::inspect_c2pa(p).ok())
        .unwrap_or_default();
    let default_title = path
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(str::to_owned)
        .unwrap_or_default();
    ui.set_c2pa_title(info.title.clone().unwrap_or(default_title).into());
    ui.set_c2pa_creator(info.creator.clone().unwrap_or_default().into());
    ui.set_c2pa_action(info.action.clone().unwrap_or_default().into());
    ui.set_c2pa_on_save(info.present);
    ui.set_c2pa_present(info.present);
    ui.set_c2pa_raw(info.raw_json.clone().unwrap_or_default().into());
    ui.set_has_source(path.is_some());
    st.borrow_mut().c2pa_info = info;
    refresh_info(ui, st);
}

fn c2pa_summary(zh: bool, c: &studio_core::C2paInfo) -> String {
    if !c.present {
        return tl(
            zh,
            "此檔案沒有 C2PA 內容憑證。",
            "No C2PA manifest in this file.",
        );
    }
    format!(
        "{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}",
        tl(zh, "狀態", "Status"),
        c.validation_state.as_deref().unwrap_or("Unknown"),
        tl(zh, "Manifest 數量", "Manifests"),
        c.manifest_count,
        tl(zh, "標題", "Title"),
        c.title.clone().unwrap_or_default(),
        tl(zh, "作者", "Creator"),
        c.creator.clone().unwrap_or_default(),
        tl(zh, "動作", "Action"),
        c.action.clone().unwrap_or_default()
    )
}

/// Encode to a temporary sibling file, then sign it into `out` with the manifest.
fn save_with_c2pa(
    img: &DynamicImage,
    out: &Path,
    opts: ConvertOptions,
    draft: &C2paManifestDraft,
    source_meta: Option<&studio_core::metadata::SourceMetadata>,
) -> anyhow::Result<()> {
    studio_core::atomic_file::write_with(out, |staged| {
        let ext = out.extension().and_then(|e| e.to_str()).unwrap_or("tmp");
        // The staging filename is randomly reserved by tempfile.
        let tmp = staged.with_extension(format!("source.{ext}"));
        let signed = staged.with_extension(format!("c2pa.{ext}"));
        let result = (|| -> anyhow::Result<()> {
            studio_core::save_dynamic_image(img, &tmp, opts)?;
            if let Some(meta) = source_meta {
                studio_core::metadata::apply_metadata(meta, &tmp)?;
            }
            studio_core::write_c2pa_manifest(&tmp, &signed, draft)?;
            copy_to_staged(&signed, staged)
        })();
        let _ = std::fs::remove_file(tmp);
        let _ = std::fs::remove_file(signed);
        result
    })
}

fn name_with_suffix_ext(path: Option<&Path>, suffix: &str, ext: &str) -> String {
    match path
        .and_then(|p| p.file_stem())
        .and_then(|v| v.to_str())
        .filter(|s| !s.is_empty())
    {
        Some(stem) => format!("{stem}{suffix}.{ext}"),
        None => studio_core::naming::timestamp_name(ext),
    }
}

fn same_existing_path(a: &Path, b: &Path) -> bool {
    matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}

fn resolve_c2pa_removal_target(
    raw: &Path,
    src: &Path,
    src_fmt: Option<SupportedFormat>,
) -> Result<PathBuf, String> {
    if let Some(out_fmt) = SupportedFormat::from_path(raw) {
        if let Some(sfmt) = src_fmt
            && out_fmt != sfmt
        {
            return Err(format!(
                "container format must match source (.{}), got .{}",
                src.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or(sfmt.extension()),
                out_fmt.extension()
            ));
        }
        return Ok(raw.to_path_buf());
    }
    let src_ext = src
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty())
        .or_else(|| src_fmt.map(|f| f.extension()))
        .unwrap_or("jpg");
    let mut out = raw.to_path_buf();
    let has_nonempty_ext = raw
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| !e.is_empty());
    if !has_nonempty_ext {
        out.set_extension(src_ext);
    } else {
        let mut os = out.into_os_string();
        os.push(".");
        os.push(src_ext);
        out = PathBuf::from(os);
    }
    Ok(out)
}

fn remove_c2pa_atomic(src: &Path, out: &Path) -> studio_core::Result<()> {
    studio_core::atomic_file::write_with(out, |staged| -> anyhow::Result<()> {
        let ext = src
            .extension()
            .and_then(|e| e.to_str())
            .or_else(|| out.extension().and_then(|e| e.to_str()))
            .unwrap_or("tmp");
        // Write to a fresh sibling file so we never unlink/recreate `staged` while
        // `NamedTempFile` holds an open Windows handle to it.
        let stripped = staged.with_extension(format!("noc2pa.{ext}"));
        let inner = (|| -> anyhow::Result<()> {
            studio_core::remove_c2pa_manifest(src, &stripped)?;
            copy_to_staged(&stripped, staged)
        })();
        let _ = std::fs::remove_file(&stripped);
        inner
    })
    .map_err(|e| studio_core::PhotoError::InvalidInput(e.to_string()))
}

/// "Add C2PA": render the current (edited) image and save a signed copy.
fn c2pa_save_copy(ui: &AppWindow, st: &S) {
    let (img, src, source_meta, last_dir, keep, zh) = {
        let mut s = st.borrow_mut();
        sync_state_from_export_ui(ui, &mut s);
        (
            s.doc.flattened(),
            s.doc.path.clone(),
            s.doc.source_metadata.clone(),
            s.last_save_dir.clone(),
            ui.get_keep_metadata(),
            s.zh,
        )
    };
    let fmt = format_at(ui.get_export_format());
    let preferred = if fmt.supports_c2pa() {
        fmt
    } else {
        SupportedFormat::Jpeg
    };
    let name = name_with_suffix_ext(src.as_deref(), "_c2pa", preferred.extension());
    let Some(raw) = save_image_filters(
        suggested_save_dialog(preferred, src.as_deref(), last_dir.as_deref()).set_file_name(name),
        preferred,
        zh,
        true,
    )
    .save_file() else {
        return;
    };
    let (out, format) = studio_core::resolve_save_target(raw, preferred);
    if !format.supports_c2pa() {
        ui.set_status(
            format!(
                "{} {}",
                tl(zh, "此格式不支援 C2PA：", "C2PA is unsupported for format:"),
                format.label()
            )
            .into(),
        );
        return;
    }
    let opts = format_options(ui, &st.borrow(), format);
    let meta = source_meta
        .as_ref()
        .filter(|_| keep && format.supports_exif_icc());
    let r = save_with_c2pa(&img, &out, opts, &c2pa_draft(ui), meta);
    if r.is_ok() {
        st.borrow_mut().last_save_dir = out
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf);
        push_recent(ui, st, &out);
    }
    ui.set_status(
        match r {
            Ok(()) => format!(
                "{} {} ({}×{})",
                tl(zh, "已添加 C2PA 並另存：", "Added C2PA and saved"),
                out.display(),
                img.width(),
                img.height()
            ),
            Err(e) => format!("{} {e}", tl(zh, "添加 C2PA 失敗：", "Adding C2PA failed:")),
        }
        .into(),
    );
}

/// "Remove C2PA": copy the source bytes without the manifest (same extension, new file).
fn c2pa_remove_copy(ui: &AppWindow, st: &S) {
    let (src, last_dir, zh) = {
        let s = st.borrow();
        (s.doc.path.clone(), s.last_save_dir.clone(), s.zh)
    };
    let Some(src) = src else {
        ui.set_status(
            tl(
                zh,
                "尚未開啟可移除 C2PA 的來源檔。",
                "No source file loaded for C2PA removal.",
            )
            .into(),
        );
        return;
    };
    let src_fmt = SupportedFormat::from_path(&src)
        .or_else(|| studio_core::inspect_image(&src).ok().map(|i| i.format));
    if !src_fmt.is_some_and(|f| f.supports_c2pa()) {
        ui.set_status(
            tl(
                zh,
                "來源檔案格式不支援 C2PA 憑證。",
                "Source file format does not support C2PA credentials.",
            )
            .into(),
        );
        return;
    }
    let preferred = src_fmt.unwrap_or(SupportedFormat::Jpeg);
    let src_ext = src
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty())
        .unwrap_or(preferred.extension());
    let name = name_with_suffix_ext(Some(&src), "_no_c2pa", src_ext);
    let Some(raw) = save_image_filters(
        suggested_save_dialog(preferred, Some(&src), last_dir.as_deref()).set_file_name(name),
        preferred,
        zh,
        true,
    )
    .save_file() else {
        return;
    };
    let out = match resolve_c2pa_removal_target(&raw, &src, src_fmt) {
        Ok(p) => p,
        Err(reason) => {
            ui.set_status(
                format!(
                    "{} {reason}",
                    tl(
                        zh,
                        "移除 C2PA 須保留與來源相同的格式：",
                        "Removing C2PA must keep the source format:"
                    )
                )
                .into(),
            );
            return;
        }
    };
    if same_existing_path(&src, &out) {
        ui.set_status(
            tl(
                zh,
                "移除 C2PA 請另存為不同檔案。",
                "Choose a different file when removing C2PA.",
            )
            .into(),
        );
        return;
    }
    let result = remove_c2pa_atomic(&src, &out);
    if result.is_ok() {
        st.borrow_mut().last_save_dir = out
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf);
        push_recent(ui, st, &out);
    }
    ui.set_status(
        match result {
            Ok(()) => format!(
                "{} {}",
                tl(zh, "已移除 C2PA 並另存：", "Removed C2PA and saved"),
                out.display()
            ),
            Err(e) => format!(
                "{} {e}",
                tl(zh, "移除 C2PA 失敗：", "Removing C2PA failed:")
            ),
        }
        .into(),
    );
}

fn refresh_info(ui: &AppWindow, st: &S) {
    let mut s = st.borrow_mut();
    maybe_refresh_histogram(ui, &mut s);
    let zh = s.zh;
    let mut info = format!(
        "{}×{} px, {} {}\n",
        s.doc.width(),
        s.doc.height(),
        s.doc.layers.len(),
        tl(zh, "圖層", "layers")
    );
    let mut c2 = tl(zh, "（未從檔案開啟）", "(not opened from a file)");
    if let Some(p) = &s.doc.path {
        info += &format!("{}\n", p.display());
        if let Ok(i) = studio_core::inspect_image(p) {
            info += &format!(
                "{} · {} · {:.1} KB\n",
                i.format.label(),
                i.color_type,
                i.file_size as f64 / 1024.0
            );
        }
        let has_exif = s.doc.source_metadata.as_ref().is_some_and(|m| m.has_exif())
            || studio_core::metadata::has_exif(p);
        info += &format!(
            "EXIF: {}\n",
            if has_exif {
                tl(zh, "有", "yes")
            } else {
                tl(zh, "無", "no")
            }
        );
        c2 = c2pa_summary(zh, &s.c2pa_info);
    }
    ui.set_info_text(info.into());
    ui.set_c2pa_text(c2.into());
}

// ------------------------------------------------------------------- stickers

fn thumb(s: &mut State, id: &str) -> Image {
    if let Some(i) = s.thumbs.get(id) {
        return i.clone();
    }
    let img = stickers::find(&s.doc.library, id)
        .map(|st| {
            let style = StickerStyle {
                fill: st.default_fill,
                ..StickerStyle::default()
            };
            stickers::render_sticker(st, &style, 96, 96).to_rgba8()
        })
        .unwrap_or_else(|| image::RgbaImage::new(1, 1));
    let buf =
        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(img.as_raw(), img.width(), img.height());
    let i = Image::from_rgba8(buf);
    s.thumbs.insert(id.to_owned(), i.clone());
    i
}

fn update_stickers(ui: &AppWindow, st: &S) {
    let mut s = st.borrow_mut();
    let q: String = ui.get_sticker_query().into();
    let cat = ui.get_sticker_category();
    let category = (cat > 0).then(|| StickerCategory::ALL[(cat - 1) as usize]);
    let ids: Vec<String> = stickers::search(&s.doc.library, &q, category)
        .into_iter()
        .map(|x| x.id.clone())
        .collect();
    let items: Vec<StickerItem> = ids
        .iter()
        .map(|id| StickerItem {
            id: id.into(),
            img: thumb(&mut s, id),
        })
        .collect();
    ui.set_stickers(ModelRc::new(VecModel::from(items)));
}

/// Change the sticker style; applies to the selected sticker layer too.
fn restyle_selected_sticker(st: &S, f: impl FnOnce(&mut StickerStyle)) {
    let mut s = st.borrow_mut();
    let base = match s.doc.selected.and_then(|i| s.doc.layers.get(i)) {
        Some(ComposeLayer::Image(l)) => l.sticker.as_ref().map(|i| i.style),
        _ => None,
    };
    let mut style = base.unwrap_or(s.sticker_style);
    f(&mut style);
    s.sticker_style = StickerStyle {
        fill: s.sticker_style.fill,
        ..style
    };
    if base.is_some() {
        s.doc.checkpoint();
        s.doc.with_selected(|l| {
            if let ComposeLayer::Image(l) = l
                && let Some(inst) = &mut l.sticker
            {
                inst.style = style;
            }
        });
    }
}

// ------------------------------------------------------------------- layer editor

fn sync_layer_fields(ui: &AppWindow, s: &State) {
    let Some(l) = s.doc.selected.and_then(|i| s.doc.layers.get(i)) else {
        return;
    };
    ui.set_l_name(l.name().into());
    ui.set_l_blend(l.blend_mode().index() as i32);
    match l {
        ComposeLayer::Text(t) => {
            ui.set_l_text(t.text.clone().into());
            ui.set_l_font_size(t.font_size);
            ui.set_l_opacity(t.opacity * 100.0);
            ui.set_l_stroke(t.stroke);
            ui.set_l_shadow(t.shadow);
            ui.set_l_font(
                t.font_path
                    .as_ref()
                    .and_then(|p| s.font_paths.iter().position(|q| q == p))
                    .map(|i| i as i32 + 1)
                    .unwrap_or(0),
            );
        }
        ComposeLayer::Image(i) => {
            ui.set_l_opacity(i.opacity * 100.0);
            ui.set_l_w(i.width as i32);
            ui.set_l_h(i.height as i32);
            if let Some(inst) = &i.sticker {
                ui.set_st_stroke(inst.style.stroke_width);
                ui.set_st_shadow(inst.style.shadow);
                ui.set_st_fill(to_color(inst.style.fill));
                ui.set_st_outline(to_color(inst.style.stroke));
            }
        }
        ComposeLayer::Shape(sh) => {
            ui.set_l_opacity(sh.opacity * 100.0);
            ui.set_l_stroke_width(sh.stroke_width);
            ui.set_l_filled(sh.filled);
        }
    }
}

fn layer_edited(ui: &AppWindow, st: &S, field: &str) {
    let mut s = st.borrow_mut();
    let Some(idx) = s.doc.selected else { return };
    if field == "blend" {
        let mode = BlendMode::from_index(ui.get_l_blend().max(0) as usize);
        s.edit_session = None;
        s.doc.set_selected_blend_mode(mode);
        return;
    }
    // One undo step per continuous edit of the same field.
    let session = (idx, field.to_owned());
    if s.edit_session.as_ref() != Some(&session) {
        s.doc.checkpoint();
        s.edit_session = Some(session);
    }
    let (c1, c2) = (s.tools.color1, s.tools.color2);
    let font = {
        let i = ui.get_l_font();
        if i > 0 {
            s.font_paths.get((i - 1) as usize).cloned()
        } else {
            None
        }
    };
    let opacity = (ui.get_l_opacity() / 100.0).clamp(0.0, 1.0);
    s.doc.with_selected(|l| match (field, l) {
        ("name", l) => *l.name_mut() = ui.get_l_name().into(),
        ("opacity", ComposeLayer::Text(t)) => t.opacity = opacity,
        ("opacity", ComposeLayer::Image(t)) => t.opacity = opacity,
        ("opacity", ComposeLayer::Shape(t)) => t.opacity = opacity,
        ("text", ComposeLayer::Text(t)) => t.text = ui.get_l_text().into(),
        ("font-size", ComposeLayer::Text(t)) => t.font_size = ui.get_l_font_size(),
        ("font", ComposeLayer::Text(t)) => t.font_path = font,
        ("stroke", ComposeLayer::Text(t)) => t.stroke = ui.get_l_stroke(),
        ("shadow", ComposeLayer::Text(t)) => t.shadow = ui.get_l_shadow(),
        ("color", ComposeLayer::Text(t)) => t.color = c1,
        ("color", ComposeLayer::Shape(t)) => {
            t.stroke_color = c1;
            t.fill_color = c2;
        }
        ("w", ComposeLayer::Image(t)) => t.width = ui.get_l_w().max(1) as u32,
        ("h", ComposeLayer::Image(t)) => t.height = ui.get_l_h().max(1) as u32,
        ("stroke-width", ComposeLayer::Shape(t)) => t.stroke_width = ui.get_l_stroke_width(),
        ("filled", ComposeLayer::Shape(t)) => t.filled = ui.get_l_filled(),
        _ => {}
    });
}

// ------------------------------------------------------------------- pointer input

fn to_img(s: &State, x: f32, y: f32) -> (f32, f32) {
    ((x - s.pan.0) / s.zoom, (y - s.pan.1) / s.zoom)
}

/// Cursor targets use the same hit tests as pointer-down; text inputs live outside
/// this canvas TouchArea and keep Slint's native text cursor and selection handling.
fn canvas_cursor(
    doc: &Doc,
    tool: Tool,
    drag: Drag,
    pan: (f32, f32),
    zoom: f32,
    mouse: Option<(f32, f32)>,
) -> i32 {
    match drag {
        Drag::Move { .. } | Drag::BoxMove { .. } | Drag::Pan { .. } => return 2,
        Drag::Resize { .. } | Drag::BoxResize { .. } => return 3,
        Drag::CanvasEdge { kind, .. } => {
            return match kind {
                0 => 4,
                1 => 5,
                _ => 3,
            };
        }
        _ => {}
    }
    if tool == Tool::Hand {
        return 1;
    }
    let Some((mx, my)) = mouse else { return 0 };
    let (x, y) = ((mx - pan.0) / zoom, (my - pan.1) / zoom);
    if tool == Tool::Select {
        if let Some(kind) = canvas_edge(doc, pan, zoom, mx, my) {
            return match kind {
                0 => 4,
                1 => 5,
                _ => 3,
            };
        }
        if doc.on_resize_handle(x, y, 8.0 / zoom) {
            return 3;
        }
        if doc.hit_test(x, y).is_some() {
            return 1;
        }
    }
    if tool == Tool::RedactBox {
        let tol = 8.0 / zoom;
        if doc.boxes.iter().any(|b| {
            (x - (b.x as f32 + b.width as f32)).abs() <= tol
                && (y - (b.y as f32 + b.height as f32)).abs() <= tol
        }) {
            return 3;
        }
        if doc.box_at(x, y).is_some() {
            return 1;
        }
    }
    0
}

fn canvas_edge(doc: &Doc, pan: (f32, f32), zoom: f32, mx: f32, my: f32) -> Option<u8> {
    let (ex, ey) = (
        pan.0 + doc.width() as f32 * zoom,
        pan.1 + doc.height() as f32 * zoom,
    );
    let near = |ax: f32, ay: f32| (mx - ax).abs() <= 9.0 && (my - ay).abs() <= 9.0;
    if near(ex + 4.0, ey + 4.0) {
        Some(2)
    } else if near(ex + 3.0, pan.1 + (ey - pan.1) / 2.0) {
        Some(0)
    } else if near(pan.0 + (ex - pan.0) / 2.0, ey + 3.0) {
        Some(1)
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn pointer(
    ui: &AppWindow,
    st: &S,
    kind: i32,
    button: i32,
    mx: f32,
    my: f32,
    shift: bool,
    alt: bool,
) {
    let mut guard = st.borrow_mut();
    let s = &mut *guard;
    s.pointer_pos = Some((mx, my));
    let p = to_img(s, mx, my);
    if kind == 2 || kind == 0 {
        ui.set_cursor_info(
            match s.doc.sample(p.0, p.1) {
                Some(c) => format!(
                    "{}, {}  #{:02X}{:02X}{:02X}{:02X}",
                    p.0 as i32, p.1 as i32, c[0], c[1], c[2], c[3]
                ),
                None => String::new(),
            }
            .into(),
        );
        if kind == 2 && matches!(s.drag, Drag::None) {
            ui.set_canvas_cursor(canvas_cursor(
                &s.doc,
                if s.view_mode { Tool::Hand } else { s.tool },
                s.drag,
                s.pan,
                s.zoom,
                s.pointer_pos,
            ));
            return; // hover only
        }
    }
    let tool = if s.view_mode { Tool::Hand } else { s.tool };
    s.tools.stroke_width = ui.get_stroke_width();
    s.tools.shape_filled = ui.get_shape_filled();
    s.tools.fill_tolerance = ui.get_fill_tolerance() as u8;
    s.tools.redact_brush_radius = ui.get_redact_radius();
    s.tools.tape_width = ui.get_tape_width();
    s.tools.tape_pattern = ui.get_tape_pattern().clamp(0, 9) as usize;
    s.tools.gradient_radial = ui.get_gradient_mode() == 1;
    s.tools.wand_tolerance = ui.get_wand_tolerance().clamp(0.0, 128.0) as u8;
    s.tools.wand_contiguous = ui.get_wand_contiguous();
    s.tools.clone_hardness = ui.get_clone_hardness().clamp(0.1, 1.0);
    s.tools.redact_color = s.tools.color1;
    s.tools.shape = [
        ShapeKind::Line,
        ShapeKind::Arrow,
        ShapeKind::Rectangle,
        ShapeKind::Ellipse,
        ShapeKind::Triangle,
    ][ui.get_shape_kind().clamp(0, 4) as usize];
    match kind {
        0 => {
            cancel_gesture(s);
            s.edit_session = None;
            if tool != Tool::RedactBox {
                s.doc.focus_box(None);
            }
            if button == 2 || tool == Tool::Hand {
                s.drag = Drag::Pan {
                    mouse: (mx, my),
                    pan: s.pan,
                };
                ui.set_canvas_cursor(2);
                return;
            }
            let secondary = button == 1;
            // Right-click opens the context menu for non-painting tools.
            if secondary && matches!(tool, Tool::Select | Tool::Crop | Tool::Text | Tool::Shapes) {
                ui.set_ctx_x(mx);
                ui.set_ctx_y(my);
                ui.set_ctx_open(true);
                return;
            }
            // Canvas edge handles (Select tool): drag to resize the canvas.
            if tool == Tool::Select && !secondary {
                let kind = canvas_edge(&s.doc, s.pan, s.zoom, mx, my);
                if let Some(kind) = kind {
                    s.drag = Drag::CanvasEdge {
                        kind,
                        now: (s.doc.width() as f32, s.doc.height() as f32),
                    };
                    ui.set_canvas_cursor(match kind {
                        0 => 4,
                        1 => 5,
                        _ => 3,
                    });
                    return;
                }
            }
            match tool {
                t if t.is_raster() || t == Tool::RedactBrush || t == Tool::Tape => {
                    if t == Tool::Tape && s.doc.sample(p.0, p.1).is_none() {
                        return;
                    }
                    s.gesture = Some(s.doc.begin_gesture());
                    if t == Tool::Tape {
                        s.doc.begin_tape_stroke();
                    }
                    s.doc.stroke(t, p, p, secondary, &s.tools);
                    s.drag = Drag::Paint { last: p, secondary };
                }
                Tool::Fill => {
                    let c = if secondary {
                        s.tools.color2
                    } else {
                        s.tools.color1
                    };
                    s.doc.flood_fill(p.0, p.1, c, s.tools.fill_tolerance);
                }
                Tool::Picker => {
                    if let Some(c) = s.doc.sample(p.0, p.1) {
                        if secondary {
                            s.tools.color2 = c;
                            ui.set_color2(to_color(c));
                        } else {
                            s.tools.color1 = c;
                            ui.set_color1(to_color(c));
                        }
                    }
                }
                Tool::Text => {
                    let name = format!("{} {}", tl(s.zh, "文字", "Text"), s.doc.layers.len() + 1);
                    s.doc.add_text(p.0, p.1, name);
                    sync_layer_fields(ui, s);
                    ui.set_tab(1);
                    ui.set_status(
                        tl(
                            s.zh,
                            "已新增文字，請在右側「圖層」編輯內容。",
                            "Text added; edit it in the Layers tab.",
                        )
                        .into(),
                    );
                }
                Tool::Shapes => s.drag = Drag::Shape { start: p, now: p },
                Tool::RedactBox => {
                    if secondary {
                        let target = s.doc.box_at(p.0, p.1);
                        s.doc.focus_box(target);
                        if let Some(i) = target {
                            s.doc.delete_box(i);
                        }
                    } else {
                        let tol = 8.0 / s.zoom;
                        let corner = s.doc.boxes.iter().rposition(|b| {
                            (p.0 - (b.x as f32 + b.width as f32)).abs() <= tol
                                && (p.1 - (b.y as f32 + b.height as f32)).abs() <= tol
                        });
                        s.doc.focus_box(corner.or_else(|| s.doc.box_at(p.0, p.1)));
                        s.drag = match (corner, s.doc.box_at(p.0, p.1)) {
                            (Some(i), _) => Drag::BoxResize { i },
                            (None, Some(i)) => Drag::BoxMove {
                                i,
                                last: p,
                                moved: false,
                            },
                            _ => Drag::RedactBox { start: p, now: p },
                        };
                        s.gesture = Some(s.doc.begin_gesture());
                    }
                }
                Tool::Select | Tool::Crop => {
                    let tol = 8.0 / s.zoom;
                    let hit = if tool == Tool::Select {
                        s.doc.hit_test(p.0, p.1)
                    } else {
                        None
                    };
                    if tool == Tool::Select && s.doc.on_resize_handle(p.0, p.1, tol) {
                        let aspect = match s.doc.selected.and_then(|i| s.doc.layers.get(i)) {
                            Some(ComposeLayer::Image(l)) => l.height as f32 / l.width.max(1) as f32,
                            _ => 1.0,
                        };
                        s.gesture = Some(s.doc.begin_gesture());
                        s.drag = Drag::Resize { aspect };
                    } else if let Some(i) = hit {
                        s.doc.selected = Some(i);
                        s.edit_session = None;
                        s.gesture = Some(s.doc.begin_gesture());
                        s.drag = Drag::Move { last: p };
                        sync_layer_fields(ui, s);
                    } else {
                        s.gesture = Some(s.doc.begin_gesture());
                        s.doc.selected = None;
                        s.doc.selection = None;
                        s.doc.selection_mask = None;
                        s.doc.selection_path.clear();
                        if ui.get_select_shape() == 2 {
                            s.doc.selection_path.push((
                                p.0.clamp(0.0, s.doc.width() as f32),
                                p.1.clamp(0.0, s.doc.height() as f32),
                            ));
                        }
                        s.drag = Drag::Select { start: p };
                    }
                }
                Tool::Gradient => {
                    // `Doc::apply_gradient` calls `checkpoint()` internally; do not start `s.gesture`.
                    s.drag = Drag::Gradient {
                        start: p,
                        now: p,
                        secondary,
                    };
                }
                Tool::MagicWand => {
                    if s.doc.magic_wand_select(
                        p.0,
                        p.1,
                        s.tools.wand_tolerance,
                        s.tools.wand_contiguous,
                    ) {
                        if let Some(sel) = s.doc.selection {
                            let (sw, sh) = (sel.w.round() as u32, sel.h.round() as u32);
                            ui.set_status(
                                if s.zh {
                                    format!("魔術棒已選取 {sw}×{sh} px 區域")
                                } else {
                                    format!("Magic wand selected {sw}x{sh} px region")
                                }
                                .into(),
                            );
                        }
                    } else {
                        ui.set_status(
                            tl(s.zh, "魔術棒未選取任何像素", "Magic wand matched no pixels").into(),
                        );
                    }
                }
                Tool::Clone => {
                    if secondary || alt {
                        s.tools.clone_source = Some(p);
                        let (cx, cy) = (p.0.round() as i32, p.1.round() as i32);
                        ui.set_status(
                            if s.zh {
                                format!("已設定仿製取樣起點 ({cx}, {cy})")
                            } else {
                                format!("Clone source set at ({cx}, {cy})")
                            }
                            .into(),
                        );
                    } else if let Some(src0) = s.tools.clone_source {
                        s.gesture = Some(s.doc.begin_gesture());
                        let r = (s.tools.stroke_width * 0.5).max(1.0);
                        s.doc.clone_dab(src0, p, r, s.tools.clone_hardness);
                        s.drag = Drag::Clone {
                            offset: (src0.0 - p.0, src0.1 - p.1),
                            last: p,
                        };
                    } else {
                        ui.set_status(
                            tl(
                                s.zh,
                                "請先右鍵或 Alt+點選設定取樣起點",
                                "Right-click or Alt+click to sample source first",
                            )
                            .into(),
                        );
                    }
                }
                _ => {}
            }
        }
        2 => match s.drag {
            Drag::Paint { last, secondary } => {
                s.doc.stroke(tool, last, p, secondary, &s.tools);
                s.drag = Drag::Paint { last: p, secondary };
            }
            Drag::Pan { mouse, pan } => s.pan = (pan.0 + mx - mouse.0, pan.1 + my - mouse.1),
            Drag::Select { start } => {
                if !s.doc.selection_path.is_empty() {
                    let point = (
                        p.0.clamp(0.0, s.doc.width() as f32),
                        p.1.clamp(0.0, s.doc.height() as f32),
                    );
                    let last = *s.doc.selection_path.last().unwrap();
                    if (point.0 - last.0).hypot(point.1 - last.1) >= 1.0 {
                        s.doc.selection_path.push(point);
                    }
                    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                    for &(x, y) in &s.doc.selection_path {
                        x0 = x0.min(x);
                        y0 = y0.min(y);
                        x1 = x1.max(x);
                        y1 = y1.max(y);
                    }
                    s.doc.selection = Some(Selection::from_points(
                        (x0, y0),
                        (x1, y1),
                        false,
                        s.doc.width(),
                        s.doc.height(),
                    ));
                } else {
                    s.doc.selection = Some(Selection::from_points(
                        start,
                        p,
                        ui.get_select_shape() == 1,
                        s.doc.width(),
                        s.doc.height(),
                    ));
                }
            }
            Drag::Shape { start, .. } => {
                let mut now = p;
                if shift {
                    let d = (p.0 - start.0).abs().max((p.1 - start.1).abs());
                    now = (
                        start.0 + d * (p.0 - start.0).signum(),
                        start.1 + d * (p.1 - start.1).signum(),
                    );
                }
                s.drag = Drag::Shape { start, now };
            }
            Drag::RedactBox { start, .. } => s.drag = Drag::RedactBox { start, now: p },
            Drag::Move { last } => {
                let (dx, dy) = ((p.0 - last.0).round() as i32, (p.1 - last.1).round() as i32);
                if dx != 0 || dy != 0 {
                    s.doc.move_selected(dx, dy);
                    s.drag = Drag::Move {
                        last: (last.0 + dx as f32, last.1 + dy as f32),
                    };
                }
            }
            Drag::Resize { aspect } => s.doc.resize_selected_to(p.0, aspect),
            Drag::BoxMove { i, last, moved } => {
                let (dx, dy) = ((p.0 - last.0).round() as i32, (p.1 - last.1).round() as i32);
                if let Some(b) = s.doc.boxes.get_mut(i)
                    && (dx != 0 || dy != 0)
                {
                    b.x += dx;
                    b.y += dy;
                    s.drag = Drag::BoxMove {
                        i,
                        last: (last.0 + dx as f32, last.1 + dy as f32),
                        moved: true,
                    };
                } else {
                    s.drag = Drag::BoxMove { i, last, moved };
                }
            }
            Drag::BoxResize { i } => {
                if let Some(b) = s.doc.boxes.get_mut(i) {
                    b.width = (p.0 - b.x as f32).round().max(3.0) as u32;
                    b.height = (p.1 - b.y as f32).round().max(3.0) as u32;
                }
            }
            Drag::CanvasEdge { kind, .. } => {
                let (w, h) = (s.doc.width() as f32, s.doc.height() as f32);
                let nw = if kind == 1 {
                    w
                } else {
                    p.0.round().clamp(1.0, 20000.0)
                };
                let nh = if kind == 0 {
                    h
                } else {
                    p.1.round().clamp(1.0, 20000.0)
                };
                s.drag = Drag::CanvasEdge {
                    kind,
                    now: (nw, nh),
                };
            }
            Drag::Gradient {
                start, secondary, ..
            } => {
                let mut now = p;
                if shift {
                    let (dx, dy) = (p.0 - start.0, p.1 - start.1);
                    let dist = dx.hypot(dy);
                    let angle = (dy.atan2(dx) / std::f32::consts::FRAC_PI_4).round()
                        * std::f32::consts::FRAC_PI_4;
                    now = (start.0 + dist * angle.cos(), start.1 + dist * angle.sin());
                }
                s.drag = Drag::Gradient {
                    start,
                    now,
                    secondary,
                };
                let (x0, y0, x1, y1) = (
                    start.0.round() as i32,
                    start.1.round() as i32,
                    now.0.round() as i32,
                    now.1.round() as i32,
                );
                ui.set_status(
                    if s.zh {
                        format!("漸層 ({x0}, {y0}) → ({x1}, {y1})")
                    } else {
                        format!("Gradient ({x0}, {y0}) -> ({x1}, {y1})")
                    }
                    .into(),
                );
            }
            Drag::Clone { offset, last } => {
                let dist = (p.0 - last.0).hypot(p.1 - last.1);
                if dist >= 0.5 {
                    let r = (s.tools.stroke_width * 0.5).max(1.0);
                    let step = (r * 0.35).max(1.0);
                    let steps = (dist / step).ceil().max(1.0) as u32;
                    for k in 1..=steps {
                        let t = k as f32 / steps as f32;
                        let dst = (last.0 + (p.0 - last.0) * t, last.1 + (p.1 - last.1) * t);
                        let src = (dst.0 + offset.0, dst.1 + offset.1);
                        s.doc.clone_dab(src, dst, r, s.tools.clone_hardness);
                    }
                    s.drag = Drag::Clone { offset, last: p };
                }
            }
            Drag::None => {}
        },
        3 => cancel_gesture(s),
        1 => {
            if tool == Tool::Tape && matches!(s.drag, Drag::Paint { .. }) {
                s.doc.finish_tape_stroke();
            }
            match s.drag {
                Drag::Shape { start, now }
                    if (start.0 - now.0).abs() + (start.1 - now.1).abs() > 2.0 =>
                {
                    let t = s.tools.clone();
                    s.doc.add_shape(t.shape, start, now, &t);
                    sync_layer_fields(ui, s);
                }
                Drag::RedactBox { start, now } => {
                    let (x0, y0) = (start.0.min(now.0), start.1.min(now.1));
                    let (w, h) = ((start.0 - now.0).abs(), (start.1 - now.1).abs());
                    if w >= 3.0 && h >= 3.0 {
                        s.doc.boxes.push(studio_core::redact::RedactBox::manual(
                            x0 as i32, y0 as i32, w as u32, h as u32,
                        ));
                        s.doc.focus_box(Some(s.doc.boxes.len() - 1));
                    }
                }
                Drag::Select { .. } => {
                    if s.doc
                        .selection
                        .is_some_and(|sel| sel.w < 2.0 || sel.h < 2.0)
                    {
                        s.doc.clear_selection();
                    }
                    if !s.doc.selection_path.is_empty() && s.doc.selection_path.len() < 3 {
                        s.doc.clear_selection();
                    }
                    if tool == Tool::Crop && s.doc.selection.is_some() {
                        ui.set_status(
                            tl(
                                s.zh,
                                "按 Enter 或「調整 > 裁切」裁切選取區。",
                                "Press Enter or Adjust > Crop to crop.",
                            )
                            .into(),
                        );
                    }
                }
                Drag::Move { .. } | Drag::Resize { .. } => sync_layer_fields(ui, s),
                Drag::CanvasEdge { now, .. } => {
                    let bg = s.tools.color2;
                    s.doc.resize_canvas(now.0 as u32, now.1 as u32, bg);
                    ui.set_rs_w(s.doc.width() as i32);
                    ui.set_rs_h(s.doc.height() as i32);
                }
                Drag::Gradient {
                    start,
                    now,
                    secondary,
                } if (start.0 - now.0).hypot(start.1 - now.1) > 1.0 => {
                    let (c1, c2) = if secondary {
                        (s.tools.color2, s.tools.color1)
                    } else {
                        (s.tools.color1, s.tools.color2)
                    };
                    s.doc
                        .apply_gradient(start, now, c1, c2, s.tools.gradient_radial);
                }
                _ => {}
            }
            s.drag = Drag::None;
            if let Some(before) = s.gesture.take() {
                s.doc.commit_gesture(before);
            }
        }
        _ => {}
    }
    drop(guard);
    refresh(ui, st);
}

// ------------------------------------------------------------------- actions

fn with_doc(ui: &AppWindow, st: &S, f: impl FnOnce(&mut Doc)) {
    {
        let mut s = st.borrow_mut();
        f(&mut s.doc);
        s.edit_session = None;
    }
    refresh(ui, st);
}

fn centre(ui: &AppWindow) -> (f32, f32) {
    (ui.get_canvas_w() / 2.0, ui.get_canvas_h() / 2.0)
}

fn new_doc(ui: &AppWindow, st: &S, img: DynamicImage) -> bool {
    if !confirm_discard(st) {
        return false;
    }
    st.borrow_mut().doc.load(img, None);
    // New/pasted content has no on-disk copy yet.
    st.borrow_mut().doc.invalidate_all();
    reset_document_ui(ui, st);
    load_c2pa_state(ui, st, None);
    let (w, h) = {
        let s = st.borrow();
        (s.doc.width(), s.doc.height())
    };
    ui.set_rs_w(w as i32);
    ui.set_rs_h(h as i32);
    fit(ui, st);
    true
}

fn confirm_discard(st: &S) -> bool {
    let (modified, zh) = {
        let s = st.borrow();
        (s.doc.is_modified(), s.zh)
    };
    !modified
        || rfd::MessageDialog::new()
            .set_title("Pixel Studio")
            .set_description(tl(
                zh,
                "目前文件有未儲存的變更。確定捨棄？如需保留可編輯圖層，請先儲存專案。",
                "Discard unsaved changes? Save a project first to preserve editable layers.",
            ))
            .set_buttons(rfd::MessageButtons::YesNo)
            .show()
            == rfd::MessageDialogResult::Yes
}

fn reset_document_ui(ui: &AppWindow, st: &S) {
    let mut s = st.borrow_mut();
    s.quick_ok = false;
    s.edit_session = None;
    s.drag = Drag::None;
    s.gesture = None;
    s.hist_revision = None;
    ui.set_rs_w(s.doc.width() as i32);
    ui.set_rs_h(s.doc.height() as i32);
}

fn cancel_gesture(s: &mut State) {
    if let Some(before) = s.gesture.take() {
        s.doc.cancel_gesture(before);
    }
    if let Drag::Pan { pan, .. } = s.drag {
        s.pan = pan;
    }
    s.drag = Drag::None;
}

fn action(ui: &AppWindow, st: &S, a: &str) {
    use studio_core::{filters as fl, transform as tr};
    cancel_gesture(&mut st.borrow_mut());
    let zh = st.borrow().zh;
    if matches!(a, "rot-left" | "rot-right" | "rot-180" | "flip-h" | "flip-v" | "resize")
        && !st.borrow().doc.layers.is_empty()
        && rfd::MessageDialog::new()
            .set_title("Pixel Studio")
            .set_description(tl(zh,
                "此操作會先合併圖層，再變換整張圖片；可用復原還原圖層。繼續？",
                "This will flatten layers and transform the whole image. Undo restores editable layers. Continue?"))
            .set_buttons(rfd::MessageButtons::YesNo)
            .show() != rfd::MessageDialogResult::Yes
    {
        return;
    }
    match a {
        "new" => {
            new_doc(ui, st, blank(1080, 720));
        }
        "new-custom" => {
            new_doc(
                ui,
                st,
                blank(ui.get_new_w().max(1) as u32, ui.get_new_h().max(1) as u32),
            );
        }
        "open" => {
            if let Some(p) = open_image_filter(rfd::FileDialog::new(), zh)
                .add_filter("Pixel Studio Project", &["pixel-project"])
                .pick_file()
            {
                open_path(ui, st, &p);
            }
            return;
        }
        "open-folder" => {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                match studio_core::collect_supported_files(&dir).first() {
                    Some(first) => open_path(ui, st, first),
                    None => ui.set_status(
                        tl(
                            zh,
                            "資料夾中沒有支援的影像。",
                            "No supported images in that folder.",
                        )
                        .into(),
                    ),
                }
            }
            return;
        }
        "open-project" => {
            if let Some(p) = rfd::FileDialog::new()
                .add_filter("Pixel Studio Project", &["pixel-project"])
                .pick_file()
            {
                open_path(ui, st, &p);
            }
            return;
        }
        "save-project" => {
            if let Some(p) = rfd::FileDialog::new()
                .add_filter("Pixel Studio Project", &["pixel-project"])
                .set_file_name(studio_core::naming::timestamp_name("pixel-project"))
                .save_file()
            {
                let r = st.borrow().doc.to_project_json().and_then(|j| {
                    studio_core::atomic_file::write(&p, j.as_bytes()).map_err(|e| e.to_string())
                });
                if r.is_ok() {
                    let mut s = st.borrow_mut();
                    s.doc.mark_saved();
                    s.edit_session = None;
                }
                ui.set_status(
                    match r {
                        Ok(()) => format!(
                            "{} {}",
                            tl(zh, "專案已儲存：", "Project saved:"),
                            p.display()
                        ),
                        Err(e) => {
                            format!("{} {e}", tl(zh, "專案儲存失敗：", "Project save failed:"))
                        }
                    }
                    .into(),
                );
            }
            return;
        }
        "save" | "quick-save" => {
            let (ok, path) = {
                let s = st.borrow();
                (s.quick_ok, s.doc.path.clone())
            };
            match path {
                Some(p) if ok && SupportedFormat::from_path(&p).is_some() => {
                    let fmt = SupportedFormat::from_path(&p).unwrap_or(SupportedFormat::Png);
                    save_to(ui, st, &p, fmt, true);
                }
                _ => open_save_modal(ui, st, 0),
            }
            return;
        }
        "quick-save-confirm" => {
            let (mode, chosen_fmt, path, convert_src, last_dir) = {
                let mut s = st.borrow_mut();
                sync_state_from_export_ui(ui, &mut s);
                (
                    s.save_dialog_mode,
                    s.export_format,
                    s.doc.path.clone(),
                    s.convert_input.clone(),
                    s.last_save_dir.clone(),
                )
            };
            match mode {
                0 => {
                    let target = match path {
                        Some(p) if SupportedFormat::from_path(&p).is_some() => {
                            let orig_fmt = SupportedFormat::from_path(&p).unwrap_or(chosen_fmt);
                            Some((p, orig_fmt))
                        }
                        other => {
                            let d = save_image_filters(
                                suggested_save_dialog(
                                    chosen_fmt,
                                    other.as_deref(),
                                    last_dir.as_deref(),
                                ),
                                chosen_fmt,
                                zh,
                                false,
                            );
                            d.save_file()
                                .map(|p| studio_core::resolve_save_target(p, chosen_fmt))
                        }
                    };
                    if let Some((out, actual_fmt)) = target {
                        let saved = save_to(ui, st, &out, actual_fmt, true);
                        let mut s = st.borrow_mut();
                        s.quick_ok = saved;
                        if saved {
                            s.export_format = actual_fmt;
                            ui.set_export_format(format_index(actual_fmt));
                            sync_export_format_ui(ui, &s);
                        }
                    }
                }
                1 => {
                    let d = save_image_filters(
                        suggested_save_dialog(chosen_fmt, path.as_deref(), last_dir.as_deref()),
                        chosen_fmt,
                        zh,
                        false,
                    );
                    if let Some(raw) = d.save_file() {
                        let (out, actual_fmt) = studio_core::resolve_save_target(raw, chosen_fmt);
                        let saved = save_to(ui, st, &out, actual_fmt, true);
                        let mut s = st.borrow_mut();
                        s.quick_ok = saved;
                        if saved {
                            s.export_format = actual_fmt;
                            ui.set_export_format(format_index(actual_fmt));
                            sync_export_format_ui(ui, &s);
                        }
                    }
                }
                2 => {
                    let d = save_image_filters(
                        suggested_save_dialog(chosen_fmt, path.as_deref(), last_dir.as_deref()),
                        chosen_fmt,
                        zh,
                        false,
                    );
                    if let Some(raw) = d.save_file() {
                        let (out, actual_fmt) = studio_core::resolve_save_target(raw, chosen_fmt);
                        let saved = save_to(ui, st, &out, actual_fmt, false);
                        if saved {
                            let mut s = st.borrow_mut();
                            s.export_format = actual_fmt;
                            ui.set_export_format(format_index(actual_fmt));
                            sync_export_format_ui(ui, &s);
                        }
                    }
                }
                _ => {
                    let src_for_name = convert_src.as_deref().or(path.as_deref());
                    let d = save_image_filters(
                        suggested_save_dialog(chosen_fmt, src_for_name, last_dir.as_deref()),
                        chosen_fmt,
                        zh,
                        false,
                    );
                    if let Some(raw) = d.save_file() {
                        let (out, actual_fmt) = studio_core::resolve_save_target(raw, chosen_fmt);
                        let saved = if let Some(ref src) = convert_src {
                            convert_file_to(ui, st, src, &out, actual_fmt)
                        } else {
                            save_to(ui, st, &out, actual_fmt, false)
                        };
                        if saved {
                            let mut s = st.borrow_mut();
                            s.export_format = actual_fmt;
                            ui.set_export_format(format_index(actual_fmt));
                            sync_export_format_ui(ui, &s);
                        }
                    }
                }
            }
            return;
        }
        "toggle-grid" => {
            let on = !ui.get_show_grid();
            ui.set_show_grid(on);
            st.borrow_mut().show_grid = on;
        }
        "save-as" => {
            open_save_modal(ui, st, 1);
            return;
        }
        "export" => {
            open_save_modal(ui, st, 2);
            return;
        }
        "convert-file" => {
            open_save_modal(ui, st, 3);
            return;
        }
        "convert-pick-input" => {
            let mut d = open_image_filter(rfd::FileDialog::new(), zh);
            let start_dir = {
                let s = st.borrow();
                suggested_save_dir(
                    s.convert_input.as_deref().or(s.doc.path.as_deref()),
                    s.last_save_dir.as_deref(),
                )
            };
            if let Some(dir) = start_dir
                && dir.is_dir()
            {
                d = d.set_directory(dir);
            }
            if let Some(p) = d.pick_file() {
                let mut s = st.borrow_mut();
                s.convert_input = Some(p);
                refresh_convert_source_ui(ui, &mut s);
                sync_export_format_ui(ui, &s);
            }
            return;
        }
        "convert-use-current" => {
            let mut s = st.borrow_mut();
            s.convert_input = None;
            refresh_convert_source_ui(ui, &mut s);
            sync_export_format_ui(ui, &s);
            return;
        }
        "export-format-changed" => {
            let mut s = st.borrow_mut();
            FormatPanel::Export.sync_state_from_ui(ui, &mut s);
            FormatPanel::Export.sync_ui(ui, &s);
            return;
        }
        "export-quality-edited" => {
            let mut s = st.borrow_mut();
            FormatPanel::Export.handle_quality_edited(ui, &mut s);
            return;
        }
        "webp-lossless-toggled" => {
            let mut s = st.borrow_mut();
            FormatPanel::Export.handle_lossless_toggled(ui, &mut s);
            return;
        }
        "alpha-bg-changed" => {
            let mut s = st.borrow_mut();
            FormatPanel::Export.handle_alpha_changed(ui, &mut s);
            return;
        }
        "alpha-bg-hex" => {
            let mut s = st.borrow_mut();
            FormatPanel::Export.handle_alpha_hex(ui, &mut s);
            return;
        }
        "batch-format-changed" => {
            let mut s = st.borrow_mut();
            FormatPanel::Batch.sync_state_from_ui(ui, &mut s);
            FormatPanel::Batch.sync_ui(ui, &s);
            return;
        }
        "batch-quality-edited" => {
            let mut s = st.borrow_mut();
            FormatPanel::Batch.handle_quality_edited(ui, &mut s);
            return;
        }
        "batch-webp-lossless-toggled" => {
            let mut s = st.borrow_mut();
            FormatPanel::Batch.handle_lossless_toggled(ui, &mut s);
            return;
        }
        "batch-alpha-bg-changed" => {
            let mut s = st.borrow_mut();
            FormatPanel::Batch.handle_alpha_changed(ui, &mut s);
            return;
        }
        "batch-alpha-bg-hex" => {
            let mut s = st.borrow_mut();
            FormatPanel::Batch.handle_alpha_hex(ui, &mut s);
            return;
        }
        "undo" => {
            with_doc(ui, st, |d| {
                d.undo();
            });
            sync_layer_fields(ui, &st.borrow());
        }
        "redo" => {
            with_doc(ui, st, |d| {
                d.redo();
            });
            sync_layer_fields(ui, &st.borrow());
        }
        "paste" | "paste-layer" => {
            match arboard::Clipboard::new().and_then(|mut c| c.get_image()) {
                Ok(img) => {
                    let Some(rgba) = image::RgbaImage::from_raw(
                        img.width as u32,
                        img.height as u32,
                        img.bytes.into_owned(),
                    ) else {
                        return;
                    };
                    let dynimg = DynamicImage::ImageRgba8(rgba);
                    if a == "paste" {
                        if !new_doc(ui, st, dynimg) {
                            return;
                        }
                        ui.set_status(
                            tl(
                                zh,
                                "已將剪貼簿影像貼為新影像。",
                                "Pasted clipboard image as a new image.",
                            )
                            .into(),
                        );
                    } else {
                        let n = format!(
                            "{} {}",
                            tl(zh, "貼上", "Pasted"),
                            st.borrow().doc.layers.len() + 1
                        );
                        st.borrow_mut().doc.add_image_layer(n, dynimg);
                    }
                }
                Err(_) => match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
                    Ok(t) if !t.trim().is_empty() => {
                        let mut s = st.borrow_mut();
                        let n = format!("{} {}", tl(zh, "文字", "Text"), s.doc.layers.len() + 1);
                        s.doc.add_text(20.0, 20.0, n);
                        s.doc.with_selected(|l| {
                            if let ComposeLayer::Text(l) = l {
                                l.text = t;
                            }
                        });
                    }
                    _ => ui.set_status(
                        tl(zh, "剪貼簿沒有影像。", "No image on the clipboard.").into(),
                    ),
                },
            }
        }
        "copy" => {
            let s = st.borrow();
            let flat = s.doc.flattened().to_rgba8();
            let img = match s.doc.selection.and_then(|sel| {
                Selection {
                    circle: false,
                    ..sel
                }
                .mask(s.doc.width(), s.doc.height())
            }) {
                Some(tr::SelectionMask::Rectangle {
                    x,
                    y,
                    width,
                    height,
                }) => {
                    let mut cropped =
                        image::imageops::crop_imm(&flat, x, y, width, height).to_image();
                    let bitmap = s.doc.selection_bitmap();
                    for (px, py, pixel) in cropped.enumerate_pixels_mut() {
                        if bitmap.get_pixel(x + px, y + py).0[0] == 0 {
                            *pixel = image::Rgba([0, 0, 0, 0]);
                        }
                    }
                    cropped
                }
                _ => flat,
            };
            let r = arboard::Clipboard::new().and_then(|mut c| {
                c.set_image(arboard::ImageData {
                    width: img.width() as usize,
                    height: img.height() as usize,
                    bytes: img.into_raw().into(),
                })
            });
            ui.set_status(
                match r {
                    Ok(()) => tl(zh, "已複製到剪貼簿。", "Copied to clipboard."),
                    Err(e) => format!("{} {e}", tl(zh, "複製失敗：", "Copy failed:")),
                }
                .into(),
            );
        }
        "screenshot" => {
            ui.window().set_minimized(true);
            let weak = ui.as_weak();
            let st = st.clone();
            slint::Timer::single_shot(Duration::from_millis(450), move || {
                let Some(ui) = weak.upgrade() else { return };
                let r = crate::capture::capture_fullscreen();
                ui.window().set_minimized(false);
                match r {
                    Ok(img) => {
                        if !new_doc(&ui, &st, img) {
                            return;
                        }
                        ui.set_status(
                            tl(
                                zh,
                                "已擷取螢幕，可用「裁切」工具保留範圍。",
                                "Screen captured; use Crop to keep a region.",
                            )
                            .into(),
                        );
                    }
                    Err(e) => ui.set_status(
                        format!("{} {e}", tl(zh, "截圖失敗：", "Screenshot failed:")).into(),
                    ),
                }
                refresh(&ui, &st);
            });
            return;
        }
        "zoom-in" => zoom_at(st, 1.25, centre(ui)),
        "zoom-out" => zoom_at(st, 0.8, centre(ui)),
        "zoom-fit" => fit(ui, st),
        "zoom-100" => {
            let z = st.borrow().zoom;
            zoom_at(st, 1.0 / z, centre(ui));
        }
        "prev" | "next" => {
            let next = {
                let s = st.borrow();
                let n = s.folder.len();
                s.folder_idx.filter(|_| n > 1).map(|i| {
                    s.folder[if a == "next" {
                        (i + 1) % n
                    } else {
                        (i + n - 1) % n
                    }]
                    .clone()
                })
            };
            if let Some(p) = next {
                open_path(ui, st, &p);
            }
            return;
        }
        "toggle-mode" => {
            let mut s = st.borrow_mut();
            s.view_mode = !s.view_mode;
            s.panels_hidden = false;
            ui.set_panels_hidden(false);
        }
        "swap-colors" => {
            let mut s = st.borrow_mut();
            let t = &mut s.tools;
            std::mem::swap(&mut t.color1, &mut t.color2);
            ui.set_color1(to_color(t.color1));
            ui.set_color2(to_color(t.color2));
        }
        h if h.starts_with("hex:") => {
            let v = h[4..].trim().trim_start_matches('#');
            let p = |i: usize| v.get(i..i + 2).and_then(|x| u8::from_str_radix(x, 16).ok());
            if let (Some(r), Some(g), Some(b)) = (p(0), p(2), p(4)) {
                let c = [r, g, b, p(6).unwrap_or(255)];
                st.borrow_mut().tools.color1 = c;
                ui.set_color1(to_color(c));
            }
        }
        "grid" => st.borrow_mut().show_grid = ui.get_show_grid(),
        "layer-up" => with_doc(ui, st, |d| d.reorder_selected(1)),
        "layer-down" => with_doc(ui, st, |d| d.reorder_selected(-1)),
        "layer-dup" => with_doc(ui, st, |d| d.duplicate_selected()),
        "layer-delete" => with_doc(ui, st, |d| {
            d.delete_selected();
        }),
        "layer-merge-down" => {
            let mut ok = false;
            with_doc(ui, st, |d| ok = d.merge_selected_down());
            if ok {
                sync_layer_fields(ui, &st.borrow());
            } else {
                ui.set_status(
                    tl(
                        zh,
                        "請先選取上方圖層才能向下合併",
                        "Select a layer above another layer or the canvas to merge down",
                    )
                    .into(),
                );
            }
        }
        "layer-center" => with_doc(ui, st, |d| d.center_selected_layer()),
        "layer-flip-h" => with_doc(ui, st, |d| d.flip_selected_layer(true)),
        "layer-flip-v" => with_doc(ui, st, |d| d.flip_selected_layer(false)),
        "layer-rot" => {
            with_doc(ui, st, |d| d.rotate_selected_layer(true));
            sync_layer_fields(ui, &st.borrow());
        }
        "flatten" => with_doc(ui, st, |d| d.flatten()),
        "add-text" => {
            let n = format!(
                "{} {}",
                tl(zh, "文字", "Text"),
                st.borrow().doc.layers.len() + 1
            );
            with_doc(ui, st, |d| {
                d.add_text(d.width() as f32 * 0.1, d.height() as f32 * 0.1, n)
            });
            sync_layer_fields(ui, &st.borrow());
        }
        "add-image" => {
            if let Some(p) = open_image_filter(rfd::FileDialog::new(), zh).pick_file() {
                match studio_core::load_dynamic_image(&p) {
                    Ok(img) => {
                        let n = p
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        with_doc(ui, st, |d| d.add_image_layer(n, img));
                    }
                    Err(e) => ui.set_status(e.to_string().into()),
                }
            }
        }
        "rot-left" => with_doc(ui, st, |d| d.apply_transform(tr::rotate_left)),
        "rot-right" => with_doc(ui, st, |d| d.apply_transform(tr::rotate_right)),
        "rot-180" => with_doc(ui, st, |d| d.apply_transform(fl::rotate_180)),
        "flip-h" => with_doc(ui, st, |d| d.apply_transform(tr::flip_horizontal)),
        "flip-v" => with_doc(ui, st, |d| d.apply_transform(tr::flip_vertical)),
        "select-all" => with_doc(ui, st, |d| {
            d.focus_box(None);
            d.selection_mask = None;
            d.selection_path.clear();
            d.selection = Some(Selection {
                x: 0.0,
                y: 0.0,
                w: d.width() as f32,
                h: d.height() as f32,
                circle: false,
            })
        }),
        "deselect" => with_doc(ui, st, |d| {
            d.clear_selection();
            d.focus_box(None);
        }),
        "mosaic-selection" => {
            let block = ui.get_selection_mosaic_block() as u32;
            if block == 1 || !ui.get_has_selection() {
                return;
            }
            if !st.borrow().doc.layers.is_empty()
                && rfd::MessageDialog::new()
                    .set_title("Pixel Studio")
                    .set_description(tl(zh, "選區馬賽克會合併圖層。復原可還原所有圖層。繼續？", "Mosaicking the selection flattens the layers. Undo restores all layers. Continue?"))
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show() != rfd::MessageDialogResult::Yes
            {
                return;
            }
            with_doc(ui, st, |d| {
                d.mosaic_selection(block);
            });
        }
        "crop" => {
            let remove = ui.get_crop_mode() == 1;
            let needs_flatten = {
                let s = st.borrow();
                !s.doc.layers.is_empty()
                    && (s.doc.selection_mask.is_some()
                        || s.doc.selection.is_some_and(|sel| {
                            remove || sel.circle || !s.doc.selection_path.is_empty()
                        }))
            };
            if needs_flatten && rfd::MessageDialog::new()
                .set_title("Pixel Studio")
                .set_description(tl(zh, "此裁切會合併可見圖層以產生透明區域；復原可還原圖層。繼續？", "This crop flattens visible layers to create transparency. Undo restores the layers. Continue?"))
                .set_buttons(rfd::MessageButtons::YesNo)
                .show() != rfd::MessageDialogResult::Yes { return; }
            let ok = if remove {
                st.borrow_mut().doc.crop_selection(true)
            } else {
                st.borrow_mut().doc.crop_to_selection()
            };
            if !ok {
                ui.set_status(
                    tl(
                        zh,
                        "請先用「選取」或「裁切」工具拖曳出範圍。",
                        "Drag a selection with Select or Crop first.",
                    )
                    .into(),
                );
            }
            fit(ui, st);
        }
        "rs-w" | "rs-h" => {
            if ui.get_rs_keep() {
                let (w, h) = {
                    let s = st.borrow();
                    (s.doc.width() as f32, s.doc.height() as f32)
                };
                if a == "rs-w" {
                    ui.set_rs_h((ui.get_rs_w() as f32 * h / w).round().max(1.0) as i32);
                } else {
                    ui.set_rs_w((ui.get_rs_h() as f32 * w / h).round().max(1.0) as i32);
                }
            }
            return;
        }
        "resize" => {
            let (w, h) = (ui.get_rs_w().max(1) as u32, ui.get_rs_h().max(1) as u32);
            with_doc(ui, st, |d| {
                d.apply_transform(|i| studio_core::resize_exact(i, w, h))
            });
            fit(ui, st);
        }
        "canvas-size" => {
            let (w, h) = (ui.get_rs_w().max(1) as u32, ui.get_rs_h().max(1) as u32);
            let bg = st.borrow().tools.color2;
            with_doc(ui, st, |d| d.resize_canvas(w, h, bg));
            fit(ui, st);
        }
        "f-bright" => {
            let v = ui.get_a_bright() as i32;
            with_doc(ui, st, |d| d.apply_filter(|i| tr::adjust_brightness(i, v)))
        }
        "f-contrast" => {
            let v = ui.get_a_contrast();
            with_doc(ui, st, |d| d.apply_filter(|i| tr::adjust_contrast(i, v)))
        }
        "f-sat" => {
            let v = ui.get_a_sat();
            with_doc(ui, st, |d| d.apply_filter(|i| tr::adjust_saturation(i, v)))
        }
        "f-expo" => {
            let v = ui.get_a_expo();
            with_doc(ui, st, |d| d.apply_filter(|i| tr::adjust_exposure(i, v)))
        }
        "f-blur" => {
            let v = ui.get_a_blur();
            with_doc(ui, st, |d| d.apply_filter(|i| tr::blur(i, v)))
        }
        "f-sharp" => {
            let v = ui.get_a_sharp();
            with_doc(ui, st, |d| d.apply_filter(|i| tr::sharpen(i, v, 1)))
        }
        "f-hue" => {
            let v = ui.get_a_hue();
            with_doc(ui, st, |d| d.apply_filter(|i| fl::adjust_hue(i, v)))
        }
        "f-sepia" => {
            let v = ui.get_a_sepia();
            with_doc(ui, st, |d| d.apply_filter(|i| fl::sepia(i, v)))
        }
        "f-vig" => {
            let v = ui.get_a_vig();
            with_doc(ui, st, |d| d.apply_filter(|i| fl::vignette(i, v)))
        }
        "f-temp" => {
            let v = ui.get_a_temp();
            with_doc(ui, st, |d| d.apply_filter(|i| fl::adjust_temperature(i, v)))
        }
        "f-tint" => {
            let v = ui.get_a_tint();
            with_doc(ui, st, |d| d.apply_filter(|i| fl::adjust_tint(i, v)))
        }
        "f-post" => {
            let v = (ui.get_a_post() as u8).max(2);
            with_doc(ui, st, |d| d.apply_filter(|i| fl::posterize(i, v)))
        }
        "f-gray" => with_doc(ui, st, |d| d.apply_filter(tr::grayscale)),
        "f-invert" => with_doc(ui, st, |d| d.apply_filter(tr::invert)),
        "f-auto" => with_doc(ui, st, |d| d.apply_filter(fl::auto_contrast)),
        "f-look" => {
            let looks = ColorLook::all();
            let idx = (ui.get_pro_look().max(0) as usize).min(looks.len() - 1);
            let look = looks[idx];
            with_doc(ui, st, |d| {
                d.apply_filter(|i| studio_core::apply_color_look(i, look))
            });
        }
        "f-levels" => {
            let in_black = ui.get_a_lvl_black().clamp(0.0, 240.0) as u8;
            let gamma = ui.get_a_lvl_gamma().clamp(0.2, 3.0);
            let in_white = ui.get_a_lvl_white().clamp(f32::from(in_black) + 1.0, 255.0) as u8;
            with_doc(ui, st, |d| {
                d.apply_filter(|i| studio_core::adjust_levels(i, in_black, gamma, in_white, 0, 255))
            });
        }
        "f-shadows" => {
            let v = ui.get_a_shadows();
            with_doc(ui, st, |d| {
                d.apply_filter(|i| studio_core::adjust_shadows_highlights(i, v, 0.0))
            });
        }
        "f-highlights" => {
            let v = ui.get_a_highlights();
            with_doc(ui, st, |d| {
                d.apply_filter(|i| studio_core::adjust_shadows_highlights(i, 0.0, v))
            });
        }
        "f-vibrance" => {
            let v = ui.get_a_vibrance();
            with_doc(ui, st, |d| {
                d.apply_filter(|i| studio_core::adjust_vibrance(i, v))
            });
        }
        "f-clarity" => {
            let v = ui.get_a_clarity();
            with_doc(ui, st, |d| {
                d.apply_filter(|i| studio_core::adjust_clarity(i, v))
            });
        }
        "f-balance" => {
            let (r, g, b) = (ui.get_a_cb_r(), ui.get_a_cb_g(), ui.get_a_cb_b());
            with_doc(ui, st, |d| {
                d.apply_filter(|i| studio_core::adjust_color_balance(i, r, g, b))
            });
        }
        "f-threshold" => {
            let lvl = ui.get_a_thresh().clamp(0.0, 255.0) as u8;
            with_doc(ui, st, |d| {
                d.apply_filter(|i| studio_core::threshold(i, lvl))
            });
        }
        "f-tilt-shift" => {
            let (y, band, sigma) = (ui.get_e_ts_y(), ui.get_e_ts_band(), ui.get_e_ts_sigma());
            with_doc(ui, st, |d| {
                d.apply_filter(|i| studio_core::tilt_shift(i, y, band, sigma))
            });
        }
        "f-pixelate" => {
            let block = ui.get_e_pixelate().round().max(2.0) as u32;
            with_doc(ui, st, |d| {
                d.apply_filter(|i| studio_core::pixelate(i, block))
            });
        }
        "f-grain" => {
            let s_val = ui.get_e_grain();
            with_doc(ui, st, |d| {
                d.apply_filter(|i| studio_core::film_grain(i, s_val))
            });
        }
        "f-oil" => {
            let rad = ui.get_e_oil().round().clamp(1.0, 5.0) as u32;
            with_doc(ui, st, |d| {
                d.apply_filter(|i| studio_core::oil_paint(i, rad, 20))
            });
        }
        "f-emboss" => {
            let s_val = ui.get_e_emboss();
            with_doc(ui, st, |d| {
                d.apply_filter(|i| studio_core::emboss(i, s_val))
            });
        }
        "f-edge" => with_doc(ui, st, |d| {
            d.apply_filter(|i| studio_core::edge_detect(i, false))
        }),
        "f-sketch" => with_doc(ui, st, |d| {
            d.apply_filter(|i| studio_core::edge_detect(i, true))
        }),
        "f-remove-bg" => {
            let tol = ui.get_c_bg_tol();
            let edge = ui.get_c_bg_edge();
            let c2 = st.borrow().tools.color2;
            with_doc(ui, st, |d| {
                d.apply_remove_background([c2[0], c2[1], c2[2]], tol, 2.0, edge)
            });
        }
        "f-trim-alpha" => {
            with_doc(ui, st, |d| {
                let res = d.apply_auto_trim(TrimMode::Transparent);
                ui.set_rs_w(res.width as i32);
                ui.set_rs_h(res.height as i32);
            });
            fit(ui, st);
        }
        "f-trim-margin" => {
            with_doc(ui, st, |d| {
                let res = d.apply_auto_trim(TrimMode::CornerColor(16));
                ui.set_rs_w(res.width as i32);
                ui.set_rs_h(res.height as i32);
            });
            fit(ui, st);
        }
        "f-round-corners" => {
            let rad = ui.get_c_round();
            with_doc(ui, st, |d| {
                d.apply_filter(|i| studio_core::round_corners(i, rad))
            });
        }
        "f-drop-shadow" => {
            let sigma = ui.get_c_shadow_sigma();
            let pad = ui.get_c_shadow_pad().round().max(0.0) as u32;
            with_doc(ui, st, |d| {
                d.apply_transform(|i| {
                    studio_core::add_drop_shadow(i, 6, 8, sigma, [0, 0, 0, 140], pad)
                });
                ui.set_rs_w(d.width() as i32);
                ui.set_rs_h(d.height() as i32);
            });
            fit(ui, st);
        }
        "f-frame-card" => {
            let pad = ui.get_c_frame_pad().round().max(0.0) as u32;
            let rad = ui.get_c_frame_radius();
            let c2 = st.borrow().tools.color2;
            with_doc(ui, st, |d| {
                d.apply_frame_card(FrameCardOptions {
                    padding: pad,
                    corner_radius: rad,
                    shadow_sigma: 12.0,
                    shadow_offset_x: 0,
                    shadow_offset_y: 8,
                    background_color: c2,
                    border_width: 0,
                    border_color: [220, 224, 230, 255],
                });
                ui.set_rs_w(d.width() as i32);
                ui.set_rs_h(d.height() as i32);
            });
            fit(ui, st);
        }
        "reset" => with_doc(ui, st, |d| d.reset_to_original()),
        "st-fill" => {
            let c = st.borrow().tools.color1;
            restyle_selected_sticker(st, |s| s.fill = c);
            st.borrow_mut().sticker_style.fill = c;
            ui.set_st_fill(to_color(c));
        }
        "st-outline" => {
            let c = st.borrow().tools.color2;
            restyle_selected_sticker(st, |s| s.stroke = c);
            ui.set_st_outline(to_color(c));
        }
        "st-style" => {
            let (w, sh) = (ui.get_st_stroke(), ui.get_st_shadow());
            restyle_selected_sticker(st, |s| {
                s.stroke_width = w;
                s.shadow = sh;
            });
        }
        "tab-redact" => {}
        "cut" => {
            action(ui, st, "copy");
            action(ui, st, "delete");
            return;
        }
        "delete" => {
            // Repeated Delete after removing the last/active box must not fall
            // through to pixels or a previously selected composition layer.
            if st.borrow().doc.focused_box.is_some() || st.borrow().tool == Tool::RedactBox {
                with_doc(ui, st, |d| {
                    if let Some(i) = d.focused_box {
                        d.delete_box(i);
                    }
                });
                return;
            }
            let (has_layer, has_sel, bg) = {
                let s = st.borrow();
                (
                    s.doc.selected.is_some(),
                    s.doc.selection.is_some(),
                    s.tools.color2,
                )
            };
            if has_layer {
                action(ui, st, "layer-delete");
                return;
            }
            if has_sel {
                // Paint-style: clear the selected pixels to colour 2.
                with_doc(ui, st, |d| {
                    d.apply_filter(|i| {
                        DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                            i.width(),
                            i.height(),
                            image::Rgba(bg),
                        ))
                    })
                });
            }
        }
        "print" => {
            let img = st.borrow().doc.flattened();
            let path = std::env::temp_dir().join(studio_core::naming::timestamp_name("png"));
            let r = img
                .save(&path)
                .map_err(|e| e.to_string())
                .and_then(|_| print_file(&path));
            ui.set_status(
                match r {
                    Ok(()) => tl(
                        zh,
                        "已送出列印（系統列印對話框）。",
                        "Sent to the system print dialog.",
                    ),
                    Err(e) => format!("{} {e}", tl(zh, "列印失敗：", "Print failed:")),
                }
                .into(),
            );
            return;
        }
        "properties" => {
            refresh_info(ui, st);
            ui.set_props_text(format!("{}\n{}", ui.get_info_text(), ui.get_c2pa_text()).into());
            ui.set_props_open(true);
            return;
        }
        "exit" => {
            if confirm_discard(st) {
                let _ = slint::quit_event_loop();
            }
            return;
        }
        "toggle-rulers" => {
            let mut s = st.borrow_mut();
            s.rulers = !s.rulers;
            ui.set_show_rulers(s.rulers);
            save_settings(&s);
        }
        "toggle-status" => {
            let mut s = st.borrow_mut();
            s.status_bar = !s.status_bar;
            ui.set_show_status(s.status_bar);
            save_settings(&s);
            return;
        }
        "toggle-inspector" => {
            let (view, collapsed, hidden) = {
                let s = st.borrow();
                (s.view_mode, s.inspector_collapsed, s.panels_hidden)
            };
            if !view {
                // From focus mode, F4 brings the inspector back expanded.
                set_panel_layout(ui, st, !collapsed && !hidden, false);
            }
            return;
        }
        "toggle-panels" => {
            let (view, collapsed, hidden) = {
                let s = st.borrow();
                (s.view_mode, s.inspector_collapsed, s.panels_hidden)
            };
            if !view {
                set_panel_layout(ui, st, collapsed, !hidden);
            }
            return;
        }
        "toggle-toolbar" => {
            ui.set_tb_mode(if ui.get_tb_mode() == 2 { 0 } else { 2 });
            ui.invoke_toolbar_moved();
            return;
        }
        "fullscreen" => {
            let on = !ui.window().is_fullscreen();
            ui.window().set_fullscreen(on);
            ui.set_full_screen_on(on);
            return;
        }
        "resize-dialog" => {
            let s = st.borrow();
            ui.set_rs_w(s.doc.width() as i32);
            ui.set_rs_h(s.doc.height() as i32);
            ui.set_rs_open(true);
            return;
        }
        "c2pa-dialog" => {
            refresh_info(ui, st);
            ui.set_c2pa_open(true);
            return;
        }
        "shortcuts" => {
            ui.set_keys_open(true);
            return;
        }
        "about" => {
            ui.set_about_open(true);
            return;
        }
        a if a.starts_with("tab:") => {
            let n: i32 = a[4..].parse().unwrap_or(0);
            if st.borrow().view_mode {
                action(ui, st, "toggle-mode");
            }
            ui.set_tab(n);
            set_panel_layout(ui, st, false, false);
            match n {
                2 => {
                    let mut s = st.borrow_mut();
                    maybe_refresh_histogram(ui, &mut s);
                }
                4 => action(ui, st, "tab-redact"),
                5 => refresh_info(ui, st),
                7 => action(ui, st, "refresh-shell"),
                _ => {}
            }
            return;
        }
        a if a.starts_with("recent:") => {
            let i: usize = a[7..].parse().unwrap_or(usize::MAX);
            let p = st.borrow().recent.get(i).cloned();
            if let Some(p) = p {
                open_path(ui, st, &p);
            }
            return;
        }
        "tool-redact-box" | "tool-redact-brush" => {
            let t = if a == "tool-redact-box" {
                Tool::RedactBox
            } else {
                Tool::RedactBrush
            };
            st.borrow_mut().tool = t;
            st.borrow_mut().doc.focus_box(None);
            ui.set_tool(t.index());
        }
        "detect" => start_detection(ui, st),
        "sel-sensitive" => with_doc(ui, st, |d| {
            preselect_sensitive(&mut d.boxes);
        }),
        "sel-all-boxes" => with_doc(ui, st, |d| {
            d.boxes.iter_mut().for_each(|b| b.selected = true)
        }),
        "sel-no-boxes" => with_doc(ui, st, |d| {
            d.boxes.iter_mut().for_each(|b| b.selected = false)
        }),
        "clear-boxes" => with_doc(ui, st, |d| {
            if !d.boxes.is_empty() {
                d.checkpoint();
                d.boxes.clear();
                d.focus_box(None);
            }
        }),
        "sel-keyword" => {
            let k: String = ui.get_redact_keyword().into();
            let mut n = 0;
            with_doc(ui, st, |d| n = select_by_keyword(&mut d.boxes, &k));
            ui.set_status(format!("{} {n}", tl(zh, "符合關鍵字：", "Keyword matches:")).into());
        }
        "apply-redact" => {
            let style = match ui.get_redact_style() {
                1 => RedactStyle::Mosaic(ui.get_mosaic_block() as u32),
                2 => RedactStyle::Blur(ui.get_blur_sigma()),
                _ => RedactStyle::Solid(st.borrow().tools.color1),
            };
            let pad = ui.get_redact_padding() as u32;
            let mut n = 0;
            with_doc(ui, st, |d| n = d.apply_redaction(style, pad));
            ui.set_status(
                format!(
                    "{n} {}",
                    tl(
                        zh,
                        "個區域已打碼（已合併圖層；匯出時預設移除中繼資料）。",
                        "regions redacted (layers flattened; metadata stripped on export by default)."
                    )
                )
                .into(),
            );
        }
        "refresh-hist" => {
            let mut s = st.borrow_mut();
            maybe_refresh_histogram(ui, &mut s);
            return;
        }
        "refresh-info" => refresh_info(ui, st),
        "c2pa-write" => {
            c2pa_save_copy(ui, st);
            return;
        }
        "c2pa-remove" => {
            c2pa_remove_copy(ui, st);
            return;
        }
        "c2pa-in" | "c2pa-out" => {
            if let Some(d) = rfd::FileDialog::new().pick_folder() {
                let v = SharedString::from(d.display().to_string());
                if a == "c2pa-in" {
                    ui.set_c2pa_in(v)
                } else {
                    ui.set_c2pa_out(v)
                }
            }
            return;
        }
        "batch-in" | "batch-out" => {
            if let Some(d) = rfd::FileDialog::new().pick_folder() {
                let s = SharedString::from(d.display().to_string());
                if a == "batch-in" {
                    ui.set_batch_in(s)
                } else {
                    ui.set_batch_out(s)
                }
            }
            return;
        }
        "batch-run" | "batch-c2pa-add" | "batch-c2pa-remove" => {
            start_batch(ui, st, a);
            return;
        }
        "c2pa-multi-pick" => {
            if let Some(mut f) = open_image_filter(rfd::FileDialog::new(), zh).pick_files() {
                f.sort();
                set_c2pa_files(ui, st, f);
            }
            return;
        }
        "c2pa-multi-clear" => {
            set_c2pa_files(ui, st, Vec::new());
            return;
        }
        "c2pa-multi-parent" => {
            if let Some(d) = rfd::FileDialog::new().pick_folder() {
                ui.set_c2pa_multi_parent(d.display().to_string().into());
            }
            return;
        }
        "c2pa-multi-add" | "c2pa-multi-remove" => {
            start_c2pa_multi(ui, st, a == "c2pa-multi-add");
            return;
        }
        "c2pa-open-result" => {
            let dir: String = ui.get_c2pa_result_dir().into();
            if !dir.is_empty()
                && let Err(e) = open_folder(Path::new(&dir))
            {
                ui.set_status(
                    format!("{}: {e}", tl(zh, "無法開啟資料夾", "Cannot open folder")).into(),
                );
            }
            return;
        }
        "gif-pick" => {
            if let Some(mut f) = open_image_filter(rfd::FileDialog::new(), zh).pick_files() {
                f.sort();
                ui.set_gif_frames(format!("{} {}", f.len(), tl(zh, "張影格", "frames")).into());
                st.borrow_mut().gif_frames = f;
            }
            return;
        }
        "gif-run" => {
            let frames = st.borrow().gif_frames.clone();
            if frames.len() < 2 {
                ui.set_status(tl(zh, "請至少選擇 2 張影格。", "Pick at least 2 frames.").into());
                return;
            }
            if let Some(out) = rfd::FileDialog::new()
                .add_filter("GIF", &["gif"])
                .set_file_name(studio_core::naming::timestamp_name("gif"))
                .save_file()
            {
                let m = ui.get_gif_max();
                let opts = GifOptions {
                    delay_ms: ui.get_gif_delay() as u32,
                    repeat: ui.get_gif_repeat(),
                    max_width: (m > 0).then_some(m as u32),
                    max_height: (m > 0).then_some(m as u32),
                    background: [255, 255, 255, 255],
                };
                ui.set_status(
                    match studio_core::write_animated_gif(&frames, &out, opts) {
                        Ok(r) => format!(
                            "GIF: {} ({} frames, {}×{})",
                            r.output.display(),
                            r.frame_count,
                            r.width,
                            r.height
                        ),
                        Err(e) => format!("GIF: {e}"),
                    }
                    .into(),
                );
            }
            return;
        }
        "pano-run" => {
            if let Some(out) = rfd::FileDialog::new()
                .add_filter("JPEG", &["jpg", "jpeg", "jfif"])
                .set_file_name(studio_core::naming::timestamp_name("jpg"))
                .save_file()
            {
                let w = ui.get_pano_width();
                let opts = PanoramaOptions {
                    mode: [PanoramaMode::Pad, PanoramaMode::Stretch, PanoramaMode::Crop]
                        [ui.get_pano_mode().clamp(0, 2) as usize],
                    target_width: (w > 0).then_some(w as u32),
                    quality: ui.get_quality() as u8,
                    background: [0, 0, 0, 255],
                };
                let (img, input) = {
                    let s = st.borrow();
                    (s.doc.flattened(), s.doc.path.clone().unwrap_or_default())
                };
                ui.set_status(
                    match studio_core::write_panorama_dynamic_jpeg(&img, &out, opts, input) {
                        Ok(_) => format!(
                            "{} {}",
                            tl(zh, "全景已輸出：", "Panorama written:"),
                            out.display()
                        ),
                        Err(e) => format!("Panorama: {e}"),
                    }
                    .into(),
                );
            }
            return;
        }
        "refresh-shell" => {
            ui.set_shell_state(
                if shell::is_registered() {
                    tl(
                        zh,
                        "已註冊右鍵選單（以 Pixel Studio 編輯／開啟）。",
                        "Right-click menu is registered.",
                    )
                } else {
                    tl(
                        zh,
                        "尚未註冊右鍵選單。",
                        "Right-click menu is not registered.",
                    )
                }
                .into(),
            );
            return;
        }
        "shell-register" | "shell-unregister" => {
            let r = if a == "shell-register" {
                shell::register(zh)
            } else {
                shell::unregister()
            };
            ui.set_status(r.unwrap_or_else(|e| e.to_string()).into());
            action(ui, st, "refresh-shell");
            return;
        }
        _ => {}
    }
    refresh(ui, st);
}

fn start_detection(ui: &AppWindow, st: &S) {
    if st.borrow().job.is_some() {
        return;
    }
    let revision = st.borrow().doc.revision();
    let boxes_before = st.borrow().doc.boxes.clone();
    let zh = st.borrow().zh;
    let img = st.borrow().doc.flattened();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let r = (|| -> anyhow::Result<_> {
            let dir = studio_core::ocr::ensure_models()?;
            studio_core::ocr::TextDetector::load(&dir)?.detect(&img)
        })()
        .map_err(|e| e.to_string());
        let _ = tx.send(r);
    });
    st.borrow_mut().job = Some(Job::Detect(revision, boxes_before, rx));
    ui.set_detecting(true);
    ui.set_redact_info(
        tl(
            zh,
            "偵測中…（首次使用會下載約 12 MB 離線模型）",
            "Detecting… (first use downloads ~12 MB of offline models)",
        )
        .into(),
    );
}

fn start_batch(ui: &AppWindow, st: &S, kind: &str) {
    if st.borrow().batch_job.is_some() {
        return;
    }
    let zh = st.borrow().zh;
    let is_c2pa = kind.starts_with("batch-c2pa");
    let (input, output): (String, String) = if is_c2pa {
        (ui.get_c2pa_in().into(), ui.get_c2pa_out().into())
    } else {
        (ui.get_batch_in().into(), ui.get_batch_out().into())
    };
    if input.is_empty() || output.is_empty() {
        ui.set_batch_progress(
            tl(
                zh,
                "請選擇來源與輸出資料夾。",
                "Choose input and output folders.",
            )
            .into(),
        );
        return;
    }
    let files = studio_core::collect_supported_files(&input);
    let out = PathBuf::from(output);
    let watermark: String = ui.get_batch_watermark().into();
    let opts = BatchOptions {
        output_dir: out.clone(),
        convert: batch_convert_options(ui, &st.borrow()),
        max_width: (ui.get_batch_max_w() > 0).then_some(ui.get_batch_max_w() as u32),
        max_height: (ui.get_batch_max_h() > 0).then_some(ui.get_batch_max_h() as u32),
        watermark_text: (!watermark.trim().is_empty()).then_some(watermark),
        watermark_opacity: 0.5,
        watermark_font_size: 32.0,
    };
    let draft = c2pa_draft(ui);
    spawn_batch(ui, st, kind, files, out, opts, draft, None);
}

/// Pick (or clear) the explicit multi-image C2PA selection and refresh its summary.
fn set_c2pa_files(ui: &AppWindow, st: &S, files: Vec<PathBuf>) {
    let zh = st.borrow().zh;
    let text = if files.is_empty() {
        String::new()
    } else {
        let names: Vec<String> = files
            .iter()
            .take(4)
            .map(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            })
            .collect();
        let more = if files.len() > 4 { "…" } else { "" };
        if zh {
            format!("已選 {} 張：{}{more}", files.len(), names.join("、"))
        } else {
            format!("{} selected: {}{more}", files.len(), names.join(", "))
        }
    };
    if let Some(parent) = files.first().and_then(|p| p.parent()) {
        ui.set_c2pa_multi_parent(parent.display().to_string().into());
    }
    ui.set_c2pa_multi_count(files.len() as i32);
    ui.set_c2pa_multi_text(text.into());
    ui.set_c2pa_result_dir("".into());
    st.borrow_mut().c2pa_files = files;
}

/// Add or strip C2PA for the explicitly picked files, saving them together.
fn start_c2pa_multi(ui: &AppWindow, st: &S, add: bool) {
    if st.borrow().batch_job.is_some() {
        return;
    }
    let zh = st.borrow().zh;
    let files = st.borrow().c2pa_files.clone();
    if files.is_empty() {
        ui.set_c2pa_batch_text(tl(zh, "請先選擇圖片。", "Pick images first.").into());
        return;
    }
    let typed: String = ui.get_c2pa_multi_parent().into();
    let parent = if typed.trim().is_empty() {
        files[0]
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        PathBuf::from(typed.trim())
    };
    let created = if ui.get_c2pa_new_folder() {
        let prefix = if add {
            "pixelstudio_c2pa_added"
        } else {
            "pixelstudio_c2pa_removed"
        };
        studio_core::create_unique_output_dir(&parent, prefix)
    } else {
        std::fs::create_dir_all(&parent).map(|_| parent.clone())
    };
    let out = match created {
        Ok(d) => d,
        Err(e) => {
            ui.set_c2pa_batch_text(
                format!(
                    "{} {}: {e}",
                    tl(zh, "無法建立資料夾", "Cannot create folder"),
                    parent.display()
                )
                .into(),
            );
            return;
        }
    };
    let opts = BatchOptions {
        output_dir: out.clone(),
        convert: batch_convert_options(ui, &st.borrow()),
        max_width: None,
        max_height: None,
        watermark_text: None,
        watermark_opacity: 0.5,
        watermark_font_size: 32.0,
    };
    let kind = if add {
        "batch-c2pa-add"
    } else {
        "batch-c2pa-remove"
    };
    let draft = c2pa_draft(ui);
    ui.set_c2pa_result_dir("".into());
    spawn_batch(ui, st, kind, files, out.clone(), opts, draft, Some(out));
}

fn open_folder(dir: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    let mut cmd = std::process::Command::new("explorer");
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(dir).spawn().map(|_| ())
}

#[allow(clippy::too_many_arguments)]
fn spawn_batch(
    ui: &AppWindow,
    st: &S,
    kind: &str,
    files: Vec<PathBuf>,
    out: PathBuf,
    opts: BatchOptions,
    draft: C2paManifestDraft,
    result_dir: Option<PathBuf>,
) {
    let zh = st.borrow().zh;
    let is_c2pa = kind.starts_with("batch-c2pa");
    let kind = kind.to_owned();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let ptx = std::sync::Mutex::new(tx.clone());
        let progress = |c: usize, t: usize| {
            if let Ok(tx) = ptx.lock() {
                let _ = tx.send(BatchMsg::Progress(c, t, is_c2pa));
            }
        };
        let summary = match kind.as_str() {
            "batch-c2pa-add" => studio_core::process_c2pa_add_batch_with_progress(
                &files,
                &C2paAddOptions {
                    output_dir: out,
                    draft,
                },
                progress,
            ),
            "batch-c2pa-remove" => studio_core::process_c2pa_removal_batch_with_progress(
                &files,
                &C2paRemovalOptions { output_dir: out },
                progress,
            ),
            _ => studio_core::process_batch_with_progress(&files, &opts, progress),
        };
        let mut msg = format!(
            "{} / {} OK, {} failed",
            summary.succeeded, summary.total, summary.failed
        );
        if let Some(d) = &result_dir {
            let label = if zh {
                "輸出資料夾"
            } else {
                "Output folder"
            };
            msg = format!("{label}: {}\n{msg}", d.display());
        }
        for r in summary.results.iter().take(8) {
            msg += &match (&r.output, &r.error) {
                (Some(o), None) => format!("\nOK {}", o.display()),
                (_, Some(e)) => format!("\nFailed {}: {e}", r.input.display()),
                _ => format!("\nSkipped {}", r.input.display()),
            };
        }
        let done_dir = result_dir.filter(|_| summary.succeeded > 0);
        let _ = tx.send(BatchMsg::Done(msg, is_c2pa, done_dir));
    });
    st.borrow_mut().batch_job = Some(Job::Batch(rx));
    ui.set_batch_running(true);
    ui.set_batch_progress(tl(zh, "處理中…", "Working…").into());
}

fn poll_jobs(ui: &AppWindow, st: &S) {
    let mut changed = false;
    {
        let mut s = st.borrow_mut();
        let zh = s.zh;
        let mut detect_done = None;
        if let Some(Job::Detect(revision, boxes_before, rx)) = &s.job {
            match rx.try_recv() {
                Ok(r) => {
                    detect_done = Some(
                        if *revision == s.doc.revision()
                            && *boxes_before == s.doc.boxes
                            && s.gesture.is_none()
                        {
                            r
                        } else {
                            Err(tl(
                                zh,
                                "文件已變更，請重新辨識。",
                                "Document changed; run detection again.",
                            ))
                        },
                    )
                }
                // The worker panicked (unwind) or dropped its sender: don't leave
                // the UI stuck in "detecting".
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    detect_done = Some(Err("worker stopped unexpectedly".into()))
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(r) = detect_done {
            s.job = None;
            ui.set_detecting(false);
            match r {
                Ok(boxes) => {
                    let sel = boxes.iter().filter(|b| b.selected).count();
                    ui.set_redact_info(
                        if zh {
                            format!("偵測到 {} 個文字區塊，已預選 {sel} 個敏感資料（Email、電話、身分證、卡號、網址、地址）。", boxes.len())
                        } else {
                            format!("Detected {} text regions; {sel} sensitive ones pre-selected.", boxes.len())
                        }
                        .into(),
                    );
                    s.doc.boxes = boxes;
                    s.doc.focus_box(None);
                    changed = true;
                }
                Err(e) => ui.set_redact_info(
                    format!("{} {e}", tl(zh, "偵測失敗：", "Detection failed:")).into(),
                ),
            }
        }
        let mut batch_done = false;
        if let Some(Job::Batch(rx)) = &s.batch_job {
            loop {
                let m = match rx.try_recv() {
                    Ok(m) => m,
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        if !batch_done {
                            ui.set_batch_running(false);
                            ui.set_batch_progress(
                                tl(
                                    zh,
                                    "批次工作意外中止。",
                                    "Batch worker stopped unexpectedly.",
                                )
                                .into(),
                            );
                            batch_done = true;
                        }
                        break;
                    }
                };
                match m {
                    BatchMsg::Progress(c, t, c2) => {
                        let m = SharedString::from(format!("{c} / {t}"));
                        if c2 {
                            ui.set_c2pa_batch_text(m)
                        } else {
                            ui.set_batch_progress(m)
                        }
                    }
                    BatchMsg::Done(msg, c2, dir) => {
                        if let Some(d) = dir {
                            ui.set_c2pa_result_dir(d.display().to_string().into());
                        }
                        if c2 {
                            ui.set_c2pa_batch_text(msg.into())
                        } else {
                            ui.set_batch_progress(msg.into())
                        }
                        ui.set_batch_running(false);
                        batch_done = true;
                    }
                }
            }
        }
        if batch_done {
            s.batch_job = None;
        }
    }
    if changed {
        refresh(ui, st);
    }
}

/// Hand an image file to the OS print path (Windows shell "print" verb, `lp` elsewhere).
fn print_file(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "Start-Process", "-FilePath"])
        .arg(path)
        .args(["-Verb", "Print"])
        .status();
    #[cfg(not(windows))]
    let status = std::process::Command::new("lp").arg(path).status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("exit status {s}")),
        Err(e) => Err(e.to_string()),
    }
}

/// Single-letter tool shortcuts (index into `Tool::ALL`), Paint/Photoshop-like.
fn tool_for_key(t: &str, shift: bool) -> Option<i32> {
    Some(match (t, shift) {
        ("s", false) => 0,
        ("p", false) => 1,
        ("b", false) => 2,
        ("l", false) => 3,
        ("a", false) => 4,
        ("k", false) => 5,
        ("e", false) => 6,
        ("f", false) => 7,
        ("i", false) => 8,
        ("t", false) => 9,
        ("m", false) => 10,
        ("c", false) => 11,
        ("h", false) => 12,
        ("u", false) => 13,
        ("r", false) => 14,
        ("r", true) => 15,
        ("j", false) => 16,
        ("g", false) => 17,
        ("w", false) => 18,
        ("o", false) => 19,
        _ => return None,
    })
}

fn key(ui: &AppWindow, st: &S, text: &str, ctrl: bool, shift: bool) -> bool {
    use slint::platform::Key;
    if std::env::var_os("PIXEL_STUDIO_DEBUG").is_some() {
        eprintln!("key {:?} ctrl={ctrl} shift={shift}", text);
    }
    let is = |k: Key| text.starts_with(char::from(k)) && text.chars().count() == 1;
    let mut t = text.to_lowercase();
    // Some platforms deliver Ctrl+letter as the ASCII control character.
    if let Some(c) = t.chars().next()
        && t.chars().count() == 1
        && (c as u32) >= 1
        && (c as u32) <= 26
        && ctrl
    {
        t = ((b'a' + c as u8 - 1) as char).to_string();
    }
    if ctrl {
        let a = match t.as_str() {
            "x" => "cut",
            "e" if shift => "flatten",
            "e" => "export",
            "p" => "print",
            "r" => "toggle-rulers",
            "w" => "resize-dialog",
            "j" => "layer-dup",
            "m" if shift => "layer-merge-down",
            "]" => "layer-up",
            "[" => "layer-down",
            "z" if shift => "redo",
            "z" => "undo",
            "y" => "redo",
            "s" if shift => "save-as",
            "s" => "quick-save",
            "o" => "open",
            "n" => "new",
            "v" if shift => "paste-layer",
            "v" => "paste",
            "c" if shift => "convert-file",
            "c" => "copy",
            "a" => "select-all",
            "d" => "deselect",
            "g" => "toggle-grid",
            "=" | "+" => "zoom-in",
            "-" => "zoom-out",
            "0" => "zoom-fit",
            _ => return false,
        };
        action(ui, st, a);
        return true;
    }
    let nudge = if shift { 10 } else { 1 };
    let mv = |dx: i32, dy: i32| -> bool {
        if st.borrow().doc.selected.is_none() {
            return false;
        }
        {
            let mut s = st.borrow_mut();
            s.doc.nudge_selected(dx, dy);
            s.edit_session = None;
        }
        refresh(ui, st);
        true
    };
    if is(Key::Delete) || is(Key::Backspace) {
        if !ui.get_editor_key_focus() || ui.get_modal_open() {
            return false;
        }
        action(ui, st, "delete");
        return true;
    }
    if is(Key::F11) {
        action(ui, st, "fullscreen");
        return true;
    }
    if is(Key::F4) {
        if st.borrow().view_mode || ui.get_modal_open() {
            return false;
        }
        action(ui, st, "toggle-inspector");
        return true;
    }
    // Tab reaches here only when no focused widget consumed it; text fields keep focus traversal.
    if is(Key::Tab) && !shift {
        if st.borrow().view_mode || !ui.get_editor_key_focus() || ui.get_modal_open() {
            return false;
        }
        action(ui, st, "toggle-panels");
        return true;
    }
    if !st.borrow().view_mode {
        if let Some(i) = tool_for_key(&t, shift) {
            ui.set_tool(i);
            ui.invoke_tool_changed(i);
            return true;
        }
        if t == "x" {
            action(ui, st, "swap-colors");
            return true;
        }
    }
    if is(Key::LeftArrow) {
        if !mv(-nudge, 0) {
            action(ui, st, "prev");
        }
        return true;
    }
    if is(Key::RightArrow) {
        if !mv(nudge, 0) {
            action(ui, st, "next");
        }
        return true;
    }
    if is(Key::UpArrow) {
        return mv(0, -nudge);
    }
    if is(Key::DownArrow) {
        return mv(0, nudge);
    }
    if is(Key::Return) {
        action(ui, st, "crop");
        return true;
    }
    if is(Key::Escape) {
        {
            let mut s = st.borrow_mut();
            cancel_gesture(&mut s);
            s.doc.clear_selection();
            s.doc.selected = None;
            s.doc.focus_box(None);
            s.drag = Drag::None;
        }
        refresh(ui, st);
        return true;
    }
    match t.as_str() {
        "+" | "=" => action(ui, st, "zoom-in"),
        "-" => action(ui, st, "zoom-out"),
        "0" => action(ui, st, "zoom-fit"),
        "1" => action(ui, st, "zoom-100"),
        _ => return false,
    }
    true
}

fn wire(ui: &AppWindow, st: &S) {
    macro_rules! cb {
        ($method:ident, |$ui:ident, $st:ident $(, $arg:ident : $ty:ty)*| $body:expr) => {{
            let weak = ui.as_weak();
            let $st = st.clone();
            ui.$method(move |$($arg: $ty),*| {
                let $ui = weak.unwrap();
                $body
            });
        }};
    }
    cb!(on_action, |ui, st, a: SharedString| action(&ui, &st, &a));
    cb!(on_canvas_pointer, |ui,
                            st,
                            kind: i32,
                            button: i32,
                            x: f32,
                            y: f32,
                            shift: bool,
                            _ctrl: bool,
                            alt: bool| {
        pointer(&ui, &st, kind, button, x, y, shift, alt)
    });
    cb!(on_canvas_scroll, |ui,
                           st,
                           x: f32,
                           y: f32,
                           dy: f32,
                           ctrl: bool| {
        if ctrl {
            zoom_at(&st, if dy > 0.0 { 1.15 } else { 1.0 / 1.15 }, (x, y));
        } else {
            st.borrow_mut().pan.1 += dy;
        }
        refresh(&ui, &st)
    });
    cb!(on_canvas_resized, |ui, st| {
        let refit = std::mem::take(&mut st.borrow_mut().refit_pending);
        if refit {
            fit(&ui, &st);
        }
        sync_view(&ui, &st.borrow())
    });
    cb!(on_key, |ui,
                 st,
                 text: SharedString,
                 ctrl: bool,
                 shift: bool| key(
        &ui, &st, &text, ctrl, shift
    ));
    cb!(on_pick_color, |ui, st, c: Color, right: bool| {
        let mut s = st.borrow_mut();
        if right {
            s.tools.color2 = from_color(c);
            ui.set_color2(c);
        } else {
            s.tools.color1 = from_color(c);
            ui.set_color1(c);
        }
    });
    cb!(on_select_layer, |ui, st, row: i32| {
        {
            let mut s = st.borrow_mut();
            let n = s.doc.layers.len() as i32;
            let i = n - 1 - row;
            s.doc.selected = (i >= 0 && i < n).then_some(i as usize);
            s.doc.focus_box(None);
            s.edit_session = None;
            sync_layer_fields(&ui, &s);
        }
        refresh(&ui, &st)
    });
    cb!(on_toggle_layer, |ui, st, row: i32| {
        {
            let mut s = st.borrow_mut();
            let i = s.doc.layers.len() as i32 - 1 - row;
            if i >= 0 {
                s.doc.checkpoint();
                s.doc.selected = Some(i as usize);
                s.doc.with_selected(|l| match l {
                    ComposeLayer::Text(t) => t.visible = !t.visible,
                    ComposeLayer::Image(t) => t.visible = !t.visible,
                    ComposeLayer::Shape(t) => t.visible = !t.visible,
                });
            }
        }
        refresh(&ui, &st)
    });
    cb!(on_layer_edited, |ui, st, field: SharedString| {
        layer_edited(&ui, &st, &field);
        refresh(&ui, &st)
    });
    cb!(on_sticker_click, |ui, st, id: SharedString| sticker_insert(
        &ui, &st, &id, None
    ));
    cb!(on_sticker_drop, |ui,
                          st,
                          id: SharedString,
                          x: f32,
                          y: f32| {
        let p = to_img(&st.borrow(), x, y);
        sticker_insert(&ui, &st, &id, Some(p))
    });
    cb!(on_sticker_search, |ui, st| update_stickers(&ui, &st));
    cb!(on_toolbar_moved, |ui, st| {
        let mut s = st.borrow_mut();
        s.toolbar = (ui.get_tb_mode(), ui.get_tb_x(), ui.get_tb_y());
        save_settings(&s);
    });
    cb!(on_toggle_box, |ui, st, i: i32| {
        if let Some(b) = st.borrow_mut().doc.boxes.get_mut(i as usize) {
            b.selected = !b.selected;
        }
        refresh(&ui, &st)
    });
    cb!(on_select_box, |ui, st, i: i32| {
        let mut s = st.borrow_mut();
        cancel_gesture(&mut s);
        s.doc.focus_box(usize::try_from(i).ok());
        s.edit_session = None;
        s.tool = Tool::RedactBox;
        ui.set_tool(s.tool.index());
        drop(s);
        refresh(&ui, &st);
    });
    cb!(on_delete_box, |ui, st, i: i32| {
        let mut s = st.borrow_mut();
        cancel_gesture(&mut s);
        if let Ok(i) = usize::try_from(i) {
            s.doc.focus_box(Some(i));
            s.doc.delete_box(i);
        }
        s.edit_session = None;
        drop(s);
        refresh(&ui, &st);
    });
    cb!(on_apply_preset, |ui, st, i: i32| {
        let (_, w, h) = CANVAS_PRESETS[i.clamp(0, CANVAS_PRESETS.len() as i32 - 1) as usize];
        new_doc(&ui, &st, blank(w, h));
        refresh(&ui, &st)
    });
    cb!(on_text_style, |ui, st, i: i32| {
        {
            let mut s = st.borrow_mut();
            let c = s.tools.color1;
            let text: String = ui.get_style_text().into();
            s.doc.add_styled_text(i as usize, &text, c);
            sync_layer_fields(&ui, &s);
        }
        refresh(&ui, &st)
    });
    cb!(on_tool_changed, |ui, st, i: i32| {
        let t = Tool::from_index(i);
        cancel_gesture(&mut st.borrow_mut());
        st.borrow_mut().edit_session = None;
        st.borrow_mut().tool = t;
        st.borrow_mut().doc.focus_box(None);
        if t == Tool::Tape {
            ui.set_tab(0);
        }
        if t == Tool::Crop {
            ui.set_tab(2);
        }
        if matches!(t, Tool::RedactBox | Tool::RedactBrush) {
            ui.set_tab(4);
        }
        refresh(&ui, &st);
    });
    cb!(on_language_changed, |ui, st, i: i32| {
        let mut s = st.borrow_mut();
        s.zh = i == 0;
        T::get(&ui).set_zh(s.zh);
        set_language_models(&ui, &s);
        if s.save_dialog_mode == 3 {
            refresh_convert_source_ui(&ui, &mut s);
        }
        sync_export_format_ui(&ui, &s);
        sync_batch_format_ui(&ui, &s);
        s.hist_revision = None;
        maybe_refresh_histogram(&ui, &mut s);
        save_settings(&s);
    });
}

fn sticker_insert(ui: &AppWindow, st: &S, id: &str, at: Option<(f32, f32)>) {
    {
        let mut s = st.borrow_mut();
        let style = StickerStyle {
            stroke_width: ui.get_st_stroke(),
            shadow: ui.get_st_shadow(),
            ..s.sticker_style
        };
        s.doc.insert_sticker(id, at, style);
        s.tool = Tool::Select;
        ui.set_tool(Tool::Select.index());
        sync_layer_fields(ui, &s);
    }
    refresh(ui, st)
}

#[cfg(test)]
mod cursor_tests {
    use super::*;

    #[test]
    #[cfg(feature = "ui-testing")]
    fn crop_and_tape_pointer_workflow() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        T::get(&ui).set_zh(false);
        let st = Rc::new(RefCell::new(initial_state(
            &Settings::default(),
            false,
            false,
        )));
        {
            let mut s = st.borrow_mut();
            s.doc = Doc::new(100, 100);
            s.zoom = 1.0;
            s.pan = (0.0, 0.0);
        }
        wire(&ui, &st);
        ui.show().unwrap();
        ui.set_tool(Tool::Crop.index());
        ui.invoke_tool_changed(Tool::Crop.index());
        ui.set_select_shape(2);
        ui.set_crop_mode(1);
        ui.invoke_canvas_pointer(0, 0, 10.0, 10.0, false, false, false);
        ui.invoke_canvas_pointer(2, 0, 80.0, 10.0, false, false, false);
        ui.invoke_canvas_pointer(2, 0, 10.0, 80.0, false, false, false);
        ui.invoke_canvas_pointer(1, 0, 10.0, 80.0, false, false, false);
        assert!(ui.get_has_selection());
        assert!(!ui.get_selection_path().is_empty());
        ui.invoke_action("crop".into());
        assert_eq!(st.borrow().doc.image.get_pixel(20, 20).0[3], 0);
        assert_eq!(st.borrow().doc.image.get_pixel(80, 80).0[3], 255);
        ui.invoke_action("undo".into());
        assert_eq!(st.borrow().doc.image.get_pixel(20, 20).0[3], 255);
        {
            let mut s = st.borrow_mut();
            s.zoom = 1.0;
            s.pan = (0.0, 0.0);
        }
        ui.set_tool(Tool::Tape.index());
        ui.invoke_tool_changed(Tool::Tape.index());
        ui.set_tape_pattern(4);
        ui.invoke_canvas_pointer(0, 0, 20.0, 50.0, false, false, false);
        ui.invoke_canvas_pointer(2, 0, 70.0, 50.0, false, false, false);
        ui.invoke_canvas_pointer(1, 0, 70.0, 50.0, false, false, false);
        assert_eq!(st.borrow().doc.layers.len(), 1);
        assert_ne!(
            st.borrow().doc.flattened().to_rgba8().get_pixel(50, 50).0,
            [255, 255, 255, 255]
        );
        ui.invoke_action("undo".into());
        assert!(st.borrow().doc.layers.is_empty());
        ui.hide().unwrap();
    }

    #[test]
    #[cfg(feature = "ui-testing")]
    fn redaction_delete_ui_selection_focus_and_stale_detection() {
        use i_slint_backend_testing::ElementHandle;
        use slint::Model;
        use slint::platform::{Key, WindowEvent};
        use studio_core::redact::RedactBox;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        T::get(&ui).set_zh(false);
        let st = Rc::new(RefCell::new(initial_state(
            &Settings::default(),
            false,
            false,
        )));
        let mut auto = RedactBox::manual(20, 20, 30, 20);
        auto.text = "sample@example.com".into();
        auto.kind = Some(studio_core::redact::SensitiveKind::Email);
        let manual = RedactBox::manual(100, 20, 30, 20);
        {
            let mut s = st.borrow_mut();
            s.doc = Doc::new(200, 120);
            s.doc.boxes = vec![auto.clone(), manual.clone()];
            s.tool = Tool::RedactBox;
            s.zoom = 2.0;
            s.pan = (10.0, 15.0);
        }
        wire(&ui, &st);
        ui.set_tool(Tool::RedactBox.index());
        ui.set_tab(4);
        refresh(&ui, &st);
        ui.show().unwrap();
        // Same image-coordinate transform as the actual pointer callback.
        ui.invoke_canvas_pointer(0, 0, 70.0, 75.0, false, false, false);
        ui.invoke_canvas_pointer(1, 0, 70.0, 75.0, false, false, false);
        assert_eq!(st.borrow().doc.focused_box, Some(0));
        assert_eq!(st.borrow().doc.boxes, vec![auto.clone(), manual.clone()]);
        assert!(!st.borrow().doc.can_undo()); // selecting alone is not an edit

        // A list row selects one editing target without toggling apply checkboxes.
        ElementHandle::find_by_accessible_label(
            &ui,
            "Select redaction box: [Email] sample@example.com",
        )
        .next()
        .unwrap()
        .invoke_accessible_default_action();
        assert!(ui.get_editor_key_focus());
        let press = |key: Key| {
            let text: SharedString = char::from(key).to_string().into();
            ui.window()
                .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
            ui.window()
                .dispatch_event(WindowEvent::KeyReleased { text });
        };
        press(Key::Delete);
        assert_eq!(st.borrow().doc.boxes, vec![manual.clone()]);
        assert_eq!(ui.get_box_list().row_count(), 1);
        assert_eq!(ui.get_focused_box(), -1);
        let pixels = st.borrow().doc.image.clone();
        press(Key::Delete); // held/repeated key must not delete other content
        assert_eq!(st.borrow().doc.image, pixels);
        assert_eq!(st.borrow().doc.boxes, vec![manual.clone()]);
        ui.invoke_action("undo".into());
        assert_eq!(st.borrow().doc.focused_box, Some(0));
        assert_eq!(st.borrow().doc.boxes, vec![auto.clone(), manual.clone()]);
        ui.invoke_action("redo".into());
        assert_eq!(ui.get_box_list().row_count(), 1);
        ui.invoke_delete_box(0);
        assert!(!ui.get_show_boxes());
        assert_eq!(ui.get_box_list().row_count(), 0);
        ui.invoke_action("undo".into());

        // Empty canvas, Escape and switching to a layer/tool clear box focus.
        ui.invoke_select_box(0);
        ui.invoke_canvas_pointer(0, 0, 360.0, 200.0, false, false, false);
        ui.invoke_canvas_pointer(1, 0, 360.0, 200.0, false, false, false);
        assert_eq!(st.borrow().doc.focused_box, None);
        ui.invoke_select_box(0);
        ui.invoke_key(char::from(Key::Escape).to_string().into(), false, false);
        assert_eq!(st.borrow().doc.focused_box, None);
        ui.invoke_select_box(0);
        ui.invoke_tool_changed(Tool::Text.index());
        assert_eq!(st.borrow().doc.focused_box, None);

        // Text widgets must own Delete even at empty-string/cursor boundaries.
        {
            let mut s = st.borrow_mut();
            s.doc.add_text(5.0, 60.0, "text".into());
        }
        ui.set_tab(1);
        sync_layer_fields(&ui, &st.borrow());
        refresh(&ui, &st);
        st.borrow_mut().doc.focused_box = Some(0); // deliberate stale target
        let editor = ElementHandle::find_by_accessible_label(&ui, "Text content")
            .next()
            .unwrap();
        editor.mock_single_click(slint::platform::PointerEventButton::Left);
        assert!(!ui.get_editor_key_focus());
        for text in ["abc", ""] {
            ui.set_l_text(text.into());
            press(Key::End);
            press(Key::Delete);
            press(Key::Backspace);
            assert_eq!(
                ui.get_l_text().as_str(),
                if text.is_empty() { "" } else { "ab" }
            );
            assert_eq!(st.borrow().doc.boxes, vec![manual.clone()]);
            assert_eq!(st.borrow().doc.layers.len(), 1);
        }
        ui.set_tab(4);
        let keyword = ElementHandle::find_by_accessible_label(&ui, "Redaction keyword")
            .next()
            .unwrap();
        keyword.mock_single_click(slint::platform::PointerEventButton::Left);
        press(Key::Delete);
        press(Key::Backspace);
        assert_eq!(st.borrow().doc.boxes, vec![manual.clone()]);
        let box_label =
            ElementHandle::find_by_accessible_label(&ui, "Select redaction box: (30×20)")
                .next()
                .unwrap();
        box_label.invoke_accessible_default_action();
        assert!(ui.get_editor_key_focus());
        ui.set_qs_open(true);
        assert!(ui.get_editor_key_focus());
        assert!(!ui.invoke_key(char::from(Key::Delete).to_string().into(), false, false));
        assert_eq!(st.borrow().doc.boxes, vec![manual.clone()]);
        ui.set_qs_open(false);
        ui.set_c2pa_open(true);
        ui.set_c2pa_creator("abc".into());
        let creator = ElementHandle::find_by_accessible_label(&ui, "C2PA creator")
            .next()
            .unwrap();
        creator.mock_single_click(slint::platform::PointerEventButton::Left);
        press(Key::End);
        press(Key::Delete);
        press(Key::Backspace);
        assert_eq!(ui.get_c2pa_creator().as_str(), "ab");
        assert_eq!(st.borrow().doc.boxes, vec![manual.clone()]);
        ui.set_c2pa_open(false);

        // OCR completion must not resurrect a removed box or replace an active gesture.
        let (tx, rx) = channel();
        let original = st.borrow().doc.boxes.clone();
        let revision = st.borrow().doc.revision();
        st.borrow_mut().job = Some(Job::Detect(revision, original.clone(), rx));
        ui.invoke_delete_box(0);
        tx.send(Ok(original.clone())).unwrap();
        poll_jobs(&ui, &st);
        assert!(st.borrow().doc.boxes.is_empty());
        ui.invoke_action("undo".into());
        let (tx, rx) = channel();
        let mut s = st.borrow_mut();
        s.job = Some(Job::Detect(s.doc.revision(), s.doc.boxes.clone(), rx));
        s.gesture = Some(s.doc.begin_gesture());
        drop(s);
        tx.send(Ok(vec![auto])).unwrap();
        poll_jobs(&ui, &st);
        assert_eq!(st.borrow().doc.boxes, original);
        // Right-click removal uses the same transform at a zoom below 100%.
        {
            let mut s = st.borrow_mut();
            cancel_gesture(&mut s);
            s.tool = Tool::RedactBox;
            s.zoom = 0.5;
            s.pan = (-5.0, 30.0);
        }
        ui.set_tool(Tool::RedactBox.index());
        ui.invoke_canvas_pointer(0, 1, 52.5, 45.0, false, false, false);
        ui.invoke_canvas_pointer(1, 1, 52.5, 45.0, false, false, false);
        assert!(st.borrow().doc.boxes.is_empty());
        ui.invoke_action("undo".into());
        assert_eq!(st.borrow().doc.boxes, vec![manual]);
        ui.hide().unwrap();
    }

    #[test]
    fn failed_c2pa_export_preserves_previous_output() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("existing.unsupported");
        std::fs::write(&out, b"keep this original").unwrap();
        let image = DynamicImage::new_rgba8(8, 8);
        let result = save_with_c2pa(
            &image,
            &out,
            ConvertOptions {
                format: SupportedFormat::Png,
                ..Default::default()
            },
            &C2paManifestDraft::default(),
            None,
        );
        assert!(result.is_err());
        assert_eq!(std::fs::read(&out).unwrap(), b"keep this original");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn movable_boxes_and_handles_have_precise_cursor_targets() {
        let mut doc = Doc::new(300, 200);
        doc.add_shape(
            ShapeKind::Rectangle,
            (30.0, 30.0),
            (80.0, 80.0),
            &ToolSettings::default(),
        );
        let hover =
            |doc: &Doc, tool, drag, point| canvas_cursor(doc, tool, drag, (10.0, 20.0), 2.0, point);
        let point = Some((110.0, 120.0)); // canvas pixel (50, 50)
        assert_eq!(hover(&doc, Tool::Select, Drag::None, point), 1);
        assert_eq!(
            hover(&doc, Tool::Select, Drag::Move { last: (50.0, 50.0) }, point),
            2
        );
        assert_eq!(hover(&doc, Tool::Select, Drag::None, point), 1); // release/cancel
        assert_eq!(hover(&doc, Tool::Brush, Drag::None, point), 0); // tool switch
        assert_eq!(hover(&doc, Tool::Text, Drag::None, point), 0); // text tool creates, does not move
        assert_eq!(hover(&doc, Tool::Select, Drag::None, None), 0);
        assert_eq!(
            hover(&doc, Tool::Select, Drag::None, Some((410.0, 220.0))),
            0
        );
        assert_eq!(
            hover(&doc, Tool::Select, Drag::None, Some((613.0, 220.0))),
            4
        );
        assert_eq!(
            hover(&doc, Tool::Select, Drag::None, Some((310.0, 423.0))),
            5
        );
        doc.layers[0].set_visible(false);
        assert_eq!(hover(&doc, Tool::Select, Drag::None, point), 0);
        doc.boxes
            .push(studio_core::redact::RedactBox::manual(30, 30, 50, 50));
        assert_eq!(hover(&doc, Tool::RedactBox, Drag::None, point), 1);
        assert_eq!(
            hover(&doc, Tool::RedactBox, Drag::None, Some((170.0, 180.0))),
            3
        );
        assert_eq!(hover(&doc, Tool::Brush, Drag::None, point), 0);
        assert_eq!(hover(&doc, Tool::Hand, Drag::None, point), 1);
        assert_eq!(
            hover(
                &doc,
                Tool::Hand,
                Drag::Pan {
                    mouse: (0.0, 0.0),
                    pan: (0.0, 0.0)
                },
                point
            ),
            2
        );
    }
}

#[cfg(test)]
mod save_convert_tests {
    use super::*;

    #[test]
    fn parse_hex_and_alpha_bg_helpers() {
        assert_eq!(parse_hex_rgba("#112233"), Some([0x11, 0x22, 0x33, 255]));
        assert_eq!(parse_hex_rgba("aabbccdd"), Some([0xAA, 0xBB, 0xCC, 0xDD]));
        assert_eq!(parse_hex_rgba("#12345"), None);
        assert_eq!(parse_hex_rgba("not-hex"), None);

        assert_eq!(format_rgba_hex([0x11, 0x22, 0x33, 255]), "#112233");
        assert_eq!(format_rgba_hex([0x11, 0x22, 0x33, 0x80]), "#11223380");

        let c1 = [200, 10, 20, 255];
        let c2 = [10, 20, 200, 255];
        let custom = [30, 40, 50, 255];
        assert_eq!(resolve_alpha_bg(0, c1, c2, custom), [255, 255, 255, 255]);
        assert_eq!(resolve_alpha_bg(1, c1, c2, custom), [0, 0, 0, 255]);
        assert_eq!(resolve_alpha_bg(2, c1, c2, custom), c1);
        assert_eq!(resolve_alpha_bg(3, c1, c2, custom), c2);
        assert_eq!(resolve_alpha_bg(4, c1, c2, custom), custom);
    }

    #[test]
    fn suggested_save_dir_and_open_filters() {
        assert_eq!(
            suggested_save_dir(
                Some(Path::new("photo.png")),
                Some(Path::new("D:/last_exports"))
            ),
            Some(PathBuf::from("D:/last_exports"))
        );
        assert_eq!(
            suggested_save_dir(
                Some(Path::new("D:/src_dir/photo.png")),
                Some(Path::new("D:/last_exports"))
            ),
            Some(PathBuf::from("D:/src_dir"))
        );

        let entries = SupportedFormat::open_filter_entries(false);
        assert_eq!(entries.len(), SupportedFormat::ALL.len() + 1);
        assert_eq!(entries[0].0, "All Supported Images");
        assert!(entries[0].1.contains(&"jfif"));
        assert!(entries[0].1.contains(&"tga"));
        assert!(entries[0].1.contains(&"qoi"));

        let (p_no_ext, f_no_ext) =
            studio_core::resolve_save_target("out_dir/capture", SupportedFormat::Jfif);
        assert_eq!(p_no_ext, PathBuf::from("out_dir/capture.jfif"));
        assert_eq!(f_no_ext, SupportedFormat::Jfif);

        let (p_explicit, f_explicit) =
            studio_core::resolve_save_target("out_dir/capture.jfif", SupportedFormat::Jpeg);
        assert_eq!(p_explicit, PathBuf::from("out_dir/capture.jfif"));
        assert_eq!(f_explicit, SupportedFormat::Jfif);

        let (p_unknown, f_unknown) =
            studio_core::resolve_save_target("out_dir/capture.unknown", SupportedFormat::Qoi);
        assert_eq!(p_unknown, PathBuf::from("out_dir/capture.unknown.qoi"));
        assert_eq!(f_unknown, SupportedFormat::Qoi);
    }

    #[test]
    fn settings_serde_backward_compat_and_roundtrip() {
        let legacy: Settings =
            serde_json::from_str(r#"{"lang":"en","recent":[],"rulers":true}"#).unwrap();
        assert_eq!(legacy.export_format, None);
        assert_eq!(legacy.export_quality, None);
        assert_eq!(legacy.webp_lossless, None);
        assert_eq!(legacy.alpha_bg, None);
        assert_eq!(legacy.alpha_bg_mode, 0);
        assert_eq!(legacy.last_save_dir, None);

        let legacy_state = initial_state(&legacy, false, false);
        assert_eq!(legacy_state.export_format, SupportedFormat::Png);
        assert_eq!(legacy_state.export_quality, 90);
        assert!(legacy_state.webp_lossless);
        assert_eq!(legacy_state.alpha_bg, [255, 255, 255, 255]);
        assert_eq!(legacy_state.alpha_bg_mode, 0);

        let rich = Settings {
            lang: Some("zh-TW".into()),
            export_format: Some("jfif".into()),
            export_quality: Some(82),
            webp_lossless: Some(false),
            alpha_bg: Some([10, 20, 30, 255]),
            alpha_bg_mode: 4,
            last_save_dir: Some(PathBuf::from("D:/exports")),
            ..Default::default()
        };
        let encoded = serde_json::to_string(&rich).unwrap();
        let decoded: Settings = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.export_format.as_deref(), Some("jfif"));
        assert_eq!(decoded.export_quality, Some(82));
        assert_eq!(decoded.webp_lossless, Some(false));
        assert_eq!(decoded.alpha_bg, Some([10, 20, 30, 255]));
        assert_eq!(decoded.alpha_bg_mode, 4);
        assert_eq!(decoded.last_save_dir, Some(PathBuf::from("D:/exports")));

        let hydrated = initial_state(&decoded, true, false);
        assert_eq!(hydrated.export_format, SupportedFormat::Jfif);
        assert_eq!(hydrated.export_quality, 82);
        assert!(!hydrated.webp_lossless);
        assert_eq!(hydrated.alpha_bg, [10, 20, 30, 255]);
        assert_eq!(hydrated.alpha_bg_mode, 4);
        assert_eq!(hydrated.last_save_dir, Some(PathBuf::from("D:/exports")));
    }

    #[cfg(feature = "ui-testing")]
    fn make_test_ui(view_mode: bool) -> (AppWindow, S) {
        thread_local! {
            static SLINT_TEST_INIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        SLINT_TEST_INIT.with(|init| {
            if !init.get() {
                i_slint_backend_testing::init_no_event_loop();
                init.set(true);
            }
        });
        let ui = AppWindow::new().unwrap();
        T::get(&ui).set_zh(false);
        let st = Rc::new(RefCell::new(initial_state(
            &Settings::default(),
            false,
            view_mode,
        )));
        ui.set_format_names(ModelRc::new(VecModel::from(
            FORMATS
                .iter()
                .map(|f| SharedString::from(f.label()))
                .collect::<Vec<_>>(),
        )));
        init_export_ui(&ui, &st.borrow());
        set_language_models(&ui, &st.borrow());
        wire(&ui, &st);
        ui.show().unwrap();
        (ui, st)
    }

    #[test]
    #[cfg(feature = "ui-testing")]
    fn save_as_export_and_convert_capabilities_and_c2pa_guard() {
        let (ui, st) = make_test_ui(false);
        {
            let mut s = st.borrow_mut();
            let transparent = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                16,
                16,
                image::Rgba([0, 128, 255, 0]),
            ));
            s.doc.load(transparent, Some(PathBuf::from("photo.png")));
        }

        ui.invoke_action("save-as".into());
        assert!(ui.get_qs_open());
        assert_eq!(ui.get_save_dialog_mode(), 1);
        assert_eq!(ui.get_export_format(), format_index(SupportedFormat::Png));
        assert!(ui.get_export_supports_alpha());
        assert!(ui.get_export_supports_c2pa());
        assert!(!ui.get_export_supports_quality());
        assert!(!ui.get_export_is_webp());
        assert!(ui.get_export_alpha_present());
        assert_eq!(ui.get_export_target_preview().as_str(), "photo.png");

        ui.set_c2pa_on_save(true);
        ui.set_export_format(format_index(SupportedFormat::Jfif));
        ui.invoke_action("export-format-changed".into());
        assert!(ui.get_export_supports_quality());
        assert!(!ui.get_export_supports_alpha());
        assert!(ui.get_export_supports_c2pa());
        assert!(ui.get_c2pa_on_save());
        assert_eq!(ui.get_export_target_preview().as_str(), "photo.jfif");

        ui.set_export_format(format_index(SupportedFormat::Webp));
        ui.invoke_action("export-format-changed".into());
        assert!(ui.get_export_is_webp());
        assert!(ui.get_export_supports_quality());
        assert!(ui.get_export_supports_alpha());
        assert!(ui.get_export_supports_c2pa());
        assert_eq!(ui.get_export_target_preview().as_str(), "photo.webp");

        ui.set_export_format(format_index(SupportedFormat::Qoi));
        ui.invoke_action("export-format-changed".into());
        assert!(!ui.get_export_supports_quality());
        assert!(ui.get_export_supports_alpha());
        assert!(!ui.get_export_supports_c2pa());
        // Preserve user's C2PA preference even when previewing a non-C2PA format.
        assert!(ui.get_c2pa_on_save());
        assert_eq!(ui.get_export_target_preview().as_str(), "photo.qoi");

        ui.set_export_format(format_index(SupportedFormat::Bmp));
        ui.invoke_action("export-format-changed".into());
        assert!(!ui.get_export_supports_alpha());
        assert!(!ui.get_export_supports_c2pa());
        assert_eq!(ui.get_export_target_preview().as_str(), "photo.bmp");

        ui.set_export_format(format_index(SupportedFormat::Tga));
        ui.invoke_action("export-format-changed".into());
        assert!(ui.get_export_supports_alpha());
        assert!(!ui.get_export_supports_c2pa());
        assert_eq!(ui.get_export_target_preview().as_str(), "photo.tga");

        ui.hide().unwrap();
    }

    #[test]
    #[cfg(feature = "ui-testing")]
    fn alpha_bg_mode_and_hex_drive_format_options() {
        let (ui, st) = make_test_ui(false);
        {
            let mut s = st.borrow_mut();
            s.tools.color1 = [200, 10, 20, 255];
            s.tools.color2 = [10, 20, 200, 255];
        }

        ui.set_alpha_bg_mode(1);
        ui.invoke_action("alpha-bg-changed".into());
        assert_eq!(ui.get_alpha_bg_hex().as_str(), "#000000");
        assert_eq!(
            format_options(&ui, &st.borrow(), SupportedFormat::Jpeg).background,
            [0, 0, 0, 255]
        );

        ui.set_alpha_bg_mode(2);
        ui.invoke_action("alpha-bg-changed".into());
        assert_eq!(ui.get_alpha_bg_hex().as_str(), "#C80A14");
        assert_eq!(
            format_options(&ui, &st.borrow(), SupportedFormat::Bmp).background,
            [200, 10, 20, 255]
        );

        ui.set_alpha_bg_mode(3);
        ui.invoke_action("alpha-bg-changed".into());
        assert_eq!(ui.get_alpha_bg_hex().as_str(), "#0A14C8");
        assert_eq!(
            format_options(&ui, &st.borrow(), SupportedFormat::Jfif).background,
            [10, 20, 200, 255]
        );

        ui.set_alpha_bg_hex("#123456".into());
        ui.invoke_action("alpha-bg-hex".into());
        assert_eq!(ui.get_alpha_bg_mode(), 4);
        assert_eq!(
            format_options(&ui, &st.borrow(), SupportedFormat::Jpeg).background,
            [0x12, 0x34, 0x56, 255]
        );

        ui.set_alpha_bg_mode(0);
        ui.invoke_action("alpha-bg-changed".into());
        assert_eq!(ui.get_alpha_bg_hex().as_str(), "#FFFFFF");

        ui.set_alpha_bg_mode(4);
        ui.invoke_action("alpha-bg-changed".into());
        assert_eq!(ui.get_alpha_bg_hex().as_str(), "#123456");
        assert_eq!(
            format_options(&ui, &st.borrow(), SupportedFormat::Jpeg).background,
            [0x12, 0x34, 0x56, 255]
        );

        ui.hide().unwrap();
    }

    #[test]
    #[cfg(feature = "ui-testing")]
    fn save_to_and_convert_file_to_preserve_or_update_doc_state() {
        let dir = tempfile::tempdir().unwrap();
        let opened_path = dir.path().join("canvas.png");
        let source_path = dir.path().join("external.png");
        let export_qoi = dir.path().join("export.qoi");
        let converted_bmp = dir.path().join("converted.bmp");
        let converted_tga = dir.path().join("converted.tga");
        let saved_png = dir.path().join("saved_as.png");

        let mut src_img = image::RgbaImage::from_pixel(12, 12, image::Rgba([0, 0, 0, 0]));
        src_img.put_pixel(6, 6, image::Rgba([20, 210, 40, 255]));
        image::DynamicImage::ImageRgba8(src_img)
            .save(&source_path)
            .unwrap();

        let (ui, st) = make_test_ui(false);
        {
            let mut s = st.borrow_mut();
            s.doc = Doc::new(24, 24);
            s.doc.path = Some(opened_path.clone());
            s.doc.add_shape(
                ShapeKind::Rectangle,
                (2.0, 2.0),
                (10.0, 10.0),
                &ToolSettings::default(),
            );
        }

        // Unmodified source check & "convert-use-current"
        open_save_modal(&ui, &st, 3);
        assert!(st.borrow().doc.is_modified());
        assert_eq!(st.borrow().convert_input, None);
        assert!(!ui.get_convert_src_has_file());

        // ExportCopy (`update_doc == false`) leaves doc path, layers, and modified state untouched
        assert!(save_to(&ui, &st, &export_qoi, SupportedFormat::Qoi, false));
        let qoi_info = studio_core::inspect_image(&export_qoi).unwrap();
        assert_eq!(qoi_info.format, SupportedFormat::Qoi);
        assert_eq!(st.borrow().doc.path.as_ref(), Some(&opened_path));
        assert_eq!(st.borrow().doc.layers.len(), 1);
        assert!(st.borrow().doc.is_modified());
        assert_eq!(st.borrow().last_save_dir.as_deref(), Some(dir.path()));

        // Convert external transparent PNG -> BMP with custom alpha background
        ui.set_alpha_bg_hex("#102030".into());
        ui.invoke_action("alpha-bg-hex".into());
        assert!(convert_file_to(
            &ui,
            &st,
            &source_path,
            &converted_bmp,
            SupportedFormat::Bmp
        ));
        let bmp_loaded = image::open(&converted_bmp).unwrap().to_rgba8();
        assert_eq!(bmp_loaded.get_pixel(0, 0).0, [0x10, 0x20, 0x30, 255]);
        assert_eq!(bmp_loaded.get_pixel(6, 6).0, [20, 210, 40, 255]);
        assert_eq!(st.borrow().doc.path.as_ref(), Some(&opened_path));
        assert_eq!(st.borrow().doc.layers.len(), 1);

        // Convert to non-C2PA TGA even if C2PA toggle was set: skipped cleanly without failure
        ui.set_c2pa_on_save(true);
        assert!(convert_file_to(
            &ui,
            &st,
            &source_path,
            &converted_tga,
            SupportedFormat::Tga
        ));
        assert_eq!(
            studio_core::inspect_image(&converted_tga).unwrap().format,
            SupportedFormat::Tga
        );
        ui.set_c2pa_on_save(false);

        // Save (`update_doc == true`) updates doc path & title immediately
        assert!(save_to(&ui, &st, &saved_png, SupportedFormat::Png, true));
        assert_eq!(st.borrow().doc.path.as_ref(), Some(&saved_png));
        assert_eq!(ui.get_title_file().as_str(), "saved_as.png");
        assert!(saved_png.is_file());

        // When no layers exist and doc is cleanable, Save clears modified flag and sets convert_input when opening modal 3
        {
            let mut s = st.borrow_mut();
            s.doc = Doc::new(16, 16);
            s.doc.load(
                image::DynamicImage::new_rgba8(16, 16),
                Some(source_path.clone()),
            );
            s.doc.apply_transform(|i| i.brighten(10));
        }
        assert!(st.borrow().doc.is_modified());
        assert!(save_to(&ui, &st, &source_path, SupportedFormat::Png, true));
        assert!(!st.borrow().doc.is_modified());

        open_save_modal(&ui, &st, 3);
        assert_eq!(st.borrow().convert_input.as_ref(), Some(&source_path));
        assert!(ui.get_convert_src_has_file());
        ui.invoke_action("convert-use-current".into());
        assert_eq!(st.borrow().convert_input, None);
        assert!(!ui.get_convert_src_has_file());

        ui.hide().unwrap();
    }

    #[test]
    #[cfg(feature = "ui-testing")]
    fn save_export_and_convert_shortcuts_work_in_edit_and_view_modes() {
        for view_mode in [false, true] {
            let (ui, st) = make_test_ui(view_mode);

            assert!(key(&ui, &st, "s", true, true));
            assert!(ui.get_qs_open());
            assert_eq!(ui.get_save_dialog_mode(), 1);
            ui.set_qs_open(false);

            assert!(key(&ui, &st, "e", true, false));
            assert!(ui.get_qs_open());
            assert_eq!(ui.get_save_dialog_mode(), 2);
            ui.set_qs_open(false);

            assert!(key(&ui, &st, "c", true, true));
            assert!(ui.get_qs_open());
            assert_eq!(ui.get_save_dialog_mode(), 3);
            ui.set_qs_open(false);

            // Plain Ctrl+C copies instead of opening Convert dialog
            assert!(key(&ui, &st, "c", true, false));
            assert!(!ui.get_qs_open());

            assert_eq!(st.borrow().view_mode, view_mode);
            assert_eq!(ui.get_view_mode(), view_mode);
            ui.hide().unwrap();
        }
    }

    #[test]
    #[cfg(feature = "ui-testing")]
    fn quick_save_mode_locks_existing_file_and_updates_untitled_target() {
        let dir = tempfile::tempdir().unwrap();
        let existing_png = dir.path().join("photo.png");
        image::DynamicImage::new_rgba8(8, 8)
            .save(&existing_png)
            .unwrap();

        // Loaded disk file: Quick Save locks format, suppresses ellipsis, and keeps target synced.
        let (ui, st) = make_test_ui(false);
        open_path(&ui, &st, &existing_png);
        assert!(!st.borrow().quick_ok);

        ui.invoke_action("quick-save".into());
        assert!(ui.get_qs_open());
        assert_eq!(ui.get_save_dialog_mode(), 0);
        assert!(ui.get_qs_format_locked());
        assert!(!ui.get_qs_prompts_file());
        assert!(ui.get_qs_target().as_str().contains("Overwrite:"));
        assert!(ui.get_qs_target().as_str().contains("photo.png"));
        assert!(ui.get_export_target_preview().is_empty());

        let save_btn =
            i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, "Save")
                .collect::<Vec<_>>();
        assert!(
            !save_btn.is_empty(),
            "expected Save button without ellipsis"
        );

        ui.set_qs_open(false);
        ui.invoke_action("quick-save-confirm".into());
        assert!(st.borrow().quick_ok);
        assert_eq!(st.borrow().doc.path.as_ref(), Some(&existing_png));

        // Subsequent Ctrl+S (quick-save) overwrites directly without re-opening modal.
        ui.invoke_action("quick-save".into());
        assert!(!ui.get_qs_open());
        assert_eq!(st.borrow().doc.path.as_ref(), Some(&existing_png));

        // Non-C2PA file with c2pa_on_save enabled skips signing cleanly with a status notice while preserving toggle.
        let existing_bmp = dir.path().join("flat.bmp");
        let (ui_bmp, st_bmp) = make_test_ui(false);
        {
            let mut s = st_bmp.borrow_mut();
            s.doc = Doc::new(8, 8);
            s.doc.path = Some(existing_bmp.clone());
        }
        ui_bmp.set_c2pa_on_save(true);
        ui_bmp.invoke_action("quick-save".into());
        assert!(ui_bmp.get_c2pa_on_save());
        assert!(!ui_bmp.get_export_supports_c2pa());
        ui_bmp.set_qs_open(false);
        ui_bmp.invoke_action("quick-save-confirm".into());
        assert!(ui_bmp.get_c2pa_on_save());
        assert!(ui_bmp.get_status().as_str().contains("C2PA unsupported"));
        assert!(existing_bmp.is_file());

        // Untitled doc: Quick Save enables format picker, shows "Save…", and dynamically updates qs_target.
        let (ui2, _st2) = make_test_ui(false);
        ui2.invoke_action("quick-save".into());
        assert!(ui2.get_qs_open());
        assert_eq!(ui2.get_save_dialog_mode(), 0);
        assert!(!ui2.get_qs_format_locked());
        assert!(ui2.get_qs_prompts_file());
        assert!(ui2.get_qs_target().as_str().contains("New file:"));
        assert!(ui2.get_qs_target().as_str().ends_with(".png"));

        ui2.set_export_format(format_index(SupportedFormat::Webp));
        ui2.invoke_action("export-format-changed".into());
        assert!(ui2.get_qs_target().as_str().contains("New file:"));
        assert!(ui2.get_qs_target().as_str().ends_with(".webp"));

        let save_prompt_btn =
            i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui2, "Save…")
                .collect::<Vec<_>>();
        assert!(
            !save_prompt_btn.is_empty(),
            "expected Save… button for untitled Quick Save"
        );

        ui.hide().unwrap();
        ui_bmp.hide().unwrap();
        ui2.hide().unwrap();
    }

    #[test]
    #[cfg(feature = "ui-testing")]
    fn webp_quality_lossless_sync_and_cli_parity() {
        let (ui, st) = make_test_ui(false);

        // Set JPEG quality to 82, then switch to WebP lossless -> slider locks at 100 without losing 82.
        ui.set_export_format(format_index(SupportedFormat::Jpeg));
        ui.invoke_action("export-format-changed".into());
        ui.set_quality(82.0);
        ui.invoke_action("export-quality-edited".into());
        assert_eq!(
            format_options(&ui, &st.borrow(), SupportedFormat::Jpeg).quality,
            82
        );

        ui.set_export_format(format_index(SupportedFormat::Webp));
        ui.invoke_action("export-format-changed".into());
        assert!(ui.get_webp_lossless());
        assert_eq!(ui.get_quality().round() as u8, 100);
        assert_eq!(
            format_options(&ui, &st.borrow(), SupportedFormat::Webp).quality,
            100
        );

        // Switching back to JPEG restores the preserved 82 quality.
        ui.set_export_format(format_index(SupportedFormat::Jpeg));
        ui.invoke_action("export-format-changed".into());
        assert_eq!(ui.get_quality().round() as u8, 82);

        // Switch back to WebP and toggle lossless off -> slider updates to 82 (lossy).
        ui.set_export_format(format_index(SupportedFormat::Webp));
        ui.invoke_action("export-format-changed".into());
        ui.set_webp_lossless(false);
        ui.invoke_action("webp-lossless-toggled".into());
        assert!(!ui.get_webp_lossless());
        assert_eq!(ui.get_quality().round() as u8, 82);
        assert_eq!(
            format_options(&ui, &st.borrow(), SupportedFormat::Webp).quality,
            82
        );

        // Dragging WebP slider to 100 automatically checks lossless.
        ui.set_quality(100.0);
        ui.invoke_action("export-quality-edited".into());
        assert!(ui.get_webp_lossless());
        assert_eq!(
            format_options(&ui, &st.borrow(), SupportedFormat::Webp).quality,
            100
        );

        // Programmatic parity with studio_core::effective_quality when quality == 100.
        ui.set_webp_lossless(false);
        ui.set_quality(100.0);
        assert_eq!(
            format_options(&ui, &st.borrow(), SupportedFormat::Webp).quality,
            studio_core::effective_quality(SupportedFormat::Webp, 100, false)
        );

        ui.hide().unwrap();
    }

    #[test]
    #[cfg(feature = "ui-testing")]
    fn batch_tab_uses_own_options_independent_of_export_modal() {
        let (ui, st) = make_test_ui(false);

        // Defaults: Batch WebP is lossy q=90 even when Export defaults to webp_lossless=true.
        assert_eq!(ui.get_batch_format(), format_index(SupportedFormat::Webp));
        assert!(!ui.get_batch_webp_lossless());
        assert_eq!(ui.get_batch_quality().round() as u8, 90);
        assert_eq!(batch_convert_options(&ui, &st.borrow()).quality, 90);

        // Mutate Export modal aggressively and confirm Batch options remain completely isolated.
        ui.set_export_format(format_index(SupportedFormat::Jpeg));
        ui.invoke_action("export-format-changed".into());
        ui.set_quality(41.0);
        ui.invoke_action("export-quality-edited".into());
        ui.set_alpha_bg_hex("#112233".into());
        ui.invoke_action("alpha-bg-hex".into());

        let isolated = batch_convert_options(&ui, &st.borrow());
        assert_eq!(isolated.format, SupportedFormat::Webp);
        assert_eq!(isolated.quality, 90);
        assert_eq!(isolated.background, [255, 255, 255, 255]);

        // Toggle Batch WebP lossless on/off independently of Export modal.
        ui.set_batch_webp_lossless(true);
        ui.invoke_action("batch-webp-lossless-toggled".into());
        assert_eq!(ui.get_batch_quality().round() as u8, 100);
        assert_eq!(batch_convert_options(&ui, &st.borrow()).quality, 100);
        ui.set_batch_webp_lossless(false);
        ui.invoke_action("batch-webp-lossless-toggled".into());
        assert_eq!(ui.get_batch_quality().round() as u8, 90);
        assert_eq!(batch_convert_options(&ui, &st.borrow()).quality, 90);

        // Now configure Batch's own controls (JPEG + custom background + custom quality).
        ui.set_batch_format(format_index(SupportedFormat::Jpeg));
        ui.invoke_action("batch-format-changed".into());
        assert!(ui.get_batch_supports_quality());
        assert!(!ui.get_batch_supports_alpha());
        ui.set_batch_quality(77.0);
        ui.invoke_action("batch-quality-edited".into());
        ui.set_batch_alpha_bg_hex("#445566".into());
        ui.invoke_action("batch-alpha-bg-hex".into());

        let configured = batch_convert_options(&ui, &st.borrow());
        assert_eq!(configured.format, SupportedFormat::Jpeg);
        assert_eq!(configured.quality, 77);
        assert_eq!(configured.background, [0x44, 0x55, 0x66, 255]);

        ui.hide().unwrap();
    }

    #[test]
    #[cfg(feature = "ui-testing")]
    fn save_convert_dialog_fits_600px_window_without_clipping_buttons() {
        use i_slint_backend_testing::ElementHandle;

        let dir = tempfile::tempdir().unwrap();
        let long_path = dir
            .path()
            .join("very_long_source_filename_with_alpha_for_layout_overflow_check.png");
        let mut rgba = image::RgbaImage::from_pixel(12, 12, image::Rgba([0, 0, 0, 0]));
        rgba.put_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
        image::DynamicImage::ImageRgba8(rgba)
            .save(&long_path)
            .unwrap();

        let (ui, st) = make_test_ui(false);
        ui.window().set_size(slint::PhysicalSize::new(960, 600));
        open_path(&ui, &st, &long_path);
        ui.set_export_format(format_index(SupportedFormat::Bmp));
        ui.invoke_action("convert-file".into());
        ui.set_export_format(format_index(SupportedFormat::Bmp));
        ui.invoke_action("export-format-changed".into());

        // Trigger layout at 960x600 and measure Cancel / Convert & save… button geometry.
        let win_h = ui.window().size().height as f32 / ui.window().scale_factor();
        let cancel_btns: Vec<_> = ElementHandle::find_by_accessible_label(&ui, "Cancel").collect();
        let confirm_btns: Vec<_> =
            ElementHandle::find_by_accessible_label(&ui, "Convert & save…").collect();
        assert!(!cancel_btns.is_empty());
        assert!(!confirm_btns.is_empty());
        for btn in cancel_btns.into_iter().chain(confirm_btns) {
            let pos = btn.absolute_position();
            let sz = btn.size();
            assert!(sz.height > 10.0, "button height must be non-zero: {sz:?}");
            assert!(
                pos.y >= 20.0 && pos.y + sz.height <= win_h - 20.0,
                "button vertically outside dialog card bounds: y={}, h={}, win_h={win_h}",
                pos.y,
                sz.height
            );
        }

        ui.hide().unwrap();
    }

    #[test]
    #[cfg(feature = "ui-testing")]
    fn keep_metadata_succeeds_even_after_source_file_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let src_jpg = dir.path().join("with_exif.jpg");
        let out_jpg = dir.path().join("saved_after_delete.jpg");
        let out_c2pa_jpg = dir.path().join("signed_after_delete.jpg");

        let rgb = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            8,
            8,
            image::Rgb([50, 100, 150]),
        ));
        studio_core::save_dynamic_image(
            &rgb,
            &src_jpg,
            ConvertOptions {
                format: SupportedFormat::Jpeg,
                quality: 90,
                ..Default::default()
            },
        )
        .unwrap();
        // Inject minimal APP1 EXIF segment right after SOI (FF D8).
        let raw = std::fs::read(&src_jpg).unwrap();
        let mut with_exif = Vec::with_capacity(raw.len() + 20);
        with_exif.extend_from_slice(&raw[..2]);
        with_exif.extend_from_slice(&[
            0xFF, 0xE1, 0x00, 0x12, b'E', b'x', b'i', b'f', 0, 0, b'M', b'M', 0, b'*', 0, 0, 0, 8,
            0, 0,
        ]);
        with_exif.extend_from_slice(&raw[2..]);
        std::fs::write(&src_jpg, with_exif).unwrap();
        assert!(studio_core::metadata::has_exif(&src_jpg));

        let (ui, st) = make_test_ui(false);
        open_path(&ui, &st, &src_jpg);
        assert!(
            st.borrow()
                .doc
                .source_metadata
                .as_ref()
                .is_some_and(|m| m.has_exif())
        );

        // Delete original file on disk before saving with keep_metadata = true.
        std::fs::remove_file(&src_jpg).unwrap();
        assert!(!src_jpg.exists());

        ui.set_keep_metadata(true);
        assert!(save_to(&ui, &st, &out_jpg, SupportedFormat::Jpeg, false));
        assert!(studio_core::metadata::has_exif(&out_jpg));

        ui.set_c2pa_on_save(true);
        assert!(save_to(
            &ui,
            &st,
            &out_c2pa_jpg,
            SupportedFormat::Jpeg,
            false
        ));
        assert!(studio_core::metadata::has_exif(&out_c2pa_jpg));
        assert!(studio_core::inspect_c2pa(&out_c2pa_jpg).unwrap().present);

        ui.hide().unwrap();
    }

    #[test]
    fn c2pa_remove_atomic_and_target_resolution_work_on_windows() {
        let dir = tempfile::tempdir().unwrap();
        let unsigned = dir.path().join("plain.jpg");
        let signed = dir.path().join("signed.jpg");
        let stripped = dir.path().join("stripped.jpg");

        let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            8,
            8,
            image::Rgb([12, 34, 56]),
        ));
        studio_core::save_dynamic_image(
            &img,
            &unsigned,
            ConvertOptions {
                format: SupportedFormat::Jpeg,
                quality: 90,
                ..Default::default()
            },
        )
        .unwrap();
        studio_core::write_c2pa_manifest(
            &unsigned,
            &signed,
            &C2paManifestDraft {
                title: "test".into(),
                creator: "ci".into(),
                action: "c2pa.created".into(),
            },
        )
        .unwrap();
        assert!(studio_core::inspect_c2pa(&signed).unwrap().present);

        remove_c2pa_atomic(&signed, &stripped).unwrap();
        assert!(stripped.is_file());
        assert!(!studio_core::inspect_c2pa(&stripped).unwrap().present);

        // Target resolution preserves source extension for bare/trailing-dot/custom names and rejects mismatches.
        assert_eq!(
            resolve_c2pa_removal_target(
                Path::new("out/clean"),
                &signed,
                Some(SupportedFormat::Jpeg)
            ),
            Ok(PathBuf::from("out/clean.jpg"))
        );
        assert_eq!(
            resolve_c2pa_removal_target(
                Path::new("out/clean.v2"),
                &signed,
                Some(SupportedFormat::Jpeg)
            ),
            Ok(PathBuf::from("out/clean.v2.jpg"))
        );
        assert!(
            resolve_c2pa_removal_target(
                Path::new("out/clean.png"),
                &signed,
                Some(SupportedFormat::Jpeg)
            )
            .is_err()
        );
    }

    #[test]
    fn tool_labels_and_shortcuts_cover_pro_tools() {
        assert_eq!(TOOL_LABELS.len(), 20);
        assert_eq!(tool_for_key("g", false), Some(17));
        assert_eq!(tool_for_key("w", false), Some(18));
        assert_eq!(tool_for_key("o", false), Some(19));
    }

    #[test]
    #[cfg(feature = "ui-testing")]
    fn gradient_wand_and_clone_pointer_workflows() {
        let (ui, st) = make_test_ui(false);
        {
            let mut s = st.borrow_mut();
            s.doc = Doc::new(80, 60);
            s.zoom = 1.0;
            s.pan = (0.0, 0.0);
            s.tools.color1 = [255, 0, 0, 255];
            s.tools.color2 = [0, 0, 255, 255];
        }
        refresh(&ui, &st);

        // Gradient tool (`G`): live line preview + linear fill + undo.
        assert!(ui.invoke_key("g".into(), false, false));
        assert_eq!(ui.get_tool(), Tool::Gradient.index());
        ui.invoke_canvas_pointer(0, 0, 10.0, 20.0, false, false, false);
        ui.invoke_canvas_pointer(2, 0, 70.0, 20.0, false, false, false);
        assert!(ui.get_has_preview());
        assert!(!ui.get_pv_line_path().is_empty());
        ui.invoke_canvas_pointer(1, 0, 70.0, 20.0, false, false, false);
        assert!(!ui.get_has_preview());
        assert!(ui.get_pv_line_path().is_empty());
        {
            let s = st.borrow();
            assert!(s.doc.image.get_pixel(10, 20).0[0] > 200);
            assert!(s.doc.image.get_pixel(70, 20).0[2] > 200);
        }
        ui.invoke_action("undo".into());
        assert_eq!(
            st.borrow().doc.image.get_pixel(10, 20).0,
            [255, 255, 255, 255]
        );

        // Magic Wand (`W`): click selects colored patch into `selection_mask`; Gradient respects mask; `deselect` clears mask.
        {
            let mut s = st.borrow_mut();
            for y in 20..50 {
                for x in 20..50 {
                    s.doc.image.put_pixel(x, y, image::Rgba([20, 180, 40, 255]));
                }
            }
            s.doc.dirty = Some(IRect::full(80, 60));
        }
        refresh(&ui, &st);
        assert!(ui.invoke_key("w".into(), false, false));
        assert_eq!(ui.get_tool(), Tool::MagicWand.index());
        ui.invoke_canvas_pointer(0, 0, 30.0, 30.0, false, false, false);
        ui.invoke_canvas_pointer(1, 0, 30.0, 30.0, false, false, false);
        assert!(ui.get_has_selection());
        assert!(st.borrow().doc.selection_mask.is_some());
        assert!(ui.get_status().as_str().contains("px"));

        assert!(ui.invoke_key("g".into(), false, false));
        ui.invoke_canvas_pointer(0, 0, 20.0, 30.0, false, false, false);
        ui.invoke_canvas_pointer(2, 0, 49.0, 30.0, false, false, false);
        ui.invoke_canvas_pointer(1, 0, 49.0, 30.0, false, false, false);
        assert_eq!(
            st.borrow().doc.image.get_pixel(5, 5).0,
            [255, 255, 255, 255]
        );
        let before_mosaic = st.borrow().doc.image.clone();
        ui.invoke_action("mosaic-selection".into());
        assert_ne!(st.borrow().doc.image, before_mosaic);
        assert!(ui.get_has_selection());
        assert_eq!(
            st.borrow().doc.image.get_pixel(5, 5),
            before_mosaic.get_pixel(5, 5)
        );
        ui.invoke_action("undo".into());
        assert_eq!(st.borrow().doc.image, before_mosaic);
        ui.invoke_action("deselect".into());
        assert!(!ui.get_has_selection());
        assert!(st.borrow().doc.selection_mask.is_none());

        // Clone Stamp (`O`): right-click & Alt+click sample without opening ctx menu; left-drag clones in one undo step.
        {
            let mut s = st.borrow_mut();
            s.doc = Doc::new(80, 60);
            for y in 10..26 {
                for x in 10..26 {
                    s.doc.image.put_pixel(x, y, image::Rgba([220, 20, 20, 255]));
                }
            }
            s.doc.dirty = Some(IRect::full(80, 60));
        }
        refresh(&ui, &st);
        assert!(ui.invoke_key("o".into(), false, false));
        assert_eq!(ui.get_tool(), Tool::Clone.index());
        ui.set_stroke_width(12.0);
        ui.set_clone_hardness(1.0);
        ui.invoke_canvas_pointer(0, 1, 18.0, 18.0, false, false, false);
        ui.invoke_canvas_pointer(1, 1, 18.0, 18.0, false, false, false);
        assert!(!ui.get_ctx_open());
        assert!(ui.get_has_clone_source());
        assert_eq!(st.borrow().tools.clone_source, Some((18.0, 18.0)));
        assert!(ui.get_status().as_str().contains("Clone source set"));

        ui.invoke_canvas_pointer(0, 0, 55.0, 40.0, false, false, false);
        ui.invoke_canvas_pointer(2, 0, 60.0, 40.0, false, false, false);
        ui.invoke_canvas_pointer(1, 0, 60.0, 40.0, false, false, false);
        assert_eq!(
            st.borrow().doc.image.get_pixel(55, 40).0,
            [220, 20, 20, 255]
        );
        ui.invoke_action("undo".into());
        assert_eq!(
            st.borrow().doc.image.get_pixel(55, 40).0,
            [255, 255, 255, 255]
        );
        ui.invoke_action("redo".into());
        assert_eq!(
            st.borrow().doc.image.get_pixel(55, 40).0,
            [220, 20, 20, 255]
        );

        ui.invoke_canvas_pointer(0, 0, 20.0, 20.0, false, false, true);
        ui.invoke_canvas_pointer(1, 0, 20.0, 20.0, false, false, true);
        assert_eq!(st.borrow().tools.clone_source, Some((20.0, 20.0)));

        ui.hide().unwrap();
    }

    #[test]
    #[cfg(feature = "ui-testing")]
    fn blend_mode_and_layer_productivity_actions() {
        use slint::Model;
        let (ui, st) = make_test_ui(false);
        assert_eq!(ui.get_blend_mode_names().row_count(), BlendMode::ALL.len());
        {
            let mut s = st.borrow_mut();
            s.doc = Doc::new(100, 80);
            s.tools.color1 = [0, 128, 255, 255];
            s.tools.color2 = [0, 128, 255, 255];
            s.tools.shape_filled = true;
            let t = s.tools.clone();
            s.doc
                .add_shape(ShapeKind::Rectangle, (5.0, 5.0), (25.0, 25.0), &t);
            sync_layer_fields(&ui, &s);
        }
        refresh(&ui, &st);

        ui.set_l_blend(BlendMode::Multiply.index() as i32);
        ui.invoke_layer_edited("blend".into());
        assert_eq!(st.borrow().doc.layers[0].blend_mode(), BlendMode::Multiply);
        ui.invoke_action("undo".into());
        assert_eq!(st.borrow().doc.layers[0].blend_mode(), BlendMode::Normal);
        assert_eq!(ui.get_l_blend(), 0);

        ui.invoke_action("layer-center".into());
        let (cx, cy, _, _) = st.borrow().doc.layer_bounds(0).unwrap();
        assert!((cx + 10.0 - 50.0).abs() <= 2.0);
        assert!((cy + 10.0 - 40.0).abs() <= 2.0);
        ui.invoke_action("layer-flip-h".into());
        ui.invoke_action("layer-flip-v".into());
        ui.invoke_action("layer-rot".into());
        assert!(ui.invoke_key("m".into(), true, true));
        assert!(st.borrow().doc.layers.is_empty());
        ui.invoke_action("undo".into());
        assert_eq!(st.borrow().doc.layers.len(), 1);

        ui.hide().unwrap();
    }

    #[test]
    #[cfg(feature = "ui-testing")]
    fn live_histogram_pro_adjust_effects_and_cutout_framing() {
        let (ui, st) = make_test_ui(false);
        {
            let mut s = st.borrow_mut();
            s.doc = Doc::new(40, 40);
            for y in 0..40 {
                for x in 0..40 {
                    s.doc.image.put_pixel(
                        x,
                        y,
                        image::Rgba([(x * 6) as u8, (y * 6) as u8, 120, 255]),
                    );
                }
            }
            s.doc.dirty = Some(IRect::full(40, 40));
        }
        refresh(&ui, &st);

        ui.invoke_action("tab:2".into());
        assert!(ui.get_histogram_img().size().width > 0);
        assert!(ui.get_histogram_stats().as_str().contains("Mean"));
        let stats_before = ui.get_histogram_stats().to_string();

        ui.set_pro_look(1);
        for act in [
            "f-look",
            "f-levels",
            "f-shadows",
            "f-highlights",
            "f-vibrance",
            "f-clarity",
            "f-balance",
            "f-tilt-shift",
            "f-pixelate",
            "f-grain",
            "f-oil",
            "f-emboss",
            "f-edge",
            "f-sketch",
            "f-threshold",
        ] {
            ui.invoke_action(act.into());
        }
        assert_ne!(ui.get_histogram_stats().as_str(), stats_before);
        ui.invoke_action("tab:5".into());
        assert!(!ui.get_histogram_stats().is_empty());

        // Cutout & framing workflow.
        {
            let mut s = st.borrow_mut();
            s.doc = Doc::new(40, 40);
            s.tools.color2 = [255, 255, 255, 255];
            for y in 10..30 {
                for x in 10..30 {
                    s.doc.image.put_pixel(x, y, image::Rgba([220, 40, 40, 255]));
                }
            }
            s.doc.dirty = Some(IRect::full(40, 40));
        }
        refresh(&ui, &st);

        ui.invoke_action("f-remove-bg".into());
        assert_eq!(st.borrow().doc.image.get_pixel(0, 0).0[3], 0);
        assert_eq!(st.borrow().doc.image.get_pixel(20, 20).0[3], 255);

        ui.invoke_action("f-trim-alpha".into());
        let (tw, th) = (st.borrow().doc.width(), st.borrow().doc.height());
        assert!(tw < 40 && th < 40);
        assert_eq!(ui.get_img_w(), tw as i32);
        assert_eq!(ui.get_rs_w(), tw as i32);

        ui.set_c_round(8.0);
        ui.invoke_action("f-round-corners".into());
        assert_eq!(st.borrow().doc.image.get_pixel(0, 0).0[3], 0);

        ui.invoke_action("f-drop-shadow".into());
        let (sw, sh) = (st.borrow().doc.width(), st.borrow().doc.height());
        assert!(sw > tw && sh > th);

        ui.invoke_action("f-frame-card".into());
        let (fw, fh) = (st.borrow().doc.width(), st.borrow().doc.height());
        assert!(fw > sw && fh > sh);

        ui.hide().unwrap();
    }

    #[test]
    fn inspector_collapsed_setting_defaults_and_roundtrips() {
        let empty: Settings = serde_json::from_str("{}").unwrap();
        assert!(!empty.inspector_collapsed);
        assert!(!initial_state(&empty, false, false).inspector_collapsed);

        let on = Settings {
            inspector_collapsed: true,
            ..Default::default()
        };
        let decoded: Settings = serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
        assert!(decoded.inspector_collapsed);
        let s = initial_state(&decoded, false, false);
        assert!(s.inspector_collapsed);
        assert!(!s.panels_hidden); // focus mode is never restored
    }

    #[test]
    fn fit_for_centres_with_margin_and_detects_fit() {
        // Width-limited: (1040 - 40) / 1000 = 1.0 < (740 - 40) / 500.
        let (z, pan) = fit_for(1040.0, 740.0, 1000.0, 500.0);
        assert!((z - 1.0).abs() < 1e-6);
        assert_eq!(pan, (20.0, 120.0));
        // Height-limited.
        let (z, pan) = fit_for(1000.0, 440.0, 400.0, 800.0);
        assert!((z - 0.5).abs() < 1e-6);
        assert_eq!(pan, (400.0, 20.0));
        // Tiny canvases are clamped to 100px before fitting; zoom stays within limits.
        assert_eq!(
            fit_for(10.0, 10.0, 60.0, 60.0),
            fit_for(100.0, 100.0, 60.0, 60.0)
        );
        assert_eq!(fit_for(5000.0, 5000.0, 1.0, 1.0).0, 16.0);
        assert_eq!(fit_for(100.0, 100.0, 100_000.0, 10.0).0, 0.02);

        let (z, pan) = fit_for(1200.0, 800.0, 640.0, 480.0);
        assert!(is_at_fit(z, pan, 1200.0, 800.0, 640.0, 480.0));
        assert!(is_at_fit(
            z,
            (pan.0 + 0.2, pan.1),
            1200.0,
            800.0,
            640.0,
            480.0
        ));
        assert!(!is_at_fit(z * 1.15, pan, 1200.0, 800.0, 640.0, 480.0));
        assert!(!is_at_fit(
            z,
            (pan.0 + 30.0, pan.1),
            1200.0,
            800.0,
            640.0,
            480.0
        ));
        // A fitted view stops being "at fit" once the canvas grows.
        assert!(!is_at_fit(z, pan, 1526.0, 800.0, 640.0, 480.0));
    }

    #[test]
    #[cfg(feature = "ui-testing")]
    fn inspector_collapse_rail_and_refit() {
        use i_slint_backend_testing::ElementHandle;
        use slint::platform::Key;
        let (ui, st) = make_test_ui(false);
        ui.window().set_size(slint::PhysicalSize::new(1400, 860));
        st.borrow_mut().doc = Doc::new(400, 300);
        fit(&ui, &st);
        refresh(&ui, &st);
        let open_w = ui.get_canvas_w();
        assert!(!ui.get_inspector_collapsed());
        assert_eq!(
            ElementHandle::find_by_accessible_label(&ui, "Collapse panel (F4)").count(),
            2 // top bar + inspector title row
        );

        ui.invoke_action("toggle-inspector".into());
        assert!(ui.get_inspector_collapsed());
        assert!(st.borrow().inspector_collapsed);
        assert!(st.borrow().refit_pending); // the view was fitted
        let rail_w = ui.get_canvas_w();
        assert!(
            (rail_w - open_w - 326.0).abs() < 1.0,
            "{open_w} -> {rail_w}"
        );
        assert_eq!(
            ElementHandle::find_by_accessible_label(&ui, "Expand panel (F4)").count(),
            2 // top bar + rail chevron
        );
        ui.invoke_canvas_resized();
        assert!(!st.borrow().refit_pending);
        let (z, pan) = fit_for(rail_w, ui.get_canvas_h(), 400.0, 300.0);
        assert_eq!((st.borrow().zoom, st.borrow().pan), (z, pan));

        // Picking a tool that switches tabs must not reopen the collapsed panel.
        ui.invoke_tool_changed(Tool::Crop.index());
        assert!(ui.get_inspector_collapsed());
        assert_eq!(ui.get_tab(), 2);

        // A rail icon expands straight to its tab.
        ElementHandle::find_by_accessible_label(&ui, "Batch / GIF / panorama")
            .next()
            .unwrap()
            .invoke_accessible_default_action();
        assert!(!ui.get_inspector_collapsed());
        assert!(!st.borrow().inspector_collapsed);
        assert_eq!(ui.get_tab(), 6);
        ui.invoke_canvas_resized();
        let (z, pan) = fit_for(open_w, ui.get_canvas_h(), 400.0, 300.0);
        assert_eq!((st.borrow().zoom, st.borrow().pan), (z, pan));

        // tab:N from the menus expands too and runs the tab's side effects.
        ui.invoke_action("toggle-inspector".into());
        ui.invoke_action("tab:2".into());
        assert!(!ui.get_inspector_collapsed());
        assert_eq!(ui.get_tab(), 2);
        assert!(ui.get_histogram_img().size().width > 0);

        // Manually zoomed views are not re-fitted.
        st.borrow_mut().zoom = 3.0;
        st.borrow_mut().pan = (-50.0, -40.0);
        let f4 = || char::from(Key::F4).to_string().into();
        assert!(ui.invoke_key(f4(), false, false));
        assert!(ui.get_inspector_collapsed());
        assert!(!st.borrow().refit_pending);
        ui.invoke_canvas_resized();
        assert_eq!((st.borrow().zoom, st.borrow().pan), (3.0, (-50.0, -40.0)));
        assert!(ui.invoke_key(f4(), false, false));
        assert!(!ui.get_inspector_collapsed());

        // F4 is ignored behind a modal and in viewer mode.
        ui.set_qs_open(true);
        assert!(!ui.invoke_key(f4(), false, false));
        ui.set_qs_open(false);
        st.borrow_mut().view_mode = true;
        assert!(!ui.invoke_key(f4(), false, false));
        assert!(!ui.get_inspector_collapsed());
        st.borrow_mut().view_mode = false;
        ui.hide().unwrap();
    }

    #[test]
    #[cfg(feature = "ui-testing")]
    fn focus_mode_tab_key_hides_both_panels() {
        use i_slint_backend_testing::ElementHandle;
        use slint::platform::{Key, WindowEvent};
        let (ui, st) = make_test_ui(false);
        ui.window().set_size(slint::PhysicalSize::new(1400, 860));
        {
            let mut s = st.borrow_mut();
            s.doc = Doc::new(200, 120);
            s.doc.boxes = vec![studio_core::redact::RedactBox::manual(100, 20, 30, 20)];
        }
        ui.set_tab(4);
        refresh(&ui, &st);
        let open_w = ui.get_canvas_w();
        let press = |key: Key| {
            let text: SharedString = char::from(key).to_string().into();
            ui.window()
                .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
            ui.window()
                .dispatch_event(WindowEvent::KeyReleased { text });
        };
        let focus_editor = || {
            ElementHandle::find_by_accessible_label(&ui, "Select redaction box: (30×20)")
                .next()
                .unwrap()
                .invoke_accessible_default_action();
            assert!(ui.get_editor_key_focus());
        };

        // Tab inside a text field keeps normal focus traversal.
        ElementHandle::find_by_accessible_label(&ui, "Redaction keyword")
            .next()
            .unwrap()
            .mock_single_click(slint::platform::PointerEventButton::Left);
        assert!(!ui.get_editor_key_focus());
        press(Key::Tab);
        assert!(!ui.get_panels_hidden());

        // Tab on the editor (real window dispatch through the FocusScope) toggles focus mode.
        focus_editor();
        press(Key::Tab);
        assert!(ui.get_panels_hidden());
        assert!(st.borrow().panels_hidden);
        assert!(ui.get_canvas_w() >= open_w + 372.0 + 112.0 - 1.0);
        assert_eq!(
            ElementHandle::find_by_accessible_label(&ui, "Expand panel (F4)").count(),
            0 // no rail in focus mode
        );
        assert!(ui.get_editor_key_focus());
        press(Key::Tab);
        assert!(!ui.get_panels_hidden());

        // Ignored behind a modal.
        focus_editor();
        ui.set_qs_open(true);
        let tab: SharedString = char::from(Key::Tab).to_string().into();
        assert!(!ui.invoke_key(tab, false, false));
        assert!(!ui.get_panels_hidden());
        ui.set_qs_open(false);

        // F4 leaves focus mode with the inspector expanded; toggle-mode clears focus mode.
        ui.invoke_action("toggle-panels".into());
        assert!(ui.get_panels_hidden());
        assert!(ui.invoke_key(char::from(Key::F4).to_string().into(), false, false));
        assert!(!ui.get_panels_hidden());
        assert!(!ui.get_inspector_collapsed());
        ui.invoke_action("toggle-panels".into());
        ui.invoke_action("toggle-mode".into());
        assert!(!ui.get_panels_hidden());
        assert!(!st.borrow().panels_hidden);
        ui.invoke_action("toggle-panels".into()); // no-op in viewer mode
        assert!(!ui.get_panels_hidden());
        ui.invoke_action("toggle-mode".into());
        ui.hide().unwrap();
    }

    #[cfg(feature = "ui-testing")]
    fn wait_batch(ui: &AppWindow, st: &S) {
        for _ in 0..600 {
            poll_jobs(ui, st);
            if st.borrow().batch_job.is_none() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("batch did not finish");
    }

    #[cfg(feature = "ui-testing")]
    fn write_png(path: &Path, v: u8) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(8, 8, image::Rgb([v, 40, 90])))
            .save(path)
            .unwrap();
    }

    #[test]
    #[cfg(feature = "ui-testing")]
    fn multi_c2pa_add_and_strip_into_new_folders() {
        let (ui, st) = make_test_ui(false);
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("one").join("photo.png");
        let b = dir.path().join("two").join("photo.png");
        write_png(&a, 10);
        write_png(&b, 200);
        let out_parent = dir.path().join("out");

        // Nothing selected: buttons' guard logic reports and spawns nothing.
        assert_eq!(ui.get_c2pa_multi_count(), 0);
        ui.invoke_action("c2pa-multi-add".into());
        assert!(st.borrow().batch_job.is_none());
        assert!(!ui.get_c2pa_batch_text().is_empty());

        set_c2pa_files(&ui, &st, vec![a.clone(), b.clone()]);
        assert_eq!(ui.get_c2pa_multi_count(), 2);
        assert!(ui.get_c2pa_multi_text().contains("2 selected"));
        assert_eq!(
            ui.get_c2pa_multi_parent().as_str(),
            a.parent().unwrap().display().to_string()
        );
        ui.set_c2pa_multi_parent(out_parent.display().to_string().into());
        ui.set_c2pa_new_folder(true);
        ui.set_c2pa_title("T".into());
        ui.set_c2pa_creator("C".into());

        ui.invoke_action("c2pa-multi-add".into());
        assert!(ui.get_batch_running());
        wait_batch(&ui, &st);
        assert!(!ui.get_batch_running());
        let added = std::path::PathBuf::from(ui.get_c2pa_result_dir().to_string());
        assert_eq!(added.parent().unwrap(), out_parent);
        assert!(
            added
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("pixelstudio_c2pa_added_")
        );
        assert!(
            ui.get_c2pa_batch_text()
                .contains(&added.display().to_string())
        );
        let mut names: Vec<_> = std::fs::read_dir(&added)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        names.sort();
        assert_eq!(names.len(), 2, "same-named inputs yield two outputs");
        for n in &names {
            assert!(studio_core::inspect_c2pa(n).unwrap().present);
        }
        // Inputs untouched.
        assert!(!studio_core::inspect_c2pa(&a).unwrap().present);

        // Strip from the signed copies into another brand-new folder.
        set_c2pa_files(&ui, &st, names.clone());
        ui.set_c2pa_multi_parent(out_parent.display().to_string().into());
        ui.invoke_action("c2pa-multi-remove".into());
        wait_batch(&ui, &st);
        let stripped = std::path::PathBuf::from(ui.get_c2pa_result_dir().to_string());
        assert_ne!(stripped, added);
        assert!(
            stripped
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("pixelstudio_c2pa_removed_")
        );
        let outs: Vec<_> = std::fs::read_dir(&stripped)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(outs.len(), 2);
        for o in &outs {
            assert!(!studio_core::inspect_c2pa(o).unwrap().present);
        }

        // New-folder OFF writes straight into the chosen folder; clear resets state.
        set_c2pa_files(&ui, &st, vec![a.clone()]);
        let direct = dir.path().join("direct");
        ui.set_c2pa_multi_parent(direct.display().to_string().into());
        ui.set_c2pa_new_folder(false);
        ui.invoke_action("c2pa-multi-add".into());
        wait_batch(&ui, &st);
        assert_eq!(std::fs::read_dir(&direct).unwrap().count(), 1);
        ui.invoke_action("c2pa-multi-clear".into());
        assert_eq!(ui.get_c2pa_multi_count(), 0);
        assert!(st.borrow().c2pa_files.is_empty());
        assert_eq!(ui.get_c2pa_result_dir().as_str(), "");
        ui.hide().unwrap();
    }
}

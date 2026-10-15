//! Built-in sticker / icon library for social-media post images.
//!
//! All artwork is original SVG drawn for Pixel Studio (MIT, same as the project).
//! No trademarked platform logos are included: "social" icons are generic
//! (thumbs-up, share, comment, play button, bell, camera...).
//!
//! Each sticker is an SVG template on a 100x100 view box. `{F}` is the fill colour,
//! `{S}` the outline colour and `{W}` the outline width, so stickers stay recolorable.

use image::{DynamicImage, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StickerCategory {
    Arrows,
    Callouts,
    Hearts,
    Checks,
    Badges,
    Highlights,
    Emoji,
    Labels,
    Social,
}

impl StickerCategory {
    pub const ALL: [StickerCategory; 9] = [
        Self::Arrows,
        Self::Callouts,
        Self::Hearts,
        Self::Checks,
        Self::Badges,
        Self::Highlights,
        Self::Emoji,
        Self::Labels,
        Self::Social,
    ];
    pub fn label(self, zh: bool) -> &'static str {
        match (self, zh) {
            (Self::Arrows, true) => "箭頭",
            (Self::Arrows, false) => "Arrows",
            (Self::Callouts, true) => "對話框",
            (Self::Callouts, false) => "Callouts",
            (Self::Hearts, true) => "愛心／星星",
            (Self::Hearts, false) => "Hearts & stars",
            (Self::Checks, true) => "勾／叉",
            (Self::Checks, false) => "Checks & crosses",
            (Self::Badges, true) => "數字徽章",
            (Self::Badges, false) => "Numbers & badges",
            (Self::Highlights, true) => "標示圈框",
            (Self::Highlights, false) => "Highlights",
            (Self::Emoji, true) => "表情",
            (Self::Emoji, false) => "Emoji",
            (Self::Labels, true) => "標籤",
            (Self::Labels, false) => "Labels",
            (Self::Social, true) => "社群通用",
            (Self::Social, false) => "Social (generic)",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Sticker {
    pub id: String,
    pub category: StickerCategory,
    /// Search keywords (English + Chinese).
    pub keywords: String,
    pub svg: String,
    pub default_fill: [u8; 4],
}

/// Rendering options for a sticker instance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StickerStyle {
    pub fill: [u8; 4],
    pub stroke: [u8; 4],
    pub stroke_width: f32,
    pub shadow: bool,
}

impl Default for StickerStyle {
    fn default() -> Self {
        Self {
            fill: [229, 57, 53, 255],
            stroke: [255, 255, 255, 255],
            stroke_width: 4.0,
            shadow: true,
        }
    }
}

const RED: [u8; 4] = [229, 57, 53, 255];
const YELLOW: [u8; 4] = [255, 196, 0, 255];
const GREEN: [u8; 4] = [67, 160, 71, 255];
const BLUE: [u8; 4] = [30, 136, 229, 255];

fn p(fill_path: &str) -> String {
    format!(
        r#"<path d="{fill_path}" fill="{{F}}" stroke="{{S}}" stroke-width="{{W}}" stroke-linejoin="round"/>"#
    )
}

fn ring(shape: &str) -> String {
    // Highlight outlines: the fill colour is used as the stroke so they stay hollow.
    format!(
        r#"{shape} fill="none" stroke="{{S}}" stroke-width="calc" /><!--outer-->{shape} fill="none" stroke="{{F}}" stroke-width="8"/>"#
    )
}

fn label(text: &str, fill: &str, font: u32) -> String {
    format!(
        r#"<rect x="4" y="28" width="92" height="44" rx="10" fill="{fill}" stroke="{{S}}" stroke-width="{{W}}"/><text x="50" y="{y}" font-family="{{FONT}}" font-weight="bold" font-size="{font}" text-anchor="middle" fill="white">{text}</text>"#,
        y = 50 + font * 36 / 100
    )
}

fn face(features: &str) -> String {
    format!(
        r##"<circle cx="50" cy="50" r="44" fill="{{F}}" stroke="{{S}}" stroke-width="{{W}}"/>{features}"##
    )
}

const EYES: &str = r##"<circle cx="35" cy="40" r="6" fill="#3b2a1a"/><circle cx="65" cy="40" r="6" fill="#3b2a1a"/>"##;

fn digit_badge(n: u32) -> String {
    format!(
        r#"<circle cx="50" cy="50" r="42" fill="{{F}}" stroke="{{S}}" stroke-width="{{W}}"/><text x="50" y="68" font-family="{{FONT}}" font-weight="bold" font-size="52" text-anchor="middle" fill="white">{n}</text>"#
    )
}

/// The full built-in library.
pub fn library() -> Vec<Sticker> {
    use StickerCategory::*;
    let mut v: Vec<(String, StickerCategory, &str, String, [u8; 4])> = vec![
        // Arrows
        ("arrow-right".into(), Arrows, "arrow right 右 箭頭", p("M8 38 H58 V18 L94 50 L58 82 V62 H8 Z"), RED),
        ("arrow-left".into(), Arrows, "arrow left 左 箭頭", p("M92 38 H42 V18 L6 50 L42 82 V62 H92 Z"), RED),
        ("arrow-up".into(), Arrows, "arrow up 上 箭頭", p("M38 92 V42 H18 L50 6 L82 42 H62 V92 Z"), RED),
        ("arrow-down".into(), Arrows, "arrow down 下 箭頭", p("M38 8 V58 H18 L50 94 L82 58 H62 V8 Z"), RED),
        ("arrow-curve".into(), Arrows, "arrow curve bend 彎 箭頭", p("M10 85 C10 40 40 22 66 22 V6 L94 32 L66 58 V42 C46 42 28 52 26 85 Z"), RED),
        ("arrow-double".into(), Arrows, "double arrow 雙向 箭頭", p("M4 50 L28 22 V38 H72 V22 L96 50 L72 78 V62 H28 V78 Z"), BLUE),
        ("chevrons".into(), Arrows, "chevron next 下一步", p("M10 20 L40 50 L10 80 L24 80 L54 50 L24 20 Z M46 20 L76 50 L46 80 L60 80 L90 50 L60 20 Z"), YELLOW),
        // Callouts
        ("bubble-round".into(), Callouts, "speech bubble 對話 泡泡", p("M50 10 C80 10 94 26 94 44 C94 62 78 76 52 76 C46 76 40 75 35 74 L16 90 L22 70 C10 63 6 54 6 44 C6 26 22 10 50 10 Z"), [255, 255, 255, 255]),
        ("bubble-square".into(), Callouts, "speech box 對話框", p("M8 10 H92 V70 H40 L20 90 V70 H8 Z"), [255, 255, 255, 255]),
        ("bubble-think".into(), Callouts, "thought cloud 想法 雲", p("M30 22 C34 8 56 6 62 18 C76 10 92 22 86 38 C98 46 92 66 76 64 C72 76 52 78 46 68 C32 76 14 68 18 54 C4 48 8 28 24 30 Z M22 80 a6 6 0 1 0 0.1 0 Z M10 92 a4 4 0 1 0 0.1 0 Z"), [255, 255, 255, 255]),
        ("bubble-shout".into(), Callouts, "shout burst 爆炸 驚嘆", p("M50 4 L60 26 L84 14 L76 38 L98 46 L76 58 L88 82 L62 72 L54 96 L42 74 L18 88 L24 62 L2 54 L24 42 L12 18 L38 28 Z"), YELLOW),
        ("bubble-info".into(), Callouts, "info tip 提示 資訊", format!(r#"{}<text x="50" y="66" font-family="{{FONT}}" font-weight="bold" font-size="50" text-anchor="middle" fill="{{S}}">i</text>"#, p("M50 6 A44 44 0 1 1 49.9 6 Z")), BLUE),
        // Hearts / likes / stars
        ("heart".into(), Hearts, "heart love like 愛心 喜歡", p("M50 88 C20 66 6 50 6 32 C6 18 18 8 30 8 C40 8 46 14 50 22 C54 14 60 8 70 8 C82 8 94 18 94 32 C94 50 80 66 50 88 Z"), RED),
        ("star".into(), Hearts, "star favorite 星星 收藏", p("M50 6 L62 36 L94 38 L69 59 L77 92 L50 74 L23 92 L31 59 L6 38 L38 36 Z"), YELLOW),
        ("thumbs-up".into(), Hearts, "thumbs up like 讚 按讚", p("M8 44 H24 V90 H8 Z M30 44 L48 10 C56 10 60 16 58 26 L54 40 H84 C92 40 96 48 92 56 L82 86 C80 90 76 92 72 92 H30 Z"), BLUE),
        ("sparkle".into(), Hearts, "sparkle shine 閃亮 亮點", p("M50 4 C54 34 66 46 96 50 C66 54 54 66 50 96 C46 66 34 54 4 50 C34 46 46 34 50 4 Z"), YELLOW),
        ("hearts-three".into(), Hearts, "hearts many 多愛心", p("M30 60 C14 48 6 40 6 30 C6 22 12 16 20 16 C25 16 28 19 30 23 C32 19 35 16 40 16 C48 16 54 22 54 30 C54 40 46 48 30 60 Z M70 90 C54 78 46 70 46 60 C46 52 52 46 60 46 C65 46 68 49 70 53 C72 49 75 46 80 46 C88 46 94 52 94 60 C94 70 86 78 70 90 Z"), [233, 30, 99, 255]),
        // Checks / crosses
        ("check".into(), Checks, "check ok yes 勾 正確", p("M8 52 L22 38 L40 56 L78 14 L92 28 L40 86 Z"), GREEN),
        ("cross".into(), Checks, "cross no x wrong 叉 錯誤", p("M14 28 L28 14 L50 36 L72 14 L86 28 L64 50 L86 72 L72 86 L50 64 L28 86 L14 72 L36 50 Z"), RED),
        ("check-circle".into(), Checks, "check circle done 完成", format!(r#"{}<path d="M28 52 L44 66 L74 34" fill="none" stroke="white" stroke-width="10" stroke-linecap="round" stroke-linejoin="round"/>"#, p("M50 6 A44 44 0 1 1 49.9 6 Z")), GREEN),
        ("cross-circle".into(), Checks, "cross circle cancel 取消", format!(r#"{}<path d="M33 33 L67 67 M67 33 L33 67" stroke="white" stroke-width="10" stroke-linecap="round"/>"#, p("M50 6 A44 44 0 1 1 49.9 6 Z")), RED),
        ("warning".into(), Checks, "warning alert caution 警告 注意", format!(r#"{}<path d="M50 36 V62" stroke="rgb(34,34,34)" stroke-width="10" stroke-linecap="round"/><circle cx="50" cy="76" r="5" fill="rgb(34,34,34)"/>"#, p("M50 8 L94 88 H6 Z")), YELLOW),
        // Highlights
        ("hl-circle".into(), Highlights, "highlight circle ring 圈 標示", ring(r#"<ellipse cx="50" cy="50" rx="44" ry="34""#), RED),
        ("hl-box".into(), Highlights, "highlight box frame 框 標示", ring(r#"<rect x="6" y="16" width="88" height="68" rx="6""#), RED),
        ("hl-rounded".into(), Highlights, "highlight rounded pill 圓角框", ring(r#"<rect x="6" y="26" width="88" height="48" rx="24""#), YELLOW),
        ("hl-underline".into(), Highlights, "underline marker 底線 螢光", r#"<path d="M6 70 C30 60 70 78 94 66" fill="none" stroke="{S}" stroke-width="18" stroke-linecap="round"/><path d="M6 70 C30 60 70 78 94 66" fill="none" stroke="{F}" stroke-width="12" stroke-linecap="round"/>"#.into(), YELLOW),
        ("hl-scribble".into(), Highlights, "scribble circle hand drawn 手繪圈", r#"<path d="M20 30 C40 8 88 14 92 46 C96 80 30 92 12 62 C2 44 22 22 56 20" fill="none" stroke="{F}" stroke-width="7" stroke-linecap="round"/>"#.into(), RED),
        // Emoji (original faces)
        ("emoji-smile".into(), Emoji, "smile happy 微笑 開心", face(&format!(r##"{EYES}<path d="M28 60 Q50 82 72 60" fill="none" stroke="#3b2a1a" stroke-width="6" stroke-linecap="round"/>"##)), YELLOW),
        ("emoji-laugh".into(), Emoji, "laugh lol 大笑", face(r##"<path d="M28 42 Q35 32 42 42 M58 42 Q65 32 72 42" fill="none" stroke="#3b2a1a" stroke-width="6" stroke-linecap="round"/><path d="M24 56 H76 Q72 84 50 84 Q28 84 24 56 Z" fill="#3b2a1a"/>"##), YELLOW),
        ("emoji-wink".into(), Emoji, "wink 眨眼", face(r##"<circle cx="35" cy="40" r="6" fill="#3b2a1a"/><path d="M58 40 H72" stroke="#3b2a1a" stroke-width="6" stroke-linecap="round"/><path d="M30 62 Q50 78 70 62" fill="none" stroke="#3b2a1a" stroke-width="6" stroke-linecap="round"/>"##), YELLOW),
        ("emoji-love".into(), Emoji, "love eyes 愛心眼", face(r##"<path d="M35 48 C24 40 22 30 30 28 C33 28 35 31 35 33 C35 31 37 28 40 28 C48 30 46 40 35 48 Z M65 48 C54 40 52 30 60 28 C63 28 65 31 65 33 C65 31 67 28 70 28 C78 30 76 40 65 48 Z" fill="#e53935"/><path d="M30 62 Q50 80 70 62" fill="none" stroke="#3b2a1a" stroke-width="6" stroke-linecap="round"/>"##), YELLOW),
        ("emoji-sad".into(), Emoji, "sad cry 難過", face(&format!(r##"{EYES}<path d="M30 72 Q50 56 70 72" fill="none" stroke="#3b2a1a" stroke-width="6" stroke-linecap="round"/><path d="M65 50 Q60 60 65 64 Q70 60 65 50 Z" fill="#42a5f5"/>"##)), YELLOW),
        ("emoji-wow".into(), Emoji, "wow surprised 驚訝", face(&format!(r##"{EYES}<ellipse cx="50" cy="68" rx="10" ry="13" fill="#3b2a1a"/>"##)), YELLOW),
        ("emoji-cool".into(), Emoji, "cool sunglasses 酷", face(r##"<path d="M16 34 H84 V40 Q80 54 64 52 Q56 50 54 40 H46 Q44 50 36 52 Q20 54 16 40 Z" fill="#212121"/><path d="M32 64 Q50 76 68 62" fill="none" stroke="#3b2a1a" stroke-width="6" stroke-linecap="round"/>"##), YELLOW),
        ("emoji-angry".into(), Emoji, "angry mad 生氣", face(r##"<path d="M26 32 L42 40 M74 32 L58 40" stroke="#3b2a1a" stroke-width="6" stroke-linecap="round"/><circle cx="36" cy="46" r="5" fill="#3b2a1a"/><circle cx="64" cy="46" r="5" fill="#3b2a1a"/><path d="M32 72 Q50 60 68 72" fill="none" stroke="#3b2a1a" stroke-width="6" stroke-linecap="round"/>"##), [255, 112, 67, 255]),
        // Labels
        ("label-new".into(), Labels, "new 新品 新", label("NEW", "{F}", 34), RED),
        ("label-hot".into(), Labels, "hot 熱門 熱銷", label("HOT", "{F}", 34), [255, 87, 34, 255]),
        ("label-sale".into(), Labels, "sale discount 特價 優惠", label("SALE", "{F}", 30), [233, 30, 99, 255]),
        ("label-free".into(), Labels, "free 免費", label("FREE", "{F}", 30), GREEN),
        ("label-off".into(), Labels, "off percent discount 折扣", label("-50%", "{F}", 30), RED),
        ("label-zh-new".into(), Labels, "新品 new chinese", label("新品", "{F}", 32), RED),
        ("label-zh-hot".into(), Labels, "熱賣 hot chinese", label("熱賣", "{F}", 32), [255, 87, 34, 255]),
        ("label-zh-sale".into(), Labels, "限時特價 sale chinese", label("限時特價", "{F}", 20), [233, 30, 99, 255]),
        ("label-ribbon".into(), Labels, "ribbon banner 緞帶 橫幅", p("M2 34 H18 V26 H82 V34 H98 L90 50 L98 66 H82 V74 H18 V66 H2 L10 50 Z"), RED),
        // Social (generic, no trademarks)
        ("social-share".into(), Social, "share 分享", format!(r#"<path d="M30 50 L70 26 M30 50 L70 74" stroke="{{F}}" stroke-width="8"/>{}"#, p("M30 38 A12 12 0 1 1 29.9 38 Z M70 14 A12 12 0 1 1 69.9 14 Z M70 62 A12 12 0 1 1 69.9 62 Z")), BLUE),
        ("social-comment".into(), Social, "comment chat message 留言 訊息", p("M14 14 H86 Q92 14 92 20 V64 Q92 70 86 70 H40 L22 88 V70 H14 Q8 70 8 64 V20 Q8 14 14 14 Z"), BLUE),
        ("social-play".into(), Social, "play video 播放 影片", r#"<circle cx="50" cy="50" r="44" fill="{F}" stroke="{S}" stroke-width="{W}"/><path d="M40 30 L72 50 L40 70 Z" fill="white"/>"#.to_string(), [55, 71, 79, 255]),
        ("social-bell".into(), Social, "bell notification subscribe 鈴鐺 訂閱 通知", p("M50 8 C52 8 54 10 54 13 C68 16 76 28 76 44 V62 L86 74 H14 L24 62 V44 C24 28 32 16 46 13 C46 10 48 8 50 8 Z M40 80 H60 C60 86 56 92 50 92 C44 92 40 86 40 80 Z"), YELLOW),
        ("social-camera".into(), Social, "camera photo 相機 拍照", format!(r##"{}<circle cx="50" cy="54" r="16" fill="white"/><circle cx="50" cy="54" r="9" fill="{{F}}"/>"##, p("M10 30 H30 L38 18 H62 L70 30 H90 V82 H10 Z")), [156, 39, 176, 255]),
        ("social-at".into(), Social, "at mention 提及 標註", r#"<circle cx="50" cy="50" r="15" fill="none" stroke="{F}" stroke-width="10"/><path d="M65 36 V58 Q65 68 74 68 Q88 68 88 50 A38 38 0 1 0 70 82" fill="none" stroke="{F}" stroke-width="10" stroke-linecap="round"/>"#.into(), BLUE),
        ("social-hashtag".into(), Social, "hashtag tag 主題標籤", r#"<path d="M38 10 L30 90 M70 10 L62 90 M14 34 H90 M10 66 H86" stroke="{F}" stroke-width="10" stroke-linecap="round"/>"#.into(), BLUE),
        ("social-pin".into(), Social, "location pin map 地點 打卡", format!(r#"{}<circle cx="50" cy="38" r="12" fill="white"/>"#, p("M50 94 C30 66 18 52 18 38 C18 20 32 6 50 6 C68 6 82 20 82 38 C82 52 70 66 50 94 Z")), RED),
    ];
    for n in 1..=9 {
        v.push((
            format!("badge-{n}"),
            Badges,
            "number badge step 數字 步驟",
            digit_badge(n),
            RED,
        ));
    }
    v.push((
        "badge-plus".into(),
        Badges,
        "plus add 加",
        digit_badge(0).replace(">0<", ">+<"),
        GREEN,
    ));
    v.push((
        "badge-exclaim".into(),
        Badges,
        "exclamation important 重要",
        digit_badge(0).replace(">0<", ">!<"),
        [255, 87, 34, 255],
    ));
    v.push((
        "badge-question".into(),
        Badges,
        "question help 問題",
        digit_badge(0).replace(">0<", ">?<"),
        BLUE,
    ));
    v.push(("badge-seal".into(), Badges, "seal award best 推薦 獎章", p("M50 4 L60 14 L74 10 L78 24 L92 28 L88 42 L98 52 L88 62 L92 76 L78 80 L74 94 L60 90 L50 100 L40 90 L26 94 L22 80 L8 76 L12 62 L2 52 L12 42 L8 28 L22 24 L26 10 L40 14 Z"), YELLOW));
    v.into_iter()
        .map(|(id, category, keywords, svg, default_fill)| Sticker {
            id,
            category,
            keywords: keywords.to_owned(),
            svg,
            default_fill,
        })
        .collect()
}

/// Case-insensitive search across id and keywords.
pub fn search<'a>(
    lib: &'a [Sticker],
    query: &str,
    category: Option<StickerCategory>,
) -> Vec<&'a Sticker> {
    let q = query.trim().to_lowercase();
    lib.iter()
        .filter(|s| category.is_none_or(|c| s.category == c))
        .filter(|s| q.is_empty() || s.id.contains(&q) || s.keywords.to_lowercase().contains(&q))
        .collect()
}

pub fn find<'a>(lib: &'a [Sticker], id: &str) -> Option<&'a Sticker> {
    lib.iter().find(|s| s.id == id)
}

fn hex(c: [u8; 4]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// Build the final SVG document for a sticker with a style applied.
pub fn styled_svg(sticker: &Sticker, style: &StickerStyle) -> String {
    let w = style.stroke_width.max(0.0);
    let body = sticker
        .svg
        .replace(
            "stroke-width=\"calc\"",
            &format!("stroke-width=\"{}\"", 8.0 + w * 2.0),
        )
        .replace("{F}", &hex(style.fill))
        .replace(
            "{S}",
            &if w > 0.0 {
                hex(style.stroke)
            } else {
                "none".into()
            },
        )
        .replace("{W}", &format!("{w}"))
        .replace("{FONT}", FONT_STACK);
    let fo = f32::from(style.fill[3]) / 255.0;
    let shadow = if style.shadow {
        String::from(
            r#"<defs><filter id="sh" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="2.5" dy="3" stdDeviation="2.5" flood-color="black" flood-opacity="0.45"/></filter></defs>"#,
        )
    } else {
        String::new()
    };
    let filter = if style.shadow {
        r#" filter="url(#sh)""#
    } else {
        ""
    };
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-6 -6 112 112">{shadow}<g opacity="{fo}"{filter}>{body}</g></svg>"#
    )
}

/// Font fallback chain for sticker labels (Latin first, then Traditional Chinese).
const FONT_STACK: &str = "Arial, 'Segoe UI', Helvetica, 'DejaVu Sans', 'Liberation Sans', 'Noto Sans', \
'Microsoft JhengHei', 'PingFang TC', 'Heiti TC', 'Noto Sans CJK TC', 'Noto Sans TC', 'WenQuanYi Micro Hei', sans-serif";

fn font_db() -> &'static resvg::usvg::fontdb::Database {
    static DB: std::sync::OnceLock<resvg::usvg::fontdb::Database> = std::sync::OnceLock::new();
    DB.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        // Make the generic `sans-serif` resolve to something that exists on this machine.
        let have = |n: &str| {
            db.faces()
                .any(|f| f.families.iter().any(|(fam, _)| fam == n))
        };
        if let Some(f) = [
            "Arial",
            "Segoe UI",
            "Helvetica",
            "DejaVu Sans",
            "Liberation Sans",
            "Noto Sans",
        ]
        .into_iter()
        .find(|n| have(n))
        {
            db.set_sans_serif_family(f);
        }
        db
    })
}

/// Rasterize a sticker to an RGBA image of `width` x `height` pixels.
pub fn render_sticker(
    sticker: &Sticker,
    style: &StickerStyle,
    width: u32,
    height: u32,
) -> DynamicImage {
    let svg = styled_svg(sticker, style);
    let opt = resvg::usvg::Options {
        fontdb: std::sync::Arc::new(font_db().clone()),
        ..Default::default()
    };
    let (w, h) = (width.max(1), height.max(1));
    let mut out = RgbaImage::new(w, h);
    if let (Ok(tree), Some(mut pixmap)) = (
        resvg::usvg::Tree::from_str(&svg, &opt),
        resvg::tiny_skia::Pixmap::new(w, h),
    ) {
        let size = tree.size();
        let t = resvg::tiny_skia::Transform::from_scale(
            w as f32 / size.width(),
            h as f32 / size.height(),
        );
        resvg::render(&tree, t, &mut pixmap.as_mut());
        // tiny-skia uses premultiplied alpha
        for (dst, src) in out.pixels_mut().zip(pixmap.pixels()) {
            let c = src.demultiply();
            *dst = image::Rgba([c.red(), c.green(), c.blue(), c.alpha()]);
        }
    }
    DynamicImage::ImageRgba8(out)
}

/// Social media canvas presets (name, width, height).
pub const CANVAS_PRESETS: [(&str, u32, u32); 9] = [
    ("Instagram post 1:1", 1080, 1080),
    ("Instagram portrait 4:5", 1080, 1350),
    ("Instagram / Reels story 9:16", 1080, 1920),
    ("Facebook post / link 1.91:1", 1200, 630),
    ("X (Twitter) post 16:9", 1600, 900),
    ("YouTube thumbnail", 1280, 720),
    ("LINE post / 貼文", 1040, 1040),
    ("LINE rich message", 1040, 1040),
    ("Blog cover 16:9 (1920)", 1920, 1080),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_covers_required_categories() {
        let lib = library();
        assert!(lib.len() >= 50, "{}", lib.len());
        for c in StickerCategory::ALL {
            assert!(lib.iter().any(|s| s.category == c), "{c:?}");
        }
        let mut ids: Vec<_> = lib.iter().map(|s| &s.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), lib.len(), "unique ids");
    }

    #[test]
    fn every_sticker_renders_non_empty() {
        for s in library() {
            let style = StickerStyle {
                fill: s.default_fill,
                ..Default::default()
            };
            let img = render_sticker(&s, &style, 64, 64).to_rgba8();
            let opaque = img.pixels().filter(|p| p[3] > 0).count();
            assert!(opaque > 64, "{} rendered {opaque} px", s.id);
        }
    }

    #[test]
    fn recolor_changes_pixels() {
        let lib = library();
        let s = find(&lib, "heart").unwrap();
        let mut style = StickerStyle {
            shadow: false,
            stroke_width: 0.0,
            ..Default::default()
        };
        style.fill = [0, 0, 255, 255];
        let img = render_sticker(s, &style, 100, 100).to_rgba8();
        let c = img.get_pixel(50, 50);
        assert!(c[2] > 200 && c[0] < 40, "{c:?}");
    }

    #[test]
    fn search_works_in_both_languages() {
        let lib = library();
        assert!(search(&lib, "愛心", None).iter().any(|s| s.id == "heart"));
        assert!(
            search(&lib, "SALE", None)
                .iter()
                .any(|s| s.id == "label-sale")
        );
        assert_eq!(search(&lib, "", Some(StickerCategory::Badges)).len(), 13);
    }

    #[test]
    fn label_text_is_rendered() {
        // A label with text must have white glyph pixels inside the coloured plate.
        let lib = library();
        let s = find(&lib, "label-new").unwrap();
        let style = StickerStyle {
            fill: [229, 57, 53, 255],
            stroke_width: 0.0,
            shadow: false,
            ..Default::default()
        };
        let img = render_sticker(s, &style, 200, 200).to_rgba8();
        let white = img
            .pixels()
            .filter(|p| p[0] > 240 && p[1] > 240 && p[2] > 240 && p[3] > 240)
            .count();
        assert!(white > 200, "glyph pixels: {white}");
    }
}

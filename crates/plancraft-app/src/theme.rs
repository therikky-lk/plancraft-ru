//! Colors and egui styles for PlanCraft's light and dark themes.
//!
//! All drawing code reads colors from [`pal()`], the palette of the theme that
//! is currently shown. The deep-blue brand accent is shared by both themes; the
//! dark palette uses lighter tints of it where it is drawn on dark surfaces.

use std::sync::atomic::{AtomicU8, Ordering};

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Theme, ThemePreference, Visuals};
use serde::{Deserialize, Serialize};

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Every color the UI paints with. Field names follow the old constant names.
#[allow(non_snake_case)]
pub struct Palette {
    // Brand
    pub ACCENT: Color32,
    pub ACCENT_DARK: Color32,
    pub ACCENT_MID: Color32,
    pub ACCENT_LIGHT: Color32,
    pub ACCENT_PALE: Color32,
    pub TITLE_BAR: Color32,
    pub GOLD: Color32,
    // Surfaces & text
    pub TEXT: Color32,
    pub TEXT_ON_ACCENT: Color32,
    pub TEXT_WEAK: Color32,
    pub BG: Color32,
    pub PANEL_BG: Color32,
    pub HEADER_BG: Color32,
    pub INPUT_BG: Color32,
    pub TAB_HOVER: Color32,
    pub TRACK: Color32,
    pub SCROLL_THUMB: Color32,
    pub GRID_LINE: Color32,
    pub GRID_LINE_STRONG: Color32,
    pub SELECTION: Color32,
    pub SELECTION_CELL: Color32,
    pub SELECTION_TINT: Color32,
    pub NONWORKING: Color32,
    // Charts
    pub CHART_GRID: Color32,
    pub PROJECT_START: Color32,
    pub BAR_TASK: Color32,
    pub BAR_TASK_EDGE: Color32,
    pub BAR_PROGRESS: Color32,
    pub BAR_CRITICAL: Color32,
    pub BAR_CRITICAL_EDGE: Color32,
    pub BAR_SUMMARY: Color32,
    pub BAR_MANUAL: Color32,
    pub BAR_BASELINE: Color32,
    /// Bars of Baseline, Baseline1 … Baseline10 in the Multiple Baselines Gantt.
    pub BASELINE_SERIES: [Color32; 11],
    pub BAR_SLACK: Color32,
    pub BAR_INACTIVE: Color32,
    pub BAR_COMPLETE: Color32,
    pub MILESTONE: Color32,
    pub MEMBER_LINK: Color32,
    pub LINK: Color32,
    pub TODAY: Color32,
    pub STATUS_DATE: Color32,
    pub DEADLINE: Color32,
    pub TIMELINE_BAND: Color32,
    pub TIMELINE_TICK: Color32,
    pub TIMELINE_WINDOW: Color32,
    pub HEAT_LOW: Color32,
    pub HEAT_HIGH: Color32,
    pub OVER_LOW: Color32,
    pub OVER_HIGH: Color32,
    pub EXCEPTION_WORKING: Color32,
    pub EXCEPTION_OFF: Color32,
    // Status
    pub OK: Color32,
    pub WARN: Color32,
    pub ERROR: Color32,
}

/// Light theme: white work area, light gray ribbon, green accent.
pub const LIGHT: Palette = Palette {
    ACCENT: rgb(0x2B, 0x6E, 0x30),
    ACCENT_DARK: rgb(0x1D, 0x4F, 0x21),
    ACCENT_MID: rgb(0x2F, 0x7D, 0x34),
    ACCENT_LIGHT: rgb(0xCF, 0xE6, 0xD0),
    ACCENT_PALE: rgb(0xE6, 0xF2, 0xE6),
    TITLE_BAR: rgb(0x22, 0x5A, 0x26),
    GOLD: rgb(0xF2, 0xB1, 0x1C),
    TEXT: rgb(0x26, 0x26, 0x26),
    TEXT_ON_ACCENT: rgb(0xFF, 0xFF, 0xFF),
    TEXT_WEAK: rgb(0x5C, 0x5C, 0x5C),
    BG: rgb(0xFF, 0xFF, 0xFF),
    PANEL_BG: rgb(0xF3, 0xF3, 0xF3),
    HEADER_BG: rgb(0xF2, 0xF2, 0xF2),
    INPUT_BG: rgb(0xFF, 0xFF, 0xFF),
    TAB_HOVER: rgb(0xE4, 0xE4, 0xE4),
    TRACK: rgb(0xE6, 0xE6, 0xE6),
    SCROLL_THUMB: rgb(0xC6, 0xC6, 0xC6),
    GRID_LINE: rgb(0xE1, 0xE1, 0xE1),
    GRID_LINE_STRONG: rgb(0xC6, 0xC6, 0xC6),
    SELECTION: rgb(0xDD, 0xEC, 0xDD),
    SELECTION_CELL: rgb(0xC2, 0xDE, 0xC3),
    SELECTION_TINT: Color32::from_rgba_premultiplied(0x5C, 0x5C, 0x5C, 110),
    NONWORKING: rgb(0xF0, 0xF0, 0xF0),
    CHART_GRID: rgb(0xEC, 0xEC, 0xEC),
    PROJECT_START: rgb(0xB5, 0xB5, 0xB5),
    BAR_TASK: rgb(0x6F, 0x9F, 0xDB),
    BAR_TASK_EDGE: rgb(0x3E, 0x76, 0xBF),
    BAR_PROGRESS: rgb(0x1E, 0x46, 0x82),
    BAR_CRITICAL: rgb(0xD9, 0x4F, 0x4F),
    BAR_CRITICAL_EDGE: rgb(0xA8, 0x2E, 0x2E),
    BAR_SUMMARY: rgb(0x26, 0x26, 0x26),
    BAR_MANUAL: rgb(0x9E, 0xC1, 0xEA),
    BAR_BASELINE: rgb(0xA6, 0xA6, 0xA6),
    BASELINE_SERIES: [
        rgb(0x8A, 0x90, 0x9C),
        rgb(0x1F, 0x8A, 0x8A),
        rgb(0xC8, 0x8A, 0x12),
        rgb(0x7B, 0x4F, 0xB8),
        rgb(0x3C, 0x8D, 0x3A),
        rgb(0xC0, 0x4A, 0x7A),
        rgb(0x2F, 0x6F, 0xC8),
        rgb(0xA0, 0x5A, 0x2C),
        rgb(0x5C, 0x7A, 0x1E),
        rgb(0x8E, 0x3B, 0x3B),
        rgb(0x4A, 0x55, 0x6A),
    ],
    BAR_SLACK: rgb(0x2C, 0x9C, 0x8F),
    BAR_INACTIVE: rgb(0xE6, 0xE6, 0xE6),
    BAR_COMPLETE: rgb(0x92, 0xA8, 0xC6),
    MILESTONE: rgb(0x26, 0x26, 0x26),
    MEMBER_LINK: rgb(0xC8, 0x96, 0x1E),
    LINK: rgb(0x3E, 0x76, 0xBF),
    TODAY: rgb(0x2B, 0x6E, 0x30),
    STATUS_DATE: rgb(0x8E, 0x44, 0xAD),
    DEADLINE: rgb(0x2E, 0x8B, 0x57),
    TIMELINE_BAND: rgb(0xE6, 0xEE, 0xF8),
    TIMELINE_TICK: rgb(0xC8, 0xD4, 0xE4),
    TIMELINE_WINDOW: Color32::from_rgba_premultiplied(0x03, 0x06, 0x0F, 28),
    HEAT_LOW: rgb(0xE9, 0xF4, 0xE9),
    HEAT_HIGH: rgb(0x8C, 0xC7, 0x8F),
    OVER_LOW: rgb(0xF6, 0xC6, 0xC6),
    OVER_HIGH: rgb(0xE0, 0x6A, 0x6A),
    EXCEPTION_WORKING: rgb(0xC9, 0xE8, 0xD8),
    EXCEPTION_OFF: rgb(0xF4, 0xC7, 0xC7),
    OK: rgb(0x2E, 0x7D, 0x32),
    WARN: rgb(0xC0, 0x62, 0x00),
    ERROR: rgb(0xC6, 0x28, 0x28),
};

/// Builds a neutral (gray / black) dark palette with the green accent.
/// `bg` is the work area, `panel` the ribbon and side panels, `header` grid headers.
const fn dark_palette(
    title: Color32,
    bg: Color32,
    panel: Color32,
    header: Color32,
    input: Color32,
    hover: Color32,
    line: Color32,
    line_strong: Color32,
    nonworking: Color32,
    text_weak: Color32,
) -> Palette {
    Palette {
        ACCENT: rgb(0x2E, 0x7D, 0x32),
        ACCENT_DARK: rgb(0xB4, 0xE0, 0xB6),
        ACCENT_MID: rgb(0x7C, 0xC9, 0x80),
        ACCENT_LIGHT: rgb(0x2C, 0x45, 0x2E),
        ACCENT_PALE: hover,
        TITLE_BAR: title,
        GOLD: rgb(0xF2, 0xB1, 0x1C),
        TEXT: rgb(0xEC, 0xEC, 0xEC),
        TEXT_ON_ACCENT: rgb(0xFF, 0xFF, 0xFF),
        TEXT_WEAK: text_weak,
        BG: bg,
        PANEL_BG: panel,
        HEADER_BG: header,
        INPUT_BG: input,
        TAB_HOVER: hover,
        TRACK: line_strong,
        SCROLL_THUMB: rgb(0x60, 0x60, 0x60),
        GRID_LINE: line,
        GRID_LINE_STRONG: line_strong,
        SELECTION: rgb(0x26, 0x3A, 0x28),
        SELECTION_CELL: rgb(0x31, 0x4E, 0x34),
        SELECTION_TINT: Color32::from_rgba_premultiplied(0x0A, 0x0A, 0x0A, 120),
        NONWORKING: nonworking,
        CHART_GRID: line,
        PROJECT_START: rgb(0x60, 0x60, 0x60),
        BAR_TASK: rgb(0x4F, 0x86, 0xD0),
        BAR_TASK_EDGE: rgb(0x86, 0xAE, 0xEA),
        BAR_PROGRESS: rgb(0x14, 0x2C, 0x5A),
        BAR_CRITICAL: rgb(0xD9, 0x55, 0x55),
        BAR_CRITICAL_EDGE: rgb(0xF0, 0x8A, 0x8A),
        BAR_SUMMARY: rgb(0xD8, 0xD8, 0xD8),
        BAR_MANUAL: rgb(0x6F, 0x92, 0xC8),
        BASELINE_SERIES: [
            rgb(0x9A, 0xA1, 0xAE),
            rgb(0x4F, 0xC1, 0xC1),
            rgb(0xE8, 0xB0, 0x40),
            rgb(0xA9, 0x86, 0xE6),
            rgb(0x72, 0xC2, 0x6E),
            rgb(0xE8, 0x7D, 0xA8),
            rgb(0x6E, 0xA4, 0xF0),
            rgb(0xD0, 0x8C, 0x5C),
            rgb(0xA8, 0xC8, 0x5A),
            rgb(0xD8, 0x7A, 0x7A),
            rgb(0xA8, 0xB2, 0xC6),
        ],
        BAR_BASELINE: rgb(0x70, 0x70, 0x70),
        BAR_SLACK: rgb(0x3F, 0xB8, 0xA8),
        BAR_INACTIVE: line_strong,
        BAR_COMPLETE: rgb(0x5E, 0x6E, 0x88),
        MILESTONE: rgb(0xE8, 0xE8, 0xE8),
        MEMBER_LINK: rgb(0xD9, 0xA8, 0x3A),
        LINK: rgb(0x9A, 0xB8, 0xE6),
        TODAY: rgb(0x7C, 0xC9, 0x80),
        STATUS_DATE: rgb(0xB3, 0x7F, 0xD6),
        DEADLINE: rgb(0x4C, 0xC3, 0x8A),
        TIMELINE_BAND: header,
        TIMELINE_TICK: line_strong,
        TIMELINE_WINDOW: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 60),
        HEAT_LOW: rgb(0x22, 0x33, 0x23),
        HEAT_HIGH: rgb(0x3D, 0x7A, 0x42),
        OVER_LOW: rgb(0x5A, 0x2C, 0x2E),
        OVER_HIGH: rgb(0xA3, 0x3F, 0x42),
        EXCEPTION_WORKING: rgb(0x24, 0x4A, 0x30),
        EXCEPTION_OFF: rgb(0x5A, 0x2C, 0x2E),
        OK: rgb(0x6C, 0xC9, 0x70),
        WARN: rgb(0xF0, 0x93, 0x1E),
        ERROR: rgb(0xFF, 0x8A, 0x80),
    }
}

/// Dark: neutral dark gray.
pub const DARK: Palette = dark_palette(
    rgb(0x1B, 0x1B, 0x1B),
    rgb(0x1F, 0x1F, 0x1F),
    rgb(0x29, 0x29, 0x29),
    rgb(0x2D, 0x2D, 0x2D),
    rgb(0x33, 0x33, 0x33),
    rgb(0x38, 0x38, 0x38),
    rgb(0x33, 0x33, 0x33),
    rgb(0x48, 0x48, 0x48),
    rgb(0x25, 0x25, 0x25),
    rgb(0xA8, 0xA8, 0xA8),
);

/// Gray: a lighter dark gray.
pub const GRAY: Palette = dark_palette(
    rgb(0x2A, 0x2A, 0x2A),
    rgb(0x33, 0x33, 0x33),
    rgb(0x3B, 0x3B, 0x3B),
    rgb(0x40, 0x40, 0x40),
    rgb(0x47, 0x47, 0x47),
    rgb(0x4B, 0x4B, 0x4B),
    rgb(0x46, 0x46, 0x46),
    rgb(0x5A, 0x5A, 0x5A),
    rgb(0x39, 0x39, 0x39),
    rgb(0xBC, 0xBC, 0xBC),
);

/// Black (AMOLED): true black surfaces.
pub const BLACK: Palette = dark_palette(
    rgb(0x00, 0x00, 0x00),
    rgb(0x00, 0x00, 0x00),
    rgb(0x0A, 0x0A, 0x0A),
    rgb(0x10, 0x10, 0x10),
    rgb(0x16, 0x16, 0x16),
    rgb(0x1C, 0x1C, 0x1C),
    rgb(0x1E, 0x1E, 0x1E),
    rgb(0x30, 0x30, 0x30),
    rgb(0x0B, 0x0B, 0x0B),
    rgb(0x9E, 0x9E, 0x9E),
);

/// Which palette is on screen: 0 light, 1 dark, 2 gray, 3 black.
static ACTIVE: AtomicU8 = AtomicU8::new(0);
/// The dark palette used when the dark theme is showing (follows the user's choice).
static DARK_VARIANT: AtomicU8 = AtomicU8::new(1);

fn palette(i: u8) -> &'static Palette {
    match i {
        1 => &DARK,
        2 => &GRAY,
        3 => &BLACK,
        _ => &LIGHT,
    }
}

/// The palette of the theme currently on screen.
pub fn pal() -> &'static Palette {
    if PRINTING.with(|p| p.get()) { &LIGHT } else { palette(ACTIVE.load(Ordering::Relaxed)) }
}

thread_local! {
    static PRINTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Print on white paper without changing the application's selected theme.
pub fn on_paper<R>(paint: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            PRINTING.with(|p| p.set(self.0));
        }
    }
    let restore = Restore(PRINTING.with(|p| p.replace(true)));
    let result = paint();
    drop(restore);
    result
}

/// Whether a dark palette is on screen.
pub fn is_dark() -> bool {
    ACTIVE.load(Ordering::Relaxed) != 0
}

/// The user's theme choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeChoice {
    /// Follow the operating system.
    #[default]
    System,
    Light,
    Dark,
    Gray,
    Black,
}

impl ThemeChoice {
    pub const ALL: [ThemeChoice; 5] = [ThemeChoice::Light, ThemeChoice::Dark, ThemeChoice::Gray, ThemeChoice::Black, ThemeChoice::System];

    pub fn label(self) -> &'static str {
        match self {
            ThemeChoice::System => crate::tr("System"),
            ThemeChoice::Light => crate::tr("Light"),
            ThemeChoice::Dark => crate::tr("Dark"),
            ThemeChoice::Gray => crate::tr("Gray"),
            ThemeChoice::Black => crate::tr("Black (AMOLED)"),
        }
    }

    /// The dark palette this choice uses when dark.
    fn dark_variant(self) -> u8 {
        match self {
            ThemeChoice::Gray => 2,
            ThemeChoice::Black => 3,
            _ => 1,
        }
    }

    pub fn parse(s: &str) -> Option<ThemeChoice> {
        ThemeChoice::ALL.into_iter().find(|t| t.label().eq_ignore_ascii_case(s.trim()))
    }

    fn preference(self) -> ThemePreference {
        match self {
            ThemeChoice::System => ThemePreference::System,
            ThemeChoice::Light => ThemePreference::Light,
            ThemeChoice::Dark | ThemeChoice::Gray | ThemeChoice::Black => ThemePreference::Dark,
        }
    }
}

pub const ROW_HEIGHT: f32 = 22.0;
pub const HEADER_HEIGHT: f32 = 40.0;

/// Heat-map color for a utilization ratio (0 = idle, 1 = fully booked, >1 = overallocated).
pub fn utilization_color(u: f64) -> Color32 {
    let p = pal();
    if u <= 0.0 {
        return p.BG;
    }
    if u > 1.0001 {
        let t = ((u - 1.0) / 1.0).clamp(0.0, 1.0) as f32;
        return lerp(p.OVER_LOW, p.OVER_HIGH, t);
    }
    lerp(p.HEAT_LOW, p.HEAT_HIGH, u as f32)
}

pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

fn visuals(p: &Palette, dark: bool) -> Visuals {
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = p.BG;
    v.window_fill = p.BG;
    v.faint_bg_color = p.PANEL_BG;
    v.extreme_bg_color = p.BG;
    v.code_bg_color = p.HEADER_BG;
    v.hyperlink_color = p.ACCENT_MID;
    v.selection.bg_fill = if dark { p.SELECTION_CELL } else { p.ACCENT_LIGHT };
    v.selection.stroke = Stroke::new(1.0, if dark { p.TEXT } else { p.ACCENT });
    v.window_corner_radius = CornerRadius::same(6);
    v.menu_corner_radius = CornerRadius::same(4);
    v.window_stroke = Stroke::new(1.0, p.GRID_LINE_STRONG);
    v.override_text_color = None;
    v.warn_fg_color = p.WARN;
    v.error_fg_color = p.ERROR;
    let r = CornerRadius::same(3);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.TEXT);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.GRID_LINE);
    v.widgets.noninteractive.bg_fill = p.BG;
    v.widgets.noninteractive.weak_bg_fill = p.BG;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.TEXT);
    v.widgets.inactive.weak_bg_fill = p.INPUT_BG;
    v.widgets.inactive.bg_fill = p.INPUT_BG;
    v.widgets.inactive.corner_radius = r;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.GRID_LINE_STRONG);
    v.text_edit_bg_color = Some(if dark { p.INPUT_BG } else { p.BG });
    v.widgets.hovered.weak_bg_fill = p.ACCENT_PALE;
    v.widgets.hovered.bg_fill = p.ACCENT_PALE;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.ACCENT_MID);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.ACCENT_DARK);
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.weak_bg_fill = p.ACCENT_LIGHT;
    v.widgets.active.bg_fill = p.ACCENT_LIGHT;
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.0, p.ACCENT_DARK);
    v.widgets.active.corner_radius = r;
    v.widgets.open.weak_bg_fill = p.ACCENT_LIGHT;
    v.widgets.open.bg_fill = p.ACCENT_LIGHT;
    v.widgets.open.fg_stroke = Stroke::new(1.0, p.TEXT);
    v.widgets.open.corner_radius = r;
    v
}

/// Installs PlanCraft's styles for both themes and applies the user's choice.
pub fn install(ctx: &egui::Context, choice: ThemeChoice) {
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 3.0);
        style.spacing.interact_size.y = 22.0;
    });
    ctx.style_mut_of(Theme::Light, |s| s.visuals = visuals(&LIGHT, false));
    ctx.style_mut_of(Theme::Dark, |s| s.visuals = visuals(&DARK, true));
    set_choice(ctx, choice);
}

/// Switches between system, light and the dark variants.
pub fn set_choice(ctx: &egui::Context, choice: ThemeChoice) {
    let v = choice.dark_variant();
    DARK_VARIANT.store(v, Ordering::Relaxed);
    ctx.style_mut_of(Theme::Dark, |s| s.visuals = visuals(palette(v), true));
    ctx.set_theme(choice.preference());
    sync(ctx);
}

/// Points [`pal()`] at the theme egui is currently showing (call once per frame,
/// so following the system theme updates live).
pub fn sync(ctx: &egui::Context) {
    let v = if ctx.theme() == Theme::Dark { DARK_VARIANT.load(Ordering::Relaxed) } else { 0 };
    ACTIVE.store(v, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::{BLACK, DARK, GRAY, LIGHT, Palette};
    use egui::Color32;

    /// WCAG relative luminance.
    fn luminance(c: Color32) -> f64 {
        let ch = |v: u8| {
            let v = v as f64 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f64 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn both_themes_keep_text_readable() {
        for (name, p) in [("light", &LIGHT), ("dark", &DARK), ("gray", &GRAY), ("black", &BLACK)] {
            let p: &Palette = p;
            for (what, bg) in [("BG", p.BG), ("PANEL_BG", p.PANEL_BG), ("HEADER_BG", p.HEADER_BG), ("SELECTION", p.SELECTION)] {
                assert!(contrast(p.TEXT, bg) >= 7.0, "{name}: TEXT on {what} is {:.1}:1", contrast(p.TEXT, bg));
                assert!(contrast(p.TEXT_WEAK, bg) >= 4.5, "{name}: TEXT_WEAK on {what} is {:.1}:1", contrast(p.TEXT_WEAK, bg));
            }
            assert!(contrast(Color32::WHITE, p.TITLE_BAR) >= 7.0, "{name}: title bar text");
            assert!(contrast(Color32::WHITE, p.ACCENT) >= 4.5, "{name}: text on accent buttons");
            assert!(contrast(p.ERROR, p.BG) >= 4.5, "{name}: error text");
        }
    }
    /// Glyphs the bundled egui fonts cannot render (they show as empty boxes).
    /// Found by rendering a glyph sheet; extend when a new icon turns out to be missing.
    const MISSING_GLYPHS: &str = "→↶↷⊘▤▸◆●⮜🗂🗒🤖ⓘ⊕⊖⇄✎⬚▦◯🗓✓✗✘⬤▢◈◇❖✜🧭🧮🗃🧑⏩▬↑↓⇥⇤⇹";

    fn sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                sources(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }

    #[test]
    fn ui_text_avoids_glyphs_missing_from_bundled_fonts() {
        let mut files = Vec::new();
        sources(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let mut problems = Vec::new();
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap();
            for (n, line) in text.lines().enumerate() {
                let code = line.trim_start();
                if code.starts_with("//") || code.starts_with("const MISSING_GLYPHS") {
                    continue;
                }
                for c in line.chars().filter(|c| MISSING_GLYPHS.contains(*c)) {
                    problems.push(format!("{}:{}: '{c}'", f.display(), n + 1));
                }
            }
        }
        assert!(problems.is_empty(), "glyphs that render as boxes:\n{}", problems.join("\n"));
    }
}

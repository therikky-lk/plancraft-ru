//! The title band, the tab row and the ribbon (Office 2016–2024 style: large and small
//! buttons in captioned groups, a green "File" tab that opens the File page).
//!
//! Layout:
//!
//! ```text
//! ┌──────────┬───────────────────────────────────────────────────────────┐
//! │  view    │ File ▾  Task  Resource  Project  Report  View  Format      │  tabs
//! │ selector ├───────────────────────────────────────────────────────────┤
//! │    ▾     │ [+ Task] [♦ Milestone] │ ⬅ ➡ │ [🔗 Link] … single toolbar  │  toolbar
//! └──────────┴───────────────────────────────────────────────────────────┘
//! ```
//!
//! The view selector spans both rows. Tabs are plain tabs, and the toolbar is
//! one row of compact inline buttons separated by thin dividers.

use std::cell::RefCell;

use egui::{Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};
use plancraft_core::model::ScheduleMode;

use crate::columns::TaskTable;
use crate::commands::Command;
use crate::logo;
use crate::state::{MenuTab, TaskFilter, UiState, ViewKind};
use crate::theme;

pub struct MenuInfo {
    pub has_doc: bool,
    pub title: String,
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_label: Option<String>,
    pub task_selected: bool,
    pub resource_selected: bool,
    pub mcp_running: bool,
    pub mcp_url: Option<String>,
    /// Fields offered for sorting and grouping: (key, title), the table's columns first.
    pub fields: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// Title band
// ---------------------------------------------------------------------------

/// The deep-blue band at the top: logo, quick access buttons, document title, MCP status.
pub fn title_bar(ui: &mut Ui, info: &MenuInfo, out: &mut Vec<Command>) {
    let rect = ui.available_rect_before_wrap();
    let rect = Rect::from_min_size(rect.min, vec2(rect.width(), 34.0));
    ui.painter().rect_filled(rect, 0.0, theme::pal().TITLE_BAR);
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(8.0, 0.0))).layout(Layout::left_to_right(Align::Center)));
    let ui2 = &mut child;
    logo::paint_on_dark(ui2.painter(), Rect::from_center_size(pos2(ui2.cursor().left() + 11.0, rect.center().y), vec2(22.0, 22.0)));
    ui2.add_space(28.0);
    ui2.label(egui::RichText::new("PlanCraft").color(Color32::WHITE).size(14.0).strong());
    ui2.add_space(12.0);
    let qa = |ui: &mut Ui, icon: &str, tip: &str, enabled: bool| -> bool {
        let (r, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), if enabled { Sense::click() } else { Sense::hover() });
        if enabled && resp.hovered() {
            ui.painter().rect_filled(r, 3.0, Color32::from_white_alpha(30));
        }
        let color = if enabled { Color32::WHITE } else { Color32::from_white_alpha(90) };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, icon, FontId::proportional(14.0), color);
        resp.on_hover_text(tip).clicked()
    };
    if qa(ui2, "💾", crate::tr("Save (Ctrl+S)"), info.has_doc) {
        out.push(Command::Save);
    }
    let undo_tip = match &info.undo_label {
        Some(l) => crate::tr_dyn(&format!("Undo {l} (Ctrl+Z)")),
        None => crate::tr("Undo (Ctrl+Z)").into(),
    };
    if qa(ui2, "⟲", &undo_tip, info.can_undo) {
        out.push(Command::Undo);
    }
    if qa(ui2, "⟳", crate::tr("Redo (Ctrl+Y)"), info.can_redo) {
        out.push(Command::Redo);
    }
    let title = if info.has_doc { crate::tr_dyn(&format!("{} - PlanCraft", info.title)) } else { "PlanCraft".into() };
    ui2.painter().text(rect.center(), Align2::CENTER_CENTER, title, FontId::proportional(13.0), Color32::WHITE);
    ui2.with_layout(Layout::right_to_left(Align::Center), |ui| {
        #[cfg(target_os = "windows")]
        window_buttons(ui);
        let (txt, color) = if info.mcp_running {
            (crate::tr("⚡ MCP live"), Color32::from_rgb(0x8E, 0xF0, 0xB0))
        } else {
            (crate::tr("○ MCP off"), Color32::from_white_alpha(150))
        };
        let resp = ui.add(egui::Label::new(egui::RichText::new(txt).color(color).size(12.0)).sense(Sense::click()));
        let tip = match &info.mcp_url {
            Some(u) => crate::tr_dyn(&format!("AI agents can control this window via MCP at {u}\nClick to manage.")),
            None => crate::tr("Start the built-in MCP server so AI agents (e.g. Claude Code) can work with your projects.").into(),
        };
        if resp.on_hover_text(tip).clicked() {
            out.push(Command::McpPanel);
        }
    });
    #[cfg(target_os = "windows")]
    {
        // Only the empty middle of the band moves the window; controls keep their own clicks.
        let drag_rect = Rect::from_min_max(
            pos2(rect.left() + 220.0, rect.top() + 6.0),
            pos2((rect.right() - 250.0).max(rect.left() + 220.0), rect.bottom()),
        );
        let (pressed, double_clicked) = ui.input(|i| {
            let inside = i.pointer.interact_pos().is_some_and(|pos| drag_rect.contains(pos));
            (inside && i.pointer.primary_pressed(), inside && i.pointer.button_double_clicked(egui::PointerButton::Primary))
        });
        if double_clicked {
            let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        } else if pressed {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
    }
    let spacing = ui.spacing().item_spacing.y;
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.allocate_rect(rect, Sense::hover());
    ui.spacing_mut().item_spacing.y = spacing;
}

#[cfg(target_os = "windows")]
fn window_buttons(ui: &mut Ui) {
    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
    for (label, tip, command) in [
        ("×", "Close", egui::ViewportCommand::Close),
        (if maximized { "❐" } else { "□" }, if maximized { "Restore" } else { "Maximize" }, egui::ViewportCommand::Maximized(!maximized)),
        ("−", "Minimize", egui::ViewportCommand::Minimized(true)),
    ] {
        let (rect, response) = ui.allocate_exact_size(vec2(42.0, 34.0), Sense::click());
        if response.hovered() {
            ui.painter().rect_filled(rect, 0.0, Color32::from_white_alpha(35));
        }
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(18.0), Color32::WHITE);
        if response.on_hover_text(crate::tr(tip)).clicked() {
            ui.ctx().send_viewport_cmd(command);
        }
    }
    ui.add_space(8.0);
}

// ---------------------------------------------------------------------------
// Ribbon widgets
// ---------------------------------------------------------------------------

const BUTTON_H: f32 = 22.0;
/// Height of the ribbon's button area (above the group captions).
const BODY_H: f32 = 70.0;
const CAPTION_H: f32 = 17.0;
const TAB_ROW_H: f32 = 28.0;
pub const RIBBON_H: f32 = BODY_H + CAPTION_H + 8.0;

#[derive(Clone, Default)]
struct RibbonGroup {
    caption: String,
    widths: [Option<f32>; 3],
    mode: usize,
}

fn ribbon_plan(groups: &mut [RibbonGroup], available: f32) {
    for group in groups.iter_mut() {
        group.mode = 0;
    }
    let width = |g: &RibbonGroup, mode: usize| {
        let full = g.widths.first().copied().flatten().unwrap_or(100.0);
        g.widths.get(mode).copied().flatten().unwrap_or(if mode == 1 { full * 0.7 } else { 80.0 }).min(full)
    };
    let mut total: f32 = groups.iter().map(|g| width(g, 0)).sum();
    // Reduce one group at a time, keeping left-hand primary commands visible.
    // Try every compact representation before folding any group into a menu.
    for mode in 1..=2 {
        for group in groups.iter_mut().rev() {
            if total <= available {
                return;
            }
            let next = width(group, mode);
            let previous = width(group, group.mode);
            if next < previous {
                total -= previous - next;
                group.mode = mode;
            }
        }
    }
}

fn icon_only(ui: &Ui) -> bool {
    ui.ctx().data(|data| data.get_temp::<bool>(egui::Id::new("ribbon_icon_only"))).unwrap_or(false)
}

fn icon_color(enabled: bool) -> Color32 {
    if enabled { theme::pal().ACCENT_MID } else { theme::pal().GRID_LINE_STRONG }
}

fn hover_fill(ui: &Ui, rect: Rect, hovered: bool, active: bool) {
    let p = ui.painter();
    if active {
        p.rect_filled(rect, 2.0, theme::pal().ACCENT_LIGHT);
        p.rect_stroke(rect, 2.0, Stroke::new(1.0, theme::pal().ACCENT_MID), StrokeKind::Inside);
    } else if hovered {
        p.rect_filled(rect, 2.0, theme::pal().ACCENT_PALE);
        p.rect_stroke(rect, 2.0, Stroke::new(1.0, theme::pal().ACCENT_LIGHT), StrokeKind::Inside);
    }
}

/// Large ribbon button: icon above a label of up to two lines.
fn big(ui: &mut Ui, icon: &str, label: &str, tip: &str, enabled: bool, active: bool) -> egui::Response {
    if icon_only(ui) {
        ui.ctx().data_mut(|data| data.insert_temp(egui::Id::new("ribbon_icon_only"), false));
        let response = big(ui, icon, "", tip, enabled, active).on_hover_text(label);
        ui.ctx().data_mut(|data| data.insert_temp(egui::Id::new("ribbon_icon_only"), true));
        return response;
    }
    let font = FontId::proportional(11.5);
    let text_color = if enabled { theme::pal().TEXT } else { theme::pal().TEXT_WEAK };
    // Wrap at spaces only: as wide as the longest word, and about half the label per line.
    let measure = |t: &str| ui.painter().layout_no_wrap(t.to_string(), font.clone(), text_color).size().x;
    let longest = label.split_whitespace().map(measure).fold(0.0f32, f32::max);
    let total = measure(label);
    let wrap = if total <= 70.0 { total + 2.0 } else { (longest + 2.0).max((total / 2.0 + 20.0).min(124.0)) };
    let galley = ui.painter().layout(label.to_string(), font, text_color, wrap);
    let w = (galley.size().x + 10.0).max(46.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, BODY_H), if enabled { Sense::click() } else { Sense::hover() });
    hover_fill(ui, rect, enabled && resp.hovered(), active);
    let p = ui.painter();
    if icon == "printer" {
        let c = pos2(rect.center().x, rect.top() + 18.0);
        let stroke = Stroke::new(1.6, icon_color(enabled));
        p.rect_stroke(Rect::from_min_max(c + vec2(-7.0, -12.0), c + vec2(7.0, -5.0)), 0.5, stroke, StrokeKind::Inside);
        p.line_segment([c + vec2(-11.0, 6.0), c + vec2(-11.0, -5.0)], stroke);
        p.line_segment([c + vec2(-11.0, -5.0), c + vec2(11.0, -5.0)], stroke);
        p.line_segment([c + vec2(11.0, -5.0), c + vec2(11.0, 6.0)], stroke);
        p.line_segment([c + vec2(-11.0, 6.0), c + vec2(-7.0, 6.0)], stroke);
        p.line_segment([c + vec2(7.0, 6.0), c + vec2(11.0, 6.0)], stroke);
        p.rect_stroke(Rect::from_min_max(c + vec2(-7.0, 1.0), c + vec2(7.0, 10.0)), 0.5, stroke, StrokeKind::Inside);
        p.line_segment([c + vec2(-4.0, 4.0), c + vec2(4.0, 4.0)], stroke);
        p.line_segment([c + vec2(-4.0, 7.0), c + vec2(4.0, 7.0)], stroke);
        p.circle_filled(c + vec2(7.0, -2.0), 1.0, icon_color(enabled));
    } else {
        p.text(pos2(rect.center().x, rect.top() + 18.0), Align2::CENTER_CENTER, icon, FontId::proportional(24.0), icon_color(enabled));
    }
    let gp = pos2(rect.center().x - galley.size().x / 2.0, rect.top() + 36.0);
    // Center every line of the label.
    let lines = galley.rows.len();
    if lines <= 1 {
        p.galley(gp, galley, text_color);
    } else {
        let mut y = rect.top() + 36.0;
        for row in &galley.rows {
            let t = row.text();
            p.text(pos2(rect.center().x, y), Align2::CENTER_TOP, t, FontId::proportional(11.5), text_color);
            y += 13.0;
        }
    }
    let resp = if tip.is_empty() { resp } else { resp.on_hover_text(tip) };
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// Small ribbon button: icon and label in one row (three fit under each other).
fn small(ui: &mut Ui, icon: &str, label: &str, tip: &str, enabled: bool, active: bool) -> egui::Response {
    let original_label = label;
    let label = if icon_only(ui) { "" } else { label };
    let font = FontId::proportional(12.0);
    let label_w =
        if label.is_empty() { 0.0 } else { ui.painter().layout_no_wrap(label.to_string(), font.clone(), theme::pal().TEXT).size().x + 8.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0 + label_w, BUTTON_H), if enabled { Sense::click() } else { Sense::hover() });
    hover_fill(ui, rect, enabled && resp.hovered(), active);
    let p = ui.painter();
    p.text(pos2(rect.left() + 12.0, rect.center().y), Align2::CENTER_CENTER, icon, FontId::proportional(14.0), icon_color(enabled));
    if !label.is_empty() {
        p.text(
            pos2(rect.left() + 24.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            font,
            if enabled { theme::pal().TEXT } else { theme::pal().TEXT_WEAK },
        );
    }
    let resp = if tip.is_empty() { resp.on_hover_text(original_label) } else { resp.on_hover_text(tip) };
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// A collapsed group is a palette, not a menu item: its combo boxes need the
/// shared popup memory for their own lists, independently of the palette.
fn group_popup(response: &egui::Response, add: impl FnOnce(&mut Ui)) {
    let state_id = response.id.with("group_palette_open");
    let mut open = response.ctx.data(|data| data.get_temp::<bool>(state_id)).unwrap_or(false);
    if response.clicked() {
        open = !open;
    }
    let behavior = if egui::Popup::is_any_open(&response.ctx) {
        // A combo's list may extend beyond the palette. Clicking an option must
        // not dismiss its parent before the combo can process the selection.
        egui::PopupCloseBehavior::IgnoreClicks
    } else {
        egui::PopupCloseBehavior::CloseOnClickOutside
    };
    egui::Popup::from_response(response).open_bool(&mut open).close_behavior(behavior).show(add);
    response.ctx.data_mut(|data| data.insert_temp(state_id, open));
}

/// A ribbon group: contents laid out left to right, the caption centered below, a divider after.
fn group(ui: &mut Ui, caption: &str, add: impl FnOnce(&mut Ui)) {
    let layout_id = egui::Id::new("ribbon_groups");
    let mut groups = ui.ctx().data(|data| data.get_temp::<Vec<RibbonGroup>>(layout_id)).unwrap_or_default();
    let index = groups.iter().position(|g| g.caption == caption).unwrap_or(groups.len());
    if index == groups.len() {
        groups.push(RibbonGroup { caption: caption.into(), ..Default::default() });
    }
    let mode = groups.get(index).map_or(0, |g| g.mode);
    let start = ui.cursor().left();
    ui.ctx().data_mut(|data| data.insert_temp(egui::Id::new("ribbon_icon_only"), mode == 1));
    if mode == 2 {
        // Retain the familiar ribbon silhouette: a large icon, wrapped caption,
        // and dropdown indicator, rather than a tall text-only button.
        let icon = if caption == crate::tr("Clipboard") {
            "📋"
        } else if caption == crate::tr("Properties") {
            "ℹ"
        } else if caption == crate::tr("Insert") {
            "➕"
        } else if caption == crate::tr("Zoom") {
            "🔍"
        } else {
            "☰"
        };
        let response = big(ui, icon, &format!("{caption} ⏷"), caption, true, false);
        group_popup(&response, |ui| {
            // A group can contain several controls, including combo boxes and text
            // fields. Keep the original controls together inside the popup.
            ui.set_max_width((ui.ctx().content_rect().width() - 32.0).max(100.0));
            egui::ScrollArea::horizontal().id_salt("ribbon_group_scroll").show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.set_height(BODY_H);
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.spacing_mut().interact_size.y = BUTTON_H;
                    ui.spacing_mut().button_padding.y = 2.0;
                    add(ui);
                });
            });
        });
        if let Some(width) = groups.get_mut(index).and_then(|group| group.widths.get_mut(mode)) {
            *width = Some(ui.cursor().left() - start);
        }
        ui.ctx().data_mut(|data| data.insert_temp(layout_id, groups));
        return;
    }
    let cap_w = ui.painter().layout_no_wrap(caption.to_string(), FontId::proportional(11.0), theme::pal().TEXT_WEAK).size().x + 12.0;
    let r = ui
        .vertical(|ui| {
            // The parent ribbon body is inset by 3 px; remove egui's default vertical
            // item gap so the caption row stays inside the fixed-height body.
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.set_min_width(cap_w);
            ui.horizontal_top(|ui| {
                ui.set_height(BODY_H);
                ui.spacing_mut().item_spacing.x = 2.0;
                // Combo boxes default to a 28 px hit target. Three stacked rows
                // then exceed BODY_H and push this group's caption below its peers.
                ui.spacing_mut().interact_size.y = BUTTON_H;
                ui.spacing_mut().button_padding.y = 2.0;
                add(ui);
            });
            ui.add_space(CAPTION_H);
        })
        .response
        .rect;
    let p = ui.painter().clone();
    p.text(
        // Anchor captions and separators to the shared ribbon grid, not to the
        // group's measured height: egui controls can make one group's response
        // taller than BODY_H even though the visible ribbon is fixed-height.
        pos2(r.center().x, r.top() + BODY_H + CAPTION_H / 2.0),
        Align2::CENTER_CENTER,
        caption,
        FontId::proportional(11.0),
        theme::pal().TEXT_WEAK,
    );
    ui.add_space(4.0);
    let x = ui.cursor().left();
    p.vline(x, (r.top() + 4.0)..=(r.top() + BODY_H + CAPTION_H - 4.0), Stroke::new(1.0, theme::pal().GRID_LINE_STRONG));
    ui.add_space(6.0);
    ui.ctx().data_mut(|data| data.insert_temp(egui::Id::new("ribbon_icon_only"), false));
    if let Some(width) = groups.get_mut(index).and_then(|group| group.widths.get_mut(mode)) {
        *width = Some(ui.cursor().left() - start);
    }
    ui.ctx().data_mut(|data| data.insert_temp(layout_id, groups));
}

/// Up to three small controls stacked vertically.
fn stack(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.add_space(1.0);
        add(ui);
    });
}

fn row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.set_height(BUTTON_H);
        ui.spacing_mut().item_spacing.x = 1.0;
        add(ui);
    });
}

/// "Label: [combo]" row inside a stack.
fn labeled_combo(ui: &mut Ui, icon: &str, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.set_height(BUTTON_H);
        if !icon.is_empty() {
            ui.label(egui::RichText::new(icon).size(13.0).color(theme::pal().ACCENT_MID));
        }
        ui.add_sized([78.0, BUTTON_H], egui::Label::new(egui::RichText::new(label).size(12.0)).truncate());
        add(ui);
    });
}

// ---------------------------------------------------------------------------
// Tab row (on the title band, like Office)
// ---------------------------------------------------------------------------

fn tab_strip(ui: &mut Ui, rect: Rect, st: &mut UiState, push: &dyn Fn(Command)) {
    let p = ui.painter().clone();
    p.rect_filled(rect, 0.0, theme::pal().TITLE_BAR);
    let font = FontId::proportional(13.0);
    let mut x = rect.left() + 4.0;
    let on_band = Color32::WHITE;

    // "File" opens the full-window File page (backstage).
    let w = p.layout_no_wrap(crate::tr("File").into(), font.clone(), on_band).size().x + 28.0;
    let r = Rect::from_min_max(pos2(x, rect.top() + 2.0), pos2(x + w, rect.bottom()));
    let resp = ui.interact(r, egui::Id::new("tab_file"), Sense::click());
    let file_fill = if theme::is_dark() { theme::pal().ACCENT } else { theme::lerp(theme::pal().TITLE_BAR, Color32::BLACK, 0.18) };
    p.rect_filled(r, 0.0, if resp.hovered() { theme::pal().ACCENT_MID } else { file_fill });
    p.text(r.center(), Align2::CENTER_CENTER, crate::tr("File"), font.clone(), on_band);
    if resp.clicked() {
        push(Command::StartPage);
    }
    x += w + 4.0;

    for tab in MenuTab::ALL {
        let w = p.layout_no_wrap(tab.label().into(), font.clone(), on_band).size().x + 24.0;
        let r = Rect::from_min_max(pos2(x, rect.top() + 2.0), pos2(x + w, rect.bottom()));
        let resp = ui.interact(r, egui::Id::new(("tab", tab.label())), Sense::click());
        let active = st.menu_tab == tab && !st.ribbon_collapsed;
        if active {
            p.rect_filled(r, CornerRadius { nw: 2, ne: 2, sw: 0, se: 0 }, theme::pal().PANEL_BG);
        } else if resp.hovered() {
            p.rect_filled(r, CornerRadius { nw: 2, ne: 2, sw: 0, se: 0 }, Color32::from_white_alpha(28));
        }
        let color = if active { theme::pal().ACCENT_MID } else { on_band };
        p.text(r.center(), Align2::CENTER_CENTER, tab.label(), font.clone(), color);
        if resp.double_clicked() {
            st.ribbon_collapsed = !st.ribbon_collapsed;
        } else if resp.clicked() {
            if st.ribbon_collapsed && st.menu_tab == tab {
                st.ribbon_collapsed = false;
            }
            st.menu_tab = tab;
        }
        x += w + 2.0;
    }

    // Contextual tab caption, as Office shows above "Format".
    if st.menu_tab == MenuTab::Format && matches!(st.view, ViewKind::Gantt | ViewKind::TrackingGantt) {
        p.text(
            pos2(x + 10.0, rect.center().y),
            Align2::LEFT_CENTER,
            crate::tr("Gantt Chart Tools"),
            FontId::proportional(11.5),
            Color32::from_white_alpha(190),
        );
    }

    // Right side: collapse/expand the ribbon, help.
    let r = Rect::from_center_size(pos2(rect.right() - 44.0, rect.center().y + 1.0), vec2(22.0, 20.0));
    let resp = ui.interact(r, egui::Id::new("ribbon_toggle"), Sense::click());
    if resp.hovered() {
        p.rect_filled(r, 2.0, Color32::from_white_alpha(28));
    }
    p.text(r.center(), Align2::CENTER_CENTER, if st.ribbon_collapsed { "⏷" } else { "⏶" }, FontId::proportional(12.0), on_band);
    if resp.on_hover_text(crate::tr("Collapse or expand the ribbon (double-click a tab)")).clicked() {
        st.ribbon_collapsed = !st.ribbon_collapsed;
    }
    let r = Rect::from_center_size(pos2(rect.right() - 16.0, rect.center().y + 1.0), vec2(22.0, 20.0));
    let resp = ui.interact(r, egui::Id::new("tab_help"), Sense::click());
    if resp.hovered() {
        p.rect_filled(r, 2.0, Color32::from_white_alpha(28));
    }
    p.text(r.center(), Align2::CENTER_CENTER, "?", FontId::proportional(13.0), on_band);
    if resp.on_hover_text(crate::tr("About PlanCraft")).clicked() {
        push(Command::About);
    }
}

// ---------------------------------------------------------------------------
// Ribbon contents per tab
// ---------------------------------------------------------------------------

fn view_button(ui: &mut Ui, st: &UiState, enabled: bool, push: &dyn Fn(Command)) {
    let resp = big(ui, st.view.icon(), &format!("{} ⏷", st.view.label()), crate::tr("Change view"), enabled, false);
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(220.0);
        ui.label(egui::RichText::new(crate::tr("Task views")).small().color(theme::pal().TEXT_WEAK));
        for v in ViewKind::ALL {
            if v == ViewKind::ResourceSheet {
                ui.separator();
                ui.label(egui::RichText::new(crate::tr("Resource views")).small().color(theme::pal().TEXT_WEAK));
            }
            if ui.selectable_label(st.view == v, format!("{}  {}", v.icon(), v.label())).clicked() {
                push(Command::SetView(v));
            }
        }
    });
}

fn ribbon(ui: &mut Ui, st: &mut UiState, info: &MenuInfo, push: &dyn Fn(Command)) {
    let doc = info.has_doc;
    let sel = info.task_selected && doc;
    let res = info.resource_selected && doc;
    let bg = |ui: &mut Ui, icon: &str, label: &str, tip: &str, enabled: bool, cmd: Command| {
        let response = big(ui, icon, label, tip, enabled, false);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
        if response.clicked() {
            push(cmd);
        }
    };
    let sm = |ui: &mut Ui, icon: &str, label: &str, tip: &str, enabled: bool, cmd: Command| {
        if small(ui, icon, label, tip, enabled, false).clicked() {
            push(cmd);
        }
    };
    match st.menu_tab {
        MenuTab::Task => {
            group(ui, crate::tr("View"), |ui| view_button(ui, st, doc, push));
            group(ui, crate::tr("Clipboard"), |ui| {
                bg(ui, "📥", crate::tr("Paste"), crate::tr("Paste (Ctrl+V)"), doc && !st.clipboard.is_empty(), Command::Paste);
                stack(ui, |ui| {
                    sm(ui, "✂", crate::tr("Cut"), crate::tr("Cut (Ctrl+X)"), sel, Command::Cut);
                    sm(ui, "📋", crate::tr("Copy"), crate::tr("Copy (Ctrl+C)"), sel, Command::Copy);
                });
            });
            group(ui, crate::tr("Scheduling"), |ui| {
                stack(ui, |ui| {
                    row(ui, |ui| {
                        for pct in [0u8, 25, 50, 75, 100] {
                            if pie(ui, pct, sel).clicked() {
                                push(Command::SetPercent(pct));
                            }
                        }
                    });
                    row(ui, |ui| {
                        sm(ui, "⬅", "", crate::tr("Outdent (Alt+Shift+Left)"), sel, Command::Outdent);
                        sm(ui, "➡", "", crate::tr("Indent (Alt+Shift+Right)"), sel, Command::Indent);
                        sm(ui, "🔗", "", crate::tr("Link the selected tasks finish-to-start (Ctrl+F2)"), sel, Command::LinkSelected);
                        sm(ui, "⛓", "", crate::tr("Remove links between the selected tasks (Ctrl+Shift+F2)"), sel, Command::UnlinkSelected);
                    });
                });
                stack(ui, |ui| {
                    sm(
                        ui,
                        "✔",
                        crate::tr("On Track"),
                        crate::tr("Set progress to where the schedule says it should be today"),
                        sel,
                        Command::MarkOnTrack,
                    );
                    sm(ui, "⏸", crate::tr("Inactivate"), crate::tr("Inactivate / activate"), sel, Command::ToggleActive);
                });
            });
            group(ui, crate::tr("Tasks"), |ui| {
                bg(
                    ui,
                    "📌",
                    crate::tr("Manually Schedule…"),
                    crate::tr("Manually schedule the selected tasks"),
                    sel,
                    Command::SetMode(ScheduleMode::Manual),
                );
                bg(
                    ui,
                    "⟳",
                    crate::tr("Auto Schedule"),
                    crate::tr("Auto schedule the selected tasks"),
                    sel,
                    Command::SetMode(ScheduleMode::Auto),
                );
                stack(ui, |ui| {
                    sm(ui, "⬆", crate::tr("Up"), crate::tr("Move up (Alt+Shift+Up)"), sel, Command::MoveUp);
                    sm(ui, "⬇", crate::tr("Down"), crate::tr("Move down (Alt+Shift+Down)"), sel, Command::MoveDown);
                    sm(ui, "🗑", crate::tr("Delete"), crate::tr("Delete the selected tasks (Delete)"), sel, Command::DeleteSelection);
                });
            });
            group(ui, crate::tr("Insert"), |ui| {
                bg(ui, "➕", crate::tr("Task"), crate::tr("Insert a task above the selection (Insert)"), doc, Command::InsertTask);
                stack(ui, |ui| {
                    sm(
                        ui,
                        "☰",
                        crate::tr("Summary"),
                        crate::tr("Group the selected tasks under a new summary task"),
                        doc,
                        Command::InsertSummary,
                    );
                    sm(ui, "♦", crate::tr("Milestone"), crate::tr("Insert a milestone"), doc, Command::InsertMilestone);
                    sm(
                        ui,
                        "↻",
                        crate::tr("Recurring Task…"),
                        crate::tr("Insert a recurring task (e.g. a weekly meeting)"),
                        doc,
                        Command::RecurringTask,
                    );
                });
            });
            group(ui, crate::tr("Properties"), |ui| {
                bg(ui, "ℹ", crate::tr("Information"), crate::tr("Task information (Shift+F2)"), sel, Command::TaskInfo);
                stack(ui, |ui| {
                    sm(ui, "📝", crate::tr("Notes"), crate::tr("Task notes"), sel, Command::TaskNotes);
                    sm(ui, "🌐", crate::tr("Hyperlink"), crate::tr("Add a hyperlink to a document or web page"), sel, Command::Hyperlink);
                    sm(ui, "📅", crate::tr("To Timeline"), crate::tr("Show on the timeline"), sel, Command::ToggleTimeline);
                });
            });
            group(ui, crate::tr("Editing"), |ui| {
                stack(ui, |ui| {
                    sm(ui, "÷", crate::tr("Split Task"), crate::tr("Click a Gantt bar to split it"), sel, Command::SplitTask);
                    sm(ui, "↻", crate::tr("Rejoin Split Task"), "", sel, Command::RejoinTasks);
                });
                bg(ui, "🎯", crate::tr("Scroll to Task"), crate::tr("Scroll the chart to the task"), sel, Command::ScrollToTask);
            });
        }
        MenuTab::Resource => {
            group(ui, crate::tr("View"), |ui| {
                bg(ui, "👥", crate::tr("Team Planner"), "", doc, Command::SetView(ViewKind::TeamPlanner));
            });
            group(ui, crate::tr("Assignments"), |ui| {
                bg(
                    ui,
                    "👥",
                    crate::tr("Assign Resources"),
                    crate::tr("Assign resources to the selected tasks"),
                    sel,
                    Command::AssignResources,
                );
            });
            group(ui, crate::tr("Insert"), |ui| {
                bg(ui, "➕", crate::tr("Add Resource"), crate::tr("Add a resource"), doc, Command::InsertResource);
            });
            group(ui, crate::tr("Properties"), |ui| {
                bg(
                    ui,
                    "ℹ",
                    crate::tr("Information"),
                    crate::tr("Resource information, time off and availability"),
                    res,
                    Command::ResourceInfo,
                );
                stack(ui, |ui| {
                    sm(ui, "🗑", crate::tr("Delete"), crate::tr("Delete the selected resources"), res, Command::DeleteResources);
                });
            });
            group(ui, crate::tr("Leveling"), |ui| {
                bg(ui, "⚖", crate::tr("Level All"), crate::tr("Delay tasks to resolve resource overallocations"), doc, Command::LevelAll);
                stack(ui, |ui| {
                    sm(ui, "↺", crate::tr("Clear Leveling"), crate::tr("Remove all leveling delays"), doc, Command::ClearLeveling);
                });
            });
            group(ui, crate::tr("Tracking"), |ui| {
                bg(
                    ui,
                    "⏱",
                    crate::tr("Timesheet"),
                    crate::tr("Log hours per resource and task"),
                    doc,
                    Command::SetView(ViewKind::Timesheet),
                );
                stack(ui, |ui| {
                    sm(
                        ui,
                        "⟳",
                        crate::tr("Update Progress"),
                        crate::tr("Set % complete from tracked time"),
                        doc,
                        Command::UpdateProgressFromTime,
                    );
                });
            });
        }
        MenuTab::Report => {
            group(ui, crate::tr("View Reports"), |ui| {
                bg(ui, "🗠", crate::tr("Reports…"), crate::tr("Reports and analysis"), doc, Command::Reports);
                bg(
                    ui,
                    "🛠",
                    crate::tr("New Report"),
                    crate::tr("Custom report: choose the table, filter, sort order and assignment details"),
                    doc,
                    Command::CustomReport,
                );
                bg(ui, "📊", crate::tr("Statistics"), crate::tr("Project statistics, variance and earned value"), doc, Command::Statistics);
                stack(ui, |ui| {
                    sm(ui, "♦", crate::tr("Milestones"), crate::tr("Milestone status report"), doc, Command::SetView(ViewKind::Milestones));
                    sm(
                        ui,
                        "📶",
                        crate::tr("Workload"),
                        crate::tr("Resource utilization and resource graph"),
                        doc,
                        Command::SetView(ViewKind::Workload),
                    );
                    sm(
                        ui,
                        "📈",
                        crate::tr("Tracking Gantt"),
                        crate::tr("Current schedule against the baseline"),
                        doc,
                        Command::SetView(ViewKind::TrackingGantt),
                    );
                });
            });
            group(ui, crate::tr("Export"), |ui| {
                stack(ui, |ui| {
                    sm(ui, "printer", crate::tr("Print / PDF…"), crate::tr("Print preview and page setup"), doc, Command::Print);
                    sm(ui, "PDF", crate::tr("Export PDF…"), "", doc, Command::ExportPdf);
                });
                bg(ui, "printer", crate::tr("Print Gantt…"), crate::tr("Preview and print the Gantt chart"), doc, Command::PrintGantt);
                bg(ui, "🗐", crate::tr("Export CSV"), crate::tr("Export the task list as CSV"), doc, Command::ExportCsv);
                bg(ui, "📤", crate::tr("Export to MS Project XML…"), "", doc, Command::ExportMspdi);
            });
        }
        MenuTab::Project => {
            group(ui, crate::tr("Fields"), |ui| {
                bg(ui, "#", crate::tr("Custom Fields…"), crate::tr("Define and edit custom task fields"), doc, Command::CustomFields);
            });
            group(ui, crate::tr("Properties"), |ui| {
                bg(
                    ui,
                    "🗄",
                    crate::tr("Project Information"),
                    crate::tr("Name, dates, calendar and scheduling options"),
                    doc,
                    Command::ProjectInfo,
                );
                bg(
                    ui,
                    "📆",
                    crate::tr("Change Working Time…"),
                    crate::tr("Calendars, work weeks and holidays"),
                    doc,
                    Command::ChangeWorkingTime,
                );
                stack(ui, |ui| {
                    sm(
                        ui,
                        "🔗",
                        crate::tr("Links Between Projects"),
                        crate::tr("Depend on tasks in other project files"),
                        doc,
                        Command::ExternalLinks,
                    );
                    sm(ui, "#", crate::tr("WBS"), crate::tr("Define the WBS code mask"), doc, Command::DefineWbs);
                    sm(ui, "⟳", crate::tr("Refresh links"), crate::tr("Refresh links to other projects"), doc, Command::RefreshExternal);
                });
            });
            group(ui, crate::tr("Project Schedule"), |ui| {
                bg(ui, "📏", crate::tr("Set Baseline"), crate::tr("Save the current schedule as the baseline"), doc, Command::SetBaseline);
                stack(ui, |ui| {
                    sm(ui, "✖", crate::tr("Clear Baseline"), crate::tr("Clear the baseline"), doc, Command::ClearBaseline);
                });
            });
            group(ui, crate::tr("Status"), |ui| {
                bg(
                    ui,
                    "🔄",
                    crate::tr("Update Project"),
                    crate::tr("Mark work complete through a date, or move unfinished work after a date"),
                    doc,
                    Command::UpdateProject,
                );
                stack(ui, |ui| {
                    sm(
                        ui,
                        "✔",
                        crate::tr("On Track"),
                        crate::tr("Set progress to where the schedule says it should be today"),
                        sel,
                        Command::MarkOnTrack,
                    );
                });
            });
            group(ui, crate::tr("AI Agents"), |ui| {
                if big(ui, "⚡", crate::tr("AI Agents"), crate::tr("Built-in MCP server for AI agents"), true, info.mcp_running).clicked()
                {
                    push(Command::McpPanel);
                }
            });
        }
        MenuTab::View => {
            group(ui, crate::tr("Task Views"), |ui| {
                let vb = |ui: &mut Ui, v: ViewKind, bigb: bool| {
                    let r = if bigb {
                        big(ui, v.icon(), v.label(), "", doc, st.view == v)
                    } else {
                        small(ui, v.icon(), v.label(), "", doc, st.view == v)
                    };
                    if r.clicked() {
                        push(Command::SetView(v));
                    }
                };
                vb(ui, ViewKind::Gantt, true);
                vb(ui, ViewKind::TaskUsage, true);
                stack(ui, |ui| {
                    vb(ui, ViewKind::TrackingGantt, false);
                    vb(ui, ViewKind::Calendar, false);
                    vb(ui, ViewKind::Milestones, false);
                });
            });
            group(ui, crate::tr("Resource Views"), |ui| {
                let vb = |ui: &mut Ui, v: ViewKind, bigb: bool| {
                    let r = if bigb {
                        big(ui, v.icon(), v.label(), "", doc, st.view == v)
                    } else {
                        small(ui, v.icon(), v.label(), "", doc, st.view == v)
                    };
                    if r.clicked() {
                        push(Command::SetView(v));
                    }
                };
                vb(ui, ViewKind::TeamPlanner, true);
                vb(ui, ViewKind::ResourceUsage, true);
                stack(ui, |ui| {
                    vb(ui, ViewKind::ResourceSheet, false);
                    vb(ui, ViewKind::Workload, false);
                    vb(ui, ViewKind::Timesheet, false);
                });
            });
            group(ui, crate::tr("Fields"), |ui| {
                bg(ui, "#", crate::tr("Custom Fields…"), "", doc, Command::CustomFields);
            });
            group(ui, crate::tr("Data"), |ui| {
                stack(ui, |ui| {
                    labeled_combo(ui, "", crate::tr("Sort"), |ui| {
                        if st.view == ViewKind::ResourceSheet {
                            egui::ComboBox::from_id_salt("tb_sort_res").width(130.0).selected_text(crate::tr("Sort by")).show_ui(
                                ui,
                                |ui| {
                                    for k in plancraft_core::extras::SortKey::RESOURCES {
                                        if ui.selectable_label(false, k.label()).clicked() {
                                            push(Command::SortResources(k));
                                        }
                                    }
                                },
                            );
                        } else {
                            egui::ComboBox::from_id_salt("tb_sort").width(130.0).selected_text(crate::tr("Sort by")).show_ui(ui, |ui| {
                                ui.selectable_value(&mut st.sort, None, crate::tr("ID"));
                                for (key, title) in &info.fields {
                                    ui.selectable_value(&mut st.sort, Some((key.clone(), false)), title);
                                }
                            });
                        }
                    });
                    labeled_combo(ui, "", crate::tr("Outline"), |ui| {
                        let lvl_text = if st.show_level == 0 {
                            crate::tr("All Subtasks").to_string()
                        } else {
                            crate::tr_dyn(&format!("Level {}", st.show_level))
                        };
                        egui::ComboBox::from_id_salt("tb_level").width(130.0).selected_text(lvl_text).show_ui(ui, |ui| {
                            ui.selectable_value(&mut st.show_level, 0, crate::tr("All Subtasks"));
                            for l in 1..=9u16 {
                                ui.selectable_value(&mut st.show_level, l, crate::tr_dyn(&format!("Level {l}")));
                            }
                        });
                    });
                    labeled_combo(ui, "", crate::tr("Table"), |ui| {
                        egui::ComboBox::from_id_salt("tb_table").width(130.0).selected_text(st.table.label()).show_ui(ui, |ui| {
                            for t in TaskTable::ALL {
                                if ui.selectable_label(st.table == t, t.label()).clicked() {
                                    push(Command::Table(t));
                                }
                            }
                        });
                    });
                });
                stack(ui, |ui| {
                    labeled_combo(ui, "", crate::tr("Filter"), |ui| {
                        egui::ComboBox::from_id_salt("tb_filter").width(140.0).selected_text(st.filter.label()).show_ui(ui, |ui| {
                            for f in TaskFilter::ALL {
                                if ui.selectable_label(st.filter == f, f.label()).clicked() {
                                    push(Command::Filter(f));
                                }
                            }
                        });
                    });
                    if st.filter.needs_value() {
                        let hint = match st.filter {
                            TaskFilter::CostGreater => crate::tr("Cost greater than:"),
                            TaskFilter::WorkGreater => crate::tr("Work greater than (hours):"),
                            _ => crate::tr("Starting within (days):"),
                        };
                        labeled_combo(ui, "", hint, |ui| {
                            ui.add_sized([140.0, BUTTON_H], egui::TextEdit::singleline(&mut st.filter_value).hint_text("0"));
                        });
                    }
                });
                stack(ui, |ui| {
                    labeled_combo(ui, "", crate::tr("Group by"), |ui| {
                        if st.view == ViewKind::ResourceSheet {
                            egui::ComboBox::from_id_salt("tb_group_res").width(130.0).selected_text(st.resource_group.label()).show_ui(
                                ui,
                                |ui| {
                                    for g in crate::state::ResourceGroup::ALL {
                                        ui.selectable_value(&mut st.resource_group, g, g.label());
                                    }
                                },
                            );
                        } else {
                            egui::ComboBox::from_id_salt("tb_group").width(130.0).selected_text(crate::tr("Group by")).show_ui(ui, |ui| {
                                for g in crate::state::TaskGroup::ALL {
                                    ui.selectable_value(&mut st.group, g.key().map(str::to_string), g.label());
                                }
                                ui.separator();
                                for (key, title) in &info.fields {
                                    ui.selectable_value(&mut st.group, Some(key.clone()), title);
                                }
                            });
                        }
                    });
                    labeled_combo(ui, "", crate::tr("Search"), |ui| {
                        ui.add_sized([130.0, BUTTON_H], egui::TextEdit::singleline(&mut st.search).hint_text(crate::tr("🔍 Search tasks")));
                    });
                });
            });
            group(ui, crate::tr("Zoom"), |ui| {
                stack(ui, |ui| {
                    sm(ui, "🔍", crate::tr("Zoom In"), crate::tr("Zoom in (Ctrl++)"), doc, Command::ZoomIn);
                    sm(ui, "🔎", crate::tr("Zoom Out"), crate::tr("Zoom out (Ctrl+-)"), doc, Command::ZoomOut);
                });
                bg(ui, "⛶", crate::tr("Entire Project"), crate::tr("Fit the whole project in the chart"), doc, Command::ZoomEntire);
                bg(ui, "🎯", crate::tr("Selected Tasks"), crate::tr("Fit the selected tasks"), sel, Command::ZoomSelected);
            });
            group(ui, crate::tr("Split View"), |ui| {
                stack(ui, |ui| {
                    ui.checkbox(&mut st.timeline_visible, crate::tr("Timeline"));
                    let mut h = st.show_history;
                    if ui.checkbox(&mut h, crate::tr("History")).changed() {
                        push(Command::History);
                    }
                    row(ui, |ui| {
                        sm(ui, "⊟", "", crate::tr("Collapse all summaries"), doc, Command::CollapseAll);
                        sm(ui, "⊞", "", crate::tr("Expand all summaries"), doc, Command::ExpandAll);
                    });
                });
            });
        }
        MenuTab::Format => {
            if st.view == ViewKind::NetworkDiagram {
                group(ui, crate::tr("Network Diagram"), |ui| {
                    stack(ui, |ui| {
                        ui.checkbox(&mut st.network.opts.critical_first, crate::tr("Critical First"));
                        ui.checkbox(&mut st.network.show_summaries, crate::tr("Summary Tasks"));
                        ui.checkbox(&mut st.network.link_labels, crate::tr("Link Labels"));
                    });
                    stack(ui, |ui| {
                        ui.checkbox(&mut st.network.progress_marks, crate::tr("Progress Marks"));
                        ui.checkbox(&mut st.network.opts.ids_only, crate::tr("IDs Only"));
                    });
                });
                return;
            }
            if st.view == ViewKind::MultipleBaselines {
                group(ui, crate::tr("Baselines"), |ui| {
                    egui::ComboBox::from_id_salt("multiple_baselines_picker").selected_text(crate::tr("Baselines to display")).show_ui(
                        ui,
                        |ui| {
                            for n in 0..=plancraft_core::baseline::MAX_BASELINE {
                                let mut shown = st.gantt.multi_baselines & (1u16 << n) != 0;
                                if ui.checkbox(&mut shown, plancraft_core::baseline::PlanSlot::Baseline(n).name()).changed() {
                                    if shown {
                                        st.gantt.multi_baselines |= 1u16 << n;
                                    } else {
                                        st.gantt.multi_baselines &= !(1u16 << n);
                                    }
                                }
                            }
                        },
                    );
                });
            }
            let g = &mut st.gantt;
            group(ui, crate::tr("Baseline"), |ui| {
                egui::ComboBox::from_id_salt("baseline_slot_picker")
                    .selected_text(plancraft_core::baseline::PlanSlot::Baseline(g.baseline_slot).name())
                    .show_ui(ui, |ui| {
                        for n in 0..=plancraft_core::baseline::MAX_BASELINE {
                            ui.selectable_value(&mut g.baseline_slot, n, plancraft_core::baseline::PlanSlot::Baseline(n).name());
                        }
                    });
            });
            group(ui, crate::tr("Bar Styles"), |ui| {
                stack(ui, |ui| {
                    ui.checkbox(&mut g.show_critical, crate::tr("Critical Tasks"));
                    ui.checkbox(&mut g.show_slack, crate::tr("Slack"));
                    ui.checkbox(&mut g.show_baseline, crate::tr("Baseline"));
                });
                stack(ui, |ui| {
                    ui.checkbox(&mut g.show_progress, crate::tr("Progress"));
                    ui.checkbox(&mut g.show_links, crate::tr("Links"));
                    ui.checkbox(&mut g.show_resource_names, crate::tr("Resource Names"));
                });
            });
            group(ui, crate::tr("Show/Hide"), |ui| {
                stack(ui, |ui| {
                    ui.checkbox(&mut g.show_project_summary, crate::tr("Project Summary Task"));
                    let mut on = st.table == TaskTable::Summary;
                    if ui.checkbox(&mut on, crate::tr("Outline Number")).changed() {
                        push(Command::Table(if on { TaskTable::Summary } else { TaskTable::Entry }));
                    }
                });
            });
        }
    }
}

// ---------------------------------------------------------------------------
// The whole command area
// ---------------------------------------------------------------------------

/// Draws the tab row and the ribbon. Commands are appended to `out`.
pub fn command_bar(ui: &mut Ui, st: &mut UiState, info: &MenuInfo, out: &mut Vec<Command>) {
    let avail = ui.available_rect_before_wrap();
    let body_h = if st.ribbon_collapsed { 0.0 } else { RIBBON_H };
    let rect = Rect::from_min_size(avail.min, vec2(avail.width(), TAB_ROW_H + body_h));
    ui.allocate_rect(rect, Sense::hover());
    let cmds = RefCell::new(Vec::new());
    let push = |c: Command| cmds.borrow_mut().push(c);

    let tabs = Rect::from_min_size(rect.min, vec2(rect.width(), TAB_ROW_H));
    tab_strip(ui, tabs, st, &push);

    if !st.ribbon_collapsed {
        let body = Rect::from_min_max(pos2(rect.left(), tabs.bottom()), rect.max);
        ui.painter().rect_filled(body, 0.0, theme::pal().PANEL_BG);
        let mut child =
            ui.new_child(egui::UiBuilder::new().max_rect(body.shrink2(vec2(6.0, 3.0))).layout(Layout::left_to_right(Align::Min)));
        let width_id = egui::Id::new(("ribbon_widths", st.menu_tab.label(), plancraft_core::i18n::lang().native_name()));
        let mut groups = ui.ctx().data(|data| data.get_temp::<Vec<RibbonGroup>>(width_id)).unwrap_or_default();
        ribbon_plan(&mut groups, body.width() - 12.0);
        let before: Vec<_> = groups.iter().map(|g| g.widths).collect();
        ui.ctx().data_mut(|data| data.insert_temp(egui::Id::new("ribbon_groups"), groups));
        egui::ScrollArea::horizontal()
            .id_salt("ribbon_scroll")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(&mut child, |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.with_layout(Layout::left_to_right(Align::Min), |ui| ribbon(ui, st, info, &push)).response.rect.width()
            });
        let groups = ui.ctx().data(|data| data.get_temp::<Vec<RibbonGroup>>(egui::Id::new("ribbon_groups"))).unwrap_or_default();
        let after: Vec<_> = groups.iter().map(|g| g.widths).collect();
        ui.ctx().data_mut(|data| data.insert_temp(width_id, groups));
        if before != after {
            ui.ctx().request_repaint();
        }
    }
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, theme::pal().GRID_LINE_STRONG));
    out.extend(cmds.into_inner());
}

/// A mini pie button for "x% complete".
fn pie(ui: &mut Ui, pct: u8, enabled: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(22.0, BUTTON_H), if enabled { Sense::click() } else { Sense::hover() });
    if enabled && resp.hovered() {
        ui.painter().rect_filled(r, 4.0, theme::pal().ACCENT_PALE);
    }
    let color = if enabled { theme::pal().ACCENT } else { theme::pal().GRID_LINE_STRONG };
    let c = r.center();
    ui.painter().circle_stroke(c, 6.5, Stroke::new(1.0, color));
    if pct >= 100 {
        ui.painter().circle_filled(c, 6.5, color);
    } else if pct > 0 {
        let n = 16;
        let mut pts = vec![c];
        for i in 0..=n {
            let a = -std::f32::consts::FRAC_PI_2 + (pct as f32 / 100.0) * std::f32::consts::TAU * i as f32 / n as f32;
            pts.push(c + vec2(a.cos(), a.sin()) * 6.5);
        }
        ui.painter().add(Shape::convex_polygon(pts, color, Stroke::NONE));
    }
    resp.on_hover_text(crate::tr_dyn(&format!("Mark {pct}% complete")))
}

#[cfg(test)]
mod responsive_tests {
    use super::*;

    #[test]
    fn collapsed_group_allows_nested_combo_selection() {
        use egui_kittest::{Harness, kittest::Queryable};
        let mut harness = Harness::new_ui_state(
            |ui, selected| {
                let response = ui.button("Appearance palette");
                group_popup(&response, |ui| {
                    egui::ComboBox::from_label("Theme").selected_text(*selected).show_ui(ui, |ui| {
                        ui.selectable_value(selected, "Light", "Light");
                        ui.selectable_value(selected, "Dark", "Dark");
                    });
                    ui.label("Palette stays open");
                });
                ui.put(Rect::from_min_size(pos2(500.0, 400.0), vec2(80.0, 24.0)), egui::Button::new("Outside"));
            },
            "Light",
        );
        harness.get_by_label("Appearance palette").click();
        harness.run();
        harness.get_by_label("Theme").click();
        harness.run();
        harness.get_by_label("Dark").click();
        harness.run();
        assert_eq!(*harness.state(), "Dark");
        assert!(harness.query_by_label("Palette stays open").is_some());
        harness.get_by_label("Theme").click();
        harness.run();
        harness.get_by_label("Light").click();
        harness.run();
        assert_eq!(*harness.state(), "Light");
        harness.get_by_label("Outside").click();
        harness.run();
        harness.run_steps(2);
        assert!(harness.query_by_label("Palette stays open").is_none());
    }

    #[test]
    fn resizing_preserves_direct_commands_before_collapsing_groups() {
        let mut groups: Vec<_> =
            (0..4).map(|_| RibbonGroup { widths: [Some(200.0), Some(130.0), Some(70.0)], ..Default::default() }).collect();
        ribbon_plan(&mut groups, 790.0);
        assert_eq!(groups.iter().filter(|g| g.mode == 1).count(), 1);
        assert!(groups.iter().all(|g| g.mode != 2));
        ribbon_plan(&mut groups, 500.0);
        assert_eq!(groups.iter().filter(|g| g.mode == 2).count(), 1);
        ribbon_plan(&mut groups, 900.0);
        assert!(groups.iter().all(|g| g.mode == 0));
    }

    #[test]
    fn all_tabs_settle_when_resized() {
        let ctx = egui::Context::default();
        let mut state = UiState::default();
        let info = MenuInfo {
            has_doc: true,
            title: String::new(),
            can_undo: false,
            can_redo: false,
            undo_label: None,
            task_selected: true,
            resource_selected: true,
            mcp_running: false,
            mcp_url: None,
            fields: Vec::new(),
        };
        for tab in MenuTab::ALL {
            state.menu_tab = tab;
            for width in [1440.0, 1200.0, 900.0, 1440.0] {
                for _ in 0..12 {
                    let mut output = ctx.run_ui(
                        egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 600.0))), ..Default::default() },
                        |ui| {
                            command_bar(ui, &mut state, &info, &mut Vec::new());
                        },
                    );
                    output.textures_delta.clear();
                }
                let groups = ctx.data(|d| d.get_temp::<Vec<RibbonGroup>>(egui::Id::new("ribbon_groups"))).unwrap_or_default();
                let mut planned = groups.clone();
                ribbon_plan(&mut planned, width - 12.0);
                assert_eq!(groups.iter().map(|g| g.mode).collect::<Vec<_>>(), planned.iter().map(|g| g.mode).collect::<Vec<_>>());
            }
        }
    }
}

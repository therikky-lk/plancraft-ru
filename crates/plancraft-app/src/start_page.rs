//! The Start Page: shown when no project is open, or from File ▸ Start Page.
//!
//! A centered, card-based launcher under the regular title band: start actions
//! on the left, recent projects on the right, a summary of the open project
//! when there is one, and work recovered after a crash.

use std::path::Path;

use egui::{Align, Align2, Color32, FontId, Layout, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};

use crate::commands::Command;
use crate::format;
use crate::recovery::RecoveredItem;
use crate::settings::AppSettings;
use crate::theme;

pub struct StartInfo<'a> {
    pub has_doc: bool,
    pub doc_title: String,
    pub doc_path: Option<String>,
    pub stats: Option<plancraft_core::report::ProjectStats>,
    pub settings: &'a AppSettings,
    /// Recovery copies left by an earlier session that did not exit cleanly.
    pub recovered: &'a [RecoveredItem],
}

const MAX_WIDTH: f32 = 1000.0;
const CARD_RADIUS: u8 = 8;

fn card_frame(hovered: bool) -> egui::Frame {
    egui::Frame::new()
        .fill(theme::pal().BG)
        .stroke(Stroke::new(1.0, if hovered { theme::pal().ACCENT_MID } else { theme::pal().GRID_LINE }))
        .corner_radius(CARD_RADIUS)
        .inner_margin(egui::Margin::same(14))
}

fn section_title(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(15.0).strong().color(theme::pal().TEXT));
    ui.add_space(6.0);
}

/// An action card: tinted icon badge, title and description. Returns true when clicked.
fn action(ui: &mut Ui, id: &str, icon: &str, title: &str, subtitle: &str) -> bool {
    let width = ui.available_width();
    let hovered = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new(("start_action_hover", id)))).unwrap_or(false);
    let resp = card_frame(hovered)
        .show(ui, |ui| {
            ui.set_width(width - 30.0);
            ui.horizontal(|ui| {
                let (badge, _) = ui.allocate_exact_size(vec2(38.0, 38.0), Sense::hover());
                ui.painter().rect_filled(badge, 9.0, if hovered { theme::pal().ACCENT_LIGHT } else { theme::pal().ACCENT_PALE });
                ui.painter().text(badge.center(), Align2::CENTER_CENTER, icon, FontId::proportional(18.0), theme::pal().ACCENT);
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.add_space(1.0);
                    ui.label(RichText::new(title).size(14.0).color(theme::pal().TEXT));
                    ui.label(RichText::new(subtitle).size(11.5).color(theme::pal().TEXT_WEAK));
                });
            });
        })
        .response
        .interact(Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(("start_action_hover", id)), resp.hovered()));
    ui.add_space(8.0);
    resp.clicked()
}

/// Shows paths under the home directory as `~/…`.
fn home_relative(path: &Path) -> String {
    let full = path.display().to_string();
    match std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        Some(home) => {
            let home = home.to_string_lossy().to_string();
            match full.strip_prefix(&home) {
                Some(rest) if !home.is_empty() => format!("~{rest}"),
                _ => full,
            }
        }
        None => full,
    }
}

fn modified_label(path: &Path) -> Option<String> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(time_label(modified.into(), false))
}

/// "Today 14:05", "Yesterday" (with the time when `with_time`), or a date.
fn time_label(dt: chrono::DateTime<chrono::Local>, with_time: bool) -> String {
    let today = chrono::Local::now().date_naive();
    let d = dt.date_naive();
    let time = if with_time { dt.format(" %H:%M").to_string() } else { String::new() };
    if d == today {
        crate::tr_dyn(&format!("Today {}", dt.format("%H:%M")))
    } else if (today - d).num_days() == 1 {
        crate::tr_dyn(&format!("Yesterday{time}"))
    } else {
        format!("{}{time}", crate::format::df(&dt, "%b %-d, %Y"))
    }
}

/// Work recovered after PlanCraft closed without saving: restore or discard each copy.
fn recovered_work(ui: &mut Ui, items: &[RecoveredItem], out: &mut Vec<Command>) {
    let width = ui.available_width();
    let resp = egui::Frame::new()
        .fill(theme::pal().BG)
        .stroke(Stroke::new(1.0, theme::pal().GRID_LINE))
        .corner_radius(CARD_RADIUS)
        .inner_margin(egui::Margin { left: 18, right: 14, top: 12, bottom: 8 })
        .show(ui, |ui| {
            ui.set_width(width - 34.0);
            ui.label(
                RichText::new(crate::tr(
                    "PlanCraft closed before these changes were saved. Restore a copy to keep working on it, or discard it.",
                ))
                .size(11.5)
                .color(theme::pal().TEXT_WEAK),
            );
            ui.add_space(4.0);
            for (i, item) in items.iter().enumerate() {
                let top = ui.cursor().top();
                if i > 0 {
                    ui.painter().hline(ui.max_rect().x_range(), top, Stroke::new(1.0, theme::pal().GRID_LINE));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let (badge, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
                    ui.painter().rect_filled(badge, 8.0, theme::pal().ACCENT_PALE);
                    ui.painter().text(badge.center(), Align2::CENTER_CENTER, "⟳", FontId::proportional(17.0), theme::pal().ACCENT);
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&item.name).size(13.5).color(theme::pal().TEXT));
                        let origin =
                            item.original_path.as_deref().map(home_relative).unwrap_or_else(|| crate::tr("Never saved to a file").into());
                        let when =
                            item.saved_at.map(|t| crate::tr_dyn(&format!("  ·  Copied {}", time_label(t, true)))).unwrap_or_default();
                        ui.label(RichText::new(format!("{origin}{when}")).size(11.0).color(theme::pal().TEXT_WEAK))
                            .on_hover_text(item.original_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default());
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(crate::tr("Discard")).on_hover_text(crate::tr("Delete this recovery copy")).clicked() {
                            out.push(Command::DiscardRecovered(item.stem.clone()));
                        }
                        let restore =
                            egui::Button::new(RichText::new(crate::tr("Restore")).color(Color32::WHITE)).fill(theme::pal().ACCENT);
                        let hint = match &item.original_path {
                            Some(_) => crate::tr("Open this copy as an unsaved document; Save writes it back to the original file"),
                            None => crate::tr("Open this copy as an unsaved document"),
                        };
                        if ui.add(restore).on_hover_text(hint).clicked() {
                            out.push(Command::RestoreRecovered(item.stem.clone()));
                        }
                    });
                });
                ui.add_space(6.0);
            }
        })
        .response;
    // A warm edge marks work that still needs a decision.
    let r = resp.rect;
    ui.painter().rect_filled(
        Rect::from_min_max(r.min, pos2(r.left() + 4.0, r.bottom())),
        egui::CornerRadius { nw: CARD_RADIUS, sw: CARD_RADIUS, ne: 0, se: 0 },
        theme::pal().WARN,
    );
}

fn recent_list(ui: &mut Ui, settings: &AppSettings, out: &mut Vec<Command>) {
    let width = ui.available_width();
    card_frame(false).inner_margin(egui::Margin::symmetric(6, 6)).show(ui, |ui| {
        ui.set_width(width - 14.0);
        if settings.recent.is_empty() {
            ui.add_space(18.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(crate::tr("No recent projects yet")).color(theme::pal().TEXT_WEAK));
                ui.label(RichText::new(crate::tr("Projects you open or save appear here.")).size(11.5).color(theme::pal().TEXT_WEAK));
            });
            ui.add_space(18.0);
            return;
        }
        for (i, path) in settings.recent.iter().enumerate() {
            let exists = path.exists();
            let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let folder = path.parent().map(home_relative).unwrap_or_default();
            let (rect, resp) =
                ui.allocate_exact_size(vec2(ui.available_width(), 46.0), if exists { Sense::click() } else { Sense::hover() });
            let p = ui.painter();
            if exists && resp.hovered() {
                p.rect_filled(rect, 6.0, theme::pal().ACCENT_PALE);
            }
            if i > 0 {
                p.hline((rect.left() + 10.0)..=(rect.right() - 10.0), rect.top(), Stroke::new(1.0, theme::pal().GRID_LINE));
            }
            let icon_color = if exists { theme::pal().ACCENT_MID } else { theme::pal().GRID_LINE_STRONG };
            p.text(pos2(rect.left() + 20.0, rect.center().y), Align2::CENTER_CENTER, "📄", FontId::proportional(16.0), icon_color);
            let text_color = if exists { theme::pal().TEXT } else { theme::pal().TEXT_WEAK };
            p.text(pos2(rect.left() + 40.0, rect.top() + 15.0), Align2::LEFT_CENTER, &name, FontId::proportional(13.5), text_color);
            let date = if exists { modified_label(path).unwrap_or_default() } else { crate::tr("File not found").into() };
            let date_w = p.layout_no_wrap(date.clone(), FontId::proportional(11.5), theme::pal().TEXT_WEAK).size().x;
            p.text(
                pos2(rect.right() - 12.0, rect.center().y),
                Align2::RIGHT_CENTER,
                &date,
                FontId::proportional(11.5),
                theme::pal().TEXT_WEAK,
            );
            let folder_clip = Rect::from_min_max(pos2(rect.left() + 40.0, rect.top()), pos2(rect.right() - date_w - 24.0, rect.bottom()));
            p.with_clip_rect(folder_clip).text(
                pos2(rect.left() + 40.0, rect.top() + 32.0),
                Align2::LEFT_CENTER,
                &folder,
                FontId::proportional(11.0),
                theme::pal().TEXT_WEAK,
            );
            if resp.on_hover_text(path.display().to_string()).clicked() {
                out.push(Command::OpenPath(path.clone()));
            }
        }
    });
}

fn stat(ui: &mut Ui, label: &str, value: &str) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).size(11.0).color(theme::pal().TEXT_WEAK));
        ui.label(RichText::new(value).size(13.5).color(theme::pal().TEXT));
    });
    ui.add_space(14.0);
}

fn current_project(ui: &mut Ui, info: &StartInfo, out: &mut Vec<Command>) {
    let width = ui.available_width();
    let resp = egui::Frame::new()
        .fill(theme::pal().BG)
        .stroke(Stroke::new(1.0, theme::pal().GRID_LINE))
        .corner_radius(CARD_RADIUS)
        .inner_margin(egui::Margin { left: 18, right: 14, top: 14, bottom: 14 })
        .show(ui, |ui| {
            ui.set_width(width - 34.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(&info.doc_title).size(16.0).strong());
                    ui.label(
                        RichText::new(info.doc_path.clone().unwrap_or_else(|| crate::tr("Not saved yet").into()))
                            .size(11.5)
                            .color(theme::pal().TEXT_WEAK),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(crate::tr("Close Project")).clicked() {
                        out.push(Command::Close);
                    }
                    if ui.button(crate::tr("Project Information…")).clicked() {
                        out.push(Command::ProjectInfo);
                    }
                });
            });
            if let Some(s) = &info.stats {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    stat(ui, crate::tr("Start"), &format::date(s.start));
                    stat(ui, crate::tr("Finish"), &format::date(s.finish));
                    stat(ui, crate::tr("Tasks"), &s.tasks.to_string());
                    stat(ui, crate::tr("Milestones"), &s.milestones.to_string());
                    stat(ui, crate::tr("Resources"), &s.resources.to_string());
                    ui.vertical(|ui| {
                        ui.label(RichText::new(crate::tr("Complete")).size(11.0).color(theme::pal().TEXT_WEAK));
                        ui.horizontal(|ui| {
                            let (bar, _) = ui.allocate_exact_size(vec2(120.0, 8.0), Sense::hover());
                            ui.painter().rect_filled(bar, 4.0, theme::pal().TRACK);
                            let w = bar.width() * (s.percent_complete / 100.0).clamp(0.0, 1.0);
                            ui.painter().rect_filled(Rect::from_min_size(bar.min, vec2(w, bar.height())), 4.0, theme::pal().ACCENT_MID);
                            ui.label(RichText::new(format::percent(s.percent_complete)).size(13.5));
                        });
                    });
                });
            }
        })
        .response;
    // An accent edge marks the open project.
    let r = resp.rect;
    ui.painter().rect_filled(
        Rect::from_min_max(r.min, pos2(r.left() + 4.0, r.bottom())),
        egui::CornerRadius { nw: CARD_RADIUS, sw: CARD_RADIUS, ne: 0, se: 0 },
        theme::pal().ACCENT,
    );
}

/// Draws the Start Page. Returns true when the user asks to go back to the open project.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Info,
    New,
    Open,
    Export,
    Options,
}

fn section_id() -> egui::Id {
    egui::Id::new("backstage_section")
}

/// A small, original preview of a plan (staggered bars) for template cards.
fn plan_preview(ui: &Ui, rect: Rect, seed: u32, accent: Color32) {
    let p = ui.painter();
    p.rect_filled(rect, 2.0, theme::pal().BG);
    p.rect_stroke(rect, 2.0, Stroke::new(1.0, theme::pal().GRID_LINE), egui::StrokeKind::Inside);
    // Table lines on the left, bars on the right.
    let split = rect.left() + rect.width() * 0.38;
    for i in 0..8u32 {
        let y = rect.top() + 12.0 + i as f32 * 11.0;
        if y > rect.bottom() - 6.0 {
            break;
        }
        let w = (rect.width() * 0.3) * (0.45 + ((i * 7 + seed * 3) % 5) as f32 * 0.12);
        p.hline(
            (rect.left() + 8.0)..=(rect.left() + 8.0 + w.min(split - rect.left() - 12.0)),
            y,
            Stroke::new(2.0, theme::pal().GRID_LINE_STRONG),
        );
        let x0 = split + 6.0 + ((i * (3 + seed)) % 7) as f32 * (rect.width() * 0.06);
        let bw = rect.width() * (0.08 + ((i + seed) % 4) as f32 * 0.05);
        let bar = Rect::from_min_size(pos2(x0, y - 3.0), vec2(bw.min(rect.right() - x0 - 6.0).max(4.0), 6.0));
        let color = if i % 3 == 0 { theme::pal().BAR_SUMMARY } else { accent };
        p.rect_filled(bar, 1.0, color);
    }
    p.vline(split, (rect.top() + 4.0)..=(rect.bottom() - 4.0), Stroke::new(1.0, theme::pal().GRID_LINE));
}

/// A gallery card; returns true when clicked.
fn gallery_card(ui: &mut Ui, id: &str, title: &str, preview: impl FnOnce(&Ui, Rect)) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(190.0, 168.0), Sense::click());
    let hovered = resp.hovered();
    let thumb = Rect::from_min_size(rect.min + vec2(6.0, 6.0), vec2(rect.width() - 12.0, 124.0));
    if hovered {
        ui.painter().rect_filled(rect, 2.0, theme::pal().TAB_HOVER);
    }
    preview(ui, thumb);
    if hovered {
        ui.painter().rect_stroke(thumb, 2.0, Stroke::new(1.5, theme::pal().ACCENT_MID), egui::StrokeKind::Outside);
    }
    let galley = ui.painter().layout(title.to_string(), FontId::proportional(12.5), theme::pal().TEXT, rect.width() - 12.0);
    ui.painter().galley(pos2(rect.left() + 6.0, thumb.bottom() + 8.0), galley, theme::pal().TEXT);
    let _ = id;
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

fn new_section(ui: &mut Ui, out: &mut Vec<Command>) {
    ui.label(RichText::new(crate::tr("New")).size(30.0).color(theme::pal().TEXT));
    ui.add_space(14.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(14.0, 14.0);
        if gallery_card(ui, "blank", crate::tr("Blank project"), |ui, r| {
            ui.painter().rect_filled(r, 2.0, theme::pal().BG);
            ui.painter().rect_stroke(r, 2.0, Stroke::new(1.0, theme::pal().GRID_LINE), egui::StrokeKind::Inside);
        }) {
            out.push(Command::NewProject);
        }
        if gallery_card(ui, "sample", crate::tr("Sample project"), |ui, r| plan_preview(ui, r, 1, theme::pal().BAR_TASK)) {
            out.push(Command::NewDemo);
        }
        if gallery_card(ui, "import", crate::tr("Import (CSV, Excel, MS Project XML)…"), |ui, r| {
            plan_preview(ui, r, 5, theme::pal().ACCENT_MID);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, "📥", FontId::proportional(34.0), theme::pal().ACCENT);
        }) {
            out.push(Command::Import);
        }
        for (k, t) in plancraft_core::templates::Template::ALL.into_iter().enumerate() {
            if gallery_card(ui, t.label(), t.label(), |ui, r| plan_preview(ui, r, 2 + k as u32, theme::pal().BAR_TASK)) {
                out.push(Command::NewFromTemplate(t));
            }
        }
        if gallery_card(ui, "mine", crate::tr("My templates"), |ui, r| {
            plan_preview(ui, r, 9, theme::pal().BAR_MANUAL);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, "🗋", FontId::proportional(34.0), theme::pal().ACCENT);
        }) {
            out.push(Command::Templates);
        }
    });
}

fn nav_item(ui: &mut Ui, label: &str, selected: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 0.0, theme::lerp(theme::pal().TITLE_BAR, Color32::BLACK, 0.25));
    } else if resp.hovered() {
        p.rect_filled(rect, 0.0, Color32::from_white_alpha(30));
    }
    p.text(pos2(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, label, FontId::proportional(15.0), Color32::WHITE);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// The File page (backstage): a green command column and the chosen section.
pub fn show(ui: &mut Ui, info: &StartInfo, font_filter: &mut String, out: &mut Vec<Command>) -> bool {
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, theme::pal().BG);
    let ctx = ui.ctx().clone();
    let default = if info.has_doc { 0u8 } else { 1u8 };
    let mut sec_n = ctx.data(|d| d.get_temp::<u8>(section_id())).unwrap_or(default);
    if !info.has_doc && sec_n == 0 {
        sec_n = 1;
    }
    let section = match sec_n {
        0 => Section::Info,
        2 => Section::Open,
        3 => Section::Export,
        4 => Section::Options,
        _ => Section::New,
    };
    let mut back = false;

    // Left column.
    let nav = Rect::from_min_size(full.min, vec2(220.0, full.height()));
    ui.painter().rect_filled(nav, 0.0, theme::pal().TITLE_BAR);
    let mut nav_ui = ui.new_child(egui::UiBuilder::new().max_rect(nav));
    nav_ui.spacing_mut().item_spacing.y = 0.0;
    nav_ui.add_space(14.0);
    if info.has_doc {
        let (r, resp) = nav_ui.allocate_exact_size(vec2(nav.width(), 44.0), Sense::click());
        if resp.hovered() {
            nav_ui.painter().circle_filled(pos2(r.left() + 34.0, r.center().y), 16.0, Color32::from_white_alpha(30));
        }
        nav_ui.painter().circle_stroke(pos2(r.left() + 34.0, r.center().y), 15.0, Stroke::new(1.5, Color32::WHITE));
        nav_ui.painter().text(pos2(r.left() + 34.0, r.center().y), Align2::CENTER_CENTER, "⬅", FontId::proportional(15.0), Color32::WHITE);
        if resp.on_hover_text(crate::tr_dyn(&format!("Back to {}  ➡", info.doc_title))).clicked() {
            back = true;
        }
        nav_ui.add_space(10.0);
    }
    let set = |n: u8| ctx.data_mut(|d| d.insert_temp(section_id(), n));
    if info.has_doc && nav_item(&mut nav_ui, crate::tr("Info"), section == Section::Info) {
        set(0);
    }
    if nav_item(&mut nav_ui, crate::tr("New"), section == Section::New) {
        set(1);
    }
    if nav_item(&mut nav_ui, crate::tr("Open"), section == Section::Open) {
        set(2);
    }
    if info.has_doc {
        if nav_item(&mut nav_ui, crate::tr("Save"), false) {
            out.push(Command::Save);
        }
        if nav_item(&mut nav_ui, crate::tr("Save As…"), false) {
            out.push(Command::SaveAs);
        }
        if nav_item(&mut nav_ui, crate::tr("Save as Template…"), false) {
            out.push(Command::SaveTemplate);
        }
        if nav_item(&mut nav_ui, crate::tr("Export"), section == Section::Export) {
            set(3);
        }
        if nav_item(&mut nav_ui, crate::tr("Print Gantt…"), false) {
            out.push(Command::PrintGantt);
            back = true;
        }
        if nav_item(&mut nav_ui, crate::tr("Close"), false) {
            out.push(Command::Close);
        }
    }
    nav_ui.add_space(18.0);
    nav_ui.painter().hline(
        (nav.left() + 20.0)..=(nav.right() - 20.0),
        nav_ui.cursor().top() - 9.0,
        Stroke::new(1.0, Color32::from_white_alpha(70)),
    );
    if nav_item(&mut nav_ui, crate::tr("Options"), section == Section::Options) {
        set(4);
    }
    if nav_item(&mut nav_ui, crate::tr("AI Agents"), false) {
        out.push(Command::McpPanel);
    }
    if nav_item(&mut nav_ui, crate::tr("About PlanCraft"), false) {
        out.push(Command::About);
    }
    if nav_item(&mut nav_ui, crate::tr("Exit"), false) {
        out.push(Command::Quit);
    }

    // Content.
    let content = Rect::from_min_max(pos2(nav.right() + 36.0, full.top() + 24.0), pos2(full.right() - 24.0, full.bottom()));
    let mut c_ui = ui.new_child(egui::UiBuilder::new().max_rect(content));
    egui::ScrollArea::vertical().id_salt("start_page").auto_shrink([false, false]).show(&mut c_ui, |ui| {
        let width = ui.available_width().min(MAX_WIDTH + 140.0);
        ui.set_width(width);
        if !info.recovered.is_empty() && matches!(section, Section::New | Section::Open | Section::Info) {
            section_title(ui, crate::tr("Recovered work"));
            recovered_work(ui, info.recovered, out);
            ui.add_space(22.0);
        }
        match section {
            Section::Info => {
                ui.label(RichText::new(crate::tr("Info")).size(30.0).color(theme::pal().TEXT));
                ui.add_space(14.0);
                current_project(ui, info, out);
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if ui.button(crate::tr("Statistics")).clicked() {
                        out.push(Command::Statistics);
                        back = true;
                    }
                    if ui.button(crate::tr("Reports…")).clicked() {
                        out.push(Command::Reports);
                        back = true;
                    }
                });
            }
            Section::New => new_section(ui, out),
            Section::Open => {
                ui.label(RichText::new(crate::tr("Open")).size(30.0).color(theme::pal().TEXT));
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if action(ui, "open", "📂", crate::tr("Open…"), crate::tr("Browse for a .plancraft file")) {
                        out.push(Command::Open);
                    }
                });
                ui.horizontal(|ui| {
                    if action(
                        ui,
                        "import",
                        "📥",
                        crate::tr("Import (CSV, Excel, MS Project XML)…"),
                        crate::tr("Import tasks from a CSV / Excel table or MS Project XML"),
                    ) {
                        out.push(Command::Import);
                    }
                });
                ui.add_space(10.0);
                section_title(ui, crate::tr("Recent projects"));
                recent_list(ui, info.settings, out);
            }
            Section::Export => {
                ui.label(RichText::new(crate::tr("Export")).size(30.0).color(theme::pal().TEXT));
                ui.add_space(14.0);
                if action(ui, "print-pdf", "PDF", crate::tr("Print / PDF…"), crate::tr("Print preview and page setup")) {
                    out.push(Command::Print);
                    back = true;
                }
                if action(ui, "export-pdf", "PDF", crate::tr("Export PDF…"), "") {
                    out.push(Command::ExportPdf);
                }
                if action(ui, "print-gantt", "PRN", crate::tr("Print Gantt…"), crate::tr("Preview and print the Gantt chart")) {
                    out.push(Command::PrintGantt);
                    back = true;
                }
                if action(ui, "xml", "📤", crate::tr("Export to MS Project XML…"), crate::tr("Microsoft Project XML")) {
                    out.push(Command::ExportMspdi);
                }
                if action(
                    ui,
                    "xml-extended",
                    "XML",
                    crate::tr("Export XML with extended fields…"),
                    crate::tr("WBS codes, hyperlinks and cost accrual"),
                ) {
                    out.push(Command::ExportXml);
                }
                if action(ui, "csv", "🗐", crate::tr("Export CSV…"), crate::tr("Export the task list as CSV")) {
                    out.push(Command::ExportCsv);
                }
                if action(ui, "reports", "🗠", crate::tr("Reports…"), crate::tr("Publish the report as a web page")) {
                    out.push(Command::Reports);
                    back = true;
                }
            }
            Section::Options => {
                ui.label(RichText::new(crate::tr("Options")).size(30.0).color(theme::pal().TEXT));
                ui.add_space(14.0);
                egui::Grid::new("bs_opts").num_columns(2).spacing([16.0, 12.0]).show(ui, |ui| {
                    ui.label(crate::tr("Theme"));
                    ui.horizontal(|ui| {
                        for t in crate::theme::ThemeChoice::ALL {
                            if ui.selectable_label(info.settings.theme == t, t.label()).clicked() {
                                out.push(Command::SetTheme(t));
                            }
                        }
                    });
                    ui.end_row();
                    ui.label(crate::tr("Font"));
                    ui.vertical(|ui| {
                        ui.add(egui::TextEdit::singleline(font_filter).hint_text(crate::tr("Search fonts…")).desired_width(250.0));
                        let selected = if info.settings.font_family.is_empty() { crate::tr("Default") } else { &info.settings.font_family };
                        egui::ComboBox::from_id_salt("option_font_family").selected_text(selected).width(250.0).show_ui(ui, |ui| {
                            if ui.selectable_label(info.settings.font_family.is_empty(), crate::tr("Default")).clicked() {
                                out.push(Command::SetFont(String::new()));
                            }
                            let needle = font_filter.to_lowercase();
                            for name in crate::fonts::families().iter().filter(|n| n.to_lowercase().contains(&needle)).take(300) {
                                if ui.selectable_label(info.settings.font_family == *name, name).clicked() {
                                    out.push(Command::SetFont(name.clone()));
                                }
                            }
                        });
                    });
                    ui.end_row();
                    ui.label(crate::tr("Language"));
                    let current = plancraft_core::i18n::lang();
                    egui::ComboBox::from_id_salt("option_language").selected_text(current.native_name()).width(250.0).show_ui(ui, |ui| {
                        for &l in plancraft_core::i18n::Lang::ALL {
                            if ui.selectable_label(current == l, l.native_name()).clicked() {
                                out.push(Command::SetLang(l));
                            }
                        }
                    });
                    ui.end_row();
                });
                ui.add_space(20.0);
                ui.label(RichText::new(format!("PlanCraft v{}", env!("CARGO_PKG_VERSION"))).color(theme::pal().TEXT_WEAK));
            }
        }
        ui.add_space(24.0);
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && info.has_doc {
        back = true;
    }
    back
}

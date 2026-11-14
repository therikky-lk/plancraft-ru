//! Smaller dialogs: Assign Resources, Project Statistics, Links Between Projects, About, History.

use std::path::PathBuf;

use egui::{Context, RichText};
use plancraft_core::io;
use plancraft_core::prelude::*;
use plancraft_core::report;
use plancraft_core::workspace::{ChangeSource, Document};
use uuid::Uuid;

use crate::edit::EditExt;
use crate::format;
use crate::logo;
use crate::state::{Selection, ToastKind, UiState};
use crate::theme;

// ---------------------------------------------------------------------------
// Assign Resources
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct AssignResources {
    units: std::collections::HashMap<ResourceId, String>,
    filter: String,
}

impl AssignResources {
    pub fn show(&mut self, ctx: &Context, doc: &mut Document, sel: &Selection, st_toasts: &mut Vec<(ToastKind, String)>) -> bool {
        let mut open = true;
        let tasks: Vec<TaskId> =
            doc.project.tasks.iter().filter(|t| sel.tasks.contains(&t.id) && !t.sched.is_summary).map(|t| t.id).collect();
        super::window(ctx, crate::tr("Assign Resources")).id(egui::Id::new("assign_resources"))
            .pivot(egui::Align2::CENTER_TOP)
            .default_pos(ctx.content_rect().center_top() + egui::vec2(0.0, 120.0)).open(&mut open).default_width(480.0).show(ctx, |ui| {
            let label = match tasks.as_slice() {
                [] => crate::tr("Select one or more (non-summary) tasks.").to_string(),
                [one] => crate::tr_dyn(&format!("Task: {}", doc.project.task(*one).map(|t| t.name.as_str()).unwrap_or(""))),
                many => crate::tr_dyn(&format!("{} tasks selected", many.len())),
            };
            ui.label(RichText::new(label).strong());
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text(crate::tr("Filter resources…")).desired_width(f32::INFINITY));
            ui.add_space(4.0);
            let mut actions: Vec<(ResourceId, Option<f64>)> = Vec::new();
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                egui::Grid::new("assign_grid").striped(true).num_columns(5).spacing([10.0, 6.0]).show(ui, |ui| {
                    for h in ["", crate::tr("Resource Name"), crate::tr("Category"), crate::tr("Units"), ""] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    let p = &doc.project;
                    for r in &p.resources {
                        if !self.filter.is_empty() && !r.name.to_lowercase().contains(&self.filter.to_lowercase()) {
                            continue;
                        }
                        let assigned_all = !tasks.is_empty() && tasks.iter().all(|t| p.assignments.iter().any(|a| a.task == *t && a.resource == r.id));
                        ui.label(if assigned_all { RichText::new("✔").color(theme::pal().OK) } else { RichText::new("") });
                        ui.label(&r.name);
                        ui.label(RichText::new(r.category.label()).color(theme::pal().TEXT_WEAK));
                        let current = tasks.first().and_then(|t| p.assignments.iter().find(|a| a.task == *t && a.resource == r.id)).map(|a| format::units(a.units));
                        let text = self.units.entry(r.id).or_insert_with(|| current.unwrap_or_else(|| "100%".into()));
                        ui.add(egui::TextEdit::singleline(text).desired_width(56.0));
                        ui.horizontal(|ui| {
                            if ui.add_enabled(!tasks.is_empty(), egui::Button::new(if assigned_all { "Update" } else { "Assign" })).clicked() {
                                // Bad text is an error, never a removal (None means "remove").
                                match format::parse_units(text) {
                                    Some(u) => actions.push((r.id, Some(u))),
                                    None => st_toasts.push((ToastKind::Error, format!("'{}' is not a valid units value (try 50% or 100%)", text.trim()))),
                                }
                            }
                            if assigned_all && ui.button(crate::tr("Remove")).clicked() {
                                actions.push((r.id, None));
                            }
                        });
                        ui.end_row();
                    }
                });
            });
            for (rid, units) in actions {
                let tasks = tasks.clone();
                let r = doc.try_edit(if units.is_some() { crate::tr("Assign Resource") } else { crate::tr("Remove Assignment") }, ChangeSource::User, |p| {
                    for t in &tasks {
                        match units {
                            Some(u) => {
                                p.assign(*t, rid, Some(u), None)?;
                            }
                            None => {
                                if let Some(a) = p.assignments.iter().find(|a| a.task == *t && a.resource == rid).map(|a| a.id) {
                                    p.unassign(a)?;
                                }
                            }
                        }
                    }
                    Ok::<(), PlanError>(())
                });
                if let Err(e) = r {
                    st_toasts.push((ToastKind::Error, e.to_string()));
                }
            }
            ui.add_space(4.0);
            ui.label(RichText::new(crate::tr("Tip: units are a percentage of the resource's time (100% = full time). Type new names in the Resource Names column to create resources on the fly.")).small().color(theme::pal().TEXT_WEAK));
        });
        open
    }
}

// ---------------------------------------------------------------------------
// Project Statistics
// ---------------------------------------------------------------------------

pub fn statistics(ctx: &Context, doc: &Document, open: &mut bool) {
    let p = &doc.project;
    let s = report::project_stats(p);
    super::window(ctx, crate::tr_dyn(&format!("Project Statistics for '{}'", p.name)))
        .id(egui::Id::new("stats"))
        .pivot(egui::Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + egui::vec2(0.0, 120.0))
        .open(open)
        .default_width(560.0)
        .show(ctx, |ui| {
            egui::Grid::new("stats_dates").striped(true).num_columns(3).spacing([24.0, 6.0]).show(ui, |ui| {
                ui.label("");
                ui.label(RichText::new(crate::tr("Start")).strong());
                ui.label(RichText::new(crate::tr("Finish")).strong());
                ui.end_row();
                ui.label(crate::tr("Current"));
                ui.label(format::date(s.start));
                ui.label(format::date(s.finish));
                ui.end_row();
                ui.label(crate::tr("Baseline"));
                ui.label(format::date(p.tasks.iter().filter_map(|t| t.baseline.map(|b| b.start)).min()));
                ui.label(format::date(s.baseline_finish));
                ui.end_row();
                ui.label(crate::tr("Variance"));
                ui.label("");
                ui.label(RichText::new(p.format_duration(s.finish_variance, false)).color(if s.finish_variance > 0 {
                    theme::pal().ERROR
                } else {
                    theme::pal().TEXT
                }));
                ui.end_row();
            });
            ui.separator();
            egui::Grid::new("stats_totals").striped(true).num_columns(4).spacing([24.0, 6.0]).show(ui, |ui| {
                ui.label(RichText::new(crate::tr("Duration")).strong());
                ui.label(p.format_duration(s.duration, false));
                ui.label(RichText::new(crate::tr("Work")).strong());
                ui.label(crate::tr_dyn(&format!(
                    "{} ({} actual, {} remaining)",
                    format_work(s.work),
                    format_work(s.actual_work),
                    format_work(s.remaining_work)
                )));
                ui.end_row();
                ui.label(RichText::new(crate::tr("Cost")).strong());
                ui.label(format::money(p, s.cost));
                ui.label(RichText::new(crate::tr("Baseline cost")).strong());
                ui.label(format::money(p, s.baseline_cost));
                ui.end_row();
                ui.label(RichText::new(crate::tr("Complete")).strong());
                ui.label(format::percent(s.percent_complete));
                ui.label(RichText::new(crate::tr("Tasks")).strong());
                ui.label(crate::tr_dyn(&format!(
                    "{} ({} done, {} in progress, {} late, {} critical)",
                    s.tasks, s.completed_tasks, s.in_progress_tasks, s.late_tasks, s.critical_tasks
                )));
                ui.end_row();
                ui.label(RichText::new(crate::tr("Milestones")).strong());
                ui.label(s.milestones.to_string());
                ui.label(RichText::new(crate::tr("Resources")).strong());
                ui.label(crate::tr_dyn(&format!("{} ({} overallocated)", s.resources, s.overallocated_resources)));
                ui.end_row();
            });
            if let Some(ev) = &s.earned_value {
                ui.separator();
                let against = plancraft_core::baseline::PlanSlot::Baseline(ev.baseline).name();
                ui.label(RichText::new(format!("Earned value (against {against})")).strong());
                egui::Grid::new("stats_ev").num_columns(4).spacing([24.0, 4.0]).show(ui, |ui| {
                    ui.label(crate::tr("Planned value (BCWS)"));
                    ui.label(format::money(p, ev.bcws));
                    ui.label(crate::tr("Schedule perf. (SPI)"));
                    ui.label(RichText::new(format!("{:.2}", ev.spi)).color(if ev.spi < 0.95 {
                        theme::pal().ERROR
                    } else {
                        theme::pal().OK
                    }));
                    ui.end_row();
                    ui.label(crate::tr("Earned value (BCWP)"));
                    ui.label(format::money(p, ev.bcwp));
                    ui.label(crate::tr("Cost perf. (CPI)"));
                    ui.label(RichText::new(format!("{:.2}", ev.cpi)).color(if ev.cpi < 0.95 {
                        theme::pal().ERROR
                    } else {
                        theme::pal().OK
                    }));
                    ui.end_row();
                    ui.label(crate::tr("Actual cost (ACWP)"));
                    ui.label(format::money(p, ev.acwp));
                    ui.label(crate::tr("SV / CV"));
                    ui.label(format!("{} / {}", format::money(p, ev.sv), format::money(p, ev.cv)));
                    ui.end_row();
                });
            } else {
                let against = plancraft_core::baseline::PlanSlot::Baseline(p.settings.earned_value_baseline).name();
                ui.label(
                    RichText::new(format!("Save {against} (Project > Set Baseline) to track earned value; variance uses Baseline."))
                        .small()
                        .color(theme::pal().TEXT_WEAK),
                );
            }
            if !s.warnings.is_empty() {
                ui.separator();
                ui.label(RichText::new(crate::tr("Scheduling warnings")).strong().color(theme::pal().ERROR));
                for w in &s.warnings {
                    ui.label(RichText::new(format!("⚠ {w}")).color(theme::pal().ERROR));
                }
            }
        });
}

// ---------------------------------------------------------------------------
// Links Between Projects
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct ExternalLinks {
    selected: Option<Uuid>,
    tasks: Vec<ExternalTaskSnapshot>,
    loaded_for: Option<Uuid>,
    error: Option<String>,
    filter: String,
}

impl ExternalLinks {
    pub fn show(&mut self, ctx: &Context, doc: &mut Document, st: &mut UiState) -> bool {
        let mut open = true;
        super::window(ctx, crate::tr("Links Between Projects")).id(egui::Id::new("external_links"))
            .pivot(egui::Align2::CENTER_TOP)
            .default_pos(ctx.content_rect().center_top() + egui::vec2(0.0, 120.0)).open(&mut open).default_width(640.0).show(ctx, |ui| {
            ui.label(crate::tr("Tasks and milestones can depend on tasks stored in other PlanCraft files. Linked dates are cached in this file and refreshed when it opens."));
            if doc.path.is_none() {
                ui.colored_label(theme::pal().WARN, crate::tr("Save this project first so links can be stored relative to its location."));
            }
            ui.horizontal(|ui| {
                if ui.button(crate::tr("➕ Add Project File…")).clicked()
                    && let Some(path) = rfd::FileDialog::new().add_filter(crate::tr("PlanCraft project"), &["plancraft"]).pick_file()
                {
                    let doc_path = doc.path.clone();
                    match doc.edit_msg(crate::tr("Add External Project"), |p| io::add_external_project(p, doc_path.as_deref(), &path).map_err(|e| e.to_string())) {
                        Ok(id) => {
                            self.selected = Some(id);
                            self.loaded_for = None;
                        }
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
                if ui.button(crate::tr("⟳ Refresh All")).clicked() {
                    let w = doc.refresh_external();
                    if w.is_empty() {
                        st.toast(ToastKind::Success, crate::tr("External links refreshed"));
                    } else {
                        for m in w {
                            st.toast(ToastKind::Warning, m);
                        }
                    }
                    self.loaded_for = None;
                }
            });
            ui.separator();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(220.0);
                    ui.label(RichText::new(crate::tr("Linked projects")).strong());
                    for e in &doc.project.external_projects {
                        let resp = ui.selectable_label(self.selected == Some(e.id), format!("📁 {}", e.name));
                        if resp.clicked() {
                            self.selected = Some(e.id);
                        }
                        resp.on_hover_text(crate::tr_dyn(&format!("{}\nLast refreshed: {}", e.path, format::date_long(e.last_sync))));
                    }
                    if doc.project.external_projects.is_empty() {
                        ui.label(RichText::new(crate::tr("None yet")).color(theme::pal().TEXT_WEAK));
                    }
                });
                ui.separator();
                ui.vertical(|ui| {
                    let Some(ext_id) = self.selected else {
                        ui.label(crate::tr("Select a linked project to browse its tasks."));
                        return;
                    };
                    if self.loaded_for != Some(ext_id) {
                        match io::external_task_list(&doc.project, doc.path.as_deref(), ext_id) {
                            Ok(t) => {
                                self.tasks = t;
                                self.error = None;
                            }
                            Err(e) => {
                                self.tasks.clear();
                                self.error = Some(e.to_string());
                            }
                        }
                        self.loaded_for = Some(ext_id);
                    }
                    let target = st.sel.single_task();
                    let target_name = target.and_then(|t| doc.project.task(t)).map(|t| t.name.clone());
                    ui.label(match &target_name {
                        Some(n) => crate::tr_dyn(&format!("Link a task below to the selected task: '{n}'")),
                        None => crate::tr("Select a task in the Gantt Chart to link to.").into(),
                    });
                    ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text(crate::tr("Filter…")).desired_width(f32::INFINITY));
                    let mut link: Option<(TaskId, bool)> = None;
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        egui::Grid::new("ext_tasks").striped(true).num_columns(4).spacing([10.0, 4.0]).show(ui, |ui| {
                            for t in &self.tasks {
                                if !self.filter.is_empty() && !t.name.to_lowercase().contains(&self.filter.to_lowercase()) {
                                    continue;
                                }
                                ui.label(if t.milestone { format!("♦ {}", t.name) } else { t.name.clone() });
                                ui.label(RichText::new(format!("{} – {}", format::date(t.start), format::date(t.finish))).color(theme::pal().TEXT_WEAK));
                                if ui.add_enabled(target.is_some(), egui::Button::new(crate::tr("Predecessor"))).on_hover_text(crate::tr("Selected task starts after this external task finishes")).clicked() {
                                    link = Some((t.id, false));
                                }
                                let is_ms = target.and_then(|t| doc.project.task(t)).is_some_and(|t| t.is_milestone());
                                if ui.add_enabled(is_ms, egui::Button::new(crate::tr("Milestone member"))).on_hover_text(crate::tr("Selected milestone requires this external task")).clicked() {
                                    link = Some((t.id, true));
                                }
                                ui.end_row();
                            }
                        });
                    });
                    if let (Some((tid, member)), Some(target)) = (link, target) {
                        let doc_path = doc.path.clone();
                        let r = doc.edit_msg(crate::tr("Link External Task"), |p| {
                            io::ensure_external_task(p, doc_path.as_deref(), ext_id, tid).map_err(|e| e.to_string())?;
                            let r = TaskRef::External { project: ext_id, task: tid };
                            if member { p.add_milestone_members(target, &[r]) } else { p.link(r, target, LinkType::FS, Lag::None) }.map_err(|e| e.to_string())
                        });
                        match r {
                            Ok(()) => st.toast(ToastKind::Success, crate::tr("Linked to external task")),
                            Err(e) => self.error = Some(e),
                        }
                    }
                });
            });
            if let Some(e) = &self.error {
                ui.colored_label(theme::pal().ERROR, e);
            }
        });
        open
    }
}

// ---------------------------------------------------------------------------
// About & History
// ---------------------------------------------------------------------------

pub fn about(ctx: &Context, open: &mut bool) {
    super::window(ctx, crate::tr("About PlanCraft"))
        .id(egui::Id::new("about"))
        .pivot(egui::Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + egui::vec2(0.0, 120.0))
        .open(open)
        .collapsible(false)
        .resizable(false)
        .default_width(380.0)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(72.0, 72.0), egui::Sense::hover());
                logo::paint(ui.painter(), r);
                ui.heading("PlanCraft");
                ui.label(crate::tr_dyn(&format!("Version {}", env!("CARGO_PKG_VERSION"))));
                ui.add_space(6.0);
                ui.label(crate::tr("Project planning and scheduling, written entirely in Rust."));
                ui.label(crate::tr("Tasks · Milestones · Resources · Calendars · Workload · Time tracking"));
                ui.label(crate::tr("Built-in MCP server for AI agents."));
            });
        });
}

pub fn history(ctx: &Context, doc: &mut Document, open: &mut bool) {
    super::window(ctx, crate::tr("Undo History"))
        .id(egui::Id::new("history"))
        .pivot(egui::Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + egui::vec2(0.0, 120.0))
        .open(open)
        .default_width(320.0)
        .show(ctx, |ui| {
            let items = doc.history();
            if items.is_empty() {
                ui.label(RichText::new(crate::tr("No changes yet")).color(theme::pal().TEXT_WEAK));
            }
            let mut undo_to = None;
            egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                for (i, (label, src)) in items.iter().enumerate() {
                    let tag = match src {
                        plancraft_core::workspace::ChangeSource::Agent => RichText::new(crate::tr("⚡ AI")).color(theme::pal().ACCENT_MID),
                        plancraft_core::workspace::ChangeSource::User => RichText::new("👤").color(theme::pal().TEXT_WEAK),
                    };
                    ui.horizontal(|ui| {
                        ui.label(tag);
                        if ui.link(label).on_hover_text(crate::tr("Undo back to before this change")).clicked() {
                            undo_to = Some(i + 1);
                        }
                    });
                }
            });
            if let Some(n) = undo_to {
                for _ in 0..n {
                    doc.undo();
                }
            }
        });
}

/// Paths for recent files, stored in the user's config directory.
pub fn config_dir() -> Option<PathBuf> {
    plancraft_core::paths::config_dir()
}

// ---------------------------------------------------------------------------
// New from Template
// ---------------------------------------------------------------------------

/// Folder with the user's own templates.
pub fn templates_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join("templates"))
}

pub fn templates(ctx: &Context, open: &mut bool, out: &mut Vec<crate::commands::Command>) {
    use crate::commands::Command;
    super::window(ctx, crate::tr("Project Templates"))
        .id(egui::Id::new("templates_window"))
        .pivot(egui::Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + egui::vec2(0.0, 120.0))
        .collapsible(false)
        .default_width(460.0)
        .open(open)
        .show(ctx, |ui| {
            ui.label(RichText::new(crate::tr("Built-in templates")).strong());
            for t in plancraft_core::templates::Template::ALL {
                if ui.add(egui::Button::new(format!("📄  {}", t.label())).min_size(egui::vec2(420.0, 28.0))).clicked() {
                    out.push(Command::NewFromTemplate(t));
                }
            }
            ui.add_space(8.0);
            ui.label(RichText::new(crate::tr("My templates")).strong());
            let mut files: Vec<PathBuf> = templates_dir()
                .and_then(|d| std::fs::read_dir(d).ok())
                .map(|rd| {
                    rd.filter_map(|e| e.ok().map(|e| e.path()))
                        .filter(|p| p.extension().is_some_and(|x| x == plancraft_core::model::FILE_EXTENSION))
                        .collect()
                })
                .unwrap_or_default();
            files.sort();
            if files.is_empty() {
                ui.label(RichText::new(crate::tr("No saved templates yet. Use File > Save as Template.")).color(theme::pal().TEXT_WEAK));
            }
            for f in files.into_iter().take(200) {
                let name = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if ui
                    .add(egui::Button::new(format!("🗋  {name}")).min_size(egui::vec2(420.0, 26.0)))
                    .on_hover_text(f.display().to_string())
                    .clicked()
                {
                    out.push(Command::NewFromTemplateFile(f));
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use egui_kittest::kittest::Queryable;
    use plancraft_core::workspace::Workspace;

    /// Regression: unreadable Units text parsed as `None`, which the dialog treats as "remove",
    /// so clicking Update with a typo deleted the assignment.
    #[test]
    fn bad_units_text_reports_an_error_and_keeps_the_assignment() {
        let mut ws = Workspace::default();
        ws.add(plancraft_core::sample::demo_project(NaiveDate::from_ymd_opt(2026, 10, 19).unwrap()), None);
        let (task, res, name) = {
            let p = &ws.docs[0].project;
            let a = p.assignments.first().unwrap();
            (a.task, a.resource, p.resource(a.resource).unwrap().name.clone())
        };
        let assignments = ws.docs[0].project.assignments.len();
        let mut dialog = AssignResources { filter: name, ..Default::default() };
        dialog.units.insert(res, "abc".into());
        let sel = Selection { tasks: [task].into_iter().collect(), ..Default::default() };
        let state = (ws, dialog, sel, Vec::new());
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1000.0, 800.0)).build_ui_state(
            |ui, (ws, dialog, sel, toasts): &mut (Workspace, AssignResources, Selection, Vec<(ToastKind, String)>)| {
                dialog.show(ui.ctx(), &mut ws.docs[0], sel, toasts);
            },
            state,
        );
        h.run();
        h.get_by_label("Update").click();
        h.run();
        let (ws, _, _, toasts) = h.state();
        assert_eq!(ws.docs[0].project.assignments.len(), assignments, "the assignment is kept");
        assert!(toasts.iter().any(|(k, m)| *k == ToastKind::Error && m.contains("abc")), "{toasts:?}");
    }
}

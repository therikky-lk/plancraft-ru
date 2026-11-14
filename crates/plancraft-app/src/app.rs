//! The PlanCraft desktop application: layout, command execution and document management.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{MutexGuard, PoisonError};
use std::time::{Duration as StdDuration, Instant};

use chrono::{Duration, NaiveDate};
use egui::{Align, Align2, Color32, FontId, Key, KeyboardShortcut, Layout, Modifiers, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use plancraft_core::leveling::{LevelingOptions, clear_leveling, level_resources};
use plancraft_core::prelude::*;
use plancraft_core::workspace::{ChangeSource, Document, SharedWorkspace, Workspace};

use crate::commands::Command;
use crate::dialogs::{self, Dialogs};
use crate::mcp_host::McpHost;
use crate::menubar::{self, MenuInfo};
use crate::recovery::{Recovery, RecoveryStore};
use crate::settings::AppSettings;
use crate::start_page;
use crate::state::{ToastKind, UiState, ViewKind};
use crate::theme;
use crate::views;
use crate::views::print_gantt::PrintSettings;

#[cfg(target_os = "windows")]
fn resize_window_edges(ctx: &egui::Context) {
    use egui::{CursorIcon, ResizeDirection as D, ViewportCommand};
    let edge = ctx.input(|i| {
        if i.viewport().maximized.unwrap_or(false) {
            return None;
        }
        let pos = i.pointer.hover_pos()?;
        let rect = i.content_rect();
        let left = pos.x <= rect.left() + 5.0;
        let right = pos.x >= rect.right() - 5.0;
        let top = pos.y <= rect.top() + 5.0;
        let bottom = pos.y >= rect.bottom() - 5.0;
        let (direction, cursor) = match (left, right, top, bottom) {
            (true, _, true, _) => (D::NorthWest, CursorIcon::ResizeNwSe),
            (_, true, true, _) => (D::NorthEast, CursorIcon::ResizeNeSw),
            (true, _, _, true) => (D::SouthWest, CursorIcon::ResizeNeSw),
            (_, true, _, true) => (D::SouthEast, CursorIcon::ResizeNwSe),
            (true, _, _, _) => (D::West, CursorIcon::ResizeHorizontal),
            (_, true, _, _) => (D::East, CursorIcon::ResizeHorizontal),
            (_, _, true, _) => (D::North, CursorIcon::ResizeVertical),
            (_, _, _, true) => (D::South, CursorIcon::ResizeVertical),
            _ => return None,
        };
        Some((direction, cursor, i.pointer.primary_pressed()))
    });
    if let Some((direction, cursor, pressed)) = edge {
        ctx.set_cursor_icon(cursor);
        if pressed {
            ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
        }
    }
}

pub struct PlanCraftApp {
    ws: SharedWorkspace,
    st: UiState,
    dialogs: Dialogs,
    settings: AppSettings,
    font_loaded: bool,
    mcp: McpHost,
    seen: HashMap<u64, u64>,
    status: Option<(String, Instant)>,
    confirm_close: Option<CloseRequest>,
    allow_exit: bool,
    last_title: String,
    /// Developer screenshot automation (`PLANCRAFT_SCREENSHOT=out.png`).
    shot: Option<(PathBuf, u32)>,
    /// Handle of the active document last frame (agents may open or switch documents).
    last_active: Option<u64>,
    /// Recovery copies of unsaved work.
    recovery: Recovery,
    /// The last recovery-copy error shown (shown once until it changes).
    recovery_error: Option<String>,
    /// Message of the last internal error caught by the frame guard, until dismissed.
    crash: Option<String>,
    /// Developer/test hook: panic inside the next frame (`PLANCRAFT_CMD=Panic`).
    inject_panic: bool,
    print_settings: PrintSettings,
}

/// Locks the shared workspace. A panic caught while it was held must not lock the user out of
/// their plans, so a poisoned lock is simply taken over.
pub fn lock(ws: &SharedWorkspace) -> MutexGuard<'_, Workspace> {
    ws.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Whether a summary task above task `i` is one of `sel`.
fn has_selected_ancestor(p: &Project, i: usize, sel: &[TaskId]) -> bool {
    let mut a = p.parent_index(i);
    while let Some(ai) = a {
        if p.tasks.get(ai).is_some_and(|t| sel.contains(&t.id)) {
            return true;
        }
        a = p.parent_index(ai);
    }
    false
}

/// Resolve selected task IDs again after a structural edit: their row indices can change.
fn follow_task_row(ctx: &egui::Context, project: &Project, st: &mut UiState, target: TaskId) {
    let old_row = st.task_grid.cursor.map_or(0, |(row, _)| row);
    let col = st.task_grid.cursor.map_or(2, |(_, col)| col);
    let locate = |rows: &[usize], id: TaskId| {
        rows.iter()
            .enumerate()
            .filter_map(|(row, &index)| project.tasks.get(index).filter(|task| task.id == id).map(|_| row))
            .min_by_key(|row| row.abs_diff(old_row))
    };
    let mut rows = views::visible_rows(project, st);
    if locate(&rows, target).is_none() {
        // An indent can place the target inside a previously collapsed summary.
        let mut parent = project.task_index(target).and_then(|index| project.parent_index(index));
        while let Some(index) = parent {
            if let Some(task) = project.tasks.get(index) {
                st.collapsed.remove(&task.id);
            }
            parent = project.parent_index(index);
        }
        rows = views::visible_rows(project, st);
    }
    let Some(row) = locate(&rows, target) else {
        // A filter or collapsed group can still hide the target. Do not point at an unrelated row.
        st.task_grid.cursor = None;
        st.task_grid.selection = Some(crate::widgets::grid::GridSelection::Rows { anchor: 0, focus: 0 });
        return;
    };
    let anchor = st.sel.anchor.and_then(|id| locate(&rows, id)).unwrap_or(row);
    st.task_grid.cursor = Some((row, col));
    st.task_grid.selection = Some(crate::widgets::grid::GridSelection::Rows { anchor, focus: row });
    st.task_grid.scroll_to = Some(row);
    if matches!(st.view, ViewKind::Gantt | ViewKind::TrackingGantt) {
        ctx.memory_mut(|memory| memory.request_focus(egui::Id::new(("task_grid", st.table.label())).with("body")));
    }
}

fn active_selected_task(project: &Project, st: &UiState) -> Option<TaskId> {
    let rows = views::visible_rows(project, st);
    st.task_grid
        .cursor
        .and_then(|(row, _)| views::task_at(project, &rows, row).map(|task| task.id))
        .filter(|id| st.sel.tasks.contains(id))
        .or(st.sel.anchor.filter(|id| st.sel.tasks.contains(id)))
        .or_else(|| views::selected_in_order(project, &st.sel).first().copied())
}

/// Longest panic message shown to the user.
const MAX_CRASH_CHARS: usize = 400;

#[derive(Clone, Copy, PartialEq)]
enum CloseRequest {
    Document(u64),
    App,
}

impl PlanCraftApp {
    pub fn new(cc: &eframe::CreationContext<'_>, ws: SharedWorkspace) -> Self {
        let mut settings = AppSettings::load();
        if let Some(t) = std::env::var("PLANCRAFT_THEME").ok().and_then(|v| theme::ThemeChoice::parse(&v)) {
            settings.theme = t;
        }
        theme::install(&cc.egui_ctx, settings.theme);
        let has_doc = !lock(&ws).docs.is_empty();
        let st = UiState { table_width: settings.table_width, start_page: !has_doc, ..UiState::default() };
        let mut app = Self {
            ws,
            st,
            dialogs: Dialogs::default(),
            settings,
            font_loaded: false,
            mcp: McpHost::default(),
            seen: HashMap::new(),
            status: None,
            confirm_close: None,
            allow_exit: false,
            last_title: String::new(),
            shot: std::env::var_os("PLANCRAFT_SCREENSHOT").map(|p| (PathBuf::from(p), 0)),
            last_active: None,
            // Tests never touch the user's recovery folder; they inject their own store.
            recovery: Recovery::new(if cfg!(test) { None } else { RecoveryStore::user() }),
            recovery_error: None,
            crash: None,
            inject_panic: false,
            print_settings: PrintSettings::default(),
        };
        if !app.recovery.found.is_empty() && !app.st.start_page {
            app.st.toast(
                ToastKind::Info,
                crate::tr("PlanCraft found unsaved work from an earlier session. Open the Start Page (File menu) to restore it."),
            );
        }
        if let Ok(v) = std::env::var("PLANCRAFT_TAB")
            && let Some(tab) = crate::state::MenuTab::ALL
                .into_iter()
                .find(|t| t.label().eq_ignore_ascii_case(&v) || format!("{t:?}").eq_ignore_ascii_case(&v))
        {
            app.st.menu_tab = tab;
        }
        if let Ok(v) = std::env::var("PLANCRAFT_VIEW") {
            let key = v.to_lowercase().replace([' ', '_', '-'], "");
            if let Some(view) = ViewKind::ALL
                .into_iter()
                .find(|k| k.label().to_lowercase().replace(' ', "") == key || format!("{k:?}").to_lowercase() == key)
            {
                app.st.view = view;
            }
        }
        if let Ok(v) = std::env::var("PLANCRAFT_GROUP") {
            if let Some(g) = crate::state::TaskGroup::ALL.into_iter().find(|g| format!("{g:?}").eq_ignore_ascii_case(&v)) {
                app.st.group = g;
            }
            if let Some(g) = crate::state::ResourceGroup::ALL.into_iter().find(|g| format!("{g:?}").eq_ignore_ascii_case(&v)) {
                app.st.resource_group = g;
            }
        }
        if app.settings.mcp_autostart {
            app.mcp.start(app.ws.clone(), cc.egui_ctx.clone(), app.settings.mcp_port);
        }
        app
    }

    fn set_status(&mut self, s: impl Into<String>) {
        self.status = Some((s.into(), Instant::now()));
    }

    // ------------------------------------------------------------------
    // Commands
    // ------------------------------------------------------------------

    fn shortcuts(&mut self, ctx: &egui::Context, out: &mut Vec<Command>) {
        // Only a focused text field should swallow shortcuts (the task grid holds focus too).
        let typing = ctx.text_edit_focused();
        let grid_focused = ctx.memory(|m| m.has_focus(egui::Id::new(("task_grid", self.st.table.label())).with("body")));
        if typing && ctx.input(|i| i.events.iter().any(|event| matches!(event, egui::Event::Copy))) {
            self.st.task_clipboard_text = None;
        }
        if !typing {
            let copy_rows = !grid_focused || self.st.task_grid.rows_selected();
            if !copy_rows && ctx.input(|i| i.events.iter().any(|event| matches!(event, egui::Event::Copy))) {
                self.st.task_clipboard_text = None;
            }
            ctx.input_mut(|i| {
                i.events.retain(|event| {
                    let command = match event {
                        egui::Event::Copy if copy_rows => Some(Command::Copy),
                        egui::Event::Cut => Some(Command::Cut),
                        egui::Event::Paste(text) if self.st.task_clipboard_text.as_ref() == Some(text) => Some(Command::Paste),
                        egui::Event::Paste(_) if !grid_focused => Some(Command::Paste),
                        _ => None,
                    };
                    if let Some(command) = command {
                        out.push(command);
                        false
                    } else {
                        true
                    }
                });
            });
        }
        let cmd = Modifiers::COMMAND;
        let map: Vec<(KeyboardShortcut, Command, bool)> = vec![
            (KeyboardShortcut::new(cmd, Key::N), Command::NewProject, true),
            (KeyboardShortcut::new(cmd, Key::O), Command::Open, true),
            (KeyboardShortcut::new(cmd, Key::P), Command::PrintGantt, true),
            (KeyboardShortcut::new(cmd | Modifiers::SHIFT, Key::S), Command::SaveAs, true),
            (KeyboardShortcut::new(cmd, Key::S), Command::Save, true),
            (KeyboardShortcut::new(cmd | Modifiers::SHIFT, Key::Z), Command::Redo, false),
            (KeyboardShortcut::new(cmd, Key::Z), Command::Undo, false),
            (KeyboardShortcut::new(cmd, Key::Y), Command::Redo, false),
            (KeyboardShortcut::new(Modifiers::ALT | Modifiers::SHIFT, Key::ArrowRight), Command::Indent, false),
            (KeyboardShortcut::new(Modifiers::ALT | Modifiers::SHIFT, Key::ArrowLeft), Command::Outdent, false),
            (KeyboardShortcut::new(Modifiers::ALT | Modifiers::SHIFT, Key::ArrowUp), Command::MoveUp, false),
            (KeyboardShortcut::new(Modifiers::ALT | Modifiers::SHIFT, Key::ArrowDown), Command::MoveDown, false),
            (KeyboardShortcut::new(cmd | Modifiers::SHIFT, Key::F2), Command::UnlinkSelected, false),
            (KeyboardShortcut::new(cmd, Key::F2), Command::LinkSelected, false),
            (KeyboardShortcut::new(Modifiers::SHIFT, Key::F2), Command::TaskInfo, false),
            (KeyboardShortcut::new(cmd, Key::Plus), Command::ZoomIn, false),
            (KeyboardShortcut::new(cmd, Key::Equals), Command::ZoomIn, false),
            (KeyboardShortcut::new(cmd, Key::Minus), Command::ZoomOut, false),
            (KeyboardShortcut::new(cmd, Key::C), Command::Copy, false),
            (KeyboardShortcut::new(cmd, Key::X), Command::Cut, false),
            (KeyboardShortcut::new(cmd, Key::V), Command::Paste, false),
        ];
        for (sc, c, global) in map {
            let grid_cell_clipboard = grid_focused && matches!(c, Command::Copy | Command::Paste);
            if !grid_cell_clipboard && (global || !typing) && ctx.input_mut(|i| i.consume_shortcut(&sc)) {
                out.push(c);
            }
        }
    }

    fn with_doc<R>(&mut self, f: impl FnOnce(&mut Document, &mut UiState) -> R) -> Option<R> {
        let ws = self.ws.clone();
        let mut ws = lock(&ws);
        let doc = ws.active_doc_mut()?;
        Some(f(doc, &mut self.st))
    }

    fn open_path(&mut self, path: PathBuf) {
        let r = lock(&self.ws).open(&path);
        match r {
            Ok(_) => {
                self.settings.add_recent(path.clone());
                self.st.start_page = false;
                self.st.sel = Default::default();
                self.st.gantt.initialized_for = None;
                self.set_status(crate::tr_dyn(&format!("Opened {}", path.display())));
            }
            Err(e) => self.st.toast(ToastKind::Error, e.to_string()),
        }
    }

    /// Imports a CSV / Excel / MS Project XML file as a new, unsaved document.
    fn import_path(&mut self, path: &std::path::Path) {
        match plancraft_core::interchange::import_file(path) {
            Ok(p) => {
                let (n, r) = (p.tasks.len(), p.resources.len());
                lock(&self.ws).add(p, None);
                self.st.start_page = false;
                self.st.sel = Default::default();
                self.st.view = ViewKind::Gantt;
                self.st.gantt.initialized_for = None;
                self.st.toast(ToastKind::Success, crate::tr_dyn(&format!("Imported {n} tasks and {r} resources")));
            }
            Err(e) => {
                self.st.toast(ToastKind::Error, crate::tr_dyn(&format!("Could not import {}: {}", path.display(), crate::tr_dyn(&e))))
            }
        }
    }

    fn save(&mut self, save_as: bool) -> bool {
        let ws = self.ws.clone();
        let mut ws = lock(&ws);
        let Some(doc) = ws.active_doc_mut() else {
            return false;
        };
        let path = if save_as || doc.path.is_none() {
            let name = format!("{}.{}", doc.project.name, FILE_EXTENSION);
            match rfd::FileDialog::new().add_filter(crate::tr("PlanCraft project"), &[FILE_EXTENSION]).set_file_name(name).save_file() {
                Some(mut p) => {
                    if p.extension().is_none() {
                        p.set_extension(FILE_EXTENSION);
                    }
                    Some(p)
                }
                None => return false,
            }
        } else {
            None
        };
        let r = match path {
            Some(p) => doc.save_as(&p).map(|_| p),
            None => doc.save().map(|_| doc.path.clone().unwrap_or_default()),
        };
        drop(ws);
        match r {
            Ok(p) => {
                self.settings.add_recent(p.clone());
                self.set_status(crate::tr_dyn(&format!("Saved {}", p.display())));
                true
            }
            Err(e) => {
                self.st.toast(ToastKind::Error, e.to_string());
                false
            }
        }
    }

    fn export_csv(&mut self) {
        let Some((name, csv)) = self.with_doc(|doc, _| {
            let p = &doc.project;
            let headers = [
                "ID",
                "Unique ID",
                "WBS",
                "Outline Level",
                "Name",
                "Milestone",
                "Duration",
                "Start",
                "Finish",
                "% Complete",
                "Work",
                "Cost",
                "Predecessors",
                "Resource Names",
                "Critical",
            ];
            let mut t = dialogs::reports::ReportTable {
                title: String::new(),
                headers: headers.iter().map(|h| crate::tr(h).to_string()).collect(),
                right: vec![false; headers.len()],
                rows: Vec::new(),
                bold: Vec::new(),
                cash: Vec::new(),
                note: None,
            };
            let num = |v: f64| {
                let s = format!("{v:.2}");
                if plancraft_core::i18n::is_ru() { s.replace('.', ",") } else { s }
            };
            let yes = |b: bool| if b { crate::tr("Yes") } else { crate::tr("No") };
            for (i, task) in p.tasks.iter().enumerate() {
                t.rows.push(vec![
                    (i + 1).to_string(),
                    task.id.0.to_string(),
                    p.wbs(i),
                    task.outline_level.to_string(),
                    task.name.clone(),
                    yes(task.is_milestone()).to_string(),
                    p.format_duration(task.sched.duration, task.estimated),
                    crate::format::iso(task.sched.start),
                    crate::format::iso(task.sched.finish),
                    format!("{:.0}", task.sched.percent_complete),
                    num(task.sched.work as f64 / 60.0),
                    num(task.sched.cost),
                    p.format_predecessors(task.id),
                    p.format_resource_names(task.id),
                    yes(task.sched.critical).to_string(),
                ]);
                t.bold.push(false);
            }
            (p.name.clone(), dialogs::reports::to_csv(&t))
        }) else {
            return;
        };
        if let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).set_file_name(format!("{name}.csv")).save_file() {
            match std::fs::write(&path, csv) {
                Ok(()) => self.set_status(crate::tr_dyn(&format!("Exported {}", path.display()))),
                Err(e) => self.st.toast(ToastKind::Error, e.to_string()),
            }
        }
    }

    fn exec(&mut self, ctx: &egui::Context, cmd: Command) {
        match cmd {
            Command::NewProject => {
                let start = chrono::Local::now().date_naive();
                let mut ws = lock(&self.ws);
                let n = ws.docs.len() + 1;
                ws.add(Project::new(crate::tr_dyn(&format!("Project{n}")), start), None);
                drop(ws);
                self.st.start_page = false;
                self.st.sel = Default::default();
                self.st.view = ViewKind::Gantt;
            }
            Command::NewDemo => {
                let today = chrono::Local::now().date_naive();
                let monday = today - Duration::days(chrono::Datelike::weekday(&today).num_days_from_monday() as i64) - Duration::days(7);
                lock(&self.ws).add(plancraft_core::sample::demo_project(monday), None);
                self.st.start_page = false;
                self.st.sel = Default::default();
                self.st.view = ViewKind::Gantt;
            }
            Command::Open => {
                if let Some(path) = rfd::FileDialog::new().add_filter(crate::tr("PlanCraft project"), &[FILE_EXTENSION]).pick_file() {
                    self.open_path(path);
                }
            }
            Command::OpenPath(p) => self.open_path(p),
            Command::Save => {
                self.save(false);
            }
            Command::SaveAs => {
                self.save(true);
            }
            Command::Close => {
                let info = lock(&self.ws).active_doc().map(|d| (d.handle, d.dirty));
                if let Some((h, dirty)) = info {
                    if dirty {
                        self.confirm_close = Some(CloseRequest::Document(h));
                    } else {
                        self.close_doc(h);
                    }
                }
            }
            Command::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Command::StartPage => {
                self.st.start_page = true;
            }
            Command::RestoreRecovered(stem) => self.restore_recovered(&stem),
            Command::DiscardRecovered(stem) => {
                self.recovery.discard(&stem);
                self.set_status(crate::tr("Recovery copy discarded"));
            }
            Command::ExportCsv => self.export_csv(),
            Command::PrintGantt => {
                self.st.print_preview = true;
                self.st.start_page = false;
            }
            Command::Templates => self.dialogs.templates = true,
            Command::NewFromTemplate(t) => {
                let start = chrono::Local::now().date_naive();
                lock(&self.ws).add(plancraft_core::templates::build(t, start), None);
                self.st.start_page = false;
                self.st.sel = Default::default();
                self.st.view = ViewKind::Gantt;
                self.dialogs.templates = false;
            }
            Command::NewFromTemplateFile(path) => match plancraft_core::io::load(&path) {
                Ok(mut p) => {
                    // A copy of the template: new identity, starting today, not linked to the file.
                    p.id = uuid::Uuid::new_v4();
                    let today = chrono::Local::now().date_naive();
                    p.start = plancraft_core::schedule::project_calendar(&p).day_start(today);
                    plancraft_core::schedule::recalculate(&mut p);
                    lock(&self.ws).add(p, None);
                    self.st.start_page = false;
                    self.st.sel = Default::default();
                    self.st.view = ViewKind::Gantt;
                    self.dialogs.templates = false;
                }
                Err(e) => self.st.toast(ToastKind::Error, e.to_string()),
            },
            Command::Import => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(crate::tr("Microsoft Project XML"), &["xml"])
                    .add_filter(crate::tr("Spreadsheet (CSV, XLSX)"), &["csv", "xlsx", "xlsm", "txt"])
                    .add_filter("*", &["xml", "csv", "xlsx", "xlsm", "txt", "mpp"])
                    .pick_file()
                {
                    self.import_path(&path);
                }
            }
            Command::McpPanel => {
                self.dialogs.mcp = true;
                self.st.start_page = false;
            }
            Command::About => self.dialogs.about = true,
            Command::SetView(v) => {
                self.st.view = v;
                self.st.start_page = false;
            }
            Command::Filter(f) => self.st.filter = f,
            Command::Table(t) => self.st.table = t,
            Command::History => self.dialogs.history = !self.dialogs.history,
            Command::SetLang(l) => {
                plancraft_core::i18n::set_lang(l);
                crate::fonts::install(ctx, &self.settings.font_family);
                self.settings.language = l;
                self.settings.save();
            }
            Command::SetFont(family) => {
                crate::fonts::install(ctx, &family);
                self.settings.font_family = family;
                self.settings.save();
            }
            Command::SetTheme(t) => {
                self.settings.theme = t;
                self.settings.save();
                theme::set_choice(ctx, t);
            }
            other => {
                let ws = self.ws.clone();
                let mut ws = lock(&ws);
                let Some(doc) = ws.active_doc_mut() else {
                    return;
                };
                self.exec_doc(ctx, doc, other);
            }
        }
    }

    /// Opens a recovery copy as an unsaved document, bound to its original file when it had one.
    fn restore_recovered(&mut self, stem: &str) {
        let Some(item) = self.recovery.find(stem).cloned() else { return };
        let Some(store) = self.recovery.store().cloned() else { return };
        match store.load(&item) {
            Ok((project, notes)) => {
                let mut ws = lock(&self.ws);
                ws.add(project, item.original_path.clone());
                if let Some(doc) = ws.active_doc_mut() {
                    doc.dirty = true;
                    doc.load_notes = notes;
                }
                // Copy the restored document under its new handle before the old copy goes.
                let copied = self.recovery.write_dirty(&ws);
                drop(ws);
                match copied {
                    Ok(_) => self.recovery.discard(stem),
                    // Keep the old copy and drop it from the list only (it stays on disk).
                    Err(e) => {
                        self.recovery.found.retain(|i| i.stem != stem);
                        self.st.toast(ToastKind::Warning, e);
                    }
                }
                self.st.start_page = false;
                self.st.sel = Default::default();
                self.st.gantt.initialized_for = None;
                self.st.view = ViewKind::Gantt;
                let target = match &item.original_path {
                    Some(p) => crate::tr_dyn(&format!("Save writes it back to {}", p.display())),
                    None => crate::tr("Save it to keep it").to_string(),
                };
                self.set_status(crate::tr_dyn(&format!("Restored {} from a recovery copy. {target}.", item.name)));
            }
            Err(e) => self.st.toast(ToastKind::Error, crate::tr_dyn(&format!("Could not restore {}: {e}", item.name))),
        }
    }

    fn close_doc(&mut self, handle: u64) {
        let mut ws = lock(&self.ws);
        ws.close(handle);
        let empty = ws.docs.is_empty();
        // The user chose to save or discard: the recovery copy is no longer needed.
        self.recovery.prune(&ws);
        drop(ws);
        self.st.sel = Default::default();
        self.st.gantt.initialized_for = None;
        if empty {
            self.st.start_page = true;
        }
    }

    fn exec_doc(&mut self, ctx: &egui::Context, doc: &mut Document, cmd: Command) {
        let st = &mut self.st;
        let sel: Vec<TaskId> = views::selected_in_order(&doc.project, &st.sel);
        let first_sel_index = sel.first().and_then(|id| doc.project.task_index(*id));
        fn report<T>(st: &mut UiState, r: PlanResult<T>) {
            if let Err(e) = r {
                st.toast(ToastKind::Warning, e.to_string());
            }
        }
        match cmd {
            Command::Undo => {
                let target = st.task_grid.rows_selected().then(|| active_selected_task(&doc.project, st)).flatten();
                if doc.undo() {
                    if let Some(id) = target.filter(|id| doc.project.task(*id).is_some()) {
                        follow_task_row(ctx, &doc.project, st, id);
                    }
                    let l = doc.last_change.as_ref().map(|l| l.0.clone()).unwrap_or_default();
                    self.status = Some((l, Instant::now()));
                }
            }
            Command::Redo => {
                let target = st.task_grid.rows_selected().then(|| active_selected_task(&doc.project, st)).flatten();
                if doc.redo()
                    && let Some(id) = target.filter(|id| doc.project.task(*id).is_some())
                {
                    follow_task_row(ctx, &doc.project, st, id);
                }
            }
            Command::InsertTask | Command::InsertMilestone => {
                let milestone = cmd == Command::InsertMilestone;
                let at = first_sel_index;
                let r = doc.edit(if milestone { crate::tr("Insert Milestone") } else { crate::tr("Insert Task") }, |p| {
                    if milestone { p.add_milestone(crate::tr("<New Milestone>"), at) } else { p.add_task(crate::tr("<New Task>"), at) }
                });
                let id = match r {
                    Ok(id) => id,
                    Err(e) => return st.toast(ToastKind::Warning, e.to_string()),
                };
                st.sel.select_task(id);
                if let Some(r) = doc.project.task_index(id) {
                    let rows = views::visible_rows(&doc.project, st);
                    if let Some(row) = rows.iter().position(|&i| i == r) {
                        let name_col = st.table.columns().iter().position(|c| *c == crate::columns::TaskCol::Name).unwrap_or(0);
                        st.task_grid.begin_edit(row, name_col, String::new());
                        st.task_grid.scroll_to = Some(row);
                    }
                }
            }
            Command::InsertSummary => {
                if sel.is_empty() {
                    match doc.edit(crate::tr("Insert Summary Task"), |p| p.add_task(crate::tr("<New Summary Task>"), None)) {
                        Ok(id) => st.sel.select_task(id),
                        Err(e) => st.toast(ToastKind::Warning, e.to_string()),
                    }
                } else {
                    let r = doc.try_edit(crate::tr("Insert Summary Task"), ChangeSource::User, |p| {
                        p.add_summary_over(crate::tr("<New Summary Task>"), &sel)
                    });
                    match r {
                        Ok(id) => st.sel.select_task(id),
                        Err(e) => st.toast(ToastKind::Warning, e.to_string()),
                    }
                }
            }
            Command::DeleteSelection => {
                if st.view == ViewKind::ResourceSheet {
                    return self.exec_doc(ctx, doc, Command::DeleteResources);
                }
                if !sel.is_empty() {
                    let n = sel.len();
                    let r =
                        doc.try_edit(if n == 1 { crate::tr("Delete Task") } else { crate::tr("Delete Tasks") }, ChangeSource::User, |p| {
                            p.delete_tasks(&sel).map(|_| ())
                        });
                    report(st, r);
                    st.sel.tasks.clear();
                }
            }
            Command::Indent | Command::Outdent => {
                let indent = cmd == Command::Indent;
                let target = active_selected_task(&doc.project, st);
                // Only move top-most selected blocks (children follow their parent).
                let tops: Vec<TaskId> = sel
                    .iter()
                    .copied()
                    .filter(|id| doc.project.task_index(*id).is_some_and(|i| !has_selected_ancestor(&doc.project, i, &sel)))
                    .collect();
                if tops.is_empty() {
                    return;
                }
                let r: PlanResult<()> =
                    doc.try_edit(if indent { crate::tr("Indent") } else { crate::tr("Outdent") }, ChangeSource::User, |p| {
                        for id in &tops {
                            if indent { p.indent(*id)? } else { p.outdent(*id)? }
                        }
                        Ok(())
                    });
                match r {
                    Ok(()) => {
                        if let Some(id) = target {
                            follow_task_row(ctx, &doc.project, st, id);
                        }
                    }
                    Err(e) => st.toast(ToastKind::Warning, e.to_string()),
                }
            }
            Command::MoveUp | Command::MoveDown => {
                if let Some(id) = sel.first().copied() {
                    let up = cmd == Command::MoveUp;
                    let r = doc.try_edit(if up { crate::tr("Move Up") } else { crate::tr("Move Down") }, ChangeSource::User, |p| {
                        p.shift_task(id, up)
                    });
                    match r {
                        Ok(()) => follow_task_row(ctx, &doc.project, st, id),
                        Err(e) => st.toast(ToastKind::Warning, e.to_string()),
                    }
                }
            }
            Command::LinkSelected => {
                if sel.len() >= 2 {
                    let r = doc.try_edit(crate::tr("Link Tasks"), ChangeSource::User, |p| p.link_chain(&sel));
                    report(st, r);
                } else {
                    st.toast(ToastKind::Info, crate::tr("Select two or more tasks to link them in order."));
                }
            }
            Command::UnlinkSelected => {
                if !sel.is_empty() {
                    let r = doc.edit(crate::tr("Unlink Tasks"), |p| p.unlink_tasks(&sel));
                    report(st, r);
                }
            }
            Command::SetPercent(pct) => {
                if !sel.is_empty() {
                    let r = doc.try_edit(&crate::tr_dyn(&format!("{pct}% Complete")), ChangeSource::User, |p| {
                        for id in &sel {
                            p.set_percent_complete(*id, pct)?;
                        }
                        Ok(())
                    });
                    report(st, r);
                }
            }
            Command::MarkOnTrack => {
                // Progress up to the status date (or now).
                let at = plancraft_core::report::status_moment(&doc.project);
                let r = doc.try_edit(crate::tr("Mark on Track"), ChangeSource::User, |p| {
                    let targets: Vec<(TaskId, u8)> = {
                        let wc = plancraft_core::schedule::project_calendar(p);
                        sel.iter()
                            .filter_map(|id| {
                                let t = p.task(*id)?;
                                let (s, f) = (t.sched.start?, t.sched.finish?);
                                let pct = if at >= f {
                                    100
                                } else if at <= s {
                                    0
                                } else {
                                    (wc.between(s, at) * 100 / wc.between(s, f).max(1)) as u8
                                };
                                Some((*id, pct))
                            })
                            .collect()
                    };
                    for (id, pct) in targets {
                        p.set_percent_complete(id, pct)?;
                    }
                    Ok(())
                });
                report(st, r);
            }
            Command::ToggleActive => {
                if !sel.is_empty() {
                    let r = doc.edit(crate::tr("Inactivate Task"), |p| {
                        let new = !sel.first().and_then(|id| p.task(*id)).map(|t| t.active).unwrap_or(true);
                        for id in &sel {
                            if let Some(t) = p.task_mut(*id) {
                                t.active = new;
                            }
                        }
                    });
                    report(st, r);
                }
            }
            Command::SetMode(mode) => {
                if !sel.is_empty() {
                    let r = doc.edit(
                        if mode == ScheduleMode::Manual { crate::tr("Manually Schedule") } else { crate::tr("Auto Schedule") },
                        |p| {
                            for id in &sel {
                                if let Some(t) = p.task_mut(*id) {
                                    if mode == ScheduleMode::Manual && t.mode == ScheduleMode::Auto {
                                        t.manual_start = t.sched.start;
                                        t.manual_finish = t.sched.finish;
                                    }
                                    t.mode = mode;
                                }
                            }
                        },
                    );
                    report(st, r);
                }
            }
            Command::TaskInfo | Command::TaskNotes => {
                if let Some(id) = st.sel.single_task().or(sel.first().copied()) {
                    self.dialogs.task_info = dialogs::task_info::TaskInfo::new(&doc.project, id, cmd == Command::TaskNotes);
                }
            }
            Command::ToggleTimeline => {
                if !sel.is_empty() {
                    let r = doc.edit(crate::tr("Add to Timeline"), |p| {
                        let new = !sel.first().and_then(|id| p.task(*id)).map(|t| t.show_on_timeline).unwrap_or(false);
                        for id in &sel {
                            if let Some(t) = p.task_mut(*id) {
                                t.show_on_timeline = new;
                            }
                        }
                    });
                    report(st, r);
                    st.timeline_visible = true;
                }
            }
            Command::ScrollToTask => {
                if let Some(id) = sel.first()
                    && let Some(t) = doc.project.task(*id)
                    && let Some(s) = t.sched.start
                {
                    st.gantt.scroll_to(s);
                    let rows = views::visible_rows(&doc.project, st);
                    if let Some(idx) = doc.project.task_index(*id)
                        && let Some(r) = rows.iter().position(|&i| i == idx)
                    {
                        st.task_grid.scroll_to = Some(r);
                    }
                }
            }
            Command::Copy | Command::Cut => {
                if !sel.is_empty() {
                    let mut copied = Vec::new();
                    for (i, t) in doc.project.tasks.iter().enumerate() {
                        let inside = sel.contains(&t.id) || has_selected_ancestor(&doc.project, i, &sel);
                        if inside {
                            copied.push(t.clone());
                        }
                    }
                    st.clipboard = copied;
                    let text = st.clipboard.iter().map(|task| task.name.as_str()).collect::<Vec<_>>().join("\n");
                    ctx.copy_text(text.clone());
                    st.task_clipboard_text = Some(text);
                    if cmd == Command::Cut {
                        let r = doc.try_edit(crate::tr("Cut Tasks"), ChangeSource::User, |p| p.delete_tasks(&sel).map(|_| ()));
                        report(st, r);
                        st.sel.tasks.clear();
                    }
                }
            }
            Command::Paste => {
                if !st.clipboard.is_empty() {
                    let clip = st.clipboard.clone();
                    let at = first_sel_index.unwrap_or(doc.project.tasks.len());
                    let r = doc.edit(crate::tr("Paste Tasks"), |p| {
                        let base = clip.iter().map(|t| t.outline_level).min().unwrap_or(1);
                        let target_level = p.tasks.get(at).map_or(1, |t| t.outline_level);
                        let at = at.min(p.tasks.len());
                        let mut ids = Vec::new();
                        for (k, t) in clip.iter().enumerate() {
                            let mut nt = t.clone();
                            nt.id = p.ids.next_task();
                            nt.predecessors.clear();
                            nt.members.clear();
                            nt.baseline = None;
                            nt.outline_level = target_level.saturating_add(t.outline_level.saturating_sub(base));
                            ids.push(nt.id);
                            p.tasks.insert(at + k, nt);
                        }
                        p.normalize_outline();
                        ids
                    });
                    match r {
                        Ok(ids) => {
                            let first = ids.first().copied();
                            st.sel.tasks = ids.into_iter().collect();
                            st.sel.anchor = first;
                            if let Some(first) = first {
                                let rows = views::visible_rows(&doc.project, st);
                                if let Some(row) =
                                    rows.iter().position(|&index| doc.project.tasks.get(index).is_some_and(|task| task.id == first))
                                {
                                    let col = st.task_grid.cursor.map_or(2, |(_, col)| col);
                                    let last = rows
                                        .iter()
                                        .enumerate()
                                        .filter_map(|(row, &index)| {
                                            doc.project.tasks.get(index).filter(|task| st.sel.tasks.contains(&task.id)).map(|_| row)
                                        })
                                        .max()
                                        .unwrap_or(row);
                                    st.task_grid.selection = Some(crate::widgets::grid::GridSelection::Rows { anchor: row, focus: last });
                                    st.task_grid.cursor = Some((row, col));
                                    st.task_grid.scroll_to = Some(row);
                                }
                            }
                        }
                        Err(e) => st.toast(ToastKind::Warning, e.to_string()),
                    }
                }
            }
            Command::InsertResource => {
                let r = doc.edit(crate::tr("Add Resource"), |p| {
                    let n = p.resources.len() + 1;
                    p.add_resource(crate::tr_dyn(&format!("Resource {n}")))
                });
                let id = match r {
                    Ok(id) => id,
                    Err(e) => return st.toast(ToastKind::Warning, e.to_string()),
                };
                st.sel.select_resource(id);
                if st.view != ViewKind::ResourceSheet {
                    st.view = ViewKind::ResourceSheet;
                }
                if let Some(row) = doc.project.resources.iter().position(|r| r.id == id) {
                    st.resource_grid.begin_edit(row, 1, String::new());
                    st.resource_grid.scroll_to = Some(row);
                }
            }
            Command::DeleteResources => {
                let ids: Vec<ResourceId> = st.sel.resources.iter().copied().collect();
                if !ids.is_empty() {
                    let r = doc.try_edit(crate::tr("Delete Resource"), ChangeSource::User, |p| p.delete_resources(&ids));
                    report(st, r);
                    st.sel.resources.clear();
                }
            }
            Command::ResourceInfo => {
                if let Some(id) = st.sel.single_resource() {
                    self.dialogs.resource_info = dialogs::resource_info::ResourceInfo::new(&doc.project, id);
                }
            }
            Command::AssignResources => self.dialogs.assign = Some(Default::default()),
            Command::LevelAll => {
                let r = match doc.edit(crate::tr("Level Resources"), |p| level_resources(p, &LevelingOptions::default())) {
                    Ok(r) => r,
                    Err(e) => return st.toast(ToastKind::Warning, e.to_string()),
                };
                let msg = if r.delayed.is_empty() && r.unresolved.is_empty() {
                    crate::tr("No overallocations needed leveling.").to_string()
                } else {
                    crate::tr_dyn(&format!(
                        "Leveling delayed {} task(s). Finish: {} – {}",
                        r.delayed.len(),
                        crate::format::date(r.finish_before),
                        crate::format::date(r.finish_after)
                    ))
                };
                st.toast(ToastKind::Success, msg);
                for u in r.unresolved {
                    st.toast(ToastKind::Warning, u);
                }
            }
            Command::ClearLeveling => report(st, doc.edit(crate::tr("Clear Leveling"), clear_leveling)),
            Command::ProjectInfo => self.dialogs.project_info = Some(dialogs::project_info::ProjectInfo::new(&doc.project)),
            Command::ChangeWorkingTime => self.dialogs.working_time = Some(dialogs::working_time::WorkingTime::new(&doc.project)),
            Command::UpdateProgressFromTime => match doc.edit(crate::tr("Update Progress from Time"), |p| p.update_progress_from_time()) {
                Ok(_) => st.toast(ToastKind::Success, crate::tr("Progress updated from tracked time")),
                Err(e) => st.toast(ToastKind::Warning, e.to_string()),
            },
            Command::ExternalLinks => self.dialogs.external = Some(Default::default()),
            Command::RefreshExternal => {
                let w = doc.refresh_external();
                if w.is_empty() {
                    st.toast(ToastKind::Success, crate::tr("External links are up to date"));
                }
                for m in w {
                    st.toast(ToastKind::Warning, m);
                }
            }
            Command::Statistics => self.dialogs.stats = true,
            Command::BaselineDialog(clear) => self.dialogs.baseline = Some(dialogs::extra::BaselineDialog::new(clear, sel.clone())),
            Command::UpdateProject => {
                self.dialogs.update_project = Some(dialogs::extra::UpdateProjectDialog::new(&doc.project, sel.clone()))
            }
            Command::DefineWbs => self.dialogs.wbs = Some(dialogs::extra::WbsDialog::new(&doc.project)),
            Command::Reports => {
                if self.dialogs.reports.is_none() {
                    self.dialogs.reports = Some(Default::default());
                }
            }
            Command::CustomReport => {
                self.dialogs.reports.get_or_insert_with(Default::default).kind = dialogs::reports::ReportKind::Custom;
            }
            Command::RecurringTask => {
                let at = first_sel_index.unwrap_or(doc.project.tasks.len());
                self.dialogs.recurring = Some(dialogs::extra::RecurringTask::new(&doc.project, at));
            }
            Command::Hyperlink => {
                if let Some(id) = st.sel.single_task().or(sel.first().copied()) {
                    self.dialogs.hyperlink = Some(dialogs::extra::HyperlinkDialog::new(&doc.project, id));
                }
            }
            Command::OpenHyperlink => {
                let addr =
                    st.sel.single_task().or(sel.first().copied()).and_then(|id| doc.project.task(id)).and_then(|t| t.hyperlink.clone());
                if let Some(h) = addr
                    && let Err(e) = dialogs::extra::open_link(&h.address)
                {
                    st.toast(ToastKind::Warning, crate::tr_dyn(&format!("Could not open {}: {}", h.address, e)));
                }
            }
            Command::SortResources(k) => report(st, doc.edit(crate::tr("Sort Resources"), |p| p.sort_resources(k))),
            Command::ExportXml => {
                let name = doc.project.name.clone();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(crate::tr("Microsoft Project XML"), &["xml"])
                    .set_file_name(format!("{name}.xml"))
                    .save_file()
                {
                    let xml = plancraft_core::interchange::export_mspdi(&doc.project);
                    match plancraft_core::io::write_atomic(&path, xml.as_bytes(), plancraft_core::io::Privacy::KeepExisting) {
                        Ok(()) => st.toast(ToastKind::Success, crate::tr_dyn(&format!("Exported to MS Project XML: {}", path.display()))),
                        Err(e) => st.toast(ToastKind::Error, crate::tr_dyn(&format!("Could not save {}: {}", path.display(), e))),
                    }
                }
            }
            Command::SaveTemplate => {
                if let Some(dir) = crate::dialogs::misc::config_dir().map(|d| d.join("templates")) {
                    let _ = std::fs::create_dir_all(&dir);
                    let name = format!("{}.{}", doc.project.name, plancraft_core::model::FILE_EXTENSION);
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter(crate::tr("PlanCraft project"), &[plancraft_core::model::FILE_EXTENSION])
                        .set_directory(&dir)
                        .set_file_name(name)
                        .save_file()
                    {
                        // A template keeps the plan but not progress or tracked time.
                        let mut t = doc.project.clone();
                        t.time_entries.clear();
                        for task in &mut t.tasks {
                            task.percent_complete = 0;
                            task.actual_start = None;
                            task.actual_finish = None;
                            task.baseline = None;
                            task.baselines.clear();
                            task.interim.clear();
                        }
                        match plancraft_core::io::save(&t, &path) {
                            Ok(()) => st.toast(ToastKind::Success, crate::tr_dyn(&format!("Template saved: {}", path.display()))),
                            Err(e) => st.toast(ToastKind::Error, e.to_string()),
                        }
                    }
                }
            }
            Command::ZoomIn | Command::ZoomOut => {
                let g = &mut st.gantt;
                g.zoom(if cmd == Command::ZoomIn { 1 } else { -1 }, 400.0, 0.0);
            }
            Command::ZoomEntire | Command::ZoomSelected => {
                let (s, f) = if cmd == Command::ZoomEntire {
                    (doc.project.sched.start, doc.project.sched.finish)
                } else {
                    let ts: Vec<&Task> = sel.iter().filter_map(|id| doc.project.task(*id)).collect();
                    (ts.iter().filter_map(|t| t.sched.start).min(), ts.iter().filter_map(|t| t.sched.finish).max())
                };
                if let (Some(s), Some(f)) = (s, f) {
                    let width = (ctx.content_rect().width() - st.table_width - 80.0).max(200.0);
                    let days = ((f - s).num_minutes() as f32 / 1440.0).max(1.0) * 1.1 + 2.0;
                    st.gantt.px_per_day = (width / days).clamp(0.5, 140.0);
                    st.gantt.left = s - Duration::days(1);
                }
            }
            Command::CollapseAll => {
                for (i, t) in doc.project.tasks.iter().enumerate() {
                    if doc.project.has_children(i) {
                        st.collapsed.insert(t.id);
                    }
                }
            }
            Command::ExpandAll => st.collapsed.clear(),
            _ => {}
        }
    }

    // ------------------------------------------------------------------
    // Layout pieces
    // ------------------------------------------------------------------

    fn menu_info(&self, ws: &Workspace) -> MenuInfo {
        let doc = ws.active_doc();
        MenuInfo {
            has_doc: doc.is_some(),
            title: doc.map(|d| d.title()).unwrap_or_default(),
            can_undo: doc.is_some_and(|d| d.can_undo()),
            can_redo: doc.is_some_and(|d| d.can_redo()),
            undo_label: doc.and_then(|d| d.undo_label().map(String::from)),
            task_selected: !self.st.sel.tasks.is_empty(),
            resource_selected: !self.st.sel.resources.is_empty(),
            mcp_running: self.mcp.running(),
            mcp_url: self.mcp.url(),
        }
    }

    fn doc_tabs(&mut self, ui: &mut Ui, ws: &mut Workspace, out: &mut Vec<Command>) {
        if ws.docs.len() < 2 {
            return;
        }
        let rect = ui.available_rect_before_wrap();
        let rect = Rect::from_min_size(rect.min, vec2(rect.width(), 26.0));
        ui.painter().rect_filled(rect, 0.0, theme::pal().PANEL_BG);
        let mut child =
            ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(8.0, 2.0))).layout(Layout::left_to_right(Align::Center)));
        let mut switch = None;
        let mut close = None;
        for (i, d) in ws.docs.iter().enumerate() {
            let active = ws.active == Some(i);
            let resp = child.selectable_label(active, format!("📄 {}", d.title()));
            if resp.clicked() {
                switch = Some(i);
            }
            if child.small_button("✖").on_hover_text(crate::tr("Close")).clicked() {
                close = Some((d.handle, d.dirty));
            }
            child.add_space(6.0);
        }
        if let Some(i) = switch {
            ws.active = Some(i);
            self.st.sel = Default::default();
            self.st.gantt.initialized_for = None;
        }
        if let Some((h, dirty)) = close {
            if dirty {
                self.confirm_close = Some(CloseRequest::Document(h));
            } else {
                ws.close(h);
                if ws.docs.is_empty() {
                    self.st.start_page = true;
                }
            }
        }
        let _ = out;
        ui.allocate_rect(rect, Sense::hover());
    }

    fn status_bar(&mut self, ui: &mut Ui, ws: &mut Workspace, out: &mut Vec<Command>) {
        let doc = ws.active_doc_mut();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let msg = match &self.status {
                Some((m, at)) if at.elapsed() < StdDuration::from_secs(8) => m.clone(),
                _ => crate::tr("Ready").to_string(),
            };
            ui.label(RichText::new(msg).size(12.0));
            if let Some(doc) = doc {
                ui.separator();
                let auto = doc.project.settings.new_tasks_auto;
                let resp = ui.add(
                    egui::Label::new(
                        RichText::new(if auto {
                            crate::tr("⟳ New Tasks: Auto Scheduled")
                        } else {
                            crate::tr("📌 New Tasks: Manually Scheduled")
                        })
                        .size(12.0),
                    )
                    .sense(Sense::click()),
                );
                if resp.on_hover_text(crate::tr("Click to change how new tasks are scheduled")).clicked()
                    && let Err(e) = doc.edit(crate::tr("New Task Mode"), |p| p.settings.new_tasks_auto = !auto)
                {
                    self.st.toast(ToastKind::Warning, e.to_string());
                }
                if self.st.filter != crate::state::TaskFilter::All {
                    ui.separator();
                    ui.label(
                        RichText::new(crate::tr_dyn(&format!("Filter: {}", self.st.filter.label())))
                            .size(12.0)
                            .color(theme::pal().ACCENT_MID),
                    );
                }
                let warnings = doc.project.sched.warnings.len();
                if warnings > 0 {
                    ui.separator();
                    let r = ui.add(
                        egui::Label::new(
                            RichText::new(crate::tr_dyn(&format!("⚠ {warnings} scheduling issue(s)"))).size(12.0).color(theme::pal().ERROR),
                        )
                        .sense(Sense::click()),
                    );
                    if r.on_hover_text(doc.project.sched.warnings.join("\n")).clicked() {
                        out.push(Command::Statistics);
                    }
                }
                if let Some((label, ChangeSource::Agent)) = &doc.last_change {
                    ui.separator();
                    ui.label(
                        RichText::new(crate::tr_dyn(&format!("⚡ Last AI change: {label}"))).size(12.0).color(theme::pal().ACCENT_MID),
                    );
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let mut z = self.st.gantt.px_per_day;
                if ui
                    .add(egui::Slider::new(&mut z, 1.0..=140.0).logarithmic(true).show_value(false))
                    .on_hover_text(crate::tr("Timescale zoom"))
                    .changed()
                {
                    self.st.gantt.px_per_day = z;
                }
                ui.label(RichText::new("🔍").size(12.0));
                ui.separator();
                for v in [ViewKind::Timesheet, ViewKind::ResourceSheet, ViewKind::TeamPlanner, ViewKind::TaskUsage, ViewKind::Gantt] {
                    let active = self.st.view == v;
                    if ui.selectable_label(active, RichText::new(v.icon()).size(13.0)).on_hover_text(v.label()).clicked() {
                        out.push(Command::SetView(v));
                    }
                }
            });
        });
    }

    fn view_bar(&self, ui: &Ui, rect: Rect) {
        let p = ui.painter();
        p.rect_filled(rect, 0.0, theme::pal().TITLE_BAR);
        let label = self.st.view.label().to_uppercase();
        let galley = p.layout_no_wrap(label, FontId::proportional(12.0), Color32::WHITE);
        let w = galley.size().x;
        let pos = pos2(rect.center().x - galley.size().y / 2.0, rect.center().y + w / 2.0);
        p.add(egui::epaint::TextShape::new(pos, galley, Color32::WHITE).with_angle(-std::f32::consts::FRAC_PI_2));
    }

    fn toasts(&mut self, ctx: &egui::Context) {
        self.st.toasts.retain(|t| t.at.elapsed() < StdDuration::from_secs(6));
        if self.st.toasts.is_empty() {
            return;
        }
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::new("toasts")).order(egui::Order::Foreground).anchor(Align2::RIGHT_BOTTOM, vec2(-16.0, -36.0)).show(
            ctx,
            |ui| {
                ui.set_max_width(420.0f32.min(screen.width() - 40.0));
                let mut dismiss = None;
                for (i, t) in self.st.toasts.iter().enumerate() {
                    let (icon, color) = match t.kind {
                        ToastKind::Info => ("ℹ", theme::pal().ACCENT_MID),
                        ToastKind::Success => ("✔", theme::pal().OK),
                        ToastKind::Warning => ("⚠", theme::pal().WARN),
                        ToastKind::Error => ("✖", theme::pal().ERROR),
                    };
                    let resp = egui::Frame::popup(ui.style())
                        .stroke(Stroke::new(1.0, color))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(icon).color(color).size(15.0));
                                ui.label(&t.text);
                            });
                        })
                        .response;
                    if resp.interact(Sense::click()).clicked() {
                        dismiss = Some(i);
                    }
                }
                if let Some(i) = dismiss {
                    self.st.toasts.remove(i);
                }
            },
        );
        ctx.request_repaint_after(StdDuration::from_millis(500));
    }

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(req) = self.confirm_close else {
            return;
        };
        let names: Vec<String> = {
            let ws = lock(&self.ws);
            match req {
                CloseRequest::Document(h) => ws.docs.iter().filter(|d| d.handle == h).map(|d| d.project.name.clone()).collect(),
                CloseRequest::App => ws.docs.iter().filter(|d| d.dirty).map(|d| d.project.name.clone()).collect(),
            }
        };
        let mut choice = None;
        let frame = egui::Frame::window(&ctx.style_of(ctx.theme()))
            .fill(theme::pal().BG)
            .stroke(Stroke::new(1.0, theme::pal().GRID_LINE))
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::same(24));
        egui::Modal::new(egui::Id::new("confirm_close")).frame(frame).show(ctx, |ui| {
            ui.set_width(400.0);
            ui.allocate_ui_with_layout(vec2(400.0, 38.0), Layout::left_to_right(Align::Center), |ui| {
                let (badge, _) = ui.allocate_exact_size(vec2(38.0, 38.0), Sense::hover());
                ui.painter().rect_filled(badge, 9.0, theme::pal().ACCENT_PALE);
                let c = badge.center();
                let stroke = Stroke::new(1.6, theme::pal().ACCENT_MID);
                ui.painter().rect_stroke(Rect::from_center_size(c, vec2(17.0, 19.0)), 2.0, stroke, egui::StrokeKind::Inside);
                ui.painter().line_segment([c + vec2(-5.0, -6.0), c + vec2(5.0, -6.0)], stroke);
                ui.painter().rect_stroke(
                    Rect::from_center_size(c + vec2(0.0, 4.0), vec2(10.0, 6.0)),
                    0.5,
                    stroke,
                    egui::StrokeKind::Inside,
                );
                ui.add_space(8.0);
                ui.label(RichText::new(crate::tr("Save changes?")).size(20.0).strong().color(theme::pal().TEXT));
            });
            ui.add_space(16.0);
            ui.label(
                RichText::new(crate::tr_dyn(&format!("Do you want to save changes to {}?", names.join(", "))))
                    .size(14.0)
                    .color(theme::pal().TEXT_WEAK),
            );
            ui.add_space(22.0);
            ui.allocate_ui_with_layout(vec2(400.0, 34.0), Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if ui.add_sized([90.0, 34.0], egui::Button::new(crate::tr("Cancel"))).clicked() {
                    choice = Some(2);
                }
                if ui.add_sized([110.0, 34.0], egui::Button::new(crate::tr("Don't Save"))).clicked() {
                    choice = Some(1);
                }
                if ui
                    .add_sized(
                        [90.0, 34.0],
                        egui::Button::new(RichText::new(crate::tr("Save")).color(Color32::WHITE)).fill(theme::pal().ACCENT),
                    )
                    .clicked()
                {
                    choice = Some(0);
                }
            });
        });
        match (choice, req) {
            (Some(0), CloseRequest::Document(h)) => {
                {
                    let mut ws = lock(&self.ws);
                    ws.active = ws.index_of(h);
                }
                if self.save(false) {
                    self.close_doc(h);
                }
                self.confirm_close = None;
            }
            (Some(1), CloseRequest::Document(h)) => {
                self.close_doc(h);
                self.confirm_close = None;
            }
            (Some(0), CloseRequest::App) => {
                let dirty: Vec<usize> = lock(&self.ws).docs.iter().enumerate().filter(|(_, d)| d.dirty).map(|(i, _)| i).collect();
                let mut all = true;
                for i in dirty {
                    lock(&self.ws).active = Some(i);
                    all &= self.save(false);
                }
                self.confirm_close = None;
                if all {
                    self.allow_exit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            (Some(1), CloseRequest::App) => {
                self.confirm_close = None;
                self.allow_exit = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            (Some(_), _) => self.confirm_close = None,
            _ => {}
        }
    }

    /// Developer automation used to capture screenshots of views and dialogs:
    /// `PLANCRAFT_SCREENSHOT=out.png [PLANCRAFT_SELECT=row] [PLANCRAFT_CMD=TaskInfo,...]`.
    fn screenshot_hook(&mut self, ctx: &egui::Context) {
        if self.shot.as_ref().is_some_and(|(_, f)| *f == 3) {
            let row: usize = std::env::var("PLANCRAFT_SELECT").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
            if let Some(doc) = lock(&self.ws).active_doc() {
                if let Some(t) = doc.project.tasks.get(row.saturating_sub(1)) {
                    self.st.sel.select_task(t.id);
                }
                if let Some(r) = doc.project.resources.first() {
                    self.st.sel.select_resource(r.id);
                }
            }
            for name in std::env::var("PLANCRAFT_CMD").unwrap_or_default().split(',').filter(|s| !s.is_empty()) {
                let cmd = match name {
                    "TaskInfo" => Command::TaskInfo,
                    "TaskNotes" => Command::TaskNotes,
                    "ResourceInfo" => Command::ResourceInfo,
                    "ProjectInfo" => Command::ProjectInfo,
                    "WorkingTime" => Command::ChangeWorkingTime,
                    "Statistics" => Command::Statistics,
                    "Mcp" => Command::McpPanel,
                    "Assign" => Command::AssignResources,
                    "External" => Command::ExternalLinks,
                    "About" => Command::About,
                    "Level" => Command::LevelAll,
                    "Reports" => Command::Reports,
                    "CashFlow" => {
                        let mut w = dialogs::reports::ReportsWindow::default();
                        w.kind = dialogs::reports::ReportKind::CashFlow;
                        self.dialogs.reports = Some(w);
                        continue;
                    }
                    "Recurring" => Command::RecurringTask,
                    "Baseline" => Command::BaselineDialog(false),
                    "UpdateProject" => Command::UpdateProject,
                    "CustomReport" => Command::CustomReport,
                    "Wbs" => Command::DefineWbs,
                    "Templates" => Command::Templates,
                    "Hyperlink" => Command::Hyperlink,
                    "McpStart" => {
                        self.mcp.start(self.ws.clone(), ctx.clone(), self.settings.mcp_port);
                        continue;
                    }
                    "Critical" => {
                        self.st.gantt.show_critical = true;
                        continue;
                    }
                    "PrintGantt" => {
                        self.st.print_preview = true;
                        self.st.start_page = false;
                        continue;
                    }
                    "FileMenu" => {
                        egui::Popup::open_id(ctx, egui::Id::new("tab_file").with("popup"));
                        continue;
                    }
                    "ViewMenu" => {
                        egui::Popup::open_id(ctx, egui::Id::new("view_selector").with("popup"));
                        continue;
                    }
                    "StartPage" => {
                        self.st.start_page = true;
                        continue;
                    }
                    "Panic" => {
                        self.inject_panic = true;
                        continue;
                    }
                    _ => continue,
                };
                self.exec(ctx, cmd);
            }
        }
        let Some((path, frame)) = &mut self.shot else {
            return;
        };
        *frame += 1;
        let delay: u32 = std::env::var("PLANCRAFT_SHOT_DELAY").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        if *frame == 40 + delay * 60 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None })
        });
        if let Some(img) = shot {
            let [w, h] = img.size;
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                let _ = buf.save(&*path);
            }
            self.allow_exit = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
    }

    fn welcome(&mut self, ui: &mut Ui, rect: Rect, out: &mut Vec<Command>) {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        child.vertical_centered(|ui| {
            ui.add_space(rect.height() * 0.3);
            ui.label(RichText::new(crate::tr("No project open")).size(18.0).color(theme::pal().TEXT_WEAK));
            if ui.button(crate::tr("Create a new project")).clicked() {
                out.push(Command::NewProject);
            }
            if ui.button(crate::tr("Open the sample project")).clicked() {
                out.push(Command::NewDemo);
            }
        });
    }
}

impl PlanCraftApp {
    /// Clean exit: settings saved, the MCP server stopped and this session's recovery copies
    /// deleted (the user saved or chose to discard every change).
    fn shutdown(&mut self) {
        // Screenshot automation may override the theme; keep the user's settings as they were.
        if self.shot.is_none() {
            self.settings.table_width = self.st.table_width;
            self.settings.save();
        }
        self.mcp.stop();
        self.recovery.forget_all();
    }

    /// Called after the frame guard caught a panic: secure the user's work first, then tell them.
    fn after_panic(&mut self, ctx: &egui::Context, message: &str) {
        let (any_dirty, saved) = {
            let ws = lock(&self.ws);
            (ws.docs.iter().any(|d| d.dirty), self.recovery.write_dirty(&ws))
        };
        let message: String = match message.char_indices().nth(MAX_CRASH_CHARS) {
            Some((cut, _)) => format!("{}…", message.get(..cut).unwrap_or(message)),
            None => message.to_string(),
        };
        self.crash = Some(match (any_dirty, saved) {
            (true, Ok(_)) => crate::tr_dyn(&format!(
                "PlanCraft hit an internal error; your unsaved work was saved as a recovery copy. Please report this: {message}"
            )),
            (true, Err(e)) => {
                crate::tr_dyn(&format!("PlanCraft hit an internal error. {e}, so please save your work now. Please report this: {message}"))
            }
            (false, _) => {
                crate::tr_dyn(&format!("PlanCraft hit an internal error; you had no unsaved changes. Please report this: {message}"))
            }
        });
        ctx.request_repaint();
    }

    /// The non-modal notice shown after an internal error, until dismissed.
    fn crash_notice(&mut self, ctx: &egui::Context) {
        let Some(msg) = self.crash.clone() else { return };
        let mut dismiss = false;
        egui::Area::new(egui::Id::new("crash_notice")).order(egui::Order::Foreground).anchor(Align2::CENTER_TOP, vec2(0.0, 72.0)).show(
            ctx,
            |ui| {
                egui::Frame::popup(ui.style()).stroke(Stroke::new(1.5, theme::pal().ERROR)).inner_margin(egui::Margin::same(12)).show(
                    ui,
                    |ui| {
                        ui.set_max_width(560.0);
                        ui.horizontal_top(|ui| {
                            ui.label(RichText::new("✖").color(theme::pal().ERROR).size(18.0));
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&msg).color(theme::pal().TEXT));
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    if ui.button(crate::tr("Copy details")).clicked() {
                                        ui.ctx().copy_text(msg.clone());
                                    }
                                    if ui.button(crate::tr("Dismiss")).clicked() {
                                        dismiss = true;
                                    }
                                });
                            });
                        });
                    },
                );
            },
        );
        if dismiss {
            self.crash = None;
        }
    }

    /// Shows what opening a file clamped into range, once per document.
    fn report_load_notes(&mut self, ws: &mut Workspace) {
        for doc in &mut ws.docs {
            if doc.load_notes.is_empty() {
                continue;
            }
            let notes = std::mem::take(&mut doc.load_notes);
            const SHOWN: usize = 5;
            let mut list = notes.iter().take(SHOWN).cloned().collect::<Vec<_>>().join("; ");
            if notes.len() > SHOWN {
                list.push_str(&crate::tr_dyn(&format!("; and {} more", notes.len() - SHOWN)));
            }
            self.st.toast(
                ToastKind::Warning,
                crate::tr_dyn(&format!("Opened {}; PlanCraft adjusted out-of-range values: {list}", doc.title())),
            );
        }
    }

    /// Everything drawn in one frame (run behind the frame guard).
    fn frame(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        #[cfg(target_os = "windows")]
        resize_window_edges(&ctx);
        theme::sync(&ctx);
        self.screenshot_hook(&ctx);
        if std::mem::take(&mut self.inject_panic) {
            injected_panic();
        }
        let mut out: Vec<Command> = Vec::new();

        // Intercept window close when there are unsaved changes.
        if ctx.input(|i| i.viewport().close_requested()) {
            let dirty = lock(&self.ws).docs.iter().any(|d| d.dirty);
            if dirty && !self.allow_exit {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.confirm_close = Some(CloseRequest::App);
            } else {
                self.shutdown();
            }
        }

        self.shortcuts(&ctx, &mut out);

        let ws_arc = self.ws.clone();
        let mut ws = lock(&ws_arc);

        // Keep recovery copies of unsaved work up to date.
        match self.recovery.tick(&ws) {
            Ok(()) => self.recovery_error = None,
            Err(e) => {
                if self.recovery_error.as_ref() != Some(&e) {
                    self.st.toast(ToastKind::Error, e.clone());
                }
                self.recovery_error = Some(e);
            }
        }
        self.report_load_notes(&mut ws);

        // Notice changes made by AI agents through MCP.
        let mut agent_msgs = Vec::new();
        for d in &ws.docs {
            let prev = self.seen.insert(d.handle, d.revision);
            if let (Some(prev), Some((label, ChangeSource::Agent))) = (prev, &d.last_change)
                && prev != d.revision
            {
                agent_msgs.push(crate::tr_dyn(&format!("⚡ AI agent: {label}")));
            }
        }
        for m in agent_msgs {
            self.set_status(m);
        }
        // A document opened or activated elsewhere (e.g. by an agent) becomes the view.
        let active = ws.active_doc().map(|d| d.handle);
        if active != self.last_active {
            if self.last_active.is_none() && active.is_some() {
                self.st.start_page = false;
            }
            self.st.sel = Default::default();
            self.st.gantt.initialized_for = None;
            self.last_active = active;
        }
        // Drop selections that no longer exist (e.g. deleted by an agent).
        if let Some(doc) = ws.active_doc() {
            let p = &doc.project;
            self.st.sel.tasks.retain(|id| p.task(*id).is_some());
            self.st.sel.resources.retain(|id| p.resource(*id).is_some());
        }

        // Window title.
        let title = ws.active_doc().map(|d| crate::tr_dyn(&format!("{} - PlanCraft", d.title()))).unwrap_or_else(|| "PlanCraft".into());
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }

        let info = self.menu_info(&ws);
        let start_open = self.st.start_page;

        if self.st.print_preview {
            let mut close_preview = false;
            if let Some(doc) = ws.active_doc() {
                egui::CentralPanel::default().show(ui, |ui| {
                    close_preview =
                        views::print_gantt::show(ui, &doc.project, &self.st, &mut self.print_settings, &self.settings.font_family);
                });
                if close_preview {
                    self.st.print_preview = false;
                }
            } else {
                self.st.print_preview = false;
            }
        } else if start_open {
            let stats = ws.active_doc().map(|d| plancraft_core::report::project_stats(&d.project));
            let sinfo = start_page::StartInfo {
                has_doc: info.has_doc,
                doc_title: ws.active_doc().map(|d| d.project.name.clone()).unwrap_or_default(),
                doc_path: ws.active_doc().and_then(|d| d.path.as_ref().map(|p| p.display().to_string())),
                stats,
                settings: &self.settings,
                recovered: &self.recovery.found,
            };
            egui::Panel::top("title").show_separator_line(false).frame(egui::Frame::new().fill(theme::pal().BG)).show(ui, |ui| {
                menubar::title_bar(ui, &info, &mut out);
            });
            egui::CentralPanel::no_frame().show(ui, |ui| {
                if start_page::show(ui, &sinfo, &mut self.st.font_filter, &mut out) {
                    self.st.start_page = false;
                }
            });
        } else {
            egui::Panel::top("title").frame(egui::Frame::new().fill(theme::pal().BG)).show(ui, |ui| {
                menubar::title_bar(ui, &info, &mut out);
                menubar::command_bar(ui, &mut self.st, &info, &mut out);
                self.doc_tabs(ui, &mut ws, &mut out);
            });
            egui::Panel::bottom("status")
                .frame(egui::Frame::new().fill(theme::pal().TITLE_BAR).inner_margin(egui::Margin::symmetric(10, 3)))
                .show(ui, |ui| {
                    // Office-style status bar: light text on the accent band.
                    let v = ui.visuals_mut();
                    v.override_text_color = Some(Color32::WHITE);
                    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                    v.widgets.hovered.weak_bg_fill = Color32::from_white_alpha(30);
                    v.selection.bg_fill = Color32::from_white_alpha(45);
                    self.status_bar(ui, &mut ws, &mut out);
                });
            egui::CentralPanel::no_frame().show(ui, |ui| {
                let full = ui.max_rect();
                ui.painter().rect_filled(full, 0.0, theme::pal().BG);
                let Some(doc) = ws.active_doc_mut() else {
                    self.welcome(ui, full, &mut out);
                    return;
                };
                let bar = Rect::from_min_size(full.min, vec2(22.0, full.height()));
                self.view_bar(ui, bar);
                let mut rect = Rect::from_min_max(pos2(full.left() + 22.0, full.top()), full.max);
                let st = &mut self.st;
                let show_timeline = st.timeline_visible && matches!(st.view, ViewKind::Gantt | ViewKind::TrackingGantt);
                if show_timeline {
                    let tl = Rect::from_min_size(rect.min, vec2(rect.width(), views::timeline::HEIGHT));
                    let chart_w = (rect.width() - st.table_width - 16.0).max(100.0);
                    views::timeline::show(ui, tl, &doc.project, &mut st.gantt, chart_w);
                    rect = Rect::from_min_max(pos2(rect.left(), tl.bottom()), rect.max);
                }
                match st.view {
                    ViewKind::Gantt => views::gantt::show(ui, rect, doc, st, false, &mut out),
                    ViewKind::TrackingGantt => views::gantt::show(ui, rect, doc, st, true, &mut out),
                    ViewKind::TaskUsage => views::task_usage::show(ui, rect, doc, st, &mut out),
                    ViewKind::Milestones => views::milestones::show(ui, rect, doc, st, &mut out),
                    ViewKind::Calendar => views::calendar::show(ui, rect, doc, st, &mut out),
                    ViewKind::ResourceSheet => views::resource_sheet::show(ui, rect, doc, st, &mut out),
                    ViewKind::ResourceUsage => views::resource_usage::show(ui, rect, doc, st, &mut out),
                    ViewKind::Workload => views::workload::show(ui, rect, doc, st),
                    ViewKind::TeamPlanner => views::team_planner::show(ui, rect, doc, st, &mut out),
                    ViewKind::Timesheet => views::timesheet::show(ui, rect, doc, st),
                }
            });
        }

        // Dialogs.
        if let Some(doc) = ws.active_doc_mut() {
            if let Some(d) = &mut self.dialogs.task_info
                && !d.show(&ctx, doc)
            {
                self.dialogs.task_info = None;
            }
            if let Some(d) = &mut self.dialogs.resource_info
                && !d.show(&ctx, doc)
            {
                self.dialogs.resource_info = None;
            }
            if let Some(d) = &mut self.dialogs.project_info
                && !d.show(&ctx, doc)
            {
                self.dialogs.project_info = None;
            }
            if let Some(d) = &mut self.dialogs.working_time
                && !d.show(&ctx, doc)
            {
                self.dialogs.working_time = None;
            }
            if let Some(d) = &mut self.dialogs.assign {
                let mut toasts = Vec::new();
                if !d.show(&ctx, doc, &self.st.sel, &mut toasts) {
                    self.dialogs.assign = None;
                }
                for (k, t) in toasts {
                    self.st.toast(k, t);
                }
            }
            if let Some(d) = &mut self.dialogs.external
                && !d.show(&ctx, doc, &mut self.st)
            {
                self.dialogs.external = None;
            }
            if self.dialogs.stats {
                dialogs::misc::statistics(&ctx, doc, &mut self.dialogs.stats);
            }
            if let Some(d) = &mut self.dialogs.recurring {
                let (keep, created) = d.show(&ctx, doc);
                if let Some(id) = created {
                    self.st.sel.select_task(id);
                }
                if !keep {
                    self.dialogs.recurring = None;
                }
            }
            if let Some(d) = &mut self.dialogs.baseline {
                let (keep, msg) = d.show(&ctx, doc);
                if let Some(m) = msg {
                    self.st.toast(ToastKind::Success, m);
                }
                if !keep {
                    self.dialogs.baseline = None;
                }
            }
            if let Some(d) = &mut self.dialogs.update_project {
                let (keep, msg) = d.show(&ctx, doc);
                if let Some(m) = msg {
                    self.st.toast(ToastKind::Success, m);
                }
                if !keep {
                    self.dialogs.update_project = None;
                }
            }
            if let Some(d) = &mut self.dialogs.wbs
                && !d.show(&ctx, doc)
            {
                self.dialogs.wbs = None;
            }
            if let Some(d) = &mut self.dialogs.hyperlink
                && !d.show(&ctx, doc)
            {
                self.dialogs.hyperlink = None;
            }
            if let Some(d) = &mut self.dialogs.reports
                && !d.show(&ctx, doc)
            {
                self.dialogs.reports = None;
            }
            if self.dialogs.history {
                dialogs::misc::history(&ctx, doc, &mut self.dialogs.history);
            }
        }
        drop(ws);
        if self.dialogs.about {
            dialogs::misc::about(&ctx, &mut self.dialogs.about);
        }
        if self.dialogs.templates {
            dialogs::misc::templates(&ctx, &mut self.dialogs.templates, &mut out);
        }
        if self.dialogs.mcp {
            let ws = self.ws.clone();
            dialogs::mcp_panel::show(&ctx, &mut self.dialogs.mcp, &mut self.mcp, &mut self.settings, &ws);
        }
        self.confirm_dialog(&ctx);
        self.toasts(&ctx);

        for c in out {
            self.exec(&ctx, c);
        }
        let _ = NaiveDate::MIN;
    }
}

/// Developer/test hook behind `PLANCRAFT_CMD=Panic`: exercises the frame guard.
#[allow(clippy::panic)]
fn injected_panic() {
    panic!("test panic requested with PLANCRAFT_CMD=Panic");
}

impl eframe::App for PlanCraftApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::pal().BG.to_normalized_gamma_f32()
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.font_loaded {
            crate::fonts::install(&ctx, &self.settings.font_family);
            self.font_loaded = true;
        }
        // Last line of defence: a bug while drawing must never take the user's plans with it.
        if let Err(p) = plancraft_core::guard::catch("drawing the window", || self.frame(ui)) {
            self.after_panic(&ctx, &p.message);
        }
        if let Err(p) = plancraft_core::guard::catch("showing an error", || self.crash_notice(&ctx)) {
            eprintln!("plancraft: {p}");
        }
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use chrono::Datelike;
    use egui_kittest::Harness;

    fn harness() -> Harness<'static, PlanCraftApp> {
        let mut ws = Workspace::default();
        ws.add(plancraft_core::sample::demo_project(NaiveDate::from_ymd_opt(2026, 10, 19).unwrap()), None);
        let shared = ws.shared();
        let mut h = Harness::builder().with_size(vec2(1440.0, 900.0)).build_eframe(move |cc| PlanCraftApp::new(cc, shared));
        h.run_steps(3);
        h
    }

    fn focus_task_grid(h: &mut Harness<'static, PlanCraftApp>) {
        let id = egui::Id::new(("task_grid", "Entry")).with("body");
        h.ctx.memory_mut(|m| m.request_focus(id));
        h.run_steps(2);
    }

    fn tasks(h: &Harness<'static, PlanCraftApp>) -> Vec<String> {
        let ws = h.state().ws.lock().unwrap();
        ws.active_doc().unwrap().project.tasks.iter().map(|t| t.name.clone()).collect()
    }

    fn grid_point(h: &Harness<'static, PlanCraftApp>, row: usize, col: Option<usize>) -> egui::Pos2 {
        let id = egui::Id::new(("task_grid", h.state().st.table.label())).with("body");
        let body = h.ctx.read_response(id).unwrap().rect;
        let grid = &h.state().st.task_grid;
        let x = col.map_or(body.left() + 17.0, |col| {
            body.left() + 34.0 + grid.widths.iter().take(col).sum::<f32>() + grid.widths[col] * 0.5 - grid.scroll_x
        });
        pos2(x, body.top() + (row as f32 + 0.5) * theme::ROW_HEIGHT - grid.scroll_y)
    }

    fn grid_drag(h: &mut Harness<'static, PlanCraftApp>, from: egui::Pos2, to: egui::Pos2) {
        h.hover_at(from);
        h.run_steps(1);
        h.drag_at(from);
        h.run_steps(1);
        h.hover_at(to);
        h.run_steps(2);
        h.drop_at(to);
        h.run_steps(2);
    }

    #[test]
    fn shift_click_extends_cells_after_shift_is_released_before_the_frame() {
        let mut h = harness();
        h.state_mut().st.task_grid.select_cell(1, 2, false);
        focus_task_grid(&mut h);
        let end = grid_point(&h, 2, Some(3));
        h.input_mut().events.extend([
            egui::Event::PointerMoved(end),
            egui::Event::PointerButton { pos: end, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::SHIFT },
            egui::Event::PointerButton { pos: end, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::SHIFT },
            egui::Event::ModifiersChanged(Modifiers::NONE),
        ]);
        h.run_steps(3);
        assert_eq!(h.state().st.task_grid.cell_bounds(), Some((1, 2, 2, 3)));
        assert!(!h.state().st.task_grid.rows_selected());
    }

    #[test]
    fn fast_first_drag_selects_rows_even_when_press_and_release_share_a_frame() {
        let mut h = harness();
        let from = grid_point(&h, 1, None);
        let to = grid_point(&h, 2, None);
        h.input_mut().events.extend([
            egui::Event::PointerMoved(from),
            egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE },
            egui::Event::PointerMoved(to),
            egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE },
        ]);
        h.run_steps(3);
        assert!(h.state().st.task_grid.rows_selected());
        assert_eq!(h.state().st.sel.tasks.len(), 2);
    }

    #[test]
    fn id_drag_inserts_tasks_but_cell_rectangle_overwrites_only_selected_cells() {
        let mut h = harness();
        let before = tasks(&h);
        let from = grid_point(&h, 1, None);
        let to = grid_point(&h, 2, None);
        grid_drag(&mut h, from, to);
        assert!(h.state().st.task_grid.rows_selected());
        assert_eq!(h.state().st.sel.tasks.len(), 2);
        h.event(egui::Event::Copy);
        h.run_steps(3);
        let copied_rows = h.state().st.task_clipboard_text.clone().unwrap();
        h.state_mut().st.task_grid.select_row(5, false);
        let id = h.state().ws.lock().unwrap().active_doc().unwrap().project.tasks[5].id;
        h.state_mut().st.sel.select_task(id);
        h.event(egui::Event::Paste(copied_rows));
        h.run_steps(3);
        assert_eq!(tasks(&h).len(), before.len() + 2);
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run_steps(3);
        assert_eq!(tasks(&h), before);

        let from = grid_point(&h, 2, Some(3));
        let to = grid_point(&h, 1, Some(2));
        grid_drag(&mut h, from, to);
        assert!(!h.state().st.task_grid.rows_selected());
        assert_eq!(h.state().st.task_grid.cell_bounds(), Some((1, 2, 2, 3)));
        h.event(egui::Event::Copy);
        h.run_steps(1);
        let copied_cells = h
            .output()
            .platform_output
            .commands
            .iter()
            .find_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
            .unwrap();
        let matrix = crate::widgets::tsv::decode(&copied_cells).unwrap();
        assert_eq!(matrix.len(), 2);
        assert_eq!(matrix[0].len(), 2);
        assert_eq!(matrix[0][0], before[1]);
        assert_eq!(matrix[1][0], before[2]);
        assert!(h.state().st.task_clipboard_text.is_none());
        h.state_mut().st.task_grid.select_cell(5, 2, false);
        h.event(egui::Event::Paste(copied_cells));
        h.run_steps(3);
        assert_eq!(tasks(&h).len(), before.len());
        assert_eq!(tasks(&h)[5], before[1]);
        assert_eq!(tasks(&h)[6], before[2]);
        assert_eq!(tasks(&h)[7], before[7]);
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run_steps(3);
        assert_eq!(tasks(&h), before, "one Undo restores all overwritten cells");
    }

    #[test]
    fn bulk_tsv_protects_auto_dates_keeps_column_positions_and_rolls_back_bad_values() {
        let mut h = harness();
        h.state_mut().st.task_grid.select_cell(1, 0, false);
        focus_task_grid(&mut h);
        let original_constraint = h.state().ws.lock().unwrap().active_doc().unwrap().project.tasks[1].constraint;
        h.event(egui::Event::Paste("ignored\tAuto Scheduled\tBulk task\t2d\t2099-01-01\t2099-01-02\t".into()));
        h.run_steps(3);
        {
            let ws = h.state().ws.lock().unwrap();
            let task = &ws.active_doc().unwrap().project.tasks[1];
            assert_eq!(task.name, "Bulk task");
            assert_eq!(task.duration, 2 * 480);
            assert_eq!(task.constraint, original_constraint);
            assert!(task.manual_start.is_none());
            assert!(task.sched.start.unwrap().date().year() < 2099);
        }
        let before_bad = serde_json::to_string(&h.state().ws.lock().unwrap().active_doc().unwrap().project).unwrap();
        h.state_mut().st.task_grid.select_cell(1, 2, false);
        h.event(egui::Event::Paste("Must roll back\tnot-a-duration\r\nSecond row\t3d".into()));
        h.run_steps(3);
        assert_eq!(serde_json::to_string(&h.state().ws.lock().unwrap().active_doc().unwrap().project).unwrap(), before_bad);
        h.state_mut().st.task_grid.select_cell(1, 4, false);
        h.key_press(Key::F2);
        h.run_steps(3);
        assert!(h.state().st.task_grid.editing.is_none(), "auto dates cannot enter the editor");
    }

    #[test]
    fn shift_selected_rows_native_copy_paste_preserves_tasks_and_undoes_together() {
        let mut h = harness();
        let before = tasks(&h);
        let original = {
            let ws = h.state().ws.lock().unwrap();
            ws.active_doc().unwrap().project.tasks.clone()
        };
        h.state_mut().st.sel.select_task(original[1].id);
        h.state_mut().st.task_grid.select_row(1, false);
        focus_task_grid(&mut h);
        // Windows can deliver Shift release before the next frame. The key event
        // still records that Shift was held when the arrow was pressed.
        for pressed in [true, false] {
            h.input_mut().events.push(egui::Event::Key {
                key: Key::ArrowDown,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            });
        }
        h.input_mut().events.push(egui::Event::ModifiersChanged(Modifiers::NONE));
        h.run_steps(3);
        assert_eq!(h.state().st.sel.tasks.len(), 2);
        h.event(egui::Event::Copy);
        h.run_steps(3);
        assert_eq!(h.state().st.clipboard.len(), 2);
        let copied = h.state().st.task_clipboard_text.clone().unwrap();
        assert_eq!(copied, format!("{}\n{}", before[1], before[2]));

        // Paste before another task after clearing the original selection.
        h.state_mut().st.sel.select_task(original[5].id);
        h.state_mut().st.task_grid.cursor = Some((5, 2));
        h.event(egui::Event::Paste(copied.clone()));
        h.run_steps(3);
        assert_eq!(tasks(&h).len(), before.len() + 2);
        {
            let ws = h.state().ws.lock().unwrap();
            let project = &ws.active_doc().unwrap().project;
            for (source, pasted) in original.iter().skip(1).take(2).zip(project.tasks.iter().skip(5).take(2)) {
                assert_ne!(source.id, pasted.id);
                assert_eq!(source.name, pasted.name);
                assert_eq!(source.duration, pasted.duration);
                assert_eq!(source.notes, pasted.notes);
            }
        }
        assert_eq!(h.state().st.sel.tasks.len(), 2);
        assert_eq!(h.state().st.task_grid.cursor, Some((5, 2)));
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run_steps(3);
        assert_eq!(tasks(&h), before, "one Undo removes the entire pasted block");

        // Pasting at a blank row appends the whole block too.
        h.state_mut().st.sel.tasks.clear();
        h.state_mut().st.task_grid.cursor = Some((before.len(), 2));
        h.event(egui::Event::Paste(copied));
        h.run_steps(3);
        assert_eq!(tasks(&h).len(), before.len() + 2);
        assert_eq!(tasks(&h)[before.len()], before[1]);
        assert_eq!(tasks(&h)[before.len() + 1], before[2]);

        // A subsequent single-cell copy must replace the row clipboard mode.
        h.state_mut().st.sel.select_task(original[1].id);
        h.state_mut().st.task_grid.select_cell(1, 2, false);
        h.event(egui::Event::Copy);
        h.run_steps(3);
        assert!(h.state().st.task_clipboard_text.is_none());
        h.state_mut().st.task_grid.select_cell(2, 2, false);
        h.event(egui::Event::Paste(before[1].clone()));
        h.run_steps(3);
        assert_eq!(tasks(&h).len(), before.len() + 2);
        assert_eq!(tasks(&h)[2], before[1]);
    }

    #[test]
    fn native_clipboard_events_copy_and_paste_cells_and_support_undo() {
        let mut h = harness();
        let before = tasks(&h);
        h.state_mut().st.task_grid.cursor = Some((1, 2));
        focus_task_grid(&mut h);
        // Windows emits Copy, without a Key C event or any required modifier state.
        h.event(egui::Event::Copy);
        h.run_steps(1);
        let copied = h
            .output()
            .platform_output
            .commands
            .iter()
            .find_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
            .expect("the cell value reaches the system clipboard");
        assert_eq!(copied, before[1]);

        h.state_mut().st.task_grid.cursor = Some((2, 2));
        h.event(egui::Event::Paste(copied.clone()));
        h.run_steps(3);
        assert_eq!(tasks(&h)[2], copied);
        assert_eq!(tasks(&h).len(), before.len(), "cell paste must not insert a task row");
        assert_eq!(h.state().st.task_grid.cursor, Some((2, 2)));
        assert!(h.state().st.task_grid.editing.is_none(), "paste commits immediately");
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run_steps(3);
        assert_eq!(tasks(&h), before);

        // External cell text overwrites an existing task; blank rows never insert tasks.
        h.state_mut().st.task_grid.select_cell(1, 2, false);
        h.event(egui::Event::Paste("External clipboard task".into()));
        h.run_steps(3);
        assert_eq!(tasks(&h)[1], "External clipboard task");
        h.state_mut().st.task_grid.select_cell(before.len(), 2, false);
        h.event(egui::Event::Paste("Must not create a task".into()));
        h.run_steps(3);
        assert_eq!(tasks(&h).len(), before.len());
    }

    #[test]
    fn native_clipboard_paste_respects_text_editor_and_enter_keeps_cursor() {
        let mut h = harness();
        h.state_mut().st.task_grid.cursor = Some((1, 2));
        focus_task_grid(&mut h);
        h.event(egui::Event::Text("Edited ".into()));
        h.run_steps(3);
        h.event(egui::Event::Paste("cell".into()));
        h.run_steps(3);
        assert_eq!(h.state().st.task_grid.editing.as_ref().unwrap().text, "Edited cell");
        h.key_press(Key::Enter);
        h.run_steps(3);
        assert_eq!(tasks(&h)[1], "Edited cell");
        assert_eq!(h.state().st.task_grid.cursor, Some((1, 2)));
        assert!(h.state().st.task_grid.editing.is_none());
    }

    #[test]
    fn typing_into_blank_row_creates_task_and_undo_removes_it() {
        let mut h = harness();
        let n = tasks(&h).len();
        // Put the cursor on the first blank row, Task Name column, and type.
        h.state_mut().st.task_grid.cursor = Some((n, 2));
        focus_task_grid(&mut h);
        h.event(egui::Event::Text("Write release notes".into()));
        h.run_steps(2);
        h.key_press(egui::Key::Enter);
        h.run_steps(3);
        let names = tasks(&h);
        assert_eq!(names.len(), n + 1, "{names:?}");
        assert_eq!(names.last().unwrap(), "Write release notes");

        h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Z);
        h.run_steps(3);
        assert_eq!(tasks(&h).len(), n);
    }

    #[test]
    fn insert_key_adds_task_and_edits_its_name() {
        let mut h = harness();
        let n = tasks(&h).len();
        let target = { h.state().ws.lock().unwrap().active_doc().unwrap().project.tasks[2].id };
        h.state_mut().st.sel.select_task(target);
        h.state_mut().st.task_grid.cursor = Some((2, 2));
        focus_task_grid(&mut h);
        h.key_press(egui::Key::Insert);
        h.run_steps(3);
        h.event(egui::Event::Text("Inserted step".into()));
        h.run_steps(2);
        h.key_press(egui::Key::Enter);
        h.run_steps(3);
        let names = tasks(&h);
        assert_eq!(names.len(), n + 1);
        assert_eq!(names[2], "Inserted step", "{names:?}");
    }

    #[test]
    fn task_shortcuts_switch_cell_selection_to_following_row_through_moves_and_outline() {
        let mut h = harness();
        let (target, above, level) = {
            let ws = h.state().ws.lock().unwrap();
            let project = &ws.active_doc().unwrap().project;
            (project.tasks[2].id, project.tasks[1].id, project.tasks[2].outline_level)
        };
        h.state_mut().st.sel.select_task(target);
        h.state_mut().st.task_grid.select_cell(2, 2, false);
        focus_task_grid(&mut h);

        for (key, expected_row) in [(Key::ArrowUp, 1), (Key::ArrowDown, 2)] {
            h.key_press_modifiers(Modifiers::ALT | Modifiers::SHIFT, key);
            h.run_steps(3);
            let st = &h.state().st;
            assert!(st.task_grid.rows_selected());
            assert_eq!(st.sel.single_task(), Some(target));
            assert_eq!(st.task_grid.cursor, Some((expected_row, 2)));
            assert_eq!(h.state().ws.lock().unwrap().active_doc().unwrap().project.task_index(target), Some(expected_row));
        }
        // Undo and Redo also move the row cursor with the same task ID.
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run_steps(3);
        assert_eq!(h.state().st.task_grid.cursor, Some((1, 2)));
        h.key_press_modifiers(Modifiers::COMMAND, Key::Y);
        h.run_steps(3);
        assert_eq!(h.state().st.task_grid.cursor, Some((2, 2)));

        // Indenting under a collapsed task must reveal and retain the target row.
        h.state_mut().st.collapsed.insert(above);
        h.state_mut().st.task_grid.select_cell(2, 2, false);
        h.key_press_modifiers(Modifiers::ALT | Modifiers::SHIFT, Key::ArrowRight);
        h.run_steps(3);
        assert!(h.state().st.task_grid.rows_selected());
        assert!(!h.state().st.collapsed.contains(&above));
        assert_eq!(h.state().st.task_grid.cursor, Some((2, 2)));
        assert_eq!(h.state().ws.lock().unwrap().active_doc().unwrap().project.task(target).unwrap().outline_level, level + 1);

        h.state_mut().st.task_grid.select_cell(2, 2, false);
        h.key_press_modifiers(Modifiers::ALT | Modifiers::SHIFT, Key::ArrowLeft);
        h.run_steps(3);
        assert!(h.state().st.task_grid.rows_selected());
        assert_eq!(h.state().st.sel.single_task(), Some(target));
        assert_eq!(h.state().st.task_grid.cursor, Some((2, 2)));
        assert_eq!(h.state().ws.lock().unwrap().active_doc().unwrap().project.task(target).unwrap().outline_level, level);
    }

    #[test]
    fn unsuccessful_move_keeps_cell_selection_and_multi_selection_keeps_task_ids() {
        let mut h = harness();
        let ids = {
            let ws = h.state().ws.lock().unwrap();
            let project = &ws.active_doc().unwrap().project;
            [project.tasks[0].id, project.tasks[1].id, project.tasks[2].id]
        };
        h.state_mut().st.sel.select_task(ids[0]);
        h.state_mut().st.task_grid.select_cell(0, 2, false);
        focus_task_grid(&mut h);
        h.key_press_modifiers(Modifiers::ALT | Modifiers::SHIFT, Key::ArrowRight);
        h.run_steps(3);
        assert_eq!(h.state().st.task_grid.cell_bounds(), Some((0, 2, 0, 2)));
        assert_eq!(h.state().st.sel.single_task(), Some(ids[0]));

        h.state_mut().st.sel.tasks = [ids[1], ids[2]].into_iter().collect();
        h.state_mut().st.sel.anchor = Some(ids[1]);
        h.state_mut().st.task_grid.select_row(1, false);
        h.state_mut().st.task_grid.select_row(2, true);
        h.key_press_modifiers(Modifiers::ALT | Modifiers::SHIFT, Key::ArrowDown);
        h.run_steps(3);
        assert_eq!(h.state().st.sel.tasks, [ids[1], ids[2]].into_iter().collect());
        assert!(h.state().st.task_grid.rows_selected());
        assert_eq!(h.state().st.task_grid.cursor, Some((2, 2)), "the task actually moved is followed");
        assert_eq!(h.state().ws.lock().unwrap().active_doc().unwrap().project.task_index(ids[1]), Some(2));
    }

    #[test]
    fn outline_and_link_shortcuts_work_while_grid_has_focus() {
        let mut h = harness();
        // Rows 17 and 18 ("Launch event", "Product live") under the Launch summary.
        let (a, b, level) = {
            let ws = h.state().ws.lock().unwrap();
            let p = &ws.active_doc().unwrap().project;
            (p.tasks[15].id, p.tasks[16].id, p.tasks[16].outline_level)
        };
        h.state_mut().st.sel.select_task(b);
        focus_task_grid(&mut h);
        h.key_press_modifiers(Modifiers::ALT | Modifiers::SHIFT, egui::Key::ArrowRight);
        h.run_steps(3);
        {
            let ws = h.state().ws.lock().unwrap();
            assert_eq!(ws.active_doc().unwrap().project.task(b).unwrap().outline_level, level + 1);
        }
        h.key_press_modifiers(Modifiers::ALT | Modifiers::SHIFT, egui::Key::ArrowLeft);
        h.run_steps(3);
        h.state_mut().st.sel.tasks = [a, b].into_iter().collect();
        h.key_press_modifiers(Modifiers::COMMAND, egui::Key::F2);
        h.run_steps(3);
        let ws = h.state().ws.lock().unwrap();
        let p = &ws.active_doc().unwrap().project;
        assert_eq!(p.task(b).unwrap().outline_level, level);
        assert!(p.task(b).unwrap().predecessors.iter().any(|d| d.predecessor == plancraft_core::model::TaskRef::Local(a)));
    }

    #[test]
    fn frame_guard_turns_a_panic_into_a_message_and_saves_unsaved_work() {
        let tmp = crate::recovery::tests::TempDir::new("frame-guard");
        let mut h = harness();
        h.state_mut().recovery = Recovery::new(Some(RecoveryStore::new(&tmp.0)));
        h.state().ws.lock().unwrap().active_doc_mut().unwrap().edit("Add", |p| p.add_task("Unsaved work", None)).unwrap();

        h.state_mut().inject_panic = true;
        h.run_steps(2);
        let msg = h.state().crash.clone().expect("the error is shown");
        assert!(msg.contains("saved as a recovery copy"), "{msg}");
        assert!(msg.contains("PLANCRAFT_CMD=Panic"), "{msg}");
        let copies = RecoveryStore::new(&tmp.0).list();
        assert_eq!(copies.len(), 1, "unsaved work was copied right away");

        // The app keeps running: frames draw and commands still work.
        h.run_steps(3);
        assert!(tasks(&h).iter().any(|t| t == "Unsaved work"));
        let ctx = h.ctx.clone();
        h.state_mut().exec(&ctx, Command::InsertTask);
        h.run_steps(2);
        assert!(tasks(&h).iter().any(|t| t == "<New Task>"));
        assert!(h.state().crash.is_some(), "the notice stays until dismissed");
    }

    #[test]
    fn recovered_work_restores_bound_to_its_file_and_can_be_discarded() {
        let tmp = crate::recovery::tests::TempDir::new("restore");
        let store = RecoveryStore::new(&tmp.0);
        // An earlier session left two copies behind.
        {
            let mut ws = Workspace::default();
            ws.add(
                Project::new("Crashed Plan", NaiveDate::from_ymd_opt(2026, 10, 19).unwrap()),
                Some(PathBuf::from("/plans/crashed.plancraft")),
            );
            ws.add(Project::new("Scratch", NaiveDate::from_ymd_opt(2026, 10, 19).unwrap()), None);
            for d in &mut ws.docs {
                d.edit("Add", |p| p.add_task("Recovered task", None)).unwrap();
            }
            Recovery::new(Some(store.clone())).write_dirty(&ws).unwrap();
        }
        let mut h = harness();
        h.state_mut().recovery = Recovery::new(Some(store.clone()));
        assert_eq!(h.state().recovery.found.len(), 2);
        h.state_mut().st.start_page = true;
        h.run_steps(2);

        let crashed = h.state().recovery.found.iter().find(|i| i.name == "Crashed Plan").unwrap().stem.clone();
        let ctx = h.ctx.clone();
        h.state_mut().exec(&ctx, Command::RestoreRecovered(crashed.clone()));
        h.run_steps(2);
        {
            let ws = h.state().ws.lock().unwrap();
            let doc = ws.active_doc().unwrap();
            assert_eq!(doc.project.name, "Crashed Plan");
            assert!(doc.dirty, "a restored copy is unsaved work");
            assert_eq!(doc.path.as_deref(), Some(std::path::Path::new("/plans/crashed.plancraft")));
            assert!(doc.project.tasks.iter().any(|t| t.name == "Recovered task"));
        }
        let left: Vec<String> = store.list().into_iter().map(|i| i.name).collect();
        assert_eq!(left.len(), 2, "the restored document has its own copy now: {left:?}");
        assert!(store.list().iter().all(|i| i.stem != crashed), "the old copy is gone");

        let scratch = h.state().recovery.found.iter().find(|i| i.name == "Scratch").unwrap().stem.clone();
        let ctx = h.ctx.clone();
        h.state_mut().exec(&ctx, Command::DiscardRecovered(scratch.clone()));
        assert!(h.state().recovery.found.is_empty());
        assert!(store.list().iter().all(|i| i.stem != scratch));
    }

    #[test]
    fn every_view_renders() {
        let mut h = harness();
        for v in ViewKind::ALL {
            h.state_mut().st.view = v;
            h.run_steps(2);
        }
        h.state_mut().st.start_page = true;
        h.run_steps(2);
    }
}

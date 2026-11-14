//! Dialogs added for project-management courses: Recurring Task, Set Baseline / Interim Plan,
//! WBS Code Definition and Hyperlink.

use egui::{Context, RichText};
use plancraft_core::extras::PlanSlot;
use plancraft_core::prelude::*;
use plancraft_core::workspace::Document;

use crate::edit::EditExt;
use crate::format;
use crate::theme;

fn window(title: String, id: &str, ctx: &Context, open: &mut bool) -> egui::Window<'static> {
    let _ = open;
    super::window(ctx, title)
        .id(egui::Id::new(id.to_string()))
        .pivot(egui::Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + egui::vec2(0.0, 120.0))
        .collapsible(false)
        .resizable(false)
}

/// OK / Cancel row; returns (ok, cancel).
fn ok_cancel(ui: &mut egui::Ui) -> (bool, bool) {
    let mut ok = false;
    let mut cancel = false;
    ui.separator();
    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 34.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui.button(crate::tr("Cancel")).clicked() {
            cancel = true;
        }
        if ui.add(egui::Button::new(RichText::new(crate::tr("  OK  ")).color(egui::Color32::WHITE)).fill(theme::pal().ACCENT)).clicked() {
            ok = true;
        }
    });
    (ok, cancel)
}

// ---------------------------------------------------------------------------
// Set Baseline / Interim Plan
// ---------------------------------------------------------------------------

pub struct BaselineDialog {
    clear: bool,
    interim: bool,
    number: u8,
    selected_only: bool,
    ids: Vec<TaskId>,
}

impl BaselineDialog {
    pub fn new(clear: bool, selected: Vec<TaskId>) -> Self {
        Self { clear, interim: false, number: 0, selected_only: false, ids: selected }
    }

    fn slot(&self) -> PlanSlot {
        if self.interim { PlanSlot::Interim(self.number.max(1)) } else { PlanSlot::Baseline(self.number) }
    }

    fn slot_name(p: &Project, slot: PlanSlot) -> String {
        let saved = p
            .plan_saved(slot)
            .map(|d| crate::tr_dyn(&format!("Saved: {}", format::date_long(Some(d)))))
            .unwrap_or_else(|| crate::tr("not set").into());
        let name = match slot {
            PlanSlot::Baseline(0) => crate::tr("Baseline").to_string(),
            PlanSlot::Baseline(n) => crate::tr_dyn(&format!("Baseline {n}")),
            PlanSlot::Interim(n) => crate::tr_dyn(&format!("Start/Finish {n}")),
        };
        format!("{name} — {saved}")
    }

    pub fn show(&mut self, ctx: &Context, doc: &mut Document) -> (bool, Option<String>) {
        let mut open = true;
        let mut close = false;
        let mut done = None;
        let title = if self.clear { crate::tr("Clear Baseline") } else { crate::tr("Set Baseline / Interim Plan") };
        window(title.into(), "baseline_dialog", ctx, &mut open).open(&mut open).show(ctx, |ui| {
            ui.set_min_width(420.0);
            let prev_interim = self.interim;
            ui.radio_value(&mut self.interim, false, crate::tr("Set baseline"));
            ui.radio_value(&mut self.interim, true, crate::tr("Set interim plan"));
            if prev_interim != self.interim {
                self.number = if self.interim { 1 } else { 0 };
            }
            let p = &doc.project;
            let range: Vec<u8> = if self.interim { (1..=10).collect() } else { (0..=10).collect() };
            egui::ComboBox::from_id_salt("bl_slot").width(380.0).selected_text(Self::slot_name(p, self.slot())).show_ui(ui, |ui| {
                for n in range {
                    let slot = if self.interim { PlanSlot::Interim(n) } else { PlanSlot::Baseline(n) };
                    ui.selectable_value(&mut self.number, n, Self::slot_name(p, slot));
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(crate::tr("For:"));
                ui.radio_value(&mut self.selected_only, false, crate::tr("Entire project"));
                ui.add_enabled_ui(!self.ids.is_empty(), |ui| {
                    ui.radio_value(&mut self.selected_only, true, crate::tr("Selected tasks"));
                });
            });
            let (ok, cancel) = ok_cancel(ui);
            if cancel {
                close = true;
            }
            if ok {
                let ids: Vec<TaskId> = if self.selected_only { self.ids.clone() } else { Vec::new() };
                let slot = self.slot();
                let clear = self.clear;
                let label = if clear { crate::tr("Clear Baseline") } else { crate::tr("Set Baseline") };
                let r = doc.edit_msg(label, |p| {
                    if clear {
                        p.clear_plan(slot, &ids);
                        Ok(())
                    } else {
                        p.save_plan(slot, &ids).map_err(|e| e.to_string())
                    }
                });
                match r {
                    Ok(()) => {
                        done = Some(match (clear, slot) {
                            (true, _) => crate::tr("Clear Baseline").to_string(),
                            (false, PlanSlot::Baseline(0)) => crate::tr("Baseline saved for all tasks").to_string(),
                            (false, PlanSlot::Baseline(n)) => crate::tr_dyn(&format!("Baseline {n} saved")),
                            (false, PlanSlot::Interim(_)) => crate::tr("Interim plan saved").to_string(),
                        });
                        close = true;
                    }
                    Err(e) => done = Some(e),
                }
            }
        });
        (open && !close, done)
    }
}

// ---------------------------------------------------------------------------
// WBS Code Definition
// ---------------------------------------------------------------------------

pub struct WbsDialog {
    mask: WbsMask,
}

impl WbsDialog {
    pub fn new(p: &Project) -> Self {
        let mut mask = p.settings.wbs.clone();
        if mask.levels.is_empty() {
            mask.levels = vec![WbsLevel { sequence: WbsSequence::Numbers, separator: ".".into() }; 3];
        }
        Self { mask }
    }

    pub fn show(&mut self, ctx: &Context, doc: &mut Document) -> bool {
        let mut open = true;
        let mut close = false;
        window(crate::tr("WBS Code Definition").into(), "wbs_dialog", ctx, &mut open).open(&mut open).show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.horizontal(|ui| {
                ui.label(crate::tr("Project code prefix:"));
                ui.add_sized([120.0, 22.0], egui::TextEdit::singleline(&mut self.mask.prefix));
            });
            ui.add_space(4.0);
            ui.label(RichText::new(crate::tr("Code mask:")).strong());
            egui::Grid::new("wbs_levels").striped(true).num_columns(3).spacing([16.0, 6.0]).show(ui, |ui| {
                ui.label(RichText::new(crate::tr("Level")).strong());
                ui.label(RichText::new(crate::tr("Sequence")).strong());
                ui.label(RichText::new(crate::tr("Separator")).strong());
                ui.end_row();
                let mut remove = None;
                for (i, l) in self.mask.levels.iter_mut().enumerate() {
                    ui.label((i + 1).to_string());
                    egui::ComboBox::from_id_salt(("wbs_seq", i)).width(160.0).selected_text(l.sequence.label()).show_ui(ui, |ui| {
                        for s in WbsSequence::ALL {
                            ui.selectable_value(&mut l.sequence, s, s.label());
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.add_sized([30.0, 22.0], egui::TextEdit::singleline(&mut l.separator));
                        if ui.small_button("✖").clicked() {
                            remove = Some(i);
                        }
                    });
                    ui.end_row();
                }
                if let Some(i) = remove
                    && self.mask.levels.len() > 1
                {
                    self.mask.levels.remove(i);
                }
            });
            if ui.button("➕").clicked() && self.mask.levels.len() < 9 {
                self.mask.levels.push(WbsLevel { sequence: WbsSequence::Numbers, separator: ".".into() });
            }
            ui.horizontal(|ui| {
                ui.label(crate::tr("Code preview:"));
                ui.label(RichText::new(self.mask.code(&[1, 2, 3])).monospace().strong());
            });
            let (ok, cancel) = ok_cancel(ui);
            if cancel {
                close = true;
            }
            if ok {
                let mask = self.mask.clone();
                let _ = doc.edit_msg(crate::tr("WBS Code Definition"), |p| {
                    p.settings.wbs = mask;
                    Ok::<(), String>(())
                });
                close = true;
            }
        });
        open && !close
    }
}

// ---------------------------------------------------------------------------
// Hyperlink
// ---------------------------------------------------------------------------

pub struct HyperlinkDialog {
    task: TaskId,
    text: String,
    address: String,
}

impl HyperlinkDialog {
    pub fn new(p: &Project, task: TaskId) -> Self {
        let h = p.task(task).and_then(|t| t.hyperlink.clone()).unwrap_or_default();
        Self { task, text: h.text, address: h.address }
    }

    pub fn show(&mut self, ctx: &Context, doc: &mut Document) -> bool {
        let mut open = true;
        let mut close = false;
        window(crate::tr("Edit Hyperlink").into(), "hyperlink_dialog", ctx, &mut open).open(&mut open).show(ctx, |ui| {
            egui::Grid::new("hl").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                ui.label(crate::tr("Text to display:"));
                ui.add_sized([360.0, 22.0], egui::TextEdit::singleline(&mut self.text));
                ui.end_row();
                ui.label(crate::tr("Address:"));
                ui.horizontal(|ui| {
                    ui.add_sized([300.0, 22.0], egui::TextEdit::singleline(&mut self.address).hint_text("https://… / C:\\…"));
                    if ui.button("📂").clicked()
                        && let Some(path) = rfd::FileDialog::new().pick_file()
                    {
                        self.address = path.display().to_string();
                        if self.text.is_empty() {
                            self.text = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        }
                    }
                });
                ui.end_row();
            });
            let mut remove = false;
            ui.horizontal(|ui| {
                if ui.button(crate::tr("Remove Link")).clicked() {
                    remove = true;
                }
            });
            let (ok, cancel) = ok_cancel(ui);
            if cancel {
                close = true;
            }
            if ok || remove {
                let id = self.task;
                let link = if remove || self.address.trim().is_empty() {
                    None
                } else {
                    Some(Hyperlink {
                        text: if self.text.trim().is_empty() { self.address.trim().to_string() } else { self.text.trim().to_string() },
                        address: self.address.trim().to_string(),
                    })
                };
                let _ = doc.edit_msg(crate::tr("Hyperlink"), |p| {
                    if let Some(t) = p.task_mut(id) {
                        t.hyperlink = link;
                    }
                    Ok::<(), String>(())
                });
                close = true;
            }
        });
        open && !close
    }
}

/// Opens a hyperlink address (web page or file) with the system's default application.
pub fn open_link(address: &str) -> std::io::Result<()> {
    let a = address.trim();
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("cmd").args(["/C", "start", "", a]).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(a).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(a).spawn();
    r.map(|_| ())
}

// ---------------------------------------------------------------------------
// Files embedded in notes ("Insert Object ▸ Create from file")
// ---------------------------------------------------------------------------

/// Folder where embedded files are written when opened.
fn attachment_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("plancraft-attachments").join(uuid::Uuid::new_v4().simple().to_string())
}

/// The list of files embedded in a note, with "Insert Object…", "Open", "Save As…" and "Remove".
/// Returns a status message to show (errors and confirmations).
pub fn attachments_ui(ui: &mut egui::Ui, list: &mut Vec<plancraft_core::attach::Attachment>, salt: &str) -> Option<String> {
    use plancraft_core::attach::{Attachment, MAX_ATTACHMENTS};
    let mut status = None;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(crate::tr("Embedded files:")).strong());
        if ui
            .add_enabled(list.len() < MAX_ATTACHMENTS, egui::Button::new(crate::tr("📎 Insert Object…")))
            .on_hover_text(crate::tr("Embed a document into the project file (Create from file)"))
            .clicked()
            && let Some(paths) = rfd::FileDialog::new().set_title(crate::tr("Insert Object")).pick_files()
        {
            for path in paths {
                if list.len() >= MAX_ATTACHMENTS {
                    break;
                }
                match Attachment::from_file(&path) {
                    Ok(a) => list.push(a),
                    Err(e) => status = Some(crate::tr_dyn(&e)),
                }
            }
        }
    });
    if list.is_empty() {
        ui.label(egui::RichText::new(crate::tr("No embedded files.")).color(crate::theme::pal().TEXT_WEAK));
        return status;
    }
    let mut remove = None;
    egui::Grid::new(("attachments", salt)).num_columns(4).spacing([10.0, 4.0]).striped(true).show(ui, |ui| {
        for (i, a) in list.iter().enumerate() {
            ui.label(format!("📄 {}", a.name));
            ui.label(egui::RichText::new(a.size_label()).color(crate::theme::pal().TEXT_WEAK));
            ui.horizontal(|ui| {
                if ui.button(crate::tr("Open")).clicked() {
                    status = Some(match a.extract_to(&attachment_dir()) {
                        Ok(path) => match open_link(&path.to_string_lossy()) {
                            Ok(()) => crate::tr_dyn(&format!("Opened {}", a.name)),
                            Err(e) => crate::tr_dyn(&format!("Could not open {}: {}", a.name, e)),
                        },
                        Err(e) => crate::tr_dyn(&format!("Could not open {}: {}", a.name, e)),
                    });
                }
                if ui.button(crate::tr("Save As…")).clicked()
                    && let Some(path) = rfd::FileDialog::new().set_file_name(&a.name).save_file()
                {
                    status = Some(match std::fs::write(&path, &a.data) {
                        Ok(()) => crate::tr_dyn(&format!("Saved {}", path.display())),
                        Err(e) => crate::tr_dyn(&format!("Could not save {}: {}", path.display(), e)),
                    });
                }
            });
            if ui.small_button("✖").on_hover_text(crate::tr("Remove from the project")).clicked() {
                remove = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = remove
        && i < list.len()
    {
        list.remove(i);
    }
    status
}

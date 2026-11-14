//! Modal-style windows.

pub mod baseline;
pub mod custom_fields;
pub mod extra;
pub mod mcp_panel;
pub mod misc;
pub mod print;
pub mod project_info;
pub mod recurring;
pub mod reports;
pub mod resource_info;
pub mod task_info;
pub mod update_project;
pub mod working_time;

/// Shared title and frame for the application's dialog windows.
fn window(ctx: &egui::Context, title: impl Into<String>) -> egui::Window<'static> {
    let frame = egui::Frame::window(&ctx.style_of(ctx.theme())).inner_margin(egui::Margin::symmetric(10, 4));
    egui::Window::new(egui::RichText::new(title.into()).size(16.0).strong()).frame(frame).collapsible(false)
}

fn tabs<T: Copy + PartialEq>(ui: &mut egui::Ui, current: &mut T, tabs: &[(T, &str)]) {
    let mut selected_rect = None;
    let row = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for &(tab, label) in tabs {
            let selected = *current == tab;
            let button = egui::Button::new(egui::RichText::new(label).color(if selected {
                crate::theme::pal().ACCENT_DARK
            } else {
                crate::theme::pal().TEXT
            }))
            .fill(if selected { crate::theme::pal().BG } else { crate::theme::pal().PANEL_BG })
            .stroke(egui::Stroke::new(1.0, crate::theme::pal().GRID_LINE_STRONG))
            .corner_radius(egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 });
            let response = ui.add_sized([0.0, 29.0], button);
            if selected {
                selected_rect = Some(response.rect);
            }
            if response.clicked() {
                *current = tab;
            }
        }
    });
    let y = row.response.rect.bottom();
    ui.painter().hline(ui.min_rect().left()..=ui.max_rect().right(), y, egui::Stroke::new(1.0, crate::theme::pal().GRID_LINE_STRONG));
    if let Some(rect) = selected_rect {
        ui.painter().hline(rect.left() + 1.0..=rect.right() - 1.0, y, egui::Stroke::new(2.0, crate::theme::pal().BG));
    }
    ui.add_space(4.0);
}

use plancraft_core::workspace::{Document, Workspace};

/// A dialog bound to the document it was opened for (by document handle). Task and resource ids
/// repeat across documents, so a dialog that applied to "the active document" would write one
/// plan's edits into another after a tab switch.
pub struct Bound<T> {
    pub doc: u64,
    pub dialog: T,
}

impl<T> Bound<T> {
    pub fn new(doc: &Document, dialog: T) -> Self {
        Self { doc: doc.handle, dialog }
    }
}

/// Shows a bound dialog against its own document; `show` returns whether it stays open. The
/// dialog closes when its document has been closed.
pub fn show_bound<T>(slot: &mut Option<Bound<T>>, ws: &mut Workspace, show: impl FnOnce(&mut T, &mut Document) -> bool) {
    let Some(b) = slot else { return };
    let keep = match ws.index_of(b.doc).and_then(|i| ws.docs.get_mut(i)) {
        Some(doc) => show(&mut b.dialog, doc),
        None => false,
    };
    if !keep {
        *slot = None;
    }
}

#[derive(Default)]
pub struct Dialogs {
    pub task_info: Option<Bound<task_info::TaskInfo>>,
    pub resource_info: Option<Bound<resource_info::ResourceInfo>>,
    pub project_info: Option<Bound<project_info::ProjectInfo>>,
    pub working_time: Option<Bound<working_time::WorkingTime>>,
    pub custom_fields: Option<Bound<custom_fields::CustomFields>>,
    pub print: Option<Bound<print::PrintDialog>>,
    pub update_project: Option<Bound<update_project::UpdateProject>>,
    pub set_baseline: Option<Bound<baseline::SetBaseline>>,
    pub clear_baseline: Option<Bound<baseline::ClearBaseline>>,
    pub recurring: Option<Bound<recurring::RecurringTask>>,
    /// Follows the active document's selection; reset when the active document changes.
    pub assign: Option<Bound<misc::AssignResources>>,
    pub external: Option<Bound<misc::ExternalLinks>>,
    pub baseline: Option<extra::BaselineDialog>,
    pub wbs: Option<extra::WbsDialog>,
    pub hyperlink: Option<extra::HyperlinkDialog>,
    pub reports: Option<reports::ReportsWindow>,
    pub templates: bool,
    pub stats: bool,
    pub about: bool,
    pub history: bool,
    pub mcp: bool,
}

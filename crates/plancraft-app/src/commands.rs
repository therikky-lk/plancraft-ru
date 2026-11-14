//! User commands issued from the menu bar, toolbar, menus, keyboard shortcuts and views.

use plancraft_core::model::ScheduleMode;

use crate::columns::TaskTable;
use crate::state::{TaskFilter, ViewKind};

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    // File
    NewProject,
    NewDemo,
    Open,
    OpenPath(std::path::PathBuf),
    Save,
    SaveAs,
    Close,
    Undo,
    Redo,
    ExportCsv,
    Import,
    ExportXml,
    Templates,
    SaveTemplate,
    NewFromTemplate(plancraft_core::templates::Template),
    NewFromTemplateFile(std::path::PathBuf),
    /// The Print window (preview, page setup, print, save as PDF).
    Print,
    /// Save the current view as a PDF with the plan's page setup.
    ExportPdf,
    /// Save the plan as a Microsoft Project XML (MSPDI) file.
    ExportMspdi,
    Quit,
    StartPage,
    /// Restore a recovery copy (by its stem in the recovery folder).
    RestoreRecovered(String),
    /// Delete a recovery copy.
    DiscardRecovered(String),
    // Tasks
    InsertTask,
    InsertMilestone,
    InsertSummary,
    /// The Recurring Task window, for a new recurring task.
    /// Split Task: the next click on a Gantt bar splits that task.
    SplitTask,
    /// Remove the splits planned in the selected tasks.
    RejoinTasks,
    DeleteSelection,
    Indent,
    Outdent,
    MoveUp,
    MoveDown,
    LinkSelected,
    UnlinkSelected,
    SetPercent(u8),
    MarkOnTrack,
    ToggleActive,
    SetMode(ScheduleMode),
    TaskInfo,
    TaskNotes,
    ToggleTimeline,
    ScrollToTask,
    Cut,
    Copy,
    Paste,
    RecurringTask,
    Hyperlink,
    OpenHyperlink,
    // Resources
    InsertResource,
    DeleteResources,
    ResourceInfo,
    AssignResources,
    LevelAll,
    ClearLeveling,
    // Project
    ProjectInfo,
    ChangeWorkingTime,
    BaselineDialog(bool),
    UpdateProject,
    DefineWbs,
    Reports,
    CustomReport,
    SortResources(plancraft_core::extras::SortKey),
    SetBaseline,
    ClearBaseline,
    UpdateProgressFromTime,
    ExternalLinks,
    RefreshExternal,
    Statistics,
    McpPanel,
    // View
    SetView(ViewKind),
    ZoomIn,
    ZoomOut,
    ZoomEntire,
    ZoomSelected,
    Filter(TaskFilter),
    Table(TaskTable),
    /// Sort task views by a field key (`true` = descending); `None` restores the outline order.
    SortBy(Option<(String, bool)>),
    /// Group task views by a field key; `None` ungroups.
    GroupBy(Option<String>),
    /// Insert a column showing field `key` before column `at` of the current table.
    InsertColumn {
        at: usize,
        key: String,
    },
    HideColumn(usize),
    MoveColumn {
        at: usize,
        right: bool,
    },
    /// Back to the current table's default columns.
    ResetColumns,
    CustomFields,
    CollapseAll,
    ExpandAll,
    History,
    SetTheme(crate::theme::ThemeChoice),
    SetLang(plancraft_core::i18n::Lang),
    SetFont(String),
    About,
}

/// Context menu shared by the task table and the Gantt chart.
pub fn task_context_menu(ui: &mut egui::Ui, emit: &mut dyn FnMut(Command)) {
    let mut item = |ui: &mut egui::Ui, label: &str, cmd: Command| {
        if ui.button(label).clicked() {
            emit(cmd);
            ui.close();
        }
    };
    item(ui, crate::tr("ℹ  Task Information…"), Command::TaskInfo);
    item(ui, crate::tr("📝  Notes…"), Command::TaskNotes);
    item(ui, crate::tr("🌐  Hyperlink…"), Command::Hyperlink);
    item(ui, crate::tr("🌐  Open hyperlink"), Command::OpenHyperlink);
    item(ui, crate::tr("👥  Assign Resources…"), Command::AssignResources);
    ui.separator();
    item(ui, crate::tr("➕  Insert Task"), Command::InsertTask);
    item(ui, crate::tr("♦  Insert Milestone"), Command::InsertMilestone);
    item(ui, crate::tr("↻  Insert Recurring Task…"), Command::RecurringTask);
    item(ui, crate::tr("🗑  Delete Task"), Command::DeleteSelection);
    ui.separator();
    item(ui, crate::tr("➡  Indent"), Command::Indent);
    item(ui, crate::tr("⬅  Outdent"), Command::Outdent);
    item(ui, crate::tr("🔗  Link Tasks"), Command::LinkSelected);
    item(ui, crate::tr("⛓  Unlink Tasks"), Command::UnlinkSelected);
    ui.separator();
    item(ui, crate::tr("✔  Mark 100% Complete"), Command::SetPercent(100));
    item(ui, crate::tr("📅  Add to Timeline"), Command::ToggleTimeline);
    item(ui, crate::tr("⏸  Inactivate / Activate"), Command::ToggleActive);
    item(ui, &format!("➗  {}", crate::tr("Split Task")), Command::SplitTask);
    item(ui, &format!("⟲  {}", crate::tr("Rejoin Split Task")), Command::RejoinTasks);
    ui.separator();
    item(ui, crate::tr("✂  Cut"), Command::Cut);
    item(ui, crate::tr("📋  Copy"), Command::Copy);
    item(ui, crate::tr("📥  Paste"), Command::Paste);
}

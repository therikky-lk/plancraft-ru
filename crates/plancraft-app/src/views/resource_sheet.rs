//! Resource Sheet: an editable table of all resources.

use std::collections::{HashMap, HashSet};

use egui::{Rect, Ui};
use plancraft_core::prelude::*;
use plancraft_core::workspace::Document;

use crate::commands::Command;
use crate::edit::EditExt;
use crate::format;
use crate::state::{ResourceGroup, ToastKind, UiState};
use crate::theme;
use crate::widgets::grid::{self, Cell, ColumnDef, GridConfig, GridEvent};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Col {
    Indicators,
    Name,
    Type,
    Category,
    Initials,
    Group,
    MaxUnits,
    StdRate,
    OvtRate,
    CostPerUse,
    AccrueAt,
    Calendar,
    Email,
    Work,
    Cost,
}

const COLS: [Col; 15] = [
    Col::Indicators,
    Col::Name,
    Col::Type,
    Col::Category,
    Col::Initials,
    Col::Group,
    Col::MaxUnits,
    Col::StdRate,
    Col::OvtRate,
    Col::CostPerUse,
    Col::AccrueAt,
    Col::Calendar,
    Col::Email,
    Col::Work,
    Col::Cost,
];

fn type_label(t: ResourceType) -> &'static str {
    match t {
        ResourceType::Work => crate::format::work_type_label(),
        ResourceType::Material => crate::tr("Material"),
        ResourceType::Cost => crate::tr("Cost"),
    }
}

fn defs(p: &Project) -> Vec<ColumnDef> {
    let cal_names: Vec<String> = p.calendars.iter().map(|c| c.name.clone()).collect();
    let cats: Vec<&str> = ResourceCategory::ALL.iter().map(|c| c.label()).collect();
    COLS.iter()
        .map(|c| match c {
            Col::Indicators => ColumnDef::new("ℹ", 30.0).read_only().center(),
            Col::Name => ColumnDef::new(crate::tr("Resource Name"), 200.0),
            Col::Type => ColumnDef::new(crate::tr("Type"), 74.0).choice(&[
                crate::format::work_type_label(),
                crate::tr("Material"),
                crate::tr("Cost"),
            ]),
            Col::Category => ColumnDef::new(crate::tr("Category"), 120.0).choice(&cats),
            Col::Initials => ColumnDef::new(crate::tr("Initials"), 60.0),
            Col::Group => ColumnDef::new(crate::tr("Group"), 110.0),
            Col::MaxUnits => ColumnDef::new(crate::tr("Max."), 62.0).right(),
            Col::StdRate => ColumnDef::new(crate::tr("Std. Rate"), 86.0).right(),
            Col::OvtRate => ColumnDef::new(crate::tr("Ovt. Rate"), 86.0).right(),
            Col::CostPerUse => ColumnDef::new(crate::tr("Cost/Use"), 84.0).right(),
            Col::AccrueAt => ColumnDef::new(crate::tr("Accrue At"), 110.0).choice(&Accrual::ALL.map(|a| a.label())),
            Col::Calendar => {
                let mut d = ColumnDef::new(crate::tr("Base Calendar"), 110.0);
                d.editor =
                    grid::Editor::Choice(std::iter::once(crate::tr("(Project)").to_string()).chain(cal_names.iter().cloned()).collect());
                d
            }
            Col::Email => ColumnDef::new(crate::tr("Email"), 170.0),
            Col::Work => ColumnDef::new(crate::tr("Work"), 76.0).right().read_only(),
            Col::Cost => ColumnDef::new(crate::tr("Cost"), 96.0).right().read_only(),
        })
        .collect()
}

fn cell(p: &Project, r: &Resource, col: Col, over: &HashSet<ResourceId>, work: &HashMap<ResourceId, (Minutes, f64)>) -> Cell {
    let red = |mut c: Cell| {
        if over.contains(&r.id) {
            c.color = Some(theme::pal().ERROR);
        }
        c
    };
    match col {
        Col::Indicators => {
            let mut icons = Vec::new();
            if over.contains(&r.id) {
                icons.push((
                    "⚠".to_string(),
                    theme::pal().ERROR,
                    crate::tr("This resource is overallocated and should be leveled.").to_string(),
                ));
            }
            if !r.exceptions.is_empty() {
                icons.push((
                    "📅".to_string(),
                    theme::pal().ACCENT_MID,
                    crate::tr_dyn(&format!("{} calendar exception(s): time off or altered hours", r.exceptions.len())),
                ));
            }
            if !r.notes.is_empty() || !r.attachments.is_empty() {
                icons.push(("📝".to_string(), theme::pal().TEXT_WEAK, crate::columns::note_tooltip(&r.notes, &r.attachments)));
            }
            icons.truncate(2);
            Cell { icons, ..Default::default() }
        }
        Col::Name => red(Cell::text(r.name.clone())),
        Col::Type => Cell::text(type_label(r.kind)),
        Col::Category => Cell::text(r.category.label()),
        Col::Initials => Cell::text(r.initials.clone()),
        Col::Group => Cell::text(r.group.clone()),
        Col::MaxUnits => {
            if r.kind == ResourceType::Work {
                Cell::text(format::units(r.max_units))
            } else {
                Cell::default()
            }
        }
        Col::StdRate => {
            let unit = match r.kind {
                ResourceType::Work => crate::tr("/hr").to_string(),
                ResourceType::Material => {
                    if r.material_label.is_empty() {
                        String::new()
                    } else {
                        format!("/{}", r.material_label)
                    }
                }
                ResourceType::Cost => return Cell::default(),
            };
            Cell {
                text: format!("{}{unit}", format::money(p, r.standard_rate)),
                edit_text: Some(r.standard_rate.to_string()),
                ..Default::default()
            }
        }
        Col::OvtRate => {
            if r.kind == ResourceType::Work {
                Cell {
                    text: crate::tr_dyn(&format!("{}/hr", format::money(p, r.overtime_rate))),
                    edit_text: Some(r.overtime_rate.to_string()),
                    ..Default::default()
                }
            } else {
                Cell::default()
            }
        }
        Col::AccrueAt => Cell::text(r.accrue_at.label()),
        Col::CostPerUse => {
            Cell { text: format::money(p, r.cost_per_use), edit_text: Some(r.cost_per_use.to_string()), ..Default::default() }
        }
        Col::Calendar => {
            Cell::text(r.calendar.and_then(|c| p.calendar(c)).map(|c| c.name.clone()).unwrap_or_else(|| crate::tr("(Project)").into()))
        }
        Col::Email => Cell::text(r.email.clone()),
        Col::Work => Cell::text(work.get(&r.id).map(|w| format_work(w.0)).unwrap_or_else(|| format_work(0))),
        Col::Cost => Cell::text(format::money(p, work.get(&r.id).map(|w| w.1).unwrap_or(0.0))),
    }
}

fn commit(p: &mut Project, id: ResourceId, col: Col, text: &str) -> Result<(), String> {
    let cal_id = p.calendars.iter().find(|c| c.name == text).map(|c| c.id);
    let r = p.resource_mut(id).ok_or(crate::tr("resource not found"))?;
    match col {
        Col::Name => {
            if text.is_empty() {
                return Err(crate::tr("resource name cannot be empty").into());
            }
            r.name = text.into();
        }
        Col::Type => {
            r.kind = match crate::untr(text) {
                "Material" | "Material resource" => ResourceType::Material,
                "Cost" => ResourceType::Cost,
                _ => ResourceType::Work,
            }
        }
        Col::Category => r.category = ResourceCategory::ALL.into_iter().find(|c| c.label() == text).unwrap_or_default(),
        Col::Initials => r.initials = text.into(),
        Col::Group => r.group = text.into(),
        Col::MaxUnits => r.max_units = format::parse_units(text).ok_or(crate::tr("enter units like 100% or 50%"))?.max(0.0),
        Col::StdRate => r.standard_rate = format::parse_money(text).ok_or(crate::tr("invalid rate"))?,
        Col::OvtRate => r.overtime_rate = format::parse_money(text).ok_or(crate::tr("invalid rate"))?,
        Col::CostPerUse => r.cost_per_use = format::parse_money(text).ok_or(crate::tr("invalid amount"))?,
        Col::AccrueAt => {
            r.accrue_at = match crate::untr(text) {
                "Start of task" => Accrual::Start,
                "End of task" => Accrual::End,
                _ => Accrual::Prorated,
            }
        }
        Col::Calendar => r.calendar = cal_id,
        Col::Email => r.email = text.into(),
        _ => return Err(crate::tr("this field is calculated").into()),
    }
    Ok(())
}

/// A group header of the grouped Resource Sheet.
struct ResHeader {
    label: String,
    count: usize,
    work: Minutes,
    cost: f64,
}

const RES_GROUP_BASE: usize = usize::MAX / 2;

/// Display rows (resource indices, or `RES_GROUP_BASE + header`) for "Group by".
fn resource_rows(p: &Project, st: &UiState, work: &HashMap<ResourceId, (Minutes, f64)>) -> (Vec<usize>, Vec<ResHeader>) {
    if st.resource_group == ResourceGroup::None {
        return ((0..p.resources.len()).collect(), Vec::new());
    }
    let field = |name: &str, value: String| format!("{}: {}", crate::tr_dyn(name), value);
    let mut groups: std::collections::BTreeMap<(i64, String), Vec<usize>> = std::collections::BTreeMap::new();
    for (i, r) in p.resources.iter().enumerate() {
        let key = match st.resource_group {
            ResourceGroup::None => (0, String::new()),
            ResourceGroup::Group => {
                if r.group.trim().is_empty() {
                    (0, field("Group", crate::tr("(none)").to_string()))
                } else {
                    (1, field("Group", r.group.trim().to_string()))
                }
            }
            ResourceGroup::Type => {
                let o = match r.kind {
                    ResourceType::Work => 0,
                    ResourceType::Material => 1,
                    ResourceType::Cost => 2,
                };
                (o, field("Type", type_label(r.kind).to_string()))
            }
            ResourceGroup::StandardRate => {
                ((r.standard_rate * 100.0).round() as i64, field("Std. Rate", format::money(p, r.standard_rate)))
            }
            ResourceGroup::Category => {
                let o = ResourceCategory::ALL.iter().position(|c| *c == r.category).unwrap_or(0) as i64;
                (o, field("Category", r.category.label().to_string()))
            }
        };
        groups.entry(key).or_default().push(i);
    }
    let mut rows = Vec::new();
    let mut headers = Vec::new();
    for ((_, label), members) in groups {
        let mut h = ResHeader { label: label.clone(), count: members.len(), work: 0, cost: 0.0 };
        for &i in &members {
            if let Some((w, c)) = p.resources.get(i).and_then(|r| work.get(&r.id)) {
                h.work += *w;
                h.cost += *c;
            }
        }
        rows.push(RES_GROUP_BASE + headers.len());
        headers.push(h);
        if !st.collapsed_groups.contains(&label) {
            rows.extend(members);
        }
    }
    (rows, headers)
}

fn header_cell(p: &Project, h: &ResHeader, col: Col, expanded: bool) -> Cell {
    let text = match col {
        Col::Name => format!("{}  ({})", h.label, h.count),
        Col::Work => format_work(h.work),
        Col::Cost => format::money(p, h.cost),
        _ => String::new(),
    };
    Cell { text, bold: true, bg: Some(theme::pal().HEADER_BG), expander: (col == Col::Name).then_some(expanded), ..Default::default() }
}

/// Work and cost per resource over all its assignments.
fn totals(p: &Project) -> HashMap<ResourceId, (Minutes, f64)> {
    let mut work: HashMap<ResourceId, (Minutes, f64)> = HashMap::new();
    for a in &p.assignments {
        let Some(r) = p.resource(a.resource) else {
            continue;
        };
        let e = work.entry(a.resource).or_default();
        e.0 = e.0.saturating_add(a.work);
        e.1 += match r.kind {
            ResourceType::Work => a.work as f64 / 60.0 * r.standard_rate + r.cost_per_use,
            ResourceType::Material => a.units * r.standard_rate + r.cost_per_use,
            ResourceType::Cost => a.cost,
        };
    }
    work
}

/// The Resource Sheet for printing (without the indicator column; overallocated resources in red).
pub fn print_sheet(p: &Project, pal: &plancraft_core::print::layout::Palette) -> plancraft_core::print::Sheet {
    use plancraft_core::print::{Align, PrintCell, PrintColumn, PrintRow, RowKind, Sheet};
    let defs = defs(p);
    let over: HashSet<ResourceId> = plancraft_core::workload::overallocations(p).into_iter().map(|(r, _)| r).collect();
    let work = totals(p);
    let cols: Vec<(Col, &ColumnDef)> = COLS.iter().copied().zip(&defs).filter(|(c, _)| *c != Col::Indicators).collect();
    let mut columns = vec![PrintColumn { title: "ID".into(), width: 26.0, align: Align::Right }];
    columns.extend(cols.iter().map(|(_, d)| PrintColumn {
        title: d.title.clone(),
        width: d.width * 0.75,
        align: if d.align == egui::Align::Max { Align::Right } else { Align::Left },
    }));
    let rows = p
        .resources
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut cells = vec![PrintCell::text((i + 1).to_string())];
            cells.extend(cols.iter().map(|(c, _)| {
                let cell = cell(p, r, *c, &over, &work);
                PrintCell { text: cell.text, color: cell.color.map(|_| pal.error), ..Default::default() }
            }));
            PrintRow { kind: RowKind::Other, cells }
        })
        .collect();
    Sheet { title: "Resource Sheet".into(), columns, rows }
}

pub fn show(ui: &mut Ui, rect: Rect, doc: &mut Document, st: &mut UiState, out: &mut Vec<Command>) {
    let defs = defs(&doc.project);
    let over: HashSet<ResourceId> = plancraft_core::workload::overallocations(&doc.project).into_iter().map(|(r, _)| r).collect();
    let work = totals(&doc.project);
    let (rows, headers) = resource_rows(&doc.project, st, &work);
    let header_of = |row: usize| rows.get(row).and_then(|&i| i.checked_sub(RES_GROUP_BASE)).and_then(|h| headers.get(h));
    let row_label = |row: usize| match rows.get(row) {
        Some(&i) if i < RES_GROUP_BASE => (i + 1).to_string(),
        _ => String::new(),
    };
    let cfg = GridConfig {
        id: egui::Id::new("resource_sheet"),
        columns: &defs,
        row_count: rows.len(),
        blank_rows: 20,
        row_numbers: true,
        show_vscroll: true,
        header_height: 30.0,
        row_height: theme::ROW_HEIGHT,
        row_label: Some(&row_label),
        spreadsheet: false,
    };
    let output = {
        let p = &doc.project;
        let sel = &st.sel;
        let collapsed = &st.collapsed_groups;
        let is_sel = |r: usize| rows.get(r).and_then(|&i| p.resources.get(i)).is_some_and(|res| sel.resources.contains(&res.id));
        let mut cellf = |r: usize, c: usize| {
            if let (Some(h), Some(&col)) = (header_of(r), COLS.get(c)) {
                return header_cell(p, h, col, !collapsed.contains(&h.label));
            }
            match (rows.get(r).and_then(|&i| p.resources.get(i)), COLS.get(c)) {
                (Some(res), Some(&col)) => cell(p, res, col, &over, &work),
                _ => Cell::default(),
            }
        };
        grid::show(ui, rect, &mut st.resource_grid, &cfg, &is_sel, &mut cellf)
    };
    let n = rows.len();
    let resource_at = |doc: &Document, row: usize| rows.get(row).and_then(|&i| doc.project.resources.get(i)).map(|r| r.id);
    for ev in output.events {
        match ev {
            GridEvent::Select { row, ctrl, .. } => {
                let Some(id) = resource_at(doc, row) else { continue };
                if ctrl {
                    if !st.sel.resources.remove(&id) {
                        st.sel.resources.insert(id);
                    }
                } else {
                    st.sel.select_resource(id);
                }
            }
            GridEvent::CursorRow { row, .. } => {
                if let Some(id) = resource_at(doc, row) {
                    st.sel.select_resource(id);
                }
            }
            GridEvent::Activate { row, .. } => {
                if let Some(id) = resource_at(doc, row) {
                    st.sel.select_resource(id);
                    out.push(Command::ResourceInfo);
                }
            }
            GridEvent::Commit { row, col, text } => {
                let (Some(&column), Some(def)) = (COLS.get(col), defs.get(col)) else { continue };
                if row < n {
                    let Some(id) = resource_at(doc, row) else { continue };
                    if let Err(e) = doc.edit_msg(&crate::tr_dyn(&format!("Edit {}", def.title)), |p| commit(p, id, column, &text)) {
                        st.toast(ToastKind::Error, e);
                    }
                } else if !text.is_empty() {
                    let r = doc.edit_msg(crate::tr("New Resource"), |p| {
                        let name =
                            if column == Col::Name { text.clone() } else { crate::tr_dyn(&format!("Resource {}", p.resources.len() + 1)) };
                        let id = p.add_resource(name);
                        if column != Col::Name {
                            commit(p, id, column, &text)?;
                        }
                        Ok::<_, String>(id)
                    });
                    match r {
                        Ok(id) => st.sel.select_resource(id),
                        Err(e) => st.toast(ToastKind::Error, e),
                    }
                }
            }
            GridEvent::DeleteKey => out.push(Command::DeleteResources),
            GridEvent::InsertKey => out.push(Command::InsertResource),
            GridEvent::ContextMenu { row, .. } => {
                if let Some(id) = resource_at(doc, row) {
                    st.sel.select_resource(id);
                }
            }
            GridEvent::ClickEmpty { .. } => st.sel.resources.clear(),
            GridEvent::HeaderClick { .. } | GridEvent::HeaderMenu { .. } | GridEvent::PasteCells { .. } => {}
            GridEvent::ToggleExpand(row) => {
                if let Some(h) = header_of(row) {
                    let l = h.label.clone();
                    if !st.collapsed_groups.remove(&l) {
                        st.collapsed_groups.insert(l);
                    }
                }
            }
        }
    }
}

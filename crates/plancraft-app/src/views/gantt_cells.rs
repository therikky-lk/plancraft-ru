//! Atomic rectangular task-cell paste. Display rows are resolved before any edits.

use plancraft_core::workspace::Document;

use crate::columns::TaskCol;
use crate::edit::EditExt;
use crate::fields::Field;
use crate::state::{ToastKind, UiState};
use crate::widgets::{grid::GridSelection, tsv};

pub(super) fn paste(doc: &mut Document, st: &mut UiState, rows: &[super::Row], cols: &[Field], row: usize, col: usize, text: &str) {
    let result = paste_inner(doc, st, rows, cols, row, col, text);
    match result {
        Ok((updated, skipped, height, width)) => {
            if updated > 0 {
                st.task_grid.selection = Some(GridSelection::Cells {
                    anchor: (row, col),
                    focus: (
                        row.saturating_add(height.saturating_sub(1)).min(rows.len().saturating_sub(1)),
                        col.saturating_add(width.saturating_sub(1)).min(cols.len().saturating_sub(1)),
                    ),
                });
                st.task_grid.cursor = Some((row, col));
            }
            st.toast(ToastKind::Info, crate::tr_dyn(&format!("Pasted {updated} cells; skipped {skipped} protected or out-of-range cells")));
        }
        Err(error) => st.toast(ToastKind::Error, error),
    }
}

fn paste_inner(
    doc: &mut Document,
    st: &UiState,
    rows: &[super::Row],
    cols: &[Field],
    row: usize,
    col: usize,
    text: &str,
) -> Result<(usize, usize, usize, usize), String> {
    let data = tsv::decode(text)?;
    let source_h = data.len();
    let source_w = data.iter().map(Vec::len).max().unwrap_or(0);
    if source_h == 0 || source_w == 0 {
        return Ok((0, 0, 0, 0));
    }
    let (mut height, mut width) = (source_h, source_w);
    if let Some((r0, c0, r1, c1)) = st.task_grid.cell_bounds() {
        let selected_h = r1.saturating_sub(r0).saturating_add(1);
        let selected_w = c1.saturating_sub(c0).saturating_add(1);
        if selected_h >= source_h && selected_w >= source_w && selected_h % source_h == 0 && selected_w % source_w == 0 {
            height = selected_h;
            width = selected_w;
        }
    }
    if height.saturating_mul(width) > 1_000_000 {
        return Err(crate::tr("Clipboard data is too large").into());
    }
    let mut targets = Vec::new();
    let mut skipped = 0usize;
    for dr in 0..height {
        for dc in 0..width {
            let target_row = row.saturating_add(dr);
            let target_col = col.saturating_add(dc);
            let value = data.get(dr % source_h).and_then(|r| r.get(dc % source_w));
            let task = super::task_at(&doc.project, rows, target_row);
            if let (Some(value), Some(task), Some(&column)) = (value, task, cols.get(target_col))
                && column.can_edit(&doc.project, task.id)
            {
                targets.push((task.id, column, value.clone(), target_row, target_col));
            } else {
                skipped = skipped.saturating_add(1);
            }
        }
    }
    if targets.is_empty() {
        return Ok((0, skipped, height, width));
    }
    // Any invalid editable value rolls the whole batch back. Protected cells are
    // skipped at their original coordinates, so later columns never slide left.
    let updated = doc.edit_msg(crate::tr("Paste Cells"), |project| {
        let mut updated = 0usize;
        for (id, column, value, r, c) in targets {
            if !column.can_edit(project, id) {
                skipped = skipped.saturating_add(1);
                continue;
            }
            if column == Field::Builtin(TaskCol::Notes) {
                if let Some(task) = project.task_mut(id) {
                    task.notes = value;
                }
                updated = updated.saturating_add(1);
                continue;
            }
            column
                .commit(project, id, &value)
                .map_err(|error| crate::tr_dyn(&format!("Row {}, column {}: {}", r.saturating_add(1), c.saturating_add(1), error)))?;
            updated = updated.saturating_add(1);
        }
        Ok(updated)
    })?;
    Ok((updated, skipped, height, width))
}

#[cfg(test)]
mod tests {
    use super::*;
    use plancraft_core::prelude::*;
    use plancraft_core::workspace::Workspace;

    #[test]
    fn text_editor_round_trip_preserves_quoted_notes_and_only_manual_dates_change() {
        let mut p = Project::new("Clipboard", chrono::NaiveDate::from_ymd_opt(2026, 10, 19).unwrap());
        let auto = p.add_task("Auto", None);
        let manual = p.add_task("Manual", None);
        let task = p.task_mut(manual).unwrap();
        task.mode = ScheduleMode::Manual;
        let mut ws = Workspace::default();
        ws.add(p, None);
        let doc = ws.active_doc_mut().unwrap();
        let mut st = UiState::default();
        st.task_grid.select_cell(0, 0, false);
        let notes = "first line\nsecond\tline with \"quotes\"";
        let text = tsv::encode(&[
            vec!["Auto edited".into(), "2026-11-02".into(), "2026-11-03".into(), notes.into()],
            vec!["Manual edited".into(), "2026-11-02".into(), "2026-11-03".into(), notes.into()],
        ]);
        let original_constraint = doc.project.task(auto).unwrap().constraint;
        let result = paste_inner(
            doc,
            &st,
            &[crate::views::Row::Task(0), crate::views::Row::Task(1)],
            &[
                Field::Builtin(TaskCol::Name),
                Field::Builtin(TaskCol::Start),
                Field::Builtin(TaskCol::Finish),
                Field::Builtin(TaskCol::Notes),
            ],
            0,
            0,
            &text,
        )
        .unwrap();
        assert_eq!((result.0, result.1), (6, 2));
        assert_eq!(doc.project.tasks.len(), 2);
        assert_eq!(doc.project.task(auto).unwrap().constraint, original_constraint);
        assert!(doc.project.task(auto).unwrap().manual_start.is_none());
        assert_eq!(doc.project.task(manual).unwrap().manual_start.unwrap().date(), chrono::NaiveDate::from_ymd_opt(2026, 11, 2).unwrap());
        assert_eq!(doc.project.task(auto).unwrap().notes, notes);
        assert_eq!(doc.project.task(manual).unwrap().notes, notes);
        assert!(doc.undo());
        assert_eq!(doc.project.task(auto).unwrap().name, "Auto");
        assert_eq!(doc.project.task(manual).unwrap().name, "Manual");
    }

    #[test]
    fn scalar_fill_and_protected_only_pastes_do_not_add_tasks_or_empty_undo_steps() {
        let mut p = Project::new("Fill", chrono::NaiveDate::from_ymd_opt(2026, 10, 19).unwrap());
        p.add_task("First", None);
        p.add_task("Second", None);
        let mut ws = Workspace::default();
        ws.add(p, None);
        let doc = ws.active_doc_mut().unwrap();
        let mut st = UiState::default();
        st.task_grid.select_cell(0, 0, false);
        st.task_grid.select_cell(1, 0, true);
        let protected = paste_inner(
            doc,
            &st,
            &[crate::views::Row::Task(0), crate::views::Row::Task(1)],
            &[Field::Builtin(TaskCol::Start)],
            0,
            0,
            "not even a date",
        )
        .unwrap();
        assert_eq!((protected.0, protected.1), (0, 2));
        assert!(!doc.can_undo());
        let fill = paste_inner(
            doc,
            &st,
            &[crate::views::Row::Task(0), crate::views::Row::Task(1)],
            &[Field::Builtin(TaskCol::Name)],
            0,
            0,
            "Bulk",
        )
        .unwrap();
        assert_eq!(fill.0, 2);
        assert!(doc.project.tasks.iter().all(|t| t.name == "Bulk"));
        assert_eq!(doc.project.tasks.len(), 2);
        assert!(doc.undo());
        assert!(!doc.can_undo());
    }
}

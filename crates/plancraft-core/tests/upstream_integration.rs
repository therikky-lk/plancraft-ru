#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

//! Compatibility between the localized edition's existing data and upstream v0.4.1 features.
use chrono::NaiveDate;
use plancraft_core::prelude::*;
use plancraft_core::workspace::Workspace;
use plancraft_core::{extras, io, recurring};
use std::path::Path;

fn monday() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 19).unwrap()
}

#[test]
fn existing_edition_data_survives_new_features_save_and_undo() {
    let mut p = Project::new("日本語の既存計画", monday());
    let id = p.add_task("既存タスク", None);
    p.set_duration(id, 4 * 480, false).unwrap();
    p.settings.currency = "¥".into();
    p.settings.currency_decimals = 0;
    p.settings.wbs.prefix = "JP-".into();
    p.task_mut(id).unwrap().notes = "複数行のメモ\nsecond line".into();
    p.task_mut(id).unwrap().attachments.push(plancraft_core::attach::Attachment { name: "memo.txt".into(), data: b"keep me".to_vec() });
    p.task_mut(id).unwrap().hyperlink = Some(Hyperlink { text: "仕様書".into(), address: "https://example.com/spec".into() });
    p.task_mut(id).unwrap().fixed_cost_accrual = Accrual::End;
    let series = p
        .insert_recurring(
            p.tasks.len(),
            "既存の定例会",
            60,
            Recurrence { kind: RecurrenceKind::Weekly, every: 1, weekdays: vec![0], day_of_month: 1, occurrences: 3 },
            monday(),
            None,
        )
        .unwrap();
    recalculate(&mut p);
    p.save_plan(extras::PlanSlot::Baseline(2), &[]).unwrap();
    p.save_plan(extras::PlanSlot::Interim(1), &[]).unwrap();
    let before = p.task(id).unwrap().clone();
    let legacy_definition = p.task(series).unwrap().recurrence.clone();
    let json = io::to_json(&p).unwrap();
    let p = io::from_json(&json, Path::new("legacy.plancraft")).unwrap();
    let mut ws = Workspace::default();
    ws.add(p, None);
    let doc = ws.active_doc_mut().unwrap();
    doc.try_edit("Add upstream features", Default::default(), |p| {
        let field = p.add_custom_field("Risk", CustomFieldKind::Text, &[])?;
        p.set_custom_value(id, field, Some(CustomValue::Text("High".into())))?;
        p.set_baseline_in(3, &Scope::entire_project())?;
        let start = p.task(id).unwrap().sched.start.unwrap();
        let after = plancraft_core::calendar::shift(start, 2 * 1440);
        p.split_task(id, plancraft_core::calendar::shift(start, 480), after)?;
        Ok::<(), PlanError>(())
    })
    .unwrap();
    assert!(!doc.project.task(id).unwrap().gaps.is_empty());
    let saved = io::to_json(&doc.project).unwrap();
    let loaded = io::from_json(&saved, Path::new("merged.plancraft")).unwrap();
    let t = loaded.task(id).unwrap();
    assert_eq!(
        (&t.notes, &t.attachments, &t.hyperlink, t.fixed_cost_accrual),
        (&before.notes, &before.attachments, &before.hyperlink, before.fixed_cost_accrual)
    );
    assert_eq!(t.baselines.get(&2), before.baselines.get(&2));
    assert_eq!(t.interim, before.interim);
    assert_eq!(loaded.task(series).unwrap().recurrence, legacy_definition);
    assert_eq!((loaded.settings.currency.as_str(), loaded.settings.currency_decimals, loaded.wbs(0)), ("¥", 0, "JP-1".into()));
    assert!(doc.undo());
    assert!(doc.project.custom_fields.is_empty());
    assert_eq!(doc.project.task(id).unwrap().attachments, before.attachments);
    assert!(doc.redo());
    assert_eq!(doc.project.custom_fields.len(), 1);
}

#[test]
fn upstream_recurrence_wire_format_loads_alongside_the_legacy_definition() {
    let mut p = Project::new("Upstream", monday());
    let rec = recurring::Recurrence {
        pattern: recurring::Pattern::Weekly { interval: 1, days: 1 },
        start: monday(),
        end: recurring::RecurEnd::After(3),
        duration: 60,
        calendar: None,
        ignore_resource_calendars: false,
    };
    let id = p.add_recurring_task("Weekly", rec, None).unwrap();
    let mut value = serde_json::to_value(&p).unwrap();
    for task in value.get_mut("tasks").unwrap().as_array_mut().unwrap() {
        if let Some(pattern) = task.as_object_mut().unwrap().remove("recurrence_pattern") {
            task.as_object_mut().unwrap().insert("recurrence".into(), pattern);
        }
    }
    let p = io::from_json(&value.to_string(), Path::new("upstream.plancraft")).unwrap();
    assert_eq!(p.task(id).unwrap().recurrence_pattern, Some(rec));
    assert!(p.task(id).unwrap().recurrence.is_none());
    assert_eq!(p.tasks.len(), 4);
}

#[test]
fn legacy_resume_is_shared_by_new_tracking_workload_and_printing() {
    let mut p = Project::new("Pause", monday());
    let id = p.add_task("Work", None);
    p.set_duration(id, 4 * 480, false).unwrap();
    recalculate(&mut p);
    p.set_percent_complete(id, 25).unwrap();
    let resume = monday().and_hms_opt(8, 0, 0).unwrap().checked_add_signed(chrono::Duration::days(7)).unwrap();
    p.task_mut(id).unwrap().resume = Some(resume);
    recalculate(&mut p);
    let t = p.task(id).unwrap();
    assert!(t.resume.is_none());
    assert_eq!(t.splits.len(), 1);
    assert_eq!(t.sched.resume, Some(resume));
    assert_eq!(t.sched.parts.len(), 2);
    assert_eq!(extras::split_points(&p, 0), t.sched.stop.zip(t.sched.resume));
    p.set_resume(id, None).unwrap();
    recalculate(&mut p);
    assert!(p.task(id).unwrap().sched.parts.is_empty());
}

#[test]
fn existing_earned_value_table_follows_the_new_baseline_selection() {
    let mut p = Project::new("Earned value", monday());
    let id = p.add_task("Work", None);
    p.task_mut(id).unwrap().fixed_cost = 100.0;
    recalculate(&mut p);
    p.set_baseline_in(0, &Scope::entire_project()).unwrap();
    p.task_mut(id).unwrap().fixed_cost = 200.0;
    recalculate(&mut p);
    p.set_baseline_in(2, &Scope::entire_project()).unwrap();
    p.set_percent_complete(id, 50).unwrap();
    p.set_earned_value_baseline(2).unwrap();
    recalculate(&mut p);
    let task = extras::task_earned_value(&p, 0).unwrap();
    let project = plancraft_core::report::earned_value(&p, plancraft_core::report::status_moment(&p)).unwrap();
    assert_eq!((task.baseline, task.bac, task.bcwp), (2, 200.0, 100.0));
    assert_eq!((task.bcwp, task.acwp), (project.bcwp, project.acwp));
}

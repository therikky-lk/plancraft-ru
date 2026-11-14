//! Task table columns: how each field is displayed and edited.

use chrono::NaiveDateTime;
use plancraft_core::prelude::*;
use plancraft_core::report;

use crate::format;
use crate::theme;
use crate::widgets::grid::{Cell, ColumnDef};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TaskCol {
    Indicators,
    Mode,
    Wbs,
    Name,
    Duration,
    Start,
    Finish,
    Predecessors,
    Successors,
    Resources,
    PercentComplete,
    Work,
    ActualWork,
    RemainingWork,
    Cost,
    FixedCost,
    /// A value of Baseline (0) or Baseline1–10.
    Baseline(u8, BlField),
    /// Interim plan 1–10: its start (`false`) or finish (`true`), the Start*n* and Finish*n* fields.
    Interim(u8, bool),
    StartVariance,
    FinishVariance,
    WorkVariance,
    CostVariance,
    ActualStart,
    ActualFinish,
    ActualDuration,
    RemainingDuration,
    Stop,
    Resume,
    EarlyStart,
    EarlyFinish,
    LateStart,
    LateFinish,
    FreeSlack,
    TotalSlack,
    ConstraintType,
    ConstraintDate,
    Deadline,
    Priority,
    TaskType,
    EstimateMethod,
    Points,
    Optimistic,
    MostLikely,
    Pessimistic,
    ExpectedWork,
    Deliverable,
    Milestones,
    Notes,
    UniqueId,
    FixedCostAccrual,
    ActualCost,
    RemainingCost,
    Bcws,
    Bcwp,
    Acwp,
    Sv,
    Cv,
    Eac,
    Bac,
    Vac,
    OutlineNumber,
    /// Yes for a recurring task and its occurrences.
    Recurring,
    /// The Task Board column a task's card is in.
    BoardStatus,
    /// Whether the task has a card on the Task Board.
    ShowOnBoard,
}

/// The values a baseline holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlField {
    Start,
    Finish,
    Duration,
    Work,
    Cost,
}

impl BlField {
    pub const ALL: [BlField; 5] = [BlField::Start, BlField::Finish, BlField::Duration, BlField::Work, BlField::Cost];

    pub fn label(self) -> &'static str {
        match self {
            BlField::Start => crate::tr("Start"),
            BlField::Finish => crate::tr("Finish"),
            BlField::Duration => crate::tr("Duration"),
            BlField::Work => crate::tr("Work"),
            BlField::Cost => crate::tr("Cost"),
        }
    }

    /// The key suffix in table layouts ("start", "finish", ...).
    pub fn key(self) -> &'static str {
        match self {
            BlField::Start => "start",
            BlField::Finish => "finish",
            BlField::Duration => "duration",
            BlField::Work => "work",
            BlField::Cost => "cost",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TaskTable {
    Entry,
    Schedule,
    Tracking,
    Work,
    Cost,
    Estimates,
    Variance,
    Summary,
    Details,
    EarnedValue,
}

impl TaskTable {
    /// Stable storage key; table layouts must survive switching the UI language.
    pub fn key(self) -> &'static str {
        match self {
            Self::Entry => "Entry",
            Self::Schedule => "Schedule",
            Self::Tracking => "Tracking",
            Self::Work => "Work",
            Self::Cost => "Cost",
            Self::Estimates => "Estimates",
            Self::Variance => "Variance",
            Self::Summary => "Summary",
            Self::Details => "Details",
            Self::EarnedValue => "Earned Value",
        }
    }
    pub const ALL: [TaskTable; 10] = [
        TaskTable::Entry,
        TaskTable::Schedule,
        TaskTable::Tracking,
        TaskTable::Work,
        TaskTable::Cost,
        TaskTable::Estimates,
        TaskTable::Variance,
        TaskTable::Summary,
        TaskTable::Details,
        TaskTable::EarnedValue,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TaskTable::Entry => crate::tr("Entry"),
            TaskTable::Schedule => crate::tr("Schedule"),
            TaskTable::Tracking => crate::tr("Tracking"),
            TaskTable::Work => crate::tr("Work"),
            TaskTable::Cost => crate::tr("Cost"),
            TaskTable::Estimates => crate::tr("Estimates"),
            TaskTable::Variance => crate::tr("Variance"),
            TaskTable::Summary => crate::tr("Summary"),
            TaskTable::Details => crate::tr("Details"),
            TaskTable::EarnedValue => crate::tr("Earned Value"),
        }
    }

    pub fn columns(self) -> Vec<TaskCol> {
        use TaskCol::*;
        match self {
            TaskTable::Entry => vec![Indicators, Mode, Name, Duration, Start, Finish, Predecessors, Resources],
            TaskTable::Schedule => vec![
                Indicators,
                Name,
                Start,
                Finish,
                LateStart,
                LateFinish,
                FreeSlack,
                TotalSlack,
                ConstraintType,
                ConstraintDate,
                Deadline,
            ],
            TaskTable::Tracking => {
                vec![
                    Indicators,
                    Name,
                    ActualStart,
                    ActualFinish,
                    PercentComplete,
                    ActualDuration,
                    RemainingDuration,
                    ActualWork,
                    RemainingWork,
                    Cost,
                ]
            }
            TaskTable::Work => {
                vec![Indicators, Name, Work, Baseline(0, BlField::Work), WorkVariance, ActualWork, RemainingWork, PercentComplete]
            }
            TaskTable::Cost => {
                vec![
                    Indicators,
                    Name,
                    FixedCost,
                    FixedCostAccrual,
                    Cost,
                    Baseline(0, BlField::Cost),
                    CostVariance,
                    ActualCost,
                    RemainingCost,
                ]
            }
            TaskTable::EarnedValue => vec![Indicators, Name, Bcws, Bcwp, Acwp, Sv, Cv, Eac, Bac, Vac],
            TaskTable::Estimates => {
                vec![Indicators, Name, EstimateMethod, Points, Optimistic, MostLikely, Pessimistic, ExpectedWork, Work, Duration, Resources]
            }
            TaskTable::Variance => vec![
                Indicators,
                Name,
                Start,
                Finish,
                Baseline(0, BlField::Start),
                Baseline(0, BlField::Finish),
                StartVariance,
                FinishVariance,
            ],
            TaskTable::Summary => {
                vec![Indicators, OutlineNumber, Wbs, Name, Duration, Start, Finish, PercentComplete, Cost, Work, Milestones]
            }
            TaskTable::Details => {
                vec![Indicators, UniqueId, Name, TaskType, Priority, EarlyStart, EarlyFinish, Predecessors, Successors, Deliverable, Notes]
            }
        }
    }
}

impl TaskCol {
    pub fn def(self, p: &Project) -> ColumnDef {
        use TaskCol::*;
        let _ = p;
        match self {
            Indicators => ColumnDef::new(crate::tr("ℹ"), 34.0).read_only().center(),
            Mode => {
                ColumnDef::new(crate::tr("Mode"), 46.0).center().choice(&[crate::tr("Auto Scheduled"), crate::tr("Manually Scheduled")])
            }
            Wbs => ColumnDef::new(crate::tr("WBS"), 50.0).read_only(),
            Name => ColumnDef::new(crate::tr("Task Name"), 230.0),
            Duration => ColumnDef::new(crate::tr("Duration"), 76.0).right(),
            Start => ColumnDef::new(crate::tr("Start"), 126.0).date(),
            Finish => ColumnDef::new(crate::tr("Finish"), 126.0).date(),
            Predecessors => ColumnDef::new(crate::tr("Predecessors"), 96.0),
            Successors => ColumnDef::new(crate::tr("Successors"), 96.0).read_only(),
            Resources => ColumnDef::new(crate::tr("Resource Names"), 160.0),
            PercentComplete => ColumnDef::new(crate::tr("% Comp."), 62.0).right(),
            Work => ColumnDef::new(crate::tr("Work"), 70.0).right(),
            ActualWork => ColumnDef::new(crate::tr("Act. Work"), 72.0).right().read_only(),
            RemainingWork => ColumnDef::new(crate::tr("Rem. Work"), 72.0).right().read_only(),
            Cost => ColumnDef::new(crate::tr("Total Cost"), 90.0).right().read_only(),
            FixedCost => ColumnDef::new(crate::tr("Fixed Cost"), 86.0).right(),
            Baseline(0, BlField::Start) => ColumnDef::new(crate::tr("Baseline Start"), 100.0).read_only(),
            Baseline(0, BlField::Finish) => ColumnDef::new(crate::tr("Baseline Finish"), 100.0).read_only(),
            Baseline(0, BlField::Work) => ColumnDef::new(crate::tr("Baseline Work"), 86.0).right().read_only(),
            Baseline(0, BlField::Cost) => ColumnDef::new(crate::tr("Baseline Cost"), 90.0).right().read_only(),
            StartVariance => ColumnDef::new(crate::tr("Start Var."), 76.0).right().read_only(),
            FinishVariance => ColumnDef::new(crate::tr("Finish Var."), 76.0).right().read_only(),
            WorkVariance => ColumnDef::new(crate::tr("Work Var."), 72.0).right().read_only(),
            CostVariance => ColumnDef::new(crate::tr("Cost Var."), 86.0).right().read_only(),
            ActualStart => ColumnDef::new(crate::tr("Actual Start"), 126.0).date(),
            ActualFinish => ColumnDef::new(crate::tr("Actual Finish"), 126.0).date(),
            EarlyStart => ColumnDef::new(crate::tr("Early Start"), 100.0).read_only(),
            EarlyFinish => ColumnDef::new(crate::tr("Early Finish"), 100.0).read_only(),
            LateStart => ColumnDef::new(crate::tr("Late Start"), 100.0).read_only(),
            LateFinish => ColumnDef::new(crate::tr("Late Finish"), 100.0).read_only(),
            FreeSlack => ColumnDef::new(crate::tr("Free Slack"), 76.0).right().read_only(),
            TotalSlack => ColumnDef::new(crate::tr("Total Slack"), 76.0).right().read_only(),
            ConstraintType => ColumnDef::new(crate::tr("Constraint"), 150.0).choice(&ConstraintKind::ALL.map(|c| c.label())),
            ConstraintDate => ColumnDef::new(crate::tr("Constraint Date"), 130.0).date(),
            Deadline => ColumnDef::new(crate::tr("Deadline"), 126.0).date(),
            Priority => ColumnDef::new(crate::tr("Priority"), 60.0).right(),
            TaskType => ColumnDef::new(crate::tr("Type"), 100.0).choice(&[
                crate::tr("Fixed Units"),
                crate::tr("Fixed Duration"),
                crate::tr("Fixed Work"),
            ]),
            EstimateMethod => ColumnDef::new(crate::tr("Estimate"), 96.0).choice(&[
                crate::tr("None"),
                crate::tr("Complexity"),
                crate::tr("Three-point"),
                crate::tr("Work"),
            ]),
            Points => ColumnDef::new(crate::tr("Points"), 58.0).right(),
            Optimistic => ColumnDef::new(crate::tr("Optimistic"), 80.0).right(),
            MostLikely => ColumnDef::new(crate::tr("Most Likely"), 80.0).right(),
            Pessimistic => ColumnDef::new(crate::tr("Pessimistic"), 80.0).right(),
            ExpectedWork => ColumnDef::new(crate::tr("Expected Work"), 90.0).right().read_only(),
            Deliverable => ColumnDef::new(crate::tr("Deliverable"), 160.0),
            Milestones => ColumnDef::new(crate::tr("Milestones"), 140.0).read_only(),
            Notes => ColumnDef::new(crate::tr("Notes"), 200.0),
            UniqueId => ColumnDef::new(crate::tr("Unique ID"), 64.0).right().read_only(),
            FixedCostAccrual => ColumnDef::new(crate::tr("Fixed Cost Accrual"), 110.0).choice(&Accrual::ALL.map(|a| a.label())),
            ActualCost => ColumnDef::new(crate::tr("Actual Cost"), 96.0).right().read_only(),
            RemainingCost => ColumnDef::new(crate::tr("Remaining Cost"), 96.0).right().read_only(),
            Bcws => ColumnDef::new(crate::tr("BCWS"), 96.0).right().read_only(),
            Bcwp => ColumnDef::new(crate::tr("BCWP"), 96.0).right().read_only(),
            Acwp => ColumnDef::new(crate::tr("ACWP"), 96.0).right().read_only(),
            Sv => ColumnDef::new(crate::tr("SV"), 96.0).right().read_only(),
            Cv => ColumnDef::new(crate::tr("CV"), 96.0).right().read_only(),
            Eac => ColumnDef::new(crate::tr("EAC"), 96.0).right().read_only(),
            Bac => ColumnDef::new(crate::tr("BAC"), 96.0).right().read_only(),
            Vac => ColumnDef::new(crate::tr("VAC"), 96.0).right().read_only(),
            OutlineNumber => ColumnDef::new(crate::tr("Outline Number"), 60.0).read_only(),
            Baseline(n, f) => {
                let title = format!("{} {}", PlanSlot::Baseline(n).name(), f.label());
                match f {
                    BlField::Start | BlField::Finish => ColumnDef::new(title, 106.0).read_only(),
                    _ => ColumnDef::new(title, 96.0).right().read_only(),
                }
            }
            Interim(n, finish) => {
                ColumnDef::new(crate::tr_dyn(&format!("{}{n}", if finish { "Finish" } else { "Start" })), 96.0).read_only()
            }
            ActualDuration => ColumnDef::new(crate::tr("Act. Dur."), 76.0).right(),
            RemainingDuration => ColumnDef::new(crate::tr("Rem. Dur."), 76.0).right(),
            Stop => ColumnDef::new(crate::tr("Stop"), 100.0).read_only(),
            Resume => ColumnDef::new(crate::tr("Resume"), 100.0),
            Recurring => ColumnDef::new(crate::tr("Recurring"), 70.0).center().read_only(),
            BoardStatus => {
                let names: Vec<&str> = p.board_columns.iter().map(|c| c.name.as_str()).collect();
                ColumnDef::new(crate::tr("Board Status"), 110.0).choice(&names)
            }
            ShowOnBoard => ColumnDef::new(crate::tr("Show on Board"), 92.0).center().choice(&["Yes", "No"]),
        }
    }
}

fn var_text(p: &Project, m: Minutes) -> String {
    if m == 0 { p.format_duration(0, false) } else { format!("{}{}", if m > 0 { "+" } else { "-" }, p.format_duration(m.abs(), false)) }
}

/// Indicator glyphs for a task.
pub fn indicators(p: &Project, t: &Task, overallocated: &dyn Fn(TaskId) -> bool) -> Vec<(String, egui::Color32, String)> {
    let mut v = Vec::new();
    if t.is_complete() {
        v.push(("✔".into(), theme::pal().OK, crate::tr("This task was completed").into()));
    }
    if !t.sched.warnings.is_empty() {
        v.push(("⚠".into(), theme::pal().ERROR, t.sched.warnings.join("\n")));
    } else if t.deadline.is_some() {
        v.push(("⚑".into(), theme::pal().DEADLINE, crate::tr_dyn(&format!("Deadline: {}", format::date(t.deadline)))));
    }
    if t.constraint.kind.needs_date() {
        v.push((
            "📌".into(),
            theme::pal().ACCENT_MID,
            crate::tr_dyn(&format!("{} constraint on {}", t.constraint.kind.label(), format::date(t.constraint.date))),
        ));
    }
    if overallocated(t.id) {
        v.push(("👤".into(), theme::pal().ERROR, crate::tr("A resource assigned to this task is overallocated").into()));
    }
    if t.predecessors.iter().any(|d| matches!(d.predecessor, TaskRef::External { .. }))
        || t.members.iter().any(|m| matches!(m, TaskRef::External { .. }))
    {
        v.push(("🔗".into(), theme::pal().ACCENT_MID, crate::tr("Linked to a task in another project").into()));
    }
    if !t.notes.is_empty() || !t.attachments.is_empty() {
        v.push(("📝".into(), theme::pal().TEXT_WEAK, note_tooltip(&t.notes, &t.attachments)));
    }
    if !t.active {
        v.push(("⏸".into(), theme::pal().TEXT_WEAK, crate::tr("Inactive task").into()));
    }
    if t.recurrence.is_some() {
        v.insert(0, ("↻".into(), theme::pal().ACCENT_MID, crate::tr("Recurring task").into()));
    }
    if let Some(h) = &t.hyperlink {
        v.insert(
            0,
            (
                "🌐".into(),
                theme::pal().ACCENT_MID,
                crate::tr_dyn(&format!("Hyperlink: {}", if h.text.is_empty() { &h.address } else { &h.text })),
            ),
        );
    }
    if t.recurrence_pattern.is_some() {
        let n = p.task_index(t.id).map_or(0, |i| p.descendants_range(i).len());
        v.insert(0, ("🔁".into(), theme::pal().ACCENT_MID, format!("Recurring task with {n} occurrences")));
    }
    let _ = p;
    v.truncate(2);
    v
}

pub struct CellCtx<'a> {
    pub overallocated: &'a dyn Fn(TaskId) -> bool,
}

/// Cell content of `col` for the task at `idx`.
pub fn cell(col: TaskCol, p: &Project, idx: usize, ctx: &CellCtx) -> Cell {
    use TaskCol::*;
    let Some(t) = p.tasks.get(idx) else { return Cell::default() };
    let s = &t.sched;
    let summary = s.is_summary;
    let mut c = match col {
        Indicators => Cell { icons: indicators(p, t, ctx.overallocated), ..Default::default() },
        Mode => {
            let (g, tip) = match t.mode {
                ScheduleMode::Auto => ("⟳", crate::tr("Auto Scheduled")),
                ScheduleMode::Manual => ("📌", crate::tr("Manually Scheduled")),
            };
            Cell {
                text: g.into(),
                color: Some(theme::pal().ACCENT_MID),
                tooltip: Some(tip.into()),
                edit_text: Some(tip.into()),
                ..Default::default()
            }
        }
        Wbs => Cell::weak(p.wbs(idx)),
        Name => {
            let indent = (t.outline_level.saturating_sub(1)) as f32 * 16.0;
            Cell { text: t.name.clone(), bold: summary, indent, expander: if summary { Some(true) } else { None }, ..Default::default() }
        }
        Duration => Cell::text(p.format_duration(s.duration, t.estimated && !summary)),
        Start => Cell { text: format::date(s.start), edit_text: Some(format::iso_date(s.start)), ..Default::default() },
        Finish => Cell { text: format::date(s.finish), edit_text: Some(format::iso_date(s.finish)), ..Default::default() },
        Predecessors => Cell::text(p.format_predecessors(t.id)),
        Successors => Cell::text(
            p.successors(t.id).iter().filter_map(|(sid, _)| p.row_number(*sid)).map(|r| r.to_string()).collect::<Vec<_>>().join(","),
        ),
        Resources => Cell::text(p.format_resource_names(t.id)),
        PercentComplete => Cell::text(format::percent(s.percent_complete)),
        Work => Cell::text(format_work(s.work)),
        ActualWork => Cell::text(format_work(s.actual_work)),
        RemainingWork => Cell::text(format_work((s.work - s.actual_work).max(0))),
        Cost => Cell::text(format::money(p, s.cost)),
        FixedCost => Cell::text(format::money(p, t.fixed_cost)),
        Baseline(n, f) => Cell::text(match (t.baseline_in(n), f) {
            (Some(b), BlField::Start) => format::date(Some(b.start)),
            (Some(b), BlField::Finish) => format::date(Some(b.finish)),
            (Some(b), BlField::Duration) => p.format_duration(b.duration, false),
            (Some(b), BlField::Work) => format_work(b.work),
            (Some(b), BlField::Cost) => format::money(p, b.cost),
            (None, BlField::Start | BlField::Finish) => "NA".into(),
            (None, _) => String::new(),
        }),
        Interim(n, finish) => {
            Cell::text(t.interim_in(n).map_or_else(|| "NA".into(), |i| format::date(Some(if finish { i.finish } else { i.start }))))
        }
        StartVariance | FinishVariance => {
            let wc = plancraft_core::schedule::project_calendar(p);
            let v = match (t.baseline, s.start, s.finish) {
                (Some(b), Some(st), Some(fi)) => {
                    Some(if col == StartVariance { wc.between(b.start, st) } else { wc.between(b.finish, fi) })
                }
                _ => None,
            };
            match v {
                Some(m) => Cell {
                    text: var_text(p, m),
                    color: Some(if m > 0 { theme::pal().ERROR } else { theme::pal().TEXT }),
                    ..Default::default()
                },
                None => Cell::default(),
            }
        }
        WorkVariance => t.baseline.map(|b| Cell::text(format_work(s.work - b.work))).unwrap_or_default(),
        CostVariance => t.baseline.map(|b| Cell::text(format::money(p, s.cost - b.cost))).unwrap_or_default(),
        ActualStart => Cell { text: format::date(t.actual_start), edit_text: Some(format::iso_date(t.actual_start)), ..Default::default() },
        ActualFinish => {
            Cell { text: format::date(t.actual_finish), edit_text: Some(format::iso_date(t.actual_finish)), ..Default::default() }
        }
        ActualDuration | RemainingDuration => {
            let (done, left) = t.duration_progress();
            Cell::text(p.format_duration(if col == ActualDuration { done } else { left }, false))
        }
        Stop => Cell::text(format::date(s.stop)),
        Resume => Cell { text: format::date(s.resume), edit_text: Some(format::iso_date(s.resume)), ..Default::default() },
        EarlyStart => Cell::text(format::date(s.early_start)),
        EarlyFinish => Cell::text(format::date(s.early_finish)),
        LateStart => Cell::text(format::date(s.late_start)),
        LateFinish => Cell::text(format::date(s.late_finish)),
        FreeSlack => Cell::text(format::slack(p, s.free_slack)),
        TotalSlack => Cell {
            text: format::slack(p, s.total_slack),
            color: if s.total_slack < 0 { Some(theme::pal().ERROR) } else { None },
            ..Default::default()
        },
        ConstraintType => Cell::text(t.constraint.kind.label()),
        ConstraintDate => {
            Cell { text: format::date(t.constraint.date), edit_text: Some(format::iso_date(t.constraint.date)), ..Default::default() }
        }
        Deadline => Cell { text: format::date(t.deadline), edit_text: Some(format::iso_date(t.deadline)), ..Default::default() },
        Priority => Cell::text(t.priority.to_string()),
        TaskType => Cell::text(match t.task_type {
            plancraft_core::model::TaskType::FixedUnits => crate::tr("Fixed Units"),
            plancraft_core::model::TaskType::FixedDuration => crate::tr("Fixed Duration"),
            plancraft_core::model::TaskType::FixedWork => crate::tr("Fixed Work"),
        }),
        EstimateMethod => Cell::text(match t.estimate {
            None => crate::tr("None"),
            Some(Estimate::Complexity { .. }) => crate::tr("Complexity"),
            Some(Estimate::ThreePoint { .. }) => crate::tr("Three-point"),
            Some(Estimate::Work { .. }) => crate::tr("Work"),
        }),
        Points => match t.estimate {
            Some(Estimate::Complexity { points }) => Cell::text(format!("{points}")),
            _ => Cell::default(),
        },
        Optimistic | MostLikely | Pessimistic => match t.estimate {
            Some(Estimate::ThreePoint { optimistic, most_likely, pessimistic }) => Cell::text(format_work(match col {
                Optimistic => optimistic,
                MostLikely => most_likely,
                _ => pessimistic,
            })),
            _ => Cell::default(),
        },
        ExpectedWork => t.estimate.map(|e| Cell::text(format_work(e.expected_work(&p.settings)))).unwrap_or_default(),
        Deliverable => Cell::text(t.deliverable.clone()),
        Milestones => {
            Cell::weak(p.milestones_of(t.id).iter().filter_map(|m| p.task(*m)).map(|m| m.name.clone()).collect::<Vec<_>>().join(", "))
        }
        Notes => {
            Cell { text: t.notes.lines().next().unwrap_or_default().to_string(), edit_text: Some(t.notes.clone()), ..Default::default() }
        }
        UniqueId => Cell::weak(t.id.0.to_string()),
        FixedCostAccrual => Cell::text(t.fixed_cost_accrual.label()),
        ActualCost | RemainingCost => {
            let actual =
                if s.work > 0 { s.cost * s.actual_work as f64 / s.work as f64 } else { s.cost * f64::from(s.percent_complete) / 100.0 };
            Cell::text(format::money(p, if col == ActualCost { actual } else { (s.cost - actual).max(0.0) }))
        }
        Bcws | Bcwp | Acwp | Sv | Cv | Eac | Bac | Vac => match plancraft_core::extras::task_earned_value(p, idx) {
            Some(ev) => {
                let v = match col {
                    Bcws => ev.bcws,
                    Bcwp => ev.bcwp,
                    Acwp => ev.acwp,
                    Sv => ev.sv,
                    Cv => ev.cv,
                    Eac => ev.eac,
                    Bac => ev.bac,
                    _ => ev.vac,
                };
                Cell {
                    text: format::money(p, v),
                    color: if matches!(col, Sv | Cv | Vac) && v < -0.005 { Some(theme::pal().ERROR) } else { None },
                    ..Default::default()
                }
            }
            None => Cell::default(),
        },
        OutlineNumber => Cell::weak(outline_number(p, idx)),
        Recurring => Cell::text(crate::tr(if p.is_recurring(idx) { "Yes" } else { "No" })),
        // Summary tasks have no card; their subtasks do.
        BoardStatus if summary => Cell::default(),
        BoardStatus => Cell::text(p.board_column_of(idx).and_then(|c| p.board_column(c)).map(|c| c.name.clone()).unwrap_or_default()),
        ShowOnBoard => Cell::text(crate::tr(if t.show_on_board { "Yes" } else { "No" })),
    };
    if summary && col != Name && col != Indicators {
        c.bold = true;
    }
    if !t.active {
        c.color = Some(theme::pal().TEXT_WEAK);
        c.strike = col == Name;
    }
    if col == Name && t.sched.critical && t.mode == ScheduleMode::Auto && !summary {
        // Critical tasks are shown in the chart; keep table readable.
    }
    c.read_only = !can_edit_at(col, p, idx);
    c
}

/// Task-specific editability used by both inline editing and bulk paste.
pub fn can_edit(col: TaskCol, p: &Project, id: TaskId) -> bool {
    p.task_index(id).is_some_and(|index| can_edit_at(col, p, index))
}

fn can_edit_at(col: TaskCol, p: &Project, index: usize) -> bool {
    if col.def(p).editor == crate::widgets::grid::Editor::None {
        return false;
    }
    let Some(task) = p.tasks.get(index) else { return false };
    match col {
        TaskCol::Start | TaskCol::Finish => task.mode == ScheduleMode::Manual && !p.has_children(index),
        TaskCol::Duration | TaskCol::Work => !p.has_children(index),
        _ => true,
    }
}

fn parse_date(p: &Project, text: &str, finish: bool) -> Result<Option<NaiveDateTime>, String> {
    let t = text.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("na") || t.to_lowercase() == "нд" {
        return Ok(None);
    }
    p.parse_datetime(t, finish).map(Some).ok_or_else(|| crate::tr_dyn(&format!("'{t}' is not a valid date (use YYYY-MM-DD)")))
}

fn parse_work_text(p: &Project, text: &str) -> Result<Minutes, String> {
    parse_work(text, &p.settings.time).ok_or_else(|| crate::tr_dyn(&format!("'{text}' is not a valid amount of work")))
}

fn task_mut(p: &mut Project, id: TaskId) -> Result<&mut Task, String> {
    p.task_mut(id).ok_or_else(|| crate::tr("task not found").to_string())
}

/// Applies an edited value to a task. Runs inside an undoable edit.
pub fn commit(col: TaskCol, p: &mut Project, id: TaskId, text: &str) -> Result<(), String> {
    use TaskCol::*;
    let idx = p.task_index(id).ok_or(crate::tr("task not found"))?;
    let summary = p.has_children(idx);
    let err = |e: PlanError| e.to_string();
    // Choice lists show translated labels; match on the English originals.
    let text = if matches!(col, Mode | ConstraintType | TaskType | EstimateMethod | FixedCostAccrual) { crate::untr(text) } else { text };
    match col {
        Name => {
            if text.trim().is_empty() {
                return Err(crate::tr("task name cannot be empty").into());
            }
            task_mut(p, id)?.name = text.trim().to_string();
        }
        Mode => {
            let t = task_mut(p, id)?;
            t.mode = if text.starts_with('M') { ScheduleMode::Manual } else { ScheduleMode::Auto };
            if t.mode == ScheduleMode::Manual {
                t.manual_start = t.sched.start;
                t.manual_finish = t.sched.finish;
            }
        }
        Duration => {
            let (m, est) = p.parse_duration(text).ok_or_else(|| crate::tr_dyn(&format!("'{text}' is not a valid duration")))?;
            p.set_duration(id, m, est).map_err(err)?;
        }
        Start | Finish => {
            let finish = col == Finish;
            let date = parse_date(p, text, finish)?.ok_or(crate::tr("a date is required"))?;
            if summary {
                return Err(crate::tr("summary task dates are calculated from their subtasks").into());
            }
            let t = task_mut(p, id)?;
            if t.mode == ScheduleMode::Manual {
                let start = if finish { t.manual_start } else { Some(date) };
                let end = if finish { Some(date) } else { t.manual_finish };
                p.set_manual_dates(id, start, end).map_err(err)?;
            } else {
                t.constraint = Constraint { kind: if finish { ConstraintKind::Fnet } else { ConstraintKind::Snet }, date: Some(date) };
            }
        }
        Predecessors => {
            let deps = p.parse_predecessors(text).map_err(err)?;
            p.set_predecessors(id, deps).map_err(err)?;
        }
        BoardStatus => {
            if summary {
                return Err("summary tasks have no card on the board; their subtasks do".into());
            }
            if text.trim().is_empty() {
                // Cleared: the card goes back to the first column.
                task_mut(p, id)?.board_status = None;
                return Ok(());
            }
            let col = p.find_board_column(text).ok_or_else(|| format!("there is no board column named '{}'", text.trim()))?;
            p.set_board_status(&[id], col).map_err(err)?;
        }
        ShowOnBoard => {
            let yes = text.trim().to_lowercase();
            let show = match yes.as_str() {
                "yes" | "y" | "true" => true,
                "no" | "n" | "false" => false,
                _ => return Err("Show on Board is Yes or No".into()),
            };
            p.set_show_on_board(&[id], show).map_err(err)?;
        }
        Resources => p.apply_resource_names(id, text).map_err(err)?,
        PercentComplete => {
            let v: f64 =
                text.trim().trim_end_matches('%').trim().parse().map_err(|_| crate::tr_dyn(&format!("'{text}' is not a percentage")))?;
            p.set_percent_complete(id, v.clamp(0.0, 100.0).round() as u8).map_err(err)?;
        }
        Work => {
            if summary {
                return Err(crate::tr("summary work is calculated from subtasks").into());
            }
            let w = parse_work_text(p, text)?;
            let assignments: Vec<(AssignmentId, Minutes)> = p
                .assignments_for_task(id)
                .filter(|a| p.resource(a.resource).is_some_and(|r| r.kind == ResourceType::Work))
                .map(|a| (a.id, a.work))
                .collect();
            if assignments.is_empty() {
                task_mut(p, id)?.estimate = Some(Estimate::Work { minutes: w });
            } else {
                let total: Minutes = assignments.iter().map(|(_, w)| w).sum();
                for (aid, aw) in &assignments {
                    let share = if total > 0 { *aw as f64 / total as f64 } else { 1.0 / assignments.len() as f64 };
                    p.set_assignment_work(*aid, (w as f64 * share).round() as Minutes).map_err(err)?;
                }
            }
        }
        FixedCost => task_mut(p, id)?.fixed_cost = format::parse_money(text).ok_or(crate::tr("invalid amount"))?,
        ActualStart => task_mut(p, id)?.actual_start = parse_date(p, text, false)?,
        ActualDuration | RemainingDuration => {
            let (m, _) = p.parse_duration(text).ok_or_else(|| format!("'{text}' is not a valid duration"))?;
            if col == ActualDuration { p.set_actual_duration(id, m) } else { p.set_remaining_duration(id, m) }.map_err(err)?;
        }
        Resume => {
            let d = parse_date(p, text, false)?;
            p.set_resume(id, d).map_err(err)?;
        }
        ActualFinish => {
            let d = parse_date(p, text, true)?;
            let t = task_mut(p, id)?;
            t.actual_finish = d;
            if d.is_some() {
                t.percent_complete = 100;
                if t.actual_start.is_none() {
                    t.actual_start = t.sched.start;
                }
            }
        }
        ConstraintType => {
            let kind = ConstraintKind::ALL.into_iter().find(|k| k.label() == text).unwrap_or_default();
            let t = task_mut(p, id)?;
            t.constraint.kind = kind;
            if kind.needs_date() && t.constraint.date.is_none() {
                t.constraint.date = if kind.is_start_constraint() { t.sched.start } else { t.sched.finish };
            }
            if !kind.needs_date() {
                t.constraint.date = None;
            }
        }
        ConstraintDate => {
            let finish = !task_mut(p, id)?.constraint.kind.is_start_constraint();
            let d = parse_date(p, text, finish)?;
            let t = task_mut(p, id)?;
            t.constraint.date = d;
            if d.is_some() && !t.constraint.kind.needs_date() {
                t.constraint.kind = ConstraintKind::Snet;
            }
            if d.is_none() {
                t.constraint.kind = ConstraintKind::Asap;
            }
        }
        Deadline => task_mut(p, id)?.deadline = parse_date(p, text, true)?,
        Priority => task_mut(p, id)?.priority = text.trim().parse::<u16>().map_err(|_| crate::tr("priority must be 0-1000"))?.min(1000),
        TaskType => {
            task_mut(p, id)?.task_type = match text {
                "Fixed Duration" => plancraft_core::model::TaskType::FixedDuration,
                "Fixed Work" => plancraft_core::model::TaskType::FixedWork,
                _ => plancraft_core::model::TaskType::FixedUnits,
            }
        }
        EstimateMethod => {
            let t = task_mut(p, id)?;
            let work_guess = t.sched.work.max(t.duration);
            t.estimate = match text {
                "Complexity" => Some(Estimate::Complexity { points: 3.0 }),
                "Three-point" => {
                    Some(Estimate::ThreePoint { optimistic: work_guess * 3 / 4, most_likely: work_guess, pessimistic: work_guess * 3 / 2 })
                }
                "Work" => Some(Estimate::Work { minutes: work_guess }),
                _ => None,
            };
        }
        Points => {
            let v: f64 = text.trim().parse().map_err(|_| crate::tr("points must be a number"))?;
            task_mut(p, id)?.estimate = Some(Estimate::Complexity { points: v.max(0.0) });
        }
        Optimistic | MostLikely | Pessimistic => {
            let w = parse_work_text(p, text)?;
            let t = task_mut(p, id)?;
            let (mut o, mut m, mut pe) = match t.estimate {
                Some(Estimate::ThreePoint { optimistic, most_likely, pessimistic }) => (optimistic, most_likely, pessimistic),
                _ => (w, w, w),
            };
            match col {
                Optimistic => o = w,
                MostLikely => m = w,
                _ => pe = w,
            }
            t.estimate = Some(Estimate::ThreePoint { optimistic: o, most_likely: m, pessimistic: pe });
        }
        Deliverable => task_mut(p, id)?.deliverable = text.to_string(),
        FixedCostAccrual => {
            task_mut(p, id)?.fixed_cost_accrual = match text {
                "Start of task" => Accrual::Start,
                "End of task" => Accrual::End,
                _ => Accrual::Prorated,
            };
        }
        Notes => {
            // The cell shows and edits only the first line; keep the rest of a longer note.
            let t = task_mut(p, id)?;
            t.notes = if text.contains('\n') || text == t.notes {
                text.to_string()
            } else {
                match t.notes.split_once('\n') {
                    Some((_, rest)) if text.is_empty() => rest.to_string(),
                    Some((_, rest)) => format!("{text}\n{rest}"),
                    None => text.to_string(),
                }
            };
        }
        _ => return Err("this field is calculated and cannot be edited".into()),
    }
    Ok(())
}

/// Outline number such as "2.1.3" (independent of a custom WBS mask).
fn outline_number(p: &Project, idx: usize) -> String {
    let mut counters: Vec<usize> = Vec::new();
    for t in p.tasks.iter().take(idx.saturating_add(1)) {
        counters.resize(usize::from(t.outline_level.max(1)), 0);
        if let Some(c) = counters.last_mut() {
            *c += 1;
        }
    }
    counters.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(".")
}

/// Late-task check used by filters and indicators.
pub fn is_late(p: &Project, t: &Task) -> bool {
    report::is_late(t, report::status_moment(p), p)
}

/// Tooltip of the notes indicator: the note text and the names of embedded files.
pub fn note_tooltip(notes: &str, files: &[plancraft_core::attach::Attachment]) -> String {
    let mut s = notes.to_string();
    for a in files {
        if !s.is_empty() {
            s.push('\n');
        }
        s += &format!("📄 {}", a.name);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    /// Regression: the Notes cell shows only the first line, and committing it used to replace
    /// the whole note with that line.
    #[test]
    fn editing_the_notes_cell_keeps_the_other_lines() {
        let mut p = Project::new("Notes", NaiveDate::from_ymd_opt(2026, 10, 19).unwrap());
        let id = p.add_task("Task", None);
        p.task_mut(id).unwrap().notes = "first\nsecond\nthird".into();
        commit(TaskCol::Notes, &mut p, id, "changed").unwrap();
        assert_eq!(p.task(id).unwrap().notes, "changed\nsecond\nthird");
        commit(TaskCol::Notes, &mut p, id, "").unwrap();
        assert_eq!(p.task(id).unwrap().notes, "second\nthird");
        p.task_mut(id).unwrap().notes = "one line".into();
        commit(TaskCol::Notes, &mut p, id, "replaced").unwrap();
        assert_eq!(p.task(id).unwrap().notes, "replaced");
    }
}

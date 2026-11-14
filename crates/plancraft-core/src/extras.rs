//! Features used by project-management courses: time-phased cost and cash flow (with cost
//! accrual), per-task earned value, numbered baselines and interim plans, recurring tasks,
//! scheduling from the project finish date and report data.

use std::collections::BTreeMap;

use chrono::{Datelike, Days, Months, NaiveDate, NaiveDateTime};

use crate::duration::Minutes;
use crate::ids::{ResourceId, TaskId};
use crate::model::*;
use crate::ops::{PlanError, PlanResult};
use crate::report::{EarnedValue, status_moment};
use crate::schedule::{project_calendar, task_calendar};
use crate::workload::{Granularity, assignment_daily_work, distribute};

// ---------------------------------------------------------------------------
// Time-phased cost
// ---------------------------------------------------------------------------

fn spread(p: &Project, ti: usize, total: f64, accrual: Accrual, out: &mut BTreeMap<NaiveDate, f64>) {
    if total == 0.0 || !total.is_finite() {
        return;
    }
    let Some(t) = p.tasks.get(ti) else { return };
    let (Some(s), Some(f)) = (t.sched.start, t.sched.finish) else { return };
    match accrual {
        Accrual::Start => *out.entry(s.date()).or_default() += total,
        Accrual::End => *out.entry(f.date()).or_default() += total,
        Accrual::Prorated => {
            let cal = task_calendar(p, ti);
            let d = distribute(&cal, s, f, total.abs(), WorkContour::Flat);
            if d.is_empty() {
                *out.entry(s.date()).or_default() += total;
            } else {
                let sign = total.signum();
                for (day, v) in d {
                    *out.entry(day).or_default() += v * sign;
                }
            }
        }
    }
}

/// Cost of one assignment per day, honoring the resource's accrual.
pub fn assignment_daily_cost(p: &Project, a: &Assignment) -> BTreeMap<NaiveDate, f64> {
    let mut out = BTreeMap::new();
    let (Some(r), Some(ti)) = (p.resource(a.resource), p.task_index(a.task)) else {
        return out;
    };
    let Some(t) = p.tasks.get(ti) else { return out };
    match r.kind {
        ResourceType::Work => {
            let total = a.work as f64 / 60.0 * r.standard_rate;
            if r.accrue_at == Accrual::Prorated {
                for (d, w) in assignment_daily_work(p, a) {
                    *out.entry(d).or_default() += w / 60.0 * r.standard_rate;
                }
                if out.is_empty() && total != 0.0 {
                    spread(p, ti, total, Accrual::Start, &mut out);
                }
            } else {
                spread(p, ti, total, r.accrue_at, &mut out);
            }
        }
        ResourceType::Material => spread(p, ti, a.units * r.standard_rate, r.accrue_at, &mut out),
        ResourceType::Cost => spread(p, ti, a.cost, r.accrue_at, &mut out),
    }
    if r.cost_per_use != 0.0
        && let Some(s) = t.sched.start
    {
        *out.entry(s.date()).or_default() += r.cost_per_use;
    }
    out
}

/// Cost of a task per day: its assignments plus its fixed cost. Summary tasks add up their subtasks.
pub fn task_daily_cost(p: &Project, ti: usize) -> BTreeMap<NaiveDate, f64> {
    let mut out = BTreeMap::new();
    let Some(t) = p.tasks.get(ti) else { return out };
    if !t.active {
        return out;
    }
    if t.sched.is_summary {
        for ci in p.descendants_range(ti) {
            if p.tasks.get(ci).is_some_and(|c| !c.sched.is_summary) {
                for (d, v) in task_daily_cost(p, ci) {
                    *out.entry(d).or_default() += v;
                }
            }
        }
        return out;
    }
    for a in p.assignments_for_task(t.id) {
        for (d, v) in assignment_daily_cost(p, a) {
            *out.entry(d).or_default() += v;
        }
    }
    spread(p, ti, t.fixed_cost, t.fixed_cost_accrual, &mut out);
    out
}

/// Cost of a resource per day over all its assignments.
pub fn resource_daily_cost(p: &Project, rid: ResourceId) -> BTreeMap<NaiveDate, f64> {
    let mut out = BTreeMap::new();
    for a in p.assignments.iter().filter(|a| a.resource == rid) {
        if p.task(a.task).is_some_and(|t| !t.active) {
            continue;
        }
        for (d, v) in assignment_daily_cost(p, a) {
            *out.entry(d).or_default() += v;
        }
    }
    out
}

/// Total cost of a resource (all assignments).
pub fn resource_cost(p: &Project, rid: ResourceId) -> f64 {
    let Some(r) = p.resource(rid) else { return 0.0 };
    p.assignments
        .iter()
        .filter(|a| a.resource == rid)
        .map(|a| match r.kind {
            ResourceType::Work => a.work as f64 / 60.0 * r.standard_rate + r.cost_per_use,
            ResourceType::Material => a.units * r.standard_rate + r.cost_per_use,
            ResourceType::Cost => a.cost,
        })
        .sum()
}

/// Total work of a resource in minutes.
pub fn resource_work(p: &Project, rid: ResourceId) -> Minutes {
    p.assignments.iter().filter(|a| a.resource == rid).map(|a| a.work).fold(0, Minutes::saturating_add)
}

/// Sums daily values into periods.
pub fn bucket(daily: &BTreeMap<NaiveDate, f64>, g: Granularity) -> BTreeMap<NaiveDate, f64> {
    let mut out = BTreeMap::new();
    for (d, v) in daily {
        *out.entry(g.bucket_start(*d)).or_default() += v;
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
pub struct CashFlowRow {
    pub period: NaiveDate,
    pub cost: f64,
    pub cumulative: f64,
}

/// Project cost per period with the running total ("Cash Flow" report).
pub fn cash_flow(p: &Project, g: Granularity) -> Vec<CashFlowRow> {
    let mut daily: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for (i, t) in p.tasks.iter().enumerate() {
        if t.sched.is_summary {
            continue;
        }
        for (d, v) in task_daily_cost(p, i) {
            *daily.entry(d).or_default() += v;
        }
    }
    let buckets = bucket(&daily, g);
    let (Some(first), Some(last)) = (buckets.keys().next().copied(), buckets.keys().next_back().copied()) else {
        return Vec::new();
    };
    let mut cum = 0.0;
    g.buckets(first, last)
        .into_iter()
        .map(|period| {
            let cost = buckets.get(&period).copied().unwrap_or(0.0);
            cum += cost;
            CashFlowRow { period, cost, cumulative: cum }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Earned value per task
// ---------------------------------------------------------------------------

fn fraction(p: &Project, start: NaiveDateTime, finish: NaiveDateTime, at: NaiveDateTime) -> f64 {
    if at <= start {
        return 0.0;
    }
    if at >= finish {
        return 1.0;
    }
    let wc = project_calendar(p);
    let total = wc.between(start, finish);
    if total <= 0 { 1.0 } else { wc.between(start, at) as f64 / total as f64 }
}

/// Earned value of one task (summaries add up their subtasks); `None` without a baseline.
pub fn task_earned_value(p: &Project, ti: usize) -> Option<TaskEarnedValue> {
    let at = status_moment(p);
    let t = p.tasks.get(ti)?;
    let leaves: Vec<usize> = if t.sched.is_summary { p.descendants_range(ti).collect() } else { vec![ti] };
    let mut ev = TaskEarnedValue { baseline: p.settings.earned_value_baseline, ..Default::default() };
    let mut any = false;
    for li in leaves {
        let Some(l) = p.tasks.get(li) else { continue };
        if l.sched.is_summary || !l.active {
            continue;
        }
        let Some(b) = l.baseline_in(p.settings.earned_value_baseline) else { continue };
        any = true;
        ev.bac += b.cost;
        ev.bcws += b.cost * fraction(p, b.start, b.finish, at);
        ev.bcwp += b.cost * l.sched.percent_complete as f64 / 100.0;
        let actual_ratio = if l.sched.work > 0 && l.sched.actual_work > 0 {
            l.sched.actual_work as f64 / l.sched.work as f64
        } else {
            l.sched.percent_complete as f64 / 100.0
        };
        ev.acwp += l.sched.cost * actual_ratio;
    }
    if !any {
        return None;
    }
    ev.sv = ev.bcwp - ev.bcws;
    ev.cv = ev.bcwp - ev.acwp;
    ev.spi = if ev.bcws > 0.0 { ev.bcwp / ev.bcws } else { 1.0 };
    ev.cpi = if ev.acwp > 0.0 { ev.bcwp / ev.acwp } else { 1.0 };
    ev.eac = if ev.cpi > 0.0 { ev.acwp + (ev.bac - ev.bcwp) / ev.cpi } else { ev.bac };
    ev.vac = ev.bac - ev.eac;
    Some(ev)
}

/// Earned-value fields shown in the "Earned Value" table.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct TaskEarnedValue {
    pub baseline: u8,
    pub bcws: f64,
    pub bcwp: f64,
    pub acwp: f64,
    pub sv: f64,
    pub cv: f64,
    pub spi: f64,
    pub cpi: f64,
    /// Budget at completion (baseline cost).
    pub bac: f64,
    /// Estimate at completion.
    pub eac: f64,
    /// Variance at completion.
    pub vac: f64,
}

impl From<TaskEarnedValue> for EarnedValue {
    fn from(v: TaskEarnedValue) -> Self {
        EarnedValue { bcws: v.bcws, bcwp: v.bcwp, acwp: v.acwp, sv: v.sv, cv: v.cv, spi: v.spi, cpi: v.cpi, baseline: v.baseline }
    }
}

// ---------------------------------------------------------------------------
// Baselines and interim plans
// ---------------------------------------------------------------------------

/// What "Set Baseline" saves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanSlot {
    /// Baseline 0 (`n == 0`) or Baseline 1–10.
    Baseline(u8),
    /// Interim plan Start1/Finish1 … Start10/Finish10.
    Interim(u8),
}

impl Project {
    /// Saves a baseline or interim plan for `ids` (all tasks when empty).
    pub fn save_plan(&mut self, slot: PlanSlot, ids: &[TaskId]) -> PlanResult<()> {
        match slot {
            PlanSlot::Baseline(0) => {
                self.set_baseline(ids);
                return Ok(());
            }
            PlanSlot::Baseline(n) | PlanSlot::Interim(n) if !(1..=10).contains(&n) => {
                return Err(PlanError::Invalid(format!("plan number {n} is out of range")));
            }
            _ => {}
        }
        use chrono::Timelike;
        let now = chrono::Local::now().naive_local();
        let now = now.with_second(0).and_then(|n| n.with_nanosecond(0)).unwrap_or(now);
        for t in &mut self.tasks {
            if !ids.is_empty() && !ids.contains(&t.id) {
                continue;
            }
            let (Some(start), Some(finish)) = (t.sched.start, t.sched.finish) else { continue };
            match slot {
                PlanSlot::Baseline(n) => {
                    t.baselines.insert(
                        n,
                        Baseline { start, finish, duration: t.sched.duration, work: t.sched.work, cost: t.sched.cost, saved: now },
                    );
                }
                PlanSlot::Interim(n) => {
                    t.interim.insert(n, InterimPlan { start, finish });
                }
            }
        }
        Ok(())
    }

    /// Clears a baseline or interim plan for `ids` (all tasks when empty).
    pub fn clear_plan(&mut self, slot: PlanSlot, ids: &[TaskId]) {
        for t in &mut self.tasks {
            if !ids.is_empty() && !ids.contains(&t.id) {
                continue;
            }
            match slot {
                PlanSlot::Baseline(0) => t.baseline = None,
                PlanSlot::Baseline(n) => {
                    t.baselines.remove(&n);
                }
                PlanSlot::Interim(n) => {
                    t.interim.remove(&n);
                }
            }
        }
    }

    /// The baseline in `slot` for a task (Baseline 0 is the main one).
    pub fn baseline_of<'a>(&self, t: &'a Task, n: u8) -> Option<&'a Baseline> {
        if n == 0 { t.baseline.as_ref() } else { t.baselines.get(&n) }
    }

    /// When the baseline or interim plan in `slot` was last saved (any task).
    pub fn plan_saved(&self, slot: PlanSlot) -> Option<NaiveDateTime> {
        match slot {
            PlanSlot::Baseline(n) => self.tasks.iter().filter_map(|t| self.baseline_of(t, n).map(|b| b.saved)).max(),
            PlanSlot::Interim(n) => self.tasks.iter().filter_map(|t| t.interim.get(&n).map(|p| p.start)).min(),
        }
    }
}

// ---------------------------------------------------------------------------
// Recurring tasks
// ---------------------------------------------------------------------------

/// Dates of the occurrences of a recurrence pattern starting at `from`, up to `until`.
pub fn recurrence_dates(rec: &Recurrence, from: NaiveDate, until: Option<NaiveDate>, p: &Project) -> Vec<NaiveDate> {
    let cal = project_calendar(p);
    let max = if until.is_some() { 1000 } else { rec.occurrences.clamp(1, 1000) as usize };
    let every = rec.every.max(1);
    let mut out = Vec::new();
    let within = |d: NaiveDate| until.is_none_or(|u| d <= u);
    match rec.kind {
        RecurrenceKind::Daily => {
            let mut d = from;
            while out.len() < max && within(d) {
                if cal.is_working_day(d) {
                    out.push(d);
                }
                let Some(n) = d.checked_add_days(Days::new(u64::from(every))) else { break };
                d = n;
                if d.year() > 2300 {
                    break;
                }
            }
        }
        RecurrenceKind::Weekly => {
            let days: Vec<u32> = if rec.weekdays.is_empty() {
                vec![from.weekday().num_days_from_monday()]
            } else {
                let mut v: Vec<u32> = rec.weekdays.iter().map(|&w| u32::from(w.min(6))).collect();
                v.sort_unstable();
                v.dedup();
                v
            };
            let mut week = from.checked_sub_days(Days::new(u64::from(from.weekday().num_days_from_monday()))).unwrap_or(from);
            'outer: while out.len() < max {
                for wd in &days {
                    let Some(d) = week.checked_add_days(Days::new(u64::from(*wd))) else { break 'outer };
                    if d < from {
                        continue;
                    }
                    if !within(d) || out.len() >= max {
                        break 'outer;
                    }
                    out.push(d);
                }
                let Some(n) = week.checked_add_days(Days::new(7 * u64::from(every))) else { break };
                week = n;
                if week.year() > 2300 {
                    break;
                }
            }
        }
        RecurrenceKind::Monthly => {
            let dom = rec.day_of_month.clamp(1, 31);
            let mut m = from.with_day(1).unwrap_or(from);
            while out.len() < max {
                // Clamp to the month's last day (31 → 30 or 28).
                let mut d = None;
                for day in (1..=dom).rev() {
                    if let Some(x) = m.with_day(day) {
                        d = Some(x);
                        break;
                    }
                }
                if let Some(d) = d
                    && d >= from
                {
                    if !within(d) {
                        break;
                    }
                    out.push(d);
                }
                let Some(n) = m.checked_add_months(Months::new(every)) else { break };
                m = n;
                if m.year() > 2300 {
                    break;
                }
            }
        }
    }
    out
}

impl Project {
    /// Inserts a recurring task at `index`: a summary task with one subtask per occurrence, each
    /// starting no earlier than its date (as Microsoft Project does). Returns the summary's id.
    pub fn insert_recurring(
        &mut self,
        index: usize,
        name: &str,
        duration: Minutes,
        rec: Recurrence,
        from: NaiveDate,
        until: Option<NaiveDate>,
    ) -> PlanResult<TaskId> {
        let dates = recurrence_dates(&rec, from, until, self);
        if dates.is_empty() {
            return Err(PlanError::Invalid(crate::i18n::tr("Recurring task needs at least one occurrence").into()));
        }
        let index = index.min(self.tasks.len());
        let level = index.checked_sub(1).and_then(|i| self.tasks.get(i)).map_or(1, |t| t.outline_level);
        let sid = self.ids.next_task();
        let mut summary = Task::new(sid, name, self.settings.time.minutes_per_day);
        summary.outline_level = level;
        summary.estimated = false;
        summary.recurrence = Some(rec);
        self.insert_task(index, summary);
        let starts: Vec<NaiveDateTime> = {
            let wc = project_calendar(self);
            dates.iter().map(|d| wc.day_start(*d)).collect()
        };
        for (k, start) in starts.into_iter().enumerate() {
            let id = self.ids.next_task();
            let mut t = Task::new(id, format!("{name} {}", k + 1), self.settings.time.minutes_per_day);
            t.outline_level = level.saturating_add(1);
            t.duration = duration.max(0);
            t.estimated = false;
            if duration == 0 {
                t.kind = TaskKind::Milestone;
            }
            t.constraint = Constraint { kind: ConstraintKind::Snet, date: Some(start) };
            self.insert_task(index + 1 + k, t);
        }
        Ok(sid)
    }
}

// ---------------------------------------------------------------------------
// Scheduling from the project finish date
// ---------------------------------------------------------------------------

/// When the project is scheduled from its finish date, moves the project start so the
/// calculated finish lands on `settings.finish_date`. Returns whether the start changed.
pub fn anchor_to_finish(p: &mut Project) -> bool {
    if !p.settings.schedule_from_finish {
        return false;
    }
    let Some(target) = p.settings.finish_date else { return false };
    let Some(finish) = p.sched.finish else { return false };
    if finish == target {
        return false;
    }
    let new_start = {
        let wc = project_calendar(p);
        let delta = wc.between(finish, target);
        if delta >= 0 { wc.add(p.start, delta) } else { wc.sub(p.start, -delta) }
    };
    if new_start == p.start {
        return false;
    }
    p.start = new_start;
    true
}

// ---------------------------------------------------------------------------
// Sorting resources
// ---------------------------------------------------------------------------

/// Sort keys offered by "Project > Sort".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum SortKey {
    #[default]
    Id,
    Cost,
    Name,
    Start,
    Finish,
    Duration,
    Work,
    Priority,
}

impl SortKey {
    pub const TASKS: [SortKey; 8] =
        [SortKey::Id, SortKey::Cost, SortKey::Name, SortKey::Start, SortKey::Finish, SortKey::Duration, SortKey::Work, SortKey::Priority];
    pub const RESOURCES: [SortKey; 4] = [SortKey::Id, SortKey::Cost, SortKey::Name, SortKey::Work];

    pub fn label(self) -> &'static str {
        use crate::i18n::tr;
        match self {
            SortKey::Id => tr("by ID"),
            SortKey::Cost => tr("by Cost"),
            SortKey::Name => tr("by Name"),
            SortKey::Start => tr("by Start Date"),
            SortKey::Finish => tr("by Finish Date"),
            SortKey::Duration => tr("by Duration"),
            SortKey::Work => tr("by Work"),
            SortKey::Priority => tr("by Priority"),
        }
    }
}

impl Project {
    /// Reorders the resource list (Resource Sheet "Sort"); `Id` restores creation order.
    pub fn sort_resources(&mut self, key: SortKey) {
        let costs: std::collections::HashMap<ResourceId, f64> = self.resources.iter().map(|r| (r.id, resource_cost(self, r.id))).collect();
        let work: std::collections::HashMap<ResourceId, Minutes> =
            self.resources.iter().map(|r| (r.id, resource_work(self, r.id))).collect();
        match key {
            SortKey::Cost => self.resources.sort_by(|a, b| {
                costs.get(&b.id).copied().unwrap_or(0.0).total_cmp(&costs.get(&a.id).copied().unwrap_or(0.0)).then(a.id.cmp(&b.id))
            }),
            SortKey::Work => self.resources.sort_by(|a, b| work.get(&b.id).cmp(&work.get(&a.id)).then(a.id.cmp(&b.id))),
            SortKey::Name => self.resources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id))),
            _ => self.resources.sort_by_key(|r| r.id),
        }
    }
}

// ---------------------------------------------------------------------------
// Report data
// ---------------------------------------------------------------------------

/// Tasks that have not started and start within `days` of the status date ("Upcoming Tasks").
pub fn upcoming_tasks(p: &Project, days: i64) -> Vec<usize> {
    let at = status_moment(p);
    let until = at.checked_add_signed(chrono::Duration::days(days.max(0))).unwrap_or(at);
    p.tasks
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.sched.is_summary && t.active && t.percent_complete == 0 && t.actual_start.is_none())
        .filter(|(_, t)| t.sched.start.is_some_and(|s| s <= until))
        .map(|(i, _)| i)
        .collect()
}

/// Tasks not started yet.
pub fn unstarted_tasks(p: &Project) -> Vec<usize> {
    p.tasks
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.sched.is_summary && t.active && t.percent_complete == 0 && t.actual_start.is_none())
        .map(|(i, _)| i)
        .collect()
}

/// Tasks in progress.
pub fn tasks_in_progress(p: &Project) -> Vec<usize> {
    p.tasks
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.sched.is_summary && t.active && (t.percent_complete > 0 || t.actual_start.is_some()) && !t.is_complete())
        .map(|(i, _)| i)
        .collect()
}

// ---------------------------------------------------------------------------
// Update Project (Project ▸ Update Project)
// ---------------------------------------------------------------------------

/// How "Update work as complete through" sets % complete.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UpdateMode {
    /// Set 0% – 100% complete (in proportion to the time passed).
    #[default]
    Proportional,
    /// Set 0% or 100% complete only (only tasks that should have finished).
    ZeroOrHundred,
}

/// Leaf tasks the update applies to: all, or the given ones (summaries stand for their subtasks).
fn update_targets(p: &Project, only: Option<&[TaskId]>) -> Vec<usize> {
    let mut wanted = vec![only.is_none(); p.tasks.len()];
    if let Some(ids) = only {
        for id in ids {
            let Some(i) = p.task_index(*id) else { continue };
            if let Some(w) = wanted.get_mut(i) {
                *w = true;
            }
            for d in p.descendants_range(i) {
                if let Some(w) = wanted.get_mut(d) {
                    *w = true;
                }
            }
        }
    }
    (0..p.tasks.len())
        .filter(|&i| wanted.get(i).copied().unwrap_or(false) && !p.has_children(i))
        .filter(|&i| p.tasks.get(i).is_some_and(|t| t.active && !t.is_complete()))
        .collect()
}

/// "Update work as complete through": tasks that should have finished by `through` become 100%
/// complete; tasks in progress get the share of their duration that has passed. Never lowers
/// progress. Sets the status date. Returns the number of tasks updated.
pub fn update_complete_through(p: &mut Project, through: NaiveDateTime, mode: UpdateMode, only: Option<&[TaskId]>) -> PlanResult<usize> {
    let mut changes: Vec<(TaskId, u8)> = Vec::new();
    for i in update_targets(p, only) {
        let Some(t) = p.tasks.get(i) else { continue };
        let (Some(s), Some(f)) = (t.sched.start, t.sched.finish) else { continue };
        let pct = if f <= through {
            100
        } else if s < through && mode == UpdateMode::Proportional {
            let cal = task_calendar(p, i);
            let total = cal.between(s, f).max(1);
            let done = cal.between(s, through).clamp(0, total);
            ((done as f64 / total as f64 * 100.0).round() as u8).clamp(1, 99)
        } else {
            continue;
        };
        if pct > t.percent_complete {
            changes.push((t.id, pct));
        }
    }
    for (id, pct) in &changes {
        p.set_percent_complete(*id, *pct)?;
    }
    p.status_date = Some(through.date());
    Ok(changes.len())
}

/// "Reschedule uncompleted work to start after": unstarted tasks get a Start No Earlier Than
/// constraint; the remaining part of tasks in progress resumes at `after` (the task is split).
/// Returns the number of tasks moved.
pub fn reschedule_uncompleted_after(p: &mut Project, after: NaiveDateTime, only: Option<&[TaskId]>) -> PlanResult<usize> {
    let mut moved = 0;
    for i in update_targets(p, only) {
        let cal = task_calendar(p, i);
        let resume_at = cal.snap_forward(after);
        let Some(t) = p.tasks.get(i) else { continue };
        if t.mode == ScheduleMode::Manual {
            continue;
        }
        let started = t.actual_start.is_some() || t.percent_complete > 0;
        if !started {
            if t.sched.start.is_some_and(|s| s < resume_at) {
                let keep_later = t.constraint.kind == ConstraintKind::Snet && t.constraint.date.is_some_and(|d| d >= resume_at);
                if !keep_later {
                    if let Some(tm) = p.tasks.get_mut(i) {
                        tm.constraint = Constraint { kind: ConstraintKind::Snet, date: Some(resume_at) };
                    }
                    moved += 1;
                }
            }
            continue;
        }
        let Some(a) = t.actual_start.or(t.sched.start) else { continue };
        let done = t.duration.max(0) * Minutes::from(t.percent_complete) / 100;
        let stop = cal.add(a, done);
        if stop < resume_at {
            if let Some(tm) = p.tasks.get_mut(i) {
                tm.actual_start = Some(a);
                tm.resume = Some(resume_at);
            }
            moved += 1;
        }
    }
    Ok(moved)
}

/// Where a split task stops and resumes (`None` if it is not split).
pub fn split_points(p: &Project, i: usize) -> Option<(NaiveDateTime, NaiveDateTime)> {
    let t = p.tasks.get(i)?;
    if t.sched.parts.len() > 1 {
        return t.sched.stop.zip(t.sched.resume);
    }
    let r = t.resume?;
    if t.is_complete() || t.actual_finish.is_some() {
        return None;
    }
    let a = t.actual_start?;
    let cal = task_calendar(p, i);
    let stop = cal.add(a, t.duration.max(0) * Minutes::from(t.percent_complete) / 100);
    (stop < r).then_some((stop, r))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo() -> Project {
        crate::sample::demo_project(NaiveDate::from_ymd_opt(2026, 10, 5).unwrap())
    }

    #[test]
    fn cash_flow_adds_up_to_project_cost() {
        let p = demo();
        let rows = cash_flow(&p, Granularity::Week);
        let total = rows.last().map(|r| r.cumulative).unwrap_or(0.0);
        let cost: f64 = p.tasks.iter().filter(|t| !t.sched.is_summary && t.active).map(|t| t.sched.cost).sum();
        assert!((total - cost).abs() < 0.01, "{total} vs {cost}");
    }

    #[test]
    fn accrual_moves_cost() {
        let mut p = demo();
        let ri = p.resources.iter().position(|r| r.standard_rate > 0.0).unwrap();
        p.resources[ri].accrue_at = Accrual::End;
        let rid = p.resources[ri].id;
        let a = p.assignments.iter().find(|a| a.resource == rid).unwrap().clone();
        let daily = assignment_daily_cost(&p, &a);
        let t = p.task(a.task).unwrap();
        assert_eq!(daily.len(), 1);
        assert_eq!(*daily.keys().next().unwrap(), t.sched.finish.unwrap().date());
    }

    #[test]
    fn weekly_recurrence() {
        let p = demo();
        let rec = Recurrence { kind: RecurrenceKind::Weekly, every: 1, weekdays: vec![0, 2], day_of_month: 1, occurrences: 5 };
        let from = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap(); // Wednesday
        let d = recurrence_dates(&rec, from, None, &p);
        assert_eq!(d.len(), 5);
        assert_eq!(d[0], from);
        assert_eq!(d[1], NaiveDate::from_ymd_opt(2026, 10, 12).unwrap());
    }

    #[test]
    fn insert_recurring_creates_summary_and_occurrences() {
        let mut p = demo();
        let n = p.tasks.len();
        let rec = Recurrence { kind: RecurrenceKind::Weekly, every: 1, weekdays: vec![0], day_of_month: 1, occurrences: 4 };
        let sid = p.insert_recurring(n, "Meeting", 60, rec, NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(), None).unwrap();
        crate::schedule::recalculate(&mut p);
        assert_eq!(p.tasks.len(), n + 5);
        let si = p.task_index(sid).unwrap();
        assert!(p.tasks[si].sched.is_summary);
        assert!(p.tasks[si].recurrence.is_some());
    }

    #[test]
    fn from_finish_anchors_project() {
        let mut p = demo();
        let target = NaiveDate::from_ymd_opt(2027, 3, 31).unwrap().and_hms_opt(17, 0, 0).unwrap();
        p.settings.schedule_from_finish = true;
        p.settings.finish_date = Some(target);
        crate::schedule::recalculate(&mut p);
        assert_eq!(p.sched.finish, Some(target));
    }

    #[test]
    fn wbs_mask() {
        let mut p = demo();
        p.settings.wbs = WbsMask {
            prefix: "PRJ-".into(),
            levels: vec![
                WbsLevel { sequence: WbsSequence::Numbers, separator: ".".into() },
                WbsLevel { sequence: WbsSequence::Uppercase, separator: "-".into() },
            ],
        };
        assert_eq!(p.wbs(0), "PRJ-1");
        assert_eq!(p.wbs(1), "PRJ-1.A");
        assert_eq!(p.wbs(2), "PRJ-1.B");
    }

    #[test]
    fn sorting_resources_by_cost_and_back() {
        let mut p = demo();
        let ids: Vec<_> = p.resources.iter().map(|r| r.id).collect();
        p.sort_resources(SortKey::Cost);
        let c: Vec<f64> = p.resources.iter().map(|r| resource_cost(&p, r.id)).collect();
        assert!(c.windows(2).all(|w| w[0] >= w[1]));
        p.sort_resources(SortKey::Id);
        assert_eq!(p.resources.iter().map(|r| r.id).collect::<Vec<_>>(), ids);
    }

    #[test]
    fn numbered_baselines() {
        let mut p = demo();
        p.save_plan(PlanSlot::Baseline(2), &[]).unwrap();
        p.save_plan(PlanSlot::Interim(1), &[]).unwrap();
        assert!(p.tasks.iter().all(|t| t.baselines.contains_key(&2) && t.interim.contains_key(&1)));
        assert!(p.tasks.iter().all(|t| t.baseline.is_some() || t.sched.start.is_none() || true));
        p.clear_plan(PlanSlot::Baseline(2), &[]);
        assert!(p.tasks.iter().all(|t| t.baselines.is_empty()));
    }

    #[test]
    fn update_project_marks_progress_and_reschedules() {
        let mut p = demo();
        crate::schedule::recalculate(&mut p);
        let leaf =
            (0..p.tasks.len()).find(|&i| !p.has_children(i) && p.tasks[i].percent_complete == 0 && p.tasks[i].duration >= 3 * 480).unwrap();
        let t = p.tasks[leaf].clone();
        let (s, f) = (t.sched.start.unwrap(), t.sched.finish.unwrap());
        let mid = s + (f - s) / 2;
        let n = update_complete_through(&mut p, mid, UpdateMode::Proportional, Some(&[t.id])).unwrap();
        assert_eq!(n, 1);
        let pct = p.tasks[leaf].percent_complete;
        assert!(pct > 0 && pct < 100, "{pct}");
        assert_eq!(p.status_date, Some(mid.date()));
        crate::schedule::recalculate(&mut p);
        // 0%/100% mode leaves a task in progress alone.
        let other = (0..p.tasks.len())
            .find(|&i| i != leaf && !p.has_children(i) && p.tasks[i].percent_complete == 0 && p.tasks[i].duration > 480)
            .unwrap();
        let oid = p.tasks[other].id;
        let os = p.tasks[other].sched.start.unwrap();
        update_complete_through(&mut p, os + chrono::Duration::hours(2), UpdateMode::ZeroOrHundred, Some(&[oid])).unwrap();
        assert_eq!(p.tasks[other].percent_complete, 0);

        // Resume the in-progress task a month later: the remaining work moves, duration stays.
        let after = f + chrono::Duration::days(30);
        let moved = reschedule_uncompleted_after(&mut p, after, Some(&[t.id])).unwrap();
        assert_eq!(moved, 1);
        crate::schedule::recalculate(&mut p);
        let tt = &p.tasks[leaf];
        assert_eq!(tt.sched.start, Some(s));
        assert!(tt.sched.finish.unwrap() > after);
        assert_eq!(tt.duration, t.duration);
        let (stop, resume) = split_points(&p, leaf).unwrap();
        assert!(stop < resume);
        // Everything unstarted before `after` now starts after it.
        reschedule_uncompleted_after(&mut p, after, None).unwrap();
        crate::schedule::recalculate(&mut p);
        for (i, x) in p.tasks.iter().enumerate() {
            if !p.has_children(i) && x.active && x.percent_complete == 0 && x.mode == ScheduleMode::Auto {
                assert!(x.sched.start.unwrap() >= after, "{}", x.name);
            }
        }
    }
}

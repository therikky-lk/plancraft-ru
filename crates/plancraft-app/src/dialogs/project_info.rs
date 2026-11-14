//! Project Information and Options dialog.

use egui::{Context, RichText};
use plancraft_core::prelude::*;
use plancraft_core::workspace::Document;

use crate::edit::EditExt;
use crate::format;
use crate::theme;

fn duration_unit_name(unit: DurationUnit) -> String {
    if plancraft_core::i18n::is_ja() {
        return match unit {
            DurationUnit::Minutes => "分",
            DurationUnit::Hours => "時",
            DurationUnit::Days => "日",
            DurationUnit::Weeks => "週",
            DurationUnit::Months => "月",
        }
        .into();
    }
    crate::tr_dyn(&format!("{unit:?}"))
}

pub struct ProjectInfo {
    name: String,
    manager: String,
    company: String,
    author: String,
    notes: String,
    attachments: Vec<plancraft_core::attach::Attachment>,
    attach_status: Option<String>,
    start: String,
    status_date: String,
    calendar: CalendarId,
    currency: String,
    currency_decimals: u8,
    hours_per_day: String,
    hours_per_week: String,
    days_per_month: String,
    hours_per_point: String,
    unit: DurationUnit,
    new_auto: bool,
    use_res_cal: bool,
    progress_from_time: bool,
    critical_slack: String,
    from_finish: bool,
    finish: String,
    ev_baseline: u8,
    error: Option<String>,
}

impl ProjectInfo {
    pub fn new(p: &Project) -> Self {
        let s = &p.settings;
        Self {
            name: p.name.clone(),
            manager: p.manager.clone(),
            company: p.company.clone(),
            author: p.author.clone(),
            notes: p.notes.clone(),
            attachments: p.attachments.clone(),
            attach_status: None,
            start: format::edit_day(p.start.date()),
            status_date: p.status_date.map(format::edit_day).unwrap_or_default(),
            calendar: p.calendar,
            currency: s.currency.clone(),
            currency_decimals: s.currency_decimals.min(6),
            hours_per_day: format!("{}", s.time.minutes_per_day as f64 / 60.0),
            hours_per_week: format!("{}", s.time.minutes_per_week as f64 / 60.0),
            days_per_month: s.time.days_per_month.to_string(),
            hours_per_point: format!("{}", s.minutes_per_point as f64 / 60.0),
            unit: s.duration_unit,
            new_auto: s.new_tasks_auto,
            use_res_cal: s.use_resource_calendars,
            progress_from_time: s.progress_from_time,
            critical_slack: p.format_duration(s.critical_slack, false),
            from_finish: s.schedule_from_finish,
            finish: s.finish_date.or(p.sched.finish).map(|d| format::edit_day(d.date())).unwrap_or_default(),
            ev_baseline: s.earned_value_baseline,
            error: None,
        }
    }

    fn apply(&self, p: &mut Project) -> Result<(), String> {
        let num = |s: &str, what: &str| s.trim().parse::<f64>().map_err(|_| crate::tr_dyn(&format!("{what} must be a number")));
        p.name = self.name.trim().to_string();
        p.manager = self.manager.clone();
        p.company = self.company.clone();
        p.author = self.author.clone();
        p.notes = self.notes.clone();
        p.attachments = self.attachments.clone();
        if self.from_finish {
            let f = p.parse_datetime(&self.finish, true).ok_or(crate::tr("Finish date must look like YYYY-MM-DD"))?;
            p.settings.schedule_from_finish = true;
            p.settings.finish_date = Some(f);
        } else {
            p.settings.schedule_from_finish = false;
            p.start = p.parse_datetime(&self.start, false).ok_or(crate::tr("Start date must look like YYYY-MM-DD"))?;
        }
        p.status_date = if self.status_date.trim().is_empty() {
            None
        } else {
            Some(
                plancraft_core::i18n::parse_date(self.status_date.trim())
                    .ok_or(())
                    .map_err(|_| crate::tr("Status date must look like YYYY-MM-DD"))?,
            )
        };
        p.calendar = self.calendar;
        let s = &mut p.settings;
        s.currency = self.currency.trim().to_string();
        s.currency_decimals = self.currency_decimals.min(6);
        s.time.minutes_per_day = (num(&self.hours_per_day, crate::tr("Hours per day"))? * 60.0).round() as i64;
        s.time.minutes_per_week = (num(&self.hours_per_week, crate::tr("Hours per week"))? * 60.0).round() as i64;
        s.time.days_per_month = num(&self.days_per_month, crate::tr("Days per month"))?.round() as i64;
        s.minutes_per_point = (num(&self.hours_per_point, crate::tr("Hours per point"))? * 60.0).round() as i64;
        s.duration_unit = self.unit;
        s.new_tasks_auto = self.new_auto;
        s.use_resource_calendars = self.use_res_cal;
        s.progress_from_time = self.progress_from_time;
        let ts = s.time.clone();
        s.critical_slack =
            parse_duration(&self.critical_slack, DurationUnit::Days, &ts).map(|(m, _)| m).ok_or(crate::tr("Invalid critical slack"))?;
        if s.time.minutes_per_day <= 0 {
            return Err(crate::tr("Hours per day must be positive").into());
        }
        p.set_earned_value_baseline(self.ev_baseline).map_err(|e| e.to_string())
    }

    pub fn show(&mut self, ctx: &Context, doc: &mut Document) -> bool {
        let mut open = true;
        let mut close = false;
        super::window(ctx, crate::tr_dyn(&format!("Project Information for '{}'", doc.project.name)))
            .id(egui::Id::new("project_info"))
            .pivot(egui::Align2::CENTER_TOP)
            .default_pos(ctx.content_rect().center_top() + egui::vec2(0.0, 120.0))
            .open(&mut open)
            .collapsible(false)
            .default_width(600.0)
            .show(ctx, |ui| {
                egui::Grid::new("pi").num_columns(4).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label(crate::tr("Project name:"));
                    ui.text_edit_singleline(&mut self.name);
                    ui.label(crate::tr("Calendar:"));
                    let cal_name = doc.project.calendar(self.calendar).map(|c| c.name.clone()).unwrap_or_default();
                    egui::ComboBox::from_id_salt("pi_cal").selected_text(cal_name).show_ui(ui, |ui| {
                        for c in &doc.project.calendars {
                            ui.selectable_value(&mut self.calendar, c.id, &c.name);
                        }
                    });
                    ui.end_row();
                    ui.label(crate::tr("Schedule from:"));
                    egui::ComboBox::from_id_salt("pi_from")
                        .selected_text(if self.from_finish { crate::tr("Project Finish Date") } else { crate::tr("Project Start Date") })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.from_finish, false, crate::tr("Project Start Date"));
                            ui.selectable_value(&mut self.from_finish, true, crate::tr("Project Finish Date"));
                        });
                    ui.label(crate::tr("Status date:"));
                    crate::widgets::date_picker::date_input(ui, ui.id().with("project-status-date"), &mut self.status_date, 270.0);
                    ui.end_row();
                    ui.label(crate::tr("Start date:"));
                    if self.from_finish {
                        ui.label(RichText::new(format::date_long(doc.project.sched.start)).color(theme::pal().TEXT_WEAK));
                    } else {
                        crate::widgets::date_picker::date_input(ui, ui.id().with("project-start-date"), &mut self.start, 170.0);
                    }
                    ui.label(crate::tr("Finish date:"));
                    if self.from_finish {
                        crate::widgets::date_picker::date_input(ui, ui.id().with("project-finish-date"), &mut self.finish, 170.0);
                    } else {
                        ui.label(RichText::new(format::date_long(doc.project.sched.finish)).color(theme::pal().TEXT_WEAK));
                    }
                    ui.end_row();
                    ui.label(crate::tr("Manager:"));
                    ui.text_edit_singleline(&mut self.manager);
                    ui.end_row();
                    ui.label(crate::tr("Company:"));
                    ui.text_edit_singleline(&mut self.company);
                    ui.end_row();
                    ui.label(crate::tr("Author:"));
                    ui.text_edit_singleline(&mut self.author);
                    ui.end_row();
                });
                ui.label(crate::tr("Notes:"));
                ui.add(egui::TextEdit::multiline(&mut self.notes).desired_rows(3).desired_width(f32::INFINITY));
                if let Some(m) = super::extra::attachments_ui(ui, &mut self.attachments, "project") {
                    self.attach_status = Some(m);
                }
                if let Some(m) = &self.attach_status {
                    ui.label(RichText::new(m).color(theme::pal().TEXT_WEAK));
                }
                ui.add_space(6.0);
                egui::CollapsingHeader::new(RichText::new(crate::tr("Options: schedule, calendar & estimates")).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("pi_opts").num_columns(4).spacing([12.0, 8.0]).show(ui, |ui| {
                            ui.label(crate::tr("Hours per day:"));
                            ui.add(egui::TextEdit::singleline(&mut self.hours_per_day).desired_width(60.0));
                            ui.label(crate::tr("Hours per week:"));
                            ui.add(egui::TextEdit::singleline(&mut self.hours_per_week).desired_width(60.0));
                            ui.end_row();
                            ui.label(crate::tr("Days per month:"));
                            ui.add(egui::TextEdit::singleline(&mut self.days_per_month).desired_width(60.0));
                            ui.label(crate::tr("Hours per complexity point:"));
                            ui.add(egui::TextEdit::singleline(&mut self.hours_per_point).desired_width(60.0));
                            ui.end_row();
                            ui.label(crate::tr("Duration shown in:"));
                            egui::ComboBox::from_id_salt("pi_unit").selected_text(duration_unit_name(self.unit)).show_ui(ui, |ui| {
                                for u in [
                                    DurationUnit::Minutes,
                                    DurationUnit::Hours,
                                    DurationUnit::Days,
                                    DurationUnit::Weeks,
                                    DurationUnit::Months,
                                ] {
                                    ui.selectable_value(&mut self.unit, u, duration_unit_name(u));
                                }
                            });
                            ui.end_row();
                            ui.label(crate::tr("Critical if slack ≤"));
                            ui.add(egui::TextEdit::singleline(&mut self.critical_slack).desired_width(60.0));
                            ui.label("Earned value uses:");
                            egui::ComboBox::from_id_salt("pi_ev_baseline")
                                .selected_text(PlanSlot::Baseline(self.ev_baseline).name())
                                .show_ui(ui, |ui| {
                                    for n in 0..=plancraft_core::baseline::MAX_BASELINE {
                                        ui.selectable_value(&mut self.ev_baseline, n, PlanSlot::Baseline(n).name());
                                    }
                                });
                            ui.end_row();
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(crate::tr("Currency:")).strong());
                            ui.label(crate::tr("Currency symbol:"));
                            ui.add(egui::TextEdit::singleline(&mut self.currency).desired_width(55.0));
                            ui.label(crate::tr("Decimal places:"));
                            egui::ComboBox::from_id_salt("pi_currency_decimals").selected_text(self.currency_decimals.to_string()).show_ui(
                                ui,
                                |ui| {
                                    for n in 0..=6 {
                                        ui.selectable_value(&mut self.currency_decimals, n, n.to_string());
                                    }
                                },
                            );
                            ui.label(crate::tr("digits"));
                        });
                        ui.checkbox(&mut self.new_auto, crate::tr("New tasks are auto scheduled"));
                        ui.checkbox(
                            &mut self.use_res_cal,
                            crate::tr("Schedule assigned tasks on their resources' calendars (time off delays work)"),
                        );
                        ui.checkbox(&mut self.progress_from_time, crate::tr("Derive % complete from tracked time"));
                    });
                if let Some(e) = &self.error {
                    ui.colored_label(theme::pal().ERROR, e);
                }
                ui.separator();
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 34.0),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        if ui.button(crate::tr("Cancel")).clicked() {
                            close = true;
                        }
                        if ui
                            .add(
                                egui::Button::new(RichText::new(crate::tr("  OK  ")).color(egui::Color32::WHITE)).fill(theme::pal().ACCENT),
                            )
                            .clicked()
                        {
                            match doc.edit_msg(crate::tr("Project Information"), |p| self.apply(p)) {
                                Ok(()) => close = true,
                                Err(e) => self.error = Some(e),
                            }
                        }
                    },
                );
            });
        open && !close
    }
}

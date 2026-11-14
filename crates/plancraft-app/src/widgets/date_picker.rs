//! Small calendar popup shared by date fields and the task grid.

use chrono::{Datelike, Duration, NaiveDate};
use egui::{Align, Id, Ui, vec2};

use crate::{format, theme};

/// Shows a calendar button and returns a chosen date.
pub fn picker_button(ui: &mut Ui, id: Id, selected: Option<NaiveDate>) -> (Option<NaiveDate>, bool) {
    let response = ui.push_id(id, |ui| ui.small_button("📅")).inner.on_hover_text(crate::tr("Choose date"));
    let mut picked = None;
    let mut is_open = false;
    egui::Popup::menu(&response).show(|ui| {
        is_open = true;
        ui.set_width(230.0);
        ui.set_min_width(210.0);
        let today = chrono::Local::now().date_naive();
        let anchor = selected.unwrap_or(today);
        let first_of_anchor = NaiveDate::from_ymd_opt(anchor.year(), anchor.month(), 1).unwrap_or(anchor);
        let month_id = id.with("visible-month");
        let mut month = ui.ctx().data_mut(|data| data.get_temp::<NaiveDate>(month_id)).unwrap_or(first_of_anchor);

        ui.horizontal(|ui| {
            if ui.small_button("‹").clicked()
                && let Some(previous) = month.checked_sub_signed(Duration::days(1)).and_then(|day| day.with_day(1))
            {
                month = previous;
            }
            ui.add_space(8.0);
            ui.label(egui::RichText::new(format::df(&month, "%B %Y")).strong().color(theme::pal().TEXT));
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("›").clicked()
                    && let Some(next) = month.checked_add_signed(Duration::days(32)).and_then(|day| day.with_day(1))
                {
                    month = next;
                }
            });
        });
        ui.ctx().data_mut(|data| data.insert_temp(month_id, month));

        ui.horizontal(|ui| {
            for weekday in [
                chrono::Weekday::Mon,
                chrono::Weekday::Tue,
                chrono::Weekday::Wed,
                chrono::Weekday::Thu,
                chrono::Weekday::Fri,
                chrono::Weekday::Sat,
                chrono::Weekday::Sun,
            ] {
                ui.add_sized(
                    vec2(25.0, 20.0),
                    egui::Label::new(
                        egui::RichText::new(plancraft_core::i18n::weekday_letter(weekday)).small().color(theme::pal().TEXT_WEAK),
                    ),
                );
            }
        });

        if let Some(first_cell) = month.checked_sub_signed(Duration::days(month.weekday().num_days_from_monday() as i64)) {
            for week in 0..6 {
                ui.horizontal(|ui| {
                    for weekday in 0..7 {
                        let offset = week * 7 + weekday;
                        let Some(day) = first_cell.checked_add_signed(Duration::days(offset)) else {
                            continue;
                        };
                        let color = if day.month() == month.month() { theme::pal().TEXT } else { theme::pal().TEXT_WEAK };
                        let text = egui::RichText::new(day.day().to_string()).color(color);
                        if ui.add_sized(vec2(25.0, 23.0), egui::Button::new(text).selected(Some(day) == selected)).clicked() {
                            picked = Some(day);
                            ui.close();
                        }
                    }
                });
            }
        }
        ui.horizontal(|ui| {
            ui.add_space(25.0 * 5.0 + 8.0);
            if ui.small_button(crate::tr("Today")).clicked() {
                picked = Some(today);
                ui.close();
            }
        });
    });
    (picked, is_open)
}

/// A date text field that accepts ISO input while focused and uses the localized
/// date display after focus leaves. The adjacent calendar button offers selection.
pub fn date_input(ui: &mut Ui, id: Id, value: &mut String, width: f32) {
    ui.horizontal(|ui| {
        let text_width = (width - 30.0).max(70.0);
        let response =
            ui.add_sized(vec2(text_width, ui.spacing().interact_size.y), egui::TextEdit::singleline(value).id(id).hint_text("YYYY-MM-DD"));
        if response.gained_focus()
            && let Some(day) = plancraft_core::i18n::parse_date(value)
        {
            *value = day.format("%Y-%m-%d").to_string();
        } else if response.lost_focus()
            && let Some(day) = plancraft_core::i18n::parse_date(value)
        {
            *value = localized_display(day);
        }
        let selected = plancraft_core::i18n::parse_date(value);
        if let Some(day) = picker_button(ui, id.with("calendar"), selected).0 {
            *value = localized_display(day);
        }
    });
}

fn localized_display(day: NaiveDate) -> String {
    if plancraft_core::i18n::is_ja() {
        day.format("%Y/%m/%d").to_string()
    } else if plancraft_core::i18n::is_ru() {
        day.format("%d.%m.%Y").to_string()
    } else {
        day.format("%Y-%m-%d").to_string()
    }
}

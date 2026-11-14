//! Display formatting helpers.

use chrono::{Datelike, NaiveDate, NaiveDateTime};
use plancraft_core::i18n;
use plancraft_core::prelude::*;

/// Dates and date-times, for [`df`].
pub trait AsDateTime {
    fn as_dt(&self) -> NaiveDateTime;
}
impl AsDateTime for NaiveDateTime {
    fn as_dt(&self) -> NaiveDateTime {
        *self
    }
}
impl<T: AsDateTime> AsDateTime for &T {
    fn as_dt(&self) -> NaiveDateTime {
        (*self).as_dt()
    }
}
impl<Tz: chrono::TimeZone> AsDateTime for chrono::DateTime<Tz> {
    fn as_dt(&self) -> NaiveDateTime {
        self.naive_local()
    }
}
impl AsDateTime for NaiveDate {
    fn as_dt(&self) -> NaiveDateTime {
        self.and_time(chrono::NaiveTime::MIN)
    }
}

/// "Work" as a resource type (Russian: "Трудовой", not "Трудозатраты").
pub fn work_type_label() -> &'static str {
    if i18n::is_ru() { "Трудовой" } else { "Work" }
}

/// The Russian equivalent of a US-style display pattern (day before month, dots).
fn ru_pattern(pat: &str) -> &str {
    match pat {
        "%a %m/%d/%y" | "%a %-m/%-d/%y" => "%a %d.%m.%y",
        "%a %b %-d, %Y %H:%M" => "%a %-d %b %Y %H:%M",
        "%a %-m/%-d" => "%a %d.%m",
        "%-m/%-d" => "%d.%m",
        "%b %-d" => "%-d %b",
        "%b %-d, %Y" => "%-d %b %Y",
        "%a %b %-d" => "%a %-d %b",
        "%b %-d, '%y" => "%d.%m.%y",
        "%b '%y" => "%b %y",
        "%m/%d/%Y" => "%d.%m.%Y",
        other => other,
    }
}

/// Formats a date with a chrono pattern in the interface language.
pub fn df(d: &impl AsDateTime, pat: &str) -> String {
    let dt = d.as_dt();
    if i18n::is_ja() {
        // Localize the requested pattern without adding fields it did not ask for.
        // In particular, date-only grid cells must stay date-only.
        let has_weekday = pat.contains("%a");
        let pat = pat
            .replace("%a", "")
            .replace("%b %-d, %Y", "%Y年%-m月%-d日")
            .replace("%m/%d/%y", "%Y/%m/%d")
            .replace("%m/%d/%Y", "%Y/%m/%d")
            .replace("%b", "%-m月")
            .replace("%B", "%-m月");
        let formatted = dt.format(&pat).to_string();
        return if has_weekday { format!("{} {}", formatted.trim(), i18n::weekday_short(dt.weekday())) } else { formatted };
    }
    if !i18n::is_ru() {
        let pat = pat
            .replace("%a", i18n::weekday_short(dt.weekday()))
            .replace("%B", i18n::month_long(dt.month()))
            .replace("%b", i18n::month_short(dt.month()));
        return dt.format(&pat).to_string();
    }
    // Substitute names ourselves: chrono only knows English ones.
    let pat = ru_pattern(pat)
        .replace("%a", i18n::weekday_short(dt.weekday()))
        .replace("%B", i18n::month_long(dt.month()))
        .replace("%b", i18n::month_short(dt.month()));
    dt.format(&pat).to_string()
}

pub fn na() -> String {
    if i18n::is_ru() { "НД".into() } else { "NA".into() }
}

pub fn date(dt: Option<NaiveDateTime>) -> String {
    dt.map(|d| df(&d, "%a %m/%d/%y")).unwrap_or_else(na)
}

pub fn date_long(dt: Option<NaiveDateTime>) -> String {
    dt.map(|d| df(&d, "%a %b %-d, %Y %H:%M")).unwrap_or_else(na)
}

pub fn day(d: NaiveDate) -> String {
    df(&d, "%a %m/%d/%y")
}

/// Date and time for editing; parsed back by `Project::parse_datetime`.
pub fn iso(dt: Option<NaiveDateTime>) -> String {
    let pat = if i18n::is_ja() {
        "%Y/%m/%d %H:%M"
    } else if i18n::is_ru() {
        "%d.%m.%Y %H:%M"
    } else {
        "%Y-%m-%d %H:%M"
    };
    dt.map(|d| d.format(pat).to_string()).unwrap_or_default()
}

/// A date for editing.
pub fn iso_date(dt: Option<NaiveDateTime>) -> String {
    dt.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default()
}

/// A plain date for editing.
pub fn edit_day(d: NaiveDate) -> String {
    d.format(if i18n::is_ja() {
        "%Y/%m/%d"
    } else if i18n::is_ru() {
        "%d.%m.%Y"
    } else {
        "%Y-%m-%d"
    })
    .to_string()
}

pub fn money(p: &Project, v: f64) -> String {
    if !v.is_finite() {
        return String::new();
    }
    let neg = v < 0.0;
    let symbol = p.currency();
    let decimals = p.settings.currency_decimals.min(6);
    let scale = 10_i64.pow(decimals as u32);
    let scaled = v.abs() * scale as f64;
    let nearest = scaled.round();
    let rounded_value = if (scaled - nearest).abs() <= f64::EPSILON * scaled.abs().max(1.0) * 4.0 { nearest } else { scaled.ceil() };
    let units = rounded_value as i64;
    let whole = units / scale;
    let frac = units % scale;
    let mut s = whole.to_string();
    let mut i = s.len() as isize - 3;
    while i > 0 {
        s.insert(i as usize, ',');
        i -= 3;
    }
    if i18n::is_ru() {
        // 1 234,56 ₽
        let s = s.replace(',', "\u{a0}");
        let fraction = if decimals == 0 { String::new() } else { format!(",{frac:0width$}", width = decimals as usize) };
        return format!("{}{s}{fraction}\u{a0}{symbol}", if neg { "-" } else { "" });
    }
    let fraction = if decimals == 0 { String::new() } else { format!(".{frac:0width$}", width = decimals as usize) };
    format!("{}{symbol}{s}{fraction}", if neg { "-" } else { "" })
}

pub fn hours(mins: f64) -> String {
    let h = mins / 60.0;
    let u = if i18n::is_ru() { "ч" } else { "h" };
    if (h - h.round()).abs() < 0.05 {
        format!("{}{u}", h.round() as i64)
    } else {
        format!("{h:.1}{u}").replace('.', if i18n::is_ru() { "," } else { "." })
    }
}

pub fn percent(v: f32) -> String {
    format!("{}%", v.round() as i64)
}

pub fn units(u: f64) -> String {
    format!("{}%", (u * 100.0).round() as i64)
}

/// Parses "50%", "50", "0.5" into units (fraction).
pub fn parse_units(text: &str) -> Option<f64> {
    let t = text.trim();
    if let Some(p) = t.strip_suffix('%') {
        return i18n::parse_f64(p).map(|v| v / 100.0);
    }
    let v: f64 = i18n::parse_f64(t)?;
    let t = t.replace(',', ".");
    // Bare numbers are percentages ("50" = 50%) unless written as a small fraction ("0.5", "1.5").
    Some(if t.contains('.') && v <= 10.0 { v } else { v / 100.0 })
}

/// Parses money like "$1,200.50".
pub fn parse_money(text: &str) -> Option<f64> {
    let kept: String = text.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',' || *c == '-').collect();
    // The last separator is the decimal one when both appear ("1,200.50", "1.200,50");
    // a lone comma is decimal in Russian ("1200,50") and a thousands separator in English.
    let last_dot = kept.rfind('.');
    let last_comma = kept.rfind(',');
    let cleaned = match (last_dot, last_comma) {
        (Some(d), Some(c)) if c > d => kept.replace('.', "").replace(',', "."),
        (Some(_), Some(_)) => kept.replace(',', ""),
        (None, Some(_)) if i18n::is_ru() => kept.replace(',', "."),
        (None, Some(_)) => kept.replace(',', ""),
        _ => kept,
    };
    let v: f64 = cleaned.parse().ok()?;
    v.is_finite().then_some(v)
}

pub fn slack(p: &Project, m: Minutes) -> String {
    p.format_duration(m, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_and_money() {
        assert_eq!(parse_units("50%"), Some(0.5));
        assert_eq!(parse_units("150"), Some(1.5));
        assert_eq!(parse_units("0.25"), Some(0.25));
        assert_eq!(parse_money("$1,200.50"), Some(1200.5));
        let p = Project::new("x", chrono::NaiveDate::from_ymd_opt(2026, 1, 5).unwrap());
        assert_eq!(money(&p, 1234567.891), "$1,234,567.89");
        assert_eq!(money(&p, -5.0), "-$5.00");
    }
}

//! Embedded language catalogs and locale selection. Missing translations fall back to English.
use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU8, Ordering};

// Keep metadata and catalog registration together: the picker, persistence and lookup use this list.
macro_rules! languages {
    ($(($variant:ident, $code:literal, $legacy:literal, $name:literal)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
        pub enum Lang {
            #[default]
            #[serde(rename = "en-US", alias = "en")]
            En,
            $(#[serde(rename = $code, alias = $legacy)] $variant,)+
        }
        impl Lang {
            pub const ALL: &'static [Lang] = &[Self::En, $(Self::$variant,)+];
            pub fn code(self) -> &'static str {
                match self { Self::En => "en-US", $(Self::$variant => $code,)+ }
            }
            pub fn native_name(self) -> &'static str {
                match self { Self::En => "English (United States)", $(Self::$variant => $name,)+ }
            }
            fn catalog_json(self) -> &'static str {
                match self {
                    Self::En => include_str!("locales/en-US.json"),
                    $(Self::$variant => include_str!(concat!("locales/", $code, ".json")),)+
                }
            }
        }
    };
}
languages! {
    (Ja, "ja-JP", "ja", "日本語"),
    (ZhHans, "zh-Hans", "zh_hans", "简体中文"),
    (ZhHant, "zh-Hant", "zh_hant", "繁體中文"),
    (Ko, "ko-KR", "ko", "한국어"),
    (De, "de-DE", "de", "Deutsch"),
    (Fr, "fr-FR", "fr", "Français"),
    (Es, "es-ES", "es", "Español"),
    (PtBr, "pt-BR", "pt_br", "Português (Brasil)"),
    (Ru, "ru-RU", "ru", "Русский"),
}

impl Lang {
    /// BCP 47 or POSIX locale. An explicit Chinese script takes precedence over region.
    pub fn from_locale(locale: &str) -> Option<Self> {
        let normalized = locale.split(['.', '@', ':']).next()?.replace('_', "-").to_ascii_lowercase();
        let parts: Vec<_> = normalized.split('-').collect();
        match parts.first().copied()? {
            "zh" => Some(if parts.contains(&"hant") {
                Self::ZhHant
            } else if parts.contains(&"hans") {
                Self::ZhHans
            } else if parts.iter().any(|p| matches!(*p, "tw" | "hk" | "mo")) {
                Self::ZhHant
            } else {
                Self::ZhHans
            }),
            primary => Self::ALL.iter().copied().find(|l| l.code().split('-').next().is_some_and(|s| s == primary)),
        }
    }
    pub fn from_env() -> Self {
        ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"]
            .iter()
            .filter_map(|key| std::env::var(key).ok())
            .find(|v| !v.is_empty())
            .and_then(|v| Self::from_locale(&v))
            .unwrap_or_default()
    }
    /// Sorted English keys not yet present in this catalog, for translation maintenance.
    pub fn missing_translation_keys(self) -> Vec<&'static str> {
        let Some(english) = catalog(Self::En) else { return Vec::new() };
        let mut missing: Vec<_> =
            english.exact.keys().filter(|key| catalog(self).is_none_or(|c| !c.exact.contains_key(*key))).map(String::as_str).collect();
        missing.sort_unstable();
        missing
    }
    pub fn cjk_fonts(self) -> &'static [&'static str] {
        match self {
            Self::Ja => &["Yu Gothic UI", "Yu Gothic", "Meiryo", "Noto Sans CJK JP", "Noto Sans JP", "MS Gothic"],
            Self::ZhHans => &["Microsoft YaHei UI", "Microsoft YaHei", "PingFang SC", "Noto Sans CJK SC", "Noto Sans SC"],
            Self::ZhHant => &["Microsoft JhengHei UI", "Microsoft JhengHei", "PingFang TC", "Noto Sans CJK TC", "Noto Sans TC"],
            Self::Ko => &["Malgun Gothic", "Apple SD Gothic Neo", "Noto Sans CJK KR", "Noto Sans KR"],
            _ => &[],
        }
    }
}
static LANG: AtomicU8 = AtomicU8::new(0);
pub fn set_lang(lang: Lang) {
    let index = Lang::ALL.iter().position(|l| *l == lang).and_then(|i| u8::try_from(i).ok()).unwrap_or(0);
    LANG.store(index, Ordering::Relaxed);
}
pub fn lang() -> Lang {
    Lang::ALL.get(usize::from(LANG.load(Ordering::Relaxed))).copied().unwrap_or_default()
}
pub fn is_ru() -> bool {
    lang() == Lang::Ru
}
pub fn is_ja() -> bool {
    lang() == Lang::Ja
}

struct Catalog {
    exact: HashMap<String, String>,
    templates: Vec<(String, String)>,
}
impl Catalog {
    fn new(json: &str) -> Self {
        // Bundled input only; tests validate JSON and placeholders.
        let exact: HashMap<String, String> = serde_json::from_str(json).unwrap_or_default();
        let mut templates: Vec<_> = exact.iter().filter(|(k, _)| k.contains("{}")).map(|(k, v)| (k.clone(), v.clone())).collect();
        templates.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
        Self { exact, templates }
    }
    fn dynamic(&self, s: &str) -> String {
        if let Some(value) = self.exact.get(s) {
            return value.clone();
        }
        for (key, value) in &self.templates {
            if let Some(caps) = match_template(&key.split("{}").collect::<Vec<_>>(), s) {
                let mut out = String::new();
                for (i, part) in value.split("{}").enumerate() {
                    if i > 0 {
                        let cap = caps.get(i.saturating_sub(1)).copied().unwrap_or("");
                        out.push_str(self.exact.get(cap).map(String::as_str).unwrap_or(cap));
                    }
                    out.push_str(part);
                }
                return out;
            }
        }
        s.to_owned()
    }
}
static CATALOGS: LazyLock<HashMap<Lang, Catalog>> =
    LazyLock::new(|| Lang::ALL.iter().map(|l| (*l, Catalog::new(l.catalog_json()))).collect());
fn catalog(language: Lang) -> Option<&'static Catalog> {
    CATALOGS.get(&language)
}
pub fn tr(s: &'static str) -> &'static str {
    catalog(lang()).and_then(|c| c.exact.get(s)).map(String::as_str).unwrap_or(s)
}
pub fn untr(s: &str) -> &str {
    catalog(lang())
        .and_then(|c| c.exact.iter().filter(|(k, v)| !k.contains("{}") && v.as_str() == s).map(|(k, _)| k.as_str()).min())
        .unwrap_or(s)
}
pub fn tr_dyn(s: &str) -> String {
    catalog(lang()).map(|c| c.dynamic(s)).unwrap_or_else(|| s.to_owned())
}

/// Matches `s` against literal `parts` separated by wildcards; returns the wildcard captures.
pub(crate) fn match_template<'a>(parts: &[&str], s: &'a str) -> Option<Vec<&'a str>> {
    let (first, rest) = parts.split_first()?;
    let mut tail = s.strip_prefix(first)?;
    let mut caps = Vec::with_capacity(rest.len());
    for (i, part) in rest.iter().enumerate() {
        let last = i + 1 == rest.len();
        if last {
            let cap = tail.strip_suffix(part)?;
            if cap.is_empty() && !part.is_empty() && !first.is_empty() {
                // Allow empty captures; nothing special to do.
            }
            caps.push(cap);
            tail = "";
        } else {
            if part.is_empty() {
                return None;
            }
            let pos = tail.find(part)?;
            caps.push(tail.get(..pos)?);
            tail = tail.get(pos + part.len()..)?;
        }
    }
    if rest.is_empty() && !tail.is_empty() {
        return None;
    }
    Some(caps)
}

/// Short weekday name, Monday = 0.
pub fn weekday_short(wd: chrono::Weekday) -> &'static str {
    use chrono::Weekday::*;
    if is_ja() {
        match wd {
            Mon => "月",
            Tue => "火",
            Wed => "水",
            Thu => "木",
            Fri => "金",
            Sat => "土",
            Sun => "日",
        }
    } else if is_ru() {
        match wd {
            Mon => "Пн",
            Tue => "Вт",
            Wed => "Ср",
            Thu => "Чт",
            Fri => "Пт",
            Sat => "Сб",
            Sun => "Вс",
        }
    } else {
        tr(match wd {
            Mon => "Mon",
            Tue => "Tue",
            Wed => "Wed",
            Thu => "Thu",
            Fri => "Fri",
            Sat => "Sat",
            Sun => "Sun",
        })
    }
}

/// One-letter weekday for timescales.
pub fn weekday_letter(wd: chrono::Weekday) -> &'static str {
    use chrono::Weekday::*;
    if is_ja() {
        match wd {
            Mon => "月",
            Tue => "火",
            Wed => "水",
            Thu => "木",
            Fri => "金",
            Sat => "土",
            Sun => "日",
        }
    } else if is_ru() {
        match wd {
            Mon => "П",
            Tue => "В",
            Wed => "С",
            Thu => "Ч",
            Fri => "П",
            Sat => "С",
            Sun => "В",
        }
    } else {
        match wd {
            Mon => "M",
            Tue => "T",
            Wed => "W",
            Thu => "T",
            Fri => "F",
            Sat => "S",
            Sun => "S",
        }
    }
}

/// Short month name, January = 1.
pub fn month_short(m: u32) -> &'static str {
    const EN: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    const RU: [&str; 12] = ["янв", "фев", "мар", "апр", "май", "июн", "июл", "авг", "сен", "окт", "ноя", "дек"];
    const JA: [&str; 12] = ["1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月"];
    let i = (m.clamp(1, 12) - 1) as usize;
    if is_ja() {
        JA.get(i).copied().unwrap_or("")
    } else if is_ru() {
        RU.get(i).copied().unwrap_or("")
    } else {
        tr(EN.get(i).copied().unwrap_or(""))
    }
}

/// Full month name (nominative), January = 1.
pub fn month_long(m: u32) -> &'static str {
    const EN: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    const RU: [&str; 12] =
        ["Январь", "Февраль", "Март", "Апрель", "Май", "Июнь", "Июль", "Август", "Сентябрь", "Октябрь", "Ноябрь", "Декабрь"];
    const JA: [&str; 12] = ["1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月"];
    let i = (m.clamp(1, 12) - 1) as usize;
    if is_ja() {
        JA.get(i).copied().unwrap_or("")
    } else if is_ru() {
        RU.get(i).copied().unwrap_or("")
    } else {
        tr(EN.get(i).copied().unwrap_or(""))
    }
}

/// Parses a decimal number written with either a point or a comma ("1,5" or "1.5").
pub fn parse_f64(text: &str) -> Option<f64> {
    let t: String = text.trim().chars().filter(|c| !c.is_whitespace() && *c != '\u{a0}').collect();
    let t = if t.contains(',') && !t.contains('.') { t.replace(',', ".") } else { t };
    let v: f64 = t.parse().ok()?;
    v.is_finite().then_some(v)
}

/// Parses a date in ISO (`2026-10-19`), Russian (`19.10.2026`, `19.10.26`) or US (`10/19/2026`) form.
pub fn parse_date(text: &str) -> Option<chrono::NaiveDate> {
    use chrono::NaiveDate;
    let t = text.trim();
    // Drop a leading weekday such as "Пн " or "Mon ".
    let t = match t.split_once(' ') {
        Some((head, rest)) if head.chars().all(|c| c.is_alphabetic()) => rest.trim(),
        _ => t,
    };
    // Ignore a trailing time.
    let t = t.split_whitespace().next().unwrap_or("");
    if let Ok(d) = NaiveDate::parse_from_str(t, "%Y-%m-%d") {
        return Some(d);
    }
    if t.contains('.') {
        let mut it = t.split('.');
        let d: u32 = it.next()?.parse().ok()?;
        let m: u32 = it.next()?.parse().ok()?;
        let y: i32 = it.next()?.parse().ok()?;
        let y = if y < 100 { 2000 + y } else { y };
        return NaiveDate::from_ymd_opt(y, m, d);
    }
    if t.contains('/') {
        let mut it = t.split('/');
        let a = it.next()?;
        let b: u32 = it.next()?.parse().ok()?;
        let c: u32 = it.next()?.parse().ok()?;
        if a.len() == 4 {
            let y: i32 = a.parse().ok()?;
            return NaiveDate::from_ymd_opt(y, b, c);
        }
        let m: u32 = a.parse().ok()?;
        let y = if c < 100 { 2000 + c as i32 } else { c as i32 };
        return NaiveDate::from_ymd_opt(y, m, b);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogs_are_valid_and_preferences_remain_compatible() {
        for &language in Lang::ALL {
            let data: std::collections::BTreeMap<String, String> = serde_json::from_str(language.catalog_json()).unwrap();
            assert!(!data.is_empty());
            for (key, value) in &data {
                assert_eq!(key.matches("{}").count(), value.matches("{}").count(), "{}: {key}", language.code());
            }
            assert_eq!(Lang::from_locale(language.code()), Some(language));
            let serialized = serde_json::to_string(&language).unwrap();
            assert_eq!(serde_json::from_str::<Lang>(&serialized).unwrap(), language);
            let catalog = Catalog::new(language.catalog_json());
            assert_eq!(catalog.dynamic("Unknown message 123"), "Unknown message 123");
            assert_eq!(catalog.dynamic("Edit example"), data.get("Edit {}").unwrap().replace("{}", "example"));
        }
        for (code, expected) in [
            ("zh-TW", Lang::ZhHant),
            ("zh-HK", Lang::ZhHant),
            ("zh-Hans-TW", Lang::ZhHans),
            ("zh-Hant-CN", Lang::ZhHant),
            ("de_DE.UTF-8", Lang::De),
            ("pt-PT", Lang::PtBr),
        ] {
            assert_eq!(Lang::from_locale(code), Some(expected));
        }
        assert_eq!(Lang::from_locale("unknown"), None);
        for (old, language) in [("en", Lang::En), ("ru", Lang::Ru), ("ja", Lang::Ja)] {
            assert_eq!(serde_json::from_str::<Lang>(&format!("\"{old}\"")).unwrap(), language);
        }
    }

    #[test]
    fn templates_and_dates() {
        assert_eq!(match_template(&["Week of ", ""], "Week of Oct 5"), Some(vec!["Oct 5"]));
        assert_eq!(match_template(&["", " of ", "!"], "a of b!"), Some(vec!["a", "b"]));
        assert_eq!(match_template(&["x"], "y"), None);
        assert_eq!(parse_date("19.10.2026"), chrono::NaiveDate::from_ymd_opt(2026, 10, 19));
        assert_eq!(parse_date("Пн 19.10.26"), chrono::NaiveDate::from_ymd_opt(2026, 10, 19));
        assert_eq!(parse_date("2026-10-19"), chrono::NaiveDate::from_ymd_opt(2026, 10, 19));
        assert_eq!(parse_f64("1,5"), Some(1.5));
    }
}

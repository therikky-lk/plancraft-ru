//! Installed system fonts available for use in the interface.

use std::sync::{Arc, LazyLock};

use egui::FontFamily;

static FAMILIES: LazyLock<Vec<String>> = LazyLock::new(|| {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut names = db.faces().flat_map(|f| f.families.iter().map(|(n, _)| n.clone())).collect::<Vec<_>>();
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    names
});

pub fn families() -> &'static [String] {
    &FAMILIES
}

/// Install the selected system font as the first proportional and monospace face.
pub fn install(ctx: &egui::Context, family: &str) {
    let mut defs = ctx.fonts(|f| f.definitions().clone());
    defs.font_data.remove("user_selected");
    for families in defs.families.values_mut() {
        families.retain(|name| name != "user_selected");
    }
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    // Keep every supported CJK script available in the language picker, including
    // when the current language or selected font uses only Latin glyphs.
    for &language in plancraft_core::i18n::Lang::ALL {
        let Some(family) = language.cjk_fonts().iter().find(|candidate| FAMILIES.iter().any(|n| n.eq_ignore_ascii_case(candidate))) else {
            continue;
        };
        let key = format!("locale_{}", language.code());
        if defs.font_data.contains_key(&key) {
            continue;
        }
        if let Some(bytes) = font_bytes(&db, family) {
            defs.font_data.insert(key.clone(), Arc::new(egui::FontData::from_owned(bytes)));
            defs.families.entry(FontFamily::Proportional).or_default().push(key.clone());
            defs.families.entry(FontFamily::Monospace).or_default().push(key);
        }
    }
    let family = if family.is_empty() {
        plancraft_core::i18n::lang()
            .cjk_fonts()
            .iter()
            .find(|candidate| FAMILIES.iter().any(|n| n.eq_ignore_ascii_case(candidate)))
            .copied()
            .unwrap_or("")
    } else {
        family
    };
    if let Some(bytes) = font_bytes(&db, family) {
        defs.font_data.insert("user_selected".to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
        defs.families.entry(FontFamily::Proportional).or_default().insert(0, "user_selected".to_owned());
        defs.families.entry(FontFamily::Monospace).or_default().insert(0, "user_selected".to_owned());
    }
    ctx.set_fonts(defs);
}

fn font_bytes(db: &fontdb::Database, family: &str) -> Option<Vec<u8>> {
    if family.is_empty() {
        return None;
    }
    let id = db.query(&fontdb::Query {
        families: &[fontdb::Family::Name(family)],
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    })?;
    db.with_face_data(id, |data, _| data.to_vec())
}

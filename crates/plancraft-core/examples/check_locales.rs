//! Lists missing translations without changing the application language.
use plancraft_core::i18n::Lang;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(code) = std::env::args().nth(1) {
        let language = Lang::from_locale(&code).ok_or("unsupported locale")?;
        println!("{}", serde_json::to_string_pretty(&language.missing_translation_keys())?);
    } else {
        for &language in Lang::ALL {
            println!("{}: {} missing keys", language.code(), language.missing_translation_keys().len());
        }
    }
    Ok(())
}

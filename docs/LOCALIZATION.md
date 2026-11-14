# Localization

The supported locale registry is the `languages!` invocation in
`crates/plancraft-core/src/i18n.rs`. Each locale has a native name, stable BCP 47
persistence code, legacy alias and embedded JSON catalog in `src/locales/`.
`Lang::ALL` drives the Options dropdown. The application uses the OS UI locale
on first launch and saved preferences thereafter. Chinese script tags take
precedence over region; TW/HK/MO default to traditional Chinese. Unsupported
languages fall back to English. `PLANCRAFT_LANG` accepts these locale codes.

## Catalogs

English text is the stable key. Do not change a key merely to translate it.
All catalogs are compiled into the executable; no runtime files or network
requests are needed. Catalogs are loaded once and shared across switches.
Fixed strings, dynamic messages and reverse lookup use the same catalog.
Missing entries display the English original. Keep `{}` placeholders in the
same order and count. JSON keys must be unique. Prefer the most specific
message template; avoid ambiguous templates.

Japanese and Russian catalogs were migrated from the previous Rust tables, with
missing entries filled into their JSON catalogs. The seven new languages have
catalog entries for every English key. The added translations were generated
as first drafts with machine translation; a native speaker should review
terminology, context, and date/number wording before treating them as polished
localizations. The placeholder check ensures each `{}` remains available to
formatted messages, though it cannot assess translation quality.

Run `cargo run -p plancraft-core --example check_locales` for missing-key
counts. Append a locale, e.g. `-- de-DE`, to emit missing keys as JSON. Add
English keys when adding UI strings, then fill each locale's entries. Run
`cargo test -p plancraft-core --lib i18n` to validate catalog syntax,
placeholder counts, dynamic messages, locale resolution and settings aliases.
The settings tests in plancraft-app check persistence.

## Adding a language

1. Add a UTF-8 JSON file under `src/locales/` and register its enum variant,
   canonical code, alias and native name in `languages!`.
2. Add locale resolution rules only when language-subtag matching is
   insufficient (e.g. Chinese scripts). Existing language families match
   regional variants automatically.
3. Add system font candidates to `Lang::cjk_fonts` if the built-in font does
   not cover the script. Fonts are taken from the OS, not downloaded.
4. Translate keys, preserve placeholders, run checks, and visually review
   dropdown switching and representative dialogs on the target OS. Include
   missing fonts, long labels and a restart with a saved font and language.

Translation catalogs contain display strings only. Scheduling, file schema,
MCP tool names and document operations remain independent of catalog files.

## Upstream v0.4.1 integration

The integration adds Japanese UI copy for the new views and print controls.
Existing translations in the other locales remain intact; newly added keys
without a translation currently contain their English fallback. Equal key
counts and passing placeholder checks do not mean those new entries have been
translated. Review and translate those English entries before describing the
new features as fully localized in every supported language.

Saved task-table layout names use stable English keys independently of the
translated table caption, so changing the interface language keeps the same
custom columns. User-defined field names, choice values and board-column names
are project data. Default board columns use the language at project creation;
switching the language does not rename an existing project's columns.

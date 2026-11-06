# Thai (th) interface localization

The UI now offers **ไทย** via View > Interface language and uses Thai automatically on a fresh
installation when the system locale is `th` or `th-TH`.

The localization follows the existing GridCraft interface translation infrastructure, the same
shape as `docs/localization-el.md`:
- `crates/ui-egui/src/i18n/th.rs` supplies Thai translations of UI strings (the standard
  `fn translate(text: &str) -> Option<&'static str>` catalog shape; there is no JSON locale file).
- `crates/ui-egui/src/i18n/th_functions.rs` supplies Thai function-help descriptions and argument
  names for the argument hint, autocomplete, Insert Function and the Formulas-tab menus.
- `crates/ui-egui/src/i18n.rs` registers `Language::Th` (`th`, `ไทย`), the picker order, locale
  tags, persisted preferences and translation tests. `cargo xtask i18n-report` reports 0 missing
  keys and 0 stale function-help rows.
- Command identifiers, spreadsheet content, function names and references remain stable.
- Thai UI translation **does not add a Thai spelling dictionary**, a `th-TH` number/date locale,
  localized formula names or separators, or complex-text shaping.
- Unknown UI labels continue to fall back to the original English text.

Thai **printing and PDF export** are a separate, optional addition described in
[thai-printing.md](thai-printing.md). They use an OFL face supplied by the external
`storytold/craft-fonts` build input (`CRAFT_FONTS_DIR`); no font file is added to this repository.
The PDF path is independent of the interface language and works even when the UI is in English.

The Thai strings were produced specifically for GridCraft with machine assistance and review, not
extracted from proprietary spreadsheet software or localization assets. They have not yet had a
native-speaker review. The MIT OR Apache-2.0 repository licences apply to this contribution.

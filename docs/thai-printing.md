# Thai printing and PDF export

> **Status:** implemented in this fork · **GridCraft side:** no font bytes · **craft-fonts side:** one OFL face
> **Applies to:** `file.print`, `file.printPreview`, `file.exportPdf` and the desktop print spooler

## What was missing

`crates/pdf` is a self-contained PDF 1.7 writer. It uses the 14 standard fonts with
`WinAnsiEncoding` and, for characters those fonts lack, a predefined **non-embedded** Adobe-GB1
CID font (`STSong-Light` / `UniGB-UTF16-H`). A Thai string therefore reached the PDF as UTF-16BE
code units in a Chinese font that has no Thai glyphs: the text survived extraction but could not
be printed. `docs/localization-parity.md` recorded this as "PDF export uses only the 14 standard
PDF fonts: non-Latin text cannot export".

Thai now prints when the optional craft-fonts build input supplies an OFL Thai face. Nothing about
that relies on files inside the GridCraft repository.

## The standard file/folder layout

Fonts live in **[storytold/craft-fonts](https://github.com/storytold/craft-fonts)**, never in this
repo (`AGENTS.md` "Non-negotiables", `ATTRIBUTION.md`, `crates/ui-egui/build.rs`). The build input
is `CRAFT_FONTS_DIR=<absolute checkout>` and `/craft-fonts/` is gitignored. This fork follows that
mechanism exactly — no `assets/fonts/`, no new `xtask` asset rule, no licence file copied here.

```
storytold/craft-fonts/                       (separate repository)
└── fonts/
    ├── manifest.txt                        (eight fields, one new row)
    └── noto-sans-thai/                     (same folder-per-family pattern as the other faces)
        ├── NotoSansThai-Regular.ttf
        └── OFL.txt

storytold/gridcraft/                        (this repository)
├── crates/pdf/
│   ├── build.rs                            (reads the manifest row -> OUT_DIR/thai_font.rs)
│   ├── README.md                           (documents the optional input)
│   └── src/
│       ├── embed.rs                        (generic, script-agnostic TrueType reader)
│       ├── thai.rs                         (Thai CID mapping + the six PDF font objects)
│       ├── lib.rs                          (font slot, run splitting, width measurement)
│       └── tests.rs                        (both build modes)
└── docs/
    ├── thai-printing.md                    (this file)
    ├── localization-th.md                  (the UI language, same shape as localization-el.md)
    └── localization-parity.md              (status table)
```

### The manifest row upstream adds

`fonts/manifest.txt` is the same eight-field manifest `crates/ui-egui/build.rs` already reads:
`family | style | file | scripts | licence | licence_file | sha256 | source`. Script tags are four-letter
ISO 15924 codes (`Thai`), the hash is a full SHA-256, and the licence must be `OFL-1.1`:

```text
Noto Sans Thai | Regular | fonts/noto-sans-thai/NotoSansThai-Regular.ttf | Thai | OFL-1.1 | fonts/noto-sans-thai/OFL.txt | 404ddfb5ed0aaa6b6ec8a85700d682978992062d67da93903967b56cbd9a4acc | https://github.com/notofonts/noto-fonts
```

Face: [Noto Sans Thai Regular](https://fonts.google.com/noto/specimen/Noto+Sans+Thai),
37,752 bytes, SIL Open Font License 1.1; the licence text is the Google Fonts
`ofl/notosansthai/OFL.txt`. Keep the hash in the manifest in sync with the file.

Because the row is an ordinary, unbarred manifest row, the existing
`crates/ui-egui/build.rs` also embeds it for **native** screen rendering and
`font_setup.rs` appends it to every fallback chain. No UI build-script change is needed. It is
deliberately **not** added to `REQUIRED_FACES` or `WEB_ALLOWED`: an older craft-fonts checkout
without Thai must still build, so the face is optional even with `CRAFT_FONTS_REQUIRED=1`.

## How the GridCraft side works

1. `crates/pdf/build.rs` reads `CRAFT_FONTS_DIR/fonts/manifest.txt`, validates every row's shape,
   hash and script tags, and selects the exact `Noto Sans Thai / Regular / Thai / OFL-1.1` row. It
   writes `OUT_DIR/thai_font.rs` containing either `Some(include_bytes!(...))` or `None`. A missing
   or malformed row is a **warning**, never a silently embedded face and never a broken build. It
   uses the same `CARGO_CFG_TARGET_ARCH == "wasm32"` test as `crates/ui-egui/build.rs`, so a
   `CRAFT_FONTS_DIR` shared with a native release build cannot put Thai bytes into the WASM bundle.
   `PDF` is layer 0 and `ui-egui` layer 6, so this script is self-contained rather than reaching
   into `crates/ui-egui/src/craft_fonts_policy.rs`; both scripts read the same manifest.
2. `crates/pdf/src/embed.rs` is a dependency-free TrueType reader (`head`, `hhea`, `hmtx`, `OS/2`,
   `cmap` formats 4 and 12). It yields glyph ids, advance widths in 1/1000 em and
   `/FontDescriptor` metrics. It is script-agnostic: another script's embedded PDF face can reuse
   it without adding a PDF library.
3. `crates/pdf/src/thai.rs` maps CID `n` to Unicode `U+0E00 + n - 1` (CID 0 stays `.notdef`,
   128 CIDs cover the Thai block) and emits a Type0 / `Identity-H` font: descendant
   `CIDFontType2`, `/FontFile2`, `/CIDToGIDMap`, `/W`, `/FontDescriptor` and a `/ToUnicode` CMap.
   Text therefore **renders and can be copied or searched** in a PDF viewer.
4. `crates/pdf/src/lib.rs` reserves one extra font slot and splits text into WinAnsi / CID / Thai
   runs. `Font::char_width` uses the same advance widths as `/W`, so the engine's cell measurement,
   wrapping and alignment match what is drawn. The slot is marked used only when a face exists.
5. `gridcraft_pdf::thai_font_available()` reports which path a build took. The printing chain is
   unchanged: `crates/engine/src/cmd/print.rs` renders with `PdfDoc`, and
   `crates/ui-egui/src/app/print.rs` sends those same bytes to the spooler or browser print.

**An ordinary build is byte-identical to upstream.** With no `CRAFT_FONTS_DIR`, Thai still goes
through the old CID fallback, the extra slot stays unused and no new PDF objects are written. This
was verified against a clean `HEAD` worktree: the same deterministic document produced 2,146 bytes
with an identical SHA-256 before and after the change.

## Build and verify

```sh
# 1. Optional: the upstream craft-fonts checkout with the Thai row above.
export CRAFT_FONTS_DIR=/absolute/path/to/craft-fonts

# 2. Both modes must pass. Run the suite twice to exercise both paths.
cargo test -p gridcraft-pdf                       # no Thai face: legacy CID fallback
CRAFT_FONTS_DIR=/absolute/path/to/craft-fonts cargo test -p gridcraft-pdf   # embedded face

cargo fmt --check
cargo clippy -p gridcraft-pdf --all-targets -- -D warnings
cargo xtask ci

# 3. Web builds must not embed the face even when the variable is still exported.
rustup target add wasm32-unknown-unknown
cargo xtask wasm
CRAFT_FONTS_DIR=/absolute/path/to/craft-fonts cargo check --target wasm32-unknown-unknown -p gridcraft-pdf
# The wasm OUT_DIR/thai_font.rs must contain `THAI_FONT_BYTES: Option<&[u8]> = None`.
```

With the face present, a generated PDF should contain `/Encoding /Identity-H`,
`/Subtype /CIDFontType2`, `/FontFile2`, `/CIDToGIDMap`, `/W` and `/ToUnicode`. Text extraction
(`pypdf` or any viewer) must return the original Thai string, and rendering must show vowels and
tone marks above/below the correct consonants. `thai::tests::embedded_face_has_glyphs_and_positive_widths`
and `tests::embedded_thai_face_emits_a_complete_identity_font` are the extended acceptance tests;
they skip themselves in an ordinary build.

To print from the app: View › Interface language › ไทย is optional — the PDF font path is
independent of the interface language. File › Print and File › Export as PDF both use the same
`render_pdf`, so either produces the embedded font.

## What upstream takes from this fork

| Repository | Files | Notes |
|---|---|---|
| `storytold/craft-fonts` | `fonts/noto-sans-thai/{NotoSansThai-Regular.ttf,OFL.txt}`, one manifest row | The only place font bytes live. Add the SHA-256 and licence file. |
| `storytold/gridcraft` | `crates/pdf/build.rs`, `crates/pdf/src/embed.rs`, `crates/pdf/src/thai.rs` | New files, no new dependencies. |
| `storytold/gridcraft` | `crates/pdf/src/lib.rs`, `crates/pdf/src/tests.rs`, `crates/pdf/README.md`, `crates/pdf/Cargo.toml` | Modified. |
| `storytold/gridcraft` | `crates/ui-egui/src/i18n.rs`, `crates/ui-egui/src/i18n/th.rs`, `crates/ui-egui/src/i18n/th_functions.rs` | The Thai interface language; see `localization-th.md`. |
| `storytold/gridcraft` | `docs/thai-printing.md`, `docs/localization-th.md`, `docs/localization-parity.md`, `ATTRIBUTION.md`, `ROADMAP.md` | Documentation. |

The two halves are independent: upstream can merge the Thai **UI** catalogs alone, the **PDF**
support alone (given the craft-fonts row), or both. Merging the GridCraft patches without the
craft-fonts row is safe — Thai PDF text simply keeps the current fallback.

## Known limits

- The PDF writer maps Unicode directly to glyph ids. Thai vowels and tone marks render correctly
  because their glyphs are zero-width and combine by design, but there is no general OpenType
  shaping, so this approach does not extend to Arabic or Devanagari joining/reordering.
- Web/WASM builds keep their existing `WEB_ALLOWED` allowlist, so the Thai face is not in the
  browser bundle and web PDF export keeps the old fallback. `crates/pdf/build.rs` enforces this
  explicitly rather than relying on the caller unsetting `CRAFT_FONTS_DIR`. Desktop screen Thai
  still has the system-font fallback from #38.
- No `th-TH` number/date/formula locale, localized Thai function names or separators, and no
  native-speaker review of the Thai catalog yet. These are tracked in `localization-parity.md`.
- The embedded face is not subsetted; the full 37 KB TTF is written into PDFs that use Thai text.
  Subsetting for every script remains a ROADMAP item.

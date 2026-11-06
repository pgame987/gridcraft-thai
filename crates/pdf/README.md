# gridcraft-pdf

A small, dependency-free PDF 1.7 writer for vector drawing and RGBA images. Text uses the 14
standard fonts with `WinAnsiEncoding`, Symbol's built-in encoding, and a predefined non-embedded
Adobe-GB1 CID font for other scripts.

## Optional Thai face for print and PDF export

`CRAFT_FONTS_DIR=<absolute storytold/craft-fonts checkout>` is an optional build input, the same
one `crates/ui-egui/build.rs` uses. When its `fonts/manifest.txt` lists the OFL `Noto Sans Thai`
face, `build.rs` generates a font table and Thai text in `file.print` / `file.exportPdf` output is
set in an embedded Type0 / `Identity-H` font with a `/ToUnicode` CMap, so it prints and can be
copied out of the PDF. Without the input, Thai keeps the previous CID fallback and the PDF is
byte-identical to a build without this feature. No font bytes live in this repository.

`gridcraft_pdf::thai_font_available()` reports which path the build took. See
[docs/thai-printing.md](../../docs/thai-printing.md) for the manifest row, the file layout and the
verification recipe.

## API migration

The geometry-taking methods now use `Point` and `Rect` values to name their coordinates, and
`image_rgba` groups its pixel dimensions into an array. Update calls to the four changed methods as
follows:

```rust
use gridcraft_pdf::{Font, PdfDoc, Point, Rect};

let mut doc = PdfDoc::new();
let page = doc.add_page(612.0, 792.0);

// Before: page.link_page(20.0, 30.0, 80.0, 16.0, 1, 40.0, 50.0);
page.link_page(Rect::new(20.0, 30.0, 80.0, 16.0), 1, Point::new(40.0, 50.0));

// Before: page.stroke_line_dashed(0.0, 0.0, 100.0, 40.0, 1.0, [0, 0, 0], &[4.0, 2.0]);
page.stroke_line_dashed(Point::new(0.0, 0.0), Point::new(100.0, 40.0), 1.0, [0, 0, 0], &[4.0, 2.0]);

// Before: page.text_rotated(20.0, 40.0, 12.0, Font::Helvetica, [0, 0, 0], "Label", 0.2);
page.text_rotated(Point::new(20.0, 40.0), 12.0, Font::Helvetica, [0, 0, 0], "Label", 0.2);

let rgba = [255, 0, 0, 255];
// Before: page.image_rgba(10.0, 10.0, 24.0, 24.0, &rgba, 1, 1);
page.image_rgba(Rect::new(10.0, 10.0, 24.0, 24.0), &rgba, [1, 1]);
```

`Point::new(x, y)` uses the page's top-left origin, where y increases downward. `Rect::new(x, y,
width, height)` describes the destination or link rectangle in the same coordinate system. The
`image_rgba` pixel-size array is `[width, height]`.

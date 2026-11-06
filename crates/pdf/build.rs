//! Optional craft-fonts build input for an embedded PDF text face.
//!
//! GridCraft never stores font bytes in this repository (`AGENTS.md` "Non-negotiables",
//! `ATTRIBUTION.md`). `CRAFT_FONTS_DIR=<absolute craft-fonts checkout>` is the only input, exactly
//! as in `crates/ui-egui/build.rs`. When the checkout's `fonts/manifest.txt` lists the permitted
//! Thai face, this script writes `OUT_DIR/thai_font.rs` with the bytes through `include_bytes!`;
//! otherwise the table is `None` and Thai PDF text keeps the pre-existing non-embedded CID
//! fallback, so an ordinary build produces byte-identical PDFs.
//!
//! The face is optional even for `CRAFT_FONTS_REQUIRED=1` builds: an older craft-fonts checkout
//! without Thai must still build a working PDF writer. `docs/thai-printing.md` has the manifest
//! row upstream adds, the folder layout and the verification recipe.
//!
//! Web builds never embed it: `crates/ui-egui/build.rs` restricts wasm to the explicit
//! `WEB_ALLOWED` allowlist, and Thai is not on it. Sharing `CRAFT_FONTS_DIR` between a native
//! release build and a web build therefore cannot pull Thai glyph bytes into the WASM bundle.
//!
//! This build script is deliberately self-contained rather than importing
//! `crates/ui-egui/src/craft_fonts_policy.rs`: `pdf` is layer 0 and `ui-egui` is layer 6, so a
//! lower layer must not reach into a higher layer's source tree. Both scripts read the *same*
//! eight-field manifest, and this one repeats only the strictness that applies to its one face:
//! no empty fields, a 64-hex SHA-256, four-letter ISO 15924 script tags, and the exact licensed
//! family and style.

use std::path::{Path, PathBuf};

/// The exact family, style, ISO 15924 script tag and licence the PDF writer may embed. This is the
/// row `docs/thai-printing.md` asks upstream `storytold/craft-fonts` to add.
const FAMILY: &str = "Noto Sans Thai";
const STYLE: &str = "Regular";
const SCRIPT: &str = "Thai";
const LICENCE: &str = "OFL-1.1";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=CRAFT_FONTS_DIR");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default()).join("thai_font.rs");
    // Same target test as `crates/ui-egui/build.rs`: a web build embeds only `WEB_ALLOWED` faces,
    // and Thai is deliberately not one of them. `CRAFT_FONTS_DIR` may still be set for a sibling
    // native release build, so it must not leak Thai bytes into the WASM binary.
    let wasm = std::env::var("CARGO_CFG_TARGET_ARCH").is_ok_and(|a| a == "wasm32");
    let dir = if wasm { None } else { std::env::var_os("CRAFT_FONTS_DIR").map(PathBuf::from) };
    let face = dir.as_ref().and_then(|dir| match thai_face(dir) {
        Ok(face) => face,
        Err(e) => {
            println!("cargo::warning=building without an embedded Thai PDF face: {e}");
            None
        }
    });
    if dir.is_some() && face.is_none() {
        println!(
            "cargo::warning=CRAFT_FONTS_DIR has no '{FAMILY} {STYLE}' / {SCRIPT} / {LICENCE} row; Thai PDF text uses the non-embedded CID fallback"
        );
    }
    let src = match face {
        Some((path, name)) => {
            println!("cargo::rerun-if-changed={}", path.display());
            format!("pub static THAI_FONT_BYTES: Option<&[u8]> = Some(include_bytes!({path:?}));\npub static THAI_FONT_PS_NAME: &str = {name:?};\n")
        }
        None => String::from("pub static THAI_FONT_BYTES: Option<&[u8]> = None;\npub static THAI_FONT_PS_NAME: &str = \"NotoSansThai-Regular\";\n"),
    };
    if let Err(e) = std::fs::write(&out, src) {
        println!("cargo::error=writing {}: {e}", out.display());
    }
}

/// The permitted Thai row of `fonts/manifest.txt`: its canonical file path and PDF base name.
/// This is the same eight-field manifest `crates/ui-egui/build.rs` reads, reduced to the fields a
/// single embedded face needs. A malformed row is a warning, never a silently embedded face.
fn thai_face(dir: &Path) -> Result<Option<(PathBuf, String)>, String> {
    let manifest = dir.join("fonts/manifest.txt");
    println!("cargo::rerun-if-changed={}", manifest.display());
    let text = std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    for (number, line) in text.lines().map(str::trim).enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split(" | ").map(str::trim).collect();
        let [family, style, file, scripts, licence, licence_file, sha256, source] = f.as_slice() else {
            return Err(format!("{}:{}: expected 8 fields separated by \" | \"", manifest.display(), number + 1));
        };
        if [family, style, file, scripts, licence, licence_file, sha256, source].iter().any(|v| v.is_empty()) {
            return Err(format!("{}:{}: an empty field", manifest.display(), number + 1));
        }
        if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("{}:{}: sha256 must be 64 hex digits", manifest.display(), number + 1));
        }
        let tags: Vec<&str> = scripts.split(',').map(str::trim).collect();
        if tags.iter().any(|tag| tag.len() != 4 || !tag.bytes().all(|b| b.is_ascii_alphabetic())) {
            return Err(format!("{}:{}: script tags must be ISO 15924 four-letter tags", manifest.display(), number + 1));
        }
        if *family != FAMILY || *style != STYLE {
            continue;
        }
        if !tags.contains(&SCRIPT) {
            return Err(format!("{FAMILY} {STYLE} must declare the {SCRIPT} script tag"));
        }
        if *licence != LICENCE {
            return Err(format!("{FAMILY} {STYLE} must be {LICENCE}, not {licence}"));
        }
        let path = dir.join(file).canonicalize().map_err(|e| format!("{file}: {e}"))?;
        return Ok(Some((path, postscript_name(family, style))));
    }
    Ok(None)
}

/// A PDF `/BaseFont` name: no spaces or punctuation outside the ASCII name alphabet.
fn postscript_name(family: &str, style: &str) -> String {
    let raw = format!("{family}-{style}");
    let name: String = raw.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    if name.is_empty() { "EmbeddedFont".to_string() } else { name }
}

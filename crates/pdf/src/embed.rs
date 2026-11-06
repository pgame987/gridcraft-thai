//! A tiny, dependency-free TrueType reader for embedded PDF fonts.
//!
//! PDF `/FontFile2` needs the face bytes, a `/CIDToGIDMap` (Unicode → glyph id), advance widths
//! scaled to 1/1000 em and a few `/FontDescriptor` numbers. This module reads just those from
//! `head`, `hhea`, `hmtx`, `OS/2` and `cmap` formats 4 and 12. It is deliberately script-agnostic:
//! [`crate::thai`] is one consumer, and ROADMAP's "embedded subset fonts in PDF" can reuse it for
//! other scripts. Nothing indexes unchecked: a truncated or unusable face yields no [`Face`]
//! rather than a panic.
//!
//! No font bytes live in this repository. The bytes come from the optional craft-fonts build
//! input (`CRAFT_FONTS_DIR`), see `build.rs` and `docs/thai-printing.md`.

fn u16be(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*b.get(o)?, *b.get(o + 1)?]))
}

fn i16be(b: &[u8], o: usize) -> Option<i16> {
    Some(i16::from_be_bytes([*b.get(o)?, *b.get(o + 1)?]))
}

fn u32be(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes([*b.get(o)?, *b.get(o + 1)?, *b.get(o + 2)?, *b.get(o + 3)?]))
}

/// A TrueType table's bytes, looked up in the sfnt directory.
fn table<'a>(font: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    let count = usize::from(u16be(font, 4)?);
    for i in 0..count {
        let rec = 12 + i * 16;
        if font.get(rec..rec + 4)? == tag {
            let off = u32be(font, rec + 8)? as usize;
            let len = u32be(font, rec + 12)? as usize;
            return font.get(off..off.saturating_add(len));
        }
    }
    None
}

/// Glyph id of `code` in a `cmap` subtable of format 4 (segmented BMP mapping).
fn cmap4(sub: &[u8], code: u32) -> Option<u16> {
    if code > 0xFFFF {
        return None;
    }
    let code = code as u16;
    let seg_count = (u16be(sub, 6)? / 2) as usize;
    let ends = 14;
    let starts = ends + seg_count * 2 + 2;
    let deltas = starts + seg_count * 2;
    let ranges = deltas + seg_count * 2;
    for i in 0..seg_count {
        let end = u16be(sub, ends + i * 2)?;
        let start = u16be(sub, starts + i * 2)?;
        if code < start || code > end {
            continue;
        }
        let delta = i16be(sub, deltas + i * 2)?;
        let range = u16be(sub, ranges + i * 2)?;
        if range == 0 {
            return Some((code as i32 + i32::from(delta)).rem_euclid(65_536) as u16);
        }
        let at = ranges + i * 2 + usize::from(range) + 2 * usize::from(code - start);
        let gid = u16be(sub, at)?;
        return Some(if gid == 0 { 0 } else { (gid as i32 + i32::from(delta)).rem_euclid(65_536) as u16 });
    }
    None
}

/// Glyph id of `code` in a `cmap` subtable of format 12 (grouped UCS-4 mapping).
fn cmap12(sub: &[u8], code: u32) -> Option<u16> {
    let groups = u32be(sub, 12)? as usize;
    for i in 0..groups {
        let at = 16 + i * 12;
        let start = u32be(sub, at)?;
        let end = u32be(sub, at + 4)?;
        if (start..=end).contains(&code) {
            let gid = u32be(sub, at + 8)? + (code - start);
            return u16::try_from(gid).ok();
        }
    }
    None
}

/// The metrics and character map of a static TrueType face.
pub(crate) struct Face<'a> {
    font: &'a [u8],
    units: u16,
    bbox: [i32; 4],
    ascent: i32,
    descent: i32,
    cap_height: i32,
    advances: Vec<u16>,
    last: u16,
}

impl<'a> Face<'a> {
    /// Parses the tables an embedded `CIDFontType2` font needs.
    pub(crate) fn parse(font: &'a [u8]) -> Option<Self> {
        let head = table(font, b"head")?;
        let hhea = table(font, b"hhea")?;
        let hmtx = table(font, b"hmtx")?;
        let units = u16be(head, 18)?;
        if units == 0 {
            return None;
        }
        let scale = |v: i32| -> i32 { v.saturating_mul(1000) / i32::from(units) };
        let bbox = [
            scale(i32::from(i16be(head, 36)?)),
            scale(i32::from(i16be(head, 38)?)),
            scale(i32::from(i16be(head, 40)?)),
            scale(i32::from(i16be(head, 42)?)),
        ];
        let ascent = scale(i32::from(i16be(hhea, 4)?));
        let descent = scale(i32::from(i16be(hhea, 6)?));
        let cap_height =
            table(font, b"OS/2").and_then(|os2| i16be(os2, 88)).map(|v| scale(i32::from(v))).filter(|v| *v > 0).unwrap_or_else(|| scale(700));
        let num_metrics = usize::from(u16be(hhea, 34)?).max(1);
        let mut advances = Vec::with_capacity(num_metrics);
        for i in 0..num_metrics {
            advances.push(u16be(hmtx, i * 4).unwrap_or(0));
        }
        let last = advances.last().copied().unwrap_or(0);
        Some(Self { font, units, bbox, ascent, descent, cap_height, advances, last })
    }

    /// The face's glyph id for a Unicode scalar, preferring a UCS-4 subtable when it has one.
    pub(crate) fn glyph_id(&self, code: u32) -> u16 {
        let Some(cmap) = table(self.font, b"cmap") else { return 0 };
        let count = usize::from(u16be(cmap, 2).unwrap_or(0));
        for wanted in [12u16, 4u16] {
            for i in 0..count {
                let rec = 4 + i * 8;
                let Some(off) = u32be(cmap, rec + 4).map(|v| v as usize) else { continue };
                let Some(sub) = cmap.get(off..) else { continue };
                if u16be(sub, 0) != Some(wanted) {
                    continue;
                }
                let found = if wanted == 12 { cmap12(sub, code) } else { cmap4(sub, code) };
                if let Some(gid) = found {
                    return gid;
                }
            }
        }
        0
    }

    /// The advance width of a glyph id in 1/1000 em, for both `/W` and text measurement.
    pub(crate) fn width(&self, gid: u16) -> u16 {
        let advance = self.advances.get(usize::from(gid)).copied().unwrap_or(self.last);
        ((u32::from(advance) * 1000) / u32::from(self.units)).min(u32::from(u16::MAX)) as u16
    }

    /// The `/FontDescriptor` entries that come from the face's metrics.
    pub(crate) fn descriptor_entries(&self) -> String {
        format!(
            "/FontBBox [{} {} {} {}] /ItalicAngle 0 /Ascent {} /Descent {} /CapHeight {} /StemV 80",
            self.bbox[0], self.bbox[1], self.bbox[2], self.bbox[3], self.ascent, self.descent, self.cap_height
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn big_endian_readers_are_bounds_checked() {
        assert_eq!(u16be(&[0x12, 0x34], 0), Some(0x1234));
        assert_eq!(u32be(&[0x12, 0x34, 0x56, 0x78], 0), Some(0x1234_5678));
        assert_eq!(i16be(&[0xFF, 0xFE], 0), Some(-2));
        assert_eq!(u16be(&[0x12], 0), None);
        assert_eq!(u32be(&[0x12, 0x34, 0x56], 0), None);
    }

    #[test]
    fn sfnt_directory_finds_a_table() {
        let mut font = vec![0u8; 28];
        font[4..6].copy_from_slice(&1u16.to_be_bytes());
        font[12..16].copy_from_slice(b"head");
        font[20..24].copy_from_slice(&28u32.to_be_bytes());
        font[24..28].copy_from_slice(&4u32.to_be_bytes());
        font.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);
        assert_eq!(table(&font, b"head"), Some(&[0xAA, 0xBB, 0xCC, 0xDD][..]));
        assert_eq!(table(&font, b"hhea"), None);
        assert!(Face::parse(&font).is_none(), "a face with no metrics tables is rejected");
    }

    #[test]
    fn cmap4_maps_a_segment_through_its_delta() {
        let mut sub = vec![0u8; 26];
        sub[0..2].copy_from_slice(&4u16.to_be_bytes());
        sub[6..8].copy_from_slice(&2u16.to_be_bytes()); // segCountX2 = 2 (one segment)
        sub[14..16].copy_from_slice(&0x0E7Fu16.to_be_bytes()); // endCode
        sub[18..20].copy_from_slice(&0x0E00u16.to_be_bytes()); // startCode
        sub[20..22].copy_from_slice(&(-3579i16).to_be_bytes()); // idDelta: 0x0E00 -> gid 5
        assert_eq!(cmap4(&sub, 0x0E00), Some(5));
        assert_eq!(cmap4(&sub, 0x0E01), Some(6));
        assert_eq!(cmap4(&sub, 0x0041), None);
        assert_eq!(cmap4(&sub, 0x1F600), None, "format 4 is BMP only");
    }

    #[test]
    fn cmap12_maps_a_ucs4_group() {
        let mut sub = vec![0u8; 28];
        sub[0..2].copy_from_slice(&12u16.to_be_bytes());
        sub[12..16].copy_from_slice(&1u32.to_be_bytes());
        sub[16..20].copy_from_slice(&0x0E00u32.to_be_bytes());
        sub[20..24].copy_from_slice(&0x0E7Fu32.to_be_bytes());
        sub[24..28].copy_from_slice(&7u32.to_be_bytes());
        assert_eq!(cmap12(&sub, 0x0E01), Some(8));
        assert_eq!(cmap12(&sub, 0x0041), None);
    }
}

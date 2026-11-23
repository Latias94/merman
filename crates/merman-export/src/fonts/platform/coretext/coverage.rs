use objc2_core_text::{CTFont, CTFontTableOptions};
use ttf_parser::{Face, RawFaceTables};

use super::super::cluster;
use super::GlyphFallback;

pub(super) fn may_shape(font: &CTFont, request: GlyphFallback<'_>) -> bool {
    // These are the physical face's mapping and substitution tables. Glyph IDs
    // can be checked without acquiring outlines, bitmap strikes or a whole TTC.
    let tables = [
        b"head", b"hhea", b"maxp", b"cmap", b"GDEF", b"GSUB", b"GPOS", b"morx", b"feat",
    ]
    .map(|tag| {
        // SAFETY: The live CTFont returns retained immutable table data.
        unsafe { font.table(u32::from_be_bytes(*tag), CTFontTableOptions::NoOptions) }
    });
    // SAFETY: The selected font and the retained immutable CFData handles outlive shaping.
    let data = tables.each_ref().map(|data| {
        data.as_ref()
            .map(|data| unsafe { data.as_bytes_unchecked() })
    });
    let [
        Some(head),
        Some(hhea),
        Some(maxp),
        Some(cmap),
        gdef,
        gsub,
        gpos,
        morx,
        feat,
    ] = data
    else {
        return true;
    };
    let Ok(face) = Face::from_raw_tables(RawFaceTables {
        head,
        hhea,
        maxp,
        cmap: Some(cmap),
        gdef,
        gsub,
        gpos,
        morx,
        feat,
        ..Default::default()
    }) else {
        return true;
    };
    cluster::may_shape(face, request)
}

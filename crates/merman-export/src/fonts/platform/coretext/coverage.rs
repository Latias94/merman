use objc2_core_text::CTFont;
use ttf_parser::{Face, RawFaceTables};

use super::super::cluster;
use super::{
    GlyphFallback, NativeFontError, OperationControl, read_table, read_tables, table_bytes,
};

pub(super) fn has_minimum_tables(
    font: &CTFont,
    control: Option<&OperationControl>,
) -> Result<bool, NativeFontError> {
    let tables = read_tables(font, [b"head", b"hhea", b"maxp", b"cmap"], control)?;
    let [Some(head), Some(hhea), Some(maxp), Some(cmap)] = tables
        .each_ref()
        .map(|table| table.as_deref().map(table_bytes))
    else {
        return Ok(false);
    };
    Ok(ttf_parser::head::Table::parse(head).is_some()
        && ttf_parser::hhea::Table::parse(hhea).is_some()
        && ttf_parser::maxp::Table::parse(maxp).is_some()
        && ttf_parser::cmap::Table::parse(cmap)
            .is_some_and(|cmap| cmap.subtables.into_iter().next().is_some()))
}

/// Check the portable maps before copying the tables needed for cluster shaping.
pub(super) fn check(
    font: &CTFont,
    fallback: Option<GlyphFallback<'_>>,
    implicit: bool,
    control: Option<&OperationControl>,
) -> Result<bool, NativeFontError> {
    if !implicit && fallback.is_none() {
        return Ok(true);
    }

    let [Some(cmap), Some(maxp)] = read_tables(font, [b"cmap", b"maxp"], control)? else {
        return Ok(false);
    };
    let (cmap, maxp) = (table_bytes(&cmap), table_bytes(&maxp));
    let Some(mapping) = ttf_parser::cmap::Table::parse(cmap) else {
        return Ok(false);
    };
    let [head, os2] = if implicit {
        if !mapping
            .subtables
            .into_iter()
            .any(|table| table.is_unicode())
        {
            return Ok(false);
        }
        let [head, os2] = read_tables(font, [b"head", b"OS/2"], control)?;
        if !head.as_deref().is_some_and(|head| {
            crate::font_selection::allows_font_tables(
                table_bytes(head),
                os2.as_deref().map(table_bytes),
                cmap,
                maxp,
                fallback.map(|request| request.text),
            )
        }) {
            return Ok(false);
        }
        [head, os2]
    } else {
        [None, None]
    };
    let Some(request) = fallback else {
        return Ok(true);
    };
    let Some(glyphs) = ttf_parser::maxp::Table::parse(maxp) else {
        return Ok(false);
    };
    let glyph_count = glyphs.number_of_glyphs.get();
    if !cluster::cmap_may_cover(mapping, glyph_count, request.text, request.rtl) {
        return Ok(false);
    }
    if request.scalar().is_some() {
        return Ok(true);
    }

    let head = match head {
        Some(head) => Some(head),
        None => read_table(font, u32::from_be_bytes(*b"head"), control)?,
    };
    let shaping = read_tables(
        font,
        [b"hhea", b"GDEF", b"GSUB", b"GPOS", b"morx", b"feat"],
        control,
    )?;
    let [Some(hhea), gdef, gsub, gpos, morx, feat] = shaping
        .each_ref()
        .map(|table| table.as_deref().map(table_bytes))
    else {
        return Ok(false);
    };
    let Some(head) = head.as_deref().map(table_bytes) else {
        return Ok(false);
    };
    let face = Face::from_raw_tables(RawFaceTables {
        head,
        hhea,
        maxp,
        cmap: Some(cmap),
        os2: os2.as_deref().map(table_bytes),
        gdef,
        gsub,
        gpos,
        morx,
        feat,
        ..Default::default()
    });
    Ok(face.is_ok_and(|face| cluster::may_shape(face, request)))
}

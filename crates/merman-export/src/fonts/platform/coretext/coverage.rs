use super::super::cluster;
use super::{
    GlyphFallback, NativeFontError, OperationControl, read_table, read_tables, table_bytes,
};
use objc2_core_text::CTFont;

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

    let [Some(cmap), Some(maxp), os2] = read_tables(font, [b"cmap", b"maxp", b"OS/2"], control)?
    else {
        return Ok(false);
    };
    let (cmap, maxp) = (table_bytes(&cmap), table_bytes(&maxp));
    let head = if implicit {
        let head = read_table(font, u32::from_be_bytes(*b"head"), control)?;
        if !head.as_deref().is_some_and(|head| {
            crate::font_selection::allows_font_tables(
                table_bytes(head),
                os2.as_deref().map(table_bytes),
                cmap,
                maxp,
                fallback.map(|request| (request.text, request.rtl)),
            )
        }) {
            return Ok(false);
        }
        head
    } else {
        None
    };
    let Some(request) = fallback else {
        return Ok(true);
    };
    let Some(glyphs) = ttf_parser::maxp::Table::parse(maxp) else {
        return Ok(false);
    };
    let glyph_count = glyphs.number_of_glyphs.get();
    if !cluster::cmap_may_cover_with_os2(
        cmap,
        os2.as_deref().map(table_bytes),
        glyph_count,
        request.text,
        request.rtl,
    ) {
        return Ok(false);
    }
    if !cluster::needs_isolated_shaping_probe(request) {
        return Ok(true);
    }

    let head = match head {
        Some(head) => Some(head),
        None => read_table(font, u32::from_be_bytes(*b"head"), control)?,
    };
    let Some(head) = head.as_deref().map(table_bytes) else {
        return Ok(false);
    };
    let tags = cluster::SHAPING_TABLES;
    let layout = read_tables(font, tags, control)?;
    let mut tables = vec![(b"head", head), (b"maxp", maxp), (b"cmap", cmap)];
    if let Some(os2) = os2.as_deref() {
        tables.push((b"OS/2", table_bytes(os2)));
    }
    tables.extend(
        tags.into_iter()
            .zip(layout.iter())
            .filter_map(|(tag, table)| table.as_deref().map(table_bytes).map(|table| (tag, table))),
    );
    Ok(cluster::may_shape_after_cmap(request, &tables))
}

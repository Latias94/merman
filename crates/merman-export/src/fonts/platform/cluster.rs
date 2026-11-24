use rustybuzz::{Direction, Feature, UnicodeBuffer, script};
use ttf_parser::{Face, Tag};
use unicode_normalization::UnicodeNormalization;

use super::GlyphFallback;

pub(super) fn is_default_ignorable(ch: char) -> bool {
    // Preserve the spacing fillers used as real glyphs by the shaping engine.
    matches!(
        ch,
        '\u{ad}' | '\u{34f}' | '\u{61c}' | '\u{17b4}'..='\u{17b5}'
        | '\u{180b}'..='\u{180e}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}'
        | '\u{2060}'..='\u{206f}' | '\u{fe00}'..='\u{fe0f}' | '\u{feff}'
        | '\u{fff0}'..='\u{fff8}' | '\u{1d173}'..='\u{1d17a}' | '\u{e0000}'..='\u{e0fff}'
    )
}

pub(super) fn has_candidate_mapping(text: &str, mut maps: impl FnMut(char) -> bool) -> bool {
    let mut meaningful = false;
    let mut mapped = |ch| {
        if is_default_ignorable(ch) {
            return false;
        }
        meaningful = true;
        maps(ch) || unicode_bidi_mirroring::get_mirrored(ch).is_some_and(&mut maps)
    };
    text.chars().any(&mut mapped)
        || (!text.is_ascii() && (text.nfc().any(&mut mapped) || text.nfd().any(&mut mapped)))
        || !meaningful
}

/// Check for any relevant Unicode mapping, including explicit variant glyphs.
pub(in crate::fonts) fn cmap_may_cover(
    cmap: ttf_parser::cmap::Table<'_>,
    glyph_count: u16,
    text: &str,
) -> bool {
    use ttf_parser::cmap::GlyphVariationResult;

    let mut preceding = None;
    for ch in text.chars() {
        if matches!(ch, '\u{fe00}'..='\u{fe0f}' | '\u{e0100}'..='\u{e01ef}')
            && let Some(base) = preceding && cmap.subtables.into_iter().any(|table| {
            table.is_unicode()
                && matches!(
                    table.glyph_variation_index(base, u32::from(ch)),
                    Some(GlyphVariationResult::Found(glyph)) if (1..glyph_count).contains(&glyph.0)
                )
        }) {
            return true;
        }
        preceding = Some(u32::from(ch));
    }
    has_candidate_mapping(text, |ch| {
        crate::font_selection::cmap_covers_character(cmap, glyph_count, ch)
    })
}

/// Reject only clusters known to produce missing glyphs before snapshot acquisition.
pub(in crate::fonts) fn may_shape(face: Face<'_>, request: GlyphFallback<'_>) -> bool {
    if !face
        .tables()
        .cmap
        .is_some_and(|cmap| cmap_may_cover(cmap, face.number_of_glyphs(), request.text))
    {
        return false;
    }
    if request.scalar().is_some() {
        return true;
    }
    let gsub = face.tables().gsub;
    if gsub.is_some_and(|table| table.variations.is_some()) {
        // usvg has the effective variation coordinates, including automatic optical size.
        return true;
    }
    let direction = if request.rtl {
        Direction::RightToLeft
    } else {
        Direction::LeftToRight
    };
    let language = request
        .language
        .and_then(|tag| tag.parse::<rustybuzz::Language>().ok());
    let fill = |buffer: &mut UnicodeBuffer| {
        buffer.push_str(request.text);
        buffer.set_direction(direction);
        if let Some(language) = &language {
            buffer.set_language(language.clone());
        }
    };
    let mut buffer = UnicodeBuffer::new();
    fill(&mut buffer);
    buffer.guess_segment_properties();
    // Common and inherited marks alone leave the script unknown. Other shapers
    // can use the surrounding run (e.g. Arabic joining), which only usvg has.
    if !matches!(
        buffer.script(),
        script::UNKNOWN | script::LATIN | script::GREEK | script::CYRILLIC | script::HAN
    ) {
        return true;
    }

    let face = rustybuzz::Face::from_face(face);
    let mut shaped = rustybuzz::shape(&face, &[], buffer);
    let glyph_count = u32::from(face.number_of_glyphs());
    let covered = |shaped: &rustybuzz::GlyphBuffer| {
        shaped
            .glyph_infos()
            .iter()
            .all(|glyph| (1..glyph_count).contains(&glyph.glyph_id))
    };
    if covered(&shaped) {
        return true;
    }

    // The fallback request lacks the span's small-caps and kerning flags.
    let small_caps = Feature::new(Tag::from_bytes(b"smcp"), 1, ..);
    let no_kerning = Feature::new(Tag::from_bytes(b"kern"), 0, ..);
    let optional_substitutions =
        gsub.is_some() || (face.tables().morx.is_some() && face.tables().feat.is_some());
    if optional_substitutions {
        for features in [&[small_caps][..], &[no_kerning], &[small_caps, no_kerning]] {
            let mut buffer = shaped.clear();
            fill(&mut buffer);
            shaped = rustybuzz::shape(&face, features, buffer);
            if covered(&shaped) {
                return true;
            }
        }
    }
    false
}

use rustybuzz::{Direction, Feature, UnicodeBuffer, script};
use ttf_parser::{Face, Tag};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::compose;

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

fn is_variation_selector(ch: char) -> bool {
    matches!(ch, '\u{fe00}'..='\u{fe0f}' | '\u{e0100}'..='\u{e01ef}')
}

#[cfg(any(target_os = "linux", target_os = "freebsd", test))]
pub(super) fn has_variation_sequence(text: &str) -> bool {
    let mut chars = text.chars().peekable();
    while let Some(base) = chars.next() {
        if !is_default_ignorable(base) && chars.peek().copied().is_some_and(is_variation_selector) {
            return true;
        }
    }
    false
}

fn scalar_may_map(ch: char, rtl: bool, maps: &mut impl FnMut(char) -> bool) -> bool {
    if maps(ch) || (rtl && unicode_bidi_mirroring::get_mirrored(ch).is_some_and(&mut *maps)) {
        return true;
    }
    if ch.is_ascii() {
        return false;
    }
    // The shaper can synthesize these spaces or use the breakable hyphen.
    match ch {
        '\u{a0}' | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}' => {
            return maps(' ');
        }
        '\u{2011}' => return maps('\u{2010}'),
        _ => {}
    }

    // Every part of a scalar's decomposition must map. Try intermediate forms too,
    // such as å + acute for ǻ, if the face lacks the fully decomposed base or marks.
    let mut rest = std::iter::once(ch).nfd();
    let Some(mut base) = rest.next().filter(|&base| base != ch) else {
        return false;
    };
    loop {
        if maps(base) && rest.clone().all(&mut *maps) {
            return true;
        }
        let Some(composed) = rest.next().and_then(|mark| compose(base, mark)) else {
            return false;
        };
        base = composed;
    }
}

/// Check for nominal glyphs that could participate in shaping this request.
pub(super) fn has_candidate_mapping(
    text: &str,
    rtl: bool,
    mut maps: impl FnMut(char) -> bool,
) -> bool {
    let meaningful = |ch: &char| !is_default_ignorable(*ch);
    let mut chars = text.chars().filter(meaningful).peekable();
    let Some(first) = chars.next() else {
        return true;
    };
    if scalar_may_map(first, rtl, &mut maps) {
        return true;
    }
    if chars.peek().is_none() {
        return false;
    }

    // A multi-scalar cluster can compose or use contextual substitutions. Leave
    // partial nominal coverage for the shaping step to decide.
    chars.any(|ch| scalar_may_map(ch, rtl, &mut maps))
        || (!text.is_ascii()
            && text
                .nfc()
                .filter(meaningful)
                .any(|ch| scalar_may_map(ch, rtl, &mut maps)))
}

/// Check for any relevant Unicode mapping, including explicit variant glyphs.
pub(in crate::fonts) fn cmap_may_cover(
    cmap: ttf_parser::cmap::Table<'_>,
    glyph_count: u16,
    text: &str,
    rtl: bool,
) -> bool {
    has_candidate_mapping(text, rtl, |ch| {
        crate::font_selection::cmap_covers_character(cmap, glyph_count, ch)
    }) || has_non_default_variation(cmap, glyph_count, text, rtl)
}

pub(in crate::fonts) fn has_non_default_variation(
    cmap: ttf_parser::cmap::Table<'_>,
    glyph_count: u16,
    text: &str,
    rtl: bool,
) -> bool {
    use ttf_parser::cmap::GlyphVariationResult;

    let mut preceding = None;
    for ch in text.chars() {
        if is_variation_selector(ch)
            && let Some(base) = preceding
        {
            // Mirroring happens before sequence lookup, and only for a nominal
            // glyph that exists in the font.
            let mirror = rtl
                .then(|| unicode_bidi_mirroring::get_mirrored(base))
                .flatten()
                .filter(|&ch| crate::font_selection::cmap_covers_character(cmap, glyph_count, ch));
            let base = u32::from(mirror.unwrap_or(base));
            if cmap.subtables.into_iter().any(|table| {
                table.is_unicode()
                    && matches!(
                        table.glyph_variation_index(base, u32::from(ch)),
                        Some(GlyphVariationResult::Found(glyph)) if (1..glyph_count).contains(&glyph.0)
                    )
            }) {
                return true;
            }
        }
        preceding = (!is_default_ignorable(ch)).then_some(ch);
    }
    false
}

/// Reject only clusters known to produce missing glyphs before snapshot acquisition.
pub(in crate::fonts) fn may_shape(face: Face<'_>, request: GlyphFallback<'_>) -> bool {
    if !face.tables().cmap.is_some_and(|cmap| {
        cmap_may_cover(cmap, face.number_of_glyphs(), request.text, request.rtl)
    }) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nominal_coverage_uses_the_run_direction_and_complete_scalar_forms() {
        assert!(!has_candidate_mapping("∋", false, |ch| ch == '∈'));
        assert!(has_candidate_mapping("∋", true, |ch| ch == '∈'));
        assert!(has_candidate_mapping("∋", true, |ch| ch == '∋'));

        assert!(!has_candidate_mapping("í", false, |ch| ch == 'i'));
        assert!(!has_candidate_mapping("í", false, |ch| ch == '\u{301}'));
        assert!(has_candidate_mapping("í", false, |ch| matches!(
            ch,
            'i' | '\u{301}'
        )));
        assert!(has_candidate_mapping("ǻ", false, |ch| matches!(
            ch,
            'å' | '\u{301}'
        )));
        assert!(has_candidate_mapping("i\u{301}", false, |ch| ch == 'í'));
        assert!(has_candidate_mapping("i\u{301}", false, |ch| ch == 'i'));
    }

    #[test]
    fn non_default_variation_covers_a_base_without_a_nominal_mapping() {
        // One Unicode cmap format 14 subtable, mapping U+2764 U+FE0F to glyph 1.
        let cmap = ttf_parser::cmap::Table::parse(&[
            0, 0, 0, 1, 0, 0, 0, 5, 0, 0, 0, 12, 0, 14, 0, 0, 0, 30, 0, 0, 0, 1, 0, 0xfe, 0x0f, 0,
            0, 0, 0, 0, 0, 0, 21, 0, 0, 0, 1, 0, 0x27, 0x64, 0, 1,
        ])
        .unwrap();
        assert!(!cmap_may_cover(cmap, 2, "❤", false));
        assert!(has_variation_sequence("❤\u{fe0f}"));
        assert!(cmap_may_cover(cmap, 2, "❤\u{fe0f}", false));
        assert!(!cmap_may_cover(cmap, 1, "❤\u{fe0f}", false));
        assert!(!cmap_may_cover(cmap, 2, "❤\u{fe0e}", false));
        assert!(!has_variation_sequence("❤\u{200d}\u{fe0f}"));
    }
}

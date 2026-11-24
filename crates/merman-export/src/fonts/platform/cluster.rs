use harfrust::unicode::{Decomposed, decompose};
use rustybuzz::{Direction, Feature, UnicodeBuffer, script};
use ttf_parser::{Face, Tag};
use unicode_normalization::UnicodeNormalization;

use super::GlyphFallback;
use crate::font_selection::PlatformCmap;

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

pub(in crate::fonts) fn has_variation_sequence(text: &str) -> bool {
    variation_sequences(text).next().is_some()
}

pub(in crate::fonts) fn variation_sequences(text: &str) -> impl Iterator<Item = (char, char)> + '_ {
    text.chars()
        .zip(text.chars().skip(1))
        .filter(|&(base, selector)| !is_default_ignorable(base) && is_variation_selector(selector))
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

    decomposition_may_map(u32::from(ch), maps)
}

fn decomposition_may_map(ch: u32, maps: &mut impl FnMut(char) -> bool) -> bool {
    // Shaping can recursively decompose the leading scalar if the second maps.
    // One-step decomposition preserves intermediate forms and composition exclusions.
    let base = match decompose(ch) {
        Some(Decomposed::Singleton(base)) => base,
        Some(Decomposed::Pair(base, mark)) if char::from_u32(mark).is_some_and(&mut *maps) => base,
        _ => return false,
    };
    char::from_u32(base).is_some_and(&mut *maps) || decomposition_may_map(base, maps)
}

/// Check for nominal glyphs that could participate in shaping this request.
fn has_candidate_mapping(text: &str, rtl: bool, mut maps: impl FnMut(char) -> bool) -> bool {
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

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(super) fn has_advertised_coverage(
    text: &str,
    rtl: bool,
    mut maps: impl FnMut(char) -> bool,
) -> bool {
    // A scan rule can deliberately remove a mark the physical font can shape.
    // Allow a complete normalized form when the rule lists a precomposed glyph.
    let mut covered = |ch| is_default_ignorable(ch) || scalar_may_map(ch, rtl, &mut maps);
    text.chars().all(&mut covered) || (!text.is_ascii() && text.nfc().all(covered))
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(super) fn unadvertised_variations(
    text: &str,
    rtl: bool,
    mut maps: impl FnMut(char) -> bool,
) -> Option<Vec<(char, char)>> {
    let mut missing = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if !is_default_ignorable(ch) && !scalar_may_map(ch, rtl, &mut maps) {
            // Fontconfig has no charset entries for UVS pairs. Only an actual
            // variation glyph can make up for an unadvertised base.
            let selector = chars
                .peek()
                .copied()
                .filter(|&ch| is_variation_selector(ch))?;
            missing.push((ch, selector));
        }
    }
    (!missing.is_empty()).then_some(missing)
}

/// Check for any relevant Unicode mapping, including explicit variant glyphs.
pub(in crate::fonts) fn cmap_may_cover(
    cmap: &[u8],
    glyph_count: u16,
    text: &str,
    rtl: bool,
) -> bool {
    PlatformCmap::parse(cmap, glyph_count).is_some_and(|cmap| {
        if has_candidate_mapping(text, rtl, |ch| cmap.maps(ch)) {
            return true;
        }
        let mut sequences = variation_sequences(text).peekable();
        sequences.peek().is_some() && sequences.any(cmap.non_default_variation(rtl))
    })
}

/// Reject only clusters known to produce missing glyphs before snapshot acquisition.
pub(in crate::fonts) fn may_shape(face: Face<'_>, request: GlyphFallback<'_>) -> bool {
    let Some(cmap) = face.raw_face().table(Tag::from_bytes(b"cmap")) else {
        return false;
    };
    cmap_may_cover(cmap, face.number_of_glyphs(), request.text, request.rtl)
        && may_shape_after_cmap(face, request)
}

pub(in crate::fonts) fn may_shape_after_cmap(face: Face<'_>, request: GlyphFallback<'_>) -> bool {
    if request.scalar().is_some() || has_variation_sequence(request.text) {
        // Rustybuzz and HarfRust can select different variation subtables.
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
        assert!(has_candidate_mapping("ệ", false, |ch| matches!(
            ch,
            'ẹ' | '\u{302}'
        )));
        assert!(has_candidate_mapping("ệ", false, |ch| matches!(
            ch,
            'e' | '\u{323}' | '\u{302}'
        )));
        assert!(!has_candidate_mapping("ệ", false, |ch| matches!(
            ch,
            'ê' | '\u{323}'
        )));
        assert!(!has_candidate_mapping("ệ", false, |ch| ch == 'ẹ'));
        assert!(has_candidate_mapping("\u{fb2c}", true, |ch| matches!(
            ch,
            '\u{fb49}' | '\u{05c1}'
        )));
        assert!(!has_candidate_mapping("\u{fb2c}", true, |ch| ch == '\u{fb49}'));
        assert!(has_candidate_mapping("i\u{301}", false, |ch| ch == 'í'));
        assert!(has_candidate_mapping("i\u{301}", false, |ch| ch == 'i'));
    }

    #[test]
    fn non_default_variation_covers_a_base_without_a_nominal_mapping() {
        // One Unicode cmap format 14 subtable, mapping U+2764 U+FE0F to glyph 1.
        let cmap = &[
            0, 0, 0, 1, 0, 0, 0, 5, 0, 0, 0, 12, 0, 14, 0, 0, 0, 30, 0, 0, 0, 1, 0, 0xfe, 0x0f, 0,
            0, 0, 0, 0, 0, 0, 21, 0, 0, 0, 1, 0, 0x27, 0x64, 0, 1,
        ];
        assert!(!cmap_may_cover(cmap, 2, "❤", false));
        assert!(has_variation_sequence("❤\u{fe0f}"));
        assert!(cmap_may_cover(cmap, 2, "❤\u{fe0f}", false));
        assert!(!cmap_may_cover(cmap, 1, "❤\u{fe0f}", false));
        assert!(!cmap_may_cover(cmap, 2, "❤\u{fe0e}", false));
        assert!(!has_variation_sequence("❤\u{200d}\u{fe0f}"));
    }

    #[test]
    fn a_secondary_cmap_cannot_supply_a_missing_scalar() {
        // The higher-ranked Windows full-Unicode subtable maps ∈ only.
        // The lower-ranked Unicode subtable maps ∋, but HarfRust never uses it.
        let cmap = &[
            0, 0, 0, 2, 0, 0, 0, 4, 0, 0, 0, 20, 0, 3, 0, 10, 0, 0, 0, 48, 0, 12, 0, 0, 0, 0, 0,
            28, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0x22, 0x0b, 0, 0, 0x22, 0x0b, 0, 0, 0, 1, 0, 12, 0,
            0, 0, 0, 0, 28, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0x22, 0x08, 0, 0, 0x22, 0x08, 0, 0, 0, 1,
        ];
        assert!(!cmap_may_cover(cmap, 2, "∋", false));
        assert!(cmap_may_cover(cmap, 2, "∋", true));
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    #[test]
    fn variation_sequences_only_replace_their_own_scan_charset_entries() {
        let maps = |ch| ch == 'A';
        assert_eq!(
            unadvertised_variations("A\u{fe0f}\u{200d}B\u{fe0e}", false, maps),
            Some(vec![('B', '\u{fe0e}')])
        );
        assert_eq!(
            unadvertised_variations("A\u{fe0f}\u{301}", false, maps),
            None
        );
    }
}

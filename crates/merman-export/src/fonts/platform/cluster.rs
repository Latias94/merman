use harfrust::unicode::{Decomposed, decompose};
use rustybuzz::{Direction, Feature, UnicodeBuffer, script};
use ttf_parser::{Face, Tag};
use unicode_general_category::{GeneralCategory, get_general_category};
use unicode_normalization::UnicodeNormalization;

use super::GlyphFallback;
use crate::font_selection::PlatformCmap;

pub(super) fn is_default_ignorable(ch: char) -> bool {
    // Preserve the spacing fillers used as real glyphs by the shaping engine.
    matches!(
        ch,
        '\u{ad}' | '\u{34f}' | '\u{61c}' | '\u{17b4}'..='\u{17b5}'
        | '\u{180b}'..='\u{180f}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}'
        | '\u{2060}'..='\u{206f}' | '\u{fe00}'..='\u{fe0f}' | '\u{feff}'
        | '\u{fff0}'..='\u{fff8}' | '\u{1d173}'..='\u{1d17a}' | '\u{e0000}'..='\u{e0fff}'
    )
}

fn is_variation_selector(ch: char) -> bool {
    // The Mongolian selectors participate in script shaping instead.
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

fn variation_clusters(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.char_indices()
        .skip(1)
        .filter_map(|(index, ch)| {
            (!matches!(
                get_general_category(ch),
                GeneralCategory::NonspacingMark
                    | GeneralCategory::SpacingMark
                    | GeneralCategory::EnclosingMark
            ))
            .then_some(index)
        })
        .chain(std::iter::once(text.len()))
        .scan(0, move |start, end| {
            let cluster = &text[*start..end];
            let index = *start;
            *start = end;
            Some((index, cluster))
        })
        .filter(|(_, cluster)| has_variation_sequence(cluster))
}

fn has_complete_nominal_coverage(
    text: &str,
    rtl: bool,
    mut maps: impl FnMut(char) -> bool,
) -> bool {
    let mut covered = |ch| is_default_ignorable(ch) || scalar_may_map(ch, rtl, &mut maps);
    text.chars().all(&mut covered) || (!text.is_ascii() && text.nfc().all(covered))
}

fn scalar_may_map(ch: char, rtl: bool, maps: &mut impl FnMut(char) -> bool) -> bool {
    if maps(ch) || (rtl && unicode_bidi_mirroring::get_mirrored(ch).is_some_and(&mut *maps)) {
        return true;
    }
    if ch.is_ascii() {
        return false;
    }
    // HarfRust substitutes some spaces and hyphens and decomposes SARA AM
    // when shaping Thai and Lao, before canonical normalization.
    let synthesized = match ch {
        '\u{a0}' | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}' => maps(' '),
        '\u{2011}' => maps('\u{2010}'),
        '\u{0e33}' => maps('\u{0e4d}') && maps('\u{0e32}'),
        '\u{0eb3}' => maps('\u{0ecd}') && maps('\u{0eb2}'),
        _ => false,
    };

    synthesized || decomposition_may_map(u32::from(ch), maps)
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
    maps: impl FnMut(char) -> bool,
) -> bool {
    // A scan rule can deliberately remove a mark the physical font can shape.
    // Allow a complete normalized form when the rule lists a precomposed glyph.
    has_complete_nominal_coverage(text, rtl, maps)
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(super) fn unadvertised_variations(
    text: &str,
    rtl: bool,
    mut maps: impl FnMut(char) -> bool,
) -> Option<Vec<(char, char)>> {
    let mut missing = Vec::new();
    let mut start = 0;
    for (index, cluster) in variation_clusters(text) {
        // Canonical composition is possible outside the selector's cluster.
        if !has_advertised_coverage(&text[start..index], rtl, &mut maps) {
            return None;
        }
        let mut chars = cluster.chars().peekable();
        while let Some(ch) = chars.next() {
            if !is_default_ignorable(ch) && !scalar_may_map(ch, rtl, &mut maps) {
                // A true non-default glyph can cover an unadvertised base.
                let selector = chars
                    .peek()
                    .copied()
                    .filter(|&ch| is_variation_selector(ch))?;
                missing.push((ch, selector));
            }
        }
        start = index + cluster.len();
    }
    (!missing.is_empty() && has_advertised_coverage(&text[start..], rtl, maps)).then_some(missing)
}

/// Check for any relevant Unicode mapping, including explicit variant glyphs.
#[cfg(any(windows, test))]
pub(in crate::fonts) fn cmap_may_cover(
    cmap: &[u8],
    glyph_count: u16,
    text: &str,
    rtl: bool,
) -> bool {
    PlatformCmap::parse(cmap, glyph_count).is_some_and(|cmap| {
        cmap_may_cover_text(&cmap, text, rtl)
            || (cmap.is_symbol() && text.chars().any(PlatformCmap::may_use_arabic_symbol_page))
    })
}

pub(in crate::fonts) fn cmap_may_cover_with_os2(
    cmap: &[u8],
    os2: Option<&[u8]>,
    glyph_count: u16,
    text: &str,
    rtl: bool,
) -> bool {
    PlatformCmap::parse_with_os2(cmap, os2, glyph_count)
        .is_some_and(|cmap| cmap_may_cover_text(&cmap, text, rtl))
}

fn cmap_may_cover_text(cmap: &PlatformCmap<'_>, text: &str, rtl: bool) -> bool {
    if !has_variation_sequence(text) {
        return has_candidate_mapping(text, rtl, |ch| cmap.maps(ch));
    }

    let maps_variant = cmap.maps_variation(rtl, false);
    // HarfRust skips normalization in a cluster containing a selector. Each
    // remaining mark needs its own mapping, even if the base uses a UVS.
    let mut start = 0;
    for (index, cluster) in variation_clusters(text) {
        if !has_complete_nominal_coverage(&text[start..index], rtl, |ch| cmap.maps(ch)) {
            return false;
        }
        let mut chars = cluster.chars().peekable();
        while let Some(ch) = chars.next() {
            if !is_default_ignorable(ch)
                && !cmap.maps_directional(ch, rtl)
                && !chars.peek().is_some_and(|&selector| {
                    is_variation_selector(selector) && maps_variant((ch, selector))
                })
            {
                return false;
            }
        }
        start = index + cluster.len();
    }
    has_complete_nominal_coverage(&text[start..], rtl, |ch| cmap.maps(ch))
}

/// Reject only clusters known to produce missing glyphs before snapshot acquisition.
pub(in crate::fonts) fn may_shape(face: Face<'_>, request: GlyphFallback<'_>) -> bool {
    let Some(cmap) = face.raw_face().table(Tag::from_bytes(b"cmap")) else {
        return false;
    };
    cmap_may_cover_with_os2(
        cmap,
        face.raw_face().table(Tag::from_bytes(b"OS/2")),
        face.number_of_glyphs(),
        request.text,
        request.rtl,
    ) && may_shape_after_cmap(face, request)
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
        assert!(!cmap_may_cover(cmap, 2, "❤\u{fe0f}\u{301}", false));
        assert!(!cmap_may_cover(cmap, 2, "x❤\u{fe0f}", false));
        assert!(!cmap_may_cover(cmap, 2, "❤\u{fe0f}\u{200d}x", false));
        assert!(!has_variation_sequence("❤\u{200d}\u{fe0f}"));
    }

    #[test]
    fn scalar_preflight_matches_harfrust_whitespace_and_decomposition() {
        for (text, nominal, expected) in [
            ("\u{2000}", &['\u{2002}'][..], true),
            ("ệ", &['ẹ', '\u{302}'], true),
            ("ệ", &['ê', '\u{323}'], false),
            ("\u{fb2c}", &['\u{fb49}', '\u{05c1}'], true),
            ("\u{0e33}", &['\u{0e4d}', '\u{0e32}'], true),
            ("\u{0e33}", &['\u{0e32}'], false),
            ("\u{0eb3}", &['\u{0ecd}', '\u{0eb2}'], true),
            ("\u{0eb3}", &['\u{0ecd}'], false),
        ] {
            let rtl = text == "\u{fb2c}";
            let cmap = fixture_cmap(nominal, false, 10);
            let font = font_with_table(b"cmap", &cmap);
            assert_eq!(cmap_may_cover(&cmap, 6, text, rtl), expected, "{text:?}");
            assert_eq!(harfrust_covers(font, text, rtl), expected, "{text:?}");
        }
    }

    #[test]
    fn a_legacy_arabic_symbol_page_uses_the_harfrust_mapping() {
        let cmap = fixture_cmap(&['\u{f141}'], false, 0);
        let mut font = font_with_table(b"cmap", &cmap);
        let face = Face::parse(&font, 0).unwrap();
        let mut os2 = face.raw_face().table(Tag::from_bytes(b"OS/2")).unwrap()[..78].to_vec();
        os2[..2].fill(0);
        os2[62] = 0xb2;
        assert!(cmap_may_cover_with_os2(&cmap, Some(&os2), 6, "ا", true));
        assert!(!cmap_may_cover_with_os2(&cmap, Some(&os2), 6, "A", false));
        replace_table(&mut font, b"OS/2", &os2);
        assert!(harfrust_covers(font, "ا", true));
    }

    #[cfg(windows)]
    #[test]
    fn a_uvs_face_missing_the_following_mark_does_not_charge_the_snapshot() {
        use crate::fonts::platform::PlatformFace;
        use crate::fonts::registry::FontSourceRegistry;
        use crate::fonts::{NativeFontLimits, OperationControl};
        use std::collections::BTreeMap;
        use std::sync::Arc;
        use usvg::fontdb::Database;

        let text = "❤\u{fe0f}\u{301}";
        let mut incomplete = font_with_table(b"cmap", &fixture_cmap(&[], true, 10));
        let complete = font_with_table(b"cmap", &fixture_cmap(&['\u{301}'], true, 10));
        incomplete.extend_from_slice(&[0; 256]);
        assert!(!harfrust_covers(incomplete.clone(), text, false));
        assert!(harfrust_covers(complete.clone(), text, false));

        let control = OperationControl::new();
        let limits = NativeFontLimits {
            max_bytes: complete.len(),
            ..Default::default()
        };
        let registry = FontSourceRegistry::new(BTreeMap::new(), BTreeMap::new(), 0, 0);
        let mut database = Arc::new(Database::new());
        let root = std::env::temp_dir().join(format!(
            "merman-uvs-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir(&root).unwrap();
        for (name, bytes, covers) in [
            ("incomplete.ttf", incomplete, false),
            ("complete.ttf", complete, true),
        ] {
            let path = root.join(name);
            std::fs::write(&path, bytes).unwrap();
            let face = PlatformFace {
                index: 0,
                family: "MermanProbeFull".into(),
                post_script_name: Some("MermanProbeFull".into()),
                other_families: Vec::new(),
                variations: Vec::new(),
                fallback_scale: 1.0,
                dwrite_identity: None,
                fvar_digest: None,
            };
            let result = registry.admit(
                face.into_file(path),
                Some(text.into()),
                &mut database,
                &limits,
                &control,
            );
            assert_eq!(result.unwrap().is_some(), covers);
        }
        assert_eq!(registry.resident_bytes(), limits.max_bytes);
        std::fs::remove_dir_all(root).unwrap();
    }

    fn fixture_cmap(nominal: &[char], heart_vs16: bool, encoding: u8) -> Vec<u8> {
        let nominal_offset = if heart_vs16 { 20u32 } else { 12 };
        let nominal_len = 16 + 12 * nominal.len() as u32;
        let mut cmap = vec![0, 0, 0, if heart_vs16 { 2 } else { 1 }];
        if heart_vs16 {
            cmap.extend_from_slice(&[0, 0, 0, 5]);
            cmap.extend_from_slice(&(nominal_offset + nominal_len).to_be_bytes());
        }
        cmap.extend_from_slice(&[0, 3, 0, encoding]);
        cmap.extend_from_slice(&nominal_offset.to_be_bytes());
        cmap.extend_from_slice(&[0, 12, 0, 0]);
        cmap.extend_from_slice(&nominal_len.to_be_bytes());
        cmap.extend_from_slice(&[0; 4]);
        cmap.extend_from_slice(&(nominal.len() as u32).to_be_bytes());
        let mut sorted = nominal.to_vec();
        sorted.sort_unstable();
        for ch in sorted {
            let key = u32::from(ch).to_be_bytes();
            cmap.extend_from_slice(&key);
            cmap.extend_from_slice(&key);
            cmap.extend_from_slice(&1u32.to_be_bytes());
        }
        if heart_vs16 {
            cmap.extend_from_slice(&[
                0, 14, 0, 0, 0, 30, 0, 0, 0, 1, 0, 0xfe, 0x0f, 0, 0, 0, 0, 0, 0, 0, 21, 0, 0, 0, 1,
                0, 0x27, 0x64, 0, 1,
            ]);
        }
        cmap
    }

    fn font_with_table(tag: &[u8; 4], table: &[u8]) -> Vec<u8> {
        let mut font = include_bytes!("../../../tests/fonts/MermanProbeFull.ttf").to_vec();
        replace_table(&mut font, tag, table);
        font
    }

    fn replace_table(font: &mut Vec<u8>, tag: &[u8; 4], table: &[u8]) {
        let count = usize::from(u16::from_be_bytes([font[4], font[5]]));
        let entry = (0..count)
            .map(|i| 12 + 16 * i)
            .find(|&i| &font[i..i + 4] == tag)
            .unwrap();
        font.resize(font.len().next_multiple_of(4), 0);
        let offset = font.len() as u32;
        font[entry + 8..entry + 12].copy_from_slice(&offset.to_be_bytes());
        font[entry + 12..entry + 16].copy_from_slice(&(table.len() as u32).to_be_bytes());
        font.extend_from_slice(table);
    }

    fn harfrust_covers(bytes: Vec<u8>, text: &str, rtl: bool) -> bool {
        use harfrust::{Buffer, Direction, Font, ShapeOptions, ShaperFont};

        let font = Font::new(bytes, 0).unwrap();
        let mut buffer = Buffer::new();
        buffer.push_str(text);
        buffer.set_direction(if rtl {
            Direction::RightToLeft
        } else {
            Direction::LeftToRight
        });
        buffer.guess_segment_properties();
        harfrust::shape(
            &ShaperFont::new(&font),
            &mut buffer,
            ShapeOptions::default(),
        )
        .unwrap();
        !buffer.glyph_infos().is_empty()
            && buffer.glyph_infos().iter().all(|glyph| glyph.glyph_id != 0)
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

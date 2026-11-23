use rustybuzz::{Direction, Feature, UnicodeBuffer, script};
use ttf_parser::{Face, Tag};

use super::GlyphFallback;

/// Reject only clusters known to produce missing glyphs before snapshot acquisition.
pub(in crate::fonts) fn may_shape(face: Face<'_>, request: GlyphFallback<'_>) -> bool {
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

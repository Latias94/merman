//! Core Text character fallback discovery. Character-set membership only filters candidates;
//! the caller admits font data, validates nonzero glyph IDs and their bounds, and shapes text.

use std::ptr;

use objc2_core_foundation::{CFCharacterSet, CFRange, CFString};
use objc2_core_text::{CTFont, CTFontTableOptions};
use usvg::fontdb::FaceInfo;

pub(super) struct FallbackFace {
    pub(super) family: String,
    pub(super) post_script_name: String,
}

/// Queries the registered base face's cascade for one Unicode scalar.
pub(super) fn fallback(character: char, base: &FaceInfo) -> Option<FallbackFace> {
    // Core Text reserves dot-prefixed names for system UI font creation APIs.
    if base.post_script_name.is_empty() || base.post_script_name.starts_with('.') {
        return None;
    }

    let name = CFString::from_str(&base.post_script_name);
    // SAFETY: `name` is live, and a null matrix requests the identity transform.
    let base_font = unsafe { CTFont::with_name(&name, 0.0, ptr::null()) };
    // SAFETY: The retained font is live; Core Text returns an owned name.
    let actual_name = unsafe { base_font.post_script_name() };
    if actual_name.to_string() != base.post_script_name {
        // CTFontCreateWithName silently substitutes another font for unknown names.
        return None;
    }

    let mut utf8 = [0; 4];
    let text = CFString::from_str(character.encode_utf8(&mut utf8));
    let range = CFRange {
        location: 0,
        length: text.length(),
    };
    // SAFETY: Both retained objects are live and the range covers the complete
    // CFString in UTF-16 code units, including both halves of a surrogate pair.
    let font = unsafe { CTFont::for_string(&base_font, &text, range) };
    // SAFETY: The retained font is live; both accessors return owned strings.
    let (family, post_script_name) = unsafe { (font.family_name(), font.post_script_name()) };
    let family = family.to_string();
    let post_script_name = post_script_name.to_string();
    if family.is_empty()
        || post_script_name.is_empty()
        || is_last_resort(&family)
        || is_last_resort(&post_script_name)
    {
        return None;
    }

    // SAFETY: The retained font is live; Core Text returns an owned tag array.
    let tables = unsafe { font.available_tables(CTFontTableOptions::NoOptions) }?;
    for index in 0..tables.count() {
        // SAFETY: The index is in bounds of the immutable retained array. This API
        // stores unboxed table tags in pointer slots; never dereference or retain
        // the values as CF objects (CTFontCopyAvailableTables documents this).
        let tag = unsafe { tables.value_at_index(index) } as usize;
        if tag == u32::from_be_bytes(*b"hvgl") as usize {
            return None;
        }
    }

    // SAFETY: The retained font is live; Core Text returns an owned character set.
    let character_set = unsafe { font.character_set() };
    if !character_set_contains_scalar(&character_set, character) {
        return None;
    }

    Some(FallbackFace {
        family,
        post_script_name,
    })
}

fn is_last_resort(name: &str) -> bool {
    let name = name.trim_start_matches('.');
    name.eq_ignore_ascii_case("LastResort") || name.eq_ignore_ascii_case("Last Resort")
}

fn character_set_contains_scalar(character_set: &CFCharacterSet, character: char) -> bool {
    // The long-character API takes a full UTF-32 scalar, including non-BMP values.
    character_set.is_long_character_member(u32::from(character))
}

#[cfg(test)]
mod tests {
    use super::*;
    use usvg::fontdb::{Database, Family, Query};

    #[test]
    fn character_set_membership_preserves_supplementary_scalars() {
        // SAFETY: None selects the default allocator, and the UTF-32 range contains
        // exactly one valid scalar. This character set does not depend on host fonts.
        let character_set = unsafe {
            CFCharacterSet::with_characters_in_range(
                None,
                CFRange {
                    location: 0x1f600,
                    length: 1,
                },
            )
        }
        .expect("create a supplementary character set");
        assert!(character_set_contains_scalar(&character_set, '\u{1f600}'));
        assert!(!character_set_contains_scalar(&character_set, '\u{f600}'));
        assert!(!character_set_contains_scalar(&character_set, '\u{1f601}'));
    }

    fn arial() -> FaceInfo {
        let mut database = Database::new();
        database.load_system_fonts();
        let id = database
            .query(&Query {
                families: &[Family::Name("Arial")],
                ..Query::default()
            })
            .expect("requires a registered Arial font");
        database.face(id).unwrap().clone()
    }

    #[test]
    #[ignore = "requires macOS Arial and its system fallback fonts"]
    fn arial_check_mark_fallback_is_a_non_emoji_discovery_candidate() {
        let face = fallback('\u{2713}', &arial()).expect("Arial check mark fallback");
        assert!(!face.family.to_ascii_lowercase().contains("emoji"));
        assert!(!face.post_script_name.to_ascii_lowercase().contains("emoji"));

        let name = CFString::from_str(&face.post_script_name);
        // SAFETY: `name` is live, and a null matrix requests the identity transform.
        let font = unsafe { CTFont::with_name(&name, 0.0, ptr::null()) };
        // SAFETY: The retained font is live and the returned name is owned.
        assert_eq!(
            unsafe { font.post_script_name() }.to_string(),
            face.post_script_name
        );
        // SAFETY: The retained font is live and the returned character set is owned.
        let character_set = unsafe { font.character_set() };
        assert!(character_set_contains_scalar(&character_set, '\u{2713}'));
    }

    #[test]
    #[ignore = "requires macOS Arial and its system fallback fonts"]
    fn unassigned_scalar_does_not_accept_last_resort() {
        assert!(fallback('\u{0378}', &arial()).is_none());
    }

    #[test]
    #[ignore = "requires macOS Arial and its system fallback fonts"]
    fn unknown_base_name_does_not_use_a_substituted_cascade() {
        let mut base = arial();
        base.post_script_name = "MermanMissingFont-46da48fb3f884be4".into();
        assert!(fallback('\u{2713}', &base).is_none());
    }
}

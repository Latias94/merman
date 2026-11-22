//! Core Text character fallback discovery. Character-set membership only filters candidates;
//! the caller admits font data, validates nonzero glyph IDs and their bounds, and shapes text.

use std::{path::Path, ptr};

use objc2_core_foundation::{
    CFCharacterSet, CFData, CFRange, CFRetained, CFString, CFURL, CFURLPathStyle,
};
use objc2_core_text::{CTFont, CTFontTableOptions, kCTFontURLAttribute};
use usvg::fontdb::FaceInfo;

use super::super::{NativeFontError, OperationControl, OperationPhase, check_limit, source_error};

/// Materializes one registered collection face without copying the original TTC.
pub(super) fn collection_snapshot(
    post_script_name: &str,
    path: &Path,
    source: &ttf_parser::RawFace<'_>,
    limit: usize,
    control: &OperationControl,
) -> Result<Option<Vec<u8>>, NativeFontError> {
    control.terminal_checkpoint_at(OperationPhase::Layout)?;
    super::super::registry::verify_font_file(path, control, OperationPhase::Layout)?;
    if post_script_name.is_empty() || post_script_name.starts_with('.') {
        return Ok(None);
    }
    let name = CFString::from_str(post_script_name);
    // SAFETY: The retained name is live and a null matrix requests identity.
    let font = unsafe { CTFont::with_name(&name, 0.0, ptr::null()) };
    // SAFETY: The font is live; accessors return retained objects.
    let actual_name = unsafe { font.post_script_name() };
    if actual_name.to_string() != post_script_name {
        return Ok(None);
    }
    // A registered font with a colliding name must not replace the selected source.
    // SAFETY: Core Text defines this attribute as a CFURL; downcast checks its type.
    let Some(url) = (unsafe { font.attribute(kCTFontURLAttribute) })
        .and_then(|attribute| attribute.downcast::<CFURL>().ok())
        .and_then(|url| url.file_system_path(CFURLPathStyle::CFURLPOSIXPathStyle))
    else {
        return Ok(None);
    };
    let expected_path = std::fs::canonicalize(path).map_err(|error| source_error(path, error))?;
    if std::fs::canonicalize(url.to_string()).ok().as_ref() != Some(&expected_path) {
        return Ok(None);
    }
    // SAFETY: The retained font is live; table tags are unboxed pointer-sized values.
    let tags = unsafe { font.available_tables(CTFontTableOptions::NoOptions) }
        .ok_or_else(|| source_error(path, "selected Core Text face exposes no tables"))?;
    check_limit("font-tables", tags.count() as usize, usize::from(u16::MAX))?;
    let mut tables = Vec::new();
    let mut total = 12_usize;
    for index in 0..tags.count() {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        // SAFETY: The index is in bounds. Table tags are not CF objects.
        let value = unsafe { tags.value_at_index(index) } as usize;
        let tag = u32::try_from(value)
            .map_err(|_| source_error(path, "Core Text returned an invalid table tag"))?;
        if tag == u32::from_be_bytes(*b"hvgl") {
            return Ok(None);
        }
        if tag == u32::from_be_bytes(*b"DSIG") {
            continue;
        }
        // SAFETY: The retained font is live. This returns immutable retained table data;
        // Core Text owns its backing storage and no mutable reference is exposed here.
        let table = unsafe { font.table(tag, CTFontTableOptions::NoOptions) }
            .ok_or_else(|| source_error(path, "Core Text omitted an advertised font table"))?;
        let aligned = table.len().checked_add(3).map(|length| length & !3);
        total = aligned
            .and_then(|length| length.checked_add(16))
            .and_then(|length| total.checked_add(length))
            .ok_or_else(|| source_error(path, "selected font table sizes overflow"))?;
        if total >= source.data.len() {
            // Shared/overlapping tables can make a standalone font larger than its
            // container. Keep the existing whole-file representation in that case.
            return Ok(None);
        }
        check_limit("system-font-bytes", total, limit)?;
        tables.push((tag, table));
    }
    control.terminal_checkpoint_at(OperationPhase::Layout)?;
    if !tables_match_source(source, &tables) {
        // A TTC may contain colliding PostScript names. The native object is usable only
        // when its actual tables describe the selected physical face, not merely its name.
        return Ok(None);
    }
    let data = super::super::sfnt::from_tables(
        tables.iter().map(|(tag, data)| {
            // SAFETY: Retained immutable CFData values outlive serialization, and are never mutated.
            (*tag, unsafe { data.as_bytes_unchecked() })
        }),
        limit,
        control,
        path,
    )?;
    Ok(Some(data))
}

fn tables_match_source(
    source: &ttf_parser::RawFace<'_>,
    tables: &[(u32, CFRetained<CFData>)],
) -> bool {
    let retained = source
        .table_records
        .into_iter()
        .filter(|record| record.tag.0 != u32::from_be_bytes(*b"DSIG"))
        .count();
    retained == tables.len()
        && tables.iter().all(|(tag, data)| {
            // SAFETY: Core Text or the test constructor owns immutable retained data;
            // it stays live through this comparison and is not mutated by this adapter.
            let data = unsafe { data.as_bytes_unchecked() };
            source.table(ttf_parser::Tag(*tag)) == Some(data)
        })
}

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
    fn native_tables_cannot_substitute_a_different_face_with_identical_names() {
        let bytes = include_bytes!("../../../tests/fonts/Cousine-Regular.ttf");
        let source = ttf_parser::RawFace::parse(bytes, 0).unwrap();
        let mut tables: Vec<_> = source
            .table_records
            .into_iter()
            .filter(|record| record.tag.0 != u32::from_be_bytes(*b"DSIG"))
            .map(|record| {
                (
                    record.tag.0,
                    CFData::from_bytes(source.table(record.tag).unwrap()),
                )
            })
            .collect();
        assert!(tables_match_source(&source, &tables));

        let (_, metrics) = tables
            .iter_mut()
            .find(|(tag, _)| *tag == u32::from_be_bytes(*b"hmtx"))
            .unwrap();
        // SAFETY: The test owns immutable retained data and copies it before replacing the handle.
        let mut changed = unsafe { metrics.as_bytes_unchecked() }.to_vec();
        changed[0] ^= 1;
        *metrics = CFData::from_bytes(&changed);
        assert!(
            !tables_match_source(&source, &tables),
            "equal name and style tables do not establish physical face identity"
        );
        tables.retain(|(tag, _)| *tag != u32::from_be_bytes(*b"hmtx"));
        assert!(!tables_match_source(&source, &tables));
    }

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

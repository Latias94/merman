//! Retained Core Text selections and bounded snapshots of their immutable tables.
//! Fallback checks the selected font's Unicode mapping before snapshot admission.

use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    ptr,
};

use objc2_core_foundation::{
    CFArray, CFData, CFRange, CFRetained, CFString, CFURL, CFURLPathStyle,
};
use objc2_core_text::{CTFont, CTFontDescriptor, CTFontTableOptions, kCTFontURLAttribute};
use usvg::fontdb::{FaceInfo, ID, Stretch, Style, Weight};

use super::super::{
    FontByteBudget, NativeFontError, OperationControl, OperationPhase, check_limit, source_error,
};

mod query;

const DSIG: u32 = u32::from_be_bytes(*b"DSIG");
const HVGL: u32 = u32::from_be_bytes(*b"hvgl");
const HEAD: u32 = u32::from_be_bytes(*b"head");

type FontTables = Vec<(u32, CFRetained<CFData>)>;

/// An immutable selection created exclusively through Core Text.
///
/// Keep the handle private: neither a raw CTFont nor its toll-free NSFont bridge may
/// escape this adapter. Paths and names are metadata, not native object identity.
#[derive(Clone, Debug)]
pub(in crate::fonts) struct NativeFace {
    font: CFRetained<CTFont>,
    pub(in crate::fonts) path: PathBuf,
    pub(in crate::fonts) family: String,
    pub(in crate::fonts) post_script_name: String,
}

// SAFETY: Apple's CTFont.h "Thread Safety Information" states that its functions
// are thread safe unless otherwise specified. core-text 21.0.0's font.rs likewise
// marks its retained CTFont wrapper Send + Sync. Here the guarantee is narrower:
// only Core Text constructors create this private, immutable handle, and no API
// exposes it to NSFont/AppKit or accepts a caller-owned (possibly bridged) CTFont.
// CFRetain/CFRelease are thread safe; the remaining fields are owned Rust values.
unsafe impl Send for NativeFace {}
// SAFETY: The same closed-handle invariant applies to shared access. Every method
// uses immutable Core Text/CF accessors; no mutable native aliases are exposed.
unsafe impl Sync for NativeFace {}

impl PartialEq for NativeFace {
    fn eq(&self, other: &Self) -> bool {
        // objc2-core-foundation forwards retained CTFont equality to CFEqual.
        self.font == other.font
    }
}

impl Eq for NativeFace {}

impl Hash for NativeFace {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // The CTFont Hash implementation uses CFHash, consistent with CFEqual.
        self.font.hash(state);
    }
}

/// Matches a named family without reconstructing the selection from its name or URL.
pub(super) fn named(
    family: &str,
    weight: Weight,
    stretch: Stretch,
    style: Style,
) -> Option<NativeFace> {
    query::match_family(family, weight, stretch, style, None)
}

impl NativeFace {
    /// Accept only handles from this module's immutable Core Text constructors.
    fn from_font(font: CFRetained<CTFont>) -> Option<Self> {
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

        // Copying tables does not instantiate variable outlines. Since macOS 10.12,
        // CTFontCopyVariation reports only non-default coordinates. Until native
        // coordinates can reach both shaping and painting, decline all such instances,
        // including wght and automatic opsz; a default variable face remains usable.
        // SAFETY: The font is live and Core Text returns an immutable retained dictionary.
        if unsafe { font.variation() }.is_some_and(|variation| !variation.is_empty()) {
            return None;
        }

        // SAFETY: The font is live; Core Text returns an owned array of unboxed tags.
        let tags = unsafe { font.available_tables(CTFontTableOptions::NoOptions) }?;
        for index in 0..tags.count() {
            // SAFETY: The index is in bounds; these pointer slots contain integers,
            // not CF objects. Never dereference or retain the slot values.
            let tag = u32::try_from(unsafe { tags.value_at_index(index) } as usize).ok()?;
            if tag == HVGL {
                return None;
            }
        }

        Some(Self {
            path: font_path(&font)?,
            font,
            family,
            post_script_name,
        })
    }

    /// Returns the largest nonempty outline table and its length before choosing a snapshot.
    pub(in crate::fonts) fn largest_outline(&self) -> Option<(ttf_parser::Tag, usize)> {
        [b"glyf", b"CFF ", b"CFF2"]
            .into_iter()
            .map(ttf_parser::Tag::from_bytes)
            .filter_map(|tag| {
                // SAFETY: The font is live; the returned CFData is retained while reading its length.
                let table = unsafe { self.font.table(tag.0, CTFontTableOptions::NoOptions) }?;
                let length = table.len();
                (length > 0).then_some((tag, length))
            })
            .max_by_key(|&(_, length)| length)
    }

    /// Repackages this selection at index zero without opening its source file.
    pub(in crate::fonts) fn snapshot(
        &self,
        budget: FontByteBudget,
        control: &OperationControl,
    ) -> Result<Vec<u8>, NativeFontError> {
        let tables = collect_tables(&self.font, &self.path, budget, control)?;
        super::super::sfnt::from_tables(
            tables.iter().map(|(tag, data)| {
                // SAFETY: The retained immutable data outlives serialization.
                (*tag, unsafe { data.as_bytes_unchecked() })
            }),
            budget,
            control,
            &self.path,
        )
    }

    /// A rejection-only prefilter; admission must still validate the actual glyph.
    pub(in crate::fonts) fn covers(&self, character: char) -> bool {
        font_covers(&self.font, character)
    }

    /// Preserves the exact native origin when following its character cascade.
    pub(in crate::fonts) fn fallback(&self, character: char) -> Option<Self> {
        fallback_from_font(&self.font, character)
    }

    /// Proves that already admitted bytes describe this selected physical face.
    /// Collections may supply a nonzero index; only new snapshots use index zero.
    pub(in crate::fonts) fn matches_data(
        &self,
        data: &[u8],
        index: u32,
        control: &OperationControl,
    ) -> Result<bool, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let Ok(source) = ttf_parser::RawFace::parse(data, index) else {
            return Ok(false);
        };
        let names = source
            .table(ttf_parser::Tag::from_bytes(b"name"))
            .and_then(ttf_parser::name::Table::parse);
        if !names
            .is_some_and(|names| super::post_script_name_matches(names, &self.post_script_name))
        {
            return Ok(false);
        }
        let source_count = source
            .table_records
            .into_iter()
            .filter(|record| record.tag.0 != DSIG)
            .count();
        // SAFETY: The font is live; Core Text returns an owned array of unboxed tags.
        let tags = unsafe { self.font.available_tables(CTFontTableOptions::NoOptions) }
            .ok_or_else(|| source_error(&self.path, "selected Core Text face exposes no tables"))?;
        let mut compared = 0;
        for index in 0..tags.count() {
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            // SAFETY: The index is in bounds; table tags are unboxed integers.
            let tag = u32::try_from(unsafe { tags.value_at_index(index) } as usize)
                .map_err(|_| source_error(&self.path, "Core Text returned an invalid table tag"))?;
            if tag == HVGL {
                return Ok(false);
            }
            if tag == DSIG {
                continue;
            }
            let Some(source_table) = source.table(ttf_parser::Tag(tag)) else {
                return Ok(false);
            };
            // Compare one retained table at a time, without a second SFNT allocation.
            let native_table = copy_table(&self.font, tag, &self.path)?;
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            // SAFETY: This immutable retained table stays live through the comparison.
            let native_table = unsafe { native_table.as_bytes_unchecked() };
            if !table_matches_snapshot(tag, native_table, source_table) {
                return Ok(false);
            }
            compared += 1;
        }
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        Ok(compared == source_count)
    }

    /// Checks metadata against admitted bytes, using their actual collection index.
    /// The registry, which knows whether the data was rebuilt, owns the index-zero check.
    pub(in crate::fonts) fn validate(&self, face: &FaceInfo) -> Result<(), NativeFontError> {
        if !super::matches_post_script_name(face, &self.post_script_name)
            || !super::matches_family(face, &self.family)
        {
            return Err(source_error(
                &self.path,
                "font properties changed after Core Text lookup; retry the operation",
            ));
        }
        Ok(())
    }
}

/// Tries the retained native origin first, then the ordered Core Text cascade.
///
/// `accept` owns admission and may return `Ok(None)` to reject one candidate and
/// continue. Its errors, including cancellation and resource limits, stop the
/// cascade and are returned unchanged. No system font directory is enumerated.
pub(in crate::fonts) fn fallback_candidates(
    character: char,
    base: &FaceInfo,
    origin: Option<&NativeFace>,
    control: &OperationControl,
    accept: &mut dyn FnMut(NativeFace) -> Result<Option<ID>, NativeFontError>,
) -> Result<Option<ID>, NativeFontError> {
    control.terminal_checkpoint_at(OperationPhase::Layout)?;
    let native_base = match origin {
        Some(origin) => Some(origin.clone()),
        None => registered_base(base, control)?,
    };
    let Some(native_base) =
        native_base.or_else(|| named("Helvetica", base.weight, base.stretch, base.style))
    else {
        return Ok(None);
    };

    let mut seen = HashSet::new();
    if let Some(candidate) = native_base.fallback(character)
        && seen.insert(candidate.clone())
    {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        if let Some(id) = accept(candidate)? {
            return Ok(Some(id));
        }
    }

    control.terminal_checkpoint_at(OperationPhase::Layout)?;
    let Some(cascade) = (unsafe { native_base.font.default_cascade_list_for_languages(None) })
    else {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        return Ok(None);
    };
    control.terminal_checkpoint_at(OperationPhase::Layout)?;
    // Core Text documents this result as an array of CTFontDescriptorRef values.
    // The typed view lets CFArray retain each descriptor before its next iteration.
    let cascade: CFRetained<CFArray<CTFontDescriptor>> =
        unsafe { CFRetained::cast_unchecked(cascade) };
    for index in 0..cascade.len() {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let Some(descriptor) = cascade.get(index) else {
            continue;
        };
        let Some(font) = query::font_from_descriptor(&descriptor) else {
            continue;
        };
        let Some(candidate) = query::fallback_face(font, character) else {
            continue;
        };
        if !seen.insert(candidate.clone()) {
            continue;
        }
        if let Some(id) = accept(candidate)? {
            return Ok(Some(id));
        }
    }
    control.terminal_checkpoint_at(OperationPhase::Layout)?;
    Ok(None)
}

fn registered_base(
    base: &FaceInfo,
    control: &OperationControl,
) -> Result<Option<NativeFace>, NativeFontError> {
    let usvg::fontdb::Source::Binary(data) = &base.source else {
        return Ok(None);
    };
    let Some(candidate) = registered_base_font(base).and_then(NativeFace::from_font) else {
        return Ok(None);
    };
    // A registered face can share the PostScript name of a caller's subset or replacement.
    Ok(candidate
        .matches_data(data.as_ref().as_ref(), base.index, control)?
        .then_some(candidate))
}

fn registered_base_font(base: &FaceInfo) -> Option<CFRetained<CTFont>> {
    if base.post_script_name.is_empty() || base.post_script_name.starts_with('.') {
        return None;
    }
    let name = CFString::from_str(&base.post_script_name);
    // SAFETY: The retained name is live and a null matrix requests identity.
    let font = unsafe { CTFont::with_name(&name, 0.0, ptr::null()) };
    // CTFontCreateWithName may substitute a different font for an unknown name.
    // An explicit face may use this path only when the actual PS name is equal.
    // SAFETY: The retained font is live; the accessor returns an owned string.
    let actual_name = unsafe { font.post_script_name() };
    (actual_name.to_string() == base.post_script_name).then_some(font)
}

fn font_path(font: &CTFont) -> Option<PathBuf> {
    // SAFETY: Core Text defines this attribute as a CFURL; downcast checks its type.
    // Converting its URL to a path does not access or canonicalize the source file.
    unsafe { font.attribute(kCTFontURLAttribute) }?
        .downcast::<CFURL>()
        .ok()?
        .file_system_path(CFURLPathStyle::CFURLPOSIXPathStyle)
        .map(|path| PathBuf::from(path.to_string()))
}

fn table_matches_snapshot(tag: u32, native: &[u8], source: &[u8]) -> bool {
    native == source
        || (tag == HEAD
            && native.len() == source.len()
            && native.len() >= 12
            // Only head.checkSumAdjustment changes when repackaging an SFNT.
            && native[..8] == source[..8]
            && native[12..] == source[12..])
}

/// Bounds retained table payloads before serialization.
fn collect_tables(
    font: &CTFont,
    path: &Path,
    budget: FontByteBudget,
    control: &OperationControl,
) -> Result<FontTables, NativeFontError> {
    control.terminal_checkpoint_at(OperationPhase::Layout)?;
    let mut total = 12_usize;
    budget.check(total as u64)?;
    // SAFETY: The retained font is live; table tags are unboxed pointer-sized values.
    let tags = unsafe { font.available_tables(CTFontTableOptions::NoOptions) }
        .ok_or_else(|| source_error(path, "selected Core Text face exposes no tables"))?;
    check_limit("font-tables", tags.count() as usize, usize::from(u16::MAX))?;
    let mut tables = Vec::new();
    for index in 0..tags.count() {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        // SAFETY: The index is in bounds. Table tags are not CF objects.
        let value = unsafe { tags.value_at_index(index) } as usize;
        let tag = u32::try_from(value)
            .map_err(|_| source_error(path, "Core Text returned an invalid table tag"))?;
        if tag == HVGL {
            return Err(source_error(path, "selected Core Text face exposes hvgl"));
        }
        if tag == DSIG {
            continue;
        }
        let table = copy_table(font, tag, path)?;
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let aligned = table.len().checked_add(3).map(|length| length & !3);
        total = aligned
            .and_then(|length| length.checked_add(16))
            .and_then(|length| total.checked_add(length))
            .ok_or_else(|| source_error(path, "selected font table sizes overflow"))?;
        budget.check(total as u64)?;
        tables.push((tag, table));
    }
    control.terminal_checkpoint_at(OperationPhase::Layout)?;
    Ok(tables)
}

fn copy_table(font: &CTFont, tag: u32, path: &Path) -> Result<CFRetained<CFData>, NativeFontError> {
    // SAFETY: The retained font is live. Core Text owns this immutable table's backing
    // storage, and no mutable reference is exposed by this adapter.
    unsafe { font.table(tag, CTFontTableOptions::NoOptions) }
        .ok_or_else(|| source_error(path, "Core Text omitted an advertised font table"))
}

/// Compatibility lookup for an explicit/non-native base. Native admissions must
/// use `NativeFace::fallback` to preserve their retained origin, including name collisions.
#[cfg(test)]
pub(super) fn fallback(character: char, base: &FaceInfo) -> Option<NativeFace> {
    let base_font = registered_base_font(base)?;
    fallback_from_font(&base_font, character)
}

fn fallback_from_font(base: &CTFont, character: char) -> Option<NativeFace> {
    let mut utf8 = [0; 4];
    let text = CFString::from_str(character.encode_utf8(&mut utf8));
    let range = CFRange {
        location: 0,
        length: text.length(),
    };
    // SAFETY: The retained base and string are live and the range covers the complete
    // CFString in UTF-16 code units, including both halves of a surrogate pair.
    let font = unsafe { CTFont::for_string(base, &text, range) };
    query::fallback_face(font, character)
}

fn is_last_resort(name: &str) -> bool {
    let name = name.trim_start_matches('.');
    name.eq_ignore_ascii_case("LastResort") || name.eq_ignore_ascii_case("Last Resort")
}

fn font_covers(font: &CTFont, character: char) -> bool {
    // Core Text can synthesize mappings absent from the SFNT (Arial U+A789), and its
    // character set can omit real glyphs (PingFang SC U+2DA5A). Use the same coverage
    // rule as admission on borrowed tables, before acquiring any outlines or bitmaps.
    let [Some(cmap), Some(maxp)] = [*b"cmap", *b"maxp"].map(|tag| {
        // SAFETY: The selected font is live and the returned table data is retained.
        unsafe { font.table(u32::from_be_bytes(tag), CTFontTableOptions::NoOptions) }
    }) else {
        return false;
    };
    // SAFETY: Both immutable CFData handles outlive the borrowed table parsers.
    let (Some(cmap), Some(maxp)) = (unsafe {
        (
            ttf_parser::cmap::Table::parse(cmap.as_bytes_unchecked()),
            ttf_parser::maxp::Table::parse(maxp.as_bytes_unchecked()),
        )
    }) else {
        return false;
    };
    crate::font_selection::cmap_covers_character(cmap, maxp.number_of_glyphs.get(), character)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use usvg::fontdb::{Database, Family, Query};

    #[test]
    #[ignore = "requires registered macOS Arial and PingFang SC faces"]
    fn cascade_coverage_and_snapshot_limits_follow_the_selected_cmap() {
        use crate::fonts::{
            NativeFontLimits, registry::FontSourceRegistry, resolver::FontCandidate,
        };
        use std::sync::Arc;

        let limits = NativeFontLimits {
            max_files: 0,
            ..Default::default()
        };
        let registry = FontSourceRegistry::new(Default::default(), Default::default(), 0, 0);
        let mut database = Arc::new(Database::new());
        for (family, character) in [("Arial", '\u{a789}'), ("PingFang SC", '\u{2da5a}')] {
            let face = named(family, Weight::NORMAL, Stretch::Normal, Style::Normal).unwrap();
            // Read the installed revision's mapping without snapshotting the whole CJK face.
            // SAFETY: The selected font and its immutable cmap remain live during parsing.
            let cmap = unsafe {
                face.font
                    .table(u32::from_be_bytes(*b"cmap"), CTFontTableOptions::NoOptions)
            }
            .unwrap();
            let cmap =
                ttf_parser::cmap::Table::parse(unsafe { cmap.as_bytes_unchecked() }).unwrap();
            // SAFETY: The retained font is live and this accessor only reads its glyph count.
            let num_glyphs = u16::try_from(unsafe { face.font.glyph_count() }).unwrap();
            let mapped = crate::font_selection::cmap_covers_character(cmap, num_glyphs, character);
            assert_eq!(
                query::fallback_face(face.font.clone(), character).is_some(),
                mapped,
                "{family} {character:?}"
            );
            let result = registry.materialize(
                FontCandidate::Native(face),
                Some(character),
                &mut database,
                &limits,
                &OperationControl::new(),
            );
            if mapped {
                assert!(
                    matches!(
                        result,
                        Err(NativeFontError::Limit {
                            resource: "system-snapshot-files",
                            ..
                        })
                    ),
                    "{family} {character:?}: {result:?}"
                );
            } else {
                assert_eq!(result.unwrap(), None, "{family} {character:?}");
            }
        }
    }

    #[test]
    fn native_sfnt_comparison_only_ignores_head_checksum_adjustment() {
        let mut source = [0_u8; 16];
        source[..8].copy_from_slice(b"headtest");
        source[12..].copy_from_slice(b"tail");
        let mut rebuilt = source;
        rebuilt[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(table_matches_snapshot(HEAD, &rebuilt, &source));

        rebuilt[12] ^= 1;
        assert!(!table_matches_snapshot(HEAD, &rebuilt, &source));
        assert!(!table_matches_snapshot(
            u32::from_be_bytes(*b"hhea"),
            &rebuilt,
            &source
        ));
    }

    #[test]
    #[ignore = "requires macOS with a registered Arial face"]
    fn native_snapshot_round_trips_and_reuses_coretext_identity() {
        let first = named("Arial", Weight::NORMAL, Stretch::Normal, Style::Normal)
            .expect("registered Arial Regular");
        let second = named("aRiAl", Weight::NORMAL, Stretch::Normal, Style::Normal)
            .expect("registered Arial Regular");
        assert_eq!(first, second);

        let mut first_hash = DefaultHasher::new();
        first.hash(&mut first_hash);
        let mut second_hash = DefaultHasher::new();
        second.hash(&mut second_hash);
        assert_eq!(first_hash.finish(), second_hash.finish());

        let bytes = first
            .snapshot(
                FontByteBudget::new("system-font-bytes", 0, 64 * 1024 * 1024),
                &OperationControl::new(),
            )
            .expect("Core Text tables form a bounded standalone SFNT");
        let parsed = ttf_parser::Face::parse(&bytes, 0).expect("snapshot parses as SFNT");
        assert!(
            parsed
                .raw_face()
                .table(ttf_parser::Tag::from_bytes(b"DSIG"))
                .is_none()
        );
        assert!(
            first
                .matches_data(&bytes, 0, &OperationControl::new())
                .expect("native table comparison completes")
        );

        let offset = {
            let raw = ttf_parser::RawFace::parse(&bytes, 0).expect("snapshot has raw tables");
            let table = raw
                .table(ttf_parser::Tag::from_bytes(b"hhea"))
                .expect("Arial has hhea");
            table.as_ptr() as usize - bytes.as_ptr() as usize
        };
        let mut changed = bytes.clone();
        changed[offset] ^= 1;
        assert!(
            !first
                .matches_data(&changed, 0, &OperationControl::new())
                .expect("changed table comparison completes")
        );

        let fallback = first
            .fallback('\u{2713}')
            .expect("Arial fallback for check mark");
        assert!(fallback.covers('\u{2713}'));
    }

    #[test]
    #[ignore = "requires the registered macOS Arial face"]
    fn explicit_base_with_same_postscript_name_must_match_native_tables() {
        let native = named("Arial", Weight::NORMAL, Stretch::Normal, Style::Normal).unwrap();
        let control = OperationControl::new();
        let mut bytes = native
            .snapshot(
                FontByteBudget::new("system-font-bytes", 0, 64 * 1024 * 1024),
                &control,
            )
            .unwrap();
        let mut database = Database::new();
        database.load_font_data(bytes.clone());
        assert!(
            registered_base(database.faces().next().unwrap(), &control)
                .unwrap()
                .is_some()
        );

        let raw = ttf_parser::RawFace::parse(&bytes, 0).unwrap();
        let hmtx = raw.table(ttf_parser::Tag::from_bytes(b"hmtx")).unwrap();
        let advance = hmtx.as_ptr() as usize - bytes.as_ptr() as usize;
        bytes[advance] ^= 1;
        let mut replacement = Database::new();
        replacement.load_font_data(bytes);
        let face = replacement.faces().next().unwrap();
        assert_eq!(face.post_script_name, native.post_script_name);
        assert!(registered_base(face, &control).unwrap().is_none());
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
        assert!(font_covers(&font, '\u{2713}'));
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

    #[test]
    #[ignore = "requires a registered macOS Arial face"]
    fn distinct_coretext_handles_share_one_physical_face_and_byte_charge() {
        use crate::fonts::{
            NativeFontLimits, registry::FontSourceRegistry, resolver::FontCandidate,
        };
        use std::sync::Arc;

        let name = CFString::from_str("ArialMT");
        // SAFETY: The name and font stay live; null matrices and no descriptor preserve the face.
        let small = unsafe { CTFont::with_name(&name, 12.0, ptr::null()) };
        let large = unsafe { small.copy_with_attributes(24.0, ptr::null(), None) };
        assert_ne!(small, large, "Core Text distinguishes these point sizes");
        let small = NativeFace::from_font(small).unwrap();
        let large = NativeFace::from_font(large).unwrap();
        assert_eq!(small.post_script_name, "ArialMT");
        assert_eq!(small.path, large.path);

        let control = OperationControl::new();
        let data = small
            .snapshot(
                FontByteBudget::new(
                    "system-font-bytes",
                    0,
                    NativeFontLimits::default().max_bytes,
                ),
                &control,
            )
            .unwrap();
        assert!(large.matches_data(&data, 0, &control).unwrap());

        let registry = FontSourceRegistry::new(Default::default(), Default::default(), 0, 0);
        let mut database = Arc::new(Database::new());
        let limits = NativeFontLimits {
            max_files: 1,
            max_faces: 1,
            max_bytes: data.len(),
            ..Default::default()
        };
        let mut admit = |face, character| {
            registry
                .materialize(
                    FontCandidate::Native(face),
                    character,
                    &mut database,
                    &limits,
                    &control,
                )
                .unwrap()
                .unwrap()
        };
        let id = admit(small, None);
        assert_eq!(admit(large, Some('A')), id);
        assert_eq!(registry.admitted_faces(), 1);
        assert_eq!(registry.resident_bytes(), data.len());
    }
}

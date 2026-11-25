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
use objc2_core_text::{
    CTFont, CTFontDescriptor, CTFontOptions, CTFontTableOptions, kCTFontURLAttribute,
};
use usvg::FontVariation;
use usvg::fontdb::{FaceInfo, ID, Stretch, Style, Weight};

use super::super::{
    FontByteBudget, NativeFontError, OperationControl, OperationPhase, check_limit, source_error,
};
use super::GlyphFallback;

mod coverage;
mod query;
mod variations;

const DSIG: u32 = u32::from_be_bytes(*b"DSIG");
const HVGL: u32 = u32::from_be_bytes(*b"hvgl");
const HEAD: u32 = u32::from_be_bytes(*b"head");
const NAME: u32 = u32::from_be_bytes(*b"name");
const REQUIRED_TABLES: [u32; 5] = [
    HEAD,
    NAME,
    u32::from_be_bytes(*b"cmap"),
    u32::from_be_bytes(*b"hhea"),
    u32::from_be_bytes(*b"maxp"),
];
const LOCAL_FONT_OPTIONS: CTFontOptions =
    CTFontOptions::PreventAutoActivation.union(CTFontOptions::PreventAutoDownload);

type FontTables = Vec<(u32, CFRetained<CFData>)>;

/// An immutable selection created exclusively through Core Text.
///
/// Keep the handle private: neither a raw CTFont nor its toll-free NSFont bridge may
/// escape this adapter. Paths and names are metadata, not native object identity.
#[derive(Clone, Debug)]
pub(in crate::fonts) struct NativeFace {
    font: CFRetained<CTFont>,
    pub(in crate::fonts) path: Option<PathBuf>,
    pub(in crate::fonts) family: String,
    pub(in crate::fonts) post_script_name: String,
    pub(in crate::fonts) variations: Vec<FontVariation>,
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
        self.font == other.font && self.variations == other.variations
    }
}

impl Eq for NativeFace {}

impl Hash for NativeFace {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // The CTFont Hash implementation uses CFHash, consistent with CFEqual.
        self.font.hash(state);
        self.variations.hash(state);
    }
}

/// Matches a named family without reconstructing the selection from its name or URL.
#[cfg(test)]
pub(super) fn named(
    family: &str,
    weight: Weight,
    stretch: Stretch,
    style: Style,
) -> Option<NativeFace> {
    query::match_family(family, (weight, stretch, style), None, |face| {
        Ok(Some(face))
    })
    .ok()
    .flatten()
}

pub(super) fn named_candidates(
    family: &str,
    weight: Weight,
    stretch: Stretch,
    style: Style,
    control: &OperationControl,
) -> Result<Vec<NativeFace>, NativeFontError> {
    let mut candidates = Vec::new();
    query::match_family(family, (weight, stretch, style), Some(control), |face| {
        candidates.push(face);
        Ok(None::<()>)
    })?;
    Ok(candidates)
}

pub(super) fn system_ui(
    weight: Weight,
    stretch: Stretch,
    style: Style,
    language: Option<&str>,
    control: &OperationControl,
) -> Result<Option<NativeFace>, NativeFontError> {
    query::system_ui((weight, stretch, style), language, Some(control))
}

impl NativeFace {
    /// Accept only handles from this module's immutable Core Text constructors.
    /// The original selection descriptor can retain explicit default-valued axes.
    fn from_font(
        font: CFRetained<CTFont>,
        selection_descriptor: Option<&CTFontDescriptor>,
        control: Option<&OperationControl>,
    ) -> Result<Option<Self>, NativeFontError> {
        checkpoint(control)?;
        // SAFETY: The font and its returned descriptor are retained through the test.
        let resolved = unsafe { font.font_descriptor() };
        if !query::descriptor_is_available(&resolved) {
            return Ok(None);
        }
        // SAFETY: The retained font is live and both accessors return owned strings.
        let (family, native_name) = unsafe { (font.family_name(), font.post_script_name()) };
        let family = family.to_string();
        let native_name = native_name.to_string();
        if family.is_empty()
            || native_name.is_empty()
            || is_last_resort(&family)
            || is_last_resort(&native_name)
        {
            return Ok(None);
        }

        let Some(tags) = table_tags(&font, control)? else {
            return Ok(None);
        };
        if tags.contains(&HVGL)
            || !REQUIRED_TABLES.iter().all(|tag| tags.contains(tag))
            || !coverage::has_minimum_tables(&font, control)?
        {
            return Ok(None);
        }

        let name = read_table(&font, NAME, control)?;
        let Some(names) = name
            .as_deref()
            .map(table_bytes)
            .and_then(ttf_parser::name::Table::parse)
        else {
            return Ok(None);
        };
        // A named variable instance can report its resolved name. Use a stable
        // PostScript name from the physical face for snapshots and comparisons.
        let Some(post_script_name) = sfnt_name(names, ttf_parser::name_id::POST_SCRIPT_NAME) else {
            return Ok(None);
        };
        let family = if super::family_names_match(names, &family) {
            family
        } else {
            // Some UI handles report a family alias that exists only in Core Text.
            sfnt_name(names, ttf_parser::name_id::TYPOGRAPHIC_FAMILY)
                .or_else(|| sfnt_name(names, ttf_parser::name_id::FAMILY))
                .unwrap_or(family)
        };
        let Some(variations) = variations::read(&font, selection_descriptor, control)? else {
            return Ok(None);
        };

        Ok(Some(Self {
            path: font_path(&font),
            font,
            family,
            post_script_name,
            variations,
        }))
    }

    pub(in crate::fonts) fn diagnostic_path(&self) -> &Path {
        self.path
            .as_deref()
            .unwrap_or_else(|| Path::new(&self.post_script_name))
    }

    /// Returns the largest outline table available in the retained face.
    pub(in crate::fonts) fn largest_outline(
        &self,
        control: &OperationControl,
    ) -> Result<Option<(ttf_parser::Tag, usize)>, NativeFontError> {
        let mut largest = None;
        for tag in [b"glyf", b"CFF ", b"CFF2"].map(ttf_parser::Tag::from_bytes) {
            if let Some(table) = read_table(&self.font, tag.0, Some(control))? {
                let length = table.len();
                if length > 0 && largest.is_none_or(|(_, previous)| length > previous) {
                    largest = Some((tag, length));
                }
            }
        }
        Ok(largest)
    }

    /// Repackages this selection at index zero without opening its source file.
    pub(in crate::fonts) fn snapshot(
        &self,
        budget: FontByteBudget,
        control: &OperationControl,
    ) -> Result<Vec<u8>, NativeFontError> {
        let path = self.diagnostic_path();
        let tables = collect_tables(&self.font, path, budget, control)?;
        super::super::sfnt::from_tables(
            tables.iter().map(|(tag, data)| (*tag, table_bytes(data))),
            budget,
            control,
            path,
        )
    }

    #[cfg(test)]
    pub(in crate::fonts) fn covers_cluster(&self, request: GlyphFallback<'_>) -> bool {
        coverage::check(&self.font, Some(request), false, None).unwrap_or(false)
    }

    /// Applies the shared suitability policy to the selected face's small tables.
    pub(in crate::fonts) fn implicit_eligible(&self, fallback: Option<GlyphFallback<'_>>) -> bool {
        coverage::check(&self.font, fallback, true, None).unwrap_or(false)
    }

    pub(in crate::fonts) fn check_eligibility(
        &self,
        fallback: Option<GlyphFallback<'_>>,
        implicit: bool,
        control: &OperationControl,
    ) -> Result<bool, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let eligible = coverage::check(&self.font, fallback, implicit, Some(control))?;
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        Ok(eligible)
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
            .table(ttf_parser::Tag(NAME))
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
        let path = self.diagnostic_path();
        let tags = table_tags(&self.font, Some(control))?
            .ok_or_else(|| source_error(path, "selected Core Text face exposes no tables"))?;
        if tags.contains(&HVGL) || tags.iter().filter(|&&tag| tag != DSIG).count() != source_count {
            return Ok(false);
        }
        for tag in tags.into_iter().filter(|&tag| tag != DSIG) {
            let Some(source_table) = source.table(ttf_parser::Tag(tag)) else {
                return Ok(false);
            };
            // Compare one retained table at a time, without a second SFNT allocation.
            let native_table = read_table(&self.font, tag, Some(control))?
                .ok_or_else(|| source_error(path, "Core Text omitted an advertised font table"))?;
            if !table_matches_snapshot(tag, table_bytes(&native_table), source_table) {
                return Ok(false);
            }
        }
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        Ok(true)
    }

    /// Checks metadata against admitted bytes, using their actual collection index.
    /// The registry, which knows whether the data was rebuilt, owns the index-zero check.
    pub(in crate::fonts) fn validate(&self, face: &FaceInfo) -> Result<(), NativeFontError> {
        if !super::matches_post_script_name(face, &self.post_script_name)
            || !super::matches_family(face, &self.family)
        {
            return Err(source_error(
                self.diagnostic_path(),
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
    request: GlyphFallback<'_>,
    base: &FaceInfo,
    origin: Option<&NativeFace>,
    control: &OperationControl,
    accept: &mut dyn FnMut(NativeFace) -> Result<Option<ID>, NativeFontError>,
) -> Result<Option<ID>, NativeFontError> {
    control.terminal_checkpoint_at(OperationPhase::Layout)?;
    if request.text.is_empty() {
        return Ok(None);
    }
    let control = Some(control);
    let style = request.style(base);
    let default_base;
    let base_font = if let Some(origin) = origin {
        &origin.font
    } else {
        let name = CFString::from_str("Helvetica");
        // SAFETY: The name is live and a null matrix requests identity. This
        // font only supplies a cascade when no exact native origin is available.
        default_base =
            unsafe { CTFont::with_name_and_options(&name, 0.0, ptr::null(), LOCAL_FONT_OPTIONS) };
        &default_base
    };

    let language = request.language.map(CFString::from_str);
    let mut seen = HashSet::new();
    let mut consider = |candidate: NativeFace| {
        checkpoint(control)?;
        if seen.insert(CascadeAttempt::from(&candidate)) {
            accept(candidate)
        } else {
            Ok(None)
        }
    };
    checkpoint(control)?;
    let font = fallback_from_font(base_font, request.text, language.as_deref());
    let direct = NativeFace::from_font(font.clone(), None, control)?;
    // Refine a static native result within its actual family when CSS calls for
    // another style. Keep the native result as an alternative, including when
    // it is the only covering face or Core Text selected a variable instance.
    if direct
        .as_ref()
        .is_none_or(|face| face.variations.is_empty())
        && let Some(family) = query::fallback_family(&font, style, direct.is_none())
        && let Some(id) = query::match_family(&family, style, control, &mut consider)?
    {
        return Ok(Some(id));
    }
    if let Some(candidate) = direct
        && let Some(id) = consider(candidate)?
    {
        return Ok(Some(id));
    }

    checkpoint(control)?;
    let languages = language
        .as_deref()
        .map(|language| CFArray::from_objects(&[language]));
    // SAFETY: A supplied language list contains only retained CFString objects.
    let Some(cascade) = (unsafe {
        base_font
            .default_cascade_list_for_languages(languages.as_deref().map(|list| list.as_opaque()))
    }) else {
        checkpoint(control)?;
        return Ok(None);
    };
    checkpoint(control)?;
    // Core Text documents this result as an array of CTFontDescriptorRef values.
    // The typed view lets CFArray retain each descriptor before its next iteration.
    let cascade: CFRetained<CFArray<CTFontDescriptor>> =
        unsafe { CFRetained::cast_unchecked(cascade) };
    for index in 0..cascade.len() {
        checkpoint(control)?;
        let Some(descriptor) = cascade.get(index) else {
            continue;
        };
        let Some(font) = query::font_from_descriptor(&descriptor, control)? else {
            continue;
        };
        let Some(candidate) = NativeFace::from_font(font, Some(&descriptor), control)? else {
            continue;
        };
        if let Some(id) = consider(candidate)? {
            return Ok(Some(id));
        }
    }
    checkpoint(control)?;
    Ok(None)
}

#[derive(PartialEq, Eq, Hash)]
enum CascadeAttempt {
    File(PathBuf, String, Vec<FontVariation>),
    TableOnly(NativeFace),
}

impl From<&NativeFace> for CascadeAttempt {
    fn from(face: &NativeFace) -> Self {
        // A registered file URL and physical SFNT name identify duplicate cascade
        // entries across Core Text point sizes. The registry verifies the bytes.
        match &face.path {
            Some(path) => Self::File(
                path.clone(),
                face.post_script_name.clone(),
                face.variations.clone(),
            ),
            None => Self::TableOnly(face.clone()),
        }
    }
}

/// Resolve a caller-owned base to a native handle only after proving table identity.
/// The registry can cache this result by physical source and face index.
pub(in crate::fonts) fn registered_base(
    base: &FaceInfo,
    control: &OperationControl,
) -> Result<Option<NativeFace>, NativeFontError> {
    control.terminal_checkpoint_at(OperationPhase::Layout)?;
    let usvg::fontdb::Source::Binary(data) = &base.source else {
        return Ok(None);
    };
    let Some(font) = registered_base_font(base) else {
        return Ok(None);
    };
    let Some(candidate) = NativeFace::from_font(font, None, Some(control))? else {
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
    let font =
        unsafe { CTFont::with_name_and_options(&name, 0.0, ptr::null(), LOCAL_FONT_OPTIONS) };
    // CTFontCreateWithName may substitute a different font for an unknown name.
    // An explicit face may use this path only when the actual PS name is equal.
    // SAFETY: The retained font is live; the accessor returns an owned string.
    let actual_name = unsafe { font.post_script_name() };
    (actual_name.to_string() == base.post_script_name).then_some(font)
}

fn font_path(font: &CTFont) -> Option<PathBuf> {
    // SAFETY: Core Text defines this attribute as a CFURL; downcast checks its type.
    // Converting its URL to a path does not access or canonicalize the source file.
    let url = unsafe { font.attribute(kCTFontURLAttribute) }?
        .downcast::<CFURL>()
        .ok()?;
    if !url.scheme()?.to_string().eq_ignore_ascii_case("file")
        || url.host_name().is_some_and(|host| {
            let host = host.to_string();
            !host.is_empty() && !host.eq_ignore_ascii_case("localhost")
        })
    {
        return None;
    }
    url.file_system_path(CFURLPathStyle::CFURLPOSIXPathStyle)
        .map(|path| PathBuf::from(path.to_string()))
        .filter(|path| path.is_absolute())
}

fn sfnt_name(names: ttf_parser::name::Table<'_>, id: u16) -> Option<String> {
    names
        .names
        .into_iter()
        .filter(|record| record.name_id == id)
        .find_map(|record| {
            let name = record.to_string().or_else(|| {
                if record.platform_id == ttf_parser::PlatformId::Macintosh
                    && record.encoding_id == 0
                    && record.name.is_ascii()
                {
                    Some(String::from_utf8_lossy(record.name).into_owned())
                } else {
                    None
                }
            });
            name.filter(|name| !name.is_empty() && !is_last_resort(name))
        })
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
    let control = Some(control);
    checkpoint(control)?;
    budget.check(12)?;
    let tags = table_tags(font, control)?
        .ok_or_else(|| source_error(path, "selected Core Text face exposes no tables"))?;
    if tags.contains(&HVGL) {
        return Err(source_error(path, "selected Core Text face exposes hvgl"));
    }
    let count = tags.iter().filter(|&&tag| tag != DSIG).count();
    // The serialized SFNT search fields cannot cover a directory larger than u16.
    check_limit("font-tables", count, usize::from(u16::MAX) / 16)?;
    let mut total = 12 + count * 16;
    budget.check(total as u64)?;
    let mut tables = Vec::with_capacity(count);
    for tag in tags.into_iter().filter(|&tag| tag != DSIG) {
        let table = read_table(font, tag, control)?
            .ok_or_else(|| source_error(path, "Core Text omitted an advertised font table"))?;
        let aligned = table.len().checked_add(3).map(|length| length & !3);
        total = aligned
            .and_then(|length| total.checked_add(length))
            .ok_or_else(|| source_error(path, "selected font table sizes overflow"))?;
        budget.check(total as u64)?;
        tables.push((tag, table));
    }
    checkpoint(control)?;
    Ok(tables)
}

fn checkpoint(control: Option<&OperationControl>) -> Result<(), NativeFontError> {
    if let Some(control) = control {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
    }
    Ok(())
}

fn table_tags(
    font: &CTFont,
    control: Option<&OperationControl>,
) -> Result<Option<Vec<u32>>, NativeFontError> {
    checkpoint(control)?;
    // SAFETY: The live font returns an owned array of unboxed tags.
    let tags = unsafe { font.available_tables(CTFontTableOptions::NoOptions) };
    checkpoint(control)?;
    let Some(tags) = tags else {
        return Ok(None);
    };
    check_limit("font-tables", tags.count() as usize, usize::from(u16::MAX))?;
    Ok((0..tags.count())
        .map(|index| {
            // SAFETY: The index is in bounds. The integer slot is never dereferenced.
            u32::try_from(unsafe { tags.value_at_index(index) } as usize).ok()
        })
        .collect())
}

fn read_table(
    font: &CTFont,
    tag: u32,
    control: Option<&OperationControl>,
) -> Result<Option<CFRetained<CFData>>, NativeFontError> {
    checkpoint(control)?;
    // SAFETY: The font is live; Core Text returns immutable retained table data.
    let table = unsafe { font.table(tag, CTFontTableOptions::NoOptions) };
    checkpoint(control)?;
    Ok(table)
}

fn read_tables<const N: usize>(
    font: &CTFont,
    tags: [&[u8; 4]; N],
    control: Option<&OperationControl>,
) -> Result<[Option<CFRetained<CFData>>; N], NativeFontError> {
    let mut tables = [const { None }; N];
    for (slot, tag) in tables.iter_mut().zip(tags) {
        *slot = read_table(font, u32::from_be_bytes(*tag), control)?;
    }
    Ok(tables)
}

fn table_bytes(table: &CFData) -> &[u8] {
    // SAFETY: This adapter only reads immutable data returned by Core Text.
    unsafe { table.as_bytes_unchecked() }
}

/// Compatibility lookup for an explicit/non-native base. Native admissions must
/// use `NativeFace::fallback` to preserve their retained origin, including name collisions.
#[cfg(test)]
pub(super) fn fallback(character: char, base: &FaceInfo) -> Option<NativeFace> {
    let base_font = registered_base_font(base)?;
    let mut text = [0; 4];
    let text = character.encode_utf8(&mut text);
    let font = fallback_from_font(&base_font, text, None);
    NativeFace::from_font(font, None, None)
        .ok()
        .flatten()
        .filter(|face| face.covers_cluster(GlyphFallback::from(&*text)))
}

fn fallback_from_font(
    base: &CTFont,
    text: &str,
    language: Option<&CFString>,
) -> CFRetained<CTFont> {
    let text = CFString::from_str(text);
    let range = CFRange {
        location: 0,
        length: text.length(),
    };
    // SAFETY: The retained base and string are live and the range covers the complete
    // CFString in UTF-16 code units, including both halves of a surrogate pair.
    unsafe { CTFont::for_string_with_language(base, &text, range, language) }
}

fn is_last_resort(name: &str) -> bool {
    let name = name.trim_start_matches('.');
    name.eq_ignore_ascii_case("LastResort") || name.eq_ignore_ascii_case("Last Resort")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use usvg::fontdb::{Database, Family, Query};

    #[test]
    #[ignore = "requires macOS Helvetica and Apple Color Emoji"]
    fn cluster_preflight_preserves_canonical_forms_and_emoji_substitutions() {
        let helvetica = named("Helvetica", Weight::NORMAL, Stretch::Normal, Style::Normal).unwrap();
        for text in ["Å", "A\u{30a}"] {
            assert!(helvetica.covers_cluster(text.into()), "{text:?}");
        }
        let emoji = named(
            "Apple Color Emoji",
            Weight::NORMAL,
            Stretch::Normal,
            Style::Normal,
        )
        .unwrap();
        for text in ["♥\u{fe0f}", "👩\u{200d}💻"] {
            assert!(emoji.covers_cluster(text.into()), "{text:?}");
        }
        assert!(!emoji.covers_cluster("f\u{301}".into()));
        assert!(!helvetica.covers_cluster("ℝ\u{20d7}".into()));
    }

    #[test]
    #[ignore = "requires a registered macOS Arial face"]
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
        let arial = named("Arial", Weight::NORMAL, Stretch::Normal, Style::Normal)
            .expect("registered Arial face");
        assert!(arial.covers_cluster("A".into()));
        assert!(!arial.covers_cluster("\u{0378}".into()));
        let pingfang = named(
            "PingFang SC",
            Weight::NORMAL,
            Stretch::Normal,
            Style::Normal,
        );
        let cases = [(&arial, 'A'), (&arial, '\u{0378}'), (&arial, '\u{a789}')]
            .into_iter()
            .chain(pingfang.as_ref().map(|face| (face, '\u{2da5a}')));
        for (face, character) in cases {
            let family = &face.family;
            // Read the installed revision's mapping without acquiring a snapshot.
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
            let mut text = [0; 4];
            let text: &str = character.encode_utf8(&mut text);
            assert_eq!(
                face.covers_cluster(text.into()),
                mapped,
                "{family} {character:?}"
            );
            let result = registry.materialize(
                FontCandidate::Native(face.clone()),
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

        let font = fallback_from_font(&first.font, "✓", None);
        let fallback = NativeFace::from_font(font, None, None)
            .unwrap()
            .expect("Arial fallback for check mark");
        assert!(fallback.covers_cluster("✓".into()));
    }

    #[test]
    #[ignore = "requires a registered macOS Arial face"]
    fn table_only_face_can_be_admitted_without_a_file_url() {
        use crate::fonts::{
            NativeFontLimits, registry::FontSourceRegistry, resolver::FontCandidate,
        };
        use std::sync::Arc;

        let mut candidate = named("Arial", Weight::NORMAL, Stretch::Normal, Style::Normal).unwrap();
        candidate.path = None;
        let registry = FontSourceRegistry::new(Default::default(), Default::default(), 0, 0);
        let mut database = Arc::new(Database::new());
        let control = OperationControl::new();
        let id = registry
            .materialize(
                FontCandidate::Native(candidate.clone()),
                Some('A'),
                &mut database,
                &NativeFontLimits::default(),
                &control,
            )
            .unwrap()
            .unwrap();
        assert!(
            database
                .with_face_data(id, |data, index| candidate
                    .matches_data(data, index, &control))
                .unwrap()
                .unwrap()
        );
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
        assert!(face.covers_cluster("\u{2713}".into()));
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
        let small = NativeFace::from_font(small, None, None).unwrap().unwrap();
        let large = NativeFace::from_font(large, None, None).unwrap().unwrap();
        assert_eq!(small.post_script_name, "ArialMT");
        assert_eq!(small.path, large.path);
        assert!(CascadeAttempt::from(&small) == CascadeAttempt::from(&large));

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

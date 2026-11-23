//! Native Fontconfig matching and borrowed metadata for selected file faces.

use std::ffi::{CStr, CString, OsStr, c_char, c_double, c_int, c_void};
use std::io::ErrorKind;
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::ptr::{self, NonNull};

use fontconfig_sys::constants::*;
use fontconfig_sys::statics::LIB_RESULT;
use fontconfig_sys::{
    Fc, FcCharSet, FcConfig, FcFontSet, FcMatchPattern, FcPattern, FcResult,
    FcResultMatch as FC_RESULT_MATCH, FcResultNoMatch as FC_RESULT_NO_MATCH,
};

use super::*;

// These functions and the FcValue union are absent from yeslogic-fontconfig-sys 6.0.1.
// Match fontconfig.h even though the only output we read is the binding.
#[repr(C)]
struct FcValue {
    kind: c_int,
    value: FcValueData,
}

#[repr(C)]
union FcValueData {
    pointer: *const c_void,
    number: c_double,
    integer: c_int,
}

struct Api {
    fc: &'static Fc,
    get_with_binding: unsafe extern "C" fn(
        *const FcPattern,
        *const c_char,
        c_int,
        *mut FcValue,
        *mut c_int,
    ) -> FcResult,
    get_sysroot: unsafe extern "C" fn(*const FcConfig) -> *const u8,
    weight_from_opentype: unsafe extern "C" fn(c_double) -> c_double,
    _library: libloading::Library,
}

impl Api {
    fn get() -> Option<&'static Self> {
        static API: OnceLock<Option<Api>> = OnceLock::new();
        API.get_or_init(|| {
            let fc = LIB_RESULT.as_ref().ok()?;
            // SAFETY: Use the system soname and signatures from fontconfig.h. Retain
            // this additional handle along with the function pointers it supplies.
            unsafe {
                let library = libloading::Library::new("libfontconfig.so.1").ok()?;
                Some(Self {
                    fc,
                    get_with_binding: *library.get(b"FcPatternGetWithBinding\0").ok()?,
                    get_sysroot: *library.get(b"FcConfigGetSysRoot\0").ok()?,
                    weight_from_opentype: *library.get(b"FcWeightFromOpenTypeDouble\0").ok()?,
                    _library: library,
                })
            }
        })
        .as_ref()
    }
}

pub(super) fn library_available() -> bool {
    Api::get().is_some()
}

pub(super) struct Catalog {
    api: &'static Api,
    config: NonNull<FcConfig>,
}

// SAFETY: This owns a reference to an initialized config. Matching uses immutable
// configured font sets and only mutates private queries. Fontconfig matching
// and config reference counting are thread-safe on the supported ABI.
unsafe impl Send for Catalog {}
// SAFETY: No config mutation or borrowed pointer escapes this adapter.
unsafe impl Sync for Catalog {}

impl Drop for Catalog {
    fn drop(&mut self) {
        // SAFETY: FcInitLoadConfigAndFonts returned this owned reference.
        unsafe { (self.api.fc.FcConfigDestroy)(self.config.as_ptr()) };
    }
}

pub(super) struct Query<'a> {
    pub(super) families: &'a [&'a str],
    pub(super) weight: Weight,
    pub(super) stretch: Stretch,
    pub(super) style: Style,
    pub(super) text: Option<&'a str>,
    pub(super) language: Option<&'a str>,
}

impl Query<'_> {
    fn explicit_faces<'db>(
        &self,
        database: &'db Database,
        preferred: &[(&str, bool)],
    ) -> Vec<(usize, &'db FaceInfo)> {
        let mut faces = Vec::new();
        if database.faces().next().is_none() {
            return faces;
        }
        let mut next_strong = usize::MAX;
        for name in self.families.iter().rev() {
            if let Some(rank) = preferred
                .iter()
                .position(|(family, strong)| *strong && eq_family(family, name))
            {
                next_strong = rank;
            }
            if let Some(face) = crate::query_named_font_family_case_insensitively(
                database,
                name,
                self.weight,
                self.stretch,
                self.style,
            )
            .and_then(|id| database.face(id))
            {
                faces.push((next_strong, face));
            }
        }
        faces.reverse();
        faces.sort_by_key(|(rank, _)| *rank);
        faces
    }
}

impl Catalog {
    pub(super) fn new() -> Result<Self, NativeFontError> {
        let api = Api::get().ok_or(NativeFontError::PlatformDiscoveryUnavailable {
            service: "Fontconfig",
        })?;
        // SAFETY: This initializes a separate configuration without changing the current one.
        let config = NonNull::new(unsafe { (api.fc.FcInitLoadConfigAndFonts)() })
            .ok_or_else(|| failed("initializing the catalog"))?;
        Ok(Self { api, config })
    }

    pub(super) fn default_family(
        &self,
        control: &OperationControl,
    ) -> Result<Option<String>, NativeFontError> {
        let pattern = self.pattern(&Query {
            families: &["sans-serif"],
            weight: Weight::NORMAL,
            stretch: Stretch::Normal,
            style: Style::Normal,
            text: None,
            language: None,
        })?;
        let mut status = FC_RESULT_NO_MATCH;
        // SAFETY: The config, substituted request and result storage are live.
        let raw = unsafe {
            (self.api.fc.FcFontMatch)(self.config.as_ptr(), pattern.as_ptr(), &mut status)
        };
        let matched = Pattern::from_raw(self.api.fc, raw);
        control.terminal_checkpoint_at(OperationPhase::Admission)?;
        match status {
            FC_RESULT_MATCH => Ok(matched
                .as_ref()
                .and_then(|p| p.get_string(FC_FAMILY, 0))
                .and_then(|s| s.to_str().ok())
                .map(str::to_owned)),
            FC_RESULT_NO_MATCH => Ok(None),
            _ => Err(failed("identifying the default family")),
        }
    }

    pub(super) fn select(
        &self,
        font: &usvg::Font,
        explicit: &Database,
        fallback: Option<GlyphFallback<'_>>,
        control: &OperationControl,
        accept: &mut AcceptFont<'_>,
    ) -> Result<Option<ID>, NativeFontError> {
        let weight = Weight(font.weight());
        let stretch = font.stretch().into();
        let style = font.style().into();
        let language = fallback
            .and_then(|request| request.language)
            .or_else(|| font.selection_language());
        let mut first_missing = None;
        let mut accept = |selection, implicit| {
            accept(selection, implicit)
                .or_else(|error| defer_missing_source(error, &mut first_missing))
        };
        let query =
            |names: &[&str], family_fallback_used, named_sources, accept: &mut AcceptFont<'_>| {
                self.candidates(
                    Query {
                        families: names,
                        weight,
                        stretch,
                        style,
                        text: fallback.map(|request| request.text),
                        language,
                    },
                    control,
                    named_sources,
                    |candidate, exact_name| {
                        accept(
                            FontSelection {
                                candidate,
                                family_fallback_used,
                            },
                            !exact_name,
                        )
                    },
                )
            };
        let mut named = Vec::new();
        for (index, family) in font
            .families()
            .iter()
            .chain(fallback.is_none().then_some(&usvg::FontFamily::Serif))
            .enumerate()
        {
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            let (name, generic) = match family {
                usvg::FontFamily::Named(name) if !name.is_empty() => (name.as_str(), false),
                usvg::FontFamily::Named(_) if explicit.faces().next().is_some() => continue,
                usvg::FontFamily::Named(_) | usvg::FontFamily::SansSerif => ("sans-serif", true),
                usvg::FontFamily::Serif => ("serif", true),
                usvg::FontFamily::Monospace => ("monospace", true),
                usvg::FontFamily::Cursive => ("cursive", true),
                usvg::FontFamily::Fantasy => ("fantasy", true),
            };
            let family_fallback_used = index == font.families().len();
            if !generic {
                named.push(name);
                continue;
            }
            if !named.is_empty() {
                if let Some(id) = query(&named, false, Some(explicit), &mut accept)? {
                    return Ok(Some(id));
                }
                named.clear();
            }
            if let Some(face) = explicit_face(explicit, family, weight, stretch, style)
                && let Some(id) = accept(
                    FontSelection {
                        candidate: FontCandidate::Loaded(face),
                        family_fallback_used,
                    },
                    true,
                )?
            {
                return Ok(Some(id));
            }
            if let Some(id) = query(&[name], family_fallback_used, None, &mut accept)? {
                return Ok(Some(id));
            }
        }
        if !named.is_empty()
            && let Some(id) = query(&named, false, Some(explicit), &mut accept)?
        {
            return Ok(Some(id));
        }
        // Glyph fallback continues with explicit sources and the general platform cascade.
        if fallback.is_none()
            && let Some(error) = first_missing
        {
            return Err(error);
        }
        Ok(None)
    }

    pub(super) fn candidates(
        &self,
        query: Query<'_>,
        control: &OperationControl,
        named_sources: Option<&Database>,
        mut accept: impl FnMut(FontCandidate, bool) -> Result<Option<ID>, NativeFontError>,
    ) -> Result<Option<ID>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let pattern = self.pattern(&query)?;
        let (preferred, callers) = match named_sources {
            Some(database) => {
                let preferred = pattern.preferred_families(self.api);
                let callers = query.explicit_faces(database, &preferred);
                (Some(preferred), callers)
            }
            None => (None, Vec::new()),
        };
        let mut callers = callers.into_iter().peekable();
        let mut status = FC_RESULT_NO_MATCH;
        // SAFETY: The configuration and pattern are live. Keep all coverage alternatives,
        // since an earlier font can be unsuitable even with identical advertised coverage.
        let raw = unsafe {
            (self.api.fc.FcFontSort)(
                self.config.as_ptr(),
                pattern.as_ptr(),
                0,
                ptr::null_mut(),
                &mut status,
            )
        };
        let sorted = NonNull::new(raw).map(|raw| FontSet {
            raw,
            fc: self.api.fc,
        });
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let sorted = match status {
            FC_RESULT_MATCH => Some(sorted.ok_or_else(|| failed("sorting candidates"))?),
            FC_RESULT_NO_MATCH => None,
            _ => return Err(failed("sorting candidates")),
        };
        let scalar = query
            .text
            .and_then(super::super::resolver::single_character);
        for face in sorted.iter().flat_map(FontSet::iter) {
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            // Insert caller faces at their strong family positions without breaking the
            // native ordering of system alternatives. A weak alias follows every strong name.
            if let Some(names) = &preferred {
                let Some(rank) = face.family_rank(names) else {
                    continue;
                };
                while let Some((_, caller)) = callers.next_if(|(position, _)| *position <= rank) {
                    if let Some(id) = accept(FontCandidate::Loaded(caller.clone()), true)? {
                        return Ok(Some(id));
                    }
                }
            }
            if scalar.is_some_and(|ch| face.covers(ch) == Some(false)) {
                continue;
            }
            let Some(candidate) = self.candidate(face) else {
                continue;
            };
            let exact_name = preferred.is_some()
                && query
                    .families
                    .iter()
                    .any(|wanted| face.families().any(|name| eq_family(name, wanted)));
            if let Some(id) = accept(candidate, exact_name)? {
                return Ok(Some(id));
            }
        }
        for (_, face) in callers {
            if let Some(id) = accept(FontCandidate::Loaded(face.clone()), true)? {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    fn pattern(&self, query: &Query<'_>) -> Result<Pattern<'static>, NativeFontError> {
        let fc = self.api.fc;
        // SAFETY: A nonnull result is a new pattern owned by this method.
        let pattern = Pattern::from_raw(fc, unsafe { (fc.FcPatternCreate)() })
            .ok_or_else(|| failed("allocating a query"))?;
        for family in query.families {
            pattern.add_string(FC_FAMILY, family)?;
        }
        if let Some(language) = query.language {
            pattern.add_string(FC_LANG, language)?;
            if let Some(orthography) = chinese_orthography(language) {
                pattern.add_string(FC_LANG, &orthography)?;
            }
        }
        let stretch = match query.stretch {
            Stretch::UltraCondensed => FC_WIDTH_ULTRACONDENSED,
            Stretch::ExtraCondensed => FC_WIDTH_EXTRACONDENSED,
            Stretch::Condensed => FC_WIDTH_CONDENSED,
            Stretch::SemiCondensed => FC_WIDTH_SEMICONDENSED,
            Stretch::Normal => FC_WIDTH_NORMAL,
            Stretch::SemiExpanded => FC_WIDTH_SEMIEXPANDED,
            Stretch::Expanded => FC_WIDTH_EXPANDED,
            Stretch::ExtraExpanded => FC_WIDTH_EXTRAEXPANDED,
            Stretch::UltraExpanded => FC_WIDTH_ULTRAEXPANDED,
        };
        let style = match query.style {
            Style::Normal => FC_SLANT_ROMAN,
            Style::Italic => FC_SLANT_ITALIC,
            Style::Oblique => FC_SLANT_OBLIQUE,
        };
        // SAFETY: All objects are owned/live, and the added numbers are copied by value.
        let added = unsafe {
            let weight = (self.api.weight_from_opentype)(f64::from(query.weight.0));
            (fc.FcPatternAddDouble)(pattern.as_ptr(), FC_WEIGHT.as_ptr(), weight) != 0
                && (fc.FcPatternAddInteger)(pattern.as_ptr(), FC_WIDTH.as_ptr(), stretch) != 0
                && (fc.FcPatternAddInteger)(pattern.as_ptr(), FC_SLANT.as_ptr(), style) != 0
        };
        if !added {
            return Err(failed("adding query properties"));
        }
        if let Some(text) = query.text {
            // SAFETY: Fontconfig allocates the set and retains a copy when adding it.
            let chars = NonNull::new(unsafe { (fc.FcCharSetCreate)() })
                .ok_or_else(|| failed("allocating a character set"))?;
            let added = unsafe {
                let added = text.chars().all(|character| {
                    (fc.FcCharSetAddChar)(chars.as_ptr(), u32::from(character)) != 0
                }) && (fc.FcPatternAddCharSet)(
                    pattern.as_ptr(),
                    FC_CHARSET.as_ptr(),
                    chars.as_ptr(),
                ) != 0;
                (fc.FcCharSetDestroy)(chars.as_ptr());
                added
            };
            if !added {
                return Err(failed("adding query character coverage"));
            }
        }
        // SAFETY: Only the private request is modified, with the initialized config live.
        unsafe {
            if (fc.FcConfigSubstitute)(self.config.as_ptr(), pattern.as_ptr(), FcMatchPattern) == 0
            {
                return Err(failed("applying query substitutions"));
            }
            (fc.FcDefaultSubstitute)(pattern.as_ptr());
        }
        Ok(pattern)
    }

    fn candidate(&self, face: PatternRef<'_>) -> Option<FontCandidate> {
        // The snapshot parser accepts SFNT faces and collections. FreeType also catalogs
        // other formats, including BDF and PCF, which cannot enter that parser.
        if face
            .get_string(FC_FONTFORMAT, 0)
            .is_some_and(|format| !matches!(format.to_bytes(), b"TrueType" | b"CFF"))
        {
            return None;
        }
        let filename = face.get_string(FC_FILE, 0)?;
        let mut families = face.families();
        let family = families.next()?;
        let index = u32::try_from(face.get_integer(FC_INDEX)?).ok()?;
        // The upper 16 bits encode a named instance. A physical face ID
        // cannot express this selection's effective variation settings.
        if index >> 16 != 0 {
            return None;
        }
        let path = Path::new(OsStr::from_bytes(filename.to_bytes()));
        // SAFETY: A nonnull sysroot is a C string borrowed from this live config.
        let root = unsafe { (self.api.get_sysroot)(self.config.as_ptr()) };
        let path = if root.is_null() {
            path.to_owned()
        } else {
            let root = unsafe { CStr::from_ptr(root.cast()) };
            Path::new(OsStr::from_bytes(root.to_bytes()))
                .join(path.strip_prefix("/").unwrap_or(path))
        };
        Some(
            PlatformFace {
                index,
                family: family.to_owned(),
                fontconfig_other_families: families.map(str::to_owned).collect(),
                post_script_name: face
                    .get_string(FC_POSTSCRIPT_NAME, 0)
                    .and_then(|name| name.to_str().ok())
                    .map(str::to_owned),
                // Config rules can edit style metadata. Validate the physical name and
                // index rather than comparing substituted style values.
                attributes: FaceAttributes::default(),
                coverage_source: None,
            }
            .into_file(path),
        )
    }
}

// Fontconfig's Chinese orthographies are keyed by territory. Preserve the original
// BCP 47 tag for config rules and also provide the script's orthography to matching.
fn chinese_orthography(language: &str) -> Option<String> {
    let mut parts = language.split('-');
    if !parts.next()?.eq_ignore_ascii_case("zh") {
        return None;
    }
    let script = parts.next()?;
    let default = if script.eq_ignore_ascii_case("Hans") {
        "cn"
    } else if script.eq_ignore_ascii_case("Hant") {
        "tw"
    } else {
        return None;
    };
    let region = parts
        .next()
        .filter(|r| r.len() == 2 && r.bytes().all(|b| b.is_ascii_alphabetic()));
    Some(format!(
        "zh-{}",
        region.unwrap_or(default).to_ascii_lowercase()
    ))
}

fn eq_family(a: &str, b: &str) -> bool {
    // Fontconfig compares family names case-insensitively and ignores blanks.
    unicase::eq(a, b) || unicase::eq(&a.replace(' ', ""), &b.replace(' ', ""))
}

fn failed(operation: &str) -> NativeFontError {
    NativeFontError::Measurement(format!("Fontconfig failed while {operation}"))
}

struct Pattern<'a>(PatternRef<'a>);

impl<'a> Pattern<'a> {
    fn from_raw(fc: &'a Fc, raw: *mut FcPattern) -> Option<Self> {
        Some(Self(PatternRef {
            fc,
            raw: NonNull::new(raw)?,
        }))
    }

    fn as_ptr(&self) -> *mut FcPattern {
        self.0.raw.as_ptr()
    }

    fn get_string(&self, key: &CStr, index: c_int) -> Option<&CStr> {
        self.0.get_string(key, index)
    }

    fn add_string(&self, key: &CStr, value: &str) -> Result<(), NativeFontError> {
        let value = CString::new(value).map_err(|_| failed("encoding a query string"))?;
        // SAFETY: The pattern and NUL-terminated strings are live. Fontconfig copies the value.
        if unsafe {
            (self.0.fc.FcPatternAddString)(self.as_ptr(), key.as_ptr(), value.as_ptr().cast())
        } == 0
        {
            return Err(failed("adding a query string"));
        }
        Ok(())
    }

    fn preferred_families(&self, api: &Api) -> Vec<(&str, bool)> {
        let mut names = Vec::new();
        let mut last_strong = None;
        for index in 0.. {
            let mut value = MaybeUninit::<FcValue>::uninit();
            let mut binding = 0;
            // SAFETY: The query is live, and both out parameters have the C ABI layout.
            let status = unsafe {
                (api.get_with_binding)(
                    self.as_ptr(),
                    FC_FAMILY.as_ptr(),
                    index,
                    value.as_mut_ptr(),
                    &mut binding,
                )
            };
            if status != FC_RESULT_MATCH {
                break;
            }
            if let Some(name) = self
                .get_string(FC_FAMILY, index)
                .and_then(|s| s.to_str().ok())
            {
                names.push((name, binding != 0));
            }
            // FcValueBindingWeak is zero. Keep weak prefer entries before the last
            // strong/same value as Skia does. Trailing defaults cannot shadow
            // the next authored family.
            if binding != 0 {
                last_strong = Some(names.len());
            }
        }
        if let Some(length) = last_strong {
            names.truncate(length);
        }
        names
    }
}

impl Drop for Pattern<'_> {
    fn drop(&mut self) {
        // SAFETY: This wrapper exclusively owns the query or FcFontMatch result.
        unsafe { (self.0.fc.FcPatternDestroy)(self.as_ptr()) };
    }
}

#[derive(Clone, Copy)]
struct PatternRef<'a> {
    fc: &'a Fc,
    raw: NonNull<FcPattern>,
}

impl<'a> PatternRef<'a> {
    fn get_string(self, key: &CStr, index: c_int) -> Option<&'a CStr> {
        let mut value = ptr::null_mut();
        // SAFETY: The pattern stays live and unmodified for the borrow of its owner.
        let status = unsafe {
            (self.fc.FcPatternGetString)(self.raw.as_ptr(), key.as_ptr(), index, &mut value)
        };
        (status == FC_RESULT_MATCH && !value.is_null())
            .then(|| unsafe { CStr::from_ptr(value.cast()) })
    }

    fn families(self) -> impl Iterator<Item = &'a str> {
        (0..)
            .map_while(move |index| self.get_string(FC_FAMILY, index))
            .filter_map(|value| value.to_str().ok())
    }

    fn family_rank(self, preferred: &[(&str, bool)]) -> Option<usize> {
        let mut weak = None;
        for (rank, &(name, strong)) in preferred.iter().enumerate() {
            if self.families().any(|family| eq_family(family, name)) {
                if strong {
                    return Some(rank);
                }
                weak = Some(usize::MAX);
            }
        }
        weak
    }

    fn get_integer(self, key: &CStr) -> Option<c_int> {
        let mut value = 0;
        // SAFETY: The pattern and output are live.
        let status = unsafe {
            (self.fc.FcPatternGetInteger)(self.raw.as_ptr(), key.as_ptr(), 0, &mut value)
        };
        (status == FC_RESULT_MATCH).then_some(value)
    }

    fn covers(self, character: char) -> Option<bool> {
        let mut chars: *mut FcCharSet = ptr::null_mut();
        // SAFETY: A returned charset is borrowed from the live immutable pattern.
        unsafe {
            let status = (self.fc.FcPatternGetCharSet)(
                self.raw.as_ptr(),
                FC_CHARSET.as_ptr(),
                0,
                &mut chars,
            );
            (status == FC_RESULT_MATCH && !chars.is_null())
                .then(|| (self.fc.FcCharSetHasChar)(chars, u32::from(character)) != 0)
        }
    }
}

struct FontSet {
    fc: &'static Fc,
    raw: NonNull<FcFontSet>,
}

impl FontSet {
    fn iter(&self) -> impl Iterator<Item = PatternRef<'_>> {
        // SAFETY: FcFontSort owns an array with nfont live pattern pointers when nonempty.
        let set = unsafe { self.raw.as_ref() };
        let pointers = if set.nfont > 0 && !set.fonts.is_null() {
            unsafe { std::slice::from_raw_parts(set.fonts, set.nfont as usize) }
        } else {
            &[]
        };
        pointers.iter().filter_map(|&p| {
            Some(PatternRef {
                fc: self.fc,
                raw: NonNull::new(p)?,
            })
        })
    }
}

impl Drop for FontSet {
    fn drop(&mut self) {
        // SAFETY: FcFontSort returned this owned font set.
        unsafe { (self.fc.FcFontSetSortDestroy)(self.raw.as_ptr()) };
    }
}

/// Try the rest of the catalog after a file disappears. Admission runs first so retained
/// snapshots win. Symlinks in the file path keep their source errors.
pub(super) fn defer_missing_source<T>(
    error: NativeFontError,
    first_missing: &mut Option<NativeFontError>,
) -> Result<Option<T>, NativeFontError> {
    if let NativeFontError::Source { path, .. } = &error
        && source_disappeared(path)
    {
        first_missing.get_or_insert(error);
        Ok(None)
    } else {
        Err(error)
    }
}

fn source_disappeared(path: &Path) -> bool {
    // A dangling ancestor also gives ENOENT at the leaf. Inspect the components
    // before treating the original source error as a stale catalog entry.
    let mut missing = false;
    for component in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
        match component.symlink_metadata() {
            Err(error) if error.kind() == ErrorKind::NotFound => missing = true,
            Ok(metadata) if missing && !metadata.is_symlink() => (),
            _ => return false,
        }
    }
    missing
}

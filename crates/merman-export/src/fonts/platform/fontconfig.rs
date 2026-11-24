//! Native Fontconfig matching and borrowed metadata for selected file faces.

use std::collections::HashMap;
use std::ffi::{CStr, CString, OsStr, c_int};
use std::io::ErrorKind;
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::ptr::{self, NonNull};

use super::*;

mod config;
mod ffi;

use config::Watch;
use ffi::*;

const FC_FAMILY: &CStr = c"family";
const FC_LANG: &CStr = c"lang";
const FC_WEIGHT: &CStr = c"weight";
const FC_WIDTH: &CStr = c"width";
const FC_SLANT: &CStr = c"slant";
const FC_CHARSET: &CStr = c"charset";
const FC_FILE: &CStr = c"file";
const FC_INDEX: &CStr = c"index";
const FC_POSTSCRIPT_NAME: &CStr = c"postscriptname";
const FC_FONTFORMAT: &CStr = c"fontformat";
const FC_FONT_VARIATIONS: &CStr = c"fontvariations";

pub(super) struct Catalog {
    api: &'static Api,
    config: NonNull<FcConfig>,
    sysroot: Option<PathBuf>,
    paths: Vec<Watch>,
}

// SAFETY: The catalog owns its config. All calls into older ABIs use the compatibility lock.
unsafe impl Send for Catalog {}
// SAFETY: Matches only mutate private patterns. Refresh serializes the rescan timestamp,
// and no native pointer escapes the catalog.
unsafe impl Sync for Catalog {}

pub(super) struct Query<'a> {
    pub(super) families: &'a [&'a str],
    pub(super) weight: Weight,
    pub(super) stretch: Stretch,
    pub(super) style: Style,
    pub(super) text: Option<&'a str>,
    pub(super) language: Option<&'a str>,
    pub(super) rtl: bool,
}

enum MatchScope<'a> {
    Named(&'a Database),
    SystemUi(&'a Database),
    Generic,
    Fallback,
}

#[derive(Clone, Copy)]
enum CoveragePass {
    Nominal,
    NonDefaultVariation,
    NominalOrVariation,
}

enum Coverage {
    Nominal,
    NonDefaultVariation(Vec<(char, char)>),
}

type InstanceBases<'a> = HashMap<(&'a CStr, u32), Option<PatternRef<'a>>>;

impl Query<'_> {
    fn explicit_face<'db>(&self, database: &'db Database, name: &str) -> Option<&'db FaceInfo> {
        let id = crate::query_named_font_family_case_insensitively(
            database,
            name,
            self.weight,
            self.stretch,
            self.style,
        )
        .or_else(|| {
            let canonical = database
                .faces()
                .flat_map(|face| &face.families)
                .map(|(family, _)| family)
                .find(|family| eq_family(family, name))?;
            crate::query_font_family(
                database,
                usvg::fontdb::Family::Name(canonical),
                self.weight,
                self.stretch,
                self.style,
            )
        })?;
        database.face(id)
    }

    fn explicit_faces<'db>(
        &self,
        database: &'db Database,
        preferred: &[(&str, bool)],
    ) -> Vec<(usize, &'db FaceInfo)> {
        let mut faces = Vec::new();
        if database.faces().next().is_none() {
            return faces;
        }
        let mut next_strong = preferred.len();
        for name in self.families.iter().rev() {
            if let Some(rank) = preferred
                .iter()
                .position(|(family, strong)| *strong && eq_family(family, name))
            {
                next_strong = rank;
            }
            if let Some(face) = self.explicit_face(database, name) {
                faces.push((next_strong, face));
            }
        }
        faces.reverse();
        faces
    }
}

impl Catalog {
    pub(super) fn default_family(
        &self,
        control: &OperationControl,
    ) -> Result<Option<String>, NativeFontError> {
        let query = Query {
            families: &["sans-serif"],
            weight: Weight::NORMAL,
            stretch: Stretch::Normal,
            style: Style::Normal,
            text: None,
            language: None,
            rtl: false,
        };
        control.terminal_checkpoint_at(OperationPhase::Admission)?;
        let matched = {
            let _lock = self.api.lock();
            let pattern = self.pattern(&query, false)?;
            let mut status = FC_RESULT_NO_MATCH;
            // SAFETY: The config, substituted request and result storage are live.
            let raw = unsafe {
                (self.api.FcFontMatch)(self.config.as_ptr(), pattern.as_ptr(), &mut status)
            };
            let matched = Pattern::from_raw(self.api, raw);
            control.terminal_checkpoint_at(OperationPhase::Admission)?;
            match (status, matched.as_ref()) {
                (FC_RESULT_MATCH, Some(face)) => {
                    self.file_face(face.as_ref(), &mut InstanceBases::new())
                }
                (FC_RESULT_NO_MATCH, None) => return Ok(None),
                _ => return Err(failed("identifying the default family")),
            }
        };
        if let Some((path, face)) = matched
            && !source_disappeared(&path)
        {
            return Ok(Some(face.family));
        }
        self.search(query, control, MatchScope::Generic, |candidate, _| {
            if let FontCandidate::File {
                path,
                expected: ExpectedFace::Platform(face),
                ..
            } = candidate
                && !source_disappeared(&path)
            {
                return Ok(Some(face.family));
            }
            Ok(None)
        })
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
        let query = |names: &[&str], family_fallback_used, scope, accept: &mut AcceptFont<'_>| {
            self.search(
                Query {
                    families: names,
                    weight,
                    stretch,
                    style,
                    text: fallback.map(|request| request.text),
                    language,
                    rtl: fallback.is_some_and(|request| request.rtl),
                },
                control,
                scope,
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
            use usvg::FontFamily;
            let system_ui = match family {
                FontFamily::Named(name) if super::is_system_ui(name) => Some(name.as_str()),
                _ => None,
            };
            let (name, generic) = match family {
                FontFamily::Named(name) if !name.is_empty() && system_ui.is_none() => {
                    named.push(name.as_str());
                    continue;
                }
                FontFamily::Named(_) | FontFamily::SansSerif => {
                    ("sans-serif", &FontFamily::SansSerif)
                }
                FontFamily::Serif => ("serif", family),
                FontFamily::Monospace => ("monospace", family),
                FontFamily::Cursive => ("cursive", family),
                FontFamily::Fantasy => ("fantasy", family),
            };
            if !named.is_empty() {
                if let Some(id) = query(&named, false, MatchScope::Named(explicit), &mut accept)? {
                    return Ok(Some(id));
                }
                named.clear();
            }
            let family_fallback_used = index == font.families().len();
            if let Some(alias) = system_ui {
                let names = [alias, "system-ui"];
                let count = if alias.eq_ignore_ascii_case("system-ui") {
                    1
                } else {
                    2
                };
                if let Some(id) = query(
                    &names[..count],
                    family_fallback_used,
                    MatchScope::SystemUi(explicit),
                    &mut accept,
                )? {
                    return Ok(Some(id));
                }
            }
            if let Some(face) = explicit_face(explicit, generic, weight, stretch, style)
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
            if let Some(id) = query(
                &[name],
                family_fallback_used,
                MatchScope::Generic,
                &mut accept,
            )? {
                return Ok(Some(id));
            }
        }
        if !named.is_empty()
            && let Some(id) = query(&named, false, MatchScope::Named(explicit), &mut accept)?
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
        accept: impl FnMut(FontCandidate, bool) -> Result<Option<ID>, NativeFontError>,
    ) -> Result<Option<ID>, NativeFontError> {
        let scope = match named_sources {
            Some(database) => MatchScope::Named(database),
            None if query.text.is_some() => MatchScope::Fallback,
            None => MatchScope::Generic,
        };
        self.search(query, control, scope, accept)
    }

    fn search<T>(
        &self,
        query: Query<'_>,
        control: &OperationControl,
        scope: MatchScope<'_>,
        mut accept: impl FnMut(FontCandidate, bool) -> Result<Option<T>, NativeFontError>,
    ) -> Result<Option<T>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let mut lock = self.api.lock();
        let pattern = self.pattern(&query, matches!(scope, MatchScope::Fallback))?;
        let preferred = matches!(scope, MatchScope::Named(_) | MatchScope::SystemUi(_))
            .then(|| pattern.preferred_families());
        let names = preferred.as_deref().unwrap_or_default();
        let mut callers = match scope {
            MatchScope::Named(database) => query.explicit_faces(database, names),
            MatchScope::SystemUi(database) => names
                .iter()
                .enumerate()
                .filter_map(|(rank, &(name, strong))| {
                    Some((
                        family_rank(names, rank, strong),
                        query.explicit_face(database, name)?,
                    ))
                })
                .collect(),
            _ => Vec::new(),
        };
        callers.sort_by_key(|(rank, _)| *rank);
        let mut callers = callers.into_iter().peekable();
        let caller_exact = matches!(scope, MatchScope::Named(_));
        // A caller at the first strong position precedes every possible system match.
        while let Some((_, caller)) = callers.next_if(|(rank, _)| *rank == 0) {
            if let Some(result) =
                lock.outside(|| accept(FontCandidate::Loaded(caller.clone()), caller_exact))?
            {
                return Ok(Some(result));
            }
        }

        // After substitution, sorting only affects the order of multiple eligible patterns.
        let mut matching = self.scan_patterns().filter(|face| {
            preferred
                .as_deref()
                .is_none_or(|names| face.family_rank(names).is_some())
        });
        let (single, sorted) = match (matching.next(), matching.next()) {
            (first, None) => (first, None),
            (_, Some(_)) => (None, Some(self.sort(&pattern, query.text)?)),
        };
        let has_variation = query.text.is_some_and(cluster::has_variation_sequence);
        // Keep the named cascade in family order. In a broad cascade a true
        // variation glyph takes precedence over nominal coverage or mirroring.
        let passes: &[_] = match (has_variation, preferred.is_some()) {
            (true, false) => &[CoveragePass::NonDefaultVariation, CoveragePass::Nominal],
            (true, true) => &[CoveragePass::NominalOrVariation],
            (false, _) => &[CoveragePass::Nominal],
        };
        let mut bases = InstanceBases::new();
        for &pass in passes {
            for face in single
                .into_iter()
                .chain(sorted.iter().flat_map(FontSet::iter))
            {
                control.terminal_checkpoint_at(OperationPhase::Layout)?;
                // Insert caller faces at their family positions while preserving the
                // native order of system alternatives.
                if let Some(names) = &preferred {
                    let Some(rank) = face.family_rank(names) else {
                        continue;
                    };
                    // A weak system-ui alias follows the system cascade, where
                    // Fontconfig can rank a language match ahead of that alias.
                    while let Some((_, caller)) = callers.next_if(|(position, _)| {
                        *position <= rank && (caller_exact || *position < names.len())
                    }) {
                        if let Some(id) = lock.outside(|| {
                            accept(FontCandidate::Loaded(caller.clone()), caller_exact)
                        })? {
                            return Ok(Some(id));
                        }
                    }
                }
                let Some(coverage) = face.coverage(query.text, query.rtl, pass) else {
                    continue;
                };
                let Some((path, mut physical)) = self.file_face(face, &mut bases) else {
                    continue;
                };
                // SAFETY: The substituted query and borrowed scan pattern are live. This
                // prepares a new pattern, applying render rules and resolving variable ranges.
                let raw = unsafe {
                    (self.api.FcFontRenderPrepare)(
                        self.config.as_ptr(),
                        pattern.as_ptr(),
                        face.raw.as_ptr(),
                    )
                };
                let prepared = Pattern::from_raw(self.api, raw)
                    .ok_or_else(|| failed("preparing a candidate"))?;
                physical.variations = prepared.as_ref().variations();
                physical.required_uvs = match coverage {
                    Coverage::Nominal => None,
                    Coverage::NonDefaultVariation(required) => Some(required),
                };
                drop(prepared);
                let exact_name = matches!(scope, MatchScope::Named(_))
                    && query
                        .families
                        .iter()
                        .any(|wanted| face.families().any(|name| eq_family(name, wanted)));
                let candidate = physical.into_file(path);
                if let Some(id) = lock.outside(|| accept(candidate, exact_name))? {
                    return Ok(Some(id));
                }
            }
        }
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        for (_, face) in callers {
            if let Some(id) =
                lock.outside(|| accept(FontCandidate::Loaded(face.clone()), caller_exact))?
            {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    fn sort(&self, pattern: &Pattern<'_>, text: Option<&str>) -> Result<FontSet, NativeFontError> {
        let scoring = pattern.for_scoring(text)?;
        let pattern = scoring.as_ref().unwrap_or(pattern);
        let mut status = FC_RESULT_NO_MATCH;
        // SAFETY: The configuration and pattern are live. Keep all alternatives;
        // candidates with identical advertised coverage can have different eligibility.
        let raw = unsafe {
            (self.api.FcFontSort)(
                self.config.as_ptr(),
                pattern.as_ptr(),
                0,
                ptr::null_mut(),
                &mut status,
            )
        };
        // Fontconfig 2.13+ returns an allocated empty set for zero matches. A
        // NULL pointer can also signal internal failure with status still NoMatch.
        let set = FontSet {
            api: self.api,
            raw: NonNull::new(raw).ok_or_else(|| failed("sorting candidates"))?,
        };
        match status {
            FC_RESULT_MATCH | FC_RESULT_NO_MATCH => Ok(set),
            _ => Err(failed("sorting candidates")),
        }
    }

    fn scan_patterns(&self) -> impl Iterator<Item = PatternRef<'_>> {
        [FC_SET_SYSTEM, FC_SET_APPLICATION]
            .into_iter()
            .filter_map(|kind| {
                // SAFETY: The config owns the immutable, possibly absent set. The
                // iterator runs with the Fontconfig compatibility lock held.
                unsafe { (self.api.FcConfigGetFonts)(self.config.as_ptr(), kind).as_ref() }
            })
            .flat_map(|set| font_patterns(self.api, set))
    }

    fn pattern(
        &self,
        query: &Query<'_>,
        weak_base: bool,
    ) -> Result<Pattern<'static>, NativeFontError> {
        let fc = self.api;
        // SAFETY: A nonnull result is a new pattern owned by this method.
        let pattern = Pattern::from_raw(fc, unsafe { (fc.FcPatternCreate)() })
            .ok_or_else(|| failed("allocating a query"))?;
        for family in query.families.iter().filter(|name| !name.is_empty()) {
            // Glyph fallback can prefer a covering sibling, while language retains priority.
            pattern.add_string(FC_FAMILY, family, weak_base)?;
        }
        if let Some(language) = query.language {
            pattern.add_string(FC_LANG, language, false)?;
            if let Some(orthography) = chinese_orthography(language) {
                pattern.add_string(FC_LANG, &orthography, false)?;
            }
        }
        let stretch = match query.stretch {
            Stretch::UltraCondensed => 50,
            Stretch::ExtraCondensed => 63,
            Stretch::Condensed => 75,
            Stretch::SemiCondensed => 87,
            Stretch::Normal => 100,
            Stretch::SemiExpanded => 113,
            Stretch::Expanded => 125,
            Stretch::ExtraExpanded => 150,
            Stretch::UltraExpanded => 200,
        };
        let style = match query.style {
            Style::Normal => 0,
            Style::Italic => 100,
            Style::Oblique => 110,
        };
        // SAFETY: All objects are owned/live, and the added numbers are copied by value.
        let added = unsafe {
            let weight = (fc.FcWeightFromOpenTypeDouble)(f64::from(query.weight.0));
            (fc.FcPatternAddDouble)(pattern.as_ptr(), FC_WEIGHT.as_ptr(), weight) != 0
                && (fc.FcPatternAddInteger)(pattern.as_ptr(), FC_WIDTH.as_ptr(), stretch) != 0
                && (fc.FcPatternAddInteger)(pattern.as_ptr(), FC_SLANT.as_ptr(), style) != 0
        };
        if !added {
            return Err(failed("adding query properties"));
        }
        if let Some(chars) = CharSet::from_chars(fc, query.text.into_iter().flat_map(str::chars))? {
            pattern.add_charset(&chars)?;
        }
        // SAFETY: Only the private request is modified, with the initialized config live.
        unsafe {
            if (fc.FcConfigSubstitute)(self.config.as_ptr(), pattern.as_ptr(), FC_MATCH_PATTERN)
                == 0
            {
                return Err(failed("applying query substitutions"));
            }
            (fc.FcDefaultSubstitute)(pattern.as_ptr());
        }
        Ok(pattern)
    }

    fn file_face<'a>(
        &'a self,
        face: PatternRef<'a>,
        bases: &mut InstanceBases<'a>,
    ) -> Option<(PathBuf, PlatformFace)> {
        // The snapshot parser accepts SFNT faces and collections. FreeType also catalogs
        // other formats, including BDF and PCF, which cannot enter that parser.
        if face
            .get_string(FC_FONTFORMAT, 0)
            .is_some_and(|format| !matches!(format.to_bytes(), b"TrueType" | b"CFF"))
        {
            return None;
        }
        let filename = face.get_string(FC_FILE, 0)?;
        let index = u32::try_from(face.get_integer(FC_INDEX)?).ok()?;
        let instance = (index >> 16) as u16;
        let named_instance = (instance != 0).then_some(instance);
        let index = index & 0xffff;
        let base = if named_instance.is_some() {
            *bases.entry((filename, index)).or_insert_with(|| {
                self.scan_patterns().find(|pattern| {
                    pattern.get_integer(FC_INDEX) == Some(index as c_int)
                        && pattern.get_string(FC_FILE, 0) == Some(filename)
                        && pattern.get_string(FC_POSTSCRIPT_NAME, 0).is_some()
                })
            })
        } else {
            Some(face)
        };
        // Named styles can have their own PostScript name. Use the physical scan
        // pattern's names when available, including any scan-rule aliases.
        let mut families = base.unwrap_or(face).families();
        let family = families.next()?;
        let path = self.resolve_path(Path::new(OsStr::from_bytes(filename.to_bytes())));
        Some((
            path,
            PlatformFace {
                index,
                family: family.to_owned(),
                other_families: families.map(str::to_owned).collect(),
                post_script_name: base
                    .and_then(|pattern| pattern.get_string(FC_POSTSCRIPT_NAME, 0))
                    .and_then(|name| name.to_str().ok())
                    .map(str::to_owned),
                variations: Vec::new(),
                fallback_scale: 1.0,
                named_instance,
                required_uvs: None,
            },
        ))
    }
}

fn family_rank(preferred: &[(&str, bool)], rank: usize, strong: bool) -> usize {
    if strong {
        rank
    } else {
        preferred.len() + 1 + rank
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
    // Fontconfig compares folded Unicode and ignores ASCII spaces. Most names
    // compare directly or can be compared with a byte iterator.
    if a.is_ascii() && b.is_ascii() {
        let fold = |byte: u8| (byte != b' ').then_some(byte.to_ascii_lowercase());
        return a.bytes().filter_map(fold).eq(b.bytes().filter_map(fold));
    }
    if unicase::eq(a, b) {
        return true;
    }
    if !a.contains(' ') && !b.contains(' ') {
        return false;
    }
    let (mut left, mut right) = ([0; 128], [0; 128]);
    unicase::eq(
        &without_blanks(a, &mut left),
        &without_blanks(b, &mut right),
    )
}

fn without_blanks<'a>(name: &'a str, buffer: &'a mut [u8]) -> std::borrow::Cow<'a, str> {
    use std::borrow::Cow;

    if !name.contains(' ') {
        return Cow::Borrowed(name);
    }
    if name.len() > buffer.len() {
        return Cow::Owned(name.replace(' ', ""));
    }
    let mut length = 0;
    for byte in name.bytes().filter(|&byte| byte != b' ') {
        buffer[length] = byte;
        length += 1;
    }
    // Removing ASCII spaces preserves UTF-8.
    Cow::Borrowed(std::str::from_utf8(&buffer[..length]).unwrap_or(name))
}

fn failed(operation: &str) -> NativeFontError {
    NativeFontError::Measurement(format!("Fontconfig failed while {operation}"))
}

struct Pattern<'a>(PatternRef<'a>);

impl<'a> Pattern<'a> {
    fn from_raw(api: &'a Api, raw: *mut FcPattern) -> Option<Self> {
        Some(Self(PatternRef {
            api,
            raw: NonNull::new(raw)?,
        }))
    }

    fn as_ptr(&self) -> *mut FcPattern {
        self.0.raw.as_ptr()
    }

    fn as_ref(&self) -> PatternRef<'_> {
        self.0
    }

    fn add_string(&self, key: &CStr, value: &str, weak: bool) -> Result<(), NativeFontError> {
        let value = CString::new(value).map_err(|_| failed("encoding a query string"))?;
        // SAFETY: The pattern and NUL-terminated strings are live. Fontconfig copies the value.
        let added = unsafe {
            if weak {
                (self.0.api.FcPatternAddWeak)(
                    self.as_ptr(),
                    key.as_ptr(),
                    FcValue::string(&value),
                    1,
                )
            } else {
                (self.0.api.FcPatternAddString)(self.as_ptr(), key.as_ptr(), value.as_ptr().cast())
            }
        };
        if added == 0 {
            return Err(failed("adding a query string"));
        }
        Ok(())
    }

    fn add_charset(&self, chars: &CharSet<'_>) -> Result<(), NativeFontError> {
        // SAFETY: The live pattern retains its own reference to the live charset.
        if unsafe {
            (self.0.api.FcPatternAddCharSet)(self.as_ptr(), FC_CHARSET.as_ptr(), chars.raw.as_ptr())
        } == 0
        {
            return Err(failed("adding query character coverage"));
        }
        Ok(())
    }

    fn for_scoring(&self, text: Option<&str>) -> Result<Option<Self>, NativeFontError> {
        let Some(text) = text else { return Ok(None) };
        let fc = self.0.api;
        let Some(omitted) = CharSet::from_chars(
            fc,
            text.chars().filter(|&ch| cluster::is_default_ignorable(ch)),
        )?
        else {
            return Ok(None);
        };
        if self.as_ref().charset(0).is_none() {
            return Ok(None);
        }
        // Keep the full substituted request for render rules. Remove ignored
        // characters only from the scoring copy, keeping other charset edits.
        // SAFETY: Only the new owned pattern is mutated. Its charsets are borrowed
        // from the original live pattern when making each scoring set.
        let scored = unsafe {
            let scored = Self::from_raw(fc, (fc.FcPatternDuplicate)(self.as_ptr()))
                .ok_or_else(|| failed("copying a query for scoring"))?;
            (fc.FcPatternDel)(scored.as_ptr(), FC_CHARSET.as_ptr());
            for chars in (0..).map_while(|index| self.as_ref().charset(index)) {
                let raw = (fc.FcCharSetSubtract)(chars.as_ptr(), omitted.raw.as_ptr());
                let chars = CharSet::from_raw(fc, raw)
                    .ok_or_else(|| failed("filtering query character coverage"))?;
                if (fc.FcCharSetCount)(chars.raw.as_ptr()) != 0 {
                    scored.add_charset(&chars)?;
                }
            }
            scored
        };
        Ok(Some(scored))
    }

    fn preferred_families(&self) -> Vec<(&str, bool)> {
        let mut names = Vec::new();
        let mut last_strong = None;
        for index in 0.. {
            let mut value = MaybeUninit::<FcValue>::uninit();
            let mut binding = FC_VALUE_BINDING_WEAK;
            // SAFETY: The query is live, and both out parameters have the C ABI layout.
            let status = unsafe {
                (self.0.api.FcPatternGetWithBinding)(
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
                .as_ref()
                .get_string(FC_FAMILY, index)
                .and_then(|s| s.to_str().ok())
            {
                names.push((name, binding != FC_VALUE_BINDING_WEAK));
            }
            // FcValueBindingWeak is zero. Keep weak prefer entries before the last
            // strong/same value as Skia does. Trailing defaults cannot shadow
            // the next authored family.
            if binding != FC_VALUE_BINDING_WEAK {
                last_strong = Some(names.len());
            }
        }
        if let Some(length) = last_strong {
            names.truncate(length);
        }
        names
    }
}

struct CharSet<'a> {
    api: &'a Api,
    raw: NonNull<FcCharSet>,
}

impl<'a> CharSet<'a> {
    fn from_raw(api: &'a Api, raw: *mut FcCharSet) -> Option<Self> {
        Some(Self {
            api,
            raw: NonNull::new(raw)?,
        })
    }

    fn from_chars(
        api: &'a Api,
        mut chars: impl Iterator<Item = char>,
    ) -> Result<Option<Self>, NativeFontError> {
        let Some(first) = chars.next() else {
            return Ok(None);
        };
        // SAFETY: A nonnull result is a new set owned by this method.
        let set = Self::from_raw(api, unsafe { (api.FcCharSetCreate)() })
            .ok_or_else(|| failed("allocating a character set"))?;
        // SAFETY: The locally owned set is mutable and each character is copied.
        if !std::iter::once(first)
            .chain(chars)
            .all(|ch| unsafe { (api.FcCharSetAddChar)(set.raw.as_ptr(), u32::from(ch)) != 0 })
        {
            return Err(failed("building query character coverage"));
        }
        Ok(Some(set))
    }
}

impl Drop for CharSet<'_> {
    fn drop(&mut self) {
        // SAFETY: This wrapper owns a reference to the set and is scoped to the lock.
        unsafe { (self.api.FcCharSetDestroy)(self.raw.as_ptr()) };
    }
}

impl Drop for Pattern<'_> {
    fn drop(&mut self) {
        // SAFETY: This wrapper owns the pattern and is scoped to a native call lock.
        unsafe { (self.0.api.FcPatternDestroy)(self.as_ptr()) };
    }
}

#[derive(Clone, Copy)]
struct PatternRef<'a> {
    api: &'a Api,
    raw: NonNull<FcPattern>,
}

impl<'a> PatternRef<'a> {
    fn get_string(self, key: &CStr, index: c_int) -> Option<&'a CStr> {
        let mut value = ptr::null_mut();
        // SAFETY: The pattern stays live and unmodified for the borrow of its owner.
        let status = unsafe {
            (self.api.FcPatternGetString)(self.raw.as_ptr(), key.as_ptr(), index, &mut value)
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
                weak.get_or_insert_with(|| family_rank(preferred, rank, false));
            }
        }
        weak
    }

    fn get_integer(self, key: &CStr) -> Option<c_int> {
        let mut value = 0;
        // SAFETY: The pattern and output are live.
        let status = unsafe {
            (self.api.FcPatternGetInteger)(self.raw.as_ptr(), key.as_ptr(), 0, &mut value)
        };
        (status == FC_RESULT_MATCH).then_some(value)
    }

    fn charset(self, index: c_int) -> Option<NonNull<FcCharSet>> {
        let mut chars = ptr::null_mut();
        // SAFETY: A returned charset is borrowed from the live immutable pattern.
        let status = unsafe {
            (self.api.FcPatternGetCharSet)(
                self.raw.as_ptr(),
                FC_CHARSET.as_ptr(),
                index,
                &mut chars,
            )
        };
        if status == FC_RESULT_MATCH {
            NonNull::new(chars)
        } else {
            None
        }
    }

    fn coverage(self, text: Option<&str>, rtl: bool, pass: CoveragePass) -> Option<Coverage> {
        let Some(text) = text else {
            return Some(Coverage::Nominal);
        };
        let first = self.charset(0);
        let maps = |ch| {
            let has_char = |chars: NonNull<FcCharSet>| {
                // SAFETY: The charset belongs to this borrowed scan pattern.
                unsafe { (self.api.FcCharSetHasChar)(chars.as_ptr(), u32::from(ch)) != 0 }
            };
            first.is_none_or(|first| {
                has_char(first) || (1..).map_while(|index| self.charset(index)).any(has_char)
            })
        };
        if cluster::has_advertised_coverage(text, rtl, maps) {
            return Some(match pass {
                CoveragePass::NonDefaultVariation => Coverage::NonDefaultVariation(Vec::new()),
                _ => Coverage::Nominal,
            });
        }
        match pass {
            CoveragePass::Nominal => None,
            _ => {
                cluster::unadvertised_variations(text, rtl, maps).map(Coverage::NonDefaultVariation)
            }
        }
    }

    fn variations(self) -> Vec<usvg::FontVariation> {
        let Some(settings) = self
            .get_string(FC_FONT_VARIATIONS, 0)
            .and_then(|s| s.to_str().ok())
        else {
            return Vec::new();
        };
        let mut axes = Vec::<usvg::FontVariation>::new();
        for setting in settings.split(',') {
            let Some((tag, value)) = setting.split_once('=') else {
                continue;
            };
            let Ok(tag) = <[u8; 4]>::try_from(tag.trim().as_bytes()) else {
                continue;
            };
            let Ok(value) = value.trim().parse::<f32>() else {
                continue;
            };
            if !value.is_finite() || !tag.iter().all(|byte| matches!(byte, b' '..=b'~')) {
                continue;
            }
            if let Some(axis) = axes.iter_mut().find(|axis| axis.tag == tag) {
                axis.value = value;
            } else {
                axes.push(usvg::FontVariation::new(tag, value));
            }
        }
        axes
    }
}

struct FontSet {
    api: &'static Api,
    raw: NonNull<FcFontSet>,
}

impl FontSet {
    fn iter(&self) -> impl Iterator<Item = PatternRef<'_>> {
        // SAFETY: FcFontSort returned this owned font set.
        font_patterns(self.api, unsafe { self.raw.as_ref() })
    }
}

impl Drop for FontSet {
    fn drop(&mut self) {
        // SAFETY: FcFontSort returned this owned font set.
        unsafe { (self.api.FcFontSetSortDestroy)(self.raw.as_ptr()) };
    }
}

fn font_patterns<'a>(api: &'a Api, set: &'a FcFontSet) -> impl Iterator<Item = PatternRef<'a>> {
    let pointers = if set.nfont > 0 && !set.fonts.is_null() {
        // SAFETY: A live Fontconfig set with nfont entries owns this array.
        unsafe { std::slice::from_raw_parts(set.fonts, set.nfont as usize) }
    } else {
        &[]
    };
    pointers.iter().filter_map(|&p| {
        Some(PatternRef {
            api,
            raw: NonNull::new(p)?,
        })
    })
}

/// Try the rest of the catalog after a file disappears. Admission runs first so retained
/// snapshots win. Dangling symlinks keep their source errors.
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
    if !matches!(path.symlink_metadata(), Err(error) if error.kind() == ErrorKind::NotFound) {
        return false;
    }
    for component in path
        .ancestors()
        .skip(1)
        .filter(|p| !p.as_os_str().is_empty())
    {
        match component.symlink_metadata() {
            Err(error) if error.kind() == ErrorKind::NotFound => (),
            Ok(metadata) => {
                return metadata.is_dir() || (metadata.is_symlink() && component.is_dir());
            }
            _ => return false,
        }
    }
    true
}

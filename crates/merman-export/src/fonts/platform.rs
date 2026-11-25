//! Native discovery supplies ranked physical faces to the bounded font resolver.

#[cfg(any(windows, target_os = "linux", target_os = "freebsd", test))]
use super::resolver::ExpectedFace;
use super::resolver::{FontCandidate, FontSelection, GlyphFallback, TriedFont};
use super::*;
use ttf_parser::{RawFace, Tag, name, name_id};
use usvg::fontdb::{Database, FaceInfo, Family, ID, Source, Stretch, Style, Weight};

#[cfg(any(windows, target_os = "macos", test))]
use std::collections::HashMap;

#[cfg(windows)]
type AcceptFallback<'a> = dyn FnMut(FontCandidate) -> Result<Option<ID>, NativeFontError> + 'a;
type AcceptFont<'a> = dyn FnMut(FontSelection, bool) -> Result<Option<ID>, NativeFontError> + 'a;

pub(super) mod cluster;

#[cfg(windows)]
#[allow(unsafe_code)]
mod dwrite;

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub(super) mod coretext;

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[allow(unsafe_code)]
mod fontconfig;

pub(super) const MAX_FVAR_TABLE_BYTES: usize = 1024 * 1024;

#[cfg(windows)]
#[derive(Clone, Copy)]
pub(super) struct DwriteFileIdentity {
    pub(super) last_write_time: u64,
    pub(super) hmtx_len: usize,
    pub(super) hvar_len: Option<usize>,
    pub(super) avar_len: Option<usize>,
    pub(super) metrics_digest: [u8; 32],
}

#[cfg(windows)]
impl DwriteFileIdentity {
    pub(super) fn metric_tables(self) -> [(&'static [u8; 4], Option<usize>); 3] {
        [
            (b"hmtx", Some(self.hmtx_len)),
            (b"HVAR", self.hvar_len),
            (b"avar", self.avar_len),
        ]
    }
}

#[derive(Clone)]
pub(super) struct PlatformFace {
    index: u32,
    family: String,
    post_script_name: Option<String>,
    other_families: Vec<String>,
    pub(super) variations: Vec<usvg::FontVariation>,
    pub(super) fallback_scale: f32,
    #[cfg(windows)]
    pub(super) dwrite_identity: Option<DwriteFileIdentity>,
    #[cfg(windows)]
    pub(super) fvar_digest: Option<[u8; 32]>,
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    pub(super) named_instance: Option<u16>,
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    // An empty list requires any exact UVS. Otherwise each listed pair needs a variant glyph.
    pub(super) required_uvs: Option<Vec<(char, char)>>,
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    pub(super) unmapped_uvs_marks: Vec<char>,
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    pub(super) allowed_symbol_pages: Option<[bool; 2]>,
}

impl PlatformFace {
    #[cfg(any(windows, target_os = "linux", target_os = "freebsd", test))]
    fn into_file(self, path: PathBuf) -> FontCandidate {
        FontCandidate::File {
            path,
            index: self.index,
            expected: ExpectedFace::Platform(self),
        }
    }

    pub(super) fn has_variations(&self) -> bool {
        #[cfg(windows)]
        if self.fvar_digest.is_some() {
            return true;
        }
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        if self.named_instance.is_some() {
            return true;
        }
        !self.variations.is_empty()
    }

    #[cfg(any(windows, target_os = "macos", test))]
    pub(super) fn metadata_bytes(&self) -> usize {
        self.family.len()
            + self.post_script_name.as_ref().map_or(0, String::len)
            + self
                .other_families
                .iter()
                .map(|name| size_of::<String>() + name.len())
                .sum::<usize>()
            + self.variations.len() * size_of::<usvg::FontVariation>()
    }

    pub(super) fn validate(&self, face: &FaceInfo, path: &Path) -> Result<(), NativeFontError> {
        if face.index != self.index
            || !self
                .post_script_name
                .as_deref()
                .is_none_or(|name| matches_post_script_name(face, name))
            || !(matches_family(face, &self.family)
                || self
                    .other_families
                    .iter()
                    .any(|name| matches_family(face, name)))
        {
            return Err(source_error(
                path,
                "font identity changed after platform lookup; retry the operation",
            ));
        }
        Ok(())
    }
}

// Each operation pins a catalog generation. Export forks share the selection view;
// independent operations start with empty candidate caches.
#[derive(Clone)]
enum Backend {
    #[cfg(windows)]
    DirectWrite(Arc<dwrite::Catalog>),
    #[cfg(target_os = "macos")]
    CoreText,
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    Fontconfig(Arc<Mutex<Arc<fontconfig::Catalog>>>),
    #[cfg(test)]
    Fixture(Arc<[FontCandidate]>),
    #[cfg(not(any(
        windows,
        target_os = "macos",
        target_os = "linux",
        target_os = "freebsd",
        test
    )))]
    Unsupported,
}

pub(super) struct PlatformCatalog {
    backend: Backend,
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    generation: OnceLock<Result<Arc<fontconfig::Catalog>, NativeFontError>>,
    #[cfg(any(windows, target_os = "macos", test))]
    family_cache: Mutex<FamilyCache>,
    #[cfg(test)]
    family_queries: std::sync::atomic::AtomicUsize,
}

impl PlatformCatalog {
    pub(super) fn new(control: &OperationControl) -> Result<Self, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Admission)?;
        #[cfg(windows)]
        let backend = Backend::DirectWrite(Arc::new(dwrite::Catalog::new()?));
        #[cfg(target_os = "macos")]
        let backend = Backend::CoreText;
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        let backend =
            Backend::Fontconfig(Arc::new(Mutex::new(Arc::new(fontconfig::Catalog::new()?))));
        #[cfg(any(
            windows,
            target_os = "macos",
            target_os = "linux",
            target_os = "freebsd"
        ))]
        {
            control.terminal_checkpoint_at(OperationPhase::Admission)?;
            Ok(Self::from_backend(backend))
        }
        #[cfg(not(any(
            windows,
            target_os = "macos",
            target_os = "linux",
            target_os = "freebsd"
        )))]
        Err(unavailable_backend())
    }

    #[cfg(any(
        windows,
        target_os = "macos",
        target_os = "linux",
        target_os = "freebsd",
        test
    ))]
    fn from_backend(backend: Backend) -> Self {
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        let generation = match &backend {
            Backend::Fontconfig(shared) => OnceLock::from(
                shared
                    .lock()
                    .map(|current| current.clone())
                    .map_err(|_| poisoned_catalog()),
            ),
            #[cfg(test)]
            Backend::Fixture(_) => OnceLock::new(),
        };
        Self {
            backend,
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            generation,
            #[cfg(any(windows, target_os = "macos", test))]
            family_cache: Mutex::default(),
            #[cfg(test)]
            family_queries: 0.into(),
        }
    }

    pub(super) fn for_operation(&self) -> Arc<Self> {
        #[cfg(windows)]
        let backend = match &self.backend {
            Backend::DirectWrite(catalog) => {
                Backend::DirectWrite(Arc::new(catalog.for_operation()))
            }
            #[cfg(test)]
            Backend::Fixture(faces) => Backend::Fixture(Arc::clone(faces)),
        };
        #[cfg(not(windows))]
        let backend = self.backend.clone();
        Arc::new(Self {
            backend,
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            generation: OnceLock::new(),
            #[cfg(any(windows, target_os = "macos", test))]
            family_cache: Mutex::default(),
            #[cfg(test)]
            family_queries: 0.into(),
        })
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn fontconfig(
        &self,
        shared: &Mutex<Arc<fontconfig::Catalog>>,
    ) -> Result<&fontconfig::Catalog, NativeFontError> {
        self.generation
            .get_or_init(|| {
                let mut current = shared.lock().map_err(|_| poisoned_catalog())?;
                let fresh = current.refreshed()?;
                *current = fresh.clone();
                Ok(fresh)
            })
            .as_deref()
            .map_err(Clone::clone)
    }

    pub(super) fn default_family(
        &self,
        control: &OperationControl,
    ) -> Result<Option<String>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Admission)?;
        match &self.backend {
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            Backend::Fontconfig(shared) => self.fontconfig(shared)?.default_family(control),
            #[cfg(any(windows, target_os = "macos", test))]
            _ => {
                for family in [usvg::FontFamily::SansSerif, usvg::FontFamily::Serif] {
                    for name in self.family_names(&family) {
                        let query = FamilyQuery::new(
                            name,
                            Weight::NORMAL,
                            Stretch::Normal,
                            Style::Normal,
                            None,
                        );
                        if self.family_candidates(query, control)?.iter().any(|face| {
                            #[cfg(target_os = "macos")]
                            if let FontCandidate::Native(face) = &face.candidate {
                                return face.implicit_eligible(None);
                            }
                            let _ = face;
                            true
                        }) {
                            return Ok(Some(name.to_owned()));
                        }
                    }
                }
                Ok(None)
            }
            #[cfg(not(any(
                windows,
                target_os = "macos",
                target_os = "linux",
                target_os = "freebsd",
                test
            )))]
            Backend::Unsupported => Err(unavailable_backend()),
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
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        match &self.backend {
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            Backend::Fontconfig(shared) => self
                .fontconfig(shared)?
                .select(font, explicit, fallback, control, accept),
            #[cfg(any(windows, target_os = "macos", test))]
            _ => self.select_families(font, explicit, fallback, control, accept),
            #[cfg(not(any(
                windows,
                target_os = "macos",
                target_os = "linux",
                target_os = "freebsd",
                test
            )))]
            Backend::Unsupported => {
                let _ = (font, explicit, fallback, accept);
                Err(unavailable_backend())
            }
        }
    }

    pub(super) fn fallback(
        &self,
        request: GlyphFallback<'_>,
        base: &TriedFont,
        control: &OperationControl,
        sources: &super::registry::FontSourceRegistry,
        accept: &mut dyn FnMut(FontCandidate) -> Result<Option<ID>, NativeFontError>,
    ) -> Result<Option<ID>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let _ = sources;
        match &self.backend {
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            Backend::Fontconfig(shared) => {
                let base = &base.face;
                let family = base.families.first().map(|(name, _)| name.as_str());
                let (weight, stretch, style) = request.style(base);
                let mut first_missing = None;
                let selected = self.fontconfig(shared)?.candidates(
                    fontconfig::Query {
                        families: family.as_slice(),
                        weight,
                        stretch,
                        style,
                        text: Some(request.text),
                        language: request.language,
                        rtl: request.rtl,
                    },
                    control,
                    None,
                    |candidate, _| {
                        accept(candidate).or_else(|error| {
                            fontconfig::defer_missing_source(error, &mut first_missing)
                        })
                    },
                )?;
                match (selected, first_missing) {
                    (None, Some(error)) => Err(error),
                    _ => Ok(selected),
                }
            }
            #[cfg(target_os = "macos")]
            Backend::CoreText => {
                let origin = sources.native_origin(request.font, base, control)?;
                coretext::fallback_candidates(
                    request,
                    &base.face,
                    origin.as_ref(),
                    control,
                    &mut |face| accept(FontCandidate::Native(face)),
                )
            }
            #[cfg(windows)]
            Backend::DirectWrite(catalog) => catalog.fallback(request, &base.face, control, accept),
            #[cfg(test)]
            Backend::Fixture(faces) => {
                for candidate in faces.iter() {
                    control.terminal_checkpoint_at(OperationPhase::Layout)?;
                    if let Some(id) = accept(candidate.clone())? {
                        return Ok(Some(id));
                    }
                }
                Ok(None)
            }
            #[cfg(not(any(
                windows,
                target_os = "macos",
                target_os = "linux",
                target_os = "freebsd",
                test
            )))]
            Backend::Unsupported => {
                let _ = (request, base, accept);
                Err(unavailable_backend())
            }
        }
    }
}

#[cfg(not(any(
    windows,
    target_os = "macos",
    target_os = "linux",
    target_os = "freebsd"
)))]
fn unavailable_backend() -> NativeFontError {
    NativeFontError::PlatformDiscoveryUnavailable {
        service: std::env::consts::OS,
    }
}

#[cfg(any(
    windows,
    target_os = "macos",
    target_os = "linux",
    target_os = "freebsd",
    test
))]
fn poisoned_catalog() -> NativeFontError {
    NativeFontError::Measurement("platform font catalog state is poisoned".to_owned())
}

#[cfg(any(windows, target_os = "macos", test))]
#[derive(PartialEq, Eq, Hash)]
struct FamilyQuery {
    name: String,
    weight: Weight,
    stretch: Stretch,
    style: Style,
    language: Option<String>,
}

#[cfg(any(windows, target_os = "macos", test))]
impl FamilyQuery {
    fn new(
        name: &str,
        weight: Weight,
        stretch: Stretch,
        style: Style,
        language: Option<&str>,
    ) -> Self {
        Self {
            name: name.to_owned(),
            weight,
            stretch,
            style,
            language: language.map(str::to_owned),
        }
    }
}

#[cfg(any(windows, target_os = "macos", test))]
#[derive(Default)]
struct FamilyCache {
    entries: HashMap<FamilyQuery, Arc<[FamilyFace]>>,
    bytes: usize,
    #[cfg(windows)]
    cmap_bytes: usize,
}

#[cfg(any(windows, target_os = "macos", test))]
struct FamilyFace {
    candidate: FontCandidate,
    #[cfg(windows)]
    coverage: Option<dwrite::FamilyCoverage>,
}

#[cfg(any(windows, target_os = "macos", test))]
impl From<FontCandidate> for FamilyFace {
    fn from(candidate: FontCandidate) -> Self {
        Self {
            candidate,
            #[cfg(windows)]
            coverage: None,
        }
    }
}

#[cfg(any(windows, target_os = "macos", test))]
impl PlatformCatalog {
    fn select_families(
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
        for (index, family) in font
            .families()
            .iter()
            .chain(fallback.is_none().then_some(&usvg::FontFamily::Serif))
            .enumerate()
        {
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            let family = match family {
                usvg::FontFamily::Named(name) if name.is_empty() => &usvg::FontFamily::SansSerif,
                _ => family,
            };
            let family_fallback_used = index == font.families().len();
            let implicit = match family {
                usvg::FontFamily::Named(name) => is_system_ui(name),
                _ => true,
            };
            let mut try_candidate = |candidate| {
                accept(
                    FontSelection {
                        candidate,
                        family_fallback_used,
                    },
                    implicit,
                )
            };
            if let Some(face) = explicit_face(explicit, family, weight, stretch, style)
                && let Some(id) = try_candidate(FontCandidate::Loaded(face))?
            {
                return Ok(Some(id));
            }
            for name in self.family_names(family) {
                let query = FamilyQuery::new(name, weight, stretch, style, language);
                for face in &*self.family_candidates(query, control)? {
                    control.terminal_checkpoint_at(OperationPhase::Layout)?;
                    #[cfg(windows)]
                    if let Some(fallback) = fallback
                        && face.coverage.as_ref().is_some_and(|coverage| {
                            !coverage.may_cover(fallback.text, fallback.rtl)
                        })
                    {
                        // The native catalog owns this negative coverage result. A positive
                        // still needs snapshot validation and contextual shaping.
                        continue;
                    }
                    if let Some(id) = try_candidate(face.candidate.clone())? {
                        return Ok(Some(id));
                    }
                }
            }
        }
        Ok(None)
    }

    fn family_names<'a>(&'a self, family: &'a usvg::FontFamily) -> Vec<&'a str> {
        #[cfg(test)]
        if let Backend::Fixture(faces) = &self.backend {
            return match family {
                usvg::FontFamily::Named(name) => vec![name],
                _ => faces
                    .iter()
                    .filter_map(|face| match face {
                        FontCandidate::File {
                            expected: ExpectedFace::Platform(face),
                            ..
                        } => Some(face.family.as_str()),
                        _ => None,
                    })
                    .collect(),
            };
        }
        use usvg::FontFamily;
        #[cfg(windows)]
        return match family {
            FontFamily::Named(name) if is_system_ui(name) => vec!["Segoe UI", "Arial"],
            FontFamily::Named(name) => vec![name],
            FontFamily::SansSerif => vec!["Arial", "Segoe UI", "Tahoma"],
            FontFamily::Serif => vec!["Times New Roman", "Georgia", "Cambria"],
            FontFamily::Monospace => vec!["Consolas", "Courier New", "Lucida Console"],
            FontFamily::Cursive => vec!["Segoe Script", "Comic Sans MS", "Segoe UI"],
            FontFamily::Fantasy => vec!["Impact", "Segoe UI"],
        };
        #[cfg(target_os = "macos")]
        return match family {
            FontFamily::Named(name) if is_system_ui(name) => {
                vec!["system-ui", "Helvetica", "Arial"]
            }
            FontFamily::Named(name) => vec![name],
            FontFamily::SansSerif => vec!["Helvetica", "Arial", "Helvetica Neue"],
            FontFamily::Serif => vec!["Times", "Times New Roman", "Georgia"],
            FontFamily::Monospace => vec!["Courier", "Menlo", "Courier New"],
            FontFamily::Cursive => vec!["Apple Chancery", "Helvetica"],
            FontFamily::Fantasy => vec!["Papyrus", "Helvetica"],
        };
        #[cfg(not(any(windows, target_os = "macos")))]
        match family {
            FontFamily::Named(name) => vec![name],
            _ => Vec::new(),
        }
    }

    fn family_candidates(
        &self,
        query: FamilyQuery,
        control: &OperationControl,
    ) -> Result<Arc<[FamilyFace]>, NativeFontError> {
        const MAX_FAMILY_ENTRIES: usize = 512;
        const MAX_FAMILY_BYTES: usize = 512 * 1024;
        #[cfg(windows)]
        const MAX_FAMILY_CMAP_BYTES: usize = 512 * 1024;

        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let cache = self.family_cache.lock().map_err(|_| poisoned_catalog())?;
        if let Some(candidates) = cache.entries.get(&query) {
            return Ok(candidates.clone());
        }
        #[cfg(windows)]
        let available_coverage = if cache.entries.len() < MAX_FAMILY_ENTRIES {
            MAX_FAMILY_CMAP_BYTES - cache.cmap_bytes
        } else {
            0
        };
        drop(cache);
        #[cfg(test)]
        self.family_queries.fetch_add(1, Ordering::Relaxed);
        #[cfg(windows)]
        let mut remaining_coverage = available_coverage;
        let candidates: Arc<[FamilyFace]> = match &self.backend {
            #[cfg(windows)]
            Backend::DirectWrite(catalog) => catalog
                .family_candidates(&query, control, &mut remaining_coverage)?
                .into(),
            #[cfg(target_os = "macos")]
            Backend::CoreText => {
                let faces = if is_system_ui(&query.name) {
                    coretext::system_ui(query.weight, query.stretch, query.style, query.language.as_deref(), control)?.into_iter().collect()
                } else {
                    coretext::named_candidates(&query.name, query.weight, query.stretch, query.style, control)?
                };
                faces.into_iter().map(|face| FontCandidate::Native(face).into()).collect()
            }
            #[cfg(test)]
            Backend::Fixture(faces) => faces.iter().filter(|candidate| {
                matches!(candidate, FontCandidate::File { expected: ExpectedFace::Platform(face), .. } if unicase::eq(face.family.as_str(), query.name.as_str()))
            }).cloned().map(FamilyFace::from).collect(),
            #[cfg(all(test, any(target_os = "linux", target_os = "freebsd")))]
            Backend::Fontconfig(_) => unreachable!("Fontconfig ranks whole substituted patterns"),
        };
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let bytes = size_of::<FamilyQuery>()
            .saturating_add(query.name.len())
            .saturating_add(query.language.as_ref().map_or(0, String::len))
            .saturating_add(
                candidates
                    .iter()
                    .map(|face| {
                        face.candidate.metadata_bytes() + size_of::<FamilyFace>()
                            - size_of::<FontCandidate>()
                    })
                    .sum(),
            );
        let mut cache = self.family_cache.lock().map_err(|_| poisoned_catalog())?;
        if let Some(candidates) = cache.entries.get(&query) {
            return Ok(candidates.clone());
        }
        let fits_cache =
            cache.entries.len() < MAX_FAMILY_ENTRIES && bytes <= MAX_FAMILY_BYTES - cache.bytes;
        #[cfg(windows)]
        let coverage_bytes = available_coverage - remaining_coverage;
        #[cfg(windows)]
        let fits_cache = fits_cache && coverage_bytes <= MAX_FAMILY_CMAP_BYTES - cache.cmap_bytes;
        if fits_cache {
            cache.bytes += bytes;
            #[cfg(windows)]
            {
                cache.cmap_bytes += coverage_bytes;
            }
            cache.entries.insert(query, candidates.clone());
        }
        Ok(candidates)
    }
}

pub(super) fn is_system_ui(name: &str) -> bool {
    ["system-ui", "-apple-system", "BlinkMacSystemFont"]
        .iter()
        .any(|alias| name.eq_ignore_ascii_case(alias))
}

#[cfg(any(windows, target_os = "macos"))]
fn css_width(stretch: Stretch) -> f32 {
    css_width_class(stretch.to_number())
}

#[cfg(any(windows, target_os = "macos"))]
fn css_width_class(class: u16) -> f32 {
    [50.0, 62.5, 75.0, 87.5, 100.0, 112.5, 125.0, 150.0, 200.0][usize::from(class - 1)]
}

#[cfg(target_os = "macos")]
fn ranked_faces<T>(
    faces: Vec<(T, Weight, Stretch, Style)>,
    weight: Weight,
    stretch: Stretch,
    style: Style,
) -> Vec<T> {
    ranked_axis_faces(
        faces
            .into_iter()
            .map(|(face, weight, stretch, style)| {
                (face, f32::from(weight.0), css_width(stretch), style)
            })
            .collect(),
        f32::from(weight.0),
        css_width(stretch),
        style,
    )
}

// CSS Fonts 4 orders width first, then style, then weight. Stable ordering preserves
// the native order for faces with the same rank.
#[cfg(any(windows, target_os = "macos", test))]
fn ranked_axis_faces<T>(
    mut faces: Vec<(T, f32, f32, Style)>,
    weight: f32,
    width: f32,
    style: Style,
) -> Vec<T> {
    let style_order = match style {
        Style::Normal => [Style::Normal, Style::Oblique, Style::Italic],
        Style::Italic => [Style::Italic, Style::Oblique, Style::Normal],
        Style::Oblique => [Style::Oblique, Style::Italic, Style::Normal],
    };
    faces.sort_by_key(|&(_, candidate_weight, candidate_width, candidate_style)| {
        let other_direction = if width <= 100.0 {
            candidate_width > width
        } else {
            candidate_width < width
        };
        let style_rank = style_order
            .iter()
            .position(|&style| style == candidate_style)
            .unwrap_or(3);
        let weight_group = if (400.0..=500.0).contains(&weight) {
            if (weight..=500.0).contains(&candidate_weight) {
                0
            } else if candidate_weight < weight {
                1
            } else {
                2
            }
        } else {
            u8::from(if weight < 400.0 {
                candidate_weight > weight
            } else {
                candidate_weight < weight
            })
        };
        // The distance keys are nonnegative, so their bits preserve numerical order.
        (
            other_direction,
            (candidate_width - width).abs().to_bits(),
            style_rank,
            weight_group,
            (candidate_weight - weight).abs().to_bits(),
        )
    });
    faces.into_iter().map(|(face, ..)| face).collect()
}

fn matches_post_script_name(face: &FaceInfo, expected: &str) -> bool {
    face.post_script_name == expected
        || face_names(face).is_some_and(|table| post_script_name_matches(table, expected))
}

fn post_script_name_matches(table: name::Table<'_>, expected: &str) -> bool {
    table.names.into_iter().any(|record| {
        record.name_id == name_id::POST_SCRIPT_NAME
            && (record.to_string().is_some_and(|name| name == expected)
                // PostScript names are ASCII, including Macintosh Roman name records.
                || (record.platform_id == ttf_parser::PlatformId::Macintosh
                    && record.encoding_id == 0
                    && record.name.is_ascii()
                    && record.name == expected.as_bytes()))
    })
}

fn matches_family(face: &FaceInfo, expected: &str) -> bool {
    face.families
        .iter()
        .any(|(family, _)| unicase::eq(family.as_str(), expected))
        || face_names(face).is_some_and(|table| family_names_match(table, expected))
}

fn face_names(face: &FaceInfo) -> Option<name::Table<'_>> {
    let Source::Binary(data) = &face.source else {
        return None;
    };
    let raw = RawFace::parse(data.as_ref().as_ref(), face.index).ok()?;
    name::Table::parse(raw.table(Tag::from_bytes(b"name"))?)
}

fn family_names_match(table: name::Table<'_>, expected: &str) -> bool {
    table.names.into_iter().any(|record| {
        matches!(
            record.name_id,
            name_id::FAMILY | name_id::TYPOGRAPHIC_FAMILY | name_id::WWS_FAMILY
        ) && record
            .to_string()
            .is_some_and(|family| unicase::eq(family.as_str(), expected))
    })
}

fn explicit_face(
    database: &Database,
    family: &usvg::FontFamily,
    weight: Weight,
    stretch: Stretch,
    style: Style,
) -> Option<FaceInfo> {
    use usvg::FontFamily;
    let family = match family {
        FontFamily::Named(name) => {
            let id = super::super::query_named_font_family_case_insensitively(
                database, name, weight, stretch, style,
            )?;
            return database.face(id).cloned();
        }
        FontFamily::Serif => Family::Serif,
        FontFamily::SansSerif => Family::SansSerif,
        FontFamily::Monospace => Family::Monospace,
        FontFamily::Cursive => Family::Cursive,
        FontFamily::Fantasy => Family::Fantasy,
    };
    let id = super::super::query_font_family(database, family, weight, stretch, style)?;
    database.face(id).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::registry::FontSourceRegistry;

    #[test]
    fn css_axis_ranking_preserves_fractional_distances_and_native_ties() {
        let faces = vec![
            ("wider-far", 499.25, 100.5, Style::Italic),
            ("wider-near", 499.25, 100.25, Style::Italic),
            ("narrower", 499.25, 99.0, Style::Italic),
            ("wrong-style", 499.25, 100.0, Style::Normal),
            ("lighter", 499.0, 100.0, Style::Italic),
            ("above-500", 500.125, 100.0, Style::Italic),
            ("heavier-far", 499.75, 100.0, Style::Italic),
            ("heavier-first", 499.5, 100.0, Style::Italic),
            ("heavier-second", 499.5, 100.0, Style::Italic),
        ];
        assert_eq!(
            ranked_axis_faces(faces, 499.25, 100.0, Style::Italic),
            [
                "heavier-first",
                "heavier-second",
                "heavier-far",
                "lighter",
                "above-500",
                "wrong-style",
                "narrower",
                "wider-near",
                "wider-far",
            ]
        );
    }

    fn test_catalog(filenames: &[&str]) -> PlatformCatalog {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts");
        let mut candidates = Vec::new();
        for name in filenames {
            let path = root.join(name);
            let mut database = Database::new();
            database.load_font_data(fs::read(&path).unwrap());
            for face in database.faces() {
                candidates.push(fixture_face(face).into_file(path.clone()));
            }
        }
        PlatformCatalog::from_backend(Backend::Fixture(candidates.into()))
    }

    fn fixture_face(face: &FaceInfo) -> PlatformFace {
        PlatformFace {
            index: face.index,
            family: face.families[0].0.clone(),
            post_script_name: Some(face.post_script_name.clone()),
            other_families: Vec::new(),
            variations: Vec::new(),
            fallback_scale: 1.0,
            #[cfg(windows)]
            dwrite_identity: None,
            #[cfg(windows)]
            fvar_digest: None,
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            named_instance: None,
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            required_uvs: None,
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            unmapped_uvs_marks: Vec::new(),
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            allowed_symbol_pages: None,
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn primary_requests_keep_distinct_core_text_origins_for_shared_font_bytes() {
        use crate::fonts::registry::{SelectedFont, SelectionRequest};

        let mut options = usvg::Options::default();
        options
            .fontdb_mut()
            .load_font_data(include_bytes!("../../tests/fonts/Cousine-Regular.ttf").to_vec());
        let face = options.fontdb.faces().next().unwrap().clone();
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="60"><text id="sans" font-family="Cousine,sans-serif">A</text><text id="mono" font-family="Cousine,monospace">B</text></svg>"#;
        let tree = usvg::Tree::from_str(svg, &options).unwrap();
        let font = |id| {
            let Some(usvg::Node::Text(node)) = tree.node_by_id(id) else {
                panic!("the fixture must shape {id}");
            };
            node.chunks()[0].spans()[0].font()
        };
        let native = |name| {
            coretext::named(name, Weight::NORMAL, Stretch::Normal, Style::Normal)
                .expect("a standard Core Text family must be available")
        };
        let origins = [
            (font("sans"), native("Helvetica")),
            (font("mono"), native("Courier")),
        ];
        let sources = FontSourceRegistry::new(BTreeMap::new(), BTreeMap::new(), 0, 0);
        let limits = NativeFontLimits::default();
        for (font, origin) in &origins {
            sources
                .remember_selection(
                    SelectionRequest::primary(font),
                    SelectedFont {
                        face: face.clone(),
                        source: NativeExportFontSource::System,
                        scale: 1.0,
                        variations: Vec::new(),
                        family_fallback_used: false,
                        native_origin: Some(origin.clone()),
                    },
                    &limits,
                )
                .unwrap();
        }
        let base = TriedFont {
            face,
            scale: 1.0,
            variations: Vec::new(),
        };
        let control = OperationControl::new();
        for (font, origin) in &origins {
            let selected = sources.native_origin(Some(font), &base, &control).unwrap();
            assert_eq!(selected.as_ref(), Some(origin));
        }
    }

    fn file_candidate(path: &Path, index: u32) -> FontCandidate {
        let mut database = Database::new();
        database.load_font_data(fs::read(path).unwrap());
        let face = database.faces().find(|face| face.index == index).unwrap();
        fixture_face(face).into_file(path.to_owned())
    }

    #[test]
    fn rejected_collection_identity_releases_the_parsed_source() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fonts/Cousine-test-collection.ttc")
            .canonicalize()
            .unwrap();
        let bytes = Arc::new(fs::read(&path).unwrap());
        let registry = FontSourceRegistry::new(
            BTreeMap::from([(path.clone(), Arc::clone(&bytes))]),
            BTreeMap::new(),
            1,
            0,
        );
        let mut candidate = file_candidate(&path, 1);
        let FontCandidate::File {
            expected: ExpectedFace::Platform(face),
            ..
        } = &mut candidate
        else {
            unreachable!()
        };
        face.post_script_name = Some("Cousine-Regular".into());
        let owners = Arc::strong_count(&bytes);
        let mut database = Arc::new(Database::new());
        let error = registry
            .materialize(
                candidate,
                None,
                &mut database,
                &NativeFontLimits::default(),
                &OperationControl::new(),
            )
            .unwrap_err();
        assert!(
            matches!(error, NativeFontError::Source { path: rejected, .. } if rejected == path)
        );
        assert!(database.is_empty());
        assert_eq!(Arc::strong_count(&bytes), owners);
    }

    #[test]
    fn uncovered_collection_member_leaves_the_last_face_slot_for_fallback() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts");
        let collection = root.join("Cousine-test-collection.ttc");
        let registry = FontSourceRegistry::new(BTreeMap::new(), BTreeMap::new(), 0, 0);
        let mut database = Arc::new(Database::new());
        let control = OperationControl::new();
        let limits = NativeFontLimits {
            max_faces: 2,
            ..Default::default()
        };
        let mut admit = |path: &Path, index, fallback| {
            registry
                .materialize(
                    file_candidate(path, index),
                    fallback,
                    &mut database,
                    &limits,
                    &control,
                )
                .unwrap()
        };

        assert!(admit(&collection, 1, None).is_some());
        assert_eq!(admit(&collection, 0, Some('!')), None);
        assert_eq!(registry.admitted_faces(), 1);
        assert!(admit(&root.join("Cousine-Regular.ttf"), 0, Some('!')).is_some());
        assert_eq!(registry.admitted_faces(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn registered_symlink_is_probed_without_admitting_an_uncovered_face() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf");
        let link = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "merman-coverage-link-{}-{:?}.ttf",
            std::process::id(),
            std::thread::current().id()
        ));
        std::os::unix::fs::symlink(fixture, &link).unwrap();
        let registry = FontSourceRegistry::new(BTreeMap::new(), BTreeMap::new(), 0, 0);
        let result = registry.materialize(
            file_candidate(&link, 0),
            Some('\u{0378}'),
            &mut Arc::new(Database::new()),
            &NativeFontLimits::default(),
            &OperationControl::new(),
        );
        fs::remove_file(&link).unwrap();
        assert_eq!(result.unwrap(), None);
        assert_eq!(registry.resident_bytes(), 0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires a registered macOS Arial face"]
    fn uninstalled_face_in_retained_explicit_data_obeys_the_face_limit() {
        let control = OperationControl::new();
        let native =
            coretext::named("Arial", Weight::NORMAL, Stretch::Normal, Style::Normal).unwrap();
        let path = native.path.as_ref().unwrap().canonicalize().unwrap();
        let data = native
            .snapshot(
                FontByteBudget::new(
                    "system-font-bytes",
                    0,
                    NativeFontLimits::default().max_bytes,
                ),
                &control,
            )
            .unwrap();
        // The loader can retain a source even when an earlier descriptor won admission.
        let registry = FontSourceRegistry::new(
            BTreeMap::from([(path, Arc::new(data))]),
            BTreeMap::new(),
            1,
            0,
        );
        let mut database = Arc::new(Database::new());
        let mut admit = |candidate, max_faces| {
            let limits = NativeFontLimits {
                max_faces,
                ..Default::default()
            };
            registry.materialize(candidate, Some('A'), &mut database, &limits, &control)
        };
        let first = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf");
        assert!(admit(file_candidate(&first, 0), 1).unwrap().is_some());
        let resident = registry.resident_bytes();

        assert!(matches!(
            admit(FontCandidate::Native(native.clone()), 1),
            Err(NativeFontError::Limit {
                resource: "font-faces",
                actual: 2,
                limit: 1
            })
        ));
        assert_eq!(registry.admitted_faces(), 1);
        assert!(admit(FontCandidate::Native(native), 2).unwrap().is_some());
        assert_eq!(registry.admitted_faces(), 2);
        assert_eq!(database.len(), 2);
        assert_eq!(registry.resident_bytes(), resident);
    }

    #[test]
    fn non_covering_fallback_does_not_consume_snapshot_budget() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf");
        let registry =
            super::super::registry::FontSourceRegistry::new(BTreeMap::new(), BTreeMap::new(), 0, 0);
        let control = OperationControl::new();
        let limits = NativeFontLimits {
            max_bytes: 1,
            ..Default::default()
        };
        let mut database = Arc::new(Database::new());
        assert!(
            registry
                .materialize(
                    file_candidate(&path, 0),
                    Some('\u{0378}'),
                    &mut database,
                    &limits,
                    &control
                )
                .unwrap()
                .is_none()
        );
        assert_eq!(registry.resident_bytes(), 0);
        assert!(database.is_empty());
        assert!(matches!(
            registry.materialize(
                file_candidate(&path, 0),
                Some('A'),
                &mut database,
                &limits,
                &control
            ),
            Err(NativeFontError::Limit {
                resource: "system-font-bytes",
                limit: 1,
                ..
            })
        ));
    }

    #[test]
    fn file_fallback_probes_clusters_before_charging_the_snapshot() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts");
        let registry = FontSourceRegistry::new(BTreeMap::new(), BTreeMap::new(), 0, 0);
        let control = OperationControl::new();
        let limits = NativeFontLimits {
            max_bytes: 1,
            ..Default::default()
        };
        let mut database = Arc::new(Database::new());
        for (filename, text, should_cover) in [
            ("MermanProbeBase.ttf", "f\u{301}", false),
            ("Cousine-Regular.ttf", "a\u{301}", true),
            ("Cousine-Regular.ttf", "कि", false),
            ("Cousine-Regular.ttf", "ل\u{64e}", false),
            ("MermanProbeBase.ttf", "ב\u{5b8}", false),
            ("Cousine-Regular.ttf", "ב\u{5b8}", true),
            ("Cousine-Regular.ttf", "\u{1100}\u{1161}", false),
            ("Cousine-Regular.ttf", "👩\u{200d}🔬", false),
        ] {
            let candidate = file_candidate(&fixtures.join(filename), 0);
            let result = registry.admit(
                candidate,
                Some(text.into()),
                &mut database,
                &limits,
                &control,
            );
            if should_cover {
                assert!(
                    matches!(
                        result,
                        Err(NativeFontError::Limit {
                            resource: "system-font-bytes",
                            ..
                        })
                    ),
                    "{result:?}"
                );
            } else {
                assert_eq!(result.unwrap(), None, "{text}");
            }
        }
        assert_eq!(registry.resident_bytes(), 0);
        assert!(database.is_empty());
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    #[test]
    fn fontconfig_fallback_rechecks_required_variation_on_cached_snapshot() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf");
        let nominal = fixture_face(&binary_face(fs::read(&path).unwrap()));
        let mut variant = nominal.clone();
        variant.required_uvs = Some(Vec::new());
        let registry = FontSourceRegistry::new(BTreeMap::new(), BTreeMap::new(), 0, 0);
        let control = OperationControl::new();
        let limits = NativeFontLimits::default();
        let mut database = Arc::new(Database::new());
        let mut admit = |face: PlatformFace| {
            registry
                .admit(
                    face.into_file(path.clone()),
                    Some("A\u{fe0f}".into()),
                    &mut database,
                    &limits,
                    &control,
                )
                .unwrap()
        };
        assert!(admit(variant.clone()).is_none());
        assert_eq!(registry.resident_bytes(), 0);
        assert!(admit(nominal).is_some());
        assert!(admit(variant).is_none());
        assert_eq!(database.len(), 1);
    }

    #[test]
    fn fallback_discovery_cannot_override_an_admitted_collection_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "merman-platform-snapshot-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let path = root.join("font.ttc");
        let original = include_bytes!("../../tests/fonts/Cousine-test-collection.ttc");
        fs::write(&path, original).unwrap();
        let candidate = file_candidate(&path, 1);
        let later_candidate = file_candidate(&root.join(".").join("font.ttc"), 1);
        let after_removal = file_candidate(&path, 1);
        let registry =
            super::super::registry::FontSourceRegistry::new(BTreeMap::new(), BTreeMap::new(), 0, 0);
        let control = OperationControl::new();
        let limits = NativeFontLimits::default();
        let mut database = Arc::new(Database::new());
        let selected = registry
            .materialize(candidate, Some('A'), &mut database, &limits, &control)
            .unwrap()
            .unwrap();
        fs::write(
            &path,
            include_bytes!("../../tests/fonts/NotoSansSC-subset.ttf"),
        )
        .unwrap();
        let repeated = registry
            .materialize(later_candidate, Some('A'), &mut database, &limits, &control)
            .unwrap()
            .unwrap();
        assert_eq!(selected, repeated);
        assert_eq!(
            database.face(selected).unwrap().post_script_name,
            "Cousine-Italic"
        );
        assert_eq!(database.face(selected).unwrap().index, 1);
        assert_eq!(registry.resident_bytes(), original.len());
        fs::remove_file(path).unwrap();
        let mut exported = Arc::new(Database::new());
        let export_id = registry
            .for_export(|| Ok(()))
            .unwrap()
            .materialize(after_removal, Some('A'), &mut exported, &limits, &control)
            .unwrap()
            .unwrap();
        exported
            .with_face_data(export_id, |data, index| {
                assert_eq!(index, 1);
                assert_eq!(data, original);
            })
            .unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn postscript_identity_accepts_macintosh_roman_ascii_records() {
        let mut bytes = Vec::new();
        for value in [0_u16, 1, 18, 1, 0, 0, name_id::POST_SCRIPT_NAME, 12, 0] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        bytes.extend_from_slice(b"LucidaGrande");
        let table = name::Table::parse(&bytes).unwrap();
        assert!(post_script_name_matches(table, "LucidaGrande"));
        assert!(!post_script_name_matches(table, "LucidaGrande-Bold"));
    }

    #[test]
    fn postscript_validation_uses_only_records_from_the_selected_collection_index() {
        let mut database = Database::new();
        database.load_font_data(
            include_bytes!("../../tests/fonts/Cousine-test-collection.ttc").to_vec(),
        );
        let mut face = database
            .faces()
            .find(|face| face.index == 1)
            .unwrap()
            .clone();
        let physical_name = std::mem::replace(&mut face.post_script_name, "AnotherRecord".into());
        assert!(matches_post_script_name(&face, &physical_name));
        assert!(!matches_post_script_name(&face, "Cousine-Regular"));
    }

    fn binary_face(bytes: Vec<u8>) -> FaceInfo {
        let mut database = Database::new();
        database.load_font_data(bytes);
        database
            .faces()
            .next()
            .expect("fixture has a font face")
            .clone()
    }

    fn fixture_with_family_aliases() -> Vec<u8> {
        let mut bytes = include_bytes!("../../tests/fonts/Cousine-Regular.ttf").to_vec();
        let table_count = usize::from(u16::from_be_bytes(bytes[4..6].try_into().unwrap()));
        let name_entry = bytes[12..]
            .as_chunks::<16>()
            .0
            .iter()
            .take(table_count)
            .position(|entry| &entry[..4] == b"name")
            .map(|index| 12 + index * 16)
            .expect("fixture has a name table");
        let name_offset = usize::try_from(u32::from_be_bytes(
            bytes[name_entry + 8..name_entry + 12].try_into().unwrap(),
        ))
        .unwrap();
        let count = usize::from(u16::from_be_bytes(
            bytes[name_offset + 2..name_offset + 4].try_into().unwrap(),
        ));
        let storage = name_offset
            + usize::from(u16::from_be_bytes(
                bytes[name_offset + 4..name_offset + 6].try_into().unwrap(),
            ));
        // Retag existing Unicode names without touching glyphs or font properties.
        // The full name and unique ID have enough storage for distinct family aliases.
        for index in 0..count {
            let record = name_offset + 6 + index * 12;
            let platform = u16::from_be_bytes(bytes[record..record + 2].try_into().unwrap());
            if platform != 3 {
                continue;
            }
            let id = u16::from_be_bytes(bytes[record + 6..record + 8].try_into().unwrap());
            let (new_id, family) = match id {
                4 => (
                    rustybuzz::ttf_parser::name_id::TYPOGRAPHIC_FAMILY,
                    "Cousine Text",
                ),
                3 => (rustybuzz::ttf_parser::name_id::WWS_FAMILY, "Cousine WWS"),
                _ => continue,
            };
            let previous_length = usize::from(u16::from_be_bytes(
                bytes[record + 8..record + 10].try_into().unwrap(),
            ));
            let offset = storage
                + usize::from(u16::from_be_bytes(
                    bytes[record + 10..record + 12].try_into().unwrap(),
                ));
            let encoded = family
                .encode_utf16()
                .flat_map(u16::to_be_bytes)
                .collect::<Vec<_>>();
            assert!(encoded.len() <= previous_length);
            bytes[record + 6..record + 8].copy_from_slice(&new_id.to_be_bytes());
            bytes[record + 8..record + 10]
                .copy_from_slice(&u16::try_from(encoded.len()).unwrap().to_be_bytes());
            bytes[offset..offset + encoded.len()].copy_from_slice(&encoded);
        }
        bytes
    }

    #[test]
    fn platform_candidate_validates_identity_independently_of_style() {
        let original =
            binary_face(include_bytes!("../../tests/fonts/Cousine-Regular.ttf").to_vec());
        let descriptor = fixture_face(&original);
        let mut different_style = original;
        different_style.weight = Weight(999);
        different_style.style = Style::Oblique;
        let path = Path::new("font.ttf");
        descriptor.validate(&different_style, path).unwrap();
        let replacement =
            binary_face(include_bytes!("../../tests/fonts/NotoSans-Latin-subset.ttf").to_vec());
        assert!(descriptor.validate(&replacement, path).is_err());
    }

    #[test]
    fn platform_candidate_accepts_binary_legacy_typographic_and_wws_aliases() {
        let face = binary_face(fixture_with_family_aliases());
        assert!(
            face.families
                .iter()
                .all(|(family, _)| family == "Cousine Text")
        );
        for family in ["cOuSiNe", "COUSINE TEXT", "cousine wws"] {
            let candidate = PlatformFace {
                family: family.to_owned(),
                ..fixture_face(&face)
            };
            candidate
                .validate(&face, Path::new("unused-font-path"))
                .expect("family aliases come from the same immutable binary");
        }
    }

    fn poisoned_platform_context(
        symbol_fixture: &str,
        include_text: bool,
        limits: NativeFontLimits,
    ) -> NativeFontContext {
        let mut fixtures = vec![symbol_fixture];
        if include_text {
            fixtures.push("Cousine-Regular.ttf");
        }
        NativeFontContext(Arc::new(FontContextInner {
            database: Arc::new(Database::new()),
            selection: Arc::new(FontSelectionPolicy::default()),
            discovery: Some(super::super::resolver::DiscoveryProvider::Platform {
                catalog: Arc::new(test_catalog(&fixtures)),
                explicit: Arc::new(Database::new()),
            }),
            sources: super::super::registry::FontSourceRegistry::new(
                BTreeMap::new(),
                BTreeMap::new(),
                0,
                0,
            ),
            default_family: String::new(),
            language: None,
            identity: "poisoned-platform-fixture".to_owned(),
            limits,
            control: OperationControl::new(),
            state: Mutex::new(MeasurementState::default()),
            fatal: OnceLock::new(),
        }))
    }

    #[test]
    fn platform_reuses_definitive_missing_coverage_in_one_operation() {
        let context = poisoned_platform_context(
            "MermanSymbolicTest.ttf",
            true,
            NativeFontLimits {
                max_measurements: 2,
                ..Default::default()
            },
        );
        let style = TextStyle::default();
        for _ in 0..2 {
            assert!(
                context
                    .measure(HostTextMeasurementRequest {
                        operation: TextMeasurementOperation::TitleBBoxX,
                        phase: merman_render::environment::TextMeasurementPhase::SvgBBox,
                        text: "\u{10ffff}",
                        style: &style,
                        max_width: None,
                        wrap_mode: WrapMode::SvgLike,
                    })
                    .unwrap()
                    .is_none()
            );
        }
        let state = context.0.state.lock().unwrap();
        assert_eq!(state.measurements, 1);
        assert_eq!(state.cache.len(), 1);
        drop(state);
        context.check().unwrap();
    }

    #[test]
    fn authored_family_queries_are_reused_for_distinct_missing_han_clusters() {
        let context =
            poisoned_platform_context("Cousine-Regular.ttf", false, NativeFontLimits::default());
        let text: String = ('\u{4e00}'..).take(500).collect();
        platform_painted_text(&context, Some("Missing One, Missing Two, Cousine"), &text);
        context.check().unwrap();
        let Some(super::super::resolver::DiscoveryProvider::Platform { catalog, .. }) =
            &context.0.discovery
        else {
            panic!("platform fixture");
        };
        assert_eq!(catalog.family_queries.load(Ordering::Relaxed), 3);
        assert_eq!(context.0.sources.admitted_faces(), 1);
    }

    fn platform_painted_families(
        context: &NativeFontContext,
        family: Option<&str>,
    ) -> BTreeSet<String> {
        platform_painted_text(context, family, "amA")
    }

    fn platform_painted_text(
        context: &NativeFontContext,
        family: Option<&str>,
        text: &str,
    ) -> BTreeSet<String> {
        let mut options = usvg::Options::default();
        context.apply_to_usvg_options(&mut options);
        let family =
            family.map_or_else(String::new, |family| format!(r#" font-family="{family}""#));
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="80"><text id="probe" x="0" y="40"{family}>{text}</text></svg>"#
        );
        let tree = usvg::Tree::from_str(&source, &options).unwrap();
        let Some(usvg::Node::Text(text)) = tree.node_by_id("probe") else {
            return BTreeSet::new();
        };
        text.layouted()
            .iter()
            .flat_map(|span| &span.positioned_glyphs)
            .map(|glyph| {
                tree.fontdb()
                    .face(glyph.font)
                    .unwrap()
                    .post_script_name
                    .clone()
            })
            .collect()
    }

    #[test]
    fn platform_family_selection_rejects_symbolic_generics_before_binary_admission() {
        for (fixture, named) in [
            ("MermanSymbolicTest.ttf", "Merman Symbolic Test"),
            ("MermanPictorialTest.ttf", "Merman Pictorial Test"),
        ] {
            let context = poisoned_platform_context(fixture, true, NativeFontLimits::default());
            for family in [
                Some("serif"),
                Some("sans-serif"),
                Some("monospace"),
                Some("__missing__"),
                None,
            ] {
                assert_eq!(
                    platform_painted_families(&context, family),
                    BTreeSet::from(["Cousine-Regular".to_owned()]),
                    "{fixture}, {family:?}"
                );
                context.check().unwrap();
            }
            assert_eq!(
                platform_painted_families(&context, Some(named)),
                BTreeSet::from([named.replace(' ', "")])
            );
            assert_eq!(context.0.sources.snapshots.lock().unwrap().files.len(), 2);
        }
    }

    #[test]
    fn named_pictorial_font_is_eligible_for_authored_glyph_fallback() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts");
        let mut context = NativeFontOptions::default()
            .with_font_file(root.join("MermanProbeBase.ttf"))
            .with_font_file(root.join("MermanPictorialTest.ttf"))
            .with_font_file(root.join("Cousine-Regular.ttf"))
            .load(&OperationControl::new())
            .unwrap();
        let inner = Arc::get_mut(&mut context.0).unwrap();
        inner.discovery = Some(super::super::resolver::DiscoveryProvider::Platform {
            catalog: Arc::new(test_catalog(&["Cousine-Regular.ttf"])),
            explicit: Arc::clone(&inner.database),
        });
        for (family, expected) in [
            ("Merman Pictorial Test", "MermanPictorialTest"),
            (
                "MermanProbeBase, Merman Pictorial Test, Cousine",
                "MermanPictorialTest",
            ),
            ("MermanProbeBase", "Cousine-Regular"),
        ] {
            assert_eq!(
                platform_painted_families(&context, Some(family)),
                BTreeSet::from([expected.to_owned()]),
                "{family}"
            );
            context.check().unwrap();
        }
    }

    #[test]
    fn platform_glyph_fallback_rejects_symbolic_faces_before_binary_admission() {
        for fixture in ["MermanSymbolicTest.ttf", "MermanPictorialTest.ttf"] {
            for include_text_fallback in [false, true] {
                let discovered = poisoned_platform_context(
                    fixture,
                    include_text_fallback,
                    NativeFontLimits::default(),
                );
                let mut context = NativeFontOptions::default()
                    .with_font_file(
                        Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("tests/fonts/NotoSans-Latin-subset.ttf"),
                    )
                    .load(&OperationControl::new())
                    .unwrap();
                let Some(super::super::resolver::DiscoveryProvider::Platform { catalog, .. }) =
                    &discovered.0.discovery
                else {
                    panic!("platform discovery");
                };
                let inner = Arc::get_mut(&mut context.0).unwrap();
                inner.discovery = Some(super::super::resolver::DiscoveryProvider::Platform {
                    catalog: Arc::clone(catalog),
                    explicit: Arc::clone(&inner.database),
                });
                let mut expected = BTreeSet::from(["NotoSans-Regular"]);
                if include_text_fallback {
                    expected.insert("Cousine-Regular");
                }
                assert_eq!(
                    platform_painted_families(&context, Some("Noto Sans")),
                    expected.into_iter().map(str::to_owned).collect(),
                    "{fixture}, include_text_fallback={include_text_fallback}"
                );
                context.check().unwrap();
            }
        }
    }

    #[test]
    fn platform_symbol_only_generics_remain_unresolved_without_blocking_named_symbol() {
        let context =
            poisoned_platform_context("MermanSymbolicTest.ttf", false, NativeFontLimits::default());
        for family in [Some("serif"), Some("__missing__"), None] {
            assert!(
                platform_painted_families(&context, family).is_empty(),
                "{family:?}"
            );
            context.check().unwrap();
        }
        assert_eq!(
            platform_painted_families(&context, Some("Merman Symbolic Test")),
            BTreeSet::from(["MermanSymbolicTest".to_owned()])
        );
    }

    #[test]
    fn platform_symbol_rejection_leaves_the_snapshot_slot_available() {
        let context = poisoned_platform_context(
            "MermanSymbolicTest.ttf",
            true,
            NativeFontLimits {
                max_files: 1,
                ..Default::default()
            },
        );
        assert_eq!(
            platform_painted_families(&context, Some("serif")),
            BTreeSet::from(["Cousine-Regular".to_owned()])
        );
        context.check().unwrap();
        assert_eq!(context.0.sources.admitted_faces(), 1);
    }

    #[test]
    fn platform_regular_symbol_repairs_a_condensed_bold_italic_base() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts");
        let mut context = NativeFontOptions::default()
            .with_font_file(root.join("MermanProbeBase.ttf"))
            .load(&OperationControl::new())
            .unwrap();
        let inner = Arc::get_mut(&mut context.0).unwrap();
        let mut base = inner.database.faces().next().unwrap().clone();
        base.weight = Weight::BOLD;
        base.style = Style::Italic;
        base.stretch = Stretch::Condensed;
        let mut database = Database::new();
        database.push_face_info(base);
        inner.database = Arc::new(database);
        let catalog = Arc::new(test_catalog(&["MermanProbeFull.ttf"]));
        inner.discovery = Some(super::super::resolver::DiscoveryProvider::Platform {
            catalog,
            explicit: Arc::clone(&inner.database),
        });
        assert_eq!(
            platform_painted_text(&context, Some("MermanProbeBase"), "✓"),
            BTreeSet::from(["MermanProbeFull".to_owned()])
        );
        context.check().unwrap();
    }

    #[test]
    fn platform_font_limit_stops_remaining_callbacks_and_survives_cancellation() {
        let context = poisoned_platform_context(
            "Cousine-Regular.ttf",
            false,
            NativeFontLimits {
                max_bytes: 1,
                ..Default::default()
            },
        );
        let control = context.0.control.clone();
        let Some(super::super::resolver::DiscoveryProvider::Platform { catalog, .. }) =
            &context.0.discovery
        else {
            panic!("platform fixture")
        };
        let mut options = usvg::Options::default();
        context.apply_to_usvg_options_for_export(&mut options, &control);
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="80">
            <text y="20" font-family="Cousine">A</text>
            <text y="40" font-family="Other">V</text>
        </svg>"#;
        let _ = usvg::Tree::from_str(svg, &options).unwrap();
        assert_eq!(catalog.family_queries.load(Ordering::Relaxed), 1);
        assert!(matches!(
            control.terminal_checkpoint_at(OperationPhase::Export),
            Err(OperationLedgerError::ResourceLimitExceeded(_))
        ));
        control.cancel();
        assert!(matches!(
            context.check(),
            Err(NativeFontError::Limit {
                resource: "system-font-bytes",
                limit: 1,
                ..
            })
        ));
    }

    #[test]
    fn cancellation_precedes_platform_initialization() {
        let control = OperationControl::new();
        control.cancel();
        assert!(matches!(
            PlatformCatalog::new(&control),
            Err(NativeFontError::Operation(_))
        ));
    }
}

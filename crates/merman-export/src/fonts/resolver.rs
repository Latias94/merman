//! Discovery supplies candidates; one resolver admits and validates their binary faces.

#[cfg(feature = "experimental-system-font-query")]
use super::registry::{SelectedFont, SelectionRequest};
use super::*;
use usvg::fontdb::{FaceInfo, ID};

/// Discovery-owned selection evidence before bounded binary admission.
pub(super) struct FontSelection {
    pub(super) candidate: FontCandidate,
    pub(super) family_fallback_used: bool,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(feature = "experimental-system-font-query"), allow(dead_code))]
pub(super) struct GlyphFallback<'a> {
    pub(super) font: Option<&'a usvg::Font>,
    pub(super) text: &'a str,
    pub(super) language: Option<&'a str>,
    pub(super) rtl: bool,
}

impl<'a> From<&'a str> for GlyphFallback<'a> {
    fn from(text: &'a str) -> Self {
        Self {
            font: None,
            text,
            language: None,
            rtl: false,
        }
    }
}

impl GlyphFallback<'_> {
    #[cfg(feature = "experimental-system-font-query")]
    pub(super) fn scalar(self) -> Option<char> {
        let mut chars = self.text.chars();
        let first = chars.next()?;
        chars.next().is_none().then_some(first)
    }

    #[cfg(all(
        feature = "experimental-system-font-query",
        any(
            windows,
            target_os = "macos",
            target_os = "linux",
            target_os = "freebsd"
        )
    ))]
    pub(super) fn style(
        self,
        base: &FaceInfo,
    ) -> (
        usvg::fontdb::Weight,
        usvg::fontdb::Stretch,
        usvg::fontdb::Style,
    ) {
        self.font
            .map_or((base.weight, base.stretch, base.style), |font| {
                (
                    usvg::fontdb::Weight(font.weight()),
                    font.stretch().into(),
                    font.style().into(),
                )
            })
    }
}

#[derive(Clone)]
pub(super) enum FontCandidate {
    Loaded(FaceInfo),
    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
    Native(platform::coretext::NativeFace),
    File {
        path: PathBuf,
        index: u32,
        expected: ExpectedFace,
    },
}

impl FontCandidate {
    fn export_source(&self) -> NativeExportFontSource {
        match self {
            // Loaded faces originate from the caller-owned database. Discovery-backed faces
            // always cross the bounded file admission path below.
            Self::Loaded(_) => NativeExportFontSource::Caller,
            Self::File { .. } => NativeExportFontSource::System,
            #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
            Self::Native(_) => NativeExportFontSource::System,
        }
    }

    #[cfg(all(
        feature = "experimental-system-font-query",
        any(windows, target_os = "macos", test)
    ))]
    pub(super) fn metadata_bytes(&self) -> usize {
        size_of::<Self>()
            + match self {
                Self::File {
                    path,
                    expected: ExpectedFace::Platform(face),
                    ..
                } => path.as_os_str().len() + face.metadata_bytes(),
                #[cfg(target_os = "macos")]
                Self::Native(face) => {
                    face.diagnostic_path().as_os_str().len()
                        + face.family.len()
                        + face.post_script_name.len()
                        + std::mem::size_of_val(face.variations.as_slice())
                }
                _ => 0,
            }
    }
}

#[derive(Clone)]
pub(super) enum ExpectedFace {
    Bounded {
        metadata: FaceInfo,
        digest: Vec<u8>,
    },
    #[cfg(feature = "experimental-system-font-query")]
    #[cfg_attr(
        not(any(windows, target_os = "linux", target_os = "freebsd", test)),
        allow(dead_code)
    )]
    Platform(platform::PlatformFace),
}

#[cfg_attr(not(feature = "experimental-system-font-query"), allow(dead_code))]
pub(super) struct AdmittedFont {
    pub(super) id: ID,
    pub(super) scale: f32,
    pub(super) variations: Vec<usvg::FontVariation>,
}

#[cfg(feature = "experimental-system-font-query")]
impl AdmittedFont {
    fn instance(self) -> (ID, f32, Vec<usvg::FontVariation>) {
        (self.id, self.scale, self.variations)
    }
}

pub(super) struct TriedFont {
    pub(super) face: FaceInfo,
    pub(super) scale: f32,
    pub(super) variations: Vec<usvg::FontVariation>,
}

#[derive(Clone, Copy)]
pub(super) struct CandidateRequest<'a> {
    pub(super) fallback: Option<GlyphFallback<'a>>,
    pub(super) implicit: bool,
    pub(super) tried: &'a [TriedFont],
}

impl ExpectedFace {
    pub(super) fn validate(&self, face: &FaceInfo, path: &Path) -> Result<(), NativeFontError> {
        match self {
            Self::Bounded { metadata, .. } if system::same_face(metadata, face) => Ok(()),
            Self::Bounded { .. } => Err(source_error(
                path,
                "font properties changed after metadata discovery; retry the operation",
            )),
            #[cfg(feature = "experimental-system-font-query")]
            Self::Platform(candidate) => candidate.validate(face, path),
        }
    }
}

#[derive(Clone)]
pub(super) enum DiscoveryProvider {
    Bounded(Arc<system::SystemCatalog>),
    #[cfg(feature = "experimental-system-font-query")]
    Platform {
        catalog: Arc<platform::PlatformCatalog>,
        explicit: Arc<usvg::fontdb::Database>,
    },
}

impl DiscoveryProvider {
    pub(super) fn for_operation(&self) -> Self {
        match self {
            Self::Bounded(_) => self.clone(),
            #[cfg(feature = "experimental-system-font-query")]
            Self::Platform { catalog, explicit } => Self::Platform {
                catalog: catalog.for_operation(),
                explicit: Arc::clone(explicit),
            },
        }
    }

    fn select(
        &self,
        font: &usvg::Font,
        control: &OperationControl,
        accept: &mut dyn FnMut(FontSelection, bool) -> Result<Option<ID>, NativeFontError>,
    ) -> Result<Option<ID>, NativeFontError> {
        match self {
            Self::Bounded(catalog) => match catalog.select(font, control)? {
                // This catalog already checked implicit families against retained classification.
                Some(selection) => accept(selection, false),
                None => Ok(None),
            },
            #[cfg(feature = "experimental-system-font-query")]
            Self::Platform { catalog, explicit } => {
                catalog.select(font, explicit, None, control, accept)
            }
        }
    }

    fn fallback(
        &self,
        request: GlyphFallback<'_>,
        tried: &[TriedFont],
        control: &OperationControl,
        sources: &super::registry::FontSourceRegistry,
        accept: &mut dyn FnMut(FontCandidate, bool) -> Result<Option<ID>, NativeFontError>,
    ) -> Result<Option<ID>, NativeFontError> {
        let Some(base) = tried.first() else {
            return Ok(None);
        };
        #[cfg(not(feature = "experimental-system-font-query"))]
        let _ = sources;
        match self {
            Self::Bounded(catalog) => match request.text.chars().next() {
                Some(character) => {
                    let excluded: Vec<_> = tried.iter().map(|font| font.face.clone()).collect();
                    catalog.fallback(
                        character,
                        &base.face,
                        &excluded,
                        control,
                        &mut |candidate| accept(candidate, true),
                    )
                }
                None => Ok(None),
            },
            #[cfg(feature = "experimental-system-font-query")]
            Self::Platform { catalog, explicit } => {
                if let Some(font) = request.font
                    && let Some(id) = catalog.select(
                        font,
                        explicit,
                        Some(request),
                        control,
                        &mut |selection, implicit| accept(selection.candidate, implicit),
                    )?
                {
                    return Ok(Some(id));
                }
                // Initial faces have stable precedence; dynamically installed faces never vote.
                for face in explicit.faces() {
                    control.terminal_checkpoint_at(OperationPhase::Layout)?;
                    if let Some(id) = accept(FontCandidate::Loaded(face.clone()), true)? {
                        return Ok(Some(id));
                    }
                }
                catalog.fallback(request, base, control, sources, &mut |candidate| {
                    accept(candidate, true)
                })
            }
        }
    }

    pub(super) fn catalog_face_count(&self) -> Option<usize> {
        match self {
            Self::Bounded(catalog) => Some(catalog.database.faces().count()),
            #[cfg(feature = "experimental-system-font-query")]
            Self::Platform { .. } => None,
        }
    }
}

pub(super) fn install_resolver(
    options: &mut usvg::Options<'_>,
    context: NativeFontContext,
    control: OperationControl,
    observer: Option<NativeExportFontObserver>,
) {
    options.set_text_fallback(context.0.language.clone(), None);
    let select_context = context.clone();
    let select_control = control.clone();
    let select_observer = observer.clone();

    #[cfg(feature = "experimental-system-font-query")]
    if matches!(
        context.0.discovery,
        Some(DiscoveryProvider::Platform { .. })
    ) {
        options.set_font_instances(
            Some(Box::new(move |font, database| {
                select(
                    &select_context,
                    &select_control,
                    select_observer.as_ref(),
                    font,
                    database,
                )
                .map(AdmittedFont::instance)
            })),
            Some(Box::new(
                move |font, text, language, rtl, excluded, database| {
                    let request = GlyphFallback {
                        font: Some(font),
                        text,
                        language,
                        rtl,
                    };
                    let tried = excluded
                        .iter()
                        .filter_map(|(id, scale, variations)| {
                            Some(TriedFont {
                                face: database.face(*id)?.clone(),
                                scale: *scale,
                                variations: variations.clone(),
                            })
                        })
                        .collect::<Vec<_>>();
                    fallback(
                        &context,
                        &control,
                        observer.as_ref(),
                        request,
                        &tried,
                        database,
                    )
                    .map(AdmittedFont::instance)
                },
            )),
        );
        return;
    }

    options.set_font_instances(None, None);
    options.font_resolver = usvg::FontResolver {
        select_font: Box::new(move |font, database| {
            select(
                &select_context,
                &select_control,
                select_observer.as_ref(),
                font,
                database,
            )
            .map(|choice| choice.id)
        }),
        select_fallback: Box::new(move |character, excluded, database| {
            let mut text = [0; 4];
            let request = GlyphFallback {
                text: character.encode_utf8(&mut text),
                language: context.0.language.as_deref(),
                font: None,
                rtl: false,
            };
            let tried = excluded
                .iter()
                .filter_map(|id| {
                    Some(TriedFont {
                        face: database.face(*id)?.clone(),
                        scale: 1.0,
                        variations: Vec::new(),
                    })
                })
                .collect::<Vec<_>>();
            fallback(
                &context,
                &control,
                observer.as_ref(),
                request,
                &tried,
                database,
            )
            .map(|choice| choice.id)
        }),
    };
}

fn select(
    context: &NativeFontContext,
    control: &OperationControl,
    observer: Option<&NativeExportFontObserver>,
    font: &usvg::Font,
    database: &mut Arc<usvg::fontdb::Database>,
) -> Option<AdmittedFont> {
    resolve(context, control, observer.is_some(), |provider| {
        #[cfg(feature = "experimental-system-font-query")]
        let mut request = matches!(provider, DiscoveryProvider::Platform { .. })
            .then(|| SelectionRequest::primary(font));
        #[cfg(feature = "experimental-system-font-query")]
        if let Some(request) = &request {
            if let Some(selected) = context.0.sources.selected(request, database)? {
                observe_primary(
                    observer,
                    selected.face,
                    selected.source,
                    selected.family_fallback_used,
                );
                return Ok(Some(AdmittedFont {
                    id: selected.face,
                    scale: selected.scale,
                    variations: selected.variations,
                }));
            }
            if context.0.sources.selection_missed(request)? {
                return Ok(None);
            }
        }
        let mut choice = None;
        provider.select(font, control, &mut |selection, implicit| {
            let source = selection.candidate.export_source();
            #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
            let origin = match &selection.candidate {
                FontCandidate::Native(native) => Some(native.clone()),
                _ => None,
            };
            let requirements = CandidateRequest {
                fallback: None,
                implicit,
                tried: &[],
            };
            let Some(admitted) = context.0.sources.admit_choice(
                selection.candidate,
                requirements,
                database,
                &context.0.limits,
                control,
            )?
            else {
                return Ok(None);
            };
            let id = admitted.id;
            #[cfg(feature = "experimental-system-font-query")]
            if let Some(request) = request.take() {
                let pinned = pin(
                    &admitted,
                    source,
                    selection.family_fallback_used,
                    database,
                    #[cfg(target_os = "macos")]
                    origin,
                )?;
                context
                    .0
                    .sources
                    .remember_selection(request, pinned, &context.0.limits)?;
            }
            observe_primary(observer, id, source, selection.family_fallback_used);
            choice = Some(admitted);
            Ok(Some(id))
        })?;
        #[cfg(feature = "experimental-system-font-query")]
        if let Some(request) = request {
            context
                .0
                .sources
                .remember_miss(request, &context.0.limits)?;
        }
        Ok(choice)
    })
}

fn observe_primary(
    observer: Option<&NativeExportFontObserver>,
    id: ID,
    source: NativeExportFontSource,
    family_fallback_used: bool,
) {
    if let Some(observer) = observer {
        observer.admit(id, source);
        if family_fallback_used {
            observer.mark_family_fallback();
        }
    }
}

fn fallback(
    context: &NativeFontContext,
    control: &OperationControl,
    observer: Option<&NativeExportFontObserver>,
    cluster: GlyphFallback<'_>,
    tried: &[TriedFont],
    database: &mut Arc<usvg::fontdb::Database>,
) -> Option<AdmittedFont> {
    let result = resolve(context, control, observer.is_some(), |provider| {
        #[cfg(feature = "experimental-system-font-query")]
        let mut request = matches!(provider, DiscoveryProvider::Platform { .. })
            .then(|| SelectionRequest::fallback(cluster, tried))
            .flatten();
        #[cfg(feature = "experimental-system-font-query")]
        if let Some(request) = &request {
            if let Some(selected) = context.0.sources.selected(request, database)? {
                if let Some(observer) = observer {
                    observer.admit(selected.face, selected.source);
                }
                return Ok(Some(AdmittedFont {
                    id: selected.face,
                    scale: selected.scale,
                    variations: selected.variations,
                }));
            }
            if context.0.sources.selection_missed(request)? {
                return Ok(None);
            }
        }
        let mut choice = None;
        provider.fallback(
            cluster,
            tried,
            control,
            &context.0.sources,
            &mut |candidate, implicit| {
                let source = candidate.export_source();
                let requirements = CandidateRequest {
                    fallback: Some(cluster),
                    implicit,
                    tried,
                };
                let Some(admitted) = context.0.sources.admit_choice(
                    candidate,
                    requirements,
                    database,
                    &context.0.limits,
                    control,
                )?
                else {
                    return Ok(None);
                };
                let id = admitted.id;
                #[cfg(feature = "experimental-system-font-query")]
                if let Some(request) = request.take() {
                    let pinned = pin(
                        &admitted,
                        source,
                        false,
                        database,
                        #[cfg(target_os = "macos")]
                        None,
                    )?;
                    context
                        .0
                        .sources
                        .remember_selection(request, pinned, &context.0.limits)?;
                }
                if let Some(observer) = observer {
                    observer.admit(id, source);
                }
                choice = Some(admitted);
                Ok(Some(id))
            },
        )?;
        #[cfg(feature = "experimental-system-font-query")]
        if let Some(request) = request {
            context
                .0
                .sources
                .remember_miss(request, &context.0.limits)?;
        }
        Ok(choice)
    });
    if let Some(observer) = observer
        && result.is_some()
    {
        observer.mark_glyph_fallback();
    }
    result
}

#[cfg(feature = "experimental-system-font-query")]
fn pin(
    admitted: &AdmittedFont,
    source: NativeExportFontSource,
    family_fallback_used: bool,
    database: &usvg::fontdb::Database,
    #[cfg(target_os = "macos")] native_origin: Option<platform::coretext::NativeFace>,
) -> Result<SelectedFont, NativeFontError> {
    let face = database
        .face(admitted.id)
        .cloned()
        .ok_or_else(|| NativeFontError::Measurement("missing selected font".to_owned()))?;
    Ok(SelectedFont {
        face,
        source,
        family_fallback_used,
        scale: admitted.scale,
        variations: admitted.variations.clone(),
        #[cfg(target_os = "macos")]
        native_origin,
    })
}

fn resolve<T>(
    context: &NativeFontContext,
    control: &OperationControl,
    exporting: bool,
    query: impl FnOnce(&DiscoveryProvider) -> Result<Option<T>, NativeFontError>,
) -> Option<T> {
    let result = (|| {
        context.checkpoint(control)?;
        let provider = context
            .0
            .discovery
            .as_ref()
            .ok_or(NativeFontError::SystemDiscoveryUnavailable)?;
        #[cfg(feature = "experimental-system-font-query")]
        let _selection = matches!(provider, DiscoveryProvider::Platform { .. })
            .then(|| context.0.sources.lock_selection())
            .transpose()?;
        let selection = context.checkpoint(control).and_then(|()| query(provider));
        Ok(selection.unwrap_or_else(|error| {
            context.record_failure(control, exporting, error);
            None
        }))
    })();
    match result {
        Ok(result) => result,
        Err(error) => {
            context.record_failure(control, exporting, error);
            None
        }
    }
}

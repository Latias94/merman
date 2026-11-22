//! Discovery supplies candidates; one resolver admits and validates their binary faces.

#[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
use super::registry::{SelectedFont, SelectionRequest};
use super::*;
use crate::font_selection::{allows_implicit_binary_face, fallback_style_matches};
use usvg::fontdb::{FaceInfo, ID};

/// Discovery-owned selection evidence before bounded binary admission.
pub(super) struct FontSelection {
    pub(super) candidate: FontCandidate,
    pub(super) family_fallback_used: bool,
}

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
}

pub(super) enum ExpectedFace {
    Bounded {
        metadata: FaceInfo,
        digest: Vec<u8>,
    },
    #[cfg(feature = "experimental-system-font-query")]
    Platform(platform::PlatformFace),
}

impl ExpectedFace {
    pub(super) fn validate(
        &self,
        face: &FaceInfo,
        path: &Path,
        origin_index: u32,
    ) -> Result<(), NativeFontError> {
        match self {
            Self::Bounded { metadata, .. }
                if metadata.index == origin_index && system::same_face(metadata, face) =>
            {
                Ok(())
            }
            Self::Bounded { .. } => Err(source_error(
                path,
                "font properties changed after metadata discovery; retry the operation",
            )),
            #[cfg(feature = "experimental-system-font-query")]
            Self::Platform(candidate) => candidate.validate(face, path, origin_index),
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
            Self::Platform { catalog, explicit } => catalog.select(font, explicit, control, accept),
        }
    }

    fn fallback(
        &self,
        character: char,
        base: &FaceInfo,
        excluded: &[FaceInfo],
        control: &OperationControl,
        sources: &super::registry::FontSourceRegistry,
        accept: &mut dyn FnMut(FontCandidate) -> Result<Option<ID>, NativeFontError>,
    ) -> Result<Option<ID>, NativeFontError> {
        #[cfg(not(feature = "experimental-system-font-query"))]
        let _ = sources;
        match self {
            Self::Bounded(catalog) => catalog.fallback(character, base, excluded, control, accept),
            #[cfg(feature = "experimental-system-font-query")]
            Self::Platform { catalog, explicit } => {
                // Initial faces have stable precedence; dynamically installed faces never vote.
                for face in explicit.faces() {
                    control.terminal_checkpoint_at(OperationPhase::Layout)?;
                    if excluded.iter().any(|used| system::same_face(used, face)) {
                        continue;
                    }
                    if let Some(id) = accept(FontCandidate::Loaded(face.clone()))? {
                        return Ok(Some(id));
                    }
                }
                catalog.fallback(character, base, control, sources, accept)
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

pub(super) fn resolver(
    context: NativeFontContext,
    control: OperationControl,
    observer: Option<NativeExportFontObserver>,
) -> usvg::FontResolver<'static> {
    let select_context = context.clone();
    let select_control = control.clone();
    let select_observer = observer.clone();
    usvg::FontResolver {
        select_font: Box::new(move |font, database| {
            let result = (|| {
                select_control.terminal_checkpoint_at(OperationPhase::Layout)?;
                let provider = select_context
                    .0
                    .discovery
                    .as_ref()
                    .ok_or(NativeFontError::SystemDiscoveryUnavailable)?;
                #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
                let _selection = match provider {
                    DiscoveryProvider::Platform { .. } => {
                        Some(select_context.0.sources.lock_selection()?)
                    }
                    _ => None,
                };
                #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
                let request = matches!(provider, DiscoveryProvider::Platform { .. })
                    .then(|| SelectionRequest::Font(font.clone()));
                #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
                if let Some(request) = &request
                    && let Some((id, selected)) =
                        select_context.0.sources.selected(request, database)?
                {
                    if let Some(observer) = &select_observer {
                        observer.admit(id, selected.source);
                        if selected.family_fallback_used {
                            observer.mark_family_fallback();
                        }
                    }
                    return Ok(Some(id));
                }
                provider.select(font, &select_control, &mut |selection, check_implicit| {
                    let source = selection.candidate.export_source();
                    let Some(id) = select_context.0.sources.materialize(
                        selection.candidate,
                        None,
                        database,
                        &select_context.0.limits,
                        &select_control,
                    )?
                    else {
                        return Ok(None);
                    };
                    if check_implicit
                        && !database
                            .face(id)
                            .is_some_and(|face| allows_implicit_binary_face(face, None))
                    {
                        return Ok(None);
                    }
                    let family_fallback_used = selection.family_fallback_used;
                    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
                    let (id, source, family_fallback_used) = if let Some(request) = &request {
                        let face = database.face(id).cloned().ok_or_else(|| {
                            NativeFontError::Measurement("missing selected font".to_owned())
                        })?;
                        let (id, selected) = select_context.0.sources.remember_selection(
                            request.clone(),
                            SelectedFont {
                                face,
                                source,
                                family_fallback_used,
                            },
                            &select_context.0.limits,
                            database,
                        )?;
                        (id, selected.source, selected.family_fallback_used)
                    } else {
                        (id, source, family_fallback_used)
                    };
                    if let Some(observer) = &select_observer {
                        observer.admit(id, source);
                        if family_fallback_used {
                            observer.mark_family_fallback();
                        }
                    }
                    Ok(Some(id))
                })
            })();
            if let Some(observer) = &select_observer
                && !matches!(&result, Ok(Some(_)))
            {
                observer.mark_unresolved_font();
            }
            resolve_or_record(&select_context, result)
        }),
        select_fallback: Box::new(move |character, excluded, database| {
            let result = (|| {
                control.terminal_checkpoint_at(OperationPhase::Layout)?;
                let excluded_faces = excluded
                    .iter()
                    .filter_map(|id| database.face(*id))
                    .cloned()
                    .collect::<Vec<_>>();
                let Some(base) = excluded_faces.first() else {
                    return Ok(None);
                };
                let provider = context
                    .0
                    .discovery
                    .as_ref()
                    .ok_or(NativeFontError::SystemDiscoveryUnavailable)?;
                #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
                let _selection = match provider {
                    DiscoveryProvider::Platform { .. } => Some(context.0.sources.lock_selection()?),
                    _ => None,
                };
                #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
                let request = matches!(provider, DiscoveryProvider::Platform { .. })
                    .then(|| SelectionRequest::fallback(character, &excluded_faces))
                    .flatten();
                #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
                if let Some(request) = &request
                    && let Some((id, selected)) = context.0.sources.selected(request, database)?
                {
                    if let Some(observer) = &observer {
                        observer.admit(id, selected.source);
                    }
                    return Ok(Some(id));
                }
                provider.fallback(
                    character,
                    base,
                    &excluded_faces,
                    &control,
                    &context.0.sources,
                    &mut |candidate| {
                        let source = candidate.export_source();
                        let Some(id) = context.0.sources.materialize(
                            candidate,
                            Some(character),
                            database,
                            &context.0.limits,
                            &control,
                        )?
                        else {
                            return Ok(None);
                        };
                        if excluded.contains(&id) {
                            return Ok(None);
                        }
                        let face = database.face(id).ok_or_else(|| {
                            NativeFontError::Measurement("missing materialized fallback".to_owned())
                        })?;
                        // Coverage belongs to the selected, admitted file and TTC index.
                        // usvg still tests whether this face can repair the shaped run.
                        if !fallback_style_matches(base, face)
                            || !allows_implicit_binary_face(face, Some(character))
                        {
                            return Ok(None);
                        }
                        #[cfg(all(
                            feature = "experimental-system-font-query",
                            target_os = "macos"
                        ))]
                        let (id, source) = if let Some(request) = &request {
                            let (id, selected) = context.0.sources.remember_selection(
                                request.clone(),
                                SelectedFont {
                                    face: face.clone(),
                                    source,
                                    family_fallback_used: false,
                                },
                                &context.0.limits,
                                database,
                            )?;
                            (id, selected.source)
                        } else {
                            (id, source)
                        };
                        if let Some(observer) = &observer {
                            observer.admit(id, source);
                        }
                        Ok(Some(id))
                    },
                )
            })();
            if let Some(observer) = &observer {
                if matches!(&result, Ok(Some(_))) {
                    observer.mark_glyph_fallback();
                } else {
                    observer.mark_unresolved_glyph();
                }
            }
            resolve_or_record(&context, result)
        }),
    }
}

fn resolve_or_record(
    context: &NativeFontContext,
    result: Result<Option<ID>, NativeFontError>,
) -> Option<ID> {
    match result {
        Ok(id) => id,
        Err(error) => {
            context.0.fatal.get_or_init(|| error);
            None
        }
    }
}

//! Discovery supplies candidates; one resolver admits and validates their binary faces.

use super::*;
use usvg::fontdb::{Database, FaceInfo, ID};

pub(super) enum FontCandidate {
    Loaded(FaceInfo),
    File {
        path: PathBuf,
        index: u32,
        expected: ExpectedFace,
    },
}

pub(super) enum ExpectedFace {
    Bounded {
        metadata: FaceInfo,
        digest: Vec<u8>,
    },
    #[cfg(feature = "experimental-system-font-query")]
    Platform(platform::PlatformCandidate),
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
            Self::Platform(candidate) => platform::validate_candidate(face, candidate, path),
        }
    }
}

#[derive(Clone)]
pub(super) enum DiscoveryProvider {
    Bounded(Arc<system::SystemCatalog>),
    #[cfg(feature = "experimental-system-font-query")]
    Platform {
        catalog: Arc<platform::PlatformCatalog>,
        explicit: Arc<Database>,
    },
}

impl DiscoveryProvider {
    fn select(
        &self,
        font: &usvg::Font,
        control: &OperationControl,
        accept: &mut dyn FnMut(FontCandidate, bool) -> Result<Option<ID>, NativeFontError>,
    ) -> Result<Option<ID>, NativeFontError> {
        match self {
            Self::Bounded(catalog) => match catalog.select(font, control)? {
                // Retained catalog classification already gates implicit families.
                Some(candidate) => accept(candidate, false),
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
        accept: &mut dyn FnMut(FontCandidate) -> Result<Option<ID>, NativeFontError>,
    ) -> Result<Option<ID>, NativeFontError> {
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
                for candidate in catalog.fallback_candidates(character, base, control)? {
                    control.terminal_checkpoint_at(OperationPhase::Layout)?;
                    if let Some(id) = accept(platform::candidate(candidate)?)? {
                        return Ok(Some(id));
                    }
                }
                Ok(None)
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
) -> usvg::FontResolver<'static> {
    let select_context = context.clone();
    let select_control = control.clone();
    usvg::FontResolver {
        select_font: Box::new(move |font, database| {
            let result = (|| {
                select_control.terminal_checkpoint_at(OperationPhase::Layout)?;
                let provider = select_context
                    .0
                    .discovery
                    .as_ref()
                    .ok_or(NativeFontError::SystemDiscoveryUnavailable)?;
                provider.select(font, &select_control, &mut |candidate, check_implicit| {
                    let id = select_context.0.sources.materialize(
                        candidate,
                        database,
                        &select_context.0.limits,
                        &select_control,
                    )?;
                    if check_implicit
                        && !database
                            .face(id)
                            .is_some_and(super::super::font_selection::allows_implicit_binary_face)
                    {
                        return Ok(None);
                    }
                    Ok(Some(id))
                })
            })();
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
                provider.fallback(
                    character,
                    base,
                    &excluded_faces,
                    &control,
                    &mut |candidate| {
                        let id = context.0.sources.materialize(
                            candidate,
                            database,
                            &context.0.limits,
                            &control,
                        )?;
                        if excluded.contains(&id) {
                            return Ok(None);
                        }
                        // Delegate style and actual glyph coverage to usvg using only admitted bytes.
                        let mut probe = Database::new();
                        let probe_excluded = excluded_faces
                            .iter()
                            .cloned()
                            .map(|face| probe.push_face_info(face))
                            .collect::<Vec<_>>();
                        let face = database.face(id).cloned().ok_or_else(|| {
                            NativeFontError::Measurement("missing materialized fallback".to_owned())
                        })?;
                        let candidate_id = probe.push_face_info(face);
                        let selected = (usvg::FontResolver::default_fallback_selector())(
                            character,
                            &probe_excluded,
                            &mut Arc::new(probe),
                        );
                        Ok((selected == Some(candidate_id)).then_some(id))
                    },
                )
            })();
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

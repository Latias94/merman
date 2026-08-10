use super::{ExportError, Result};
use merman_render::diagram_theme::{
    FontAssetFingerprint, FontCatalog, FontCatalogFingerprint, FontSource, GenericFontFamily,
};
use merman_render::svg::ResvgCompatibleSvg;
use merman_render::text::{
    PreparedTextLabelId, PreparedTextLabelLedgerEntry, PreparedTextLabelProvenance,
};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

/// Effective font-source order admitted for one native export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExportFontMode {
    /// Resolve only from fonts discovered on the host system.
    SystemOnly,
    /// Resolve only from immutable font assets retained by the rendered document.
    EmbeddedOnly,
    /// Prefer retained font assets, then consult host system fonts.
    EmbeddedThenSystem,
    /// Prefer host system fonts, then consult retained font assets.
    SystemThenEmbedded,
}

impl ExportFontMode {
    /// Returns the stable external identifier for this source order.
    pub const fn id(self) -> &'static str {
        match self {
            Self::SystemOnly => "system-only",
            Self::EmbeddedOnly => "embedded-only",
            Self::EmbeddedThenSystem => "embedded-then-system",
            Self::SystemThenEmbedded => "system-then-embedded",
        }
    }

    /// Returns whether this mode permits retained document fonts.
    pub const fn allows_embedded_fonts(self) -> bool {
        !matches!(self, Self::SystemOnly)
    }

    /// Returns whether this mode permits host system fonts.
    pub const fn allows_system_fonts(self) -> bool {
        !matches!(self, Self::EmbeddedOnly)
    }

    fn sources(self) -> &'static [FontSource] {
        match self {
            Self::SystemOnly => &[FontSource::System],
            Self::EmbeddedOnly => &[FontSource::Embedded],
            Self::EmbeddedThenSystem => &[FontSource::Embedded, FontSource::System],
            Self::SystemThenEmbedded => &[FontSource::System, FontSource::Embedded],
        }
    }
}

/// Frozen font-resolution evidence for one concrete native export target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportFontPlan {
    catalog_fingerprint: FontCatalogFingerprint,
    source_mode: ExportFontMode,
    loaded_embedded_face_count: usize,
    used_embedded_fonts: bool,
    used_system_fonts: bool,
    family_fallback_used: bool,
    glyph_fallback_used: bool,
    unresolved_font_request: bool,
    unresolved_glyph_fallback: bool,
    prepared_label_expected_count: usize,
    prepared_label_verified_count: usize,
    prepared_label_mismatch_count: usize,
    prepared_label_host_dependent_count: usize,
    unclassified_face_count: usize,
    notdef_glyph_count: usize,
}

impl ExportFontPlan {
    /// Returns the exact retained catalog identity used to build the exporter font database.
    pub const fn catalog_fingerprint(self) -> FontCatalogFingerprint {
        self.catalog_fingerprint
    }

    /// Returns the admitted source order after intersecting host policy and catalog availability.
    pub const fn source_mode(self) -> ExportFontMode {
        self.source_mode
    }

    /// Returns the number of retained faces loaded into this exporter operation.
    pub const fn loaded_embedded_face_count(self) -> usize {
        self.loaded_embedded_face_count
    }

    /// Returns whether the final usvg tree contains a visible glyph from a retained face.
    pub const fn used_embedded_fonts(self) -> bool {
        self.used_embedded_fonts
    }

    /// Returns whether the final usvg tree contains a visible glyph from a host system face.
    pub const fn used_system_fonts(self) -> bool {
        self.used_system_fonts
    }

    /// Returns whether a requested font-family list needed a generic or last-resort face.
    pub const fn family_fallback_used(self) -> bool {
        self.family_fallback_used
    }

    /// Returns whether a character needed a different face from the selected family face.
    pub const fn glyph_fallback_used(self) -> bool {
        self.glyph_fallback_used
    }

    /// Returns whether usvg could not select any face for at least one text run.
    pub const fn unresolved_font_request(self) -> bool {
        self.unresolved_font_request
    }

    /// Returns whether usvg could not find a fallback face for at least one character.
    pub const fn unresolved_glyph_fallback(self) -> bool {
        self.unresolved_glyph_fallback
    }

    /// Returns the number of renderer-owned prepared labels expected in this native artifact.
    pub const fn prepared_label_expected_count(self) -> usize {
        self.prepared_label_expected_count
    }

    /// Returns the number of prepared labels whose final face/source set matched the ledger.
    pub const fn prepared_label_verified_count(self) -> usize {
        self.prepared_label_verified_count
    }

    /// Returns the bounded count of prepared-label token or face/source mismatches.
    pub const fn prepared_label_mismatch_count(self) -> usize {
        self.prepared_label_mismatch_count
    }

    /// Returns the number of prepared labels produced by a host-dependent backend.
    pub const fn prepared_label_host_dependent_count(self) -> usize {
        self.prepared_label_host_dependent_count
    }

    /// Returns the number of final font faces that were absent from the operation font map.
    pub const fn unclassified_face_count(self) -> usize {
        self.unclassified_face_count
    }

    /// Returns the number of final positioned glyphs that resolved to `.notdef`.
    pub const fn notdef_glyph_count(self) -> usize {
        self.notdef_glyph_count
    }

    /// Returns whether all prepared-label evidence survived native parsing unchanged.
    pub const fn prepared_text_evidence_matches(self) -> bool {
        self.prepared_label_mismatch_count == 0
            && self.prepared_label_verified_count == self.prepared_label_expected_count
    }

    /// Returns whether the concrete parsed output depends on host-installed font data.
    pub const fn is_host_dependent(self) -> bool {
        self.used_system_fonts || self.prepared_label_host_dependent_count != 0
    }
}

#[derive(Default)]
struct FontResolutionRecorder {
    family_fallback_used: AtomicBool,
    glyph_fallback_used: AtomicBool,
    unresolved_font_request: AtomicBool,
    unresolved_glyph_fallback: AtomicBool,
}

pub(crate) struct ExportFontPlanSeed {
    catalog_fingerprint: FontCatalogFingerprint,
    source_mode: ExportFontMode,
    loaded_embedded_face_count: usize,
    faces: Arc<HashMap<usvg::fontdb::ID, ExportResolvedFace>>,
    catalog_assets: Arc<[ExportCatalogAssetEvidence]>,
    recorder: Arc<FontResolutionRecorder>,
}

impl ExportFontPlanSeed {
    pub(crate) fn finish_with_tree(
        &self,
        tree: &usvg::Tree,
        ledger: &[PreparedTextLabelLedgerEntry],
        evidence_valid: bool,
    ) -> ExportFontPlan {
        let evidence = FinalTreeFontEvidence::collect(
            tree,
            self.faces.as_ref(),
            &self.catalog_assets,
            !ledger.is_empty() && evidence_valid,
        );
        let prepared = if evidence_valid {
            verify_prepared_text_labels(
                self.catalog_fingerprint,
                ledger,
                &evidence.prepared_labels,
                evidence.invalid_prepared_token_count,
            )
        } else {
            PreparedTextVerification {
                expected_count: ledger.len(),
                verified_count: 0,
                mismatch_count: ledger.len(),
                host_dependent_count: ledger
                    .iter()
                    .filter(|entry| entry.provenance().is_host_dependent())
                    .count(),
            }
        };
        let unresolved_glyph_fallback = self
            .recorder
            .unresolved_glyph_fallback
            .load(Ordering::Relaxed)
            || evidence.notdef_glyph_count != 0;

        ExportFontPlan {
            catalog_fingerprint: self.catalog_fingerprint,
            source_mode: self.source_mode,
            loaded_embedded_face_count: self.loaded_embedded_face_count,
            used_embedded_fonts: evidence.used_embedded_fonts,
            used_system_fonts: evidence.used_system_fonts,
            family_fallback_used: self.recorder.family_fallback_used.load(Ordering::Relaxed),
            glyph_fallback_used: self.recorder.glyph_fallback_used.load(Ordering::Relaxed),
            unresolved_font_request: self
                .recorder
                .unresolved_font_request
                .load(Ordering::Relaxed),
            unresolved_glyph_fallback,
            prepared_label_expected_count: prepared.expected_count,
            prepared_label_verified_count: prepared.verified_count,
            prepared_label_mismatch_count: prepared.mismatch_count,
            prepared_label_host_dependent_count: prepared.host_dependent_count,
            unclassified_face_count: evidence.unclassified_face_count,
            notdef_glyph_count: evidence.notdef_glyph_count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ExportFaceKey {
    asset_fingerprint: FontAssetFingerprint,
    face_index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ExportResolvedFace {
    key: Option<ExportFaceKey>,
    source: FontSource,
}

#[derive(Debug, Clone)]
struct ExportCatalogAssetEvidence {
    fingerprint: FontAssetFingerprint,
    data: Arc<[u8]>,
    face_indices: Box<[u32]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ObservedFaceEvidence {
    Classified(ExportResolvedFace),
    Unclassified,
}

#[derive(Debug, Default)]
struct ObservedPreparedLabel {
    saw_base_token: bool,
    line_tokens: HashSet<u32>,
    duplicate_token: bool,
    faces: HashSet<ObservedFaceEvidence>,
}

impl ObservedPreparedLabel {
    fn observe_token(&mut self, line: Option<u32>) {
        match line {
            None => {
                if self.saw_base_token || !self.line_tokens.is_empty() {
                    self.duplicate_token = true;
                }
                self.saw_base_token = true;
            }
            Some(line) => {
                if self.saw_base_token || !self.line_tokens.insert(line) {
                    self.duplicate_token = true;
                }
            }
        }
    }

    fn tokens_match(&self, expected_line_count: usize) -> bool {
        if self.duplicate_token {
            return false;
        }
        if self.saw_base_token {
            return self.line_tokens.is_empty();
        }
        self.line_tokens.len() == expected_line_count
            && (0..expected_line_count).all(|line| {
                u32::try_from(line)
                    .ok()
                    .is_some_and(|line| self.line_tokens.contains(&line))
            })
    }
}

#[derive(Debug, Default)]
struct FinalTreeFontEvidence {
    used_embedded_fonts: bool,
    used_system_fonts: bool,
    unclassified_face_count: usize,
    notdef_glyph_count: usize,
    invalid_prepared_token_count: usize,
    prepared_labels: HashMap<PreparedTextLabelId, ObservedPreparedLabel>,
}

impl FinalTreeFontEvidence {
    fn collect(
        tree: &usvg::Tree,
        faces: &HashMap<usvg::fontdb::ID, ExportResolvedFace>,
        catalog_assets: &[ExportCatalogAssetEvidence],
        collect_prepared_labels: bool,
    ) -> Self {
        let mut evidence = Self::default();
        let mut unclassified_faces = HashSet::new();
        let mut selected_system_face_keys = HashMap::new();
        let mut visited_groups = HashSet::new();
        evidence.visit_group(
            tree.root(),
            tree.fontdb(),
            faces,
            catalog_assets,
            collect_prepared_labels,
            &mut unclassified_faces,
            &mut selected_system_face_keys,
            &mut visited_groups,
        );
        evidence.unclassified_face_count = unclassified_faces.len();
        evidence
    }

    fn visit_group(
        &mut self,
        group: &usvg::Group,
        fontdb: &usvg::fontdb::Database,
        faces: &HashMap<usvg::fontdb::ID, ExportResolvedFace>,
        catalog_assets: &[ExportCatalogAssetEvidence],
        collect_prepared_labels: bool,
        unclassified_faces: &mut HashSet<usvg::fontdb::ID>,
        selected_system_face_keys: &mut HashMap<usvg::fontdb::ID, Option<ExportFaceKey>>,
        visited_groups: &mut HashSet<*const usvg::Group>,
    ) {
        if !visited_groups.insert(std::ptr::from_ref(group)) {
            return;
        }
        for node in group.children() {
            match node {
                usvg::Node::Group(group) => self.visit_group(
                    group,
                    fontdb,
                    faces,
                    catalog_assets,
                    collect_prepared_labels,
                    unclassified_faces,
                    selected_system_face_keys,
                    visited_groups,
                ),
                usvg::Node::Text(text) => self.visit_text(
                    text,
                    fontdb,
                    faces,
                    catalog_assets,
                    collect_prepared_labels,
                    unclassified_faces,
                    selected_system_face_keys,
                ),
                usvg::Node::Path(_) | usvg::Node::Image(_) => {}
            }
            node.subroots(|subroot| {
                self.visit_group(
                    subroot,
                    fontdb,
                    faces,
                    catalog_assets,
                    collect_prepared_labels,
                    unclassified_faces,
                    selected_system_face_keys,
                    visited_groups,
                );
            });
        }
    }

    fn visit_text(
        &mut self,
        text: &usvg::Text,
        fontdb: &usvg::fontdb::Database,
        faces: &HashMap<usvg::fontdb::ID, ExportResolvedFace>,
        catalog_assets: &[ExportCatalogAssetEvidence],
        collect_prepared_labels: bool,
        unclassified_faces: &mut HashSet<usvg::fontdb::ID>,
        selected_system_face_keys: &mut HashMap<usvg::fontdb::ID, Option<ExportFaceKey>>,
    ) {
        let prepared_token = collect_prepared_labels
            .then(|| parse_prepared_text_token(text.id()))
            .flatten();
        if collect_prepared_labels
            && PreparedTextLabelId::is_svg_id_candidate(text.id())
            && prepared_token.is_none()
        {
            self.invalid_prepared_token_count = self.invalid_prepared_token_count.saturating_add(1);
        }

        if let Some((id, line)) = prepared_token {
            self.prepared_labels
                .entry(id)
                .or_default()
                .observe_token(line);
        }

        for span in text.layouted().iter().filter(|span| span.visible) {
            for glyph in &span.positioned_glyphs {
                if glyph.id.0 == 0 {
                    self.notdef_glyph_count = self.notdef_glyph_count.saturating_add(1);
                }
                let face = if let Some(mut face) = faces.get(&glyph.font).copied() {
                    if face.source == FontSource::System && face.key.is_none() {
                        face.key =
                            *selected_system_face_keys
                                .entry(glyph.font)
                                .or_insert_with(|| {
                                    catalog_face_key_for_database_face(
                                        fontdb,
                                        glyph.font,
                                        catalog_assets,
                                    )
                                });
                    }
                    match face.source {
                        FontSource::Embedded => self.used_embedded_fonts = true,
                        FontSource::System => self.used_system_fonts = true,
                        _ => {}
                    }
                    ObservedFaceEvidence::Classified(face)
                } else {
                    unclassified_faces.insert(glyph.font);
                    ObservedFaceEvidence::Unclassified
                };
                if let Some((id, _)) = prepared_token {
                    self.prepared_labels
                        .entry(id)
                        .or_default()
                        .faces
                        .insert(face);
                }
            }
        }
    }
}

fn parse_prepared_text_token(svg_id: &str) -> Option<(PreparedTextLabelId, Option<u32>)> {
    let id = PreparedTextLabelId::from_svg_id(svg_id)?;
    let base = id.as_svg_id();
    if svg_id == base {
        return Some((id, None));
    }
    let line = svg_id
        .strip_prefix(&format!("{base}-line-"))?
        .parse()
        .ok()?;
    Some((id, Some(line)))
}

#[derive(Debug, Clone)]
struct PreparedLabelExpectation {
    id: PreparedTextLabelId,
    catalog_fingerprint: FontCatalogFingerprint,
    provenance: PreparedTextLabelProvenance,
    line_count: usize,
    faces: HashSet<ObservedFaceEvidence>,
}

impl From<&PreparedTextLabelLedgerEntry> for PreparedLabelExpectation {
    fn from(entry: &PreparedTextLabelLedgerEntry) -> Self {
        Self {
            id: entry.id(),
            catalog_fingerprint: entry.catalog_fingerprint(),
            provenance: entry.provenance(),
            line_count: entry.line_count(),
            faces: entry
                .evidence()
                .iter()
                .map(|evidence| {
                    let key = evidence.face_key();
                    ObservedFaceEvidence::Classified(ExportResolvedFace {
                        key: Some(ExportFaceKey {
                            asset_fingerprint: key.asset_fingerprint(),
                            face_index: key.face_index(),
                        }),
                        source: evidence.font_source(),
                    })
                })
                .collect(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct PreparedTextVerification {
    expected_count: usize,
    verified_count: usize,
    mismatch_count: usize,
    host_dependent_count: usize,
}

fn verify_prepared_text_labels(
    catalog_fingerprint: FontCatalogFingerprint,
    ledger: &[PreparedTextLabelLedgerEntry],
    observed: &HashMap<PreparedTextLabelId, ObservedPreparedLabel>,
    invalid_token_count: usize,
) -> PreparedTextVerification {
    let expectations = ledger
        .iter()
        .map(PreparedLabelExpectation::from)
        .collect::<Vec<_>>();
    verify_prepared_label_expectations(
        catalog_fingerprint,
        &expectations,
        observed,
        invalid_token_count,
    )
}

fn verify_prepared_label_expectations(
    catalog_fingerprint: FontCatalogFingerprint,
    expectations: &[PreparedLabelExpectation],
    observed: &HashMap<PreparedTextLabelId, ObservedPreparedLabel>,
    invalid_token_count: usize,
) -> PreparedTextVerification {
    let mut verification = PreparedTextVerification {
        expected_count: expectations.len(),
        mismatch_count: invalid_token_count,
        host_dependent_count: expectations
            .iter()
            .filter(|entry| entry.provenance.is_host_dependent())
            .count(),
        ..PreparedTextVerification::default()
    };
    let mut unique = HashMap::with_capacity(expectations.len());
    let mut duplicate_ids = HashSet::new();
    for expectation in expectations {
        if unique.insert(expectation.id, expectation).is_some() {
            duplicate_ids.insert(expectation.id);
        }
    }

    for (id, expectation) in &unique {
        let valid = !duplicate_ids.contains(id)
            && expectation.catalog_fingerprint == catalog_fingerprint
            && observed.get(id).is_some_and(|actual| {
                actual.tokens_match(expectation.line_count) && actual.faces == expectation.faces
            });
        if valid {
            verification.verified_count = verification.verified_count.saturating_add(1);
        } else {
            verification.mismatch_count = verification.mismatch_count.saturating_add(1);
        }
    }
    verification.mismatch_count = verification
        .mismatch_count
        .saturating_add(duplicate_ids.len())
        .saturating_add(
            observed
                .keys()
                .filter(|id| !unique.contains_key(id))
                .count(),
        );
    verification
}

pub(crate) struct ExportFontEnvironment {
    pub(crate) fontdb: Arc<usvg::fontdb::Database>,
    pub(crate) default_family: String,
    pub(crate) resolver: usvg::FontResolver<'static>,
    pub(crate) plan: ExportFontPlanSeed,
    #[cfg(test)]
    pub(crate) retained_face_ids: Box<[usvg::fontdb::ID]>,
}

impl ExportFontEnvironment {
    pub(crate) fn from_svg(svg: &ResvgCompatibleSvg) -> Result<Self> {
        Self::from_resources(
            svg.font_catalog(),
            svg.font_source_policy(),
            shared_system_fontdb,
        )
    }

    fn from_resources(
        catalog: &FontCatalog,
        policy: &merman_render::diagram_theme::FontSourcePolicy,
        system_fonts: impl FnOnce() -> Arc<usvg::fontdb::Database>,
    ) -> Result<Self> {
        let effective_sources = policy
            .priority()
            .filter(|source| {
                catalog
                    .available_sources()
                    .any(|available| available == *source)
            })
            .collect::<Vec<_>>();
        let mode = match effective_sources.as_slice() {
            [FontSource::System] => ExportFontMode::SystemOnly,
            [FontSource::Embedded] => ExportFontMode::EmbeddedOnly,
            [FontSource::Embedded, FontSource::System] => ExportFontMode::EmbeddedThenSystem,
            [FontSource::System, FontSource::Embedded] => ExportFontMode::SystemThenEmbedded,
            _ => return Err(ExportError::FontSourceUnavailable),
        };

        let system_allowed = effective_sources.contains(&FontSource::System);
        let embedded_allowed = effective_sources.contains(&FontSource::Embedded);
        let system_db = system_allowed.then(system_fonts);
        let mut combined_db = system_db
            .as_deref()
            .cloned()
            .unwrap_or_else(usvg::fontdb::Database::new);
        let mut retained_db = usvg::fontdb::Database::new();
        let mut retained_to_combined = HashMap::new();
        let mut retained_face_ids = Vec::new();
        let mut resolved_faces = combined_db
            .faces()
            .map(|face| {
                (
                    face.id,
                    ExportResolvedFace {
                        key: None,
                        source: FontSource::System,
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        let catalog_assets: Arc<[ExportCatalogAssetEvidence]> = catalog
            .assets()
            .iter()
            .map(|asset| ExportCatalogAssetEvidence {
                fingerprint: asset.fingerprint(),
                data: asset.canonical_data(),
                face_indices: catalog
                    .faces()
                    .iter()
                    .filter(|face| face.asset_id() == asset.id())
                    .map(|face| face.face_index())
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            })
            .collect::<Vec<_>>()
            .into();

        if embedded_allowed {
            for asset in catalog.assets() {
                let data: Arc<dyn AsRef<[u8]> + Send + Sync> =
                    Arc::new(SharedFontData(asset.canonical_data()));
                let retained_ids =
                    retained_db.load_font_source(usvg::fontdb::Source::Binary(Arc::clone(&data)));
                let combined_ids = combined_db.load_font_source(usvg::fontdb::Source::Binary(data));
                if retained_ids.len() != combined_ids.len() || retained_ids.is_empty() {
                    return Err(ExportError::FontCatalogLoad);
                }
                for (retained, combined) in retained_ids.into_iter().zip(combined_ids) {
                    let retained_face_index = retained_db
                        .face(retained)
                        .map(|face| face.index)
                        .ok_or(ExportError::FontCatalogLoad)?;
                    let combined_face_index = combined_db
                        .face(combined)
                        .map(|face| face.index)
                        .ok_or(ExportError::FontCatalogLoad)?;
                    if retained_face_index != combined_face_index
                        || !catalog.faces().iter().any(|face| {
                            face.asset_id() == asset.id()
                                && face.face_index() == combined_face_index
                        })
                    {
                        return Err(ExportError::FontCatalogLoad);
                    }
                    retained_to_combined.insert(retained, combined);
                    resolved_faces.insert(
                        combined,
                        ExportResolvedFace {
                            key: Some(ExportFaceKey {
                                asset_fingerprint: asset.fingerprint(),
                                face_index: combined_face_index,
                            }),
                            source: FontSource::Embedded,
                        },
                    );
                    retained_face_ids.push(combined);
                }
            }
            if retained_face_ids.len() != catalog.faces().len() {
                return Err(ExportError::FontCatalogLoad);
            }
            configure_catalog_generic_families(&mut retained_db, catalog);
            configure_fontdb_generic_families(&mut retained_db);
        }

        let retained_db = Arc::new(retained_db);
        let fontdb = if embedded_allowed {
            configure_catalog_generic_families(&mut combined_db, catalog);
            configure_fontdb_generic_families(&mut combined_db);
            Arc::new(combined_db)
        } else {
            system_db
                .as_ref()
                .map(Arc::clone)
                .ok_or(ExportError::FontSourceUnavailable)?
        };
        let mappings = Arc::new(CatalogFamilyMappings::from_catalog(catalog));
        let recorder = Arc::new(FontResolutionRecorder::default());
        let default_family = catalog_default_family(catalog)
            .or_else(|| {
                default_family_for_sources(mode, system_db.as_deref(), retained_db.as_ref())
            })
            .unwrap_or_else(|| "Arial".to_owned());
        let resolver = catalog_font_resolver(
            mode,
            system_db,
            Arc::clone(&retained_db),
            Arc::new(retained_to_combined),
            mappings,
            Arc::clone(&recorder),
        );

        Ok(Self {
            fontdb,
            default_family,
            resolver,
            plan: ExportFontPlanSeed {
                catalog_fingerprint: catalog.fingerprint(),
                source_mode: mode,
                loaded_embedded_face_count: retained_face_ids.len(),
                faces: Arc::new(resolved_faces),
                catalog_assets,
                recorder,
            },
            #[cfg(test)]
            retained_face_ids: retained_face_ids.into_boxed_slice(),
        })
    }
}

fn catalog_face_key_for_database_face(
    database: &usvg::fontdb::Database,
    id: usvg::fontdb::ID,
    catalog_assets: &[ExportCatalogAssetEvidence],
) -> Option<ExportFaceKey> {
    database
        .with_face_data(id, |data, face_index| {
            let asset = catalog_assets.iter().find(|asset| {
                asset.data.as_ref() == data && asset.face_indices.contains(&face_index)
            })?;
            Some(ExportFaceKey {
                asset_fingerprint: asset.fingerprint,
                face_index,
            })
        })
        .flatten()
}

#[derive(Clone)]
struct SharedFontData(Arc<[u8]>);

impl AsRef<[u8]> for SharedFontData {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

fn catalog_font_resolver(
    mode: ExportFontMode,
    system_db: Option<Arc<usvg::fontdb::Database>>,
    retained_db: Arc<usvg::fontdb::Database>,
    retained_to_combined: Arc<HashMap<usvg::fontdb::ID, usvg::fontdb::ID>>,
    mappings: Arc<CatalogFamilyMappings>,
    recorder: Arc<FontResolutionRecorder>,
) -> usvg::FontResolver<'static> {
    let sources = Arc::new(FontResolverSources {
        mode,
        system_db,
        retained_db,
        retained_to_combined,
        mappings,
        recorder,
    });
    let select_sources = Arc::clone(&sources);
    let fallback_sources = Arc::clone(&sources);

    usvg::FontResolver {
        select_font: Box::new(move |font, combined_db| {
            if let Some((_, id)) = select_sources.query_requested_font(font) {
                debug_assert!(combined_db.face(id).is_some());
                return Some(id);
            }
            if let Some((_, id)) = select_sources.query_family_fallback(font) {
                debug_assert!(combined_db.face(id).is_some());
                select_sources
                    .recorder
                    .family_fallback_used
                    .store(true, Ordering::Relaxed);
                return Some(id);
            }
            select_sources
                .recorder
                .unresolved_font_request
                .store(true, Ordering::Relaxed);
            None
        }),
        select_fallback: Box::new(move |character, excluded, combined_db| {
            if let Some((_, id)) =
                fallback_sources.query_glyph_fallback(character, excluded, combined_db.as_ref())
            {
                fallback_sources
                    .recorder
                    .glyph_fallback_used
                    .store(true, Ordering::Relaxed);
                return Some(id);
            }
            fallback_sources
                .recorder
                .unresolved_glyph_fallback
                .store(true, Ordering::Relaxed);
            None
        }),
    }
}

struct FontResolverSources {
    mode: ExportFontMode,
    system_db: Option<Arc<usvg::fontdb::Database>>,
    retained_db: Arc<usvg::fontdb::Database>,
    retained_to_combined: Arc<HashMap<usvg::fontdb::ID, usvg::fontdb::ID>>,
    mappings: Arc<CatalogFamilyMappings>,
    recorder: Arc<FontResolutionRecorder>,
}

impl FontResolverSources {
    fn database(&self, source: FontSource) -> Option<&usvg::fontdb::Database> {
        match source {
            FontSource::Embedded => Some(self.retained_db.as_ref()),
            FontSource::System => self.system_db.as_deref(),
            _ => None,
        }
    }

    fn to_combined_id(&self, source: FontSource, id: usvg::fontdb::ID) -> Option<usvg::fontdb::ID> {
        match source {
            FontSource::Embedded => self.retained_to_combined.get(&id).copied(),
            FontSource::System => Some(id),
            _ => None,
        }
    }

    fn query_requested_font(&self, font: &usvg::Font) -> Option<(FontSource, usvg::fontdb::ID)> {
        self.mode.sources().iter().find_map(|source| {
            let database = self.database(*source)?;
            let id = query_font(font, database, self.mappings.as_ref())?;
            Some((*source, self.to_combined_id(*source, id)?))
        })
    }

    fn query_family_fallback(&self, font: &usvg::Font) -> Option<(FontSource, usvg::fontdb::ID)> {
        self.mode.sources().iter().find_map(|source| {
            let database = self.database(*source)?;
            let id = query_default_serif(font, database)
                .or_else(|| query_browser_like_fallback_font(font, database))
                .or_else(|| database.faces().next().map(|face| face.id))?;
            Some((*source, self.to_combined_id(*source, id)?))
        })
    }

    fn query_glyph_fallback(
        &self,
        character: char,
        excluded: &[usvg::fontdb::ID],
        combined_db: &usvg::fontdb::Database,
    ) -> Option<(FontSource, usvg::fontdb::ID)> {
        let base_face = excluded.first().and_then(|id| combined_db.face(*id));
        self.mode.sources().iter().find_map(|source| {
            let database = self.database(*source)?;
            database.faces().find_map(|face| {
                let combined_id = self.to_combined_id(*source, face.id)?;
                if excluded.contains(&combined_id) {
                    return None;
                }
                if let Some(base) = base_face
                    && base.style != face.style
                    && base.weight != face.weight
                    && base.stretch != face.stretch
                {
                    return None;
                }
                face_has_char(database, face.id, character).then_some((*source, combined_id))
            })
        })
    }
}

fn face_has_char(database: &usvg::fontdb::Database, id: usvg::fontdb::ID, character: char) -> bool {
    database
        .with_face_data(id, |data, face_index| {
            ttf_parser::Face::parse(data, face_index)
                .ok()
                .and_then(|face| face.glyph_index(character))
                .is_some()
        })
        .unwrap_or(false)
}

#[derive(Default)]
struct CatalogFamilyMappings {
    aliases: HashMap<String, String>,
    generic_families: HashMap<GenericFontFamily, String>,
}

impl CatalogFamilyMappings {
    fn from_catalog(catalog: &FontCatalog) -> Self {
        let mut aliases = catalog
            .aliases()
            .iter()
            .map(|alias| {
                (
                    alias.alias().to_ascii_lowercase(),
                    alias.target().to_owned(),
                )
            })
            .collect::<HashMap<_, _>>();
        if let Some(system_ui) = catalog.generic_family(GenericFontFamily::SystemUi) {
            aliases.insert("system-ui".to_owned(), system_ui.to_owned());
        }
        let generic_families = [
            GenericFontFamily::Serif,
            GenericFontFamily::SansSerif,
            GenericFontFamily::Monospace,
            GenericFontFamily::Cursive,
            GenericFontFamily::Fantasy,
        ]
        .into_iter()
        .filter_map(|generic| {
            catalog
                .generic_family(generic)
                .map(|target| (generic, target.to_owned()))
        })
        .collect();
        Self {
            aliases,
            generic_families,
        }
    }

    fn resolve(&self, family: &usvg::FontFamily) -> ResolvedFamily {
        let generic = match family {
            usvg::FontFamily::Serif => Some(GenericFontFamily::Serif),
            usvg::FontFamily::SansSerif => Some(GenericFontFamily::SansSerif),
            usvg::FontFamily::Cursive => Some(GenericFontFamily::Cursive),
            usvg::FontFamily::Fantasy => Some(GenericFontFamily::Fantasy),
            usvg::FontFamily::Monospace => Some(GenericFontFamily::Monospace),
            usvg::FontFamily::Named(name) => {
                return ResolvedFamily::Named(
                    self.aliases
                        .get(&name.to_ascii_lowercase())
                        .cloned()
                        .unwrap_or_else(|| name.clone()),
                );
            }
        };
        if let Some(target) = generic.and_then(|family| self.generic_families.get(&family)) {
            return ResolvedFamily::Named(target.clone());
        }
        match family {
            usvg::FontFamily::Serif => ResolvedFamily::Serif,
            usvg::FontFamily::SansSerif => ResolvedFamily::SansSerif,
            usvg::FontFamily::Cursive => ResolvedFamily::Cursive,
            usvg::FontFamily::Fantasy => ResolvedFamily::Fantasy,
            usvg::FontFamily::Monospace => ResolvedFamily::Monospace,
            usvg::FontFamily::Named(_) => unreachable!("named family returned above"),
        }
    }
}

enum ResolvedFamily {
    Serif,
    SansSerif,
    Cursive,
    Fantasy,
    Monospace,
    Named(String),
}

fn query_font(
    font: &usvg::Font,
    fontdb: &usvg::fontdb::Database,
    mappings: &CatalogFamilyMappings,
) -> Option<usvg::fontdb::ID> {
    let resolved = font
        .families()
        .iter()
        .map(|family| mappings.resolve(family))
        .collect::<Vec<_>>();
    let families = resolved
        .iter()
        .map(|family| match family {
            ResolvedFamily::Serif => usvg::fontdb::Family::Serif,
            ResolvedFamily::SansSerif => usvg::fontdb::Family::SansSerif,
            ResolvedFamily::Cursive => usvg::fontdb::Family::Cursive,
            ResolvedFamily::Fantasy => usvg::fontdb::Family::Fantasy,
            ResolvedFamily::Monospace => usvg::fontdb::Family::Monospace,
            ResolvedFamily::Named(name) => usvg::fontdb::Family::Name(name),
        })
        .collect::<Vec<_>>();

    fontdb.query(&usvg::fontdb::Query {
        families: &families,
        weight: usvg::fontdb::Weight(font.weight()),
        stretch: font.stretch().into(),
        style: font.style().into(),
    })
}

fn query_default_serif(
    font: &usvg::Font,
    fontdb: &usvg::fontdb::Database,
) -> Option<usvg::fontdb::ID> {
    fontdb.query(&usvg::fontdb::Query {
        families: &[usvg::fontdb::Family::Serif],
        weight: usvg::fontdb::Weight(font.weight()),
        stretch: font.stretch().into(),
        style: font.style().into(),
    })
}

fn query_browser_like_fallback_font(
    font: &usvg::Font,
    fontdb: &usvg::fontdb::Database,
) -> Option<usvg::fontdb::ID> {
    let families = if font_requests_monospace(font) {
        [
            usvg::fontdb::Family::Monospace,
            usvg::fontdb::Family::SansSerif,
            usvg::fontdb::Family::Serif,
        ]
    } else {
        [
            usvg::fontdb::Family::SansSerif,
            usvg::fontdb::Family::Serif,
            usvg::fontdb::Family::Monospace,
        ]
    };

    fontdb.query(&usvg::fontdb::Query {
        families: &families,
        weight: usvg::fontdb::Weight(font.weight()),
        stretch: font.stretch().into(),
        style: font.style().into(),
    })
}

fn font_requests_monospace(font: &usvg::Font) -> bool {
    font.families().iter().any(|family| match family {
        usvg::FontFamily::Monospace => true,
        usvg::FontFamily::Named(name) => {
            let name = name.to_ascii_lowercase();
            name.contains("mono")
                || name.contains("courier")
                || name.contains("consolas")
                || name.contains("menlo")
        }
        _ => false,
    })
}

fn catalog_default_family(catalog: &FontCatalog) -> Option<String> {
    [
        GenericFontFamily::SansSerif,
        GenericFontFamily::SystemUi,
        GenericFontFamily::Serif,
    ]
    .into_iter()
    .find_map(|generic| catalog.generic_family(generic).map(ToOwned::to_owned))
}

fn default_family_for_sources(
    mode: ExportFontMode,
    system_db: Option<&usvg::fontdb::Database>,
    retained_db: &usvg::fontdb::Database,
) -> Option<String> {
    mode.sources().iter().find_map(|source| match source {
        FontSource::Embedded => default_font_family(retained_db),
        FontSource::System => system_db.and_then(default_font_family),
        _ => None,
    })
}

fn configure_catalog_generic_families(fontdb: &mut usvg::fontdb::Database, catalog: &FontCatalog) {
    for generic in [
        GenericFontFamily::Serif,
        GenericFontFamily::SansSerif,
        GenericFontFamily::Monospace,
        GenericFontFamily::Cursive,
        GenericFontFamily::Fantasy,
    ] {
        let Some(target) = catalog.generic_family(generic) else {
            continue;
        };
        match generic {
            GenericFontFamily::Serif => fontdb.set_serif_family(target),
            GenericFontFamily::SansSerif => fontdb.set_sans_serif_family(target),
            GenericFontFamily::Monospace => fontdb.set_monospace_family(target),
            GenericFontFamily::Cursive => fontdb.set_cursive_family(target),
            GenericFontFamily::Fantasy => fontdb.set_fantasy_family(target),
            GenericFontFamily::SystemUi => unreachable!("system-ui is resolved as a named alias"),
            _ => {}
        }
    }
}

fn shared_system_fontdb() -> Arc<usvg::fontdb::Database> {
    static FONTDB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    Arc::clone(FONTDB.get_or_init(|| {
        let mut fontdb = usvg::fontdb::Database::new();
        fontdb.load_system_fonts();
        configure_fontdb_generic_families(&mut fontdb);
        Arc::new(fontdb)
    }))
}

fn configure_fontdb_generic_families(fontdb: &mut usvg::fontdb::Database) {
    let sans = first_font_family(fontdb, |face| !face.monospaced)
        .or_else(|| first_font_family(fontdb, |_| true));
    let mono = first_font_family(fontdb, |face| face.monospaced).or_else(|| sans.clone());

    if query_normal_font_family(fontdb, usvg::fontdb::Family::SansSerif).is_none()
        && let Some(family) = sans.as_ref()
    {
        fontdb.set_sans_serif_family(family.clone());
    }
    if query_normal_font_family(fontdb, usvg::fontdb::Family::Serif).is_none()
        && let Some(family) = sans.as_ref()
    {
        fontdb.set_serif_family(family.clone());
    }
    if query_normal_font_family(fontdb, usvg::fontdb::Family::Monospace).is_none()
        && let Some(family) = mono.as_ref()
    {
        fontdb.set_monospace_family(family.clone());
    }
}

fn default_font_family(fontdb: &usvg::fontdb::Database) -> Option<String> {
    query_normal_font_family(fontdb, usvg::fontdb::Family::SansSerif)
        .or_else(|| query_normal_font_family(fontdb, usvg::fontdb::Family::Serif))
        .or_else(|| first_font_family(fontdb, |_| true))
}

fn query_normal_font_family(
    fontdb: &usvg::fontdb::Database,
    family: usvg::fontdb::Family<'_>,
) -> Option<String> {
    let families = [family];
    fontdb
        .query(&usvg::fontdb::Query {
            families: &families,
            weight: usvg::fontdb::Weight::NORMAL,
            stretch: usvg::fontdb::Stretch::Normal,
            style: usvg::fontdb::Style::Normal,
        })
        .and_then(|id| fontdb.face(id))
        .and_then(face_family_name)
}

fn first_font_family<F>(fontdb: &usvg::fontdb::Database, mut predicate: F) -> Option<String>
where
    F: FnMut(&usvg::fontdb::FaceInfo) -> bool,
{
    fontdb
        .faces()
        .find(|face| predicate(face))
        .and_then(face_family_name)
}

fn face_family_name(face: &usvg::fontdb::FaceInfo) -> Option<String> {
    face.families
        .iter()
        .find(|(_, lang)| *lang == usvg::fontdb::Language::English_UnitedStates)
        .or_else(|| face.families.first())
        .map(|(family, _)| family.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_render::diagram_theme::{
        FontAssetSpec, FontCatalogSpec, FontSourcePolicy, ThemeResourcePolicy,
    };
    use merman_render::environment::RenderEnvironment;
    use merman_render::svg::finalize_resvg_svg;

    fn custom_catalog(available_sources: &[FontSource]) -> FontCatalog {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        FontCatalogSpec::new([FontAssetSpec::new("excalifont", bytes)])
            .with_alias("Sketch", "Excalifont")
            .with_generic_family(GenericFontFamily::SansSerif, "Excalifont")
            .with_available_sources(available_sources.iter().copied())
            .compile(&ThemeResourcePolicy::interactive())
            .unwrap()
    }

    fn sealed_svg(catalog: FontCatalog, policy: FontSourcePolicy) -> ResvgCompatibleSvg {
        let session = RenderEnvironment::deterministic()
            .with_font_catalog(catalog)
            .with_font_source_policy(policy)
            .begin_session()
            .unwrap();
        finalize_resvg_svg(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text font-family="Sketch">Alpha</text></svg>"#,
            &session,
        )
        .unwrap()
    }

    fn sealed_sized_svg(catalog: FontCatalog, policy: FontSourcePolicy) -> ResvgCompatibleSvg {
        sealed_sized_svg_with_text(catalog, policy, "Sketch", "MW")
    }

    fn sealed_sized_svg_with_text(
        catalog: FontCatalog,
        policy: FontSourcePolicy,
        family: &str,
        text: &str,
    ) -> ResvgCompatibleSvg {
        let session = RenderEnvironment::deterministic()
            .with_font_catalog(catalog)
            .with_font_source_policy(policy)
            .begin_session()
            .unwrap();
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="180" height="64" viewBox="0 0 180 64"><rect width="180" height="64" fill="white"/><text x="8" y="44" font-family="{family}" font-size="42" fill="black">{text}</text></svg>"#
        );
        finalize_resvg_svg(&source, &session).unwrap()
    }

    fn font_database_from_catalog(catalog: &FontCatalog) -> Arc<usvg::fontdb::Database> {
        let mut database = usvg::fontdb::Database::new();
        for asset in catalog.assets() {
            let data: Arc<dyn AsRef<[u8]> + Send + Sync> =
                Arc::new(SharedFontData(asset.canonical_data()));
            assert!(
                !database
                    .load_font_source(usvg::fontdb::Source::Binary(data))
                    .is_empty()
            );
        }
        configure_catalog_generic_families(&mut database, catalog);
        configure_fontdb_generic_families(&mut database);
        Arc::new(database)
    }

    fn resolve_font_plan(
        svg: &ResvgCompatibleSvg,
        environment: ExportFontEnvironment,
    ) -> ExportFontPlan {
        let ExportFontEnvironment {
            fontdb,
            default_family,
            resolver,
            plan,
            ..
        } = environment;
        let mut options = usvg::Options::default();
        options.fontdb = fontdb;
        options.font_family = default_family;
        options.font_resolver = resolver;
        let tree = usvg::Tree::from_str(svg.native_export_svg(), &options).unwrap();
        plan.finish_with_tree(
            &tree,
            svg.prepared_text_label_ledger(),
            svg.prepared_text_evidence_valid(),
        )
    }

    struct PreparedVerifierFixture {
        catalog_fingerprint: FontCatalogFingerprint,
        face: ExportResolvedFace,
        observed: HashMap<PreparedTextLabelId, ObservedPreparedLabel>,
        invalid_token_count: usize,
    }

    fn prepared_verifier_fixture(ids: &[&str]) -> PreparedVerifierFixture {
        let catalog = custom_catalog(&[FontSource::Embedded]);
        let environment = ExportFontEnvironment::from_resources(
            &catalog,
            &FontSourcePolicy::embedded_only(),
            || panic!("embedded-only verifier fixture must not discover system fonts"),
        )
        .unwrap();
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="{}" viewBox="0 0 240 {}">{}</svg>"#,
            (ids.len().max(1) * 32),
            (ids.len().max(1) * 32),
            ids.iter()
                .enumerate()
                .map(|(index, id)| format!(
                    r#"<text id="{id}" x="4" y="{}" font-family="Sketch" font-size="20">Alpha</text>"#,
                    index * 32 + 24
                ))
                .collect::<String>()
        );
        let mut options = usvg::Options::default();
        options.fontdb = Arc::clone(&environment.fontdb);
        options.font_family = environment.default_family.clone();
        options.font_resolver = environment.resolver;
        let tree = usvg::Tree::from_str(&source, &options).unwrap();
        let evidence = FinalTreeFontEvidence::collect(
            &tree,
            environment.plan.faces.as_ref(),
            &environment.plan.catalog_assets,
            true,
        );
        let asset = &catalog.assets()[0];
        let face = &catalog.faces()[0];

        PreparedVerifierFixture {
            catalog_fingerprint: catalog.fingerprint(),
            face: ExportResolvedFace {
                key: Some(ExportFaceKey {
                    asset_fingerprint: asset.fingerprint(),
                    face_index: face.face_index(),
                }),
                source: FontSource::Embedded,
            },
            observed: evidence.prepared_labels,
            invalid_token_count: evidence.invalid_prepared_token_count,
        }
    }

    fn prepared_expectation(
        fixture: &PreparedVerifierFixture,
        svg_id: &str,
    ) -> PreparedLabelExpectation {
        prepared_expectation_with_line_count(fixture, svg_id, 1)
    }

    fn prepared_expectation_with_line_count(
        fixture: &PreparedVerifierFixture,
        svg_id: &str,
        line_count: usize,
    ) -> PreparedLabelExpectation {
        PreparedLabelExpectation {
            id: PreparedTextLabelId::from_svg_id(svg_id).unwrap(),
            catalog_fingerprint: fixture.catalog_fingerprint,
            provenance: PreparedTextLabelProvenance::Native,
            line_count,
            faces: HashSet::from([ObservedFaceEvidence::Classified(fixture.face)]),
        }
    }

    #[test]
    fn prepared_label_verifier_accepts_matching_final_face_and_source() {
        let id = "merman-prepared-flowchart-0";
        let fixture = prepared_verifier_fixture(&[id]);
        let verification = verify_prepared_label_expectations(
            fixture.catalog_fingerprint,
            &[prepared_expectation(&fixture, id)],
            &fixture.observed,
            fixture.invalid_token_count,
        );

        assert_eq!(verification.expected_count, 1);
        assert_eq!(verification.verified_count, 1);
        assert_eq!(verification.mismatch_count, 0);
    }

    #[test]
    fn prepared_label_verifier_rejects_a_missing_token() {
        let id = "merman-prepared-flowchart-0";
        let fixture = prepared_verifier_fixture(&[]);
        let verification = verify_prepared_label_expectations(
            fixture.catalog_fingerprint,
            &[prepared_expectation(&fixture, id)],
            &fixture.observed,
            fixture.invalid_token_count,
        );

        assert_eq!(verification.verified_count, 0);
        assert_eq!(verification.mismatch_count, 1);
    }

    #[test]
    fn prepared_label_verifier_rejects_a_duplicate_base_token() {
        let id = "merman-prepared-state-4";
        let fixture = prepared_verifier_fixture(&[id, id]);
        let verification = verify_prepared_label_expectations(
            fixture.catalog_fingerprint,
            &[prepared_expectation(&fixture, id)],
            &fixture.observed,
            fixture.invalid_token_count,
        );

        assert_eq!(verification.verified_count, 0);
        assert_eq!(verification.mismatch_count, 1);
    }

    #[test]
    fn prepared_label_verifier_aggregates_distinct_line_tokens() {
        let base = "merman-prepared-state-4";
        let fixture = prepared_verifier_fixture(&[
            "merman-prepared-state-4-line-0",
            "merman-prepared-state-4-line-1",
        ]);
        let verification = verify_prepared_label_expectations(
            fixture.catalog_fingerprint,
            &[prepared_expectation_with_line_count(&fixture, base, 2)],
            &fixture.observed,
            fixture.invalid_token_count,
        );

        assert_eq!(verification.verified_count, 1);
        assert_eq!(verification.mismatch_count, 0);
    }

    #[test]
    fn prepared_label_verifier_rejects_missing_or_out_of_range_line_tokens() {
        let base = "merman-prepared-state-4";
        let fixture = prepared_verifier_fixture(&[
            "merman-prepared-state-4-line-0",
            "merman-prepared-state-4-line-2",
        ]);
        let verification = verify_prepared_label_expectations(
            fixture.catalog_fingerprint,
            &[prepared_expectation_with_line_count(&fixture, base, 2)],
            &fixture.observed,
            fixture.invalid_token_count,
        );

        assert_eq!(verification.verified_count, 0);
        assert_eq!(verification.mismatch_count, 1);
    }

    #[test]
    fn prepared_label_verifier_rejects_extra_and_malformed_tokens() {
        let expected = "merman-prepared-flowchart-0";
        let fixture = prepared_verifier_fixture(&[
            expected,
            "merman-prepared-flowchart-1",
            "merman-prepared-flowchart-00",
        ]);
        let verification = verify_prepared_label_expectations(
            fixture.catalog_fingerprint,
            &[prepared_expectation(&fixture, expected)],
            &fixture.observed,
            fixture.invalid_token_count,
        );

        assert_eq!(verification.verified_count, 1);
        assert_eq!(verification.mismatch_count, 2);
    }

    #[test]
    fn prepared_label_verifier_rejects_face_source_and_catalog_mismatches() {
        let id = "merman-prepared-sequence-9";
        let fixture = prepared_verifier_fixture(&[id]);
        let mut source_mismatch = prepared_expectation(&fixture, id);
        source_mismatch.faces =
            HashSet::from([ObservedFaceEvidence::Classified(ExportResolvedFace {
                key: fixture.face.key,
                source: FontSource::System,
            })]);
        let mut catalog_mismatch = prepared_expectation(&fixture, id);
        catalog_mismatch.catalog_fingerprint = FontCatalog::default_parity().fingerprint();

        let source_verification = verify_prepared_label_expectations(
            fixture.catalog_fingerprint,
            &[source_mismatch],
            &fixture.observed,
            fixture.invalid_token_count,
        );
        let catalog_verification = verify_prepared_label_expectations(
            fixture.catalog_fingerprint,
            &[catalog_mismatch],
            &fixture.observed,
            fixture.invalid_token_count,
        );

        assert_eq!(source_verification.mismatch_count, 1);
        assert_eq!(catalog_verification.mismatch_count, 1);
    }

    #[test]
    fn default_parity_reuses_the_shared_system_database() {
        let svg = sealed_svg(
            FontCatalog::default_parity(),
            FontSourcePolicy::embedded_then_system(),
        );
        let environment = ExportFontEnvironment::from_svg(&svg).unwrap();

        assert_eq!(environment.plan.source_mode, ExportFontMode::SystemOnly);
        assert!(Arc::ptr_eq(&environment.fontdb, &shared_system_fontdb()));
        assert!(environment.retained_face_ids.is_empty());
    }

    #[test]
    fn embedded_only_database_contains_exactly_the_retained_catalog_faces() {
        let catalog = custom_catalog(&[FontSource::Embedded]);
        let svg = sealed_svg(catalog.clone(), FontSourcePolicy::embedded_only());
        let environment = ExportFontEnvironment::from_resources(
            svg.font_catalog(),
            svg.font_source_policy(),
            || panic!("embedded-only export must not discover system fonts"),
        )
        .unwrap();

        assert_eq!(environment.plan.source_mode, ExportFontMode::EmbeddedOnly);
        assert_eq!(
            environment.plan.loaded_embedded_face_count,
            catalog.faces().len()
        );
        assert_eq!(environment.fontdb.len(), catalog.faces().len());
        assert_eq!(environment.retained_face_ids.len(), catalog.faces().len());
        assert!(
            environment
                .fontdb
                .faces()
                .all(|face| face.families.iter().any(|(name, _)| name == "Excalifont"))
        );
        assert_eq!(environment.default_family, "Excalifont");
    }

    #[test]
    fn source_priority_selects_the_expected_face_for_exact_family_collisions() {
        let catalog = custom_catalog(&[FontSource::Embedded, FontSource::System]);
        let synthetic_system = font_database_from_catalog(&catalog);
        let embedded_first =
            sealed_sized_svg(catalog.clone(), FontSourcePolicy::embedded_then_system());
        let system_first = sealed_sized_svg(catalog, FontSourcePolicy::system_then_embedded());

        let embedded_environment = ExportFontEnvironment::from_resources(
            embedded_first.font_catalog(),
            embedded_first.font_source_policy(),
            || Arc::clone(&synthetic_system),
        )
        .unwrap();
        let system_environment = ExportFontEnvironment::from_resources(
            system_first.font_catalog(),
            system_first.font_source_policy(),
            || Arc::clone(&synthetic_system),
        )
        .unwrap();

        let embedded_plan = resolve_font_plan(&embedded_first, embedded_environment);
        assert_eq!(
            embedded_plan.source_mode(),
            ExportFontMode::EmbeddedThenSystem
        );
        assert!(embedded_plan.used_embedded_fonts());
        assert!(!embedded_plan.used_system_fonts());
        assert!(!embedded_plan.family_fallback_used());

        let system_plan = resolve_font_plan(&system_first, system_environment);
        assert_eq!(
            system_plan.source_mode(),
            ExportFontMode::SystemThenEmbedded
        );
        assert!(!system_plan.used_embedded_fonts());
        assert!(system_plan.used_system_fonts());
        assert!(!system_plan.family_fallback_used());
    }

    #[test]
    fn system_only_policy_does_not_load_custom_catalog_bytes() {
        let catalog = custom_catalog(&[FontSource::Embedded, FontSource::System]);
        let svg = sealed_svg(catalog, FontSourcePolicy::system_only());
        let environment = ExportFontEnvironment::from_svg(&svg).unwrap();

        assert_eq!(environment.plan.source_mode, ExportFontMode::SystemOnly);
        assert_eq!(environment.plan.loaded_embedded_face_count, 0);
        assert!(environment.retained_face_ids.is_empty());
        assert_eq!(environment.fontdb.len(), shared_system_fontdb().len());
    }

    #[test]
    fn unavailable_font_source_intersection_fails_before_usvg_parsing() {
        let catalog = custom_catalog(&[FontSource::Embedded]);
        let svg = sealed_svg(catalog, FontSourcePolicy::system_only());

        assert!(matches!(
            ExportFontEnvironment::from_svg(&svg),
            Err(ExportError::FontSourceUnavailable)
        ));
    }

    #[cfg(feature = "png")]
    #[test]
    fn embedded_only_catalog_drives_real_raster_and_reports_actual_source() {
        let catalog = custom_catalog(&[FontSource::Embedded, FontSource::System]);
        let fingerprint = catalog.fingerprint();
        let svg = sealed_sized_svg(catalog, FontSourcePolicy::embedded_only());
        let prepared = crate::prepare_raster(&svg, &crate::RasterOptions::default()).unwrap();
        let (png, report) = prepared.encode_png_with_report().unwrap();
        let plan = report.fonts();

        assert_eq!(plan.catalog_fingerprint(), fingerprint);
        assert_eq!(plan.source_mode(), ExportFontMode::EmbeddedOnly);
        assert_eq!(plan.loaded_embedded_face_count(), 1);
        assert!(plan.used_embedded_fonts());
        assert!(!plan.used_system_fonts());
        assert!(!plan.family_fallback_used());
        assert!(!plan.unresolved_font_request());
        assert!(!plan.is_host_dependent());

        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[cfg(feature = "png")]
    #[test]
    fn family_and_glyph_fallbacks_are_reported_after_usvg_resolution() {
        let catalog = custom_catalog(&[FontSource::Embedded]);
        let svg = sealed_sized_svg_with_text(
            catalog,
            FontSourcePolicy::embedded_only(),
            "Missing Family",
            "MW你",
        );

        let prepared = crate::prepare_raster(&svg, &crate::RasterOptions::default()).unwrap();
        let (_, report) = prepared.encode_png_with_report().unwrap();
        let plan = report.fonts();

        assert!(plan.used_embedded_fonts());
        assert!(plan.family_fallback_used());
        assert!(!plan.glyph_fallback_used());
        assert!(plan.unresolved_glyph_fallback());
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn embedded_only_catalog_reaches_pdf_from_the_same_sealed_artifact() {
        let catalog = custom_catalog(&[FontSource::Embedded]);
        let fingerprint = catalog.fingerprint();
        let svg = sealed_sized_svg(catalog, FontSourcePolicy::embedded_only());
        let prepared = crate::prepare_pdf(&svg, &crate::PdfOptions::default()).unwrap();
        let report = prepared.report();
        let plan = report.fonts();

        assert_eq!(plan.catalog_fingerprint(), fingerprint);
        assert_eq!(plan.source_mode(), ExportFontMode::EmbeddedOnly);
        assert_eq!(plan.loaded_embedded_face_count(), 1);
        assert!(plan.used_embedded_fonts());
        assert!(!plan.used_system_fonts());
        assert!(!plan.unresolved_font_request());

        let pdf = prepared.encode().unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
    }
}

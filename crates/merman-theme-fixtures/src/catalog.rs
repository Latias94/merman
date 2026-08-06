use crate::corpus::{
    build_reference_themes, convert_translations, translation_index as build_translation_index,
};
use crate::error::CatalogError;
use crate::evidence::{
    load_mermaid_style_precedence, load_modern_mermaid_evidence, parse_evidence_reference,
    validate_excalidraw_font_snapshot,
};
use crate::io::{
    canonical_root, validate_hashed_file, validate_id, validate_relative_path, validate_revision,
    validate_sha256, validate_unicode_range, verified_text,
};
use crate::model::ExpectedThemeCapability;
use crate::model::{
    AssetRecord, EXPECTED_OUTPUT_TARGETS, FIXTURE_EXPECTATION_VERSION, FixtureEvidenceKind,
    FixtureExpectation, FixtureRecord, MANIFEST_RELATIVE_PATH, MermaidStyleFamily,
    ReferenceDiagramFamily, ReferenceThemeMechanism, ReferenceThemeRecord, ReferenceTranslation,
    SCHEMA_VERSION, SourceEvidenceRecord, SourceRecord, StylePrecedenceEvidence,
};
use crate::theme_input::{
    convert_theme_input, parse_fixture_source, validate_fixture_font_contract,
};
use crate::wire::{
    AssetRecordWire, FixtureExpectationWire, FixtureWire, ManifestWire, SourceRecordWire,
    SourceSnapshotHeader,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct ThemeFixtureCatalog {
    root: PathBuf,
    sources: Vec<SourceRecord>,
    assets: Vec<AssetRecord>,
    fixtures: Vec<FixtureRecord>,
    translations: Vec<ReferenceTranslation>,
    themes: Vec<ReferenceThemeRecord>,
    style_precedence: Vec<StylePrecedenceEvidence>,
    source_index: BTreeMap<String, usize>,
    asset_index: BTreeMap<String, usize>,
    fixture_index: BTreeMap<String, usize>,
    theme_reference_index: BTreeMap<String, usize>,
    translation_index: BTreeMap<ReferenceThemeMechanism, usize>,
}

impl ThemeFixtureCatalog {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, CatalogError> {
        let root = root.as_ref();
        let manifest_path = root.join(MANIFEST_RELATIVE_PATH);
        let json = fs::read_to_string(&manifest_path).map_err(|source| CatalogError::ReadFile {
            path: manifest_path,
            source,
        })?;
        Self::from_json(root, &json)
    }

    pub fn from_json(root: impl AsRef<Path>, json: &str) -> Result<Self, CatalogError> {
        let root = canonical_root(root.as_ref())?;
        let wire: ManifestWire = serde_json::from_str(json).map_err(CatalogError::InvalidJson)?;
        if wire.schema_version != SCHEMA_VERSION {
            return Err(CatalogError::UnsupportedSchemaVersion(wire.schema_version));
        }
        validate_global_wire_ids(&wire)?;

        let sources = wire
            .sources
            .into_iter()
            .map(|source| convert_source(&root, source))
            .collect::<Result<Vec<_>, _>>()?;
        let source_index = index_records(&sources, |source| &source.id);

        let assets = wire
            .assets
            .into_iter()
            .map(|asset| convert_asset(&root, asset, &source_index))
            .collect::<Result<Vec<_>, _>>()?;
        let asset_index = index_records(&assets, |asset| &asset.id);
        let assets_by_id = assets
            .iter()
            .map(|asset| (asset.id.clone(), asset))
            .collect::<BTreeMap<_, _>>();

        let translations = convert_translations(wire.mechanism_translations)?;
        let translation_index = build_translation_index(&translations);

        let fixtures = wire
            .fixtures
            .into_iter()
            .map(|fixture| {
                load_fixture(
                    &root,
                    fixture,
                    &source_index,
                    &asset_index,
                    &assets_by_id,
                    &translations,
                    &translation_index,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let fixture_index = index_records(&fixtures, |fixture| &fixture.id);

        let modern_source = required_source(&sources, &source_index, "source-modern-mermaid")?;
        let modern_evidence = load_modern_mermaid_evidence(&root, modern_source)?;

        let mermaid_source = required_source(&sources, &source_index, "source-mermaid")?;
        let style_precedence = load_mermaid_style_precedence(&root, mermaid_source)?;
        validate_fixture_style_evidence(&fixtures, &style_precedence)?;

        let excalidraw_source = required_source(&sources, &source_index, "source-excalidraw")?;
        validate_excalidraw_font_snapshot(&root, excalidraw_source, &assets, &fixtures)?;

        let themes = build_reference_themes(
            wire.themes,
            &modern_source.id,
            &fixture_index,
            &fixtures,
            &modern_evidence,
            &translations,
            &translation_index,
        )?;
        let theme_reference_index = themes
            .iter()
            .enumerate()
            .map(|(index, theme)| (theme.reference_name.clone(), index))
            .collect();

        Ok(Self {
            root,
            sources,
            assets,
            fixtures,
            translations,
            themes,
            style_precedence,
            source_index,
            asset_index,
            fixture_index,
            theme_reference_index,
            translation_index,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn sources(&self) -> impl ExactSizeIterator<Item = &SourceRecord> {
        self.sources.iter()
    }

    pub fn assets(&self) -> impl ExactSizeIterator<Item = &AssetRecord> {
        self.assets.iter()
    }

    pub fn fixtures(&self) -> impl ExactSizeIterator<Item = &FixtureRecord> {
        self.fixtures.iter()
    }

    pub fn themes(&self) -> impl ExactSizeIterator<Item = &ReferenceThemeRecord> {
        self.themes.iter()
    }

    pub fn style_precedence(&self) -> &[StylePrecedenceEvidence] {
        &self.style_precedence
    }

    pub fn source(&self, id: &str) -> Option<&SourceRecord> {
        self.source_index.get(id).map(|index| &self.sources[*index])
    }

    pub fn font_asset(&self, id: &str) -> Option<&AssetRecord> {
        self.asset_index.get(id).map(|index| &self.assets[*index])
    }

    pub fn fixture(&self, id: &str) -> Option<&FixtureRecord> {
        self.fixture_index
            .get(id)
            .map(|index| &self.fixtures[*index])
    }

    pub fn theme(&self, reference_name: &str) -> Option<&ReferenceThemeRecord> {
        self.theme_reference_index
            .get(reference_name)
            .map(|index| &self.themes[*index])
    }

    pub fn translation(&self, mechanism: ReferenceThemeMechanism) -> &ReferenceTranslation {
        &self.translations[self.translation_index[&mechanism]]
    }

    pub fn source_text(&self, fixture_id: &str) -> Result<String, CatalogError> {
        let fixture = self
            .fixture(fixture_id)
            .ok_or_else(|| CatalogError::UnknownLookup(fixture_id.to_string()))?;
        verified_text(validate_hashed_file(
            &self.root,
            &fixture.id,
            &fixture.source_path,
            &fixture.source_sha256,
        )?)
    }

    pub fn asset_bytes(&self, asset_id: &str) -> Result<Vec<u8>, CatalogError> {
        let asset = self
            .font_asset(asset_id)
            .ok_or_else(|| CatalogError::UnknownLookup(asset_id.to_string()))?;
        Ok(validate_hashed_file(&self.root, &asset.id, &asset.path, &asset.sha256)?.bytes)
    }

    pub fn source_snapshot_text(&self, source_id: &str) -> Result<String, CatalogError> {
        let source = self
            .source(source_id)
            .ok_or_else(|| CatalogError::UnknownLookup(source_id.to_string()))?;
        verified_text(validate_hashed_file(
            &self.root,
            &source.id,
            &source.snapshot_path,
            &source.snapshot_sha256,
        )?)
    }

    pub fn verify_source_checkout(
        &self,
        source_id: &str,
        revision: &str,
        checkout_root: impl AsRef<Path>,
    ) -> Result<(), CatalogError> {
        let source = self
            .source(source_id)
            .ok_or_else(|| CatalogError::UnknownLookup(source_id.to_string()))?;
        if revision != source.revision {
            return Err(CatalogError::SourceCheckoutRevisionMismatch {
                source_id: source.id.clone(),
                expected: source.revision.clone(),
                actual: revision.to_string(),
            });
        }
        let root = canonical_root(checkout_root.as_ref())?;
        validate_hashed_file(
            &root,
            &source.id,
            &source.upstream_license_path,
            &source.license_sha256,
        )?;
        let mut evidence_line_counts = BTreeMap::new();
        let mut modern_theme_source_line_count = None;
        for evidence in &source.evidence {
            let verified =
                validate_hashed_file(&root, &source.id, &evidence.source_path, &evidence.sha256)?;
            if source.id == "source-mermaid" {
                evidence_line_counts.insert(
                    evidence.source_path.as_str(),
                    verified_text(verified)?.lines().count(),
                );
            } else if source.id == "source-modern-mermaid"
                && evidence.source_path == "src/utils/themes.ts"
            {
                modern_theme_source_line_count = Some(verified_text(verified)?.lines().count());
            }
        }
        if source.id == "source-mermaid" {
            for reference in self
                .style_precedence
                .iter()
                .flat_map(|entry| entry.evidence.iter())
            {
                let (path, ranges) = parse_evidence_reference(reference)
                    .expect("style evidence references are validated during catalog loading");
                let line_count = evidence_line_counts[path];
                if ranges.iter().any(|(_, end)| *end > line_count) {
                    return Err(CatalogError::MermaidStyleEvidenceRangeOutOfBounds {
                        reference: reference.clone(),
                        line_count,
                    });
                }
            }
        }
        if source.id == "source-modern-mermaid" {
            let line_count = modern_theme_source_line_count
                .ok_or(CatalogError::ModernMermaidSnapshotMismatch)?;
            for theme in self
                .themes
                .iter()
                .filter(|theme| theme.source_id == source.id)
            {
                if usize::try_from(theme.source_line).ok() > Some(line_count) {
                    return Err(CatalogError::ModernMermaidThemeSourceLineMismatch {
                        theme: theme.reference_name.clone(),
                        source_line: theme.source_line,
                    });
                }
            }
        }
        for asset in self
            .assets
            .iter()
            .filter(|asset| asset.source_id == source.id)
        {
            validate_hashed_file(&root, &asset.id, &asset.source_path, &asset.source_sha256)?;
        }
        Ok(())
    }
}

fn validate_global_wire_ids(wire: &ManifestWire) -> Result<(), CatalogError> {
    let mut ids = BTreeMap::<&str, &'static str>::new();
    for (kind, values) in [
        (
            "source",
            wire.sources
                .iter()
                .map(|value| value.id.as_str())
                .collect::<Vec<_>>(),
        ),
        (
            "asset",
            wire.assets
                .iter()
                .map(|value| value.id.as_str())
                .collect::<Vec<_>>(),
        ),
        (
            "fixture",
            wire.fixtures
                .iter()
                .map(|value| value.id.as_str())
                .collect::<Vec<_>>(),
        ),
        (
            "theme",
            wire.themes
                .iter()
                .map(|value| value.id.as_str())
                .collect::<Vec<_>>(),
        ),
    ] {
        for id in values {
            validate_id(id)?;
            if let Some(first_kind) = ids.insert(id, kind) {
                return Err(CatalogError::DuplicateId {
                    id: id.to_string(),
                    first_kind,
                    second_kind: kind,
                });
            }
        }
    }
    Ok(())
}

fn convert_source(root: &Path, wire: SourceRecordWire) -> Result<SourceRecord, CatalogError> {
    let source = SourceRecord {
        id: wire.id,
        project: wire.project,
        revision: wire.revision,
        upstream_license_path: wire.upstream_license_path,
        license_path: wire.license_path,
        license_sha256: wire.license_sha256,
        snapshot_path: wire.snapshot_path,
        snapshot_sha256: wire.snapshot_sha256,
        evidence: wire
            .evidence
            .into_iter()
            .map(|evidence| SourceEvidenceRecord {
                source_path: evidence.source_path,
                sha256: evidence.sha256,
            })
            .collect(),
    };
    validate_source(root, &source)?;
    Ok(source)
}

fn validate_source(root: &Path, source: &SourceRecord) -> Result<(), CatalogError> {
    if source.project.trim().is_empty() || source.evidence.is_empty() {
        return Err(CatalogError::IncompleteSource(source.id.clone()));
    }
    validate_revision(&source.id, &source.revision)?;
    validate_relative_path(&source.upstream_license_path, "upstream license")?;
    validate_hashed_file(
        root,
        &source.id,
        &source.license_path,
        &source.license_sha256,
    )?;
    let snapshot_json = verified_text(validate_hashed_file(
        root,
        &source.id,
        &source.snapshot_path,
        &source.snapshot_sha256,
    )?)?;
    for evidence in &source.evidence {
        validate_relative_path(&evidence.source_path, "source evidence")?;
        validate_sha256(&source.id, &evidence.sha256)?;
    }
    let snapshot: SourceSnapshotHeader = serde_json::from_str(&snapshot_json).map_err(|error| {
        CatalogError::InvalidSourceSnapshot {
            source_id: source.id.clone(),
            error,
        }
    })?;
    let expected_evidence = source
        .evidence
        .iter()
        .map(|record| (record.source_path.as_str(), record.sha256.as_str()))
        .collect::<BTreeSet<_>>();
    let actual_evidence = snapshot
        .evidence_files
        .iter()
        .map(|record| (record.path.as_str(), record.sha256.as_str()))
        .collect::<BTreeSet<_>>();
    if snapshot.snapshot_version == 0
        || snapshot.source_id != source.id
        || snapshot.revision != source.revision
        || actual_evidence != expected_evidence
    {
        return Err(CatalogError::SourceSnapshotMismatch(source.id.clone()));
    }
    Ok(())
}

fn convert_asset(
    root: &Path,
    wire: AssetRecordWire,
    source_index: &BTreeMap<String, usize>,
) -> Result<AssetRecord, CatalogError> {
    let asset = AssetRecord {
        id: wire.id,
        path: wire.path,
        sha256: wire.sha256,
        media_type: wire.media_type,
        canonical_family: wire.canonical_family,
        family: wire.family,
        weight: wire.weight,
        style: wire.style,
        unicode_range: wire.unicode_range,
        source_id: wire.source_id,
        source_path: wire.source_path,
        source_sha256: wire.source_sha256,
        license_path: wire.license_path,
        license_sha256: wire.license_sha256,
        notice_path: wire.notice_path,
        notice_sha256: wire.notice_sha256,
    };
    validate_asset(root, &asset, source_index)?;
    Ok(asset)
}

fn validate_asset(
    root: &Path,
    asset: &AssetRecord,
    source_index: &BTreeMap<String, usize>,
) -> Result<(), CatalogError> {
    require_reference(source_index, &asset.id, "sourceId", &asset.source_id)?;
    validate_relative_path(&asset.source_path, "asset provenance")?;
    validate_sha256(&asset.id, &asset.source_sha256)?;
    validate_hashed_file(root, &asset.id, &asset.license_path, &asset.license_sha256)?;
    validate_hashed_file(root, &asset.id, &asset.notice_path, &asset.notice_sha256)?;
    if asset.canonical_family.trim().is_empty()
        || asset.family.trim().is_empty()
        || asset.style.trim().is_empty()
        || asset.unicode_range.trim().is_empty()
        || asset.weight == 0
    {
        return Err(CatalogError::InvalidAssetMetadata(asset.id.clone()));
    }
    validate_unicode_range(&asset.id, &asset.unicode_range)?;
    validate_hashed_file(root, &asset.id, &asset.path, &asset.sha256)?;
    if asset.sha256 != asset.source_sha256 {
        return Err(CatalogError::AssetSourceHashMismatch(asset.id.clone()));
    }
    Ok(())
}

fn load_fixture(
    root: &Path,
    wire: FixtureWire,
    source_index: &BTreeMap<String, usize>,
    asset_index: &BTreeMap<String, usize>,
    assets_by_id: &BTreeMap<String, &AssetRecord>,
    translations: &[ReferenceTranslation],
    translation_index: &BTreeMap<ReferenceThemeMechanism, usize>,
) -> Result<FixtureRecord, CatalogError> {
    if wire.evidence_source_ids.is_empty() {
        return Err(CatalogError::IncompleteFixture(wire.id));
    }
    for source_id in &wire.evidence_source_ids {
        require_reference(source_index, &wire.id, "evidenceSourceIds", source_id)?;
    }
    for asset_id in &wire.asset_ids {
        require_reference(asset_index, &wire.id, "assetIds", asset_id)?;
    }
    let source_text = verified_text(validate_hashed_file(
        root,
        &wire.id,
        &wire.source_path,
        &wire.source_sha256,
    )?)?;
    let expectation_json = verified_text(validate_hashed_file(
        root,
        &wire.id,
        &wire.expectation_path,
        &wire.expectation_sha256,
    )?)?;
    let expectation_wire: FixtureExpectationWire = serde_json::from_str(&expectation_json)
        .map_err(|source| CatalogError::InvalidFixtureExpectation {
            fixture: wire.id.clone(),
            source,
        })?;
    if expectation_wire.fixture_expectation_version != FIXTURE_EXPECTATION_VERSION
        || expectation_wire.fixture_id != wire.id
        || expectation_wire.intent.trim().is_empty()
    {
        return Err(CatalogError::FixtureExpectationMismatch(wire.id));
    }
    let parsed_source = parse_fixture_source(&wire.id, &source_text)?;

    let (expectation, theme_input) = match expectation_wire.evidence_kind {
        FixtureEvidenceKind::SourceCompatibility => {
            if expectation_wire.reference_mechanisms.is_empty()
                && expectation_wire.style_evidence_ids.is_empty()
            {
                return Err(CatalogError::FixtureExpectationMismatch(wire.id));
            }
            if wire.theme_input_path.is_some() || wire.theme_input_sha256.is_some() {
                return Err(CatalogError::ThemeInputMismatch(wire.id));
            }
            if expectation_wire.reference_mechanisms != parsed_source.reference_mechanisms {
                return Err(CatalogError::SourceCompatibilityMechanismMismatch {
                    fixture: wire.id,
                    expected: expectation_wire.reference_mechanisms,
                    actual: parsed_source.reference_mechanisms,
                });
            }
            (
                FixtureExpectation {
                    evidence_kind: FixtureEvidenceKind::SourceCompatibility,
                    intent: expectation_wire.intent,
                    reference_mechanisms: expectation_wire.reference_mechanisms,
                    capabilities: BTreeSet::new(),
                    outputs: BTreeSet::new(),
                    target_capabilities: BTreeMap::new(),
                    visible_text: parsed_source.visible_text.clone(),
                    style_evidence_ids: expectation_wire.style_evidence_ids,
                },
                None,
            )
        }
        FixtureEvidenceKind::TypedCapability => {
            if !expectation_wire.reference_mechanisms.is_empty()
                || !expectation_wire.style_evidence_ids.is_empty()
            {
                return Err(CatalogError::FixtureExpectationMismatch(wire.id));
            }
            if !parsed_source.reference_mechanisms.is_empty()
                || !parsed_source.style_evidence_ids.is_empty()
            {
                return Err(CatalogError::TypedFixtureContainsSourceEvidence {
                    fixture: wire.id,
                    mechanisms: parsed_source.reference_mechanisms,
                    style_evidence_ids: parsed_source.style_evidence_ids,
                });
            }
            let (input_path, input_sha256) = match (
                wire.theme_input_path.as_deref(),
                wire.theme_input_sha256.as_deref(),
            ) {
                (Some(path), Some(sha256)) => (path, sha256),
                _ => return Err(CatalogError::IncompleteThemeInputReference(wire.id)),
            };
            let input_json = verified_text(validate_hashed_file(
                root,
                &wire.id,
                input_path,
                input_sha256,
            )?)?;
            let input_wire = serde_json::from_str(&input_json).map_err(|source| {
                CatalogError::InvalidThemeInput {
                    fixture: wire.id.clone(),
                    source,
                }
            })?;
            let input = convert_theme_input(&wire.id, input_wire, parsed_source.family)?;
            validate_fixture_font_contract(
                &wire.id,
                &input,
                &wire.asset_ids,
                assets_by_id,
                &parsed_source.visible_text,
            )?;
            let mechanisms = input.mechanisms();
            let (outputs, target_capabilities, capabilities) =
                derive_fixture_targets(&mechanisms, translations, translation_index);
            (
                FixtureExpectation {
                    evidence_kind: FixtureEvidenceKind::TypedCapability,
                    intent: expectation_wire.intent,
                    reference_mechanisms: mechanisms,
                    capabilities,
                    outputs,
                    target_capabilities,
                    visible_text: parsed_source.visible_text,
                    style_evidence_ids: BTreeSet::new(),
                },
                Some(input),
            )
        }
    };
    Ok(FixtureRecord {
        id: wire.id,
        source_path: wire.source_path,
        source_sha256: wire.source_sha256,
        expectation_sha256: wire.expectation_sha256,
        theme_input_sha256: wire.theme_input_sha256,
        evidence_source_ids: wire.evidence_source_ids,
        asset_ids: wire.asset_ids,
        source_family: parsed_source.family,
        source_reference_mechanisms: parsed_source.reference_mechanisms,
        source_style_evidence_ids: parsed_source.style_evidence_ids,
        expectation,
        theme_input,
    })
}

fn derive_fixture_targets(
    mechanisms: &BTreeSet<ReferenceThemeMechanism>,
    translations: &[ReferenceTranslation],
    translation_index: &BTreeMap<ReferenceThemeMechanism, usize>,
) -> (
    BTreeSet<crate::model::ExpectedOutputTarget>,
    BTreeMap<crate::model::ExpectedOutputTarget, BTreeSet<ExpectedThemeCapability>>,
    BTreeSet<ExpectedThemeCapability>,
) {
    let outputs = EXPECTED_OUTPUT_TARGETS.into_iter().collect::<BTreeSet<_>>();
    let mut union = BTreeSet::new();
    let target_capabilities = EXPECTED_OUTPUT_TARGETS
        .into_iter()
        .map(|target| {
            let mut capabilities = BTreeSet::new();
            for mechanism in mechanisms {
                let translation = &translations[translation_index[mechanism]];
                let (disposition, translated) = translation.effective_for(target);
                if disposition == crate::model::ReferenceMechanismDisposition::Capability {
                    capabilities.extend(translated.iter().copied());
                }
            }
            union.extend(capabilities.iter().copied());
            (target, capabilities)
        })
        .collect();
    (outputs, target_capabilities, union)
}

fn validate_fixture_style_evidence(
    fixtures: &[FixtureRecord],
    style_precedence: &[StylePrecedenceEvidence],
) -> Result<(), CatalogError> {
    let known = style_precedence
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    for fixture in fixtures {
        for id in &fixture.expectation.style_evidence_ids {
            let Some(entry) = known.get(id.as_str()) else {
                return Err(CatalogError::UnknownStyleEvidence {
                    fixture: fixture.id.clone(),
                    id: id.clone(),
                });
            };
            if !style_family_matches_fixture(entry.family, fixture.source_family) {
                return Err(CatalogError::StyleEvidenceFamilyMismatch {
                    fixture: fixture.id.clone(),
                    id: id.clone(),
                    source_family: fixture.source_family,
                    evidence_family: entry.family,
                });
            }
        }
        if fixture.expectation.evidence_kind == FixtureEvidenceKind::SourceCompatibility
            && fixture.expectation.style_evidence_ids != fixture.source_style_evidence_ids
        {
            return Err(CatalogError::SourceStyleEvidenceMismatch {
                fixture: fixture.id.clone(),
                expected: fixture.expectation.style_evidence_ids.clone(),
                actual: fixture.source_style_evidence_ids.clone(),
            });
        }
    }
    let consumed = fixtures
        .iter()
        .filter(|fixture| {
            fixture.expectation.evidence_kind == FixtureEvidenceKind::SourceCompatibility
        })
        .flat_map(|fixture| fixture.expectation.style_evidence_ids.iter())
        .collect::<BTreeSet<_>>();
    for entry in style_precedence {
        if !consumed.contains(&entry.id) {
            return Err(CatalogError::UnconsumedStyleEvidence(entry.id.clone()));
        }
    }
    Ok(())
}

fn style_family_matches_fixture(
    evidence: MermaidStyleFamily,
    source: ReferenceDiagramFamily,
) -> bool {
    evidence == MermaidStyleFamily::Global
        || matches!(
            (evidence, source),
            (
                MermaidStyleFamily::Flowchart,
                ReferenceDiagramFamily::Flowchart
            ) | (
                MermaidStyleFamily::StateDiagram,
                ReferenceDiagramFamily::StateDiagram
            ) | (
                MermaidStyleFamily::ClassDiagram,
                ReferenceDiagramFamily::ClassDiagram
            ) | (
                MermaidStyleFamily::ErDiagram,
                ReferenceDiagramFamily::ErDiagram
            )
        )
}

fn index_records<T>(values: &[T], id: impl Fn(&T) -> &String) -> BTreeMap<String, usize> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| (id(value).clone(), index))
        .collect()
}

fn required_source<'a>(
    sources: &'a [SourceRecord],
    source_index: &BTreeMap<String, usize>,
    id: &str,
) -> Result<&'a SourceRecord, CatalogError> {
    source_index
        .get(id)
        .map(|index| &sources[*index])
        .ok_or_else(|| CatalogError::MissingRequiredSource(id.to_string()))
}

fn require_reference(
    index: &BTreeMap<String, usize>,
    record: &str,
    field: &'static str,
    target: &str,
) -> Result<(), CatalogError> {
    if index.contains_key(target) {
        Ok(())
    } else {
        Err(CatalogError::UnknownReference {
            record: record.to_string(),
            field,
            target: target.to_string(),
        })
    }
}

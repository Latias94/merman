use crate::error::CatalogError;
use crate::evidence::ModernThemeEvidence;
use crate::io::validate_id;
use crate::model::{
    EXPECTED_OUTPUT_TARGETS, ExpectedOutputTarget, ExpectedPortabilityGrade, FixtureEvidenceKind,
    FixtureRecord, MODERN_MERMAID_REFERENCE_THEME_COUNT, MODERN_MERMAID_REFERENCE_THEMES,
    REFERENCE_THEME_MECHANISMS, ReferenceMechanismDisposition, ReferenceThemeMechanism,
    ReferenceThemeRecord, ReferenceTranslation, ResidualRecord, TargetExpectation,
    TargetTranslation,
};
use crate::wire::{
    ReferenceThemeWire, ReferenceTranslationWire, ResidualWire, TargetPolicyEntryWire,
    TargetTranslationWire,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn convert_translations(
    wires: Vec<ReferenceTranslationWire>,
) -> Result<Vec<ReferenceTranslation>, CatalogError> {
    let mut translations = Vec::with_capacity(wires.len());
    let mut actual = BTreeSet::new();
    for wire in wires {
        if !actual.insert(wire.source) {
            return Err(CatalogError::DuplicateTranslation(wire.source));
        }
        validate_translation_shape(wire.source, wire.disposition, &wire.capabilities)?;
        let mut target_overrides = BTreeMap::new();
        for (target, override_wire) in wire.target_overrides {
            validate_translation_shape(
                wire.source,
                override_wire.disposition,
                &override_wire.capabilities,
            )?;
            target_overrides.insert(target, convert_target_translation(override_wire));
        }
        translations.push(ReferenceTranslation {
            source: wire.source,
            disposition: wire.disposition,
            capabilities: wire.capabilities,
            target_overrides,
        });
    }
    let expected = REFERENCE_THEME_MECHANISMS
        .into_iter()
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(CatalogError::TranslationSetMismatch { expected, actual });
    }
    Ok(translations)
}

fn convert_target_translation(wire: TargetTranslationWire) -> TargetTranslation {
    TargetTranslation {
        disposition: wire.disposition,
        capabilities: wire.capabilities,
    }
}

fn validate_translation_shape(
    source: ReferenceThemeMechanism,
    disposition: ReferenceMechanismDisposition,
    capabilities: &BTreeSet<crate::model::ExpectedThemeCapability>,
) -> Result<(), CatalogError> {
    match disposition {
        ReferenceMechanismDisposition::Capability if capabilities.is_empty() => {
            Err(CatalogError::InvalidTranslation(source))
        }
        ReferenceMechanismDisposition::Residual if !capabilities.is_empty() => {
            Err(CatalogError::InvalidTranslation(source))
        }
        _ => Ok(()),
    }
}

pub(crate) fn translation_index(
    translations: &[ReferenceTranslation],
) -> BTreeMap<ReferenceThemeMechanism, usize> {
    translations
        .iter()
        .enumerate()
        .map(|(index, translation)| (translation.source, index))
        .collect()
}

pub(crate) fn build_reference_themes(
    wires: Vec<ReferenceThemeWire>,
    modern_source_id: &str,
    fixture_index: &BTreeMap<String, usize>,
    fixtures: &[FixtureRecord],
    modern_evidence: &BTreeMap<String, ModernThemeEvidence>,
    translations: &[ReferenceTranslation],
    translation_index: &BTreeMap<ReferenceThemeMechanism, usize>,
) -> Result<Vec<ReferenceThemeRecord>, CatalogError> {
    validate_reference_theme_wire_set(&wires)?;
    let mut themes = Vec::with_capacity(wires.len());
    for wire in wires {
        validate_id(&wire.id)?;
        if wire.source_id != modern_source_id || wire.fixture_ids.is_empty() {
            return Err(CatalogError::IncompleteTheme(wire.id));
        }
        let source_evidence = modern_evidence
            .get(&wire.reference_name)
            .ok_or(CatalogError::ModernMermaidSnapshotMismatch)?;
        let evidence = wire
            .fixture_ids
            .iter()
            .map(|fixture_id| {
                fixture_index
                    .get(fixture_id)
                    .map(|index| &fixtures[*index])
                    .ok_or_else(|| CatalogError::UnknownReference {
                        record: wire.id.clone(),
                        field: "fixtureIds",
                        target: fixture_id.clone(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if evidence.iter().any(|fixture| {
            fixture.expectation.evidence_kind != FixtureEvidenceKind::TypedCapability
                || !fixture.evidence_source_ids.contains(&wire.source_id)
                || fixture
                    .expectation
                    .reference_mechanisms
                    .is_disjoint(&source_evidence.mechanisms)
        }) {
            return Err(CatalogError::IncompleteTheme(wire.id));
        }
        validate_source_evidence(&wire.id, source_evidence, &evidence)?;

        let mut overrides = wire.target_policy.overrides;
        let mut targets = Vec::with_capacity(EXPECTED_OUTPUT_TARGETS.len());
        for target in EXPECTED_OUTPUT_TARGETS {
            let policy = overrides
                .remove(&target)
                .unwrap_or_else(|| wire.target_policy.default.clone());
            targets.push(build_target_expectation(
                &wire.id,
                target,
                policy,
                &source_evidence.mechanisms,
                &evidence,
                translations,
                translation_index,
            )?);
        }
        debug_assert!(overrides.is_empty());
        themes.push(ReferenceThemeRecord {
            id: wire.id,
            reference_name: wire.reference_name,
            source_id: wire.source_id,
            source_line: source_evidence.source_line,
            canvas_layer_count: source_evidence.canvas_layer_count,
            source_mechanisms: source_evidence.mechanisms.clone(),
            source_facets: source_evidence.facets.clone(),
            fixture_ids: wire.fixture_ids,
            targets,
        });
    }
    validate_portability_baseline(&themes)?;
    Ok(themes)
}

fn validate_reference_theme_wire_set(wires: &[ReferenceThemeWire]) -> Result<(), CatalogError> {
    let actual = wires
        .iter()
        .map(|theme| theme.reference_name.as_str())
        .collect::<BTreeSet<_>>();
    let expected = MODERN_MERMAID_REFERENCE_THEMES
        .into_iter()
        .collect::<BTreeSet<_>>();
    if wires.len() != MODERN_MERMAID_REFERENCE_THEME_COUNT || actual != expected {
        return Err(CatalogError::ReferenceThemeSetMismatch {
            expected: expected.into_iter().map(str::to_string).collect(),
            actual: actual.into_iter().map(str::to_string).collect(),
        });
    }
    Ok(())
}

fn validate_source_evidence(
    theme_id: &str,
    source: &ModernThemeEvidence,
    evidence: &[&FixtureRecord],
) -> Result<(), CatalogError> {
    for mechanism in &source.mechanisms {
        if !evidence
            .iter()
            .any(|fixture| fixture.expectation.reference_mechanisms.contains(mechanism))
        {
            return Err(CatalogError::UncoveredSourceMechanism {
                theme: theme_id.to_string(),
                mechanism: *mechanism,
            });
        }
    }
    for facet in &source.facets {
        if !evidence.iter().any(|fixture| {
            fixture
                .theme_input
                .as_ref()
                .is_some_and(|input| input.facets().contains(facet))
        }) {
            return Err(CatalogError::UncoveredSourceFacet {
                theme: theme_id.to_string(),
                facet: *facet,
            });
        }
    }
    Ok(())
}

fn build_target_expectation(
    theme_id: &str,
    target: ExpectedOutputTarget,
    policy: TargetPolicyEntryWire,
    mechanisms: &BTreeSet<ReferenceThemeMechanism>,
    evidence: &[&FixtureRecord],
    translations: &[ReferenceTranslation],
    translation_index: &BTreeMap<ReferenceThemeMechanism, usize>,
) -> Result<TargetExpectation, CatalogError> {
    let mut capabilities = BTreeSet::new();
    let mut required_residuals = BTreeSet::new();
    for mechanism in mechanisms {
        let translation = &translations[translation_index[mechanism]];
        let (disposition, translated) = translation.effective_for(target);
        if !evidence.iter().any(|fixture| {
            fixture.expectation.outputs.contains(&target)
                && fixture.expectation.reference_mechanisms.contains(mechanism)
        }) {
            return Err(CatalogError::UncoveredTargetMechanism {
                theme: theme_id.to_string(),
                target,
                mechanism: *mechanism,
            });
        }
        match disposition {
            ReferenceMechanismDisposition::Capability => {
                capabilities.extend(translated.iter().copied());
            }
            ReferenceMechanismDisposition::Residual => {
                required_residuals.insert(*mechanism);
            }
        }
    }
    if capabilities.is_empty() {
        return Err(CatalogError::IncompleteTarget {
            theme: theme_id.to_string(),
            target,
        });
    }
    let residuals = convert_residuals(
        theme_id,
        target,
        policy.residuals,
        mechanisms,
        translations,
        translation_index,
    )?;
    let actual_residuals = residuals
        .iter()
        .map(|residual| residual.source_mechanism)
        .collect::<BTreeSet<_>>();
    if actual_residuals != required_residuals {
        return Err(CatalogError::TargetResidualSetMismatch {
            theme: theme_id.to_string(),
            target,
            expected: required_residuals,
            actual: actual_residuals,
        });
    }
    if policy.grade == ExpectedPortabilityGrade::Portable && !residuals.is_empty() {
        return Err(CatalogError::PortableTargetHasResidual {
            theme: theme_id.to_string(),
            target,
        });
    }
    if policy.grade == ExpectedPortabilityGrade::Unverified && residuals.is_empty() {
        return Err(CatalogError::UnverifiedTargetMissingResidual {
            theme: theme_id.to_string(),
            target,
        });
    }
    Ok(TargetExpectation {
        target,
        grade: policy.grade,
        capabilities,
        residuals,
    })
}

fn convert_residuals(
    theme_id: &str,
    target: ExpectedOutputTarget,
    wires: Vec<ResidualWire>,
    mechanisms: &BTreeSet<ReferenceThemeMechanism>,
    translations: &[ReferenceTranslation],
    translation_index: &BTreeMap<ReferenceThemeMechanism, usize>,
) -> Result<Vec<ResidualRecord>, CatalogError> {
    let mut ids = BTreeSet::new();
    let mut residuals = Vec::with_capacity(wires.len());
    for wire in wires {
        validate_id(&wire.id)?;
        if !ids.insert(wire.id.clone())
            || wire.reason.trim().is_empty()
            || !mechanisms.contains(&wire.source_mechanism)
            || translations[translation_index[&wire.source_mechanism]]
                .effective_for(target)
                .0
                != ReferenceMechanismDisposition::Residual
        {
            return Err(CatalogError::InvalidTargetResidual {
                theme: theme_id.to_string(),
                target,
                residual: wire.id,
            });
        }
        residuals.push(ResidualRecord {
            id: wire.id,
            source_mechanism: wire.source_mechanism,
            reason: wire.reason,
        });
    }
    Ok(residuals)
}

fn validate_portability_baseline(themes: &[ReferenceThemeRecord]) -> Result<(), CatalogError> {
    let aurora = themes
        .iter()
        .find(|theme| theme.reference_name == "aurora")
        .expect("validated reference set");
    validate_aurora(aurora)?;
    for theme in themes
        .iter()
        .filter(|theme| theme.reference_name != "aurora")
    {
        for target in &theme.targets {
            if target.grade != ExpectedPortabilityGrade::Portable || !target.residuals.is_empty() {
                return Err(CatalogError::UnexpectedReferenceThemePortability {
                    theme: theme.reference_name.clone(),
                    target: target.target,
                    grade: target.grade,
                });
            }
        }
    }
    Ok(())
}

fn validate_aurora(theme: &ReferenceThemeRecord) -> Result<(), CatalogError> {
    if !theme
        .source_mechanisms
        .contains(&ReferenceThemeMechanism::BackdropFilter)
        || theme.targets.iter().any(|target| {
            target.grade != ExpectedPortabilityGrade::Unverified
                || !target.residuals.iter().any(|residual| {
                    residual.id == "browser-backdrop-blur"
                        && residual.source_mechanism == ReferenceThemeMechanism::BackdropFilter
                })
        })
    {
        return Err(CatalogError::MissingAuroraBackdropResidual);
    }
    Ok(())
}

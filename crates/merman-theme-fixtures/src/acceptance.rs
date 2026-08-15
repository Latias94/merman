use crate::acceptance_wire::{
    C6AcceptanceCatalogWire, C6CellEnforcementWire, C6CellExpectationWire,
};
use crate::io::validate_id;
use crate::{
    CatalogError, ExpectedOutputTarget, ExpectedPortabilityGrade, ReferenceDiagramFamily,
    ReferenceThemeMechanism, ThemeFixtureCatalog,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

pub const C6_ACCEPTANCE_RELATIVE_PATH: &str = "acceptance/c6-v3.json";
pub const C6_ACCEPTANCE_SCHEMA_VERSION: u32 = 3;
pub const C6_ACCEPTANCE_CELL_COUNT: usize = 18;

pub const C6_NATIVE_OUTPUT_TARGETS: [ExpectedOutputTarget; 2] = [
    ExpectedOutputTarget::StandaloneSvg,
    ExpectedOutputTarget::Png,
];

pub const C6_PROOF_THEMES: [C6ProofTheme; 3] = [
    C6ProofTheme::Brutalist,
    C6ProofTheme::Spotless,
    C6ProofTheme::Cyberpunk,
];

pub const C6_PROOF_FAMILIES: [C6ProofFamily; 3] = [
    C6ProofFamily::Flowchart,
    C6ProofFamily::State,
    C6ProofFamily::Sequence,
];

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum C6ProofTheme {
    Brutalist,
    Cyberpunk,
    Spotless,
}

impl C6ProofTheme {
    pub const fn reference_name(self) -> &'static str {
        match self {
            Self::Brutalist => "brutalist",
            Self::Cyberpunk => "cyberpunk",
            Self::Spotless => "spotless",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum C6ProofFamily {
    Flowchart,
    Sequence,
    State,
}

impl C6ProofFamily {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Flowchart => "flowchart",
            Self::Sequence => "sequence",
            Self::State => "state",
        }
    }

    const fn fixture_family(self) -> ReferenceDiagramFamily {
        match self {
            Self::Flowchart => ReferenceDiagramFamily::Flowchart,
            Self::Sequence => ReferenceDiagramFamily::Sequence,
            Self::State => ReferenceDiagramFamily::StateDiagram,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum C6ExpectedMechanismDisposition {
    MustApply,
    MustRemainResidual,
    MustReject,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum C6ReadinessBlocker {
    FamilyAdapterIncomplete,
    ThemeSliceIncomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct C6CellKey {
    theme: C6ProofTheme,
    family: C6ProofFamily,
    target: ExpectedOutputTarget,
}

impl C6CellKey {
    pub const fn new(
        theme: C6ProofTheme,
        family: C6ProofFamily,
        target: ExpectedOutputTarget,
    ) -> Self {
        Self {
            theme,
            family,
            target,
        }
    }

    pub const fn theme(self) -> C6ProofTheme {
        self.theme
    }

    pub const fn family(self) -> C6ProofFamily {
        self.family
    }

    pub const fn target(self) -> ExpectedOutputTarget {
        self.target
    }

    fn label(self) -> String {
        format!(
            "{}/{}/{:?}",
            self.theme.reference_name(),
            self.family.as_str(),
            self.target
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C6CellExpectation {
    mechanism_requirements: BTreeMap<ReferenceThemeMechanism, C6ExpectedMechanismDisposition>,
    expected_residual_ids: BTreeSet<String>,
    semantic_assertion_id: String,
}

impl C6CellExpectation {
    pub fn mechanism_requirements(
        &self,
    ) -> &BTreeMap<ReferenceThemeMechanism, C6ExpectedMechanismDisposition> {
        &self.mechanism_requirements
    }

    pub fn expected_residual_ids(&self) -> &BTreeSet<String> {
        &self.expected_residual_ids
    }

    pub fn semantic_assertion_id(&self) -> &str {
        &self.semantic_assertion_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C6AcceptanceCell {
    key: C6CellKey,
    expectation: C6CellExpectation,
}

impl C6AcceptanceCell {
    pub const fn key(&self) -> C6CellKey {
        self.key
    }

    pub const fn expectation(&self) -> &C6CellExpectation {
        &self.expectation
    }
}

#[derive(Clone, Debug)]
pub struct C6AcceptanceSpec {
    cells: Vec<C6AcceptanceCell>,
    cell_index: BTreeMap<C6CellKey, usize>,
}

impl C6AcceptanceSpec {
    pub fn cells(&self) -> impl ExactSizeIterator<Item = &C6AcceptanceCell> {
        self.cells.iter()
    }

    pub fn cell(&self, key: C6CellKey) -> Option<&C6AcceptanceCell> {
        self.cell_index.get(&key).map(|index| &self.cells[*index])
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C6EnforcedCell {
    key: C6CellKey,
    source_fixture_id: String,
}

impl C6EnforcedCell {
    pub const fn key(&self) -> C6CellKey {
        self.key
    }

    pub fn source_fixture_id(&self) -> &str {
        &self.source_fixture_id
    }
}

#[derive(Clone, Debug)]
pub struct C6EnforcedTranche {
    cells: Vec<C6EnforcedCell>,
    cell_index: BTreeMap<C6CellKey, usize>,
    deferred: BTreeMap<C6CellKey, C6ReadinessBlocker>,
}

impl C6EnforcedTranche {
    pub fn cells(&self) -> impl ExactSizeIterator<Item = &C6EnforcedCell> {
        self.cells.iter()
    }

    pub fn cell(&self, key: C6CellKey) -> Option<&C6EnforcedCell> {
        self.cell_index.get(&key).map(|index| &self.cells[*index])
    }

    pub fn readiness_blocker(&self, key: C6CellKey) -> Option<C6ReadinessBlocker> {
        self.deferred.get(&key).copied()
    }

    pub fn deferred_cells(
        &self,
    ) -> impl ExactSizeIterator<Item = (C6CellKey, C6ReadinessBlocker)> + '_ {
        self.deferred.iter().map(|(key, blocker)| (*key, *blocker))
    }
}

#[derive(Clone, Debug)]
pub struct C6AcceptanceCatalog {
    schema_version: u32,
    manifest_digest: [u8; 32],
    specification: C6AcceptanceSpec,
    enforced_tranche: C6EnforcedTranche,
}

impl C6AcceptanceCatalog {
    pub fn load(theme_catalog: &ThemeFixtureCatalog) -> Result<Self, CatalogError> {
        let path = theme_catalog.root().join(C6_ACCEPTANCE_RELATIVE_PATH);
        let json =
            fs::read_to_string(&path).map_err(|source| CatalogError::ReadFile { path, source })?;
        Self::from_json(&json, theme_catalog)
    }

    pub fn from_json(
        json: &str,
        theme_catalog: &ThemeFixtureCatalog,
    ) -> Result<Self, CatalogError> {
        let catalog = Self::parse_json(json)?;
        catalog.validate_against(theme_catalog)?;
        Ok(catalog)
    }

    fn parse_json(json: &str) -> Result<Self, CatalogError> {
        let wire: C6AcceptanceCatalogWire =
            serde_json::from_str(json).map_err(CatalogError::InvalidC6AcceptanceJson)?;
        if wire.schema_version != C6_ACCEPTANCE_SCHEMA_VERSION {
            return Err(CatalogError::UnsupportedC6AcceptanceSchemaVersion(
                wire.schema_version,
            ));
        }

        let mut specification_cells = Vec::with_capacity(wire.cells.len());
        let mut enforced_cells = Vec::new();
        let mut deferred = BTreeMap::new();
        let mut cell_index = BTreeMap::new();
        let mut semantic_assertion_ids = BTreeSet::new();

        for wire_cell in wire.cells {
            let key = C6CellKey::new(wire_cell.theme, wire_cell.family, wire_cell.target);
            if cell_index.contains_key(&key) {
                return Err(CatalogError::DuplicateC6AcceptanceCell { key: key.label() });
            }

            let expectation = convert_expectation(key, wire_cell.expectation)?;
            if !semantic_assertion_ids.insert(expectation.semantic_assertion_id.clone()) {
                return invalid_cell(key, "semantic assertion id is duplicated");
            }
            cell_index.insert(key, specification_cells.len());
            specification_cells.push(C6AcceptanceCell { key, expectation });
            match wire_cell.enforcement {
                C6CellEnforcementWire::Enforced { source_fixture_id } => {
                    if let Err(error) = validate_id(&source_fixture_id) {
                        return invalid_cell(key, error.to_string());
                    }
                    enforced_cells.push(C6EnforcedCell {
                        key,
                        source_fixture_id,
                    });
                }
                C6CellEnforcementWire::Deferred { blocker } => {
                    deferred.insert(key, blocker);
                }
            }
        }

        let expected = expected_cell_keys();
        let actual = cell_index.keys().copied().collect::<BTreeSet<_>>();
        if expected != actual {
            return Err(CatalogError::C6AcceptanceCellSetMismatch {
                missing: expected
                    .difference(&actual)
                    .map(|key| key.label())
                    .collect(),
                unexpected: actual
                    .difference(&expected)
                    .map(|key| key.label())
                    .collect(),
            });
        }

        let specification = C6AcceptanceSpec {
            cells: specification_cells,
            cell_index: cell_index.clone(),
        };
        let enforced_tranche = C6EnforcedTranche {
            cell_index: enforced_cells
                .iter()
                .enumerate()
                .map(|(index, cell)| (cell.key, index))
                .collect(),
            cells: enforced_cells,
            deferred,
        };
        let manifest_digest =
            canonical_manifest_digest(wire.schema_version, &specification, &enforced_tranche);

        Ok(Self {
            schema_version: wire.schema_version,
            manifest_digest,
            specification,
            enforced_tranche,
        })
    }

    fn validate_against(&self, theme_catalog: &ThemeFixtureCatalog) -> Result<(), CatalogError> {
        for cell in self.specification.cells() {
            validate_expectation_against_source(cell.key, &cell.expectation, theme_catalog)?;
        }
        for cell in self.enforced_tranche.cells() {
            let expectation = self
                .specification
                .cell(cell.key())
                .expect("every enforced cell belongs to the validated specification")
                .expectation();
            validate_enforced_source_fixture(cell, expectation, theme_catalog)?;
        }
        Ok(())
    }

    pub const fn specification(&self) -> &C6AcceptanceSpec {
        &self.specification
    }

    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub const fn manifest_digest(&self) -> &[u8; 32] {
        &self.manifest_digest
    }

    pub const fn enforced_tranche(&self) -> &C6EnforcedTranche {
        &self.enforced_tranche
    }

    pub fn cells(&self) -> impl ExactSizeIterator<Item = &C6AcceptanceCell> {
        self.specification.cells()
    }

    pub fn cell(&self, key: C6CellKey) -> Option<&C6AcceptanceCell> {
        self.specification.cell(key)
    }
}

fn expected_cell_keys() -> BTreeSet<C6CellKey> {
    C6_PROOF_THEMES
        .into_iter()
        .flat_map(|theme| {
            C6_PROOF_FAMILIES.into_iter().flat_map(move |family| {
                C6_NATIVE_OUTPUT_TARGETS
                    .into_iter()
                    .map(move |target| C6CellKey::new(theme, family, target))
            })
        })
        .collect()
}

fn convert_expectation(
    key: C6CellKey,
    wire: C6CellExpectationWire,
) -> Result<C6CellExpectation, CatalogError> {
    validate_expectation_shape(
        key,
        &wire.mechanism_requirements,
        &wire.expected_residual_ids,
        &wire.semantic_assertion_id,
    )?;
    Ok(C6CellExpectation {
        mechanism_requirements: wire.mechanism_requirements,
        expected_residual_ids: wire.expected_residual_ids,
        semantic_assertion_id: wire.semantic_assertion_id,
    })
}

fn validate_expectation_shape(
    key: C6CellKey,
    mechanism_requirements: &BTreeMap<ReferenceThemeMechanism, C6ExpectedMechanismDisposition>,
    expected_residual_ids: &BTreeSet<String>,
    semantic_assertion_id: &str,
) -> Result<(), CatalogError> {
    if mechanism_requirements.is_empty() {
        return invalid_cell(key, "a C6 expectation requires at least one mechanism");
    }
    for residual_id in expected_residual_ids {
        if let Err(error) = validate_id(residual_id) {
            return invalid_cell(key, error.to_string());
        }
    }
    if let Err(error) = validate_id(semantic_assertion_id) {
        return invalid_cell(key, error.to_string());
    }

    let has_residual_mechanism = mechanism_requirements
        .values()
        .any(|disposition| *disposition == C6ExpectedMechanismDisposition::MustRemainResidual);
    if has_residual_mechanism != !expected_residual_ids.is_empty() {
        return invalid_cell(
            key,
            "residual mechanism dispositions and residual ids must both be present or absent",
        );
    }

    if !expected_residual_ids.is_empty()
        || mechanism_requirements.values().any(|disposition| {
            matches!(
                disposition,
                C6ExpectedMechanismDisposition::MustRemainResidual
                    | C6ExpectedMechanismDisposition::MustReject
            )
        })
    {
        return invalid_cell(
            key,
            "the native C6a ledger accepts only portable, residual-free cells",
        );
    }
    Ok(())
}

fn validate_expectation_against_source(
    key: C6CellKey,
    expectation: &C6CellExpectation,
    theme_catalog: &ThemeFixtureCatalog,
) -> Result<(), CatalogError> {
    let theme = theme_catalog
        .theme(key.theme.reference_name())
        .ok_or_else(|| CatalogError::InvalidC6AcceptanceCell {
            key: key.label(),
            reason: format!(
                "reference theme `{}` is absent from the source catalog",
                key.theme.reference_name()
            ),
        })?;
    let actual_mechanisms = expectation
        .mechanism_requirements
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    if !actual_mechanisms.is_subset(theme.source_mechanisms()) {
        return invalid_cell(
            key,
            "mechanism requirements include a mechanism absent from the reference theme",
        );
    }

    let target = theme
        .target(key.target)
        .ok_or_else(|| CatalogError::InvalidC6AcceptanceCell {
            key: key.label(),
            reason: "reference theme lacks the requested target".to_string(),
        })?;
    if target.grade() != ExpectedPortabilityGrade::Portable {
        return invalid_cell(
            key,
            "the native C6a ledger requires a portable reference target contract",
        );
    }
    let reference_residual_ids = target
        .residuals()
        .iter()
        .map(|residual| residual.id().to_string())
        .collect::<BTreeSet<_>>();
    if expectation.expected_residual_ids != reference_residual_ids {
        return invalid_cell(
            key,
            "expected residual ids do not match the reference target contract",
        );
    }

    Ok(())
}

fn validate_enforced_source_fixture(
    cell: &C6EnforcedCell,
    expectation: &C6CellExpectation,
    theme_catalog: &ThemeFixtureCatalog,
) -> Result<(), CatalogError> {
    let expected_family = cell.key.family.fixture_family();
    let fixture = theme_catalog
        .fixture(&cell.source_fixture_id)
        .ok_or_else(|| CatalogError::InvalidC6AcceptanceCell {
            key: cell.key.label(),
            reason: format!("unknown source fixture `{}`", cell.source_fixture_id),
        })?;
    if fixture.source_family() != expected_family {
        return invalid_cell(
            cell.key,
            "enforced source fixture family does not match the proof family",
        );
    }

    let theme = theme_catalog
        .theme(cell.key.theme.reference_name())
        .ok_or_else(|| CatalogError::InvalidC6AcceptanceCell {
            key: cell.key.label(),
            reason: format!(
                "reference theme `{}` is absent from the source catalog",
                cell.key.theme.reference_name()
            ),
        })?;
    if !theme.fixture_ids().contains(fixture.id()) {
        return invalid_cell(
            cell.key,
            format!(
                "enforced source fixture `{}` does not belong to reference theme `{}`",
                fixture.id(),
                theme.reference_name()
            ),
        );
    }

    let input = fixture
        .theme_input()
        .ok_or_else(|| CatalogError::InvalidC6AcceptanceCell {
            key: cell.key.label(),
            reason: format!(
                "enforced source fixture `{}` lacks a typed theme input",
                fixture.id()
            ),
        })?;
    if input.fixture_id() != fixture.id() {
        return invalid_cell(
            cell.key,
            "typed theme input fixture id does not match the enforced source fixture",
        );
    }
    let input_mechanisms = input.mechanisms();
    if !input_mechanisms.is_subset(theme.source_mechanisms()) {
        return invalid_cell(
            cell.key,
            "typed theme input includes a mechanism absent from the reference theme",
        );
    }
    let required_mechanisms = expectation
        .mechanism_requirements()
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    if !required_mechanisms.is_subset(&input_mechanisms) {
        return invalid_cell(
            cell.key,
            "cell mechanism requirements are not covered by the typed theme input",
        );
    }
    Ok(())
}

fn canonical_manifest_digest(
    schema_version: u32,
    specification: &C6AcceptanceSpec,
    enforced_tranche: &C6EnforcedTranche,
) -> [u8; 32] {
    let mut value = b"merman.c6a-acceptance-manifest.v3\0".to_vec();
    value.extend_from_slice(&schema_version.to_be_bytes());
    value.extend_from_slice(&(specification.cells.len() as u64).to_be_bytes());

    for key in specification.cell_index.keys().copied() {
        append_cell_key(&mut value, key);
        let expectation = specification
            .cell(key)
            .expect("indexed C6 acceptance cell")
            .expectation();
        value.extend_from_slice(&(expectation.mechanism_requirements.len() as u64).to_be_bytes());
        for (mechanism, disposition) in &expectation.mechanism_requirements {
            append_len_prefixed(&mut value, mechanism_id(*mechanism).as_bytes());
            append_len_prefixed(&mut value, disposition_id(*disposition).as_bytes());
        }
        value.extend_from_slice(&(expectation.expected_residual_ids.len() as u64).to_be_bytes());
        for residual_id in &expectation.expected_residual_ids {
            append_len_prefixed(&mut value, residual_id.as_bytes());
        }
        append_len_prefixed(&mut value, expectation.semantic_assertion_id.as_bytes());

        if let Some(enforced) = enforced_tranche.cell(key) {
            append_len_prefixed(&mut value, b"enforced");
            append_len_prefixed(&mut value, enforced.source_fixture_id().as_bytes());
        } else if let Some(blocker) = enforced_tranche.readiness_blocker(key) {
            append_len_prefixed(&mut value, b"deferred");
            append_len_prefixed(&mut value, readiness_blocker_id(blocker).as_bytes());
        } else {
            unreachable!("validated C6 acceptance cell lacks enforcement state");
        }
    }

    Sha256::digest(value).into()
}

fn append_cell_key(value: &mut Vec<u8>, key: C6CellKey) {
    append_len_prefixed(value, key.theme().reference_name().as_bytes());
    append_len_prefixed(value, key.family().as_str().as_bytes());
    append_len_prefixed(value, target_id(key.target()).as_bytes());
}

fn append_len_prefixed(value: &mut Vec<u8>, bytes: &[u8]) {
    value.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    value.extend_from_slice(bytes);
}

fn target_id(target: ExpectedOutputTarget) -> &'static str {
    match target {
        ExpectedOutputTarget::BrowserSvg => "browser-svg",
        ExpectedOutputTarget::Jpeg => "jpeg",
        ExpectedOutputTarget::Pdf => "pdf",
        ExpectedOutputTarget::Png => "png",
        ExpectedOutputTarget::StandaloneSvg => "standalone-svg",
    }
}

fn disposition_id(disposition: C6ExpectedMechanismDisposition) -> &'static str {
    match disposition {
        C6ExpectedMechanismDisposition::MustApply => "must-apply",
        C6ExpectedMechanismDisposition::MustRemainResidual => "must-remain-residual",
        C6ExpectedMechanismDisposition::MustReject => "must-reject",
        C6ExpectedMechanismDisposition::NotApplicable => "not-applicable",
    }
}

fn readiness_blocker_id(blocker: C6ReadinessBlocker) -> &'static str {
    match blocker {
        C6ReadinessBlocker::FamilyAdapterIncomplete => "family-adapter-incomplete",
        C6ReadinessBlocker::ThemeSliceIncomplete => "theme-slice-incomplete",
    }
}

fn mechanism_id(mechanism: ReferenceThemeMechanism) -> &'static str {
    match mechanism {
        ReferenceThemeMechanism::BackdropFilter => "backdrop-filter",
        ReferenceThemeMechanism::CanvasBlend => "canvas-blend",
        ReferenceThemeMechanism::CanvasGradient => "canvas-gradient",
        ReferenceThemeMechanism::CanvasLayering => "canvas-layering",
        ReferenceThemeMechanism::CanvasPattern => "canvas-pattern",
        ReferenceThemeMechanism::CanvasSolid => "canvas-solid",
        ReferenceThemeMechanism::CssFilter => "css-filter",
        ReferenceThemeMechanism::CssLetterSpacing => "css-letter-spacing",
        ReferenceThemeMechanism::CssTextTransform => "css-text-transform",
        ReferenceThemeMechanism::DashArray => "dash-array",
        ReferenceThemeMechanism::ExternalSvgFilterReference => "external-svg-filter-reference",
        ReferenceThemeMechanism::FontStack => "font-stack",
        ReferenceThemeMechanism::HasSelector => "has-selector",
        ReferenceThemeMechanism::NotSelector => "not-selector",
        ReferenceThemeMechanism::NthChildSelector => "nth-child-selector",
        ReferenceThemeMechanism::RoundedCorners => "rounded-corners",
        ReferenceThemeMechanism::StrokeStyling => "stroke-styling",
        ReferenceThemeMechanism::ThemeVariables => "theme-variables",
    }
}

fn invalid_cell<T>(key: C6CellKey, reason: impl Into<String>) -> Result<T, CatalogError> {
    Err(CatalogError::InvalidC6AcceptanceCell {
        key: key.label(),
        reason: reason.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_family_mapping_is_total() {
        assert_eq!(
            C6ProofFamily::Flowchart.fixture_family(),
            ReferenceDiagramFamily::Flowchart
        );
        assert_eq!(
            C6ProofFamily::Sequence.fixture_family(),
            ReferenceDiagramFamily::Sequence
        );
        assert_eq!(
            C6ProofFamily::State.fixture_family(),
            ReferenceDiagramFamily::StateDiagram
        );
    }
}

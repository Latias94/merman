use crate::acceptance_wire::{
    C6AcceptanceCatalogWire, C6CellEnforcementWire, C6CellExpectationWire,
};
use crate::io::validate_id;
use crate::{
    CatalogError, EXPECTED_OUTPUT_TARGETS, ExpectedOutputTarget, ExpectedPortabilityGrade,
    ReferenceDiagramFamily, ReferenceThemeMechanism, ThemeFixtureCatalog,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use thiserror::Error;

pub const C6_ACCEPTANCE_RELATIVE_PATH: &str = "acceptance/c6-v2.json";
pub const C6_ACCEPTANCE_SCHEMA_VERSION: u32 = 2;
pub const C6_ACCEPTANCE_CELL_COUNT: usize = 45;

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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum C6ExpectedFontSource {
    Embedded,
    None,
    System,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum C6RequiredAdmission {
    HostDependent,
    Portable,
    Rejected,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum C6ArtifactAssertion {
    BrowserSvgDom,
    JpegImage,
    PdfDocument,
    PngImage,
    StandaloneSvgDocument,
}

impl C6ArtifactAssertion {
    const fn target(self) -> ExpectedOutputTarget {
        match self {
            Self::BrowserSvgDom => ExpectedOutputTarget::BrowserSvg,
            Self::JpegImage => ExpectedOutputTarget::Jpeg,
            Self::PdfDocument => ExpectedOutputTarget::Pdf,
            Self::PngImage => ExpectedOutputTarget::Png,
            Self::StandaloneSvgDocument => ExpectedOutputTarget::StandaloneSvg,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum C6ReadinessBlocker {
    FamilyAdapterIncomplete,
    RuntimeRunnerMissing,
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
    required_font_source: C6ExpectedFontSource,
    required_admission: C6RequiredAdmission,
    required_artifact_assertion: C6ArtifactAssertion,
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

    pub const fn required_font_source(&self) -> C6ExpectedFontSource {
        self.required_font_source
    }

    pub const fn required_admission(&self) -> C6RequiredAdmission {
        self.required_admission
    }

    pub const fn required_artifact_assertion(&self) -> C6ArtifactAssertion {
        self.required_artifact_assertion
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

        for wire_cell in wire.cells {
            let key = C6CellKey::new(wire_cell.theme, wire_cell.family, wire_cell.target);
            if cell_index.contains_key(&key) {
                return Err(CatalogError::DuplicateC6AcceptanceCell { key: key.label() });
            }

            let expectation = convert_expectation(key, wire_cell.expectation)?;
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

        Ok(Self {
            specification: C6AcceptanceSpec {
                cells: specification_cells,
                cell_index: cell_index.clone(),
            },
            enforced_tranche: C6EnforcedTranche {
                cell_index: enforced_cells
                    .iter()
                    .enumerate()
                    .map(|(index, cell)| (cell.key, index))
                    .collect(),
                cells: enforced_cells,
                deferred,
            },
        })
    }

    fn validate_against(&self, theme_catalog: &ThemeFixtureCatalog) -> Result<(), CatalogError> {
        for cell in self.specification.cells() {
            validate_expectation_against_source(cell.key, &cell.expectation, theme_catalog)?;
        }
        for cell in self.enforced_tranche.cells() {
            validate_enforced_source_fixture(cell, theme_catalog)?;
        }
        Ok(())
    }

    pub const fn specification(&self) -> &C6AcceptanceSpec {
        &self.specification
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum C6ObservedMechanismDisposition {
    Applied,
    NotApplicable,
    Rejected,
    Residual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum C6ObservedArtifactStatus {
    Failed,
    Passed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C6ObservedCellReport {
    key: C6CellKey,
    mechanism_dispositions: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    residual_ids: BTreeSet<String>,
    font_source: C6ExpectedFontSource,
    admission: C6RequiredAdmission,
    artifact_assertion: C6ArtifactAssertion,
    artifact_status: C6ObservedArtifactStatus,
}

impl C6ObservedCellReport {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        key: C6CellKey,
        mechanism_dispositions: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
        residual_ids: BTreeSet<String>,
        font_source: C6ExpectedFontSource,
        admission: C6RequiredAdmission,
        artifact_assertion: C6ArtifactAssertion,
        artifact_status: C6ObservedArtifactStatus,
    ) -> Self {
        Self {
            key,
            mechanism_dispositions,
            residual_ids,
            font_source,
            admission,
            artifact_assertion,
            artifact_status,
        }
    }

    pub const fn key(&self) -> C6CellKey {
        self.key
    }

    pub fn mechanism_dispositions(
        &self,
    ) -> &BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> {
        &self.mechanism_dispositions
    }

    pub fn residual_ids(&self) -> &BTreeSet<String> {
        &self.residual_ids
    }

    pub const fn font_source(&self) -> C6ExpectedFontSource {
        self.font_source
    }

    pub const fn admission(&self) -> C6RequiredAdmission {
        self.admission
    }

    pub const fn artifact_assertion(&self) -> C6ArtifactAssertion {
        self.artifact_assertion
    }

    pub const fn artifact_status(&self) -> C6ObservedArtifactStatus {
        self.artifact_status
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C6ObservedReport {
    cells: BTreeMap<C6CellKey, C6ObservedCellReport>,
}

impl C6ObservedReport {
    pub fn from_enforced_cells(
        tranche: &C6EnforcedTranche,
        cells: impl IntoIterator<Item = C6ObservedCellReport>,
    ) -> Result<Self, C6ObservedReportError> {
        let mut indexed = BTreeMap::new();
        for cell in cells {
            let key = cell.key;
            if indexed.insert(key, cell).is_some() {
                return Err(C6ObservedReportError::DuplicateCell { key: key.label() });
            }
        }

        let expected = tranche
            .cells()
            .map(C6EnforcedCell::key)
            .collect::<BTreeSet<_>>();
        let actual = indexed.keys().copied().collect::<BTreeSet<_>>();
        if expected != actual {
            return Err(C6ObservedReportError::CellSetMismatch {
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

        Ok(Self { cells: indexed })
    }

    pub fn evaluate(
        &self,
        specification: &C6AcceptanceSpec,
        tranche: &C6EnforcedTranche,
    ) -> Result<C6AcceptanceEvaluation, C6AcceptanceEvaluationError> {
        if tranche.cells.is_empty() {
            return Err(C6AcceptanceEvaluationError::EmptyTranche);
        }

        let expected_keys = tranche
            .cells()
            .map(C6EnforcedCell::key)
            .collect::<BTreeSet<_>>();
        let observed_keys = self.cells.keys().copied().collect::<BTreeSet<_>>();
        if expected_keys != observed_keys {
            return Err(C6AcceptanceEvaluationError::TrancheMismatch);
        }

        for enforced in tranche.cells() {
            let key = enforced.key;
            let expectation = specification.cell(key).ok_or_else(|| {
                C6AcceptanceEvaluationError::MissingExpectation { key: key.label() }
            })?;
            let observed = &self.cells[&key];

            if observed.mechanism_dispositions.len()
                != expectation.expectation.mechanism_requirements.len()
                || expectation.expectation.mechanism_requirements.iter().any(
                    |(mechanism, disposition)| {
                        observed.mechanism_dispositions.get(mechanism)
                            != Some(&expected_observed_disposition(*disposition))
                    },
                )
            {
                return expectation_mismatch(key, C6ExpectationField::MechanismDisposition);
            }
            if observed.residual_ids != expectation.expectation.expected_residual_ids {
                return expectation_mismatch(key, C6ExpectationField::ResidualIds);
            }
            if observed.font_source != expectation.expectation.required_font_source {
                return expectation_mismatch(key, C6ExpectationField::FontSource);
            }
            if observed.admission != expectation.expectation.required_admission {
                return expectation_mismatch(key, C6ExpectationField::Admission);
            }
            if observed.artifact_assertion != expectation.expectation.required_artifact_assertion {
                return expectation_mismatch(key, C6ExpectationField::ArtifactAssertion);
            }
            if observed.artifact_status != C6ObservedArtifactStatus::Passed {
                return expectation_mismatch(key, C6ExpectationField::ArtifactStatus);
            }
        }

        Ok(C6AcceptanceEvaluation {
            verified_cell_count: self.cells.len(),
        })
    }

    pub fn cells(&self) -> impl ExactSizeIterator<Item = &C6ObservedCellReport> {
        self.cells.values()
    }

    pub fn cell(&self, key: C6CellKey) -> Option<&C6ObservedCellReport> {
        self.cells.get(&key)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum C6ObservedReportError {
    #[error("C6 observed cell `{key}` is duplicated")]
    DuplicateCell { key: String },
    #[error("C6 observed cell set mismatch; missing {missing:?}, unexpected {unexpected:?}")]
    CellSetMismatch {
        missing: Vec<String>,
        unexpected: Vec<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C6AcceptanceEvaluation {
    verified_cell_count: usize,
}

impl C6AcceptanceEvaluation {
    pub const fn verified_cell_count(&self) -> usize {
        self.verified_cell_count
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum C6ExpectationField {
    Admission,
    ArtifactAssertion,
    ArtifactStatus,
    FontSource,
    MechanismDisposition,
    ResidualIds,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum C6AcceptanceEvaluationError {
    #[error("C6 acceptance cannot pass with an empty enforced tranche")]
    EmptyTranche,
    #[error("C6 enforced cell `{key}` has no acceptance expectation")]
    MissingExpectation { key: String },
    #[error("C6 observed cell `{key}` does not satisfy the {field:?} expectation")]
    ExpectationMismatch {
        key: String,
        field: C6ExpectationField,
    },
    #[error("C6 observed report does not cover the evaluated tranche")]
    TrancheMismatch,
}

fn expected_observed_disposition(
    expected: C6ExpectedMechanismDisposition,
) -> C6ObservedMechanismDisposition {
    match expected {
        C6ExpectedMechanismDisposition::MustApply => C6ObservedMechanismDisposition::Applied,
        C6ExpectedMechanismDisposition::MustRemainResidual => {
            C6ObservedMechanismDisposition::Residual
        }
        C6ExpectedMechanismDisposition::MustReject => C6ObservedMechanismDisposition::Rejected,
        C6ExpectedMechanismDisposition::NotApplicable => {
            C6ObservedMechanismDisposition::NotApplicable
        }
    }
}

fn expectation_mismatch<T>(
    key: C6CellKey,
    field: C6ExpectationField,
) -> Result<T, C6AcceptanceEvaluationError> {
    Err(C6AcceptanceEvaluationError::ExpectationMismatch {
        key: key.label(),
        field,
    })
}

fn expected_cell_keys() -> BTreeSet<C6CellKey> {
    C6_PROOF_THEMES
        .into_iter()
        .flat_map(|theme| {
            C6_PROOF_FAMILIES.into_iter().flat_map(move |family| {
                EXPECTED_OUTPUT_TARGETS
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
        wire.required_font_source,
        wire.required_admission,
        wire.required_artifact_assertion,
    )?;
    Ok(C6CellExpectation {
        mechanism_requirements: wire.mechanism_requirements,
        expected_residual_ids: wire.expected_residual_ids,
        required_font_source: wire.required_font_source,
        required_admission: wire.required_admission,
        required_artifact_assertion: wire.required_artifact_assertion,
    })
}

fn validate_expectation_shape(
    key: C6CellKey,
    mechanism_requirements: &BTreeMap<ReferenceThemeMechanism, C6ExpectedMechanismDisposition>,
    expected_residual_ids: &BTreeSet<String>,
    required_font_source: C6ExpectedFontSource,
    required_admission: C6RequiredAdmission,
    required_artifact_assertion: C6ArtifactAssertion,
) -> Result<(), CatalogError> {
    if mechanism_requirements.is_empty() {
        return invalid_cell(key, "a C6 expectation requires at least one mechanism");
    }
    for residual_id in expected_residual_ids {
        if let Err(error) = validate_id(residual_id) {
            return invalid_cell(key, error.to_string());
        }
    }
    if required_artifact_assertion.target() != key.target {
        return invalid_cell(key, "artifact assertion does not match the output target");
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

    match required_admission {
        C6RequiredAdmission::Portable => {
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
                    "portable admission cannot require residual or rejected mechanisms",
                );
            }
            if required_font_source != C6ExpectedFontSource::Embedded {
                return invalid_cell(key, "portable admission requires an embedded font source");
            }
        }
        C6RequiredAdmission::HostDependent => {
            if required_font_source == C6ExpectedFontSource::None {
                return invalid_cell(
                    key,
                    "host-dependent admission requires a concrete font source",
                );
            }
        }
        C6RequiredAdmission::Rejected => {
            if required_font_source != C6ExpectedFontSource::None
                || !mechanism_requirements
                    .values()
                    .any(|disposition| *disposition == C6ExpectedMechanismDisposition::MustReject)
            {
                return invalid_cell(
                    key,
                    "rejected admission requires no font source and at least one rejected mechanism",
                );
            }
        }
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
    if actual_mechanisms != *theme.source_mechanisms() {
        return invalid_cell(
            key,
            "mechanism requirements do not exactly cover the reference theme",
        );
    }

    let target = theme
        .target(key.target)
        .ok_or_else(|| CatalogError::InvalidC6AcceptanceCell {
            key: key.label(),
            reason: "reference theme lacks the requested target".to_string(),
        })?;
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

    let required_admission = required_admission_for_source_grade(target.grade());
    if expectation.required_admission != required_admission {
        return invalid_cell(
            key,
            "required admission contradicts the reference target portability contract",
        );
    }
    Ok(())
}

fn validate_enforced_source_fixture(
    cell: &C6EnforcedCell,
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
    Ok(())
}

const fn required_admission_for_source_grade(
    grade: ExpectedPortabilityGrade,
) -> C6RequiredAdmission {
    match grade {
        ExpectedPortabilityGrade::Portable => C6RequiredAdmission::Portable,
        ExpectedPortabilityGrade::HostDependent => C6RequiredAdmission::HostDependent,
        ExpectedPortabilityGrade::SvgOnly | ExpectedPortabilityGrade::Unverified => {
            C6RequiredAdmission::Rejected
        }
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
    fn source_grade_mapping_is_total_and_fail_closed() {
        assert_eq!(
            required_admission_for_source_grade(ExpectedPortabilityGrade::Portable),
            C6RequiredAdmission::Portable
        );
        assert_eq!(
            required_admission_for_source_grade(ExpectedPortabilityGrade::HostDependent),
            C6RequiredAdmission::HostDependent
        );
        assert_eq!(
            required_admission_for_source_grade(ExpectedPortabilityGrade::SvgOnly),
            C6RequiredAdmission::Rejected
        );
        assert_eq!(
            required_admission_for_source_grade(ExpectedPortabilityGrade::Unverified),
            C6RequiredAdmission::Rejected
        );
    }

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

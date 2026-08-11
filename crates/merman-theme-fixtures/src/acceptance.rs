use crate::acceptance_wire::{C6AcceptanceCatalogWire, C6CellExpectationWire};
use crate::io::validate_id;
use crate::{
    CatalogError, EXPECTED_OUTPUT_TARGETS, ExpectedOutputTarget, ExpectedPortabilityGrade,
    ReferenceDiagramFamily, ReferenceThemeMechanism, ThemeFixtureCatalog,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

pub const C6_ACCEPTANCE_RELATIVE_PATH: &str = "acceptance/c6-v1.json";
pub const C6_ACCEPTANCE_SCHEMA_VERSION: u32 = 1;
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
    const fn source_family(self) -> Option<ReferenceDiagramFamily> {
        match self {
            Self::Flowchart => Some(ReferenceDiagramFamily::Flowchart),
            Self::Sequence => None,
            Self::State => Some(ReferenceDiagramFamily::StateDiagram),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Flowchart => "flowchart",
            Self::Sequence => "sequence",
            Self::State => "state",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum C6ExpectedMechanismDisposition {
    MustApply,
    MustRemainResidual,
    MustReject,
    NotApplicable,
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
pub struct C6ExecutableExpectation {
    source_fixture_id: String,
    mechanism_requirements: BTreeMap<ReferenceThemeMechanism, C6ExpectedMechanismDisposition>,
    expected_residual_ids: BTreeSet<String>,
    required_admission: C6RequiredAdmission,
    required_artifact_assertion: C6ArtifactAssertion,
}

impl C6ExecutableExpectation {
    pub fn source_fixture_id(&self) -> &str {
        &self.source_fixture_id
    }

    pub fn mechanism_requirements(
        &self,
    ) -> &BTreeMap<ReferenceThemeMechanism, C6ExpectedMechanismDisposition> {
        &self.mechanism_requirements
    }

    pub fn expected_residual_ids(&self) -> &BTreeSet<String> {
        &self.expected_residual_ids
    }

    pub const fn required_admission(&self) -> C6RequiredAdmission {
        self.required_admission
    }

    pub const fn required_artifact_assertion(&self) -> C6ArtifactAssertion {
        self.required_artifact_assertion
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum C6CellExpectation {
    Executable(C6ExecutableExpectation),
    NotReady { blocker: C6ReadinessBlocker },
}

impl C6CellExpectation {
    pub fn executable(&self) -> Option<&C6ExecutableExpectation> {
        match self {
            Self::Executable(expectation) => Some(expectation),
            Self::NotReady { .. } => None,
        }
    }

    pub const fn readiness_blocker(&self) -> Option<C6ReadinessBlocker> {
        match self {
            Self::Executable(_) => None,
            Self::NotReady { blocker } => Some(*blocker),
        }
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

    pub fn expectation(&self) -> &C6CellExpectation {
        &self.expectation
    }
}

#[derive(Clone, Debug)]
pub struct C6AcceptanceCatalog {
    cells: Vec<C6AcceptanceCell>,
    cell_index: BTreeMap<C6CellKey, usize>,
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

        let mut cells = Vec::with_capacity(wire.cells.len());
        let mut cell_index = BTreeMap::new();
        for wire_cell in wire.cells {
            let key = C6CellKey::new(wire_cell.theme, wire_cell.family, wire_cell.target);
            if cell_index.contains_key(&key) {
                return Err(CatalogError::DuplicateC6AcceptanceCell { key: key.label() });
            }
            let expectation = convert_expectation(key, wire_cell.expectation)?;
            cell_index.insert(key, cells.len());
            cells.push(C6AcceptanceCell { key, expectation });
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

        Ok(Self { cells, cell_index })
    }

    fn validate_against(&self, theme_catalog: &ThemeFixtureCatalog) -> Result<(), CatalogError> {
        for cell in &self.cells {
            if let C6CellExpectation::Executable(expectation) = &cell.expectation {
                validate_executable_against_source(cell.key, expectation, theme_catalog)?;
            }
        }
        Ok(())
    }

    pub fn cells(&self) -> impl ExactSizeIterator<Item = &C6AcceptanceCell> {
        self.cells.iter()
    }

    pub fn cell(&self, key: C6CellKey) -> Option<&C6AcceptanceCell> {
        self.cell_index.get(&key).map(|index| &self.cells[*index])
    }
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
    let executable_in_v1 =
        key.theme == C6ProofTheme::Brutalist && key.family == C6ProofFamily::State;
    match (executable_in_v1, wire) {
        (
            true,
            C6CellExpectationWire::Executable {
                source_fixture_id,
                mechanism_requirements,
                expected_residual_ids,
                required_admission,
                required_artifact_assertion,
            },
        ) => {
            validate_executable_shape(
                key,
                &source_fixture_id,
                &mechanism_requirements,
                &expected_residual_ids,
                required_admission,
                required_artifact_assertion,
            )?;
            Ok(C6CellExpectation::Executable(C6ExecutableExpectation {
                source_fixture_id,
                mechanism_requirements,
                expected_residual_ids,
                required_admission,
                required_artifact_assertion,
            }))
        }
        (false, C6CellExpectationWire::NotReady { blocker }) => {
            Ok(C6CellExpectation::NotReady { blocker })
        }
        (true, C6CellExpectationWire::NotReady { .. }) => invalid_cell(
            key,
            "the Brutalist State tranche must be executable for all five targets",
        ),
        (false, C6CellExpectationWire::Executable { .. }) => invalid_cell(
            key,
            "only the Brutalist State tranche may be executable in schema version 1",
        ),
    }
}

fn validate_executable_shape(
    key: C6CellKey,
    source_fixture_id: &str,
    mechanism_requirements: &BTreeMap<ReferenceThemeMechanism, C6ExpectedMechanismDisposition>,
    expected_residual_ids: &BTreeSet<String>,
    required_admission: C6RequiredAdmission,
    required_artifact_assertion: C6ArtifactAssertion,
) -> Result<(), CatalogError> {
    if let Err(error) = validate_id(source_fixture_id) {
        return invalid_cell(key, error.to_string());
    }
    if mechanism_requirements.is_empty() {
        return invalid_cell(key, "an executable cell requires at least one mechanism");
    }
    if mechanism_requirements
        .values()
        .any(|disposition| *disposition != C6ExpectedMechanismDisposition::MustApply)
    {
        return invalid_cell(
            key,
            "the portable Brutalist State tranche requires every source mechanism to apply",
        );
    }
    if !expected_residual_ids.is_empty() {
        return invalid_cell(
            key,
            "the portable Brutalist State tranche cannot expect residuals",
        );
    }
    if required_admission != C6RequiredAdmission::Portable {
        return invalid_cell(
            key,
            "the Brutalist State tranche requires portable target admission",
        );
    }
    if required_artifact_assertion.target() != key.target {
        return invalid_cell(key, "artifact assertion does not match the output target");
    }
    Ok(())
}

fn validate_executable_against_source(
    key: C6CellKey,
    expectation: &C6ExecutableExpectation,
    theme_catalog: &ThemeFixtureCatalog,
) -> Result<(), CatalogError> {
    let fixture = theme_catalog
        .fixture(&expectation.source_fixture_id)
        .ok_or_else(|| CatalogError::InvalidC6AcceptanceCell {
            key: key.label(),
            reason: format!("unknown source fixture `{}`", expectation.source_fixture_id),
        })?;
    if Some(fixture.source_family()) != key.family.source_family() {
        return invalid_cell(key, "source fixture family does not match the proof family");
    }

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
    if target.grade() != ExpectedPortabilityGrade::Portable {
        return invalid_cell(
            key,
            "reference target is not declared portable by the source catalog",
        );
    }
    Ok(())
}

fn invalid_cell<T>(key: C6CellKey, reason: impl Into<String>) -> Result<T, CatalogError> {
    Err(CatalogError::InvalidC6AcceptanceCell {
        key: key.label(),
        reason: reason.into(),
    })
}

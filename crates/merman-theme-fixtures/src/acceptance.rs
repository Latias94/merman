use crate::acceptance_wire::{
    C6AcceptanceCatalogV3Wire, C6AcceptanceCatalogWire, C6AcceptanceCellWire,
    C6CellEnforcementWire, C6CellExpectationWire, C6ProofRecipeWire,
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

pub const C6_ACCEPTANCE_RELATIVE_PATH: &str = "acceptance/c6-v4.json";
pub const C6_ACCEPTANCE_PREVIOUS_RELATIVE_PATH: &str = "acceptance/c6-v3.json";
pub const C6_ACCEPTANCE_SCHEMA_VERSION: u32 = 4;
pub const C6_ACCEPTANCE_CELL_COUNT: usize = 18;
pub const C6_ACCEPTANCE_RENDER_GROUP_COUNT: usize = 9;

pub const C6_REQUIRED_CRITICAL_MECHANISMS: [ReferenceThemeMechanism; 14] = [
    ReferenceThemeMechanism::CanvasBlend,
    ReferenceThemeMechanism::CanvasGradient,
    ReferenceThemeMechanism::CanvasLayering,
    ReferenceThemeMechanism::CanvasPattern,
    ReferenceThemeMechanism::CanvasSolid,
    ReferenceThemeMechanism::CssFilter,
    ReferenceThemeMechanism::CssLetterSpacing,
    ReferenceThemeMechanism::CssTextTransform,
    ReferenceThemeMechanism::DashArray,
    ReferenceThemeMechanism::FontStack,
    ReferenceThemeMechanism::NthChildSelector,
    ReferenceThemeMechanism::RoundedCorners,
    ReferenceThemeMechanism::StrokeStyling,
    ReferenceThemeMechanism::ThemeVariables,
];

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
    pub const fn as_str(self) -> &'static str {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct C6ProofRecipeKey {
    theme: C6ProofTheme,
    family: C6ProofFamily,
}

impl C6ProofRecipeKey {
    pub const fn new(theme: C6ProofTheme, family: C6ProofFamily) -> Self {
        Self { theme, family }
    }

    pub const fn theme(self) -> C6ProofTheme {
        self.theme
    }

    pub const fn family(self) -> C6ProofFamily {
        self.family
    }

    fn label(self) -> String {
        format!("{}/{}", self.theme.reference_name(), self.family.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C6ProofRecipe {
    key: C6ProofRecipeKey,
    source_fixture_id: String,
    proof_recipe_revision: String,
}

impl C6ProofRecipe {
    pub const fn key(&self) -> C6ProofRecipeKey {
        self.key
    }

    pub fn source_fixture_id(&self) -> &str {
        &self.source_fixture_id
    }

    pub fn proof_recipe_revision(&self) -> &str {
        &self.proof_recipe_revision
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
    manifest_revision: String,
    previous_manifest_digest: [u8; 32],
    manifest_digest: [u8; 32],
    required_critical_mechanisms: BTreeSet<ReferenceThemeMechanism>,
    proof_recipes: Vec<C6ProofRecipe>,
    proof_recipe_index: BTreeMap<C6ProofRecipeKey, usize>,
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
        catalog.validate_predecessor(theme_catalog)?;
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
        if let Err(error) = validate_id(&wire.manifest_revision) {
            return Err(CatalogError::InvalidC6AcceptanceManifest {
                reason: format!("invalid manifest revision: {error}"),
            });
        }
        let previous_manifest_digest = parse_hex_digest(&wire.previous_manifest_digest)
            .ok_or_else(|| CatalogError::InvalidC6AcceptanceManifest {
                reason: "previous manifest digest must be 64 lowercase hexadecimal characters"
                    .to_string(),
            })?;
        if wire.required_critical_mechanisms.is_empty() {
            return Err(CatalogError::InvalidC6AcceptanceManifest {
                reason: "required critical mechanisms must not be empty".to_string(),
            });
        }

        let (specification, enforced_tranche) = convert_cells(wire.cells)?;
        let (proof_recipes, proof_recipe_index) =
            convert_proof_recipes(wire.proof_recipes, &specification)?;
        validate_critical_mechanisms(&wire.required_critical_mechanisms, &specification)?;
        let manifest_digest = canonical_manifest_digest_v4(
            wire.schema_version,
            &wire.manifest_revision,
            &previous_manifest_digest,
            &wire.required_critical_mechanisms,
            &proof_recipes,
            &specification,
            &enforced_tranche,
        );

        Ok(Self {
            schema_version: wire.schema_version,
            manifest_revision: wire.manifest_revision,
            previous_manifest_digest,
            manifest_digest,
            required_critical_mechanisms: wire.required_critical_mechanisms,
            proof_recipes,
            proof_recipe_index,
            specification,
            enforced_tranche,
        })
    }

    fn validate_predecessor(
        &self,
        theme_catalog: &ThemeFixtureCatalog,
    ) -> Result<(), CatalogError> {
        let path = theme_catalog
            .root()
            .join(C6_ACCEPTANCE_PREVIOUS_RELATIVE_PATH);
        let json =
            fs::read_to_string(&path).map_err(|source| CatalogError::ReadFile { path, source })?;
        let predecessor = parse_v3_predecessor(&json)?;
        if predecessor.digest != self.previous_manifest_digest {
            return Err(CatalogError::C6AcceptanceManifestLineageMismatch {
                declared: encode_hex(&self.previous_manifest_digest),
                actual: encode_hex(&predecessor.digest),
            });
        }
        let current_keys = self
            .specification
            .cell_index
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        if !predecessor.cell_keys.is_subset(&current_keys) {
            return Err(CatalogError::C6AcceptanceCellSetMismatch {
                missing: predecessor
                    .cell_keys
                    .difference(&current_keys)
                    .map(|key| key.label())
                    .collect(),
                unexpected: Vec::new(),
            });
        }
        let current_enforced_keys = self
            .enforced_tranche
            .cell_index
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        let regressed_cells = predecessor
            .enforced_cell_keys
            .difference(&current_enforced_keys)
            .map(|key| key.label())
            .collect::<Vec<_>>();
        if !regressed_cells.is_empty() {
            return Err(CatalogError::InvalidC6AcceptanceManifest {
                reason: format!(
                    "previously enforced cells must remain enforced: {regressed_cells:?}"
                ),
            });
        }
        Ok(())
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
        for recipe in &self.proof_recipes {
            validate_proof_recipe_source(recipe, theme_catalog)?;
            validate_proof_recipe_cell_sources(recipe, &self.enforced_tranche)?;
        }
        Ok(())
    }

    pub const fn specification(&self) -> &C6AcceptanceSpec {
        &self.specification
    }

    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn manifest_revision(&self) -> &str {
        &self.manifest_revision
    }

    pub const fn previous_manifest_digest(&self) -> &[u8; 32] {
        &self.previous_manifest_digest
    }

    pub const fn manifest_digest(&self) -> &[u8; 32] {
        &self.manifest_digest
    }

    pub const fn enforced_tranche(&self) -> &C6EnforcedTranche {
        &self.enforced_tranche
    }

    pub fn required_critical_mechanisms(&self) -> &BTreeSet<ReferenceThemeMechanism> {
        &self.required_critical_mechanisms
    }

    pub fn proof_recipes(&self) -> impl ExactSizeIterator<Item = &C6ProofRecipe> {
        self.proof_recipes.iter()
    }

    pub fn proof_recipe(&self, key: C6ProofRecipeKey) -> Option<&C6ProofRecipe> {
        self.proof_recipe_index
            .get(&key)
            .map(|index| &self.proof_recipes[*index])
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

fn expected_proof_recipe_keys() -> BTreeSet<C6ProofRecipeKey> {
    C6_PROOF_THEMES
        .into_iter()
        .flat_map(|theme| {
            C6_PROOF_FAMILIES
                .into_iter()
                .map(move |family| C6ProofRecipeKey::new(theme, family))
        })
        .collect()
}

fn convert_cells(
    wire_cells: Vec<C6AcceptanceCellWire>,
) -> Result<(C6AcceptanceSpec, C6EnforcedTranche), CatalogError> {
    let mut specification_cells = Vec::with_capacity(wire_cells.len());
    let mut enforced_cells = Vec::new();
    let mut deferred = BTreeMap::new();
    let mut cell_index = BTreeMap::new();
    let mut semantic_assertion_ids = BTreeSet::new();

    for wire_cell in wire_cells {
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

    Ok((
        C6AcceptanceSpec {
            cells: specification_cells,
            cell_index: cell_index.clone(),
        },
        C6EnforcedTranche {
            cell_index: enforced_cells
                .iter()
                .enumerate()
                .map(|(index, cell)| (cell.key, index))
                .collect(),
            cells: enforced_cells,
            deferred,
        },
    ))
}

fn convert_proof_recipes(
    wire_recipes: Vec<C6ProofRecipeWire>,
    specification: &C6AcceptanceSpec,
) -> Result<(Vec<C6ProofRecipe>, BTreeMap<C6ProofRecipeKey, usize>), CatalogError> {
    let mut recipes = Vec::with_capacity(wire_recipes.len());
    let mut index = BTreeMap::new();
    for wire in wire_recipes {
        let key = C6ProofRecipeKey::new(wire.theme, wire.family);
        if index.contains_key(&key) {
            return Err(CatalogError::DuplicateC6ProofRecipe { key: key.label() });
        }
        if let Err(error) = validate_id(&wire.source_fixture_id) {
            return invalid_proof_recipe(key, error.to_string());
        }
        if let Err(error) = validate_id(&wire.proof_recipe_revision) {
            return invalid_proof_recipe(key, error.to_string());
        }
        for target in C6_NATIVE_OUTPUT_TARGETS {
            let cell_key = C6CellKey::new(key.theme(), key.family(), target);
            if specification.cell(cell_key).is_none() {
                return invalid_proof_recipe(key, "expected target cell is absent");
            }
        }
        index.insert(key, recipes.len());
        recipes.push(C6ProofRecipe {
            key,
            source_fixture_id: wire.source_fixture_id,
            proof_recipe_revision: wire.proof_recipe_revision,
        });
    }

    let expected = expected_proof_recipe_keys();
    let actual = index.keys().copied().collect::<BTreeSet<_>>();
    if expected != actual {
        return Err(CatalogError::C6ProofRecipeSetMismatch {
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
    Ok((recipes, index))
}

fn validate_critical_mechanisms(
    required: &BTreeSet<ReferenceThemeMechanism>,
    specification: &C6AcceptanceSpec,
) -> Result<(), CatalogError> {
    let expected = C6_REQUIRED_CRITICAL_MECHANISMS
        .into_iter()
        .collect::<BTreeSet<_>>();
    if required != &expected {
        return Err(CatalogError::C6CriticalMechanismSetMismatch {
            missing: expected
                .difference(required)
                .map(|mechanism| mechanism.id().to_string())
                .collect(),
            unexpected: required
                .difference(&expected)
                .map(|mechanism| mechanism.id().to_string())
                .collect(),
        });
    }
    let covered = specification
        .cells()
        .flat_map(|cell| cell.expectation().mechanism_requirements())
        .filter_map(|(mechanism, disposition)| {
            (*disposition == C6ExpectedMechanismDisposition::MustApply).then_some(*mechanism)
        })
        .collect::<BTreeSet<_>>();
    let uncovered = required.difference(&covered).copied().collect::<Vec<_>>();
    if uncovered.is_empty() {
        return Ok(());
    }
    Err(CatalogError::UncoveredC6CriticalMechanisms {
        mechanisms: uncovered
            .into_iter()
            .map(|mechanism| mechanism.id().to_string())
            .collect(),
    })
}

struct C6PredecessorManifest {
    digest: [u8; 32],
    cell_keys: BTreeSet<C6CellKey>,
    enforced_cell_keys: BTreeSet<C6CellKey>,
}

fn parse_v3_predecessor(json: &str) -> Result<C6PredecessorManifest, CatalogError> {
    let wire: C6AcceptanceCatalogV3Wire =
        serde_json::from_str(json).map_err(CatalogError::InvalidC6AcceptanceJson)?;
    if wire.schema_version != 3 {
        return Err(CatalogError::UnsupportedC6AcceptanceSchemaVersion(
            wire.schema_version,
        ));
    }
    let (specification, enforced_tranche) = convert_cells(wire.cells)?;
    let cell_keys = specification
        .cell_index
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    let enforced_cell_keys = enforced_tranche
        .cell_index
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    Ok(C6PredecessorManifest {
        digest: canonical_manifest_digest_v3(
            wire.schema_version,
            &specification,
            &enforced_tranche,
        ),
        cell_keys,
        enforced_cell_keys,
    })
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

fn validate_proof_recipe_source(
    recipe: &C6ProofRecipe,
    theme_catalog: &ThemeFixtureCatalog,
) -> Result<(), CatalogError> {
    let key = recipe.key();
    let fixture = theme_catalog
        .fixture(recipe.source_fixture_id())
        .ok_or_else(|| CatalogError::InvalidC6ProofRecipe {
            key: key.label(),
            reason: format!("unknown source fixture `{}`", recipe.source_fixture_id()),
        })?;
    if fixture.source_family() != key.family().fixture_family() {
        return invalid_proof_recipe(key, "source fixture family does not match the proof family");
    }
    let theme = theme_catalog
        .theme(key.theme().reference_name())
        .ok_or_else(|| CatalogError::InvalidC6ProofRecipe {
            key: key.label(),
            reason: format!(
                "reference theme `{}` is absent from the source catalog",
                key.theme().reference_name()
            ),
        })?;
    if !theme.fixture_ids().contains(fixture.id()) {
        return invalid_proof_recipe(
            key,
            format!(
                "source fixture `{}` does not belong to reference theme `{}`",
                fixture.id(),
                theme.reference_name()
            ),
        );
    }
    Ok(())
}

fn validate_proof_recipe_cell_sources(
    recipe: &C6ProofRecipe,
    enforced_tranche: &C6EnforcedTranche,
) -> Result<(), CatalogError> {
    let key = recipe.key();
    for target in C6_NATIVE_OUTPUT_TARGETS {
        let cell_key = C6CellKey::new(key.theme(), key.family(), target);
        if let Some(enforced) = enforced_tranche.cell(cell_key) {
            if enforced.source_fixture_id() != recipe.source_fixture_id() {
                return invalid_proof_recipe(
                    key,
                    "enforced target cell source fixture differs from the proof recipe",
                );
            }
        }
    }
    Ok(())
}

fn canonical_manifest_digest_v3(
    schema_version: u32,
    specification: &C6AcceptanceSpec,
    enforced_tranche: &C6EnforcedTranche,
) -> [u8; 32] {
    let mut value = b"merman.c6a-acceptance-manifest.v3\0".to_vec();
    value.extend_from_slice(&schema_version.to_be_bytes());
    value.extend_from_slice(&(specification.cells.len() as u64).to_be_bytes());
    append_cells_to_manifest_digest(&mut value, specification, enforced_tranche);
    Sha256::digest(value).into()
}

fn canonical_manifest_digest_v4(
    schema_version: u32,
    manifest_revision: &str,
    previous_manifest_digest: &[u8; 32],
    required_critical_mechanisms: &BTreeSet<ReferenceThemeMechanism>,
    proof_recipes: &[C6ProofRecipe],
    specification: &C6AcceptanceSpec,
    enforced_tranche: &C6EnforcedTranche,
) -> [u8; 32] {
    let mut value = b"merman.c6a-acceptance-manifest.v4\0".to_vec();
    value.extend_from_slice(&schema_version.to_be_bytes());
    append_len_prefixed(&mut value, manifest_revision.as_bytes());
    value.extend_from_slice(previous_manifest_digest);
    value.extend_from_slice(&(required_critical_mechanisms.len() as u64).to_be_bytes());
    for mechanism in required_critical_mechanisms {
        append_len_prefixed(&mut value, mechanism.id().as_bytes());
    }

    let recipes = proof_recipes
        .iter()
        .map(|recipe| (recipe.key(), recipe))
        .collect::<BTreeMap<_, _>>();
    value.extend_from_slice(&(recipes.len() as u64).to_be_bytes());
    for (key, recipe) in recipes {
        append_len_prefixed(&mut value, key.theme().reference_name().as_bytes());
        append_len_prefixed(&mut value, key.family().as_str().as_bytes());
        append_len_prefixed(&mut value, recipe.source_fixture_id().as_bytes());
        append_len_prefixed(&mut value, recipe.proof_recipe_revision().as_bytes());
    }

    value.extend_from_slice(&(specification.cells.len() as u64).to_be_bytes());
    append_cells_to_manifest_digest(&mut value, specification, enforced_tranche);
    Sha256::digest(value).into()
}

fn append_cells_to_manifest_digest(
    value: &mut Vec<u8>,
    specification: &C6AcceptanceSpec,
    enforced_tranche: &C6EnforcedTranche,
) {
    for key in specification.cell_index.keys().copied() {
        append_cell_key(value, key);
        let expectation = specification
            .cell(key)
            .expect("indexed C6 acceptance cell")
            .expectation();
        value.extend_from_slice(&(expectation.mechanism_requirements.len() as u64).to_be_bytes());
        for (mechanism, disposition) in &expectation.mechanism_requirements {
            append_len_prefixed(value, mechanism.id().as_bytes());
            append_len_prefixed(value, disposition_id(*disposition).as_bytes());
        }
        value.extend_from_slice(&(expectation.expected_residual_ids.len() as u64).to_be_bytes());
        for residual_id in &expectation.expected_residual_ids {
            append_len_prefixed(value, residual_id.as_bytes());
        }
        append_len_prefixed(value, expectation.semantic_assertion_id.as_bytes());

        if let Some(enforced) = enforced_tranche.cell(key) {
            append_len_prefixed(value, b"enforced");
            append_len_prefixed(value, enforced.source_fixture_id().as_bytes());
        } else if let Some(blocker) = enforced_tranche.readiness_blocker(key) {
            append_len_prefixed(value, b"deferred");
            append_len_prefixed(value, readiness_blocker_id(blocker).as_bytes());
        } else {
            unreachable!("validated C6 acceptance cell lacks enforcement state");
        }
    }
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

fn parse_hex_digest(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let mut digest = [0; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        digest[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Some(digest)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    encoded
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

fn invalid_cell<T>(key: C6CellKey, reason: impl Into<String>) -> Result<T, CatalogError> {
    Err(CatalogError::InvalidC6AcceptanceCell {
        key: key.label(),
        reason: reason.into(),
    })
}

fn invalid_proof_recipe<T>(
    key: C6ProofRecipeKey,
    reason: impl Into<String>,
) -> Result<T, CatalogError> {
    Err(CatalogError::InvalidC6ProofRecipe {
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

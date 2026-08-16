use crate::acceptance::{
    C6ExpectedMechanismDisposition, C6ProofFamily, C6ProofTheme, C6ReadinessBlocker,
};
use crate::{ExpectedOutputTarget, ReferenceThemeMechanism};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct C6AcceptanceCatalogWire {
    pub(crate) schema_version: u32,
    pub(crate) manifest_revision: String,
    pub(crate) previous_manifest_digest: String,
    pub(crate) required_critical_mechanisms: BTreeSet<ReferenceThemeMechanism>,
    pub(crate) proof_recipes: Vec<C6ProofRecipeWire>,
    pub(crate) cells: Vec<C6AcceptanceCellWire>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct C6ProofRecipeWire {
    pub(crate) theme: C6ProofTheme,
    pub(crate) family: C6ProofFamily,
    pub(crate) source_fixture_id: String,
    pub(crate) proof_recipe_revision: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct C6AcceptanceCatalogV3Wire {
    pub(crate) schema_version: u32,
    pub(crate) cells: Vec<C6AcceptanceCellWire>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct C6AcceptanceCellWire {
    pub(crate) theme: C6ProofTheme,
    pub(crate) family: C6ProofFamily,
    pub(crate) target: ExpectedOutputTarget,
    pub(crate) expectation: C6CellExpectationWire,
    pub(crate) enforcement: C6CellEnforcementWire,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct C6CellExpectationWire {
    pub(crate) mechanism_requirements:
        BTreeMap<ReferenceThemeMechanism, C6ExpectedMechanismDisposition>,
    pub(crate) expected_residual_ids: BTreeSet<String>,
    pub(crate) semantic_assertion_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum C6CellEnforcementWire {
    Enforced { source_fixture_id: String },
    Deferred { blocker: C6ReadinessBlocker },
}

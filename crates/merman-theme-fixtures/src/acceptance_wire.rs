use crate::acceptance::{
    C6ArtifactAssertion, C6ExpectedFontSource, C6ExpectedMechanismDisposition, C6ProofFamily,
    C6ProofTheme, C6ReadinessBlocker, C6RequiredAdmission,
};
use crate::{ExpectedOutputTarget, ReferenceThemeMechanism};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct C6AcceptanceCatalogWire {
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
    pub(crate) required_font_source: C6ExpectedFontSource,
    pub(crate) required_admission: C6RequiredAdmission,
    pub(crate) required_artifact_assertion: C6ArtifactAssertion,
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

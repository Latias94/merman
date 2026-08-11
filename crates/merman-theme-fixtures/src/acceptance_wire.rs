use crate::acceptance::{
    C6ArtifactAssertion, C6ExpectedMechanismDisposition, C6ProofFamily, C6ProofTheme,
    C6ReadinessBlocker, C6RequiredAdmission,
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
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum C6CellExpectationWire {
    Executable {
        source_fixture_id: String,
        mechanism_requirements: BTreeMap<ReferenceThemeMechanism, C6ExpectedMechanismDisposition>,
        expected_residual_ids: BTreeSet<String>,
        required_admission: C6RequiredAdmission,
        required_artifact_assertion: C6ArtifactAssertion,
    },
    NotReady {
        blocker: C6ReadinessBlocker,
    },
}

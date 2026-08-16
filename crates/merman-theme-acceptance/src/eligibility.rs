use std::collections::BTreeSet;

use merman::{RenderArtifactKind, TargetAdmissionReceipt, TargetAdmissionStatus, TargetFontSource};
use merman_theme_fixtures::{
    C6_ACCEPTANCE_CELL_COUNT, C6_ACCEPTANCE_RENDER_GROUP_COUNT, C6_NATIVE_OUTPUT_TARGETS,
    C6_PROOF_FAMILIES, C6_PROOF_THEMES, C6_REQUIRED_CRITICAL_MECHANISMS, C6AcceptanceCatalog,
    C6CellKey, C6ProofRecipeKey, ExpectedOutputTarget, ReferenceThemeMechanism,
};
use sha2::{Digest, Sha256};

use crate::observation::{C6EvaluatedLedger, C6ObservedMechanismDisposition, C6RuntimeError};

/// Alpha receipt proving the exact representative native C6a ledger.
///
/// The receipt has no public constructor. Only the private eligibility issuer can create it after
/// evaluating the immutable manifest lineage and all 18 Standalone SVG/PNG cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C6aEligibilityReceipt {
    verified_cell_count: usize,
    render_group_count: usize,
    manifest_revision: Box<str>,
    previous_manifest_digest: [u8; 32],
    manifest_digest: [u8; 32],
    execution_digest: [u8; 32],
    proof_recipe_revisions_digest: [u8; 32],
    critical_mechanisms_digest: [u8; 32],
    receipt_digest: [u8; 32],
}

impl C6aEligibilityReceipt {
    pub const fn verified_cell_count(&self) -> usize {
        self.verified_cell_count
    }

    pub const fn render_group_count(&self) -> usize {
        self.render_group_count
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

    pub const fn execution_digest(&self) -> &[u8; 32] {
        &self.execution_digest
    }

    pub const fn proof_recipe_revisions_digest(&self) -> &[u8; 32] {
        &self.proof_recipe_revisions_digest
    }

    pub const fn critical_mechanisms_digest(&self) -> &[u8; 32] {
        &self.critical_mechanisms_digest
    }

    pub const fn receipt_digest(&self) -> &[u8; 32] {
        &self.receipt_digest
    }
}

pub(crate) fn issue_c6a_eligibility(
    ledger: C6EvaluatedLedger,
    acceptance: &C6AcceptanceCatalog,
) -> Result<C6aEligibilityReceipt, C6RuntimeError> {
    require_eligibility(
        "schema-version",
        acceptance.schema_version() == merman_theme_fixtures::C6_ACCEPTANCE_SCHEMA_VERSION,
    )?;
    require_eligibility(
        "exact-cell-inventory",
        acceptance.specification().cells().len() == C6_ACCEPTANCE_CELL_COUNT,
    )?;
    require_eligibility(
        "no-deferred-cells",
        acceptance.enforced_tranche().deferred_cells().count() == 0,
    )?;
    require_eligibility(
        "exact-enforced-inventory",
        acceptance.enforced_tranche().cells().len() == C6_ACCEPTANCE_CELL_COUNT,
    )?;
    require_eligibility(
        "exact-proof-recipe-inventory",
        acceptance.proof_recipes().len() == C6_ACCEPTANCE_RENDER_GROUP_COUNT,
    )?;
    require_eligibility(
        "exact-critical-mechanism-inventory",
        acceptance.required_critical_mechanisms()
            == &C6_REQUIRED_CRITICAL_MECHANISMS
                .into_iter()
                .collect::<BTreeSet<_>>(),
    )?;
    require_eligibility(
        "exact-evaluated-cells",
        ledger.cells().len() == C6_ACCEPTANCE_CELL_COUNT,
    )?;
    require_eligibility(
        "exact-evaluated-groups",
        ledger.groups().len() == C6_ACCEPTANCE_RENDER_GROUP_COUNT,
    )?;

    let expected_cells = expected_cell_keys();
    require_eligibility(
        "exact-cell-keys",
        ledger.cells().keys().copied().collect::<BTreeSet<_>>() == expected_cells,
    )?;
    let expected_groups = acceptance
        .proof_recipes()
        .map(|recipe| {
            (
                recipe.key().theme(),
                recipe.key().family(),
                recipe.source_fixture_id().to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    let actual_groups = ledger
        .groups()
        .keys()
        .map(|key| {
            (
                key.theme(),
                key.family(),
                key.source_fixture_id().to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    require_eligibility(
        "exact-render-group-identities",
        actual_groups == expected_groups,
    )?;

    let mut applied_mechanisms = BTreeSet::new();
    let mut proof_recipe_digest_input = b"merman.c6a-proof-recipes.v1\0".to_vec();
    for (group_key, group) in ledger.groups() {
        let proof_recipe = acceptance
            .proof_recipe(C6ProofRecipeKey::new(group_key.theme(), group_key.family()))
            .ok_or(C6RuntimeError::EligibilityInvariant {
                field: "proof-recipe",
            })?;
        require_eligibility(
            "proof-recipe-source",
            proof_recipe.source_fixture_id() == group_key.source_fixture_id(),
        )?;
        require_eligibility(
            "proof-recipe-revision",
            proof_recipe.proof_recipe_revision() == group.proof_recipe_revision(),
        )?;
        require_eligibility("recipe-fingerprint", group.recipe_fingerprint() != &[0; 32])?;
        require_eligibility(
            "shared-document-digest",
            group.shared_document_digest() != &[0; 32],
        )?;
        require_eligibility("render-group-digest", group.digest() != &[0; 32])?;

        append_len_prefixed(
            &mut proof_recipe_digest_input,
            group_key.theme().reference_name().as_bytes(),
        );
        append_len_prefixed(
            &mut proof_recipe_digest_input,
            group_key.family().as_str().as_bytes(),
        );
        append_len_prefixed(
            &mut proof_recipe_digest_input,
            group_key.source_fixture_id().as_bytes(),
        );
        append_len_prefixed(
            &mut proof_recipe_digest_input,
            group.proof_recipe_revision().as_bytes(),
        );
        proof_recipe_digest_input.extend_from_slice(group.recipe_fingerprint());
        proof_recipe_digest_input.extend_from_slice(group.digest());

        let group_cells = ledger
            .cells()
            .values()
            .filter(|cell| cell.render_group_key() == group_key)
            .collect::<Vec<_>>();
        require_eligibility("paired-target-count", group_cells.len() == 2)?;
        require_eligibility(
            "paired-target-set",
            group_cells
                .iter()
                .map(|cell| cell.key().target())
                .collect::<BTreeSet<_>>()
                == C6_NATIVE_OUTPUT_TARGETS
                    .into_iter()
                    .collect::<BTreeSet<_>>(),
        )?;

        let mut shared_resource = None;
        let mut shared_fonts = None;
        for cell in group_cells {
            require_eligibility(
                "cell-render-group-digest",
                cell.render_group_digest() == group.digest(),
            )?;
            require_eligibility("cell-receipt-digest", cell.digest() != &[0; 32])?;
            let target = cell.target_receipt();
            validate_target_receipt(cell.key(), target)?;
            require_eligibility(
                "cell-document-identity",
                target.document_digest() == *group.shared_document_digest(),
            )?;
            require_shared_identity(
                "shared-resource-fingerprint",
                &mut shared_resource,
                *target.resource_fingerprint().as_bytes(),
            )?;
            require_shared_identity(
                "shared-font-catalog-fingerprint",
                &mut shared_fonts,
                *target.font_catalog_fingerprint().as_bytes(),
            )?;
            require_eligibility("no-residuals", cell.residual_ids().is_empty())?;
            require_eligibility(
                "positive-mechanism-dispositions",
                cell.mechanism_dispositions()
                    .values()
                    .all(|disposition| *disposition == C6ObservedMechanismDisposition::Applied),
            )?;
            applied_mechanisms.extend(cell.mechanism_dispositions().keys().copied());
        }
    }

    require_eligibility(
        "critical-mechanism-union",
        acceptance
            .required_critical_mechanisms()
            .is_subset(&applied_mechanisms),
    )?;
    let proof_recipe_revisions_digest = sha256(proof_recipe_digest_input);
    let critical_mechanisms_digest = critical_mechanisms_digest(
        acceptance.required_critical_mechanisms(),
        &applied_mechanisms,
    );
    let report = ledger.execution_report();
    require_eligibility(
        "execution-cell-count",
        report.verified_cell_count() == C6_ACCEPTANCE_CELL_COUNT,
    )?;
    require_eligibility(
        "execution-group-count",
        report.render_group_count() == C6_ACCEPTANCE_RENDER_GROUP_COUNT,
    )?;
    require_eligibility(
        "execution-manifest",
        report.manifest_digest() == acceptance.manifest_digest(),
    )?;
    require_eligibility("execution-digest", report.execution_digest() != &[0; 32])?;

    let mut receipt = C6aEligibilityReceipt {
        verified_cell_count: C6_ACCEPTANCE_CELL_COUNT,
        render_group_count: C6_ACCEPTANCE_RENDER_GROUP_COUNT,
        manifest_revision: acceptance.manifest_revision().into(),
        previous_manifest_digest: *acceptance.previous_manifest_digest(),
        manifest_digest: *acceptance.manifest_digest(),
        execution_digest: *report.execution_digest(),
        proof_recipe_revisions_digest,
        critical_mechanisms_digest,
        receipt_digest: [0; 32],
    };
    receipt.receipt_digest = receipt.canonical_digest();
    Ok(receipt)
}

impl C6aEligibilityReceipt {
    fn canonical_digest(&self) -> [u8; 32] {
        let mut value = b"merman.c6a-eligibility-receipt.v1\0".to_vec();
        append_len_prefixed(&mut value, self.manifest_revision.as_bytes());
        value.extend_from_slice(&self.previous_manifest_digest);
        value.extend_from_slice(&self.manifest_digest);
        value.extend_from_slice(&self.execution_digest);
        value.extend_from_slice(&self.proof_recipe_revisions_digest);
        value.extend_from_slice(&self.critical_mechanisms_digest);
        value.extend_from_slice(&usize_to_u64(self.verified_cell_count).to_be_bytes());
        value.extend_from_slice(&usize_to_u64(self.render_group_count).to_be_bytes());
        sha256(value)
    }
}

fn validate_target_receipt(
    key: C6CellKey,
    receipt: &TargetAdmissionReceipt,
) -> Result<(), C6RuntimeError> {
    require_eligibility(
        "target-artifact-kind",
        receipt.artifact_kind() == expected_artifact_kind(key.target()),
    )?;
    require_eligibility(
        "target-portable",
        receipt.status() == TargetAdmissionStatus::Portable,
    )?;
    require_eligibility("target-admission-reasons", receipt.reasons().is_empty())?;
    require_eligibility(
        "target-font-source",
        receipt.font_source() == TargetFontSource::Embedded,
    )?;
    require_eligibility(
        "target-document-digest",
        receipt.document_digest() != [0; 32],
    )?;
    require_eligibility(
        "target-evidence-digest",
        receipt.target_evidence_digest() != [0; 32],
    )?;
    require_eligibility(
        "target-artifact-digest",
        receipt.artifact_digest() != [0; 32],
    )?;
    require_eligibility("target-receipt-digest", receipt.receipt_digest() != [0; 32])?;
    require_eligibility(
        "target-resource-fingerprint",
        receipt.resource_fingerprint().as_bytes() != &[0; 32],
    )?;
    require_eligibility(
        "target-font-catalog-fingerprint",
        receipt.font_catalog_fingerprint().as_bytes() != &[0; 32],
    )
}

fn require_shared_identity(
    field: &'static str,
    shared: &mut Option<[u8; 32]>,
    actual: [u8; 32],
) -> Result<(), C6RuntimeError> {
    match shared {
        Some(expected) => require_eligibility(field, *expected == actual),
        None => {
            *shared = Some(actual);
            Ok(())
        }
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

fn expected_artifact_kind(target: ExpectedOutputTarget) -> RenderArtifactKind {
    match target {
        ExpectedOutputTarget::StandaloneSvg => RenderArtifactKind::Svg,
        ExpectedOutputTarget::Png => RenderArtifactKind::Png,
        ExpectedOutputTarget::BrowserSvg
        | ExpectedOutputTarget::Jpeg
        | ExpectedOutputTarget::Pdf => {
            unreachable!("C6a excludes non-native representative targets")
        }
    }
}

fn critical_mechanisms_digest(
    required: &BTreeSet<ReferenceThemeMechanism>,
    observed: &BTreeSet<ReferenceThemeMechanism>,
) -> [u8; 32] {
    let mut value = b"merman.c6a-critical-mechanisms.v1\0".to_vec();
    value.extend_from_slice(&usize_to_u64(required.len()).to_be_bytes());
    for mechanism in required {
        append_len_prefixed(&mut value, mechanism_id(*mechanism).as_bytes());
    }
    value.extend_from_slice(&usize_to_u64(observed.len()).to_be_bytes());
    for mechanism in observed {
        append_len_prefixed(&mut value, mechanism_id(*mechanism).as_bytes());
    }
    sha256(value)
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

fn require_eligibility(field: &'static str, condition: bool) -> Result<(), C6RuntimeError> {
    if condition {
        Ok(())
    } else {
        Err(C6RuntimeError::EligibilityInvariant { field })
    }
}

fn append_len_prefixed(value: &mut Vec<u8>, bytes: &[u8]) {
    value.extend_from_slice(&usize_to_u64(bytes.len()).to_be_bytes());
    value.extend_from_slice(bytes);
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).expect("C6a eligibility values fit in u64")
}

fn sha256(value: impl AsRef<[u8]>) -> [u8; 32] {
    Sha256::digest(value.as_ref()).into()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use merman_theme_fixtures::{C6_ACCEPTANCE_RELATIVE_PATH, ThemeFixtureCatalog};
    use serde_json::{Value, json};

    use super::*;
    use crate::runner::run_catalog_evaluated;

    fn themes_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("themes")
    }

    fn catalogs() -> (ThemeFixtureCatalog, C6AcceptanceCatalog) {
        let themes = ThemeFixtureCatalog::load(themes_root()).expect("load theme fixtures");
        let acceptance = C6AcceptanceCatalog::load(&themes).expect("load C6 acceptance");
        (themes, acceptance)
    }

    fn acceptance_value() -> Value {
        serde_json::from_str(
            &fs::read_to_string(themes_root().join(C6_ACCEPTANCE_RELATIVE_PATH))
                .expect("read C6 acceptance manifest"),
        )
        .expect("parse C6 acceptance manifest")
    }

    fn assert_invariant(error: C6RuntimeError, expected: &'static str) {
        match error {
            C6RuntimeError::EligibilityInvariant { field } => assert_eq!(field, expected),
            other => panic!("expected eligibility invariant {expected}, got {other}"),
        }
    }

    #[test]
    fn issuer_rejects_non_exact_ledgers_and_manifests() {
        let (themes, acceptance) = catalogs();
        let complete =
            run_catalog_evaluated(&themes, &acceptance).expect("evaluate complete C6 ledger");

        let mut partial = complete.clone();
        let png_key = partial
            .cells()
            .keys()
            .copied()
            .find(|key| key.target() == ExpectedOutputTarget::Png)
            .expect("PNG cell");
        partial.cells_mut().remove(&png_key);

        assert_invariant(
            issue_c6a_eligibility(partial, &acceptance)
                .expect_err("partial ledger must not qualify"),
            "exact-evaluated-cells",
        );

        let mut deferred = acceptance_value();
        deferred["cells"][0]["enforcement"] = json!({
            "kind": "deferred",
            "blocker": "family-adapter-incomplete"
        });
        let deferred = C6AcceptanceCatalog::from_json(
            &serde_json::to_string(&deferred).expect("serialize deferred manifest"),
            &themes,
        )
        .expect("deferred progress manifest remains structurally valid");
        assert_invariant(
            issue_c6a_eligibility(complete.clone(), &deferred)
                .expect_err("deferred cells must not qualify"),
            "no-deferred-cells",
        );

        let mut changed_revision = acceptance_value();
        changed_revision["proofRecipes"][0]["proofRecipeRevision"] =
            json!("brutalist-flowchart-v2");
        let changed_revision = C6AcceptanceCatalog::from_json(
            &serde_json::to_string(&changed_revision).expect("serialize changed recipe revision"),
            &themes,
        )
        .expect("new proof recipe revision remains structurally valid");
        assert_invariant(
            issue_c6a_eligibility(complete.clone(), &changed_revision)
                .expect_err("unexecuted proof recipe revision must not qualify"),
            "proof-recipe-revision",
        );

        let png_receipt = complete
            .cells()
            .values()
            .find(|cell| cell.key().target() == ExpectedOutputTarget::Png)
            .expect("PNG cell")
            .target_receipt()
            .clone();
        let mut wrong_target = complete.clone();
        wrong_target
            .cells_mut()
            .values_mut()
            .find(|cell| cell.key().target() == ExpectedOutputTarget::StandaloneSvg)
            .expect("SVG cell")
            .replace_target_receipt(png_receipt);
        assert_invariant(
            issue_c6a_eligibility(wrong_target, &acceptance)
                .expect_err("wrong target receipt kind must not qualify"),
            "target-artifact-kind",
        );

        let png_cells = complete
            .cells()
            .values()
            .filter(|cell| cell.key().target() == ExpectedOutputTarget::Png)
            .take(2)
            .map(|cell| (cell.key(), cell.target_receipt().clone()))
            .collect::<Vec<_>>();
        assert_eq!(png_cells.len(), 2);
        let mut wrong_document = complete.clone();
        wrong_document
            .cells_mut()
            .get_mut(&png_cells[1].0)
            .expect("second PNG cell")
            .replace_target_receipt(png_cells[0].1.clone());
        assert_invariant(
            issue_c6a_eligibility(wrong_document, &acceptance)
                .expect_err("cross-group document identity must not qualify"),
            "cell-document-identity",
        );

        let mut missing_critical = complete;
        for cell in missing_critical.cells_mut().values_mut() {
            cell.remove_mechanism(ReferenceThemeMechanism::CanvasBlend);
        }
        assert_invariant(
            issue_c6a_eligibility(missing_critical, &acceptance)
                .expect_err("missing critical mechanism union must not qualify"),
            "critical-mechanism-union",
        );
    }
}

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

use merman::svg::ThemeRecipeFingerprint;
use merman::{
    DiagramFamilyId, RenderArtifactKind, RenderEvidence, TargetAdmissionReceipt,
    TargetAdmissionStatus, TargetFontSource,
};
use merman_theme_fixtures::{
    C6AcceptanceCatalog, C6CellExpectation, C6CellKey, C6EnforcedCell, C6ProofFamily, C6ProofTheme,
    CatalogError, ExpectedOutputTarget, ReferenceThemeMechanism, ThemeFixtureCatalog,
};

use crate::cutover::usize_to_u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum C6ObservedMechanismDisposition {
    Applied,
    NotApplicable,
    Rejected,
    Residual,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct C6RenderIdentity {
    actual_family: DiagramFamilyId,
    recipe_fingerprint: Option<ThemeRecipeFingerprint>,
}

impl C6RenderIdentity {
    pub(crate) fn from_evidence(evidence: &RenderEvidence) -> Self {
        Self {
            actual_family: evidence.family_id(),
            recipe_fingerprint: evidence.theme_recipe_fingerprint(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct C6RenderGroupKey {
    theme: C6ProofTheme,
    family: C6ProofFamily,
    source_fixture_id: String,
}

impl C6RenderGroupKey {
    pub(crate) fn for_cell(cell: &C6EnforcedCell) -> Self {
        Self {
            theme: cell.key().theme(),
            family: cell.key().family(),
            source_fixture_id: cell.source_fixture_id().to_owned(),
        }
    }

    pub(crate) const fn theme(&self) -> C6ProofTheme {
        self.theme
    }

    pub(crate) const fn family(&self) -> C6ProofFamily {
        self.family
    }

    pub(crate) fn source_fixture_id(&self) -> &str {
        &self.source_fixture_id
    }

    pub(crate) fn label(&self) -> String {
        format!(
            "{}/{}/{}",
            proof_theme_id(self.theme),
            proof_family_id(self.family),
            self.source_fixture_id
        )
    }
}

/// Receipt for one deterministic render group shared by one or more target cells.
///
/// The receipt is sealed from the hash-validated fixture, typed theme input, compiled recipe,
/// completed document identity, and the target-owned standalone SVG receipt. Target adapters can
/// only refer to this receipt by its canonical digest; they cannot substitute a batch-global
/// identity or reconstruct the target receipt protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct C6RenderGroupReceipt {
    key: C6RenderGroupKey,
    source_sha256: [u8; 32],
    theme_input_sha256: [u8; 32],
    recipe_fingerprint: [u8; 32],
    // Cross-target join key for projections of the same completed document.
    shared_document_digest: [u8; 32],
    svg_target_receipt_digest: [u8; 32],
    digest: [u8; 32],
}

impl C6RenderGroupReceipt {
    pub(crate) fn seal(
        key: C6RenderGroupKey,
        catalog: &ThemeFixtureCatalog,
        identity: &C6RenderIdentity,
        standalone_receipt: &TargetAdmissionReceipt,
    ) -> Result<Self, C6RuntimeError> {
        let fixture = catalog
            .fixture(key.source_fixture_id())
            .ok_or_else(|| group_evidence_mismatch(&key, "source-fixture"))?;
        require_group_evidence(
            &key,
            "render-family",
            family_id_for_proof(key.family()) == identity.actual_family,
        )?;
        let source_sha256 = parse_hex_digest(fixture.source_sha256())
            .ok_or_else(|| group_evidence_mismatch(&key, "source-sha256"))?;
        let source = catalog
            .source_text(fixture.id())
            .map_err(|error| group_proof_failed(&key, "source-fixture-seal", error.to_string()))?;
        require_group_evidence(
            &key,
            "source-sha256",
            sha256(source.as_bytes()) == source_sha256,
        )?;
        let theme_input_sha256 = parse_hex_digest(
            fixture
                .theme_input_sha256()
                .ok_or_else(|| group_evidence_mismatch(&key, "theme-input-sha256"))?,
        )
        .ok_or_else(|| group_evidence_mismatch(&key, "theme-input-sha256"))?;
        let recipe_fingerprint = identity
            .recipe_fingerprint
            .ok_or_else(|| group_evidence_mismatch(&key, "recipe-fingerprint"))?;
        require_group_evidence(
            &key,
            "standalone-target",
            standalone_receipt.artifact_kind() == RenderArtifactKind::Svg,
        )?;
        require_group_evidence(
            &key,
            "document-digest",
            standalone_receipt.document_digest() != [0; 32],
        )?;
        require_group_evidence(
            &key,
            "target-evidence-digest",
            standalone_receipt.target_evidence_digest() != [0; 32],
        )?;
        require_group_evidence(
            &key,
            "artifact-digest",
            standalone_receipt.artifact_digest() != [0; 32],
        )?;
        require_group_evidence(
            &key,
            "resource-fingerprint",
            standalone_receipt.resource_fingerprint().as_bytes() != &[0; 32],
        )?;
        require_group_evidence(
            &key,
            "font-catalog-fingerprint",
            standalone_receipt.font_catalog_fingerprint().as_bytes() != &[0; 32],
        )?;
        require_group_evidence(
            &key,
            "target-receipt-digest",
            standalone_receipt.receipt_digest() != [0; 32],
        )?;
        let mut receipt = Self {
            key,
            source_sha256,
            theme_input_sha256,
            recipe_fingerprint: *recipe_fingerprint.as_bytes(),
            shared_document_digest: standalone_receipt.document_digest(),
            svg_target_receipt_digest: standalone_receipt.receipt_digest(),
            digest: [0; 32],
        };
        receipt.digest = receipt.canonical_digest();
        Ok(receipt)
    }

    fn matches_render(
        &self,
        identity: &C6RenderIdentity,
        target_receipt: &TargetAdmissionReceipt,
    ) -> bool {
        family_id_for_proof(self.key.family()) == identity.actual_family
            && identity
                .recipe_fingerprint
                .map(|fingerprint| *fingerprint.as_bytes() == self.recipe_fingerprint)
                .unwrap_or(false)
            && target_receipt.document_digest() == self.shared_document_digest
    }

    fn validate(&self, catalog: &ThemeFixtureCatalog) -> Result<(), C6RuntimeError> {
        let fixture = catalog
            .fixture(self.key.source_fixture_id())
            .ok_or_else(|| group_evidence_mismatch(&self.key, "source-fixture"))?;
        let source = catalog.source_text(fixture.id()).map_err(|error| {
            group_proof_failed(&self.key, "source-fixture-revalidate", error.to_string())
        })?;
        require_group_evidence(
            &self.key,
            "source-sha256",
            parse_hex_digest(fixture.source_sha256()) == Some(self.source_sha256)
                && sha256(source.as_bytes()) == self.source_sha256,
        )?;
        require_group_evidence(
            &self.key,
            "theme-input-sha256",
            fixture.theme_input_sha256().and_then(parse_hex_digest)
                == Some(self.theme_input_sha256),
        )?;
        require_group_evidence(
            &self.key,
            "render-group-digest",
            self.digest == self.canonical_digest(),
        )
    }

    fn canonical_digest(&self) -> [u8; 32] {
        let mut value = b"merman.c6-render-group-receipt.v7\0".to_vec();
        encode_render_group_key(&mut value, &self.key);
        value.extend_from_slice(&self.source_sha256);
        value.extend_from_slice(&self.theme_input_sha256);
        value.extend_from_slice(&self.recipe_fingerprint);
        value.extend_from_slice(&self.shared_document_digest);
        value.extend_from_slice(&self.svg_target_receipt_digest);
        sha256(value)
    }
}

/// Target-local semantic proof produced only after the acceptance observer checks final bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct C6TargetProof {
    semantic_assertion_id: &'static str,
    mechanism_dispositions: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
}

impl C6TargetProof {
    pub(crate) fn brutalist_state_standalone_svg(
        mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    ) -> Self {
        Self::seal("brutalist-state-standalone-svg-v1", mechanisms)
    }

    pub(crate) fn brutalist_state_png(
        mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    ) -> Self {
        Self::seal("brutalist-state-png-v1", mechanisms)
    }

    fn seal(
        semantic_assertion_id: &'static str,
        mechanism_dispositions: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    ) -> Self {
        Self {
            semantic_assertion_id,
            mechanism_dispositions,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct C6CellReceipt {
    key: C6CellKey,
    render_group_key: C6RenderGroupKey,
    render_group_digest: [u8; 32],
    // Target-owned identity; observation never re-encodes its constituent fields.
    target_receipt_digest: [u8; 32],
    semantic_assertion_id: String,
    mechanism_dispositions: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    residual_ids: BTreeSet<String>,
    digest: [u8; 32],
}

pub(crate) fn seal_cell_from_evidence(
    expectation: &C6CellExpectation,
    enforced: &C6EnforcedCell,
    group: &C6RenderGroupReceipt,
    identity: C6RenderIdentity,
    target_receipt: TargetAdmissionReceipt,
    residual_ids: BTreeSet<String>,
    proof: C6TargetProof,
) -> Result<C6CellReceipt, C6RuntimeError> {
    let key = enforced.key();
    require_evidence(
        key,
        "render-group-key",
        group.key == C6RenderGroupKey::for_cell(enforced),
    )?;
    require_evidence(
        key,
        "semantic-assertion-id",
        proof.semantic_assertion_id == expectation.semantic_assertion_id(),
    )?;
    require_evidence(
        key,
        "target-admission",
        target_receipt.artifact_kind() == render_target_for_expected(enforced.key().target()),
    )?;
    require_evidence(
        key,
        "document-digest",
        target_receipt.document_digest() != [0; 32],
    )?;
    require_evidence(
        key,
        "target-evidence-digest",
        target_receipt.target_evidence_digest() != [0; 32],
    )?;
    require_evidence(
        key,
        "artifact-digest",
        target_receipt.artifact_digest() != [0; 32],
    )?;
    require_evidence(
        key,
        "resource-fingerprint",
        target_receipt.resource_fingerprint().as_bytes() != &[0; 32],
    )?;
    require_evidence(
        key,
        "font-catalog-fingerprint",
        target_receipt.font_catalog_fingerprint().as_bytes() != &[0; 32],
    )?;
    require_evidence(
        key,
        "render-group-evidence",
        group.matches_render(&identity, &target_receipt),
    )?;
    require_evidence(
        key,
        "target-admission",
        target_receipt.status() == TargetAdmissionStatus::Portable,
    )?;
    require_evidence(
        key,
        "target-admission-reasons",
        target_receipt.reasons().is_empty(),
    )?;
    require_evidence(
        key,
        "font-source",
        target_receipt.font_source() == TargetFontSource::Embedded,
    )?;
    require_evidence(
        key,
        "target-receipt-digest",
        target_receipt.receipt_digest() != [0; 32],
    )?;
    if key.target() == ExpectedOutputTarget::StandaloneSvg {
        require_evidence(
            key,
            "target-receipt-digest",
            target_receipt.receipt_digest() == group.svg_target_receipt_digest,
        )?;
    }
    let mut receipt = C6CellReceipt {
        key,
        render_group_key: group.key.clone(),
        render_group_digest: group.digest,
        target_receipt_digest: target_receipt.receipt_digest(),
        semantic_assertion_id: proof.semantic_assertion_id.to_owned(),
        mechanism_dispositions: proof.mechanism_dispositions,
        residual_ids,
        digest: [0; 32],
    };
    receipt.digest = receipt.canonical_digest();
    Ok(receipt)
}

impl C6CellReceipt {
    fn canonical_digest(&self) -> [u8; 32] {
        let mut value = b"merman.c6-cell-receipt.v7\0".to_vec();
        encode_cell_key(&mut value, self.key);
        encode_render_group_key(&mut value, &self.render_group_key);
        value.extend_from_slice(&self.render_group_digest);
        value.extend_from_slice(&self.target_receipt_digest);
        append_len_prefixed(&mut value, self.semantic_assertion_id.as_bytes());
        append_len_prefixed(&mut value, b"mechanism-dispositions");
        value.extend_from_slice(&mechanism_digest(&self.mechanism_dispositions));
        append_len_prefixed(&mut value, b"residual-ids");
        value.extend_from_slice(&usize_to_u64(self.residual_ids.len()).to_be_bytes());
        for residual_id in &self.residual_ids {
            append_len_prefixed(&mut value, residual_id.as_bytes());
        }
        sha256(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct C6ReceiptBook {
    groups: BTreeMap<C6RenderGroupKey, C6RenderGroupReceipt>,
    cells: BTreeMap<C6CellKey, C6CellReceipt>,
}

impl C6ReceiptBook {
    pub(crate) fn from_receipts(
        expected_keys: impl IntoIterator<Item = C6CellKey>,
        groups: impl IntoIterator<Item = C6RenderGroupReceipt>,
        cells: impl IntoIterator<Item = C6CellReceipt>,
    ) -> Result<Self, C6RuntimeError> {
        let mut indexed_groups = BTreeMap::new();
        for group in groups {
            let key = group.key.clone();
            if indexed_groups.insert(key.clone(), group).is_some() {
                return Err(C6RuntimeError::DuplicateRenderGroup { group: key.label() });
            }
        }

        let mut indexed_cells = BTreeMap::new();
        for cell in cells {
            let key = cell.key;
            if indexed_cells.insert(key, cell).is_some() {
                return Err(C6RuntimeError::DuplicateCellReceipt { key });
            }
        }

        let expected = expected_keys.into_iter().collect::<BTreeSet<_>>();
        let actual = indexed_cells.keys().copied().collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(C6RuntimeError::ReceiptCoverageMismatch {
                missing: expected.difference(&actual).copied().collect(),
                unexpected: actual.difference(&expected).copied().collect(),
            });
        }

        let mut referenced_groups = BTreeSet::new();
        for cell in indexed_cells.values() {
            let group = indexed_groups
                .get(&cell.render_group_key)
                .ok_or_else(|| evidence_mismatch(cell.key, "render-group-reference"))?;
            require_evidence(
                cell.key,
                "render-group-reference",
                cell.render_group_digest == group.digest,
            )?;
            if cell.key.target() == ExpectedOutputTarget::StandaloneSvg {
                require_evidence(
                    cell.key,
                    "target-receipt-digest",
                    cell.target_receipt_digest == group.svg_target_receipt_digest,
                )?;
            }
            referenced_groups.insert(cell.render_group_key.clone());
        }
        if let Some(group) = indexed_groups
            .keys()
            .find(|key| !referenced_groups.contains(*key))
        {
            return Err(C6RuntimeError::UnreferencedRenderGroup {
                group: group.label(),
            });
        }

        Ok(Self {
            groups: indexed_groups,
            cells: indexed_cells,
        })
    }

    pub(crate) fn evaluate(
        &self,
        acceptance: &C6AcceptanceCatalog,
        theme_catalog: &ThemeFixtureCatalog,
    ) -> Result<C6ExecutionReport, C6RuntimeError> {
        if self.cells.is_empty() {
            return Err(C6RuntimeError::EmptyTranche);
        }
        let expected = acceptance
            .enforced_tranche()
            .cells()
            .map(C6EnforcedCell::key)
            .collect::<BTreeSet<_>>();
        let actual = self.cells.keys().copied().collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(C6RuntimeError::ReceiptCoverageMismatch {
                missing: expected.difference(&actual).copied().collect(),
                unexpected: actual.difference(&expected).copied().collect(),
            });
        }

        for group in self.groups.values() {
            group.validate(theme_catalog)?;
        }

        for enforced in acceptance.enforced_tranche().cells() {
            let key = enforced.key();
            let receipt = self
                .cells
                .get(&key)
                .ok_or_else(|| evidence_mismatch(key, "cell-receipt"))?;
            let expectation = acceptance
                .specification()
                .cell(key)
                .ok_or_else(|| evidence_mismatch(key, "cell-expectation"))?
                .expectation();
            let expected_group_key = C6RenderGroupKey::for_cell(enforced);
            let group = self
                .groups
                .get(&expected_group_key)
                .ok_or_else(|| evidence_mismatch(key, "render-group-reference"))?;

            require_evidence(key, "cell-key", receipt.key == key)?;
            require_evidence(
                key,
                "render-group-key",
                receipt.render_group_key == expected_group_key,
            )?;
            require_evidence(
                key,
                "render-group-digest",
                receipt.render_group_digest == group.digest,
            )?;
            if key.target() == ExpectedOutputTarget::StandaloneSvg {
                require_evidence(
                    key,
                    "target-receipt-digest",
                    receipt.target_receipt_digest == group.svg_target_receipt_digest,
                )?;
            }
            require_evidence(
                key,
                "mechanism-count",
                receipt.mechanism_dispositions.len() == expectation.mechanism_requirements().len(),
            )?;
            for (mechanism, disposition) in expectation.mechanism_requirements() {
                require_evidence(
                    key,
                    "mechanism-disposition",
                    receipt.mechanism_dispositions.get(mechanism)
                        == Some(&expected_observed_disposition(*disposition)),
                )?;
            }
            require_evidence(
                key,
                "residual-ids",
                receipt.residual_ids == *expectation.expected_residual_ids(),
            )?;
            require_evidence(
                key,
                "target-receipt-digest",
                receipt.target_receipt_digest != [0; 32],
            )?;
            require_evidence(
                key,
                "semantic-assertion-id",
                receipt.semantic_assertion_id == expectation.semantic_assertion_id(),
            )?;
            require_evidence(
                key,
                "cell-receipt-digest",
                receipt.digest == receipt.canonical_digest(),
            )?;
        }

        let mut digest_input = b"merman.c6-execution-report.v7\0".to_vec();
        digest_input.extend_from_slice(&acceptance.schema_version().to_be_bytes());
        digest_input.extend_from_slice(acceptance.manifest_digest());
        for group in self.groups.values() {
            encode_render_group_key(&mut digest_input, &group.key);
            digest_input.extend_from_slice(&group.digest);
        }
        for receipt in self.cells.values() {
            encode_cell_key(&mut digest_input, receipt.key);
            digest_input.extend_from_slice(&receipt.digest);
        }
        Ok(C6ExecutionReport {
            verified_cell_count: self.cells.len(),
            render_group_count: self.groups.len(),
            manifest_digest: *acceptance.manifest_digest(),
            execution_digest: sha256(digest_input),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C6ExecutionReport {
    verified_cell_count: usize,
    render_group_count: usize,
    manifest_digest: [u8; 32],
    execution_digest: [u8; 32],
}

impl C6ExecutionReport {
    pub const fn verified_cell_count(&self) -> usize {
        self.verified_cell_count
    }

    pub const fn render_group_count(&self) -> usize {
        self.render_group_count
    }

    pub const fn manifest_digest(&self) -> &[u8; 32] {
        &self.manifest_digest
    }

    pub const fn execution_digest(&self) -> &[u8; 32] {
        &self.execution_digest
    }
}

#[derive(Debug, thiserror::Error)]
pub enum C6RuntimeError {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error("the enforced C6 tranche is empty")]
    EmptyTranche,
    #[error("no render-group or target adapter exists for enforced C6 cell {key:?}")]
    UnsupportedEnforcedCell { key: C6CellKey },
    #[error("multiple C6 render-group adapters match `{group}`")]
    AmbiguousRenderGroupAdapter { group: String },
    #[error("C6 render group `{group}` failed invariant `{field}`")]
    RenderGroupEvidenceMismatch { group: String, field: &'static str },
    #[error("C6 render group `{group}` failed proof stage `{stage}`: {detail}")]
    RenderGroupProofFailed {
        group: String,
        stage: &'static str,
        detail: String,
    },
    #[error(
        "C6 render group `{group}` failed proof stage `{stage}` for artifact target {artifact_target:?}, required by {required_by:?}: {detail}"
    )]
    RenderGroupTargetProofFailed {
        group: String,
        artifact_target: ExpectedOutputTarget,
        required_by: Vec<ExpectedOutputTarget>,
        stage: &'static str,
        detail: String,
    },
    #[error("C6 evidence for {key:?} failed invariant `{field}`")]
    EvidenceMismatch { key: C6CellKey, field: &'static str },
    #[error("the C6 runtime produced duplicate render group `{group}`")]
    DuplicateRenderGroup { group: String },
    #[error("the C6 runtime produced duplicate cell receipt for {key:?}")]
    DuplicateCellReceipt { key: C6CellKey },
    #[error(
        "C6 receipt coverage differed from the enforced tranche; missing={missing:?}, unexpected={unexpected:?}"
    )]
    ReceiptCoverageMismatch {
        missing: Vec<C6CellKey>,
        unexpected: Vec<C6CellKey>,
    },
    #[error("C6 render group `{group}` was not referenced by any cell receipt")]
    UnreferencedRenderGroup { group: String },
}

#[derive(Debug, thiserror::Error)]
pub enum RouteCutoverRuntimeError {
    #[error("route cutover witness `{witness}` failed proof stage `{stage}`: {detail}")]
    ProofFailed {
        witness: String,
        stage: &'static str,
        detail: String,
    },
    #[error("the route cutover runtime produced duplicate receipt `{route}`")]
    DuplicateReceipt { route: String },
    #[error(
        "route cutover coverage differed from the typed legacy-replacing inventory; missing={missing:?}, unexpected={unexpected:?}"
    )]
    CoverageMismatch {
        missing: Vec<String>,
        unexpected: Vec<String>,
    },
}

fn family_id_for_proof(family: C6ProofFamily) -> DiagramFamilyId {
    match family {
        C6ProofFamily::Flowchart => DiagramFamilyId::FLOWCHART,
        C6ProofFamily::Sequence => DiagramFamilyId::SEQUENCE,
        C6ProofFamily::State => DiagramFamilyId::STATE,
    }
}

fn render_target_for_expected(target: ExpectedOutputTarget) -> RenderArtifactKind {
    match target {
        ExpectedOutputTarget::BrowserSvg | ExpectedOutputTarget::StandaloneSvg => {
            RenderArtifactKind::Svg
        }
        ExpectedOutputTarget::Png => RenderArtifactKind::Png,
        ExpectedOutputTarget::Jpeg => RenderArtifactKind::Jpeg,
        ExpectedOutputTarget::Pdf => RenderArtifactKind::Pdf,
    }
}

fn expected_observed_disposition(
    expected: merman_theme_fixtures::C6ExpectedMechanismDisposition,
) -> C6ObservedMechanismDisposition {
    match expected {
        merman_theme_fixtures::C6ExpectedMechanismDisposition::MustApply => {
            C6ObservedMechanismDisposition::Applied
        }
        merman_theme_fixtures::C6ExpectedMechanismDisposition::MustRemainResidual => {
            C6ObservedMechanismDisposition::Residual
        }
        merman_theme_fixtures::C6ExpectedMechanismDisposition::MustReject => {
            C6ObservedMechanismDisposition::Rejected
        }
        merman_theme_fixtures::C6ExpectedMechanismDisposition::NotApplicable => {
            C6ObservedMechanismDisposition::NotApplicable
        }
    }
}

fn mechanism_digest(
    mechanisms: &BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
) -> [u8; 32] {
    let mut value = b"merman.c6-mechanism-witness.v3\0".to_vec();
    value.extend_from_slice(&usize_to_u64(mechanisms.len()).to_be_bytes());
    for (mechanism, disposition) in mechanisms {
        append_len_prefixed(&mut value, reference_mechanism_id(*mechanism).as_bytes());
        append_len_prefixed(
            &mut value,
            mechanism_disposition_id(*disposition).as_bytes(),
        );
    }
    sha256(value)
}

fn encode_render_group_key(output: &mut Vec<u8>, key: &C6RenderGroupKey) {
    append_len_prefixed(output, proof_theme_id(key.theme).as_bytes());
    append_len_prefixed(output, proof_family_id(key.family).as_bytes());
    append_len_prefixed(output, key.source_fixture_id.as_bytes());
}

fn encode_cell_key(output: &mut Vec<u8>, key: C6CellKey) {
    append_len_prefixed(output, proof_theme_id(key.theme()).as_bytes());
    append_len_prefixed(output, proof_family_id(key.family()).as_bytes());
    append_len_prefixed(output, expected_target_id(key.target()).as_bytes());
}

fn proof_theme_id(theme: C6ProofTheme) -> &'static str {
    theme.reference_name()
}

fn proof_family_id(family: C6ProofFamily) -> &'static str {
    family_id_for_proof(family).as_str()
}

fn expected_target_id(target: ExpectedOutputTarget) -> &'static str {
    match target {
        ExpectedOutputTarget::BrowserSvg => "browser-svg",
        ExpectedOutputTarget::StandaloneSvg => "standalone-svg",
        ExpectedOutputTarget::Png => "png",
        ExpectedOutputTarget::Jpeg => "jpeg",
        ExpectedOutputTarget::Pdf => "pdf",
    }
}

fn mechanism_disposition_id(disposition: C6ObservedMechanismDisposition) -> &'static str {
    match disposition {
        C6ObservedMechanismDisposition::Applied => "applied",
        C6ObservedMechanismDisposition::NotApplicable => "not-applicable",
        C6ObservedMechanismDisposition::Rejected => "rejected",
        C6ObservedMechanismDisposition::Residual => "residual",
    }
}

fn reference_mechanism_id(mechanism: ReferenceThemeMechanism) -> &'static str {
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

fn parse_hex_digest(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}

pub(crate) fn sha256(bytes: impl AsRef<[u8]>) -> [u8; 32] {
    Sha256::digest(bytes.as_ref()).into()
}

pub(crate) fn append_len_prefixed(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    output.extend_from_slice(bytes);
}

fn evidence_mismatch(key: C6CellKey, field: &'static str) -> C6RuntimeError {
    C6RuntimeError::EvidenceMismatch { key, field }
}

fn group_evidence_mismatch(key: &C6RenderGroupKey, field: &'static str) -> C6RuntimeError {
    C6RuntimeError::RenderGroupEvidenceMismatch {
        group: key.label(),
        field,
    }
}

fn group_proof_failed(
    key: &C6RenderGroupKey,
    stage: &'static str,
    detail: impl Into<String>,
) -> C6RuntimeError {
    C6RuntimeError::RenderGroupProofFailed {
        group: key.label(),
        stage,
        detail: detail.into(),
    }
}

fn require_evidence(
    key: C6CellKey,
    field: &'static str,
    condition: bool,
) -> Result<(), C6RuntimeError> {
    if condition {
        Ok(())
    } else {
        Err(evidence_mismatch(key, field))
    }
}

fn require_group_evidence(
    key: &C6RenderGroupKey,
    field: &'static str,
    condition: bool,
) -> Result<(), C6RuntimeError> {
    if condition {
        Ok(())
    } else {
        Err(group_evidence_mismatch(key, field))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman::svg::{DiagramThemeCompiler, DiagramThemeSpec, ThemeResourcePolicy};
    use merman::{
        OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgRequest,
    };
    use std::path::{Path, PathBuf};

    fn themes_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("themes")
    }

    fn load_catalogs() -> (ThemeFixtureCatalog, C6AcceptanceCatalog) {
        let themes = ThemeFixtureCatalog::load(themes_root()).expect("load theme fixtures");
        let acceptance = C6AcceptanceCatalog::load(&themes).expect("load C6 acceptance catalog");
        (themes, acceptance)
    }

    fn themed_state_document_with_ceiling(
        ceiling: ThemeResourcePolicy,
    ) -> merman::RenderedDocument {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new())
            .expect("compile synthetic theme");
        let renderer = Renderer::new().with_runtime_policy(
            merman::runtime::RuntimePolicy::deterministic().with_fixed_unix_millis(42),
        );
        let mut svg_request = SvgRequest::default();
        svg_request.environment =
            SvgEnvironment::deterministic().with_theme_resource_ceiling(ceiling);
        let output = renderer
            .render(
                RenderRequest::document(
                    "stateDiagram-v2\n[*] --> Ready\n",
                    OperationControl::new(),
                    svg_request,
                )
                .with_theme(theme),
            )
            .expect("deterministic themed state document should render");
        let RenderOutput::Document(Some(output)) = output else {
            panic!("expected deterministic themed document output");
        };
        output
    }

    fn synthetic_group(
        cell: &C6EnforcedCell,
        themes: &ThemeFixtureCatalog,
        seed: u8,
    ) -> C6RenderGroupReceipt {
        let fixture = themes
            .fixture(cell.source_fixture_id())
            .expect("enforced fixture");
        let mut receipt = C6RenderGroupReceipt {
            key: C6RenderGroupKey::for_cell(cell),
            source_sha256: parse_hex_digest(fixture.source_sha256()).expect("source digest"),
            theme_input_sha256: parse_hex_digest(
                fixture
                    .theme_input_sha256()
                    .expect("enforced theme input digest"),
            )
            .expect("theme input digest"),
            recipe_fingerprint: [seed; 32],
            shared_document_digest: [seed.wrapping_add(1); 32],
            svg_target_receipt_digest: [seed.wrapping_add(5); 32],
            digest: [0; 32],
        };
        receipt.digest = receipt.canonical_digest();
        receipt
    }

    fn synthetic_cell(
        enforced: &C6EnforcedCell,
        acceptance: &C6AcceptanceCatalog,
        group: &C6RenderGroupReceipt,
    ) -> C6CellReceipt {
        let key = enforced.key();
        let expectation = acceptance
            .specification()
            .cell(key)
            .expect("enforced expectation")
            .expectation();
        let mechanism_dispositions = expectation
            .mechanism_requirements()
            .iter()
            .map(|(mechanism, disposition)| {
                (*mechanism, expected_observed_disposition(*disposition))
            })
            .collect::<BTreeMap<_, _>>();
        let target_receipt_digest = if key.target() == ExpectedOutputTarget::StandaloneSvg {
            group.svg_target_receipt_digest
        } else {
            sha256(format!("synthetic-target-receipt-{key:?}"))
        };
        let mut receipt = C6CellReceipt {
            key,
            render_group_key: group.key.clone(),
            render_group_digest: group.digest,
            target_receipt_digest,
            semantic_assertion_id: expectation.semantic_assertion_id().to_owned(),
            mechanism_dispositions,
            residual_ids: expectation.expected_residual_ids().clone(),
            digest: [0; 32],
        };
        receipt.digest = receipt.canonical_digest();
        receipt
    }

    fn synthetic_valid_book(
        themes: &ThemeFixtureCatalog,
        acceptance: &C6AcceptanceCatalog,
    ) -> C6ReceiptBook {
        let first = acceptance
            .enforced_tranche()
            .cells()
            .next()
            .expect("enforced cell");
        let group = synthetic_group(first, themes, 1);
        let cells = acceptance
            .enforced_tranche()
            .cells()
            .map(|cell| synthetic_cell(cell, acceptance, &group))
            .collect::<Vec<_>>();
        C6ReceiptBook::from_receipts(
            acceptance
                .enforced_tranche()
                .cells()
                .map(C6EnforcedCell::key),
            [group],
            cells,
        )
        .expect("synthetic receipt book")
    }

    fn assert_evidence_field(error: C6RuntimeError, expected: &'static str) {
        match error {
            C6RuntimeError::EvidenceMismatch { field, .. }
            | C6RuntimeError::RenderGroupEvidenceMismatch { field, .. } => {
                assert_eq!(field, expected);
            }
            other => panic!("expected evidence mismatch for {expected}, got {other}"),
        }
    }

    #[test]
    fn evaluator_accepts_receipts_bound_to_their_render_group() {
        let (themes, acceptance) = load_catalogs();
        let book = synthetic_valid_book(&themes, &acceptance);
        let report = book
            .evaluate(&acceptance, &themes)
            .expect("complete synthetic receipts");

        assert_eq!(report.verified_cell_count(), 2);
        assert_eq!(report.render_group_count(), 1);
        assert_eq!(report.manifest_digest(), acceptance.manifest_digest());
        assert_ne!(report.execution_digest(), &[0; 32]);
    }

    #[test]
    fn render_group_matching_fails_closed_on_recipe_mismatch() {
        let (themes, acceptance) = load_catalogs();
        let enforced = acceptance
            .enforced_tranche()
            .cells()
            .next()
            .expect("enforced cell");
        let document = themed_state_document_with_ceiling(ThemeResourcePolicy::interactive());
        let identity = C6RenderIdentity::from_evidence(document.evidence());
        let group = C6RenderGroupReceipt::seal(
            C6RenderGroupKey::for_cell(enforced),
            &themes,
            &identity,
            document.standalone_svg_admission(),
        )
        .expect("seal synthetic render group");

        assert!(group.matches_render(&identity, document.standalone_svg_admission()));

        let mut mismatched = identity;
        mismatched.recipe_fingerprint = None;
        assert!(!group.matches_render(&mismatched, document.standalone_svg_admission()));
    }

    #[test]
    fn receipt_canonical_digests_bind_opaque_target_receipt_identity() {
        let (themes, acceptance) = load_catalogs();
        let book = synthetic_valid_book(&themes, &acceptance);
        let group = book.groups.values().next().expect("render group");
        let cell = book.cells.values().next().expect("cell receipt");

        let mut changed_group = group.clone();
        changed_group.svg_target_receipt_digest[0] ^= 1;
        assert_ne!(group.canonical_digest(), changed_group.canonical_digest());

        let mut changed_group = group.clone();
        changed_group.shared_document_digest[0] ^= 1;
        assert_ne!(group.canonical_digest(), changed_group.canonical_digest());

        let mut changed = cell.clone();
        changed.target_receipt_digest[0] ^= 1;
        assert_ne!(cell.canonical_digest(), changed.canonical_digest());

        let mut changed = cell.clone();
        changed.semantic_assertion_id.push_str("-tampered");
        assert_ne!(cell.canonical_digest(), changed.canonical_digest());
    }

    #[test]
    fn execution_digest_is_sensitive_to_target_receipt_digest() {
        let (themes, acceptance) = load_catalogs();
        let first_book = synthetic_valid_book(&themes, &acceptance);
        let mut second_book = first_book.clone();
        let cell = second_book
            .cells
            .values_mut()
            .find(|cell| cell.key.target() == ExpectedOutputTarget::Png)
            .expect("native target cell receipt");
        cell.target_receipt_digest[0] ^= 1;
        cell.digest = cell.canonical_digest();

        let first = first_book
            .evaluate(&acceptance, &themes)
            .expect("first target receipt binding");
        let second = second_book
            .evaluate(&acceptance, &themes)
            .expect("second target receipt binding");

        assert_ne!(first.execution_digest(), second.execution_digest());
    }

    #[test]
    fn evaluator_rejects_resealed_svg_target_receipt_mismatch() {
        let (themes, acceptance) = load_catalogs();
        let mut book = synthetic_valid_book(&themes, &acceptance);
        let cell = book
            .cells
            .values_mut()
            .find(|cell| cell.key.target() == ExpectedOutputTarget::StandaloneSvg)
            .expect("SVG target cell receipt");
        cell.target_receipt_digest[0] ^= 1;
        cell.digest = cell.canonical_digest();

        assert_evidence_field(
            book.evaluate(&acceptance, &themes)
                .expect_err("re-sealed SVG target receipt mismatch must fail closed"),
            "target-receipt-digest",
        );
    }

    #[test]
    fn evaluator_rejects_tampered_group_and_cell_receipts() {
        let (themes, acceptance) = load_catalogs();
        let book = synthetic_valid_book(&themes, &acceptance);
        let key = acceptance
            .enforced_tranche()
            .cells()
            .next()
            .expect("enforced cell")
            .key();

        let mut tampered_group = book.clone();
        tampered_group
            .groups
            .values_mut()
            .next()
            .expect("render group")
            .recipe_fingerprint[0] ^= 1;
        assert_evidence_field(
            tampered_group
                .evaluate(&acceptance, &themes)
                .expect_err("tampered render group"),
            "render-group-digest",
        );

        let mut tampered_cell = book.clone();
        let cell = tampered_cell.cells.get_mut(&key).expect("cell receipt");
        cell.mechanism_dispositions.clear();
        assert_evidence_field(
            tampered_cell
                .evaluate(&acceptance, &themes)
                .expect_err("tampered cell receipt"),
            "mechanism-count",
        );

        let mut wrong_group = book;
        wrong_group
            .cells
            .get_mut(&key)
            .expect("cell receipt")
            .render_group_digest[0] ^= 1;
        assert_evidence_field(
            wrong_group
                .evaluate(&acceptance, &themes)
                .expect_err("wrong render-group reference"),
            "render-group-digest",
        );

        let mut wrong_assertion = synthetic_valid_book(&themes, &acceptance);
        let cell = wrong_assertion.cells.get_mut(&key).expect("cell receipt");
        cell.semantic_assertion_id = "wrong-semantic-assertion-v1".to_string();
        cell.digest = cell.canonical_digest();
        assert_evidence_field(
            wrong_assertion
                .evaluate(&acceptance, &themes)
                .expect_err("wrong semantic assertion id"),
            "semantic-assertion-id",
        );
    }

    #[test]
    fn receipt_book_rejects_duplicate_missing_unexpected_and_unreferenced_receipts() {
        let (themes, acceptance) = load_catalogs();
        let book = synthetic_valid_book(&themes, &acceptance);
        let groups = book.groups.values().cloned().collect::<Vec<_>>();
        let cells = book.cells.values().cloned().collect::<Vec<_>>();
        let expected = acceptance
            .enforced_tranche()
            .cells()
            .map(C6EnforcedCell::key)
            .collect::<Vec<_>>();

        let mut duplicate = cells.clone();
        duplicate.push(cells[0].clone());
        assert!(matches!(
            C6ReceiptBook::from_receipts(expected.clone(), groups.clone(), duplicate),
            Err(C6RuntimeError::DuplicateCellReceipt { .. })
        ));

        let mut missing = cells.clone();
        let missing_key = missing.pop().expect("cell receipt").key;
        match C6ReceiptBook::from_receipts(expected.clone(), groups.clone(), missing) {
            Err(C6RuntimeError::ReceiptCoverageMismatch {
                missing,
                unexpected,
            }) => {
                assert_eq!(missing, vec![missing_key]);
                assert!(unexpected.is_empty());
            }
            other => panic!("expected missing receipt error, got {other:?}"),
        }

        let mut extra_group = groups.clone();
        let mut unreferenced = extra_group[0].clone();
        unreferenced.key.source_fixture_id.push_str("-unused");
        unreferenced.digest = unreferenced.canonical_digest();
        extra_group.push(unreferenced);
        assert!(matches!(
            C6ReceiptBook::from_receipts(expected, extra_group, cells),
            Err(C6RuntimeError::UnreferencedRenderGroup { .. })
        ));
    }

    #[test]
    fn receipt_book_rejects_resealed_svg_target_receipt_identity() {
        let (themes, acceptance) = load_catalogs();
        let expected = acceptance
            .enforced_tranche()
            .cells()
            .map(C6EnforcedCell::key)
            .collect::<Vec<_>>();

        let book = synthetic_valid_book(&themes, &acceptance);
        let groups = book.groups.values().cloned().collect::<Vec<_>>();
        let mut cells = book.cells.values().cloned().collect::<Vec<_>>();
        let cell = cells
            .iter_mut()
            .find(|cell| cell.key.target() == ExpectedOutputTarget::StandaloneSvg)
            .expect("standalone SVG cell receipt");
        cell.target_receipt_digest[0] ^= 1;
        cell.digest = cell.canonical_digest();

        assert_evidence_field(
            C6ReceiptBook::from_receipts(expected, groups, cells)
                .expect_err("re-sealed SVG target receipt identity must fail closed"),
            "target-receipt-digest",
        );
    }
}

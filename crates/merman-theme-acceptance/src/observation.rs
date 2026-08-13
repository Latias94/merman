use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

use merman::DiagramFamilyId;
use merman::svg::{
    DocumentRenderReport, FontSource, RenderTargetKind, TargetAdmissionReport,
    TargetAdmissionStatus, ThemeRecipeFingerprint,
};
use merman_theme_fixtures::{
    C6AcceptanceCatalog, C6ArtifactAssertion, C6CellKey, C6EnforcedCell, C6ExpectedFontSource,
    C6ProofFamily, C6ProofTheme, C6RequiredAdmission, CatalogError, ExpectedOutputTarget,
    ReferenceThemeMechanism, ThemeFixtureCatalog,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum C6ObservedMechanismDisposition {
    Applied,
    NotApplicable,
    Rejected,
    Residual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum C6RenderLane {
    Native,
    Browser,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct C6RenderGroupKey {
    theme: C6ProofTheme,
    family: C6ProofFamily,
    source_fixture_id: String,
    lane: C6RenderLane,
}

impl C6RenderGroupKey {
    pub(crate) fn for_cell(cell: &C6EnforcedCell) -> Self {
        Self {
            theme: cell.key().theme(),
            family: cell.key().family(),
            source_fixture_id: cell.source_fixture_id().to_owned(),
            lane: lane_for_target(cell.key().target()),
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

    pub(crate) const fn lane(&self) -> C6RenderLane {
        self.lane
    }

    pub(crate) fn label(&self) -> String {
        format!(
            "{}/{}/{}/{}",
            proof_theme_id(self.theme),
            proof_family_id(self.family),
            self.source_fixture_id,
            render_lane_id(self.lane)
        )
    }
}

/// Receipt for one renderer execution shared by one or more target cells.
///
/// The receipt is sealed from the hash-validated fixture, typed theme input, compiled recipe,
/// terminal document and resource closure. Target adapters can only refer to this receipt by its
/// canonical digest; they cannot substitute a batch-global identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct C6RenderGroupReceipt {
    key: C6RenderGroupKey,
    source_sha256: [u8; 32],
    theme_input_sha256: [u8; 32],
    recipe_fingerprint: [u8; 32],
    document_digest: [u8; 32],
    resource_fingerprint: [u8; 32],
    digest: [u8; 32],
}

impl C6RenderGroupReceipt {
    pub(crate) fn seal(
        key: C6RenderGroupKey,
        catalog: &ThemeFixtureCatalog,
        actual_family: DiagramFamilyId,
        recipe_fingerprint: Option<ThemeRecipeFingerprint>,
        sealed_svg: &str,
        document: &DocumentRenderReport,
    ) -> Result<Self, C6RuntimeError> {
        let fixture = catalog
            .fixture(key.source_fixture_id())
            .ok_or_else(|| group_evidence_mismatch(&key, "source-fixture"))?;
        require_group_evidence(
            &key,
            "render-family",
            family_id_for_proof(key.family()) == actual_family,
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
        let recipe_fingerprint = recipe_fingerprint
            .ok_or_else(|| group_evidence_mismatch(&key, "recipe-fingerprint"))?;
        let mut receipt = Self {
            key,
            source_sha256,
            theme_input_sha256,
            recipe_fingerprint: *recipe_fingerprint.as_bytes(),
            document_digest: sha256(sealed_svg),
            resource_fingerprint: *document.resource_fingerprint().as_bytes(),
            digest: [0; 32],
        };
        receipt.digest = receipt.canonical_digest();
        Ok(receipt)
    }

    fn matches_document(
        &self,
        recipe_fingerprint: Option<ThemeRecipeFingerprint>,
        sealed_svg: &str,
        document: &DocumentRenderReport,
    ) -> bool {
        recipe_fingerprint
            .map(|fingerprint| *fingerprint.as_bytes() == self.recipe_fingerprint)
            .unwrap_or(false)
            && sha256(sealed_svg) == self.document_digest
            && document.resource_fingerprint().as_bytes() == &self.resource_fingerprint
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
        let mut value = b"merman.c6-render-group-receipt.v2\0".to_vec();
        encode_render_group_key(&mut value, &self.key);
        value.extend_from_slice(&self.source_sha256);
        value.extend_from_slice(&self.theme_input_sha256);
        value.extend_from_slice(&self.recipe_fingerprint);
        value.extend_from_slice(&self.document_digest);
        value.extend_from_slice(&self.resource_fingerprint);
        sha256(value)
    }
}

/// Target-local proof sealed only after a real artifact checker has accepted final bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct C6TargetProof {
    predicate: C6ProofPredicate,
    artifact_digest: [u8; 32],
    witness_digest: [u8; 32],
    mechanism_dispositions: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
}

impl C6TargetProof {
    pub(crate) fn brutalist_state_standalone_svg(
        artifact_bytes: &[u8],
        mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    ) -> Self {
        Self::seal(
            C6ProofPredicate::BrutalistStateStandaloneSvgV1,
            artifact_bytes,
            mechanisms,
        )
    }

    pub(crate) fn brutalist_state_png(
        artifact_bytes: &[u8],
        mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    ) -> Self {
        Self::seal(
            C6ProofPredicate::BrutalistStatePngV1,
            artifact_bytes,
            mechanisms,
        )
    }

    pub(crate) fn brutalist_state_jpeg(
        artifact_bytes: &[u8],
        mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    ) -> Self {
        Self::seal(
            C6ProofPredicate::BrutalistStateJpegV1,
            artifact_bytes,
            mechanisms,
        )
    }

    pub(crate) fn brutalist_state_pdf(
        artifact_bytes: &[u8],
        mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    ) -> Self {
        Self::seal(
            C6ProofPredicate::BrutalistStatePdfV1,
            artifact_bytes,
            mechanisms,
        )
    }

    fn seal(
        predicate: C6ProofPredicate,
        artifact_bytes: &[u8],
        mechanism_dispositions: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    ) -> Self {
        let artifact_digest = sha256(artifact_bytes);
        Self {
            predicate,
            artifact_digest,
            witness_digest: proof_witness_digest(
                predicate,
                artifact_digest,
                &mechanism_dispositions,
            ),
            mechanism_dispositions,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum C6ProofPredicate {
    BrutalistStateBrowserSvgV1,
    BrutalistStateStandaloneSvgV1,
    BrutalistStatePngV1,
    BrutalistStateJpegV1,
    BrutalistStatePdfV1,
}

impl C6ProofPredicate {
    const fn target(self) -> ExpectedOutputTarget {
        match self {
            Self::BrutalistStateBrowserSvgV1 => ExpectedOutputTarget::BrowserSvg,
            Self::BrutalistStateStandaloneSvgV1 => ExpectedOutputTarget::StandaloneSvg,
            Self::BrutalistStatePngV1 => ExpectedOutputTarget::Png,
            Self::BrutalistStateJpegV1 => ExpectedOutputTarget::Jpeg,
            Self::BrutalistStatePdfV1 => ExpectedOutputTarget::Pdf,
        }
    }

    const fn id(self) -> &'static str {
        match self {
            Self::BrutalistStateBrowserSvgV1 => "brutalist-state-browser-svg-v1",
            Self::BrutalistStateStandaloneSvgV1 => "brutalist-state-standalone-svg-v1",
            Self::BrutalistStatePngV1 => "brutalist-state-png-v1",
            Self::BrutalistStateJpegV1 => "brutalist-state-jpeg-v1",
            Self::BrutalistStatePdfV1 => "brutalist-state-pdf-v1",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct C6CellReceipt {
    key: C6CellKey,
    render_group_key: C6RenderGroupKey,
    render_group_digest: [u8; 32],
    admission_target: RenderTargetKind,
    admission: C6RequiredAdmission,
    admission_reason_ids: Vec<String>,
    artifact_assertion: C6ArtifactAssertion,
    proof_predicate: C6ProofPredicate,
    artifact_digest: [u8; 32],
    witness_digest: [u8; 32],
    mechanism_dispositions: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    residual_ids: BTreeSet<String>,
    font_source: C6ExpectedFontSource,
    digest: [u8; 32],
}

pub(crate) fn seal_cell_from_reports(
    enforced: &C6EnforcedCell,
    group: &C6RenderGroupReceipt,
    recipe_fingerprint: Option<ThemeRecipeFingerprint>,
    sealed_svg: &str,
    document: &DocumentRenderReport,
    admission: &TargetAdmissionReport,
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
        "target-proof",
        proof.predicate.target() == key.target(),
    )?;
    require_evidence(
        key,
        "proof-predicate",
        expected_proof_predicate(key) == Some(proof.predicate),
    )?;
    require_evidence(
        key,
        "target-admission",
        render_target_for_expected(proof.predicate.target()) == Some(admission.target()),
    )?;
    require_evidence(
        key,
        "render-group-document",
        group.matches_document(recipe_fingerprint, sealed_svg, document),
    )?;
    require_evidence(key, "document-residuals", document.residuals().is_empty())?;
    require_evidence(key, "artifact-digest", proof.artifact_digest != [0; 32])?;
    require_evidence(key, "witness-digest", proof.witness_digest != [0; 32])?;

    let font_source = match document.prepared_text_used_font_sources() {
        [FontSource::Embedded] => C6ExpectedFontSource::Embedded,
        [FontSource::System] => C6ExpectedFontSource::System,
        [] => C6ExpectedFontSource::None,
        _ => return Err(evidence_mismatch(key, "prepared-font-source")),
    };
    let (observed_admission, admission_reason_ids) = observed_admission(key, admission)?;
    let mut receipt = C6CellReceipt {
        key,
        render_group_key: group.key.clone(),
        render_group_digest: group.digest,
        admission_target: admission.target(),
        admission: observed_admission,
        admission_reason_ids,
        artifact_assertion: artifact_assertion_for_target(proof.predicate.target()),
        proof_predicate: proof.predicate,
        artifact_digest: proof.artifact_digest,
        witness_digest: proof.witness_digest,
        mechanism_dispositions: proof.mechanism_dispositions,
        residual_ids: BTreeSet::new(),
        font_source,
        digest: [0; 32],
    };
    receipt.digest = receipt.canonical_digest();
    Ok(receipt)
}

impl C6CellReceipt {
    fn canonical_digest(&self) -> [u8; 32] {
        let mut value = b"merman.c6-cell-receipt.v2\0".to_vec();
        encode_cell_key(&mut value, self.key);
        encode_render_group_key(&mut value, &self.render_group_key);
        value.extend_from_slice(&self.render_group_digest);
        append_len_prefixed(&mut value, self.admission_target.id().as_bytes());
        append_len_prefixed(&mut value, admission_status_id(self.admission).as_bytes());
        for reason_id in &self.admission_reason_ids {
            append_len_prefixed(&mut value, reason_id.as_bytes());
        }
        append_len_prefixed(
            &mut value,
            artifact_assertion_id(self.artifact_assertion).as_bytes(),
        );
        append_len_prefixed(&mut value, self.proof_predicate.id().as_bytes());
        value.extend_from_slice(&self.artifact_digest);
        value.extend_from_slice(&self.witness_digest);
        value.extend_from_slice(&mechanism_digest(&self.mechanism_dispositions));
        for residual_id in &self.residual_ids {
            append_len_prefixed(&mut value, residual_id.as_bytes());
        }
        append_len_prefixed(&mut value, font_source_id(self.font_source).as_bytes());
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
            require_evidence(
                key,
                "target",
                receipt.artifact_assertion == artifact_assertion_for_target(key.target())
                    && render_target_for_expected(key.target()) == Some(receipt.admission_target),
            )?;
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
                "font-source",
                receipt.font_source == expectation.required_font_source(),
            )?;
            require_evidence(
                key,
                "target-admission",
                receipt.admission == expectation.required_admission(),
            )?;
            require_evidence(
                key,
                "target-admission-reasons",
                receipt.admission != C6RequiredAdmission::Portable
                    || receipt.admission_reason_ids.is_empty(),
            )?;
            require_evidence(
                key,
                "artifact-assertion",
                receipt.artifact_assertion == expectation.required_artifact_assertion(),
            )?;
            require_evidence(
                key,
                "proof-predicate",
                expected_proof_predicate(key) == Some(receipt.proof_predicate),
            )?;
            require_evidence(key, "artifact-digest", receipt.artifact_digest != [0; 32])?;
            require_evidence(
                key,
                "witness-digest",
                receipt.witness_digest
                    == proof_witness_digest(
                        receipt.proof_predicate,
                        receipt.artifact_digest,
                        &receipt.mechanism_dispositions,
                    ),
            )?;
            require_evidence(
                key,
                "cell-receipt-digest",
                receipt.digest == receipt.canonical_digest(),
            )?;
        }

        let mut digest_input = b"merman.c6-execution-report.v2\0".to_vec();
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
            execution_digest: sha256(digest_input),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C6ExecutionReport {
    verified_cell_count: usize,
    render_group_count: usize,
    execution_digest: [u8; 32],
}

impl C6ExecutionReport {
    pub const fn verified_cell_count(&self) -> usize {
        self.verified_cell_count
    }

    pub const fn render_group_count(&self) -> usize {
        self.render_group_count
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

fn observed_admission(
    key: C6CellKey,
    admission: &TargetAdmissionReport,
) -> Result<(C6RequiredAdmission, Vec<String>), C6RuntimeError> {
    let observed = match admission.status() {
        TargetAdmissionStatus::Portable => C6RequiredAdmission::Portable,
        TargetAdmissionStatus::HostDependent => C6RequiredAdmission::HostDependent,
        TargetAdmissionStatus::Rejected => C6RequiredAdmission::Rejected,
        _ => return Err(evidence_mismatch(key, "target-admission-status")),
    };
    let reason_ids = admission
        .reasons()
        .iter()
        .map(|reason| reason.id().to_owned())
        .collect::<Vec<_>>();
    if observed == C6RequiredAdmission::Portable && !reason_ids.is_empty() {
        return Err(evidence_mismatch(key, "portable-admission-reasons"));
    }
    Ok((observed, reason_ids))
}

fn lane_for_target(target: ExpectedOutputTarget) -> C6RenderLane {
    match target {
        ExpectedOutputTarget::BrowserSvg => C6RenderLane::Browser,
        ExpectedOutputTarget::StandaloneSvg
        | ExpectedOutputTarget::Png
        | ExpectedOutputTarget::Jpeg
        | ExpectedOutputTarget::Pdf => C6RenderLane::Native,
    }
}

fn family_id_for_proof(family: C6ProofFamily) -> DiagramFamilyId {
    match family {
        C6ProofFamily::Flowchart => DiagramFamilyId::FLOWCHART,
        C6ProofFamily::Sequence => DiagramFamilyId::SEQUENCE,
        C6ProofFamily::State => DiagramFamilyId::STATE,
    }
}

fn render_target_for_expected(target: ExpectedOutputTarget) -> Option<RenderTargetKind> {
    match target {
        ExpectedOutputTarget::BrowserSvg | ExpectedOutputTarget::StandaloneSvg => {
            Some(RenderTargetKind::Svg)
        }
        ExpectedOutputTarget::Png => Some(RenderTargetKind::Png),
        ExpectedOutputTarget::Jpeg => Some(RenderTargetKind::Jpeg),
        ExpectedOutputTarget::Pdf => Some(RenderTargetKind::Pdf),
    }
}

fn artifact_assertion_for_target(target: ExpectedOutputTarget) -> C6ArtifactAssertion {
    match target {
        ExpectedOutputTarget::BrowserSvg => C6ArtifactAssertion::BrowserSvgDom,
        ExpectedOutputTarget::StandaloneSvg => C6ArtifactAssertion::StandaloneSvgDocument,
        ExpectedOutputTarget::Png => C6ArtifactAssertion::PngImage,
        ExpectedOutputTarget::Jpeg => C6ArtifactAssertion::JpegImage,
        ExpectedOutputTarget::Pdf => C6ArtifactAssertion::PdfDocument,
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
    let mut value = b"merman.c6-mechanism-witness.v2\0".to_vec();
    for (mechanism, disposition) in mechanisms {
        append_len_prefixed(&mut value, reference_mechanism_id(*mechanism).as_bytes());
        append_len_prefixed(
            &mut value,
            mechanism_disposition_id(*disposition).as_bytes(),
        );
    }
    sha256(value)
}

fn proof_witness_digest(
    predicate: C6ProofPredicate,
    artifact_digest: [u8; 32],
    mechanisms: &BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
) -> [u8; 32] {
    let mut value = b"merman.c6-target-witness.v2\0".to_vec();
    append_len_prefixed(&mut value, predicate.id().as_bytes());
    value.extend_from_slice(&artifact_digest);
    value.extend_from_slice(&mechanism_digest(mechanisms));
    sha256(value)
}

fn expected_proof_predicate(key: C6CellKey) -> Option<C6ProofPredicate> {
    match (key.theme(), key.family(), key.target()) {
        (C6ProofTheme::Brutalist, C6ProofFamily::State, ExpectedOutputTarget::BrowserSvg) => {
            Some(C6ProofPredicate::BrutalistStateBrowserSvgV1)
        }
        (C6ProofTheme::Brutalist, C6ProofFamily::State, ExpectedOutputTarget::StandaloneSvg) => {
            Some(C6ProofPredicate::BrutalistStateStandaloneSvgV1)
        }
        (C6ProofTheme::Brutalist, C6ProofFamily::State, ExpectedOutputTarget::Png) => {
            Some(C6ProofPredicate::BrutalistStatePngV1)
        }
        (C6ProofTheme::Brutalist, C6ProofFamily::State, ExpectedOutputTarget::Jpeg) => {
            Some(C6ProofPredicate::BrutalistStateJpegV1)
        }
        (C6ProofTheme::Brutalist, C6ProofFamily::State, ExpectedOutputTarget::Pdf) => {
            Some(C6ProofPredicate::BrutalistStatePdfV1)
        }
        _ => None,
    }
}

fn encode_render_group_key(output: &mut Vec<u8>, key: &C6RenderGroupKey) {
    append_len_prefixed(output, proof_theme_id(key.theme).as_bytes());
    append_len_prefixed(output, proof_family_id(key.family).as_bytes());
    append_len_prefixed(output, key.source_fixture_id.as_bytes());
    append_len_prefixed(output, render_lane_id(key.lane).as_bytes());
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

fn render_lane_id(lane: C6RenderLane) -> &'static str {
    match lane {
        C6RenderLane::Native => "native",
        C6RenderLane::Browser => "browser",
    }
}

fn artifact_assertion_id(assertion: C6ArtifactAssertion) -> &'static str {
    match assertion {
        C6ArtifactAssertion::BrowserSvgDom => "browser-svg-dom",
        C6ArtifactAssertion::StandaloneSvgDocument => "standalone-svg-document",
        C6ArtifactAssertion::PngImage => "png-image",
        C6ArtifactAssertion::JpegImage => "jpeg-image",
        C6ArtifactAssertion::PdfDocument => "pdf-document",
    }
}

fn admission_status_id(admission: C6RequiredAdmission) -> &'static str {
    match admission {
        C6RequiredAdmission::Portable => "portable",
        C6RequiredAdmission::HostDependent => "host-dependent",
        C6RequiredAdmission::Rejected => "rejected",
    }
}

fn font_source_id(source: C6ExpectedFontSource) -> &'static str {
    match source {
        C6ExpectedFontSource::Embedded => "embedded",
        C6ExpectedFontSource::None => "none",
        C6ExpectedFontSource::System => "system",
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

fn sha256(bytes: impl AsRef<[u8]>) -> [u8; 32] {
    Sha256::digest(bytes.as_ref()).into()
}

fn append_len_prefixed(output: &mut Vec<u8>, bytes: &[u8]) {
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
    use serde_json::json;
    use std::fs;
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

    fn load_catalogs_with_browser_cell() -> (ThemeFixtureCatalog, C6AcceptanceCatalog) {
        let themes = ThemeFixtureCatalog::load(themes_root()).expect("load theme fixtures");
        let source = fs::read_to_string(
            themes
                .root()
                .join(merman_theme_fixtures::C6_ACCEPTANCE_RELATIVE_PATH),
        )
        .expect("read C6 acceptance catalog");
        let mut value: serde_json::Value =
            serde_json::from_str(&source).expect("parse C6 acceptance catalog");
        let browser = value["cells"]
            .as_array_mut()
            .expect("C6 cells")
            .iter_mut()
            .find(|cell| {
                cell["theme"] == "brutalist"
                    && cell["family"] == "state"
                    && cell["target"] == "browser-svg"
            })
            .expect("Brutalist State browser cell");
        browser["enforcement"] = json!({
            "kind": "enforced",
            "sourceFixtureId": "fixture-c6-brutalist-state"
        });
        let acceptance = C6AcceptanceCatalog::from_json(
            &serde_json::to_string(&value).expect("serialize mutated acceptance catalog"),
            &themes,
        )
        .expect("load C6 acceptance catalog with browser cell");
        (themes, acceptance)
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
            document_digest: [seed.wrapping_add(1); 32],
            resource_fingerprint: [seed.wrapping_add(2); 32],
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
        let proof_predicate = expected_proof_predicate(key).expect("supported proof predicate");
        let artifact_digest = sha256(format!("synthetic-artifact-{key:?}"));
        let mut receipt = C6CellReceipt {
            key,
            render_group_key: group.key.clone(),
            render_group_digest: group.digest,
            admission_target: render_target_for_expected(key.target()).expect("render target"),
            admission: expectation.required_admission(),
            admission_reason_ids: Vec::new(),
            artifact_assertion: expectation.required_artifact_assertion(),
            proof_predicate,
            artifact_digest,
            witness_digest: proof_witness_digest(
                proof_predicate,
                artifact_digest,
                &mechanism_dispositions,
            ),
            mechanism_dispositions,
            residual_ids: expectation.expected_residual_ids().clone(),
            font_source: expectation.required_font_source(),
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

        assert_eq!(report.verified_cell_count(), 4);
        assert_eq!(report.render_group_count(), 1);
        assert_ne!(report.execution_digest(), &[0; 32]);
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
        tampered_cell
            .cells
            .get_mut(&key)
            .expect("cell receipt")
            .artifact_digest[0] ^= 1;
        assert_evidence_field(
            tampered_cell
                .evaluate(&acceptance, &themes)
                .expect_err("tampered cell receipt"),
            "witness-digest",
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

        let mut wrong_predicate = synthetic_valid_book(&themes, &acceptance);
        let cell = wrong_predicate.cells.get_mut(&key).expect("cell receipt");
        cell.proof_predicate = C6ProofPredicate::BrutalistStatePdfV1;
        cell.witness_digest = proof_witness_digest(
            cell.proof_predicate,
            cell.artifact_digest,
            &cell.mechanism_dispositions,
        );
        cell.digest = cell.canonical_digest();
        assert_evidence_field(
            wrong_predicate
                .evaluate(&acceptance, &themes)
                .expect_err("wrong proof predicate"),
            "proof-predicate",
        );
    }

    #[test]
    fn evaluator_accepts_multiple_execution_groups_without_a_global_identity() {
        let (themes, acceptance) = load_catalogs_with_browser_cell();
        let enforced = acceptance.enforced_tranche().cells().collect::<Vec<_>>();
        let native_cell = enforced
            .iter()
            .copied()
            .find(|cell| cell.key().target() == ExpectedOutputTarget::StandaloneSvg)
            .expect("native cell");
        let browser_cell = enforced
            .iter()
            .copied()
            .find(|cell| cell.key().target() == ExpectedOutputTarget::BrowserSvg)
            .expect("browser cell");
        let native = synthetic_group(native_cell, &themes, 7);
        let browser = synthetic_group(browser_cell, &themes, 11);
        let cells = enforced
            .iter()
            .map(|cell| {
                let group = match C6RenderGroupKey::for_cell(cell).lane() {
                    C6RenderLane::Native => &native,
                    C6RenderLane::Browser => &browser,
                };
                synthetic_cell(cell, &acceptance, group)
            })
            .collect::<Vec<_>>();

        let book = C6ReceiptBook::from_receipts(
            enforced.iter().map(|cell| cell.key()),
            [native, browser],
            cells,
        )
        .expect("heterogeneous receipt book");
        let report = book
            .evaluate(&acceptance, &themes)
            .expect("evaluate heterogeneous receipt book");

        assert_eq!(report.render_group_count(), 2);
        assert_eq!(report.verified_cell_count(), 5);
        assert_ne!(report.execution_digest(), &[0; 32]);
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
}

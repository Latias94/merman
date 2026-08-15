use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

use merman::svg::ThemeRecipeFingerprint;
use merman::{
    DiagramFamilyId, RenderArtifactKind, RenderEvidence, TargetAdmissionReceipt,
    TargetAdmissionStatus, TargetFontSource,
};
use merman_theme_fixtures::{
    C6AcceptanceCatalog, C6ArtifactAssertion, C6CellKey, C6EnforcedCell, C6ExpectedFontSource,
    C6ProofFamily, C6ProofTheme, C6RequiredAdmission, CatalogError, ExpectedOutputTarget,
    ReferenceThemeMechanism, ThemeFixtureCatalog,
};

use crate::cutover::usize_to_u64;

// Frozen input to the v1 operation digest. The public execution-path enum was removed, but the
// canonical renderer tag remains byte-for-byte stable for existing acceptance receipts.
const C6_V1_RENDERER_TAG: &[u8] = b"renderer";

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
    operation_digest: [u8; 32],
    admission_digest: [u8; 32],
}

impl C6RenderIdentity {
    pub(crate) fn from_evidence(evidence: &RenderEvidence) -> Self {
        let context = evidence.operation_context();
        let date = evidence.local_date();
        let time_zone = evidence.local_time_zone();
        let mut value = b"merman.c6-render-operation.v1\0".to_vec();
        append_len_prefixed(&mut value, C6_V1_RENDERER_TAG);
        append_len_prefixed(&mut value, evidence.family_id().as_str().as_bytes());
        match evidence.theme_recipe_fingerprint() {
            Some(fingerprint) => {
                value.push(1);
                value.extend_from_slice(fingerprint.as_bytes());
            }
            None => value.push(0),
        }
        value.extend_from_slice(&context.unix_millis().to_be_bytes());
        append_len_prefixed(&mut value, context.clock_source().id().as_bytes());
        value.extend_from_slice(&date.year().to_be_bytes());
        value.extend_from_slice(&date.month().to_be_bytes());
        value.extend_from_slice(&date.day().to_be_bytes());
        value.push(u8::from(context.today_is_fixed()));
        append_len_prefixed(
            &mut value,
            match time_zone.source() {
                merman::time::LocalTimeZoneSource::FixedOffset => b"fixed-offset",
                merman::time::LocalTimeZoneSource::System => b"system",
            },
        );
        append_len_prefixed(&mut value, time_zone.identifier().as_bytes());
        match time_zone.rules_sha256() {
            Some(rules) => {
                value.push(1);
                append_len_prefixed(&mut value, rules.as_bytes());
            }
            None => value.push(0),
        }
        value.extend_from_slice(&context.seed().to_be_bytes());
        append_len_prefixed(&mut value, context.random_source().id().as_bytes());
        value.extend_from_slice(&evidence.render_seed().get().to_be_bytes());
        value.extend_from_slice(
            &u64::try_from(evidence.layout_work_units())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        Self {
            actual_family: evidence.family_id(),
            recipe_fingerprint: evidence.theme_recipe_fingerprint(),
            operation_digest: sha256(value),
            admission_digest: render_admission_digest(evidence),
        }
    }

    pub(crate) const fn operation_digest(&self) -> &[u8; 32] {
        &self.operation_digest
    }

    pub(crate) const fn admission_digest(&self) -> &[u8; 32] {
        &self.admission_digest
    }
}

fn render_admission_digest(evidence: &RenderEvidence) -> [u8; 32] {
    let acceptance = merman::__theme_acceptance::theme_acceptance_evidence(evidence);
    let public = evidence.theme_evidence();
    let mut value = b"merman.c6-render-admission.v2\0".to_vec();

    append_theme_scope(&mut value, acceptance.root());
    append_theme_scope(&mut value, acceptance.family());
    value.extend_from_slice(&usize_to_u64(acceptance.source_residual_count()).to_be_bytes());
    value.extend_from_slice(&usize_to_u64(acceptance.compatibility_residual_count()).to_be_bytes());
    value.extend_from_slice(
        &usize_to_u64(acceptance.mermaid_compatibility_residual_count()).to_be_bytes(),
    );

    match acceptance.recipe_report() {
        Some(report) => {
            value.push(1);
            value.extend_from_slice(report.theme_recipe_fingerprint().as_bytes());
        }
        None => value.push(0),
    }
    append_theme_host_admission(&mut value, acceptance.host_admission_report());
    match acceptance.prepared_text_layout() {
        Some(prepared) => {
            value.push(1);
            value
                .extend_from_slice(&usize_to_u64(prepared.used_font_sources().len()).to_be_bytes());
            for source in prepared.used_font_sources() {
                append_len_prefixed(&mut value, source.id().as_bytes());
            }
        }
        None => value.push(0),
    }
    value.push(u8::from(acceptance.text_layout_failure().is_some()));
    append_theme_resource_policy(&mut value, acceptance.effective_theme_resource_policy());
    append_len_prefixed(
        &mut value,
        theme_evidence_status_id(public.status()).as_bytes(),
    );
    value.push(u8::from(public.output_mutated()));

    let host_measurement_count = evidence
        .measurement()
        .entries()
        .iter()
        .filter(|entry| entry.provenance().source == merman::svg::TextMeasurementSource::Host)
        .map(|entry| entry.count())
        .sum::<u64>();
    value.extend_from_slice(&host_measurement_count.to_be_bytes());
    sha256(value)
}

fn append_theme_resource_policy(value: &mut Vec<u8>, policy: &merman::svg::ThemeResourcePolicy) {
    match policy.profile() {
        Some(profile) => {
            value.push(1);
            append_len_prefixed(value, profile.id().as_bytes());
        }
        None => value.push(0),
    }
    for limit in merman::svg::ThemeResourceLimitId::ALL {
        append_len_prefixed(value, limit.as_str().as_bytes());
        append_optional_usize(value, policy.base_value(*limit));
        append_optional_usize(value, policy.value(*limit));
        append_optional_usize(value, policy.explicit_override(*limit));
    }
}

fn append_theme_host_admission(
    value: &mut Vec<u8>,
    report: Option<&merman_render::diagram_theme::ThemeHostAdmissionReport>,
) {
    let Some(report) = report else {
        value.push(0);
        return;
    };
    value.push(1);

    let capabilities = report.host_allowed_capabilities().collect::<Vec<_>>();
    value.extend_from_slice(&usize_to_u64(capabilities.len()).to_be_bytes());
    for capability in capabilities {
        append_len_prefixed(value, capability.id().as_bytes());
    }

    let text_capabilities = report.host_allowed_text_capabilities().collect::<Vec<_>>();
    value.extend_from_slice(&usize_to_u64(text_capabilities.len()).to_be_bytes());
    for capability in text_capabilities {
        append_len_prefixed(value, capability.id().as_bytes());
    }

    let font_sources = report.font_source_policy().priority().collect::<Vec<_>>();
    value.extend_from_slice(&usize_to_u64(font_sources.len()).to_be_bytes());
    for source in font_sources {
        append_len_prefixed(value, source.id().as_bytes());
    }

    let measurement_fallbacks = report
        .measurement_fallback_policy()
        .priority()
        .collect::<Vec<_>>();
    value.extend_from_slice(&usize_to_u64(measurement_fallbacks.len()).to_be_bytes());
    for fallback in measurement_fallbacks {
        append_len_prefixed(value, fallback.id().as_bytes());
    }

    append_len_prefixed(
        value,
        portability_requirement_id(Some(report.portability_requirement())).as_bytes(),
    );

    let trusted_lanes = report.trusted_lanes().allowed().collect::<Vec<_>>();
    value.extend_from_slice(&usize_to_u64(trusted_lanes.len()).to_be_bytes());
    for lane in trusted_lanes {
        append_len_prefixed(value, lane.id().as_bytes());
    }
}

fn append_optional_usize(value: &mut Vec<u8>, item: Option<usize>) {
    match item {
        Some(item) => {
            value.push(1);
            value.extend_from_slice(&usize_to_u64(item).to_be_bytes());
        }
        None => value.push(0),
    }
}

fn append_theme_scope(
    value: &mut Vec<u8>,
    scope: merman::__theme_acceptance::ThemeAcceptanceScopeEvidence,
) {
    append_len_prefixed(value, theme_evidence_status_id(scope.status()).as_bytes());
    value.extend_from_slice(&usize_to_u64(scope.required_count()).to_be_bytes());
    value.extend_from_slice(&usize_to_u64(scope.accounted_count()).to_be_bytes());
    value.extend_from_slice(&usize_to_u64(scope.applied_count()).to_be_bytes());
    value.extend_from_slice(&usize_to_u64(scope.not_applicable_count()).to_be_bytes());
    value.extend_from_slice(&usize_to_u64(scope.incomplete_count()).to_be_bytes());
    value.extend_from_slice(&usize_to_u64(scope.residual_count()).to_be_bytes());
    value.push(u8::from(scope.output_mutated()));
}

fn theme_evidence_status_id(status: merman::ThemeEvidenceStatus) -> &'static str {
    match status {
        merman::ThemeEvidenceStatus::NotApplicable => "not-applicable",
        merman::ThemeEvidenceStatus::Verified => "verified",
        merman::ThemeEvidenceStatus::Residual => "residual",
        merman::ThemeEvidenceStatus::Incomplete => "incomplete",
        _ => "unknown",
    }
}

fn portability_requirement_id(
    requirement: Option<merman::svg::ThemePortabilityRequirement>,
) -> &'static str {
    match requirement {
        None => "none",
        Some(merman::svg::ThemePortabilityRequirement::BestEffort) => "best-effort",
        Some(merman::svg::ThemePortabilityRequirement::RequirePortable) => "require-portable",
        Some(_) => "unknown",
    }
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

/// Receipt for one deterministic render group shared by one or more target cells.
///
/// The receipt is sealed from the hash-validated fixture, typed theme input, compiled recipe,
/// render admission evidence, completed document identity, and the target-owned standalone SVG
/// receipt. Target adapters can only refer to this receipt by its canonical digest; they cannot
/// substitute a batch-global identity or reconstruct the target receipt protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct C6RenderGroupReceipt {
    key: C6RenderGroupKey,
    source_sha256: [u8; 32],
    theme_input_sha256: [u8; 32],
    recipe_fingerprint: [u8; 32],
    operation_digest: [u8; 32],
    admission_digest: [u8; 32],
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
            operation_digest: identity.operation_digest,
            admission_digest: identity.admission_digest,
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
            && identity.operation_digest == self.operation_digest
            && identity.admission_digest == self.admission_digest
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
        let mut value = b"merman.c6-render-group-receipt.v6\0".to_vec();
        encode_render_group_key(&mut value, &self.key);
        value.extend_from_slice(&self.source_sha256);
        value.extend_from_slice(&self.theme_input_sha256);
        value.extend_from_slice(&self.recipe_fingerprint);
        value.extend_from_slice(&self.operation_digest);
        value.extend_from_slice(&self.admission_digest);
        value.extend_from_slice(&self.shared_document_digest);
        value.extend_from_slice(&self.svg_target_receipt_digest);
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
        artifact_digest: [u8; 32],
        mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    ) -> Self {
        Self::seal(
            C6ProofPredicate::BrutalistStateStandaloneSvgV1,
            artifact_digest,
            mechanisms,
        )
    }

    pub(crate) fn brutalist_state_png(
        artifact_digest: [u8; 32],
        mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    ) -> Self {
        Self::seal(
            C6ProofPredicate::BrutalistStatePngV1,
            artifact_digest,
            mechanisms,
        )
    }

    pub(crate) fn brutalist_state_jpeg(
        artifact_digest: [u8; 32],
        mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    ) -> Self {
        Self::seal(
            C6ProofPredicate::BrutalistStateJpegV1,
            artifact_digest,
            mechanisms,
        )
    }

    pub(crate) fn brutalist_state_pdf(
        artifact_digest: [u8; 32],
        mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    ) -> Self {
        Self::seal(
            C6ProofPredicate::BrutalistStatePdfV1,
            artifact_digest,
            mechanisms,
        )
    }

    fn seal(
        predicate: C6ProofPredicate,
        artifact_digest: [u8; 32],
        mechanism_dispositions: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    ) -> Self {
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
    operation_digest: [u8; 32],
    admission_digest: [u8; 32],
    admission: TargetAdmissionStatus,
    admission_reason_count: usize,
    // Target-owned identity; observation never re-encodes its constituent fields.
    target_receipt_digest: [u8; 32],
    artifact_assertion: C6ArtifactAssertion,
    proof_predicate: C6ProofPredicate,
    // Acceptance-owned artifact witness, checked against the target receipt before sealing.
    artifact_digest: [u8; 32],
    witness_digest: [u8; 32],
    mechanism_dispositions: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    residual_ids: BTreeSet<String>,
    font_source: TargetFontSource,
    digest: [u8; 32],
}

pub(crate) fn seal_cell_from_evidence(
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
        target_receipt.artifact_kind() == render_target_for_expected(enforced.key().target()),
    )?;
    require_evidence(
        key,
        "render-group-evidence",
        group.matches_render(&identity, &target_receipt),
    )?;
    require_evidence(
        key,
        "portable-admission-reasons",
        target_receipt.status() != TargetAdmissionStatus::Portable
            || target_receipt.reasons().is_empty(),
    )?;
    require_evidence(
        key,
        "target-receipt-digest",
        target_receipt.receipt_digest() != [0; 32],
    )?;
    if matches!(
        key.target(),
        ExpectedOutputTarget::BrowserSvg | ExpectedOutputTarget::StandaloneSvg
    ) {
        require_evidence(
            key,
            "target-receipt-digest",
            target_receipt.receipt_digest() == group.svg_target_receipt_digest,
        )?;
    }
    require_evidence(
        key,
        "target-artifact-digest",
        proof.artifact_digest == target_receipt.artifact_digest(),
    )?;
    require_evidence(key, "artifact-digest", proof.artifact_digest != [0; 32])?;
    require_evidence(key, "witness-digest", proof.witness_digest != [0; 32])?;
    let mut receipt = C6CellReceipt {
        key,
        render_group_key: group.key.clone(),
        render_group_digest: group.digest,
        operation_digest: identity.operation_digest,
        admission_digest: identity.admission_digest,
        admission: target_receipt.status(),
        admission_reason_count: target_receipt.reasons().len(),
        target_receipt_digest: target_receipt.receipt_digest(),
        artifact_assertion: artifact_assertion_for_target(proof.predicate.target()),
        proof_predicate: proof.predicate,
        artifact_digest: proof.artifact_digest,
        witness_digest: proof.witness_digest,
        mechanism_dispositions: proof.mechanism_dispositions,
        residual_ids,
        font_source: target_receipt.font_source(),
        digest: [0; 32],
    };
    receipt.digest = receipt.canonical_digest();
    Ok(receipt)
}

impl C6CellReceipt {
    fn canonical_digest(&self) -> [u8; 32] {
        let mut value = b"merman.c6-cell-receipt.v6\0".to_vec();
        encode_cell_key(&mut value, self.key);
        encode_render_group_key(&mut value, &self.render_group_key);
        value.extend_from_slice(&self.render_group_digest);
        value.extend_from_slice(&self.operation_digest);
        value.extend_from_slice(&self.admission_digest);
        append_len_prefixed(&mut value, self.admission.id().as_bytes());
        value.extend_from_slice(&usize_to_u64(self.admission_reason_count).to_be_bytes());
        value.extend_from_slice(&self.target_receipt_digest);
        append_len_prefixed(
            &mut value,
            artifact_assertion_id(self.artifact_assertion).as_bytes(),
        );
        append_len_prefixed(&mut value, self.proof_predicate.id().as_bytes());
        value.extend_from_slice(&self.artifact_digest);
        value.extend_from_slice(&self.witness_digest);
        append_len_prefixed(&mut value, b"mechanism-dispositions");
        value.extend_from_slice(&mechanism_digest(&self.mechanism_dispositions));
        append_len_prefixed(&mut value, b"residual-ids");
        value.extend_from_slice(&usize_to_u64(self.residual_ids.len()).to_be_bytes());
        for residual_id in &self.residual_ids {
            append_len_prefixed(&mut value, residual_id.as_bytes());
        }
        append_len_prefixed(&mut value, self.font_source.id().as_bytes());
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
            require_evidence(
                cell.key,
                "render-admission-digest",
                cell.admission_digest == group.admission_digest,
            )?;
            if matches!(
                cell.key.target(),
                ExpectedOutputTarget::BrowserSvg | ExpectedOutputTarget::StandaloneSvg
            ) {
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
                receipt.render_group_digest == group.digest
                    && receipt.operation_digest == group.operation_digest,
            )?;
            require_evidence(
                key,
                "render-admission-digest",
                receipt.admission_digest == group.admission_digest,
            )?;
            if matches!(
                key.target(),
                ExpectedOutputTarget::BrowserSvg | ExpectedOutputTarget::StandaloneSvg
            ) {
                require_evidence(
                    key,
                    "target-receipt-digest",
                    receipt.target_receipt_digest == group.svg_target_receipt_digest,
                )?;
            }
            require_evidence(
                key,
                "target",
                receipt.artifact_assertion == artifact_assertion_for_target(key.target()),
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
                receipt.font_source
                    == target_font_source_for_expected(expectation.required_font_source()),
            )?;
            require_evidence(
                key,
                "target-admission",
                receipt.admission
                    == target_admission_for_required(expectation.required_admission()),
            )?;
            require_evidence(
                key,
                "target-admission-reasons",
                receipt.admission != TargetAdmissionStatus::Portable
                    || receipt.admission_reason_count == 0,
            )?;
            require_evidence(
                key,
                "target-receipt-digest",
                receipt.target_receipt_digest != [0; 32],
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

        let mut digest_input = b"merman.c6-execution-report.v6\0".to_vec();
        for group in self.groups.values() {
            encode_render_group_key(&mut digest_input, &group.key);
            digest_input.extend_from_slice(&group.admission_digest);
            digest_input.extend_from_slice(&group.digest);
        }
        for receipt in self.cells.values() {
            encode_cell_key(&mut digest_input, receipt.key);
            digest_input.extend_from_slice(&receipt.admission_digest);
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

fn target_admission_for_required(admission: C6RequiredAdmission) -> TargetAdmissionStatus {
    match admission {
        C6RequiredAdmission::Portable => TargetAdmissionStatus::Portable,
        C6RequiredAdmission::HostDependent => TargetAdmissionStatus::HostDependent,
        C6RequiredAdmission::Rejected => TargetAdmissionStatus::Rejected,
    }
}

fn target_font_source_for_expected(source: C6ExpectedFontSource) -> TargetFontSource {
    match source {
        C6ExpectedFontSource::Embedded => TargetFontSource::Embedded,
        C6ExpectedFontSource::None => TargetFontSource::None,
        C6ExpectedFontSource::System => TargetFontSource::System,
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

fn proof_witness_digest(
    predicate: C6ProofPredicate,
    artifact_digest: [u8; 32],
    mechanisms: &BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
) -> [u8; 32] {
    let mut value = b"merman.c6-target-witness.v3\0".to_vec();
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
    use merman::svg::{
        DiagramThemeCompiler, DiagramThemeSpec, ThemeResourceLimitId, ThemeResourcePolicy,
    };
    use merman::{
        OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgRequest,
    };
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

    fn themed_state_identity() -> C6RenderIdentity {
        themed_state_identity_with_ceiling(ThemeResourcePolicy::interactive())
    }

    fn themed_state_identity_with_ceiling(ceiling: ThemeResourcePolicy) -> C6RenderIdentity {
        let document = themed_state_document_with_ceiling(ceiling);
        C6RenderIdentity::from_evidence(document.evidence())
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

    #[test]
    fn admission_digest_binds_the_effective_theme_resource_policy() {
        let limit = ThemeResourceLimitId::MaxEffectFilterRegionMagnitude;
        let policy = |maximum| {
            ThemeResourcePolicy::interactive()
                .with_limit(limit, maximum)
                .expect("valid filter-region ceiling")
        };

        let tighter = themed_state_identity_with_ceiling(policy(8));
        let looser = themed_state_identity_with_ceiling(policy(12));
        let same_as_tighter = themed_state_identity_with_ceiling(policy(8));

        assert_eq!(tighter.operation_digest(), looser.operation_digest());
        assert_ne!(tighter.admission_digest(), looser.admission_digest());
        assert_eq!(
            tighter.admission_digest(),
            same_as_tighter.admission_digest()
        );
    }

    #[test]
    fn c6_v1_operation_digest_keeps_its_canonical_renderer_vector() {
        let renderer = Renderer::new().with_runtime_policy(
            merman::runtime::RuntimePolicy::deterministic().with_fixed_unix_millis(42),
        );
        let output = renderer
            .render(RenderRequest::svg(
                "stateDiagram-v2\n[*] --> Ready\n",
                OperationControl::new(),
                SvgRequest::default(),
            ))
            .expect("deterministic state SVG should render");
        let RenderOutput::Svg(Some(output)) = output else {
            panic!("expected deterministic SVG output");
        };

        let identity = C6RenderIdentity::from_evidence(output.evidence());

        assert_eq!(
            identity.operation_digest(),
            &[
                226, 22, 14, 12, 178, 176, 221, 255, 238, 66, 59, 145, 182, 132, 104, 140, 49, 191,
                38, 99, 69, 44, 49, 59, 179, 76, 88, 217, 8, 31, 208, 80,
            ]
        );
        assert_ne!(identity.admission_digest(), &[0; 32]);
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
            operation_digest: [seed.wrapping_add(3); 32],
            admission_digest: [seed.wrapping_add(4); 32],
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
        let proof_predicate = expected_proof_predicate(key).expect("supported proof predicate");
        let artifact_digest = if matches!(
            key.target(),
            ExpectedOutputTarget::BrowserSvg | ExpectedOutputTarget::StandaloneSvg
        ) {
            sha256(format!("synthetic-svg-artifact-{key:?}"))
        } else {
            sha256(format!("synthetic-artifact-{key:?}"))
        };
        let target_receipt_digest = if matches!(
            key.target(),
            ExpectedOutputTarget::BrowserSvg | ExpectedOutputTarget::StandaloneSvg
        ) {
            group.svg_target_receipt_digest
        } else {
            sha256(format!("synthetic-target-receipt-{key:?}"))
        };
        let mut receipt = C6CellReceipt {
            key,
            render_group_key: group.key.clone(),
            render_group_digest: group.digest,
            operation_digest: group.operation_digest,
            admission_digest: group.admission_digest,
            admission: target_admission_for_required(expectation.required_admission()),
            admission_reason_count: 0,
            target_receipt_digest,
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
            font_source: target_font_source_for_expected(expectation.required_font_source()),
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

    fn reseal_book_admission(book: &mut C6ReceiptBook, admission_digest: [u8; 32]) {
        for group in book.groups.values_mut() {
            group.admission_digest = admission_digest;
            group.digest = group.canonical_digest();
        }
        let group_bindings = book
            .groups
            .iter()
            .map(|(key, group)| (key.clone(), (group.digest, group.admission_digest)))
            .collect::<BTreeMap<_, _>>();
        for receipt in book.cells.values_mut() {
            let (group_digest, group_admission_digest) = group_bindings
                .get(&receipt.render_group_key)
                .expect("cell render group");
            receipt.render_group_digest = *group_digest;
            receipt.admission_digest = *group_admission_digest;
            receipt.digest = receipt.canonical_digest();
        }
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
    fn render_group_matching_fails_closed_on_admission_digest_mismatch() {
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
        mismatched.admission_digest[0] ^= 1;
        assert!(!group.matches_render(&mismatched, document.standalone_svg_admission()));
    }

    #[test]
    fn receipt_canonical_digests_are_sensitive_to_admission_digest() {
        let (themes, acceptance) = load_catalogs();
        let book = synthetic_valid_book(&themes, &acceptance);
        let group = book.groups.values().next().expect("render group");
        let cell = book.cells.values().next().expect("cell receipt");

        let mut changed_group = group.clone();
        changed_group.admission_digest[0] ^= 1;
        assert_ne!(group.canonical_digest(), changed_group.canonical_digest());

        let mut changed_cell = cell.clone();
        changed_cell.admission_digest[0] ^= 1;
        assert_ne!(cell.canonical_digest(), changed_cell.canonical_digest());
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
        changed.admission = TargetAdmissionStatus::Rejected;
        assert_ne!(cell.canonical_digest(), changed.canonical_digest());

        let mut changed = cell.clone();
        changed.admission_reason_count += 1;
        assert_ne!(cell.canonical_digest(), changed.canonical_digest());

        let mut changed = cell.clone();
        changed.target_receipt_digest[0] ^= 1;
        assert_ne!(cell.canonical_digest(), changed.canonical_digest());

        let mut changed = cell.clone();
        changed.font_source = TargetFontSource::Mixed;
        assert_ne!(cell.canonical_digest(), changed.canonical_digest());
    }

    #[test]
    fn evaluator_fails_closed_on_resealed_admission_digest_mismatch() {
        let (themes, acceptance) = load_catalogs();
        let mut book = synthetic_valid_book(&themes, &acceptance);
        let key = acceptance
            .enforced_tranche()
            .cells()
            .next()
            .expect("enforced cell")
            .key();
        let cell = book.cells.get_mut(&key).expect("cell receipt");
        cell.admission_digest[0] ^= 1;
        cell.digest = cell.canonical_digest();

        assert_evidence_field(
            book.evaluate(&acceptance, &themes)
                .expect_err("re-sealed admission mismatch must fail closed"),
            "render-admission-digest",
        );
    }

    #[test]
    fn execution_digest_is_sensitive_to_admission_digest() {
        let (themes, acceptance) = load_catalogs();
        let first_book = synthetic_valid_book(&themes, &acceptance);
        let mut second_book = first_book.clone();
        reseal_book_admission(&mut second_book, [0x7b; 32]);
        let first = first_book
            .evaluate(&acceptance, &themes)
            .expect("first admission binding");
        let second = second_book
            .evaluate(&acceptance, &themes)
            .expect("second admission binding");

        assert_ne!(first.execution_digest(), second.execution_digest());
    }

    #[test]
    fn execution_digest_is_sensitive_to_target_receipt_digest() {
        let (themes, acceptance) = load_catalogs();
        let first_book = synthetic_valid_book(&themes, &acceptance);
        let mut second_book = first_book.clone();
        let cell = second_book
            .cells
            .values_mut()
            .find(|cell| {
                !matches!(
                    cell.key.target(),
                    ExpectedOutputTarget::BrowserSvg | ExpectedOutputTarget::StandaloneSvg
                )
            })
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
            .find(|cell| {
                matches!(
                    cell.key.target(),
                    ExpectedOutputTarget::BrowserSvg | ExpectedOutputTarget::StandaloneSvg
                )
            })
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

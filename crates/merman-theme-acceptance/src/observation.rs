use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

use merman::svg::{
    DiagramTheme, DocumentRenderReport, FontSource, RenderTargetKind, RenderedDocument,
    TargetAdmissionReport, TargetAdmissionStatus,
};
use merman_theme_fixtures::{
    C6AcceptanceCatalog, C6ArtifactAssertion, C6CellKey, C6EnforcedCell, C6ExpectedFontSource,
    C6RequiredAdmission, CatalogError, ExpectedOutputTarget, ReferenceThemeMechanism,
    ThemeFixtureCatalog,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum C6ObservedMechanismDisposition {
    Applied,
    NotApplicable,
    Rejected,
    Residual,
}

/// Target-local proof sealed by a real artifact checker.
///
/// This type deliberately carries no caller-supplied success flag. A target adapter can only
/// create it after its parser/geometry checker has accepted the final artifact bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct C6TargetProof {
    artifact_assertion: C6ArtifactAssertion,
    artifact_digest: [u8; 32],
    mechanism_digest: [u8; 32],
    mechanism_dispositions: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
}

impl C6TargetProof {
    pub(crate) fn verified(
        artifact_assertion: C6ArtifactAssertion,
        artifact_bytes: &[u8],
        mechanism_dispositions: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    ) -> Self {
        let mechanism_digest = mechanism_digest(&mechanism_dispositions);
        Self {
            artifact_assertion,
            artifact_digest: sha256(artifact_bytes),
            mechanism_digest,
            mechanism_dispositions,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct C6ObservedCell {
    key: C6CellKey,
    source_fixture_id: String,
    source_sha256: String,
    theme_input_sha256: String,
    recipe_fingerprint: [u8; 32],
    document_digest: [u8; 32],
    resource_fingerprint: [u8; 32],
    target: RenderTargetKind,
    admission_digest: [u8; 32],
    admission_reason_ids: Vec<String>,
    artifact_digest: [u8; 32],
    mechanism_digest: [u8; 32],
    target_proof_digest: [u8; 32],
    proof_stages: [C6ProofStage; 5],
    mechanism_dispositions: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    residual_ids: BTreeSet<String>,
    font_source: C6ExpectedFontSource,
    admission: C6RequiredAdmission,
    artifact_assertion: C6ArtifactAssertion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum C6ProofStage {
    SourceValidated,
    ThemeCompiled,
    DocumentSealed,
    TargetAdmitted,
    ArtifactVerified,
}

const C6_COMPLETE_PROOF_STAGES: [C6ProofStage; 5] = [
    C6ProofStage::SourceValidated,
    C6ProofStage::ThemeCompiled,
    C6ProofStage::DocumentSealed,
    C6ProofStage::TargetAdmitted,
    C6ProofStage::ArtifactVerified,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct C6ObservedReport {
    cells: BTreeMap<C6CellKey, C6ObservedCell>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct C6ExecutionIdentity {
    recipe_fingerprint: [u8; 32],
    document_digest: [u8; 32],
    resource_fingerprint: [u8; 32],
}

impl C6ExecutionIdentity {
    fn from_renderer(theme: &DiagramTheme, document: &RenderedDocument, sealed_svg: &str) -> Self {
        Self {
            recipe_fingerprint: *theme.recipe_fingerprint().as_bytes(),
            document_digest: sha256(sealed_svg),
            resource_fingerprint: *document.resource_fingerprint().as_bytes(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C6ExecutionReport {
    verified_cell_count: usize,
    enforced_cell_count: usize,
    execution_digest: [u8; 32],
}

#[derive(Debug, thiserror::Error)]
pub enum C6RuntimeError {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error("the enforced C6 tranche is empty")]
    EmptyTranche,
    #[error("no target adapter exists for enforced C6 cell {key:?}")]
    UnsupportedEnforcedCell { key: C6CellKey },
    #[error("the current C6 vertical slice requires one shared source fixture")]
    MultipleSourceFixtures,
    #[error("the enforced C6 tranche does not contain the required PNG target")]
    MissingPngTarget,
    #[error("C6 evidence for {key:?} failed invariant `{field}`")]
    EvidenceMismatch { key: C6CellKey, field: &'static str },
    #[error("the C6 runtime produced duplicate evidence for {key:?}")]
    DuplicateObservedCell { key: C6CellKey },
    #[error(
        "C6 runtime coverage differed from the enforced tranche; missing={missing:?}, unexpected={unexpected:?}"
    )]
    ObservationCoverageMismatch {
        missing: Vec<C6CellKey>,
        unexpected: Vec<C6CellKey>,
    },
}

impl C6ExecutionReport {
    pub const fn verified_cell_count(&self) -> usize {
        self.verified_cell_count
    }

    pub const fn enforced_cell_count(&self) -> usize {
        self.enforced_cell_count
    }

    pub const fn execution_digest(&self) -> &[u8; 32] {
        &self.execution_digest
    }
}

pub(crate) fn observe_cell_from_reports(
    enforced: &C6EnforcedCell,
    catalog: &ThemeFixtureCatalog,
    recipe_fingerprint: Option<merman::svg::ThemeRecipeFingerprint>,
    sealed_svg: &str,
    document: &DocumentRenderReport,
    admission: &TargetAdmissionReport,
    proof: C6TargetProof,
) -> Result<C6ObservedCell, C6RuntimeError> {
    let key = enforced.key();
    let fixture = catalog
        .fixture(enforced.source_fixture_id())
        .ok_or_else(|| evidence_mismatch(key, "source-fixture"))?;
    require_evidence(
        key,
        "enforced-target-artifact",
        proof.artifact_assertion == expected_artifact_assertion(key.target()),
    )?;
    require_evidence(
        key,
        "admitted-target-artifact",
        artifact_assertion_for_render_target(admission.target()) == Some(proof.artifact_assertion),
    )?;
    require_evidence(key, "document-residuals", document.residuals().is_empty())?;

    let source_digest = sha256(catalog.source_text(fixture.id())?.as_bytes());
    require_evidence(
        key,
        "source-sha256",
        hex_digest(&source_digest) == fixture.source_sha256(),
    )?;
    let theme_input_sha256 = fixture
        .theme_input_sha256()
        .ok_or_else(|| evidence_mismatch(key, "theme-input-sha256"))?
        .to_owned();

    let font_source = match document.prepared_text_used_font_sources() {
        [FontSource::Embedded] => C6ExpectedFontSource::Embedded,
        [FontSource::System] => C6ExpectedFontSource::System,
        [] => C6ExpectedFontSource::None,
        _ => return Err(evidence_mismatch(key, "prepared-font-source")),
    };
    let (observed_admission, admission_reason_ids, observed_admission_digest) =
        observed_admission(key, admission)?;
    let recipe_fingerprint =
        recipe_fingerprint.ok_or_else(|| evidence_mismatch(key, "recipe-fingerprint"))?;

    require_evidence(key, "artifact-digest", proof.artifact_digest != [0; 32])?;
    require_evidence(key, "mechanism-digest", proof.mechanism_digest != [0; 32])?;
    let sealed_target_proof_digest = target_proof_digest(
        proof.artifact_assertion,
        proof.artifact_digest,
        proof.mechanism_digest,
    );
    Ok(C6ObservedCell {
        key,
        source_fixture_id: fixture.id().to_owned(),
        source_sha256: fixture.source_sha256().to_owned(),
        theme_input_sha256,
        recipe_fingerprint: *recipe_fingerprint.as_bytes(),
        document_digest: sha256(sealed_svg),
        resource_fingerprint: *document.resource_fingerprint().as_bytes(),
        target: admission.target(),
        admission_digest: observed_admission_digest,
        admission_reason_ids,
        artifact_digest: proof.artifact_digest,
        mechanism_digest: proof.mechanism_digest,
        target_proof_digest: sealed_target_proof_digest,
        proof_stages: C6_COMPLETE_PROOF_STAGES,
        mechanism_dispositions: proof.mechanism_dispositions,
        residual_ids: BTreeSet::new(),
        font_source,
        admission: observed_admission,
        artifact_assertion: proof.artifact_assertion,
    })
}

impl C6ObservedReport {
    pub(crate) fn from_enforced_cells(
        tranche: &merman_theme_fixtures::C6EnforcedTranche,
        cells: impl IntoIterator<Item = C6ObservedCell>,
    ) -> Result<Self, C6RuntimeError> {
        let mut indexed = BTreeMap::new();
        for cell in cells {
            let key = cell.key;
            if indexed.insert(key, cell).is_some() {
                return Err(C6RuntimeError::DuplicateObservedCell { key });
            }
        }
        let expected = tranche
            .cells()
            .map(C6EnforcedCell::key)
            .collect::<BTreeSet<_>>();
        let actual = indexed.keys().copied().collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(C6RuntimeError::ObservationCoverageMismatch {
                missing: expected.difference(&actual).copied().collect(),
                unexpected: actual.difference(&expected).copied().collect(),
            });
        }
        Ok(Self { cells: indexed })
    }

    pub(crate) fn evaluate(
        &self,
        acceptance: &C6AcceptanceCatalog,
        theme_catalog: &ThemeFixtureCatalog,
        theme: &DiagramTheme,
        document: &RenderedDocument,
        sealed_svg: &str,
    ) -> Result<C6ExecutionReport, C6RuntimeError> {
        if self.cells.is_empty() {
            return Err(C6RuntimeError::EmptyTranche);
        }
        self.evaluate_with_identity(
            acceptance,
            theme_catalog,
            C6ExecutionIdentity::from_renderer(theme, document, sealed_svg),
        )
    }

    fn evaluate_with_identity(
        &self,
        acceptance: &C6AcceptanceCatalog,
        theme_catalog: &ThemeFixtureCatalog,
        expected_identity: C6ExecutionIdentity,
    ) -> Result<C6ExecutionReport, C6RuntimeError> {
        for enforced in acceptance.enforced_tranche().cells() {
            let key = enforced.key();
            let observed = self
                .cells
                .get(&key)
                .ok_or_else(|| evidence_mismatch(key, "enforced-observation"))?;
            let expectation = acceptance
                .specification()
                .cell(key)
                .ok_or_else(|| evidence_mismatch(key, "enforced-expectation"))?
                .expectation();
            let fixture = theme_catalog
                .fixture(enforced.source_fixture_id())
                .ok_or_else(|| evidence_mismatch(key, "source-fixture"))?;
            require_evidence(
                key,
                "source-fixture-id",
                observed.source_fixture_id == enforced.source_fixture_id(),
            )?;
            require_evidence(
                key,
                "source-sha256",
                observed.source_sha256 == fixture.source_sha256(),
            )?;
            require_evidence(
                key,
                "theme-input-sha256",
                observed.theme_input_sha256
                    == fixture
                        .theme_input_sha256()
                        .ok_or_else(|| evidence_mismatch(key, "theme-input-sha256"))?,
            )?;
            require_evidence(
                key,
                "recipe-fingerprint",
                observed.recipe_fingerprint == expected_identity.recipe_fingerprint,
            )?;
            require_evidence(
                key,
                "document-digest",
                observed.document_digest == expected_identity.document_digest,
            )?;
            require_evidence(
                key,
                "resource-fingerprint",
                observed.resource_fingerprint == expected_identity.resource_fingerprint,
            )?;
            require_evidence(
                key,
                "target",
                observed.target == render_target_for_artifact(observed.artifact_assertion),
            )?;
            require_evidence(
                key,
                "target-expectation",
                observed.artifact_assertion == expected_artifact_assertion(enforced.key().target()),
            )?;
            require_evidence(
                key,
                "mechanism-count",
                observed.mechanism_dispositions.len() == expectation.mechanism_requirements().len(),
            )?;
            for (mechanism, disposition) in expectation.mechanism_requirements() {
                require_evidence(
                    key,
                    "mechanism-disposition",
                    observed.mechanism_dispositions.get(mechanism)
                        == Some(&expected_observed_disposition(*disposition)),
                )?;
            }
            require_evidence(
                key,
                "residual-ids",
                observed.residual_ids == *expectation.expected_residual_ids(),
            )?;
            require_evidence(
                key,
                "font-source",
                observed.font_source == expectation.required_font_source(),
            )?;
            require_evidence(
                key,
                "target-admission",
                observed.admission == expectation.required_admission(),
            )?;
            require_evidence(
                key,
                "target-admission-reasons",
                observed.admission != C6RequiredAdmission::Portable
                    || observed.admission_reason_ids.is_empty(),
            )?;
            require_evidence(
                key,
                "admission-digest",
                observed.admission_digest
                    == admission_digest_from_parts(
                        observed.target,
                        observed.admission,
                        &observed.admission_reason_ids,
                    ),
            )?;
            require_evidence(
                key,
                "artifact-assertion",
                observed.artifact_assertion == expectation.required_artifact_assertion(),
            )?;
            require_evidence(key, "artifact-digest", observed.artifact_digest != [0; 32])?;
            require_evidence(
                key,
                "mechanism-digest",
                observed.mechanism_digest == mechanism_digest(&observed.mechanism_dispositions),
            )?;
            require_evidence(
                key,
                "target-proof-digest",
                observed.target_proof_digest
                    == target_proof_digest(
                        observed.artifact_assertion,
                        observed.artifact_digest,
                        observed.mechanism_digest,
                    ),
            )?;
            require_evidence(
                key,
                "proof-stages",
                observed.proof_stages == C6_COMPLETE_PROOF_STAGES,
            )?;
        }

        let mut digest_input = b"merman.c6-execution-report.v1".to_vec();
        for cell in self.cells.values() {
            append_len_prefixed(
                &mut digest_input,
                cell.key.theme().reference_name().as_bytes(),
            );
            append_len_prefixed(&mut digest_input, cell.source_fixture_id.as_bytes());
            digest_input.extend_from_slice(cell.source_sha256.as_bytes());
            digest_input.extend_from_slice(cell.theme_input_sha256.as_bytes());
            digest_input.extend_from_slice(&cell.recipe_fingerprint);
            digest_input.extend_from_slice(&cell.document_digest);
            digest_input.extend_from_slice(&cell.resource_fingerprint);
            append_len_prefixed(&mut digest_input, cell.target.id().as_bytes());
            digest_input.extend_from_slice(&cell.admission_digest);
            for reason_id in &cell.admission_reason_ids {
                append_len_prefixed(&mut digest_input, reason_id.as_bytes());
            }
            digest_input.extend_from_slice(&cell.artifact_digest);
            digest_input.extend_from_slice(&cell.mechanism_digest);
            digest_input.extend_from_slice(&cell.target_proof_digest);
            for stage in cell.proof_stages {
                digest_input.push(stage as u8);
            }
        }
        Ok(C6ExecutionReport {
            verified_cell_count: self.cells.len(),
            enforced_cell_count: acceptance.enforced_tranche().cells().len(),
            execution_digest: sha256(digest_input),
        })
    }
}

fn sha256(bytes: impl AsRef<[u8]>) -> [u8; 32] {
    Sha256::digest(bytes.as_ref()).into()
}

fn hex_digest(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn admission_status_id(admission: C6RequiredAdmission) -> &'static str {
    match admission {
        C6RequiredAdmission::Portable => "portable",
        C6RequiredAdmission::HostDependent => "host-dependent",
        C6RequiredAdmission::Rejected => "rejected",
    }
}

fn admission_digest_from_parts(
    target: RenderTargetKind,
    admission: C6RequiredAdmission,
    reason_ids: &[String],
) -> [u8; 32] {
    let mut value = format!("{}|{}", target.id(), admission_status_id(admission));
    for reason_id in reason_ids {
        value.push('|');
        value.push_str(reason_id);
    }
    sha256(value)
}

fn target_proof_digest(
    assertion: C6ArtifactAssertion,
    artifact_digest: [u8; 32],
    mechanism_digest: [u8; 32],
) -> [u8; 32] {
    let assertion_id = match assertion {
        C6ArtifactAssertion::BrowserSvgDom => "browser-svg-dom",
        C6ArtifactAssertion::JpegImage => "jpeg-image",
        C6ArtifactAssertion::PdfDocument => "pdf-document",
        C6ArtifactAssertion::PngImage => "png-image",
        C6ArtifactAssertion::StandaloneSvgDocument => "standalone-svg-document",
    };
    let mut value = b"merman.c6-target-proof.v1\0".to_vec();
    append_len_prefixed(&mut value, assertion_id.as_bytes());
    value.extend_from_slice(&artifact_digest);
    value.extend_from_slice(&mechanism_digest);
    sha256(value)
}

fn mechanism_digest(
    mechanisms: &BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
) -> [u8; 32] {
    let mut value = String::new();
    for (mechanism, disposition) in mechanisms {
        value.push_str(reference_mechanism_id(*mechanism));
        value.push('=');
        value.push_str(match disposition {
            C6ObservedMechanismDisposition::Applied => "applied",
            C6ObservedMechanismDisposition::NotApplicable => "not-applicable",
            C6ObservedMechanismDisposition::Rejected => "rejected",
            C6ObservedMechanismDisposition::Residual => "residual",
        });
        value.push('|');
    }
    sha256(value)
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

fn expected_artifact_assertion(target: ExpectedOutputTarget) -> C6ArtifactAssertion {
    match target {
        ExpectedOutputTarget::BrowserSvg => C6ArtifactAssertion::BrowserSvgDom,
        ExpectedOutputTarget::StandaloneSvg => C6ArtifactAssertion::StandaloneSvgDocument,
        ExpectedOutputTarget::Png => C6ArtifactAssertion::PngImage,
        ExpectedOutputTarget::Jpeg => C6ArtifactAssertion::JpegImage,
        ExpectedOutputTarget::Pdf => C6ArtifactAssertion::PdfDocument,
    }
}

fn render_target_for_artifact(assertion: C6ArtifactAssertion) -> RenderTargetKind {
    match assertion {
        C6ArtifactAssertion::BrowserSvgDom | C6ArtifactAssertion::StandaloneSvgDocument => {
            RenderTargetKind::Svg
        }
        C6ArtifactAssertion::PngImage => RenderTargetKind::Png,
        C6ArtifactAssertion::JpegImage => RenderTargetKind::Jpeg,
        C6ArtifactAssertion::PdfDocument => RenderTargetKind::Pdf,
    }
}

fn artifact_assertion_for_render_target(target: RenderTargetKind) -> Option<C6ArtifactAssertion> {
    match target {
        RenderTargetKind::Svg => Some(C6ArtifactAssertion::StandaloneSvgDocument),
        RenderTargetKind::Png => Some(C6ArtifactAssertion::PngImage),
        RenderTargetKind::Jpeg => Some(C6ArtifactAssertion::JpegImage),
        RenderTargetKind::Pdf => Some(C6ArtifactAssertion::PdfDocument),
        _ => None,
    }
}

fn observed_admission(
    key: C6CellKey,
    admission: &TargetAdmissionReport,
) -> Result<(C6RequiredAdmission, Vec<String>, [u8; 32]), C6RuntimeError> {
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
    let digest = admission_digest_from_parts(admission.target(), observed, &reason_ids);
    Ok((observed, reason_ids, digest))
}

fn evidence_mismatch(key: C6CellKey, field: &'static str) -> C6RuntimeError {
    C6RuntimeError::EvidenceMismatch { key, field }
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

fn append_len_prefixed(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    output.extend_from_slice(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    use merman_theme_fixtures::{C6ProofFamily, C6ProofTheme};

    fn themes_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("themes")
    }

    fn load_catalogs() -> (ThemeFixtureCatalog, C6AcceptanceCatalog) {
        let theme_catalog = ThemeFixtureCatalog::load(themes_root()).expect("load theme fixtures");
        let acceptance =
            C6AcceptanceCatalog::load(&theme_catalog).expect("load C6 acceptance catalog");
        (theme_catalog, acceptance)
    }

    fn synthetic_valid_report(
        theme_catalog: &ThemeFixtureCatalog,
        acceptance: &C6AcceptanceCatalog,
    ) -> (C6ObservedReport, C6ExecutionIdentity) {
        let identity = C6ExecutionIdentity {
            recipe_fingerprint: [1; 32],
            document_digest: [2; 32],
            resource_fingerprint: [3; 32],
        };
        let cells = acceptance.enforced_tranche().cells().map(|enforced| {
            let key = enforced.key();
            let fixture = theme_catalog
                .fixture(enforced.source_fixture_id())
                .expect("enforced fixture");
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
            let mechanism_digest = mechanism_digest(&mechanism_dispositions);
            let artifact_assertion = expectation.required_artifact_assertion();
            let target = render_target_for_artifact(artifact_assertion);
            let admission = expectation.required_admission();
            let admission_reason_ids = Vec::new();
            let artifact_digest = sha256(format!("synthetic-c6-artifact-{key:?}"));
            C6ObservedCell {
                key,
                source_fixture_id: fixture.id().to_owned(),
                source_sha256: fixture.source_sha256().to_owned(),
                theme_input_sha256: fixture
                    .theme_input_sha256()
                    .expect("enforced theme input digest")
                    .to_owned(),
                recipe_fingerprint: identity.recipe_fingerprint,
                document_digest: identity.document_digest,
                resource_fingerprint: identity.resource_fingerprint,
                target,
                admission_digest: admission_digest_from_parts(
                    target,
                    admission,
                    &admission_reason_ids,
                ),
                admission_reason_ids,
                artifact_digest,
                mechanism_digest,
                target_proof_digest: target_proof_digest(
                    artifact_assertion,
                    artifact_digest,
                    mechanism_digest,
                ),
                proof_stages: C6_COMPLETE_PROOF_STAGES,
                mechanism_dispositions,
                residual_ids: expectation.expected_residual_ids().clone(),
                font_source: expectation.required_font_source(),
                admission,
                artifact_assertion,
            }
        });
        (
            C6ObservedReport::from_enforced_cells(acceptance.enforced_tranche(), cells)
                .expect("synthetic report covers the enforced tranche"),
            identity,
        )
    }

    fn assert_evidence_field(error: C6RuntimeError, expected_field: &'static str) {
        match error {
            C6RuntimeError::EvidenceMismatch { field, .. } => {
                assert_eq!(field, expected_field);
            }
            other => panic!("expected evidence mismatch for {expected_field}, got {other}"),
        }
    }

    fn assert_mutation_rejected(
        report: &C6ObservedReport,
        acceptance: &C6AcceptanceCatalog,
        theme_catalog: &ThemeFixtureCatalog,
        identity: C6ExecutionIdentity,
        field: &'static str,
        mutate: impl FnOnce(&mut C6ObservedCell),
    ) {
        let key = acceptance
            .enforced_tranche()
            .cells()
            .next()
            .expect("enforced cell")
            .key();
        let mut tampered = report.clone();
        mutate(tampered.cells.get_mut(&key).expect("observed cell"));
        let error = tampered
            .evaluate_with_identity(acceptance, theme_catalog, identity)
            .expect_err("tampered C6 evidence must be rejected");
        assert_evidence_field(error, field);
    }

    fn replace_first_hex_digit(value: &mut String) {
        let replacement = if value.starts_with('0') { "1" } else { "0" };
        value.replace_range(..1, replacement);
    }

    #[test]
    fn evaluator_accepts_a_complete_identity_bound_report() {
        let (theme_catalog, acceptance) = load_catalogs();
        let (report, identity) = synthetic_valid_report(&theme_catalog, &acceptance);

        let evaluation = report
            .evaluate_with_identity(&acceptance, &theme_catalog, identity)
            .expect("complete synthetic verifier input");

        assert_eq!(evaluation.verified_cell_count(), 4);
        assert_eq!(evaluation.enforced_cell_count(), 4);
        assert_ne!(evaluation.execution_digest(), &[0; 32]);
    }

    #[test]
    fn evaluator_rejects_tampered_identity_and_proof_fields() {
        let (theme_catalog, acceptance) = load_catalogs();
        let (report, identity) = synthetic_valid_report(&theme_catalog, &acceptance);

        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "source-fixture-id",
            |cell| cell.source_fixture_id.push_str("-tampered"),
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "source-sha256",
            |cell| replace_first_hex_digit(&mut cell.source_sha256),
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "theme-input-sha256",
            |cell| replace_first_hex_digit(&mut cell.theme_input_sha256),
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "recipe-fingerprint",
            |cell| cell.recipe_fingerprint[0] ^= 1,
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "document-digest",
            |cell| cell.document_digest[0] ^= 1,
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "resource-fingerprint",
            |cell| cell.resource_fingerprint[0] ^= 1,
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "target",
            |cell| {
                cell.target = if cell.target == RenderTargetKind::Svg {
                    RenderTargetKind::Png
                } else {
                    RenderTargetKind::Svg
                };
            },
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "target-expectation",
            |cell| {
                cell.artifact_assertion = C6ArtifactAssertion::BrowserSvgDom;
                cell.target = RenderTargetKind::Svg;
            },
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "mechanism-disposition",
            |cell| {
                let mechanism = *cell
                    .mechanism_dispositions
                    .keys()
                    .next()
                    .expect("required mechanism");
                let disposition = cell
                    .mechanism_dispositions
                    .get(&mechanism)
                    .copied()
                    .expect("required mechanism disposition");
                let tampered = if disposition == C6ObservedMechanismDisposition::Residual {
                    C6ObservedMechanismDisposition::Applied
                } else {
                    C6ObservedMechanismDisposition::Residual
                };
                cell.mechanism_dispositions.insert(mechanism, tampered);
            },
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "residual-ids",
            |cell| {
                cell.residual_ids.insert("tampered-residual".to_owned());
            },
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "font-source",
            |cell| cell.font_source = C6ExpectedFontSource::System,
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "target-admission",
            |cell| cell.admission = C6RequiredAdmission::HostDependent,
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "target-admission-reasons",
            |cell| cell.admission_reason_ids.push("tampered-reason".to_owned()),
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "admission-digest",
            |cell| cell.admission_digest[0] ^= 1,
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "artifact-digest",
            |cell| cell.artifact_digest = [0; 32],
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "mechanism-digest",
            |cell| cell.mechanism_digest[0] ^= 1,
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "target-proof-digest",
            |cell| cell.target_proof_digest[0] ^= 1,
        );
        assert_mutation_rejected(
            &report,
            &acceptance,
            &theme_catalog,
            identity,
            "proof-stages",
            |cell| cell.proof_stages[0] = C6ProofStage::ArtifactVerified,
        );
    }

    #[test]
    fn observation_batch_rejects_duplicate_missing_and_unexpected_cells() {
        let (theme_catalog, acceptance) = load_catalogs();
        let (report, _) = synthetic_valid_report(&theme_catalog, &acceptance);
        let cells = report.cells.values().cloned().collect::<Vec<_>>();

        let mut duplicate = cells.clone();
        duplicate.push(cells[0].clone());
        assert!(matches!(
            C6ObservedReport::from_enforced_cells(acceptance.enforced_tranche(), duplicate),
            Err(C6RuntimeError::DuplicateObservedCell { .. })
        ));

        let mut missing = cells.clone();
        let missing_key = missing.pop().expect("observed cell").key;
        match C6ObservedReport::from_enforced_cells(acceptance.enforced_tranche(), missing) {
            Err(C6RuntimeError::ObservationCoverageMismatch {
                missing,
                unexpected,
            }) => {
                assert_eq!(missing, vec![missing_key]);
                assert!(unexpected.is_empty());
            }
            other => panic!("expected missing-cell coverage error, got {other:?}"),
        }

        let mut unexpected = cells;
        let mut unexpected_cell = unexpected[0].clone();
        let unexpected_key = C6CellKey::new(
            C6ProofTheme::Spotless,
            C6ProofFamily::Flowchart,
            ExpectedOutputTarget::BrowserSvg,
        );
        unexpected_cell.key = unexpected_key;
        unexpected.push(unexpected_cell);
        match C6ObservedReport::from_enforced_cells(acceptance.enforced_tranche(), unexpected) {
            Err(C6RuntimeError::ObservationCoverageMismatch {
                missing,
                unexpected,
            }) => {
                assert!(missing.is_empty());
                assert_eq!(unexpected, vec![unexpected_key]);
            }
            other => panic!("expected unexpected-cell coverage error, got {other:?}"),
        }
    }
}

//! Production admission inventory for the exact catalog recipes.
//!
//! Admission is a prerequisite for preset qualification, not a qualification receipt. These
//! observations never populate the public catalog's qualified cells or the fixed C6a ledger.
//! Scoped qualification additionally checks actual SVG and PNG output under a named host profile.

use merman::svg::{DiagramThemeCompiler, ThemePreset, ThemeRecipeFingerprint};
use merman::{
    DiagramFamilyId, Engine, MermaidConfig, OperationControl, RenderArtifactKind, RenderOutput,
    RenderRequest, Renderer, TargetAdmissionReason, TargetAdmissionReceipt, TargetAdmissionStatus,
};
use sha2::{Digest as _, Sha256};

#[path = "support/preset_state_proof.rs"]
mod state_proof;

const QUALIFICATION_SCHEMA_REVISION: u32 = 1;
const HOST_PROFILE: &str = "native-state-system-fonts-v1";

/// One frozen representative source used to inspect a catalog preset on both native targets.
#[derive(Debug, Clone, Copy)]
pub struct PresetQualificationSpec {
    family: DiagramFamilyId,
    source_id: &'static str,
    source: &'static str,
}

impl PresetQualificationSpec {
    pub const fn family_id(self) -> DiagramFamilyId {
        self.family
    }

    pub const fn source_id(self) -> &'static str {
        self.source_id
    }
}

const STATE_SPEC: PresetQualificationSpec = PresetQualificationSpec {
    family: DiagramFamilyId::STATE,
    source_id: "preset-native-state-v1",
    source: "stateDiagram-v2\n[*] --> Active\nActive --> [*]\n",
};

const SPECS: [PresetQualificationSpec; 3] = [
    PresetQualificationSpec {
        family: DiagramFamilyId::FLOWCHART,
        source_id: "preset-native-flowchart-v1",
        source: "flowchart LR\nA[Alpha] --> B[Beta]\n",
    },
    STATE_SPEC,
    PresetQualificationSpec {
        family: DiagramFamilyId::SEQUENCE,
        source_id: "preset-native-sequence-v1",
        source: "sequenceDiagram\nAlice->>Bob: Hello\nBob-->>Alice: World\n",
    },
];

/// Native admission for one actual artifact, including the independent residual domains.
#[derive(Debug, Clone)]
pub struct PresetAdmissionObservation {
    spec: PresetQualificationSpec,
    source_digest: [u8; 32],
    receipt: TargetAdmissionReceipt,
    theme_residual_count: usize,
    source_residual_count: usize,
    bridge_residual_count: usize,
    mermaid_residual_count: usize,
}

impl PresetAdmissionObservation {
    pub const fn spec(&self) -> PresetQualificationSpec {
        self.spec
    }

    pub const fn source_digest(&self) -> &[u8; 32] {
        &self.source_digest
    }

    pub const fn artifact_kind(&self) -> RenderArtifactKind {
        self.receipt.artifact_kind()
    }

    pub const fn status(&self) -> TargetAdmissionStatus {
        self.receipt.status()
    }

    pub fn reasons(&self) -> &[TargetAdmissionReason] {
        self.receipt.reasons()
    }

    /// Borrows the production receipt; presence alone does not establish portable admission.
    pub const fn target_receipt(&self) -> &TargetAdmissionReceipt {
        &self.receipt
    }

    pub const fn theme_residual_count(&self) -> usize {
        self.theme_residual_count
    }

    pub const fn source_residual_count(&self) -> usize {
        self.source_residual_count
    }

    pub const fn bridge_residual_count(&self) -> usize {
        self.bridge_residual_count
    }

    pub const fn mermaid_residual_count(&self) -> usize {
        self.mermaid_residual_count
    }
}

/// Admission observations bound to one exact compiled catalog recipe and resource catalog.
///
/// Even an entirely portable report still needs semantic and visual qualification assertions.
#[derive(Debug, Clone)]
pub struct PresetAdmissionReport {
    preset: ThemePreset,
    recipe_fingerprint: ThemeRecipeFingerprint,
    resource_fingerprint: [u8; 32],
    observations: Vec<PresetAdmissionObservation>,
}

impl PresetAdmissionReport {
    pub const fn preset(&self) -> ThemePreset {
        self.preset
    }

    pub const fn recipe_fingerprint(&self) -> ThemeRecipeFingerprint {
        self.recipe_fingerprint
    }

    pub const fn resource_fingerprint(&self) -> &[u8; 32] {
        &self.resource_fingerprint
    }

    pub fn observations(&self) -> &[PresetAdmissionObservation] {
        &self.observations
    }
}

/// Execution-local qualification of the declared State SVG/PNG host-dependent profile.
///
/// Only the production runner can construct this receipt. It is not serializable and grants no
/// portable, all-family, stable-catalog, or C6a claim. Target receipts retain exact artifact,
/// document, resource, font-source and admission evidence from this execution.
///
/// ```compile_fail
/// use merman_theme_acceptance::PresetQualificationReceipt;
/// let forged = PresetQualificationReceipt { schema_revision: 1, report: todo!() };
/// ```
#[derive(Debug, Clone)]
pub struct PresetQualificationReceipt {
    schema_revision: u32,
    report: PresetAdmissionReport,
}

impl PresetQualificationReceipt {
    pub const fn profile_id(&self) -> &'static str {
        HOST_PROFILE
    }

    pub const fn schema_revision(&self) -> u32 {
        self.schema_revision
    }

    pub const fn report(&self) -> &PresetAdmissionReport {
        &self.report
    }

    /// Checks recipe/resource freshness within this executable, not across renderer builds.
    /// Persisted or published qualification requires a new run in the candidate build.
    pub fn is_current(&self) -> bool {
        self.schema_revision == QUALIFICATION_SCHEMA_REVISION
            && DiagramThemeCompiler::new()
                .compile_preset(self.report.preset)
                .is_ok_and(|theme| {
                    theme.recipe_fingerprint() == self.report.recipe_fingerprint
                        && theme.report().font_catalog_fingerprint().as_bytes()
                            == &self.report.resource_fingerprint
                })
    }
}

/// Qualifies the three native candidates only for the declared host-dependent State profile.
///
/// No font resources, compatibility fields, or rules are injected. Missing system glyphs or
/// semantic/visual failures reject this execution; host dependence is never promoted to Portable.
pub fn run_preset_qualification(
    preset: ThemePreset,
) -> Result<PresetQualificationReceipt, PresetAdmissionError> {
    if !matches!(
        preset,
        ThemePreset::Brutalist | ThemePreset::Spotless | ThemePreset::Cyberpunk
    ) {
        return Err(PresetAdmissionError {
            preset: preset.id(),
            source_id: "catalog",
            stage: "qualification-scope",
            detail: "preset has no declared native State qualification profile".to_owned(),
        });
    }
    let report = execute_preset(preset, &[STATE_SPEC], true)?;
    Ok(PresetQualificationReceipt {
        schema_revision: QUALIFICATION_SCHEMA_REVISION,
        report,
    })
}

#[derive(Debug, thiserror::Error)]
#[error("preset {preset} at {source_id}/{stage}: {detail}")]
pub struct PresetAdmissionError {
    preset: &'static str,
    source_id: &'static str,
    stage: &'static str,
    detail: String,
}

/// Executes the catalog's own recipe without substituting a C6 proof recipe or modifying rules.
///
/// The fixed profile disables HTML labels and retains the default deterministic environment and
/// font policy. Best-effort production rendering allows inspection of rejected admission receipts;
/// it does not upgrade their status. Failures to produce an artifact remain explicit errors.
pub fn inspect_preset_admission(
    preset: ThemePreset,
) -> Result<PresetAdmissionReport, PresetAdmissionError> {
    execute_preset(preset, &SPECS, false)
}

fn execute_preset(
    preset: ThemePreset,
    specs: &[PresetQualificationSpec],
    qualify: bool,
) -> Result<PresetAdmissionReport, PresetAdmissionError> {
    let failure = |source_id, stage, detail| PresetAdmissionError {
        preset: preset.id(),
        source_id,
        stage,
        detail,
    };
    let theme = DiagramThemeCompiler::new()
        .compile_preset(preset)
        .map_err(|error| failure("catalog", "compile", error.to_string()))?;
    let mut report = PresetAdmissionReport {
        preset,
        recipe_fingerprint: theme.recipe_fingerprint(),
        resource_fingerprint: *theme.report().font_catalog_fingerprint().as_bytes(),
        observations: Vec::with_capacity(specs.len() * 2),
    };
    let renderer = Renderer::new().with_engine(Engine::new().with_site_config(
        MermaidConfig::from_value(serde_json::json!({"htmlLabels": false})),
    ));
    for &spec in specs {
        let output = renderer
            .render(
                RenderRequest::document(spec.source, OperationControl::new(), Default::default())
                    .with_theme(theme.clone()),
            )
            .map_err(|error| failure(spec.source_id, "render", error.to_string()))?;
        let RenderOutput::Document(Some(document)) = output else {
            return Err(failure(
                spec.source_id,
                "render",
                "missing document".to_owned(),
            ));
        };
        if document.evidence().family_id() != spec.family
            || document.evidence().theme_recipe_fingerprint() != Some(report.recipe_fingerprint)
        {
            return Err(failure(
                spec.source_id,
                "identity",
                "render identity mismatch".to_owned(),
            ));
        }
        let png = document
            .export_png(&Default::default(), OperationControl::new())
            .map_err(|error| failure(spec.source_id, "png", error.to_string()))?;
        let svg_receipt = document.standalone_svg_admission();
        if svg_receipt.font_catalog_fingerprint().as_bytes() != &report.resource_fingerprint
            || png.admission().document_digest() != svg_receipt.document_digest()
            || png.admission().resource_fingerprint() != svg_receipt.resource_fingerprint()
            || png.admission().font_catalog_fingerprint() != svg_receipt.font_catalog_fingerprint()
        {
            return Err(failure(
                spec.source_id,
                "identity",
                "native projection mismatch".to_owned(),
            ));
        }
        let evidence = merman::__theme_acceptance::theme_acceptance_evidence(document.evidence());
        if qualify {
            state_proof::verify(preset, &document, &png)
                .map_err(|error| failure(spec.source_id, "qualification", error.to_string()))?;
            if !evidence.family().is_satisfied()
                || !evidence.root().is_satisfied()
                || evidence.family().residual_count() != 0
                || evidence.source_residual_count() != 0
                || evidence.compatibility_residual_count() != 0
                || evidence.mermaid_compatibility_residual_count() != 0
            {
                return Err(failure(
                    spec.source_id,
                    "qualification",
                    "unsatisfied theme evidence".to_owned(),
                ));
            }
        }
        for (kind, bytes, receipt) in [
            (
                RenderArtifactKind::Svg,
                document.svg().as_bytes(),
                svg_receipt,
            ),
            (RenderArtifactKind::Png, png.bytes(), png.admission()),
        ] {
            let digest: [u8; 32] = Sha256::digest(bytes).into();
            if receipt.artifact_kind() != kind || digest != receipt.artifact_digest() {
                return Err(failure(
                    spec.source_id,
                    "identity",
                    "artifact digest mismatch".to_owned(),
                ));
            }
            report.observations.push(PresetAdmissionObservation {
                spec,
                source_digest: Sha256::digest(spec.source.as_bytes()).into(),
                receipt: receipt.clone(),
                theme_residual_count: evidence.family().residual_count(),
                source_residual_count: evidence.source_residual_count(),
                bridge_residual_count: evidence.compatibility_residual_count(),
                mermaid_residual_count: evidence.mermaid_compatibility_residual_count(),
            });
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualification_freshness_rejects_recipe_resource_and_schema_drift() {
        let original = run_preset_qualification(ThemePreset::Brutalist).unwrap();
        let mut stale = original.clone();
        stale.report.recipe_fingerprint = DiagramThemeCompiler::new()
            .compile_preset(ThemePreset::Spotless)
            .unwrap()
            .recipe_fingerprint();
        assert!(!stale.is_current());
        let mut stale = original.clone();
        stale.report.resource_fingerprint[0] ^= 1;
        assert!(!stale.is_current());
        let mut stale = original.clone();
        stale.schema_revision += 1;
        assert!(!stale.is_current());
        assert!(original.is_current());
    }

    #[test]
    fn qualification_rejects_another_presets_real_artifacts() {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(ThemePreset::Spotless)
            .unwrap();
        let renderer = Renderer::new().with_engine(Engine::new().with_site_config(
            MermaidConfig::from_value(serde_json::json!({"htmlLabels": false})),
        ));
        let RenderOutput::Document(Some(document)) = renderer
            .render(
                RenderRequest::document(
                    STATE_SPEC.source,
                    OperationControl::new(),
                    Default::default(),
                )
                .with_theme(theme),
            )
            .unwrap()
        else {
            panic!("missing document")
        };
        let png = document
            .export_png(&Default::default(), OperationControl::new())
            .unwrap();
        state_proof::verify(ThemePreset::Spotless, &document, &png).unwrap();
        let error = state_proof::verify(ThemePreset::Brutalist, &document, &png).unwrap_err();
        assert!(error.to_string().contains("preset-canvas"));
    }
}

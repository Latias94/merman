//! Production admission inventory for the exact catalog recipes.
//!
//! Admission is a prerequisite for preset qualification, not a qualification receipt. These
//! observations never populate the public catalog's qualified cells or the fixed C6a ledger.

use merman::svg::{DiagramThemeCompiler, ThemePreset, ThemeRecipeFingerprint};
use merman::{
    DiagramFamilyId, Engine, MermaidConfig, OperationControl, RenderArtifactKind, RenderOutput,
    RenderRequest, Renderer, TargetAdmissionReason, TargetAdmissionReceipt, TargetAdmissionStatus,
};
use sha2::{Digest as _, Sha256};

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

const SPECS: [PresetQualificationSpec; 3] = [
    PresetQualificationSpec {
        family: DiagramFamilyId::FLOWCHART,
        source_id: "preset-native-flowchart-v1",
        source: "flowchart LR\nA[Alpha] --> B[Beta]\n",
    },
    PresetQualificationSpec {
        family: DiagramFamilyId::STATE,
        source_id: "preset-native-state-v1",
        source: "stateDiagram-v2\n[*] --> Active\nActive --> [*]\n",
    },
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
        observations: Vec::with_capacity(SPECS.len() * 2),
    };
    let renderer = Renderer::new().with_engine(Engine::new().with_site_config(
        MermaidConfig::from_value(serde_json::json!({"htmlLabels": false})),
    ));
    for spec in SPECS {
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
        if png.admission().document_digest() != svg_receipt.document_digest()
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

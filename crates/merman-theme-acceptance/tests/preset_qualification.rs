#![cfg(merman_internal_theme_acceptance)]

use merman::svg::{DiagramThemeCompiler, theme_preset_descriptors};
use merman::{DiagramFamilyId, RenderArtifactKind, TargetAdmissionReason, TargetAdmissionStatus};
use merman_theme_acceptance::inspect_preset_admission;

#[test]
fn exact_catalog_recipes_keep_native_admission_failures_visible() {
    let descriptors = theme_preset_descriptors();
    assert_eq!(descriptors.len(), 10);
    for descriptor in descriptors {
        let preset = descriptor.preset();
        let theme = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
        let report = inspect_preset_admission(preset).unwrap();
        assert_eq!(report.preset(), preset);
        assert_eq!(report.recipe_fingerprint(), theme.recipe_fingerprint());
        assert_eq!(
            report.resource_fingerprint(),
            theme.report().font_catalog_fingerprint().as_bytes()
        );
        assert_eq!(report.observations().len(), 6);
        for (pair, family) in report.observations().chunks_exact(2).zip([
            DiagramFamilyId::FLOWCHART,
            DiagramFamilyId::STATE,
            DiagramFamilyId::SEQUENCE,
        ]) {
            assert_eq!(pair[0].artifact_kind(), RenderArtifactKind::Svg);
            assert_eq!(pair[1].artifact_kind(), RenderArtifactKind::Png);
            assert_eq!(
                pair[0].target_receipt().document_digest(),
                pair[1].target_receipt().document_digest()
            );
            assert_eq!(pair[0].source_digest(), pair[1].source_digest());
            assert_ne!(pair[0].source_digest(), &[0; 32]);
            assert_ne!(
                pair[0].target_receipt().artifact_digest(),
                pair[1].target_receipt().artifact_digest()
            );
            for observation in pair {
                assert_eq!(observation.spec().family_id(), family);
                assert_eq!(
                    observation.status(),
                    TargetAdmissionStatus::Rejected,
                    "{} {} {:?}",
                    preset.id(),
                    family,
                    observation.artifact_kind()
                );
                assert!(
                    observation
                        .reasons()
                        .contains(&TargetAdmissionReason::ThemeEvidenceIncomplete)
                );
                assert_eq!(
                    observation.mermaid_residual_count(),
                    2,
                    "the explicit base/dark-mode compatibility requests remain residuals"
                );
                assert_eq!(
                    observation.bridge_residual_count(),
                    if family == DiagramFamilyId::FLOWCHART {
                        3
                    } else {
                        0
                    }
                );
                assert_eq!(
                    observation.theme_residual_count(),
                    usize::from(family == DiagramFamilyId::SEQUENCE),
                    "Sequence's unsupported generic Text.fill request remains visible",
                );
                assert_eq!(observation.source_residual_count(), 0);
            }
            assert!(
                pair[0]
                    .reasons()
                    .contains(&TargetAdmissionReason::SvgFontsNotSelfContained)
            );
        }
        assert!(
            descriptor.qualified_cells().is_empty(),
            "admission observations must not grant preset qualification"
        );
    }
}

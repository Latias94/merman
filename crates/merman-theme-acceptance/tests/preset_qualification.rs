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
        let native_candidate = matches!(preset.id(), "brutalist" | "spotless" | "cyberpunk");
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
                let family_complete = native_candidate;
                assert_eq!(
                    observation.status(),
                    if family_complete {
                        TargetAdmissionStatus::HostDependent
                    } else {
                        TargetAdmissionStatus::Rejected
                    },
                    "{} {} {:?}",
                    preset.id(),
                    family,
                    observation.artifact_kind()
                );
                assert_eq!(
                    observation
                        .reasons()
                        .contains(&TargetAdmissionReason::ThemeEvidenceIncomplete),
                    !family_complete,
                );
                assert_eq!(
                    observation.mermaid_residual_count(),
                    if native_candidate { 0 } else { 2 },
                    "only retained recipes request base/dark-mode compatibility"
                );
                assert_eq!(observation.bridge_residual_count(), 0);
                assert_eq!(
                    observation.theme_residual_count(),
                    0,
                    "Sequence role fills cover the generic Text.fill fallback in this source",
                );
                assert_eq!(observation.source_residual_count(), 0);
            }
            assert!(
                pair[0]
                    .reasons()
                    .contains(&TargetAdmissionReason::SvgFontsNotSelfContained),
            );
        }
        assert!(
            descriptor.qualified_cells().is_empty(),
            "admission observations must not grant preset qualification"
        );
    }
}

#[test]
fn native_presets_qualify_only_the_declared_host_dependent_profile() {
    use merman::svg::ThemePreset;
    use merman_theme_acceptance::run_preset_qualification;

    for preset in [
        ThemePreset::Brutalist,
        ThemePreset::Spotless,
        ThemePreset::Cyberpunk,
    ] {
        let receipt = run_preset_qualification(preset).unwrap();
        assert!(receipt.is_current());
        assert_eq!(
            receipt.profile_id(),
            "native-state-sequence-system-fonts-v2"
        );
        assert_eq!(receipt.schema_revision(), 2);
        assert_eq!(receipt.report().preset(), preset);
        assert_eq!(receipt.report().observations().len(), 4);
        for (observation, family) in receipt.report().observations().iter().zip([
            DiagramFamilyId::STATE,
            DiagramFamilyId::STATE,
            DiagramFamilyId::SEQUENCE,
            DiagramFamilyId::SEQUENCE,
        ]) {
            assert_eq!(observation.spec().family_id(), family);
            assert_eq!(
                observation.spec().png_scale(),
                if family == DiagramFamilyId::SEQUENCE {
                    4.0
                } else {
                    1.0
                }
            );
            assert_eq!(observation.status(), TargetAdmissionStatus::HostDependent);
            assert_eq!(observation.theme_residual_count(), 0);
            assert_eq!(observation.bridge_residual_count(), 0);
            assert_eq!(observation.mermaid_residual_count(), 0);
        }
    }
    for descriptor in theme_preset_descriptors() {
        assert!(descriptor.qualified_cells().is_empty());
        if !matches!(
            descriptor.preset(),
            ThemePreset::Brutalist | ThemePreset::Spotless | ThemePreset::Cyberpunk
        ) {
            assert!(run_preset_qualification(descriptor.preset()).is_err());
        }
    }
}

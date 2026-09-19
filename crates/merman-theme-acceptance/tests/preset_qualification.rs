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
        let families = if preset == merman::svg::ThemePreset::Cyberpunk {
            [
                DiagramFamilyId::FLOWCHART,
                DiagramFamilyId::SEQUENCE,
                DiagramFamilyId::XY_CHART,
            ]
        } else {
            [
                DiagramFamilyId::FLOWCHART,
                DiagramFamilyId::STATE,
                DiagramFamilyId::SEQUENCE,
            ]
        };
        for (pair, family) in report.observations().chunks_exact(2).zip(families) {
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
                // These bounded native scenes consume the current candidate recipes.
                // Host-dependent admission does not promote public qualification cells.
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
                // The retained Mermaid-compatibility recipes still carry the shared Text paint
                // into Sequence, whose writer does not consume that generic target. The native
                // candidate recipes have family-local role paints and therefore do not retain it.
                let expected_theme_residual =
                    usize::from(!native_candidate && family == DiagramFamilyId::SEQUENCE);
                assert_eq!(
                    observation.theme_residual_count(),
                    expected_theme_residual,
                    "theme residual classification must match the preset profile and family",
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
        if preset == merman::svg::ThemePreset::Cyberpunk {
            for (pair, (source_id, source)) in report.observations().chunks_exact(2).zip([
                (
                    "public-cyberpunk-flowchart-v1",
                    include_str!(
                        "../../merman-theme-fixtures/fixtures/public-cyberpunk/flowchart.mmd"
                    ),
                ),
                (
                    "public-cyberpunk-sequence-v1",
                    include_str!(
                        "../../merman-theme-fixtures/fixtures/public-cyberpunk/sequence.mmd"
                    ),
                ),
                (
                    "public-cyberpunk-xychart-v1",
                    include_str!(
                        "../../merman-theme-fixtures/fixtures/public-cyberpunk/xychart.mmd"
                    ),
                ),
            ]) {
                for observation in pair {
                    assert_eq!(observation.spec().source_id(), source_id);
                    assert_eq!(observation.spec().source(), source);
                    assert_eq!(observation.spec().png_scale(), 1.0);
                }
            }
        }
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
        let (profile, families) = if preset == ThemePreset::Cyberpunk {
            (
                "native-cyberpunk-full-scenes-system-fonts-v1",
                [
                    DiagramFamilyId::FLOWCHART,
                    DiagramFamilyId::SEQUENCE,
                    DiagramFamilyId::XY_CHART,
                ],
            )
        } else {
            (
                "native-flowchart-state-sequence-system-fonts-v1",
                [
                    DiagramFamilyId::FLOWCHART,
                    DiagramFamilyId::STATE,
                    DiagramFamilyId::SEQUENCE,
                ],
            )
        };
        assert_eq!(receipt.profile_id(), profile);
        assert_eq!(receipt.schema_revision(), 1);
        assert_eq!(receipt.report().preset(), preset);
        assert_eq!(receipt.report().observations().len(), 6);
        for (observation, family) in receipt.report().observations().iter().zip([
            families[0],
            families[0],
            families[1],
            families[1],
            families[2],
            families[2],
        ]) {
            assert_eq!(observation.spec().family_id(), family);
            assert_eq!(
                observation.spec().png_scale(),
                if preset != ThemePreset::Cyberpunk
                    && matches!(
                        family,
                        DiagramFamilyId::FLOWCHART | DiagramFamilyId::SEQUENCE
                    )
                {
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
        let entry = receipt.catalog_entry().unwrap();
        assert_eq!(entry.qualified_cells.len(), 6);
        for cell in &entry.qualified_cells {
            assert_eq!(cell.profile_id, profile);
            assert_eq!(cell.admission_status, "host_dependent");
            assert!(
                families
                    .iter()
                    .any(|family| family.as_str() == cell.family_id)
            );
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

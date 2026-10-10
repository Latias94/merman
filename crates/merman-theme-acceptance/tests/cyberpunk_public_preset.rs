#![cfg(merman_internal_theme_acceptance)]

use merman::svg::{DiagramThemeCompiler, ThemePreset};
use merman::{
    Engine, OperationControl, RenderOutput, RenderRequest, Renderer, TargetAdmissionReason,
    TargetAdmissionStatus,
};

fn assert_recipe_round_trip(source: &str, effects: u32) {
    let compiler = DiagramThemeCompiler::new();
    let saved =
        serde_json::to_vec(&compiler.export_preset(ThemePreset::Cyberpunk).unwrap()).unwrap();
    let original = compiler.compile_preset(ThemePreset::Cyberpunk).unwrap();
    let imported = DiagramThemeCompiler::new()
        .compile_recipe(serde_json::from_slice(&saved).unwrap())
        .unwrap();
    assert_eq!(original.recipe_fingerprint(), imported.recipe_fingerprint());
    let renderer = Renderer::new().with_engine(
        Engine::new().with_site_config(merman_theme_acceptance::preset_qualification_config()),
    );
    let mut outputs = Vec::new();
    for theme in [original, imported] {
        let RenderOutput::Document(Some(document)) = renderer
            .render(
                RenderRequest::document(source, OperationControl::new(), Default::default())
                    .with_theme(theme),
            )
            .unwrap()
        else {
            panic!("complete public scene must produce a document")
        };
        let png = document
            .export_png(&Default::default(), OperationControl::new())
            .unwrap();
        assert_eq!(
            png.admission().status(),
            TargetAdmissionStatus::HostDependent,
            "admission={:?}; fonts={:?}; filters={:?}; source={source}",
            png.admission(),
            png.export_report().fonts(),
            png.export_report().native_filter_receipt(),
        );
        assert_eq!(
            png.admission().reasons(),
            &[TargetAdmissionReason::SystemOrHostFontDependency],
            "admission={:?}; source={source}",
            png.admission(),
        );
        assert_eq!(
            png.export_report()
                .native_filter_receipt()
                .unwrap()
                .reference_count(),
            effects
        );
        outputs.push((document.svg().to_owned(), png.bytes().to_vec()));
    }
    assert_eq!(outputs[0].0, outputs[1].0);
    assert!(
        outputs[0].1 == outputs[1].1,
        "recipe exchange must preserve PNG bytes"
    );
}

#[test]
fn complete_flowchart_public_recipe_round_trip_preserves_artifacts() {
    assert_recipe_round_trip(
        include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/flowchart.mmd"),
        13,
    );
}

#[test]
fn complete_sequence_public_recipe_round_trip_preserves_artifacts() {
    assert_recipe_round_trip(
        include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/sequence.mmd"),
        21,
    );
}

#[test]
fn complete_xychart_public_recipe_round_trip_preserves_artifacts() {
    assert_recipe_round_trip(
        include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/xychart.mmd"),
        28,
    );
}

//! The public SVG facade keeps font references separate from embedded font processing.

#![cfg(feature = "svg")]

use merman::diagram_theme::{DiagramThemeCompiler, ThemePreset, ThemeRecipeV1};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer};

#[test]
fn resource_free_cyberpunk_renders_through_the_public_facade() {
    let compiler = DiagramThemeCompiler::new();
    let theme = compiler.compile_preset(ThemePreset::Cyberpunk).unwrap();
    let RenderOutput::Svg(Some(output)) = Renderer::new()
        .render(
            RenderRequest::svg(
                "flowchart LR\nA[Alpha] --> B[Beta]",
                OperationControl::new(),
                Default::default(),
            )
            .with_theme(theme),
        )
        .unwrap()
    else {
        panic!("expected SVG output")
    };
    let svg = output.svg();
    assert!(svg.contains("<pattern"), "public canvas must render");
    assert!(svg.contains("<filter"), "public glow must render");
    assert!(!svg.contains("@font-face"), "no font bytes may be bundled");
}

#[test]
fn complete_spec_font_data_can_be_read_and_forwarded_without_decoding() {
    // Exchange preserves the payload; compilation owns font admission and decoding.
    let input = serde_json::json!({
        "schema_version": 1,
        "kind": "complete_spec",
        "complete_spec": {
            "assets": {
                "fonts": [{"id": "caller-font", "format": "woff2", "data_base64": "AAECAw=="}]
            }
        }
    });
    let recipe: ThemeRecipeV1 = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(serde_json::to_value(recipe).unwrap(), input);
}

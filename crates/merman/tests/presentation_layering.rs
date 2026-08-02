#![cfg(feature = "svg")]

use merman::Engine;
use merman::svg::{
    HeadlessRenderer, HostTheme, HostThemeProfile, Presentation, PresentationProfile, SvgPipeline,
    SvgPipelinePreset, ThemePreset,
};
use merman_core::MermaidConfig;
use serde_json::{Value, json};

const SOURCE: &str = "sequenceDiagram\nAlice->>Bob: Hello";

fn config(value: Value) -> MermaidConfig {
    MermaidConfig::from_value(value)
}

fn effective_config(renderer: &HeadlessRenderer, source: &str) -> Value {
    renderer
        .parse_metadata_sync(source)
        .expect("metadata parse should succeed")
        .effective_config
        .as_value()
        .clone()
}

fn presentation() -> Presentation {
    Presentation::new()
        .with_profile(PresentationProfile::MermanModern)
        .with_theme(HostTheme::from_preset(ThemePreset::OneDark))
}

#[test]
fn renderer_configuration_precedence_is_independent_of_builder_order() {
    let base = Engine::new().with_site_config(config(json!({
        "theme": "forest",
        "look": "classic",
        "flowchart": { "defaultRenderer": "dagre" },
    })));
    let explicit = config(json!({
        "theme": "dark",
        "look": "handDrawn",
        "themeVariables": { "lineColor": "#123456" },
    }));

    let forward = HeadlessRenderer::new()
        .with_engine(base.clone())
        .with_presentation(presentation())
        .with_site_config(explicit.clone());
    let reverse = HeadlessRenderer::new()
        .with_site_config(explicit)
        .with_presentation(presentation())
        .with_engine(base);

    let forward = effective_config(&forward, SOURCE);
    let reverse = effective_config(&reverse, SOURCE);
    assert_eq!(forward, reverse);
    assert_eq!(forward["theme"], "dark");
    assert_eq!(forward["look"], "handDrawn");
    assert_eq!(forward["themeVariables"]["lineColor"], "#123456");
    assert_eq!(forward["flowchart"]["defaultRenderer"], "elk");
}

#[test]
fn source_config_remains_the_final_nonsecure_mermaid_layer() {
    let source = r##"%%{init: {"theme": "neutral", "sequence": {"actorMargin": 88}}}%%
sequenceDiagram
Alice->>Bob: Hello"##;
    let renderer = HeadlessRenderer::new()
        .with_presentation(presentation())
        .with_site_config(config(json!({
            "theme": "dark",
            "sequence": { "actorMargin": 64 },
            "themeVariables": { "lineColor": "#123456" },
        })));

    let effective = effective_config(&renderer, source);
    assert_eq!(effective["theme"], "neutral");
    assert_eq!(effective["sequence"]["actorMargin"], 88);
    assert_eq!(effective["look"], "neo");
}

#[test]
fn source_config_cannot_override_secure_theme_variables() {
    let source = r##"%%{init: {"themeVariables": {"lineColor": "#abcdef"}}}%%
sequenceDiagram
Alice->>Bob: Hello"##;
    let renderer = HeadlessRenderer::new().with_site_config(config(json!({
        "themeVariables": { "lineColor": "#123456" },
    })));

    let effective = effective_config(&renderer, source);
    assert_eq!(effective["themeVariables"]["lineColor"], "#123456");
}

#[test]
fn repeated_site_config_layers_preserve_engine_normalization_order() {
    let renderer = HeadlessRenderer::new()
        .with_site_config(config(json!({ "fontFamily": "First Font" })))
        .with_site_config(config(json!({
            "themeVariables": { "fontFamily": "Second Font" },
        })));

    let effective = effective_config(&renderer, SOURCE);
    assert_eq!(effective["fontFamily"], "First Font");
    assert_eq!(effective["themeVariables"]["fontFamily"], "Second Font");
}

#[test]
fn presentation_does_not_select_or_override_svg_output_policy() {
    let no_output = HeadlessRenderer::new().with_presentation(presentation());
    assert!(no_output.svg_pipeline().is_none());

    let forward = HeadlessRenderer::new()
        .with_presentation(presentation())
        .with_svg_pipeline(SvgPipeline::readable());
    let reverse = HeadlessRenderer::new()
        .with_svg_pipeline(SvgPipeline::readable())
        .with_presentation(presentation());

    assert_eq!(
        forward.svg_pipeline().map(SvgPipeline::preset),
        Some(SvgPipelinePreset::Readable)
    );
    assert_eq!(
        reverse.svg_pipeline().map(SvgPipeline::preset),
        Some(SvgPipelinePreset::Readable)
    );

    let migrated = HeadlessRenderer::new()
        .with_host_theme(&HostThemeProfile::editor_dark())
        .with_presentation(presentation());
    assert!(migrated.svg_pipeline().is_none());
}

#[test]
fn direct_parse_and_prepared_semantic_share_the_same_materialized_engine() {
    let renderer = HeadlessRenderer::new()
        .with_presentation(presentation())
        .with_site_config(config(json!({ "theme": "dark" })));

    let direct = effective_config(&renderer, SOURCE);
    let exposed = renderer
        .engine()
        .parse_metadata_sync(SOURCE)
        .expect("materialized engine metadata should succeed")
        .effective_config
        .as_value()
        .clone();
    let prepared = renderer
        .prepare_semantic_sync(SOURCE)
        .expect("prepared semantic should succeed")
        .expect("sequence diagram should be detected")
        .metadata()
        .effective_config
        .as_value()
        .clone();

    assert_eq!(exposed, direct);
    assert_eq!(prepared, direct);
}

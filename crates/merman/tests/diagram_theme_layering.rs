#![cfg(feature = "svg")]

use merman::Engine;
use merman::svg::{
    DiagramTheme, DiagramThemeCompiler, HeadlessRenderer, SvgPipeline, SvgPipelinePreset,
    ThemePreset,
};
use merman_core::MermaidConfig;
use serde_json::{Value, json};

const SOURCE: &str = "sequenceDiagram\nAlice->>Bob: Hello";

fn config(value: Value) -> MermaidConfig {
    MermaidConfig::from_value(value)
}

fn one_dark() -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile_preset(ThemePreset::OneDark)
        .expect("one-dark preset should compile")
}

fn effective_config(renderer: &HeadlessRenderer, source: &str) -> Value {
    renderer
        .parse_metadata_sync(source)
        .expect("metadata parse should succeed")
        .effective_config
        .as_value()
        .clone()
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
        .with_theme(one_dark())
        .with_site_config(explicit.clone());
    let reverse = HeadlessRenderer::new()
        .with_site_config(explicit)
        .with_theme(one_dark())
        .with_engine(base);

    let forward = effective_config(&forward, SOURCE);
    let reverse = effective_config(&reverse, SOURCE);
    assert_eq!(forward, reverse);
    assert_eq!(forward["theme"], "dark");
    assert_eq!(forward["look"], "handDrawn");
    assert_eq!(forward["themeVariables"]["lineColor"], "#123456");
    assert_eq!(forward["flowchart"]["defaultRenderer"], "dagre");
}

#[test]
fn explicit_mermaid_values_override_the_compiled_theme() {
    let renderer = HeadlessRenderer::new()
        .with_theme(one_dark())
        .with_site_config(config(json!({
            "theme": "base",
            "themeVariables": {
                "primaryColor": "#abcdef",
                "lineColor": "#123456"
            },
        })));

    let effective = effective_config(&renderer, SOURCE);
    assert_eq!(effective["theme"], "base");
    assert_eq!(effective["themeVariables"]["primaryColor"], "#abcdef");
    assert_eq!(effective["themeVariables"]["lineColor"], "#123456");
}

#[test]
fn visual_theme_does_not_select_mermaid_behavior() {
    let source = "flowchart TD\nA --> B";
    let baseline = effective_config(&HeadlessRenderer::new(), source);
    let themed = effective_config(&HeadlessRenderer::new().with_theme(one_dark()), source);

    assert_eq!(themed.get("look"), baseline.get("look"));
    assert_eq!(
        themed["flowchart"]["defaultRenderer"],
        baseline["flowchart"]["defaultRenderer"]
    );
}

#[test]
fn source_config_remains_the_final_nonsecure_mermaid_layer() {
    let source = r##"%%{init: {"theme": "neutral", "sequence": {"actorMargin": 88}}}%%
sequenceDiagram
Alice->>Bob: Hello"##;
    let renderer = HeadlessRenderer::new()
        .with_theme(one_dark())
        .with_site_config(config(json!({
            "theme": "dark",
            "sequence": { "actorMargin": 64 },
        })));

    let effective = effective_config(&renderer, source);
    assert_eq!(effective["theme"], "neutral");
    assert_eq!(effective["sequence"]["actorMargin"], 88);
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
fn theme_does_not_select_or_override_svg_output_policy() {
    let no_output = HeadlessRenderer::new().with_theme(one_dark());
    assert!(no_output.svg_pipeline().is_none());

    let forward = HeadlessRenderer::new()
        .with_theme(one_dark())
        .with_svg_pipeline(SvgPipeline::readable());
    let reverse = HeadlessRenderer::new()
        .with_svg_pipeline(SvgPipeline::readable())
        .with_theme(one_dark());

    assert_eq!(
        forward.svg_pipeline().map(SvgPipeline::preset),
        Some(SvgPipelinePreset::Readable)
    );
    assert_eq!(
        reverse.svg_pipeline().map(SvgPipeline::preset),
        Some(SvgPipelinePreset::Readable)
    );
}

#[test]
fn direct_parse_and_prepared_semantic_share_the_same_materialized_engine() {
    let renderer = HeadlessRenderer::new()
        .with_theme(one_dark())
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

#[test]
fn render_plan_reports_renderer_capabilities_only() {
    let plain = HeadlessRenderer::new()
        .plan_svg_sync(SOURCE)
        .expect("plain plan should succeed")
        .expect("sequence diagram should be detected");
    let themed = HeadlessRenderer::new()
        .with_theme(one_dark())
        .plan_svg_sync(SOURCE)
        .expect("themed plan should succeed")
        .expect("sequence diagram should be detected");

    assert_eq!(
        plain.required_capabilities(),
        themed.required_capabilities()
    );
    assert_eq!(plain.missing_capabilities(), themed.missing_capabilities());
}

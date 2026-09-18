#![cfg(feature = "svg")]

use merman::svg::{DiagramTheme, DiagramThemeCompiler, SvgPipeline, ThemePreset};
use merman::{
    Engine, MermaidConfig, OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest,
};
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

fn effective_config(
    renderer: &Renderer,
    source: &str,
    request: SvgRequest,
    theme: Option<DiagramTheme>,
) -> Value {
    let request = RenderRequest::layout_json(source, OperationControl::new(), request);
    let request = match theme {
        Some(theme) => request.with_theme(theme),
        None => request,
    };
    let output = renderer
        .render(request)
        .expect("layout projection should succeed");
    let RenderOutput::LayoutJson(Some(output)) = output else {
        panic!("expected layout JSON for a detected diagram");
    };
    output.layout()["meta"]["effective_config"].clone()
}

fn capability_plan(
    renderer: &Renderer,
    source: &str,
    request: SvgRequest,
    theme: Option<DiagramTheme>,
) -> merman::svg::RenderCapabilityPlan {
    let request = RenderRequest::svg_plan(source, OperationControl::new(), request);
    let request = match theme {
        Some(theme) => request.with_theme(theme),
        None => request,
    };
    let output = renderer
        .render(request)
        .expect("SVG capability planning should succeed");
    let RenderOutput::SvgPlan(Some(plan)) = output else {
        panic!("expected an SVG capability plan for a detected diagram");
    };
    plan
}

#[test]
fn site_config_overrides_theme_mermaid_compatibility() {
    let renderer = Renderer::new().with_engine(Engine::new().with_site_config(config(json!({
        "theme": "forest",
        "darkMode": false,
        "look": "handDrawn",
        "themeVariables": {
            "darkMode": false,
            "lineColor": "#123456"
        },
        "flowchart": { "defaultRenderer": "dagre" },
    }))));

    let effective = effective_config(&renderer, SOURCE, SvgRequest::default(), Some(one_dark()));
    assert_eq!(effective["theme"], "forest");
    assert_eq!(effective["darkMode"], false);
    assert_eq!(effective["look"], "handDrawn");
    assert_eq!(effective["themeVariables"]["darkMode"], false);
    assert_eq!(effective["themeVariables"]["lineColor"], "#123456");
    assert_eq!(effective["flowchart"]["defaultRenderer"], "dagre");
}

#[test]
fn typed_theme_does_not_select_mermaid_layout_or_look() {
    let source = "flowchart TD\nA --> B";
    let renderer = Renderer::new();
    let baseline = effective_config(&renderer, source, SvgRequest::default(), None);
    let themed = effective_config(&renderer, source, SvgRequest::default(), Some(one_dark()));

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
    let renderer = Renderer::new().with_engine(Engine::new().with_site_config(config(json!({
        "theme": "dark",
        "sequence": { "actorMargin": 64 },
    }))));

    let effective = effective_config(&renderer, source, SvgRequest::default(), Some(one_dark()));
    assert_eq!(effective["theme"], "neutral");
    assert_eq!(effective["sequence"]["actorMargin"], 88);
}

#[test]
fn source_config_cannot_override_secure_site_values() {
    let source = r##"%%{init: {"themeVariables": {"lineColor": "#abcdef"}}}%%
sequenceDiagram
Alice->>Bob: Hello"##;
    let renderer = Renderer::new().with_engine(Engine::new().with_site_config(config(json!({
        "secure": ["secure", "securityLevel", "themeVariables"],
        "themeVariables": { "lineColor": "#123456" },
    }))));

    let effective = effective_config(&renderer, source, SvgRequest::default(), Some(one_dark()));
    assert_eq!(effective["themeVariables"]["lineColor"], "#123456");
}

#[test]
fn svg_pipeline_selection_does_not_change_theme_identity() {
    let theme = one_dark();
    let renderer = Renderer::new();
    let plain = renderer
        .render(
            RenderRequest::svg(SOURCE, OperationControl::new(), SvgRequest::default())
                .with_theme(theme.clone()),
        )
        .expect("themed SVG should render");
    let RenderOutput::Svg(Some(plain)) = plain else {
        panic!("expected a themed SVG");
    };

    let mut readable_request = SvgRequest::default();
    readable_request.pipeline = Some(SvgPipeline::readable());
    let readable = renderer
        .render(
            RenderRequest::svg(SOURCE, OperationControl::new(), readable_request)
                .with_theme(theme.clone()),
        )
        .expect("themed readable SVG should render");
    let RenderOutput::Svg(Some(readable)) = readable else {
        panic!("expected a themed readable SVG");
    };

    assert_eq!(
        plain.evidence().theme_recipe_fingerprint(),
        Some(theme.recipe_fingerprint())
    );
    assert_eq!(
        readable.evidence().theme_recipe_fingerprint(),
        Some(theme.recipe_fingerprint())
    );
}

#[test]
fn render_plan_reports_renderer_capabilities_only() {
    const PLAN_SOURCE: &str = "mindmap\n  root((Root))\n    Child";
    let renderer = Renderer::new();
    let plain = capability_plan(&renderer, PLAN_SOURCE, SvgRequest::default(), None);
    let themed = capability_plan(
        &renderer,
        PLAN_SOURCE,
        SvgRequest::default(),
        Some(one_dark()),
    );

    assert!(!plain.required_capabilities().is_empty());
    assert_eq!(
        plain.required_capabilities(),
        themed.required_capabilities()
    );
    assert_eq!(plain.missing_capabilities(), themed.missing_capabilities());
}

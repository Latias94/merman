#![cfg(all(
    feature = "svg",
    feature = "compat-json",
    any(
        feature = "diagram-flowchart",
        feature = "diagram-sequence",
        feature = "diagram-mindmap",
        feature = "diagram-packet"
    )
))]

#[cfg(feature = "diagram-sequence")]
use merman::svg::SvgPipeline;
#[cfg(feature = "diagram-sequence")]
use merman::theme::ThemeMode;
use merman::theme::{Appearance, BuiltinPreset, ThemeSelection};
#[cfg(any(feature = "diagram-sequence", feature = "diagram-packet"))]
use merman::{MermaidEngine, SourcePresentationConfig};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};
#[cfg(any(feature = "diagram-sequence", feature = "diagram-packet"))]
use serde_json::{Value, json};

#[cfg(feature = "diagram-sequence")]
const SOURCE: &str = "sequenceDiagram\nAlice->>Bob: Hello";

#[cfg(any(feature = "diagram-sequence", feature = "diagram-packet"))]
fn config(value: Value) -> SourcePresentationConfig {
    SourcePresentationConfig::from_value(value)
}

fn studio_dark() -> ThemeSelection {
    ThemeSelection::preset(BuiltinPreset::Studio, Some(Appearance::Dark))
}

#[cfg(feature = "diagram-packet")]
#[test]
fn native_packet_paint_ignores_site_theme_variables_until_source_selects_mermaid() {
    use merman::theme::ThemePrecedence;

    let renderer = Renderer::new().with_presentation_engine(MermaidEngine::new().with_site_config(
        config(json!({"themeVariables": {"packet": {"blockFillColor": "#FF00FF"}}})),
    ));
    let plain = "packet\n0-7: \"Header\"";
    let with_theme = format!("---\nconfig:\n  theme: base\n---\n{plain}");
    let render = |source, request| {
        renderer
            .render_svg_data(RenderRequest::svg(source, OperationControl::new(), request))
            .unwrap()
            .unwrap()
    };
    let host = SvgRequest::default().with_native_theme(studio_dark());
    let source = SvgRequest::default()
        .with_native_theme(studio_dark().with_precedence(ThemePrecedence::Source));
    assert!(!render(&with_theme, host).contains("#FF00FF"));
    assert!(!render(plain, source.clone()).contains("#FF00FF"));

    let classic = render(&with_theme, SvgRequest::default());
    assert!(classic.contains("#FF00FF"));
    assert_eq!(render(&with_theme, source), classic);
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-sequence"))]
fn effective_config(
    renderer: &Renderer,
    source: &str,
    request: SvgRequest,
    theme: Option<ThemeSelection>,
) -> merman::model::ManagedJson {
    let request = RenderRequest::layout_snapshot(source, OperationControl::new(), request);
    let request = match theme {
        Some(theme) => request.with_native_theme(theme),
        None => request,
    };
    let output = renderer
        .render(request)
        .expect("layout projection should succeed");
    let RenderOutput::LayoutJson(Some(output)) = output else {
        panic!("expected layout JSON for a detected diagram");
    };
    merman::model::ManagedJson::from(&output.layout()["meta"]["effective_config"])
}

#[cfg(feature = "diagram-mindmap")]
fn capability_plan(
    renderer: &Renderer,
    source: &str,
    request: SvgRequest,
    theme: Option<ThemeSelection>,
) -> merman::svg::RenderCapabilityPlan {
    let request = RenderRequest::svg_plan(source, OperationControl::new(), request);
    let request = match theme {
        Some(theme) => request.with_native_theme(theme),
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

#[cfg(feature = "diagram-sequence")]
#[test]
fn native_theme_preserves_site_mermaid_config() {
    let renderer = Renderer::new().with_presentation_engine(MermaidEngine::new().with_site_config(
        config(json!({
            "theme": "forest",
            "darkMode": false,
            "look": "handDrawn",
            "themeVariables": {
                "darkMode": false,
                "lineColor": "#123456"
            },
            "flowchart": { "defaultRenderer": "dagre" },
        })),
    ));

    let effective = effective_config(
        &renderer,
        SOURCE,
        SvgRequest::default(),
        Some(studio_dark()),
    );
    assert_eq!(effective["theme"], "forest");
    assert_eq!(effective["darkMode"], false);
    assert_eq!(effective["look"], "handDrawn");
    assert_eq!(effective["themeVariables"]["darkMode"], false);
    assert_eq!(effective["themeVariables"]["lineColor"], "#123456");
    assert_eq!(effective["flowchart"]["defaultRenderer"], "dagre");
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn native_theme_does_not_select_mermaid_layout_or_look() {
    let source = "flowchart TD\nA --> B";
    let renderer = Renderer::new();
    let baseline = effective_config(&renderer, source, SvgRequest::default(), None);
    let themed = effective_config(
        &renderer,
        source,
        SvgRequest::default(),
        Some(studio_dark()),
    );

    assert_eq!(themed.get("look"), baseline.get("look"));
    assert_eq!(
        themed["flowchart"]["defaultRenderer"],
        baseline["flowchart"]["defaultRenderer"]
    );
}

#[cfg(feature = "diagram-sequence")]
#[test]
fn source_config_remains_the_final_nonsecure_mermaid_layer() {
    let source = r##"%%{init: {"theme": "neutral", "sequence": {"actorMargin": 88}}}%%
sequenceDiagram
Alice->>Bob: Hello"##;
    let renderer = Renderer::new().with_presentation_engine(MermaidEngine::new().with_site_config(
        config(json!({
            "theme": "dark",
            "sequence": { "actorMargin": 64 },
        })),
    ));

    let effective = effective_config(
        &renderer,
        source,
        SvgRequest::default(),
        Some(studio_dark()),
    );
    assert_eq!(effective["theme"], "neutral");
    assert_eq!(effective["sequence"]["actorMargin"], 88);
}

#[cfg(feature = "diagram-sequence")]
#[test]
fn layout_projection_materializes_scoped_theme_with_source_variables() {
    let source = r##"%%{init: {"themeVariables": {"primaryColor": "#123456"}}}%%
sequenceDiagram
Alice->>Bob: Hello"##;
    let renderer = Renderer::new().with_presentation_engine(MermaidEngine::new().with_site_config(
        config(json!({
            "theme": "default",
            "sequence": {"theme": "base"}
        })),
    ));

    let effective = effective_config(&renderer, source, SvgRequest::default(), None);
    assert_eq!(effective["theme"], "base");
    assert_eq!(effective["themeVariables"]["primaryColor"], "#123456");
    assert_eq!(effective["themeVariables"]["actorBkg"], "#123456");
}

#[cfg(feature = "diagram-sequence")]
#[test]
fn source_config_cannot_override_secure_site_values() {
    let source = r##"%%{init: {"themeVariables": {"lineColor": "#abcdef"}}}%%
sequenceDiagram
Alice->>Bob: Hello"##;
    let renderer = Renderer::new().with_presentation_engine(MermaidEngine::new().with_site_config(
        config(json!({
            "secure": ["secure", "securityLevel", "themeVariables"],
            "themeVariables": { "lineColor": "#123456" },
        })),
    ));

    let effective = effective_config(
        &renderer,
        source,
        SvgRequest::default(),
        Some(studio_dark()),
    );
    assert_eq!(effective["themeVariables"]["lineColor"], "#123456");
}

#[cfg(feature = "diagram-sequence")]
#[test]
fn request_theme_does_not_leak_after_success_or_cancellation() {
    let renderer = Renderer::new();
    let request = |control| RenderRequest::svg(SOURCE, control, SvgRequest::default());
    let baseline = renderer
        .render_svg_data(request(OperationControl::new()))
        .unwrap()
        .unwrap();
    let themed = renderer
        .render_svg_data(request(OperationControl::new()).with_native_theme(studio_dark()))
        .unwrap()
        .unwrap();
    assert_ne!(themed, baseline, "the request theme must take effect");
    assert_eq!(
        renderer
            .render_svg_data(request(OperationControl::new()))
            .unwrap()
            .unwrap(),
        baseline
    );

    let control = OperationControl::new();
    control.cancel();
    let cancelled = renderer.render(request(control).with_native_theme(studio_dark()));
    assert!(matches!(cancelled, Err(merman::RenderError::Cancelled(_))));
    assert_eq!(
        renderer
            .render_svg_data(request(OperationControl::new()))
            .unwrap()
            .unwrap(),
        baseline
    );
}

#[cfg(feature = "diagram-sequence")]
#[test]
fn svg_pipeline_selection_preserves_the_native_theme_outcome() {
    let renderer = Renderer::new();
    let render = |pipeline| {
        let request = SvgRequest {
            pipeline,
            ..SvgRequest::default()
        }
        .with_native_theme(studio_dark());
        let RenderOutput::Svg(Some(output)) = renderer
            .render(RenderRequest::svg(SOURCE, OperationControl::new(), request))
            .unwrap()
        else {
            panic!("expected a themed SVG");
        };
        output
    };
    let parity = render(None);
    let readable = render(Some(SvgPipeline::readable()));
    assert_eq!(parity.theme_outcome().mode, ThemeMode::MermanNative);
    assert_eq!(
        parity.theme_outcome().preset_id,
        Some(BuiltinPreset::Studio)
    );
    assert_eq!(parity.theme_outcome().appearance, Some(Appearance::Dark));
    assert_eq!(readable.theme_outcome(), parity.theme_outcome());
}

#[cfg(feature = "diagram-mindmap")]
#[test]
fn render_plan_reports_renderer_capabilities_only() {
    const PLAN_SOURCE: &str = "mindmap\n  root((Root))\n    Child";
    let renderer = Renderer::new();
    let plain = capability_plan(&renderer, PLAN_SOURCE, SvgRequest::default(), None);
    let themed = capability_plan(
        &renderer,
        PLAN_SOURCE,
        SvgRequest::default(),
        Some(studio_dark()),
    );

    assert_eq!(
        plain.required_capabilities().is_empty(),
        !cfg!(any(feature = "layout-cytoscape", feature = "layout-elk"))
    );
    assert_eq!(
        plain.required_capabilities(),
        themed.required_capabilities()
    );
    assert_eq!(plain.missing_capabilities(), themed.missing_capabilities());
}

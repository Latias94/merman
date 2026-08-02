#![cfg(feature = "svg")]

use merman::Engine;
use merman::svg::{
    HeadlessRenderer, HostTheme, HostThemePreset, Presentation, PresentationAspectState,
    PresentationProfile, SvgPipeline, SvgPipelinePreset,
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
        .with_theme(HostTheme::from_preset(HostThemePreset::OneDark))
}

fn assert_json_keys_absent(value: &Value, forbidden: &[&str]) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                assert!(
                    !forbidden.contains(&key.as_str()),
                    "unexpected private presentation key `{key}` in {object:?}"
                );
                assert_json_keys_absent(value, forbidden);
            }
        }
        Value::Array(values) => {
            for value in values {
                assert_json_keys_absent(value, forbidden);
            }
        }
        _ => {}
    }
}

fn aspect_state(plan: &merman::svg::RenderCapabilityPlan, id: &str) -> PresentationAspectState {
    plan.presentation_aspects()
        .iter()
        .copied()
        .find(|aspect| aspect.id() == id)
        .unwrap_or_else(|| panic!("missing presentation aspect `{id}`"))
        .state()
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

#[test]
fn modern_flowchart_policy_is_typed_and_survives_an_explicit_non_elk_renderer() {
    let renderer = HeadlessRenderer::new()
        .with_presentation(presentation())
        .with_site_config(config(json!({
            "flowchart": {
                "defaultRenderer": "dagre-wrapper",
                "edgeLabelPadding": 0,
                "compactEdgeCorners": false,
            },
        })));
    let source = r#"flowchart TD
    A[Start] --> B{Condition?}
    B -->|Yes| C[Execute]
    B -->|No| D[End]
"#;

    let effective = effective_config(&renderer, source);
    assert_eq!(effective["flowchart"]["defaultRenderer"], "dagre-wrapper");
    let svg = renderer
        .render_svg_sync(source)
        .expect("typed presentation render should succeed")
        .expect("flowchart should be detected");
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class") == Some("label")
                && node.attribute("data-id") == Some("L_B_C_0")
        })
        .expect("B-C edge label group");
    let background = label
        .parent()
        .expect("edge label container")
        .children()
        .find(|node| node.has_tag_name("rect"))
        .expect("edge label background");
    assert_eq!(background.attribute("rx"), Some("4"));
    assert_eq!(background.attribute("ry"), Some("4"));

    let isolated = HeadlessRenderer::new()
        .with_presentation(presentation())
        .with_site_config(config(json!({
            "flowchart": { "defaultRenderer": "dagre-wrapper" },
        })));
    let effective = effective_config(&isolated, source);
    assert!(effective["flowchart"].get("edgeCornerRadius").is_none());
    assert!(effective["flowchart"].get("edgeLabelPadding").is_none());
    assert!(effective["flowchart"].get("compactEdgeCorners").is_none());

    let layout = isolated
        .layout_json_sync(source)
        .expect("layout projection should succeed")
        .expect("flowchart should be detected");
    assert_json_keys_absent(
        &layout,
        &["edgeCornerRadius", "edgeLabelPadding", "compactEdgeCorners"],
    );
}

#[test]
fn render_plan_reports_each_presentation_aspect_independently() {
    let sequence = HeadlessRenderer::new()
        .with_presentation(presentation())
        .plan_svg_sync(SOURCE)
        .expect("sequence plan should succeed")
        .expect("sequence diagram should be detected");
    assert_eq!(sequence.presentation_profile_id(), Some("merman-modern"));
    assert_eq!(
        aspect_state(&sequence, "global-defaults"),
        PresentationAspectState::Active
    );
    assert_eq!(
        aspect_state(&sequence, "flowchart-svg"),
        PresentationAspectState::Inactive
    );
    assert_eq!(
        aspect_state(&sequence, "flowchart-elk-default"),
        PresentationAspectState::Inactive
    );

    let dagre = HeadlessRenderer::new()
        .with_presentation(presentation())
        .with_site_config(config(json!({
            "flowchart": { "defaultRenderer": "dagre-wrapper" },
        })))
        .plan_svg_sync("flowchart TD\nA --> B")
        .expect("Flowchart plan should succeed")
        .expect("Flowchart should be detected");
    assert!(dagre.is_ready());
    assert_eq!(
        aspect_state(&dagre, "flowchart-svg"),
        PresentationAspectState::Active
    );
    assert_eq!(
        aspect_state(&dagre, "flowchart-elk-default"),
        PresentationAspectState::Inactive
    );

    let default_flowchart = HeadlessRenderer::new()
        .with_presentation(presentation())
        .plan_svg_sync("flowchart TD\nA --> B")
        .expect("Flowchart plan should succeed")
        .expect("Flowchart should be detected");
    let expected_state = if merman::svg::layout_elk_available() {
        PresentationAspectState::Active
    } else {
        PresentationAspectState::Blocked
    };
    assert_eq!(
        aspect_state(&default_flowchart, "flowchart-elk-default"),
        expected_state
    );

    let parity = HeadlessRenderer::new()
        .plan_svg_sync(SOURCE)
        .expect("parity plan should succeed")
        .expect("sequence diagram should be detected");
    assert_eq!(parity.presentation_profile_id(), None);
    assert!(parity.presentation_aspects().is_empty());
}

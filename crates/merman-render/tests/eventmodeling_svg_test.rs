mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::EventModelingDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

#[test]
fn eventmodeling_typed_render_model_outputs_svg() {
    let input = r##"---
config:
  eventmodeling:
    padding: 24
    useMaxWidth: true
  themeVariables:
    emRelationStroke: '#135790'
    emCommandFill: '#DDEEFF'
    emCommandStroke: '#336699'
    textColor: '#111111'
---
eventmodeling
tf 01 ui Web.ShopCart
tf 02 cmd Cart.AddItem ->> 01 { sku: "SKU-1" }
tf 03 evt Cart.ItemAdded ->> 02 [[ItemAddedData]]
rf 04 rmo Cart.Summary
tf 05 evt Cart.CheckedOut

data ItemAddedData {
  sku: "SKU-1"
  quantity: 1
}
"##;

    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.metadata().diagram_type, "eventmodeling");

    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("eventmodeling-test".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap();
    let svg = rendered.svg();

    assert!(svg.contains(r#"aria-roledescription="eventmodeling""#));
    assert!(svg.contains(r#"width="100%""#));
    assert!(svg.contains(r#"max-width:"#));
    assert!(svg.contains(r#"<g/><g class="em-swimlane">"#));
    assert!(svg.contains(r#"class="em-relation""#));
    assert!(svg.contains(r#"class="em-box""#));
    assert!(svg.contains(r#"id="em-arrowhead-eventmodeling-test""#));
    assert!(svg.contains(r#"font-family: "trebuchet ms",verdana,arial,sans-serif;"#));
    assert!(svg.contains(r#"color: #111111;"#));
    assert!(svg.contains(r##"stroke="#135790""##));
    assert!(svg.contains(r##"fill="#DDEEFF""##));
}

#[test]
fn eventmodeling_svg_wires_accessibility_metadata_to_the_root() {
    let input = r#"eventmodeling
accTitle: Accessible event model
accDescr {
  Event model description
}
tf 01 event Start
"#;
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse eventmodeling")
        .expect("detect eventmodeling");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("eventmodeling-a11y".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render eventmodeling")
        .svg()
        .to_owned();

    assert!(svg.contains(r#"aria-labelledby="chart-title-eventmodeling-a11y""#));
    assert!(svg.contains(r#"aria-describedby="chart-desc-eventmodeling-a11y""#));
    assert!(
        svg.contains(
            r#"<title id="chart-title-eventmodeling-a11y">Accessible event model</title>"#
        )
    );
    assert!(
        svg.contains(r#"<desc id="chart-desc-eventmodeling-a11y">Event model description</desc>"#)
    );
}

#[test]
fn eventmodeling_docs_minimum_layout_tracks_upstream_html_label_metrics() {
    let input =
        include_str!("../../../fixtures/eventmodeling/upstream_docs_eventmodeling_minimum.mmd");
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    let projection = artifact
        .layout_json()
        .expect("serialize EventModeling layout");
    let layout: EventModelingDiagramLayout =
        serde_json::from_value(projection["layout"]["EventModelingDiagram"].clone())
            .expect("EventModeling layout projection");

    assert_close(layout.total_width, 1_157.666_666_666_666_7, 1.0);
    assert_close(layout.boxes[0].width, 134.0, 2.0);
    assert_close(layout.boxes[2].width, 307.333_333_333_333_3, 1.0);
    assert_close(layout.boxes[2].height, 116.0, 1.0);
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {actual} to be within {tolerance} of {expected}"
    );
}

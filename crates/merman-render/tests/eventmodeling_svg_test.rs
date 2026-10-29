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
fn eventmodeling_docs_minimum_layout_uses_bounded_deterministic_label_metrics() {
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

    assert_eq!(layout.boxes.len(), 5);
    assert!(!layout.swimlanes.is_empty());
    assert!(!layout.relations.is_empty());
    assert!(
        layout
            .boxes
            .iter()
            .all(|box_layout| box_layout.width.is_finite()
                && box_layout.height.is_finite()
                && (100.0..=470.0).contains(&box_layout.width)
                && (100.0..=770.0).contains(&box_layout.height)),
        "EventModeling boxes must stay inside the source-defined min/max geometry: {:?}",
        layout.boxes
    );
    assert!(
        layout.boxes[2].width > layout.boxes[0].width,
        "the data-rich event must remain wider than the short UI frame: {:?}",
        layout.boxes
    );
    assert!(
        layout.boxes[2].height >= layout.boxes[0].height,
        "the data-rich event must not become shorter than the short UI frame: {:?}",
        layout.boxes
    );
    let rightmost_box = layout
        .boxes
        .iter()
        .map(|box_layout| box_layout.x + box_layout.width)
        .fold(0.0_f64, f64::max);
    assert!(layout.total_width > rightmost_box);
    assert!(layout.total_height > 0.0);
}

#[test]
fn reset_frames_keep_explicit_sources_without_inventing_previous_frame_relations() {
    let source = "eventmodeling\ntf 01 ui UI\ntf 02 cmd Command\ntf 03 evt Event\nrf 04 rmo ReadModel ->> 03\nrf 05 ui OtherUI\n";
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let artifact = family::prepare(
        parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .unwrap();
    let projection = artifact.layout_json().unwrap();
    let layout: EventModelingDiagramLayout =
        serde_json::from_value(projection["layout"]["EventModelingDiagram"].clone()).unwrap();
    let pairs: Vec<_> = layout
        .relations
        .iter()
        .map(|relation| {
            (
                relation.source_frame.as_str(),
                relation.target_frame.as_str(),
            )
        })
        .collect();
    assert_eq!(pairs, [("01", "02"), ("02", "03"), ("03", "04")]);
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("em-relation"))
            .count(),
        3
    );
}

#[derive(Default)]
struct EventModelingLabelProbe {
    measured_lines: std::sync::Mutex<Vec<String>>,
}

impl merman_render::text::TextMeasurer for EventModelingLabelProbe {
    fn measure(
        &self,
        text: &str,
        style: &merman_render::text::TextStyle,
    ) -> merman_render::text::TextMetrics {
        merman_render::text::DeterministicTextMeasurer::default().measure(text, style)
    }

    fn measure_svg_simple_text_bbox_width_px(
        &self,
        text: &str,
        style: &merman_render::text::TextStyle,
    ) -> f64 {
        self.measured_lines.lock().unwrap().push(text.to_owned());
        merman_render::text::DeterministicTextMeasurer::default()
            .measure_svg_simple_text_bbox_width_px(text, style)
    }
}

fn render_prepared_eventmodeling_labels(
    source: &str,
    config: serde_json::Value,
) -> (serde_json::Value, String) {
    use merman_render::environment::{
        MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity,
    };
    use std::sync::Arc;

    let measurer = Arc::new(EventModelingLabelProbe::default());
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("eventmodeling-prepared-xhtml").unwrap(),
        "1",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(identity, measurer.clone())),
    );
    let parsed = Engine::new()
        .with_site_config(merman_core::MermaidConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let artifact = family::prepare(
        parsed,
        &LayoutOptions::headless_svg_defaults(),
        environment.begin_session().unwrap(),
    )
    .expect("prepare Event Modeling XHTML");
    let projection = artifact.layout_json().unwrap();
    let layout = projection["layout"]["EventModelingDiagram"].clone();
    let measured = measurer.measured_lines.lock().unwrap().clone();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("consume prepared Event Modeling XHTML");
    assert_eq!(
        *measurer.measured_lines.lock().unwrap(),
        measured,
        "emission must not wrap or measure the labels again",
    );
    let _document = roxmltree::Document::parse(rendered.svg()).expect("valid XHTML in SVG");
    let _: EventModelingDiagramLayout =
        serde_json::from_value(layout.clone()).expect("existing public layout schema");
    let mut keys = layout
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "bounds",
            "boxes",
            "padding",
            "relations",
            "swimlanes",
            "total_height",
            "total_width",
            "use_max_width",
            "viewbox_x",
            "viewbox_y",
        ]
    );
    for box_layout in layout["boxes"].as_array().unwrap() {
        let mut keys = box_layout
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "entity_identifier",
                "fill",
                "frame_kind",
                "frame_name",
                "height",
                "index",
                "model_entity_type",
                "stroke",
                "swimlane_index",
                "text",
                "width",
                "x",
                "y",
            ]
        );
    }
    (layout, rendered.svg().to_owned())
}

#[test]
fn eventmodeling_prepared_identifier_wrap_is_measured_and_emitted() {
    let identifier = "W".repeat(90);
    let source = format!("eventmodeling\ntf 01 ui {identifier}\n");
    let (layout, svg) = render_prepared_eventmodeling_labels(&source, serde_json::json!({}));
    assert_eq!(layout["boxes"][0]["text"], identifier);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let title = document
        .descendants()
        .find(|node| node.has_tag_name("b"))
        .unwrap();
    assert!(title.children().any(|node| node.has_tag_name("br")));
    let text = title
        .descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect::<String>();
    assert!(
        text.contains('-'),
        "wrapLabel must retain its generated hyphens"
    );
    assert_eq!(text.replace('-', ""), identifier);
}

#[test]
fn eventmodeling_prepared_data_preserves_wrap_nbsp_entities_and_reference_break() {
    let words = "alpha beta gamma delta ".repeat(8);
    let source = format!(
        "eventmodeling\ntf 01 ui Inline {{ {words}&lt;img&gt; &amp; &quot;q&quot; }}\ntf 02 evt Referenced [[D]]\ndata D {{\n  key: &lt;img&gt; &amp; &quot;q&quot; &bogus;\n}}\n"
    );
    let (layout, svg) = render_prepared_eventmodeling_labels(&source, serde_json::json!({}));
    assert!(
        layout["boxes"][0]["text"]
            .as_str()
            .unwrap()
            .contains(&words)
    );
    assert!(
        layout["boxes"][1]["text"]
            .as_str()
            .unwrap()
            .starts_with("Referenced\n")
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let codes = document
        .descendants()
        .filter(|node| node.has_tag_name("code"))
        .collect::<Vec<_>>();
    assert_eq!(codes.len(), 2);
    assert!(
        codes[0].children().any(|node| node.has_tag_name("br")),
        "inline data wraps"
    );
    assert!(
        !codes[0].last_child().unwrap().has_tag_name("br"),
        "inline data has no reference break"
    );
    assert!(
        codes[1].last_child().unwrap().has_tag_name("br"),
        "even short references retain the final break"
    );
    for code in &codes {
        let text = code
            .descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect::<String>();
        assert!(text.contains('\u{a0}'), "wrapped data spaces become NBSP");
        assert!(text.contains("<img>"), "encoded tags remain text");
        assert!(
            text.contains('&') && text.contains("\"q\""),
            "HTML entities decode once"
        );
        assert!(!code.descendants().any(|node| node.has_tag_name("img")));
    }
    let reference = codes[1]
        .descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect::<String>();
    assert!(
        reference.contains("&bogus;"),
        "unknown entities stay literal XML text"
    );
}

#[test]
fn eventmodeling_prepared_authored_data_is_sanitized_before_xhtml_emission() {
    let source = "eventmodeling\ntf 01 ui Safe { <img src=x onerror=alert(1)> &lt;script&gt;literal&lt;/script&gt; &amp; }\n";
    for config in [
        serde_json::json!({}),
        serde_json::json!({"securityLevel": "strict"}),
        serde_json::json!({"securityLevel": "loose"}),
        serde_json::json!({"securityLevel": "custom"}),
    ] {
        let (_, svg) = render_prepared_eventmodeling_labels(source, config);
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            !document
                .descendants()
                .any(|node| node.has_tag_name("script"))
        );
        assert!(document.descendants().all(|node| {
            node.attributes()
                .all(|attr| !attr.name().to_ascii_lowercase().starts_with("on"))
        }));
        let text = document
            .descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect::<String>();
        assert!(
            text.contains("script"),
            "entity-authored script remains literal text"
        );
    }
}

#[test]
fn eventmodeling_prepared_label_preserves_host_cancellation() {
    use merman_render::environment::{
        MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity,
    };
    use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
    use std::sync::Arc;

    struct CancellingMeasurer(merman_core::OperationControl);
    impl TextMeasurer for CancellingMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.0.cancel();
            merman_render::text::DeterministicTextMeasurer::default().measure(text, style)
        }
    }
    let control = merman_core::OperationControl::new();
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("eventmodeling-label-cancellation").unwrap(),
        "1",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(CancellingMeasurer(control.clone())),
        )),
    );
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(
            &format!("eventmodeling\ntf 01 ui {}\n", "W".repeat(90)),
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();
    let result = family::prepare(
        parsed,
        &LayoutOptions::headless_svg_defaults(),
        environment.begin_session_with_control(control).unwrap(),
    );
    let Err(merman_render::Error::Cancelled(error)) = result else {
        panic!("Event Modeling label preparation must preserve cancellation");
    };
    assert_eq!(error.phase, merman_core::OperationPhase::Layout);
}

mod stage_contract {
    use merman_core::{Engine, ParseOptions};
    use merman_render::environment::{
        MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity,
    };
    use merman_render::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle};
    use merman_render::{LayoutOptions, family};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Probe(Mutex<Vec<String>>);
    impl TextMeasurer for Probe {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            DeterministicTextMeasurer::default().measure(text, style)
        }
        fn measure_svg_simple_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.0.lock().unwrap().push(text.to_owned());
            DeterministicTextMeasurer::default().measure_svg_simple_text_bbox_width_px(text, style)
        }
    }

    #[test]
    fn eventmodeling_calculate_text_dimensions_keeps_pre_serialization_nbsp_spelling() {
        let source = "eventmodeling\ntf 01 ui Data { W W W W W W W W }\n";
        let probe = Arc::new(Probe::default());
        let id = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("audit-event-stage").unwrap(),
            "1",
        )
        .unwrap();
        let env = RenderEnvironment::deterministic().with_text_measurement_policy(
            TextMeasurementPolicy::uniform(TextMeasurementProfile::new(id, probe.clone())),
        );
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let artifact = family::prepare(
            parsed,
            &LayoutOptions::headless_svg_defaults(),
            env.begin_session().unwrap(),
        )
        .unwrap();
        assert_eq!(
            artifact.layout_json().unwrap()["layout"]["EventModelingDiagram"]["boxes"][0]["width"],
            333.0
        );
        let calls = probe.0.lock().unwrap();
        let code: Vec<_> = calls.iter().filter(|s| s.contains("<code")).collect();
        assert!(!code.is_empty());
        assert!(
            code.iter().all(|s| s.contains("&nbsp;")),
            "pinned db.ts measures generated content before XHTML serialization; drawSimpleText uses tspan.text, so entity spellings are literal in this probe"
        );
    }
}

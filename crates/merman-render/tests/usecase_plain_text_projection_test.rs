#![cfg(feature = "diagram-usecase")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::DeterministicTextMeasurer;
use std::sync::{Arc, Mutex};

fn assert_plain_actor_visible_projection(
    html_labels: bool,
    source_label: &str,
    expected_visible: &str,
    expected_label_width: f64,
) {
    let source = format!(
        "---\nconfig:\n  layout: dagre\n  look: classic\n  htmlLabels: {html_labels}\n  usecase:\n    wrappingWidth: 1000\n---\nusecase-beta\nactor A(\"{source_label}\")\n"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse Usecase actor")
        .expect("detect Usecase")
        .into_presentation();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = requests.clone();
    let measurer = DeterministicTextMeasurer::default().with_width_callback(move |text, _| {
        recorded.lock().unwrap().push(text.to_owned());
        text.chars().count() as f64 * 30.0
    });
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.usecase-plain-projection").unwrap(),
            "1",
        )
        .unwrap(),
        measurer,
    );
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile))
        .begin_session()
        .unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Usecase actor");
    let projection = artifact.layout_json().expect("Usecase layout projection");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Usecase actor");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Usecase SVG");
    let label = document
        .descendants()
        .find(|node| {
            if html_labels {
                node.has_tag_name("p")
            } else {
                node.has_tag_name("text")
            }
        })
        .expect("actor label");
    let visible: String = label
        .descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect();
    assert_eq!(visible, expected_visible);

    let node = projection["layout"]["UsecaseDiagram"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "A")
        .expect("actor layout");
    // These visible labels exceed the 56px actor glyph and receive 20px padding.
    assert_eq!(
        node["width"].as_f64(),
        Some(expected_label_width + 20.0),
        "htmlLabels={html_labels}, visible={visible:?}, node width={}, height={}",
        node["width"],
        node["height"],
    );
    if html_labels {
        let foreign_object = label
            .ancestors()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        assert_eq!(
            foreign_object
                .attribute("width")
                .unwrap()
                .parse::<f64>()
                .unwrap(),
            expected_label_width,
        );
    } else {
        assert_eq!(label.children().filter(|node| node.is_element()).count(), 1);
    }
    let requests = requests.lock().unwrap();
    assert!(
        requests.iter().any(|text| text == &visible),
        "visible actor text must reach measurement: {requests:?}"
    );
}

#[test]
fn usecase_html_plain_actor_measures_visible_ampersand() {
    assert_plain_actor_visible_projection(true, "A&B", "A&B", 90.0);
}

#[test]
fn usecase_svg_plain_actor_measures_visible_ampersand() {
    assert_plain_actor_visible_projection(false, "A&B", "A&B", 90.0);
}

#[test]
fn usecase_html_plain_actor_measures_literal_break_spelling() {
    assert_plain_actor_visible_projection(true, "A<br/>B", "A<br>B", 180.0);
}

#[test]
fn usecase_svg_plain_actor_measures_literal_break_spelling() {
    assert_plain_actor_visible_projection(false, "A<br/>B", "A<br>B", 180.0);
}

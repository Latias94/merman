#![cfg(feature = "diagram-state")]

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

#[test]
fn state_html_entity_break_spelling_remains_one_visible_line() {
    let source = "---\nconfig:\n  layout: dagre\n  look: classic\n  htmlLabels: true\n  themeVariables:\n    fontSize: 16px\n  state:\n    minNodeWidth: 0\n    wrappingWidth: 1000\n---\nstateDiagram-v2\nstate \"A&lt;br/&gt;B\" as S\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse State label")
        .expect("detect State");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = requests.clone();
    let measurer = DeterministicTextMeasurer::default().with_width_callback(move |text, _| {
        recorded.lock().unwrap().push(text.to_owned());
        text.chars().count() as f64 * 30.0
    });
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.state-html-entity-projection").unwrap(),
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
        .expect("prepare State label");
    let projection = artifact.layout_json().expect("State layout projection");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State label");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid State SVG");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("span")
                && node.attribute("class").is_some_and(|classes| {
                    classes.split_whitespace().any(|class| class == "nodeLabel")
                })
        })
        .expect("State node label");
    let visible: String = label
        .descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect();
    assert_eq!(visible, "A<br/>B");
    assert!(!label.descendants().any(|node| node.has_tag_name("br")));
    let foreign_object = label
        .ancestors()
        .find(|node| node.has_tag_name("foreignObject"))
        .expect("State HTML label box");
    // Seven literal characters occupy one 16px * 1.5 HTML line box.
    assert_eq!(foreign_object.attribute("width"), Some("210"));
    assert_eq!(foreign_object.attribute("height"), Some("24"));
    let node = projection["layout"]["StateDiagramV2"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "S")
        .expect("State layout node");
    // StateDB assigns this rounded node 8px padding, ahead of the config fallback.
    assert_eq!(
        node["width"].as_f64(),
        Some(226.0),
        "visible={visible:?}, node={node}"
    );
    assert_eq!(
        node["height"].as_f64(),
        Some(40.0),
        "visible={visible:?}, node={node}"
    );
    let requests = requests.lock().unwrap();
    assert!(
        requests.iter().any(|text| text == &visible),
        "the literal HTML text line must reach measurement: {requests:?}"
    );
}

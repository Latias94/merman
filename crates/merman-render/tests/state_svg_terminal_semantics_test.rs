#![cfg(feature = "diagram-state")]

use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::StateDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::DeterministicTextMeasurer;
use std::sync::{Arc, Mutex};

struct RenderedState {
    svg: String,
    layout: StateDiagramLayout,
    requests: Vec<String>,
}

fn render(body: &str) -> RenderedState {
    let source = format!(
        "---\nconfig:\n  layout: dagre\n  look: classic\n  theme: default\n  htmlLabels: false\n  themeVariables:\n    fontSize: 16px\n  state:\n    minNodeWidth: 0\n    wrappingWidth: 1000\n---\nstateDiagram-v2\n{body}\n"
    );
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse State terminal fixture")
        .expect("detect State");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = requests.clone();
    let measurer = DeterministicTextMeasurer::default().with_width_callback(move |text, _| {
        recorded.lock().unwrap().push(text.to_owned());
        text.chars().count() as f64 * 10.0
    });
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.state-svg-terminal").unwrap(),
            "1",
        )
        .unwrap(),
        measurer,
    );
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile))
        .begin_session()
        .unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare State terminal fixture");
    let layout = serde_json::from_value(
        artifact.layout_json().expect("State layout")["layout"]["StateDiagramV2"].clone(),
    )
    .expect("typed State layout");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State terminal fixture")
        .svg()
        .to_owned();
    let requests = requests.lock().unwrap().clone();
    RenderedState {
        svg,
        layout,
        requests,
    }
}

fn visible_text(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect()
}

fn rows(svg: &str) -> Vec<String> {
    let document = roxmltree::Document::parse(svg).expect("valid State SVG XML");
    document
        .descendants()
        .filter(|node| {
            node.has_tag_name("tspan")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|class| class == "text-outer-tspan")
                })
        })
        .map(visible_text)
        .collect()
}

fn node_height(rendered: &RenderedState, id: &str) -> f64 {
    rendered
        .layout
        .nodes
        .iter()
        .find(|node| node.id == id)
        .expect("State node")
        .height
}

#[test]
fn state_svg_entities_use_create_text_projection_once() {
    // Both pinned createText versions (12.2.0 and 12.1.0) decode only amp/lt/gt.
    for (source, visible) in [
        ("A&lt;br&gt;B", "A<br>B"),
        ("A&lt;br/&gt;B", "A<br/>B"),
        ("A&amp;lt;br&amp;gt;B", "A&lt;br&gt;B"),
        ("&nbsp;", "&nbsp;"),
        ("A&shy;B", "A&shy;B"),
        ("A#lt;br#gt;B", "A<br>B"),
        ("A#nbsp;B", "A&nbsp;B"),
        ("A#shy;B", "A&shy;B"),
    ] {
        let rendered = render(&format!("state \"{source}\" as A"));
        assert_eq!(
            rows(&rendered.svg),
            [visible],
            "source={source}: {}",
            rendered.svg
        );
        assert!(
            rendered.requests.iter().any(|text| text == visible),
            "literal SVG text must reach measurement for {source}: {:?}",
            rendered.requests
        );
    }
}

#[test]
fn state_svg_entity_produced_break_does_not_add_phantom_node_height() {
    let control = render("state \"AXB\" as A");
    for source in ["A&lt;br&gt;B", "A&lt;br/&gt;B", "A#lt;br#gt;B"] {
        let rendered = render(&format!("state \"{source}\" as A"));
        assert_eq!(
            node_height(&rendered, "A"),
            node_height(&control, "A"),
            "{source}"
        );
    }
    let actual_break = render("state \"A<br/>B\" as A");
    assert_eq!(rows(&actual_break.svg), ["A", "B"]);
    assert!(node_height(&actual_break, "A") > node_height(&control, "A"));
}

#[test]
fn state_svg_styled_words_keep_literal_break_and_source_row_boundary() {
    let rendered = render("state \"**A&lt;br&gt;B**<br/>C\" as A");
    assert_eq!(rows(&rendered.svg), ["A<br>B", "C"]);
    let document = roxmltree::Document::parse(&rendered.svg).unwrap();
    let literal = document
        .descendants()
        .find(|node| {
            node.has_tag_name("tspan")
                && node.attribute("class") == Some("text-inner-tspan")
                && visible_text(*node) == "A<br>B"
        })
        .expect("styled SVG word");
    assert_eq!(literal.attribute("font-weight"), Some("bold"));
    assert_eq!(literal.attribute("font-style"), Some("normal"));
}

#[test]
fn state_svg_block_notes_share_terminal_text_and_height() {
    let control = render("A\nnote right of A\nAXB\nend note");
    let note_height = |rendered: &RenderedState| {
        rendered
            .layout
            .nodes
            .iter()
            .find(|node| node.id.contains("note"))
            .expect("State note")
            .height
    };
    for (source, visible) in [
        ("A&lt;br&gt;B", "A<br>B"),
        ("&nbsp;", "&nbsp;"),
        ("A&shy;B", "A&shy;B"),
    ] {
        let rendered = render(&format!("A\nnote right of A\n{source}\nend note"));
        assert!(
            rows(&rendered.svg).iter().any(|row| row == visible),
            "{}",
            rendered.svg
        );
        assert_eq!(rows(&rendered.svg).len(), 2, "{}", rendered.svg);
        assert_eq!(note_height(&rendered), note_height(&control));
        assert!(rendered.requests.iter().any(|text| text == visible));
    }
}

#[test]
fn state_svg_edge_labels_keep_shorthand_entities_literal() {
    let control = render("A --> B : AXB");
    let label_height = |rendered: &RenderedState| {
        rendered
            .layout
            .edges
            .iter()
            .find(|edge| edge.id == "edge0")
            .and_then(|edge| edge.label.as_ref())
            .expect("State edge label")
            .height
    };
    // Shorthand entities survive State's semicolon grammar in transition labels.
    for (source, visible) in [
        ("A#lt;br#gt;B", "A<br>B"),
        ("A#nbsp;B", "A&nbsp;B"),
        ("A#shy;B", "A&shy;B"),
    ] {
        let rendered = render(&format!("A --> B : {source}"));
        let document = roxmltree::Document::parse(&rendered.svg).unwrap();
        let label = document
            .descendants()
            .find(|node| node.has_tag_name("g") && node.attribute("data-id") == Some("edge0"))
            .expect("State edge label terminal");
        assert_eq!(visible_text(label), visible, "{}", rendered.svg);
        assert_eq!(label_height(&rendered), label_height(&control));
        assert!(rendered.requests.iter().any(|text| text == visible));
    }
    let actual_break = render("A --> B : A<br/>B");
    assert!(label_height(&actual_break) > label_height(&control));
}

#[test]
fn state_svg_cluster_titles_keep_literal_entities_and_one_row_height() {
    let control = render("state \"AXB\" as A {\nB\n}");
    let title_height = |rendered: &RenderedState| {
        rendered
            .layout
            .clusters
            .iter()
            .find(|cluster| cluster.id == "A")
            .expect("State cluster")
            .title_label
            .height
    };
    for (source, visible) in [
        ("A&lt;br&gt;B", "A<br>B"),
        ("&nbsp;", "&nbsp;"),
        ("A&shy;B", "A&shy;B"),
    ] {
        let rendered = render(&format!("state \"{source}\" as A {{\nB\n}}"));
        let document = roxmltree::Document::parse(&rendered.svg).unwrap();
        let label = document
            .descendants()
            .find(|node| node.attribute("class") == Some("cluster-label"))
            .expect("State cluster label");
        assert_eq!(visible_text(label), visible);
        assert_eq!(title_height(&rendered), title_height(&control));
        assert!(rendered.requests.iter().any(|text| text == visible));
    }
}

#[test]
fn state_svg_title_and_description_use_plain_svg_entity_projection() {
    let rendered = render("state \"A&lt;br&gt;B\" as A: C#lt;br#gt;D");
    let control = render("state \"AXB\" as A: CXD");
    assert_eq!(rows(&rendered.svg), ["A<br>B", "C<br>D"]);
    assert_eq!(node_height(&rendered, "A"), node_height(&control, "A"));
    for visible in ["A<br>B", "C<br>D"] {
        assert!(
            rendered.requests.iter().any(|text| text == visible),
            "{:?}",
            rendered.requests
        );
    }
    let named = render("state \"&nbsp;\" as A: A#shy;B");
    assert_eq!(rows(&named.svg), ["&nbsp;", "A&shy;B"]);
}

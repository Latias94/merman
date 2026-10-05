use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render(source: &str, config: serde_json::Value) -> String {
    let parsed = Engine::new()
        .with_site_config(MermaidConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse State")
        .expect("State diagram");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare State")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State")
        .svg()
        .to_string()
}

#[test]
fn state_choice_stroke_width_uses_node_override_or_source_default() {
    for (style, expected) in [("", "1.3"), ("style Decide stroke-width:5px", "5")] {
        let svg = render(
            &format!("stateDiagram-v2\nstate Decide <<choice>>\n[*] --> Decide\n{style}\n"),
            serde_json::json!({"themeVariables": {"strokeWidth": 9}}),
        );
        let document = roxmltree::Document::parse(&svg).expect("State SVG");
        let choice = document
            .descendants()
            .find(|node| {
                node.attribute("id").is_some_and(|id| id.contains("Decide"))
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "node")
                    })
            })
            .expect("choice node");
        let outline = choice
            .descendants()
            .find(|node| node.has_tag_name("path") && node.attribute("fill") == Some("none"))
            .expect("choice outline");
        assert_eq!(outline.attribute("stroke-width"), Some(expected));
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn state_elk_preserves_parallel_self_loop_paths_and_labels_without_dagre_dummy_nodes() {
    let svg = render(
        "stateDiagram-v2\nActive --> Active: heartbeat\nActive --> Active: refresh\nPaused --> Paused: wait\n",
        serde_json::json!({"layout": "elk"}),
    );
    let document = roxmltree::Document::parse(&svg).expect("State SVG");
    for (id, text) in [
        ("edge0", "heartbeat"),
        ("edge1", "refresh"),
        ("edge2", "wait"),
    ] {
        assert_eq!(
            document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("path") && node.attribute("data-id") == Some(id)
                })
                .count(),
            1,
            "one logical path for {id}"
        );
        let labels: Vec<_> = document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("data-id") == Some(id))
            .collect();
        assert_eq!(labels.len(), 1, "one logical label for {id}");
        assert!(
            labels[0]
                .descendants()
                .any(|node| node.text() == Some(text))
        );
    }
    assert!(
        !document
            .descendants()
            .any(|node| { node.attribute("id").is_some_and(|id| id.contains("---")) }),
        "ELK does not create Dagre self-loop placeholders"
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn state_elk_normalizes_breaks_before_markdown_block_parsing() {
    let svg = render(
        "stateDiagram-v2\nA --> B: Hello<br/>- l1<br/>- l2\n",
        serde_json::json!({"layout": "elk"}),
    );
    let document = roxmltree::Document::parse(&svg).expect("State SVG");
    let label = document
        .descendants()
        .find(|node| node.has_tag_name("span") && node.attribute("class") == Some("edgeLabel"))
        .expect("edge label");
    let elements: Vec<_> = label.children().filter(|node| node.is_element()).collect();
    assert_eq!(elements.len(), 1);
    assert!(elements[0].has_tag_name("p"));
    assert_eq!(elements[0].text(), Some("Hello"));
    assert!(!elements[0].children().any(|node| node.is_element()));
    let trailing: String = label
        .children()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect();
    assert_eq!(
        trailing.split_whitespace().collect::<Vec<_>>(),
        ["-", "l1", "-", "l2"]
    );
}

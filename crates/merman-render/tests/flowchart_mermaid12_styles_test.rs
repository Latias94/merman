use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use std::path::PathBuf;

fn render(source: &str) -> String {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse Flowchart")
        .expect("Flowchart diagram");
    family::prepare(
        parsed,
        &LayoutOptions::headless_svg_defaults(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .expect("layout Flowchart")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render Flowchart")
    .svg()
    .to_owned()
}

#[test]
fn flowchart_stylesheet_matches_mermaid12_neo_reference() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let name = "upstream_cypress_flowchart_elk_spec_74_elk_handle_labels_for_multiple_edges_from_and_to_the_same_cou_034";
    let source = std::fs::read_to_string(fixtures.join("flowchart").join(format!("{name}.mmd")))
        .expect("read Flowchart source");
    let upstream = std::fs::read_to_string(
        fixtures
            .join("upstream-svgs/flowchart")
            .join(format!("{name}.svg")),
    )
    .expect("read Mermaid 12 SVG");
    let local = render(&source);
    let stylesheet = |svg: &str| {
        roxmltree::Document::parse(svg)
            .expect("valid SVG")
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("stylesheet")
            .to_owned()
    };
    // Internal filter IDs are document-scoped; retain the upstream stylesheet contract
    // after verifying that the local reference points to the emitted definition.
    let document = roxmltree::Document::parse(&local).unwrap();
    let filter_id = document
        .descendants()
        .find(|node| {
            node.has_tag_name("filter")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.ends_with("-drop-shadow"))
        })
        .and_then(|node| node.attribute("id"))
        .expect("emitted Neo shadow filter");
    assert!(stylesheet(&local).contains(&format!("url(#{filter_id})")));
    assert_eq!(
        stylesheet(&local).replace(&format!("url(#{filter_id})"), "url(#merman-drop-shadow)"),
        stylesheet(&upstream).replace(name, "merman")
    );
}

#[test]
fn flowchart_html_edge_labels_inherit_resolved_theme_font_size() {
    for (configuration, expected_height) in [
        ("", "21"),
        ("  theme: default\n  look: classic\n", "24"),
        ("  themeVariables:\n    fontSize: 20px\n", "30"),
    ] {
        let source = format!(
            "---\nconfig:\n  layout: dagre\n{configuration}---\nflowchart LR\nA -->|edge label| B\n"
        );
        let svg = render(&source);
        let document = roxmltree::Document::parse(&svg).expect("valid SVG");
        let label = document
            .descendants()
            .find(|node| {
                node.has_tag_name("foreignObject")
                    && node
                        .descendants()
                        .any(|text| text.text() == Some("edge label"))
            })
            .expect("edge label foreignObject");
        assert_eq!(
            label.attribute("height"),
            Some(expected_height),
            "{configuration}"
        );
    }
}

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_flowchart(source: &str) -> String {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("valid Flowchart source")
        .expect("Flowchart diagram")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("Flowchart layout")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("Flowchart SVG")
        .svg()
        .to_owned()
}

#[test]
fn flowchart_label_keywords_preserve_accessible_title_and_visible_text() {
    for html_labels in [false, true] {
        let source = format!(
            "---\nconfig:\n  layout: dagre\n  look: classic\n  htmlLabels: {html_labels}\n---\nflowchart LR\naccTitle: Root title\nA[\"First\naccTitle: Visible\nLast\"] --> B\n"
        );
        let svg = render_flowchart(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let title_id = document
            .root_element()
            .attribute("aria-labelledby")
            .unwrap();
        let title = document
            .descendants()
            .find(|node| node.has_tag_name("title") && node.attribute("id") == Some(title_id))
            .expect("root accessibility reference must resolve");
        assert_eq!(title.text(), Some("Root title"));
        let node = document
            .descendants()
            .find(|node| {
                node.attribute("data-id") == Some("A") && node.attribute("data-et") == Some("node")
            })
            .unwrap();
        let text = node
            .descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .flat_map(str::split_whitespace)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains("accTitle: Visible"), "{html_labels}: {svg}");
    }
}

#[test]
fn flowchart_asset_frame_ignores_numeric_zero_but_accepts_string_zero() {
    let frame_path = |update: &str| {
        let source = format!(
            "---\nconfig:\n  layout: dagre\n  look: classic\n---\nflowchart LR\nA@{{ shape: icon, w: 96, h: 96 }}\nA@{{ {update} }}\nA --> B\n"
        );
        let svg = render_flowchart(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let node = document
            .descendants()
            .find(|node| {
                node.attribute("data-id") == Some("A") && node.attribute("data-et") == Some("node")
            })
            .unwrap();
        node.descendants()
            .find(|node| node.has_tag_name("path"))
            .expect("asset-free icon frame")
            .attribute("d")
            .unwrap()
            .to_owned()
    };
    let retained = frame_path("w: 96, h: 96");
    assert_eq!(frame_path("w: 0, h: 0"), retained);
    assert_ne!(frame_path("w: \"0\", h: \"0\""), retained);
}

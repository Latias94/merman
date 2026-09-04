use merman_core::{
    Engine, OperationControl, ParseOptions, baseline::PINNED_MERMAID_BASELINE_VERSION,
};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_svg(source: &str, diagram_id: &str) -> String {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::lenient())
        .expect("source parses")
        .expect("source detects a diagram");
    let session = RenderEnvironment::deterministic()
        .begin_session_with_control(OperationControl::new())
        .expect("render session starts");
    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("family layout succeeds")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_owned()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("canonical SVG succeeds")
        .svg()
        .to_owned()
}

#[test]
fn info_canonical_svg_keeps_document_root_and_version_structure() {
    let svg = render_svg("info", "info-parity");
    let document = roxmltree::Document::parse(&svg).expect("canonical Info SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("class"), Some("info"));
    assert_eq!(root.attribute("viewBox"), None);
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 400px; background-color: white;")
    );
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("text") && node.attribute("class") == Some("version"))
    );
    assert!(svg.contains(&format!(">v{PINNED_MERMAID_BASELINE_VERSION}</text>")));
}

#[test]
fn error_canonical_svg_keeps_source_backed_icon_and_text_classes() {
    let svg = render_svg("flowchart TD\nA -->\n", "error-parity");
    let document = roxmltree::Document::parse(&svg).expect("canonical Error SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("class"), Some("error"));
    assert_eq!(root.attribute("viewBox"), Some("0 0 2412 512"));
    assert_eq!(root.attribute("style"), Some("max-width: 512px;"));
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("error-icon"))
            .count(),
        6
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("error-text"))
            .count(),
        2
    );
    assert!(svg.contains("Syntax error in text"));
    assert!(svg.contains(&format!(
        "mermaid version {PINNED_MERMAID_BASELINE_VERSION}"
    )));
}

#[test]
fn packet_canonical_svg_keeps_root_profile_and_packet_dom_roles() {
    let svg = render_svg(
        "packet\ntitle Header\n0-7: \"Version\"\n8-15: \"Length\"\n",
        "packet-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Packet SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("class"), None);
    assert_eq!(root.attribute("viewBox"), Some("0 0 1026 94"));
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 1026px; background-color: white;")
    );
    assert_eq!(
        document
            .descendants()
            .filter(
                |node| node.has_tag_name("rect") && node.attribute("class") == Some("packetBlock")
            )
            .count(),
        2
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("packetLabel"))
            .count(),
        2
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("packetByte start"))
            .count(),
        2
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("packetByte end"))
            .count(),
        2
    );
    assert!(svg.contains("chart-title-packet-parity"));
}

#[test]
fn radar_canonical_svg_keeps_root_profile_and_family_roles() {
    let svg = render_svg(
        "radar-beta\ntitle Radar parity\naxis A,B,C\ncurve score{1,2,3}\n",
        "radar-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Radar SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("overflow"), Some("visible"));
    assert_eq!(root.attribute("viewBox"), Some("0 0 700 700"));
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 700px; background-color: white;")
    );
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("radarTitle"))
    );
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("radarAxisLabel"))
    );
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("radarCurve-0"))
    );
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("radarLegendBox-0"))
    );
}

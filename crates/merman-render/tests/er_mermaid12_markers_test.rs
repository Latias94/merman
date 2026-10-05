use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_er(layout: &str, look: &str) -> String {
    let source = format!(
        "---\nconfig:\n  layout: {layout}\n  look: {look}\n  themeVariables:\n    strokeWidth: 3\n    mainBkg: '#123456'\n---\nerDiagram\nA ||--o{{ B : owns\n"
    );
    render_er_source(&source)
}

fn render_er_source(source: &str) -> String {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned()
}

#[test]
fn er_neo_cardinality_markers_use_theme_and_source_geometry() {
    for look in ["neo", "classic", "handDrawn"] {
        let svg = render_er("dagre", look);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let markers: Vec<_> = document
            .descendants()
            .filter(|node| node.has_tag_name("marker"))
            .collect();
        assert_eq!(markers.len(), 8);
        for marker in markers {
            assert_eq!(
                marker.attribute("markerUnits"),
                (look == "neo").then_some("userSpaceOnUse")
            );
            for shape in marker.children().filter(|node| node.is_element()) {
                assert_eq!(
                    shape.attribute("stroke-width"),
                    (look == "neo").then_some("3")
                );
                if shape.has_tag_name("circle") {
                    assert_eq!(
                        shape.attribute("fill"),
                        Some(if look == "neo" { "#123456" } else { "white" })
                    );
                    if marker.attribute("id").unwrap().ends_with("zeroOrMoreStart") {
                        assert_eq!(
                            shape.attribute("cx"),
                            Some(if look == "neo" { "45.5" } else { "48" })
                        );
                    }
                    if marker.attribute("id").unwrap().ends_with("zeroOrMoreEnd") {
                        assert_eq!(
                            shape.attribute("cx"),
                            Some(if look == "neo" { "11" } else { "9" })
                        );
                    }
                }
            }
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_elk_common_painter_keeps_edges_between_clusters_and_nodes() {
    let svg = render_er("elk", "neo");
    let document = roxmltree::Document::parse(&svg).unwrap();
    let root = document
        .descendants()
        .find(|node| node.attribute("class") == Some("root"))
        .unwrap();
    let groups: Vec<_> = root
        .children()
        .filter(|node| node.is_element())
        .map(|node| node.attribute("class").unwrap_or(""))
        .collect();
    assert_eq!(
        groups,
        ["clusters", "edges edgePaths", "edgeLabels", "nodes"]
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_elk_uses_rounded_paths_default_fill_and_neo_masks() {
    for look in ["neo", "classic"] {
        let svg = render_er_source(&format!(
            "---\nconfig:\n  layout: elk\n  look: {look}\n---\nerDiagram\nCUSTOMER ||--|{{ ADDRESS : invoiced\nCUSTOMER ||..|{{ ADDRESS : receives\n"
        ));
        let document = roxmltree::Document::parse(&svg).unwrap();
        let paths: Vec<_> = document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .collect();
        assert_eq!(paths.len(), 2);
        assert!(
            paths
                .iter()
                .any(|path| path.attribute("d").unwrap().contains('Q'))
        );
        for path in paths {
            assert!(!path.attribute("d").unwrap().contains('C'));
            let style = path.attribute("style").unwrap();
            assert!(style.ends_with("fill:none;;;fill:none"), "{style}");
            assert!(!style.contains("undefined"), "{style}");
            if look == "neo" {
                assert!(style.starts_with("stroke-dasharray: 0 0 "), "{style}");
                assert!(style.contains("stroke-dashoffset: 0;"), "{style}");
                if path
                    .attribute("class")
                    .unwrap()
                    .contains("edge-pattern-dashed")
                {
                    assert!(style.starts_with("stroke-dasharray: 0 0 2 2 "), "{style}");
                }
            } else {
                assert!(!style.contains("stroke-dashoffset:"), "{style}");
            }
        }
    }
}

#[test]
fn er_neo_stylesheet_matches_mermaid12_reference() {
    let name = "upstream_cypress_erdiagram_spec_should_render_an_er_diagram_with_multiple_relationships_between_003";
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let source = std::fs::read_to_string(root.join("er").join(format!("{name}.mmd"))).unwrap();
    let upstream =
        std::fs::read_to_string(root.join("upstream-svgs/er").join(format!("{name}.svg"))).unwrap();
    let local = render_er_source(&source);
    let stylesheet = |svg: &str| {
        roxmltree::Document::parse(svg)
            .unwrap()
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .unwrap()
            .to_owned()
    };
    assert_eq!(
        stylesheet(&local),
        stylesheet(&upstream).replace(name, "merman")
    );
}

#[test]
fn er_palette_styles_cover_stamped_slots_and_wrap_backgrounds() {
    let svg = render_er_source(
        "---\nconfig:\n  theme: redux-color\n  themeVariables:\n    THEME_COLOR_LIMIT: 1\n    borderColorArray: ['#112233 ', '#445566']\n    bkgColorArray: ['#aabbcc ']\n---\nerDiagram\nA ||--o{ B : owns\n",
    );
    assert!(svg.contains(r##"[data-color-id="color-1"].node path{stroke:#445566;fill:#aabbcc;}"##));
    assert!(svg.contains(r##"[data-color-id="color-0"].node path{stroke:#112233;fill:#aabbcc;}"##));
}

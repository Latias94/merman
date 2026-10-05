use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_requirement(layout: &str, look: &str) -> String {
    let source = format!(
        "---\nconfig:\n  layout: {layout}\n  look: {look}\n  themeVariables:\n    strokeWidth: 3\n    mainBkg: '#123456'\n    requirementBackground: '#abcdef'\n---\nrequirementDiagram\nrequirement Req {{\n  id: 1\n  text: body\n  risk: high\n  verifymethod: analysis\n}}\n"
    );
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
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
fn requirement_neo_markers_and_divider_use_source_paint_rules() {
    for look in ["neo", "classic"] {
        let svg = render_requirement("dagre", look);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let arrow = document
            .descendants()
            .find(|node| {
                node.has_tag_name("marker")
                    && node
                        .attribute("id")
                        .is_some_and(|id| id.ends_with("requirement_arrowEnd"))
            })
            .unwrap();
        assert_eq!(
            arrow.attribute("markerUnits"),
            (look == "neo").then_some("userSpaceOnUse")
        );
        assert_eq!(
            arrow.attribute("stroke-width"),
            (look == "neo").then_some("3")
        );
        assert_eq!(
            arrow.attribute("viewBox"),
            (look == "neo").then_some("0 0 25 20")
        );
        assert_eq!(
            arrow
                .first_element_child()
                .unwrap()
                .attribute("stroke-linejoin"),
            (look == "neo").then_some("miter")
        );
        let contains = document
            .descendants()
            .find(|node| {
                node.has_tag_name("marker")
                    && node
                        .attribute("id")
                        .is_some_and(|id| id.ends_with("requirement_containsStart"))
            })
            .unwrap();
        for shape in contains
            .descendants()
            .filter(|node| node.has_tag_name("circle") || node.has_tag_name("line"))
        {
            assert_eq!(
                shape.attribute("stroke-width"),
                (look == "neo").then_some("3")
            );
        }
        let outer = document
            .descendants()
            .find(|node| node.attribute("class") == Some("basic label-container outer-path"))
            .unwrap();
        assert_eq!(
            outer.first_element_child().unwrap().attribute("fill"),
            Some("#123456")
        );
        let divider = document
            .descendants()
            .find(|node| node.attribute("class") == Some("divider"))
            .unwrap();
        assert_eq!(
            divider.children().filter(|node| node.is_element()).count(),
            if look == "neo" { 2 } else { 1 }
        );
        if look == "neo" {
            let fill = divider.first_element_child().unwrap();
            assert_eq!(fill.attribute("fill"), Some("#123456"));
            assert_eq!(fill.attribute("fill-rule"), Some("evenodd"));
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn requirement_elk_uses_common_edge_layer_class() {
    let svg = render_requirement("elk", "neo");
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("edges edgePaths"))
    );
}

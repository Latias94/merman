use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_flowchart(source: &str) -> String {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse Flowchart")
        .expect("Flowchart diagram");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("layout Flowchart")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Flowchart")
        .svg()
        .to_owned()
}

#[test]
fn flowchart_look_selects_complete_colored_marker_definitions() {
    for look in ["classic", "neo", "handDrawn"] {
        let svg = render_flowchart(&format!(
            "---\nconfig:\n  layout: dagre\n  look: {look}\n---\nflowchart LR\nA <--> B\nB o--o C\nC x--x D\nlinkStyle default stroke:#123456\n"
        ));
        let document = roxmltree::Document::parse(&svg).expect("valid SVG");
        let mut references = 0;
        for edge in document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
        {
            for attribute in ["marker-start", "marker-end"] {
                let reference = edge.attribute(attribute).expect("double-ended marker");
                let id = reference
                    .strip_prefix("url(#")
                    .unwrap()
                    .strip_suffix(')')
                    .unwrap();
                assert_eq!(id.contains("-margin"), look == "neo", "{id}");
                let marker = document
                    .descendants()
                    .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id))
                    .expect("every reference resolves to its colored marker");
                for shape in marker.children().filter(|node| {
                    node.has_tag_name("path")
                        || node.has_tag_name("circle")
                        || node.has_tag_name("line")
                }) {
                    let color = if look == "handDrawn" {
                        "stroke:#123456"
                    } else {
                        "#123456"
                    };
                    assert_eq!(shape.attribute("stroke"), Some(color));
                    if id.contains("-point") {
                        assert_eq!(shape.attribute("fill"), Some(color));
                    }
                }
                references += 1;
            }
        }
        assert_eq!(references, 6);
        for rect in document.descendants().filter(|node| {
            node.has_tag_name("rect") && node.attribute("class") == Some("basic label-container")
        }) {
            assert_eq!(
                rect.attribute("rx"),
                None,
                "process rounding belongs to theme CSS"
            );
            assert_eq!(rect.attribute("ry"), None);
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_edge_group_uses_shared_plural_class() {
    let svg = render_flowchart("flowchart LR\nA --> B\n");
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("edges edgePaths"))
    );
}

#[test]
fn flowchart_neo_animated_edges_keep_unoffset_markers() {
    let svg = render_flowchart(
        "---\nconfig:\n  layout: dagre\n  look: neo\n---\nflowchart LR\nA e1@--> B\ne1@{ animate: true }\nlinkStyle default stroke:red\n",
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let edge = document
        .descendants()
        .find(|node| node.attribute("data-edge") == Some("true"))
        .unwrap();
    let reference = edge.attribute("marker-end").unwrap();
    assert!(reference.ends_with("-pointEnd_red)"), "{reference}");
    let id = reference
        .strip_prefix("url(#")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id))
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_diamond_departures_stay_on_the_routed_axis() {
    use base64::Engine as _;
    let svg = render_flowchart(include_str!("../../../fixtures/flowchart/basic.mmd"));
    let document = roxmltree::Document::parse(&svg).unwrap();
    for id in ["L_B_C_0", "L_B_D_0"] {
        let edge = document
            .descendants()
            .find(|node| {
                node.attribute("data-id") == Some(id) && node.attribute("data-edge") == Some("true")
            })
            .unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(edge.attribute("data-points").unwrap())
            .unwrap();
        let points: Vec<serde_json::Value> = serde_json::from_slice(&decoded).unwrap();
        assert_eq!(points.len(), 5);
        assert_eq!(
            points[0]["x"], points[1]["x"],
            "orthogonal diamond departure"
        );
        let y = points[0]["y"].as_f64().unwrap();
        assert!(
            (y - 242.83326625823975).abs() < 1e-6,
            "upstream outline attachment: {y}"
        );
        let path = edge.attribute("d").unwrap();
        assert_eq!(
            path.matches('Q').count(),
            2,
            "no diagonal terminal turn: {path}"
        );
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_rounding_is_independent_of_theme_node_radius() {
    let source = "flowchart TD\nA[Start] --> B{Choice}\nB -->|Yes| C[OK]\nB -->|No| D[Fail]\n";
    let paths = |radius| {
        let svg = render_flowchart(&format!(
            "---\nconfig:\n  themeVariables:\n    radius: {radius}\n---\n{source}"
        ));
        let document = roxmltree::Document::parse(&svg).unwrap();
        document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .map(|node| node.attribute("d").unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(paths(5), paths(24));
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_line_hops_follow_elk_config_and_preserve_routes() {
    let source = "flowchart TD\nA & B --> C & D\n";
    let edges = |config: &str| {
        let svg = render_flowchart(&format!("---\nconfig:\n{config}---\n{source}"));
        let document = roxmltree::Document::parse(&svg).unwrap();
        document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .map(|node| {
                (
                    node.attribute("d").unwrap().to_owned(),
                    node.attribute("data-points").unwrap().to_owned(),
                )
            })
            .collect::<Vec<_>>()
    };
    let swimlane_disabled = edges("  swimlane:\n    lineHops: false\n");
    let elk_disabled = edges("  elk:\n    lineHops: false\n");
    let elk_gap = edges("  elk:\n    lineHops: gap\n");
    assert!(
        swimlane_disabled.iter().any(|(path, _)| path.contains('A')),
        "default ELK crossings have arcs"
    );
    assert!(elk_disabled.iter().all(|(path, _)| !path.contains('A')));
    assert!(
        elk_gap
            .iter()
            .any(|(path, _)| path.matches('M').count() > 1),
        "gap splits the painted path"
    );
    for ((swimlane_disabled, elk_disabled), elk_gap) in
        swimlane_disabled.iter().zip(&elk_disabled).zip(&elk_gap)
    {
        assert_eq!(
            swimlane_disabled.1, elk_disabled.1,
            "paint must preserve routed geometry"
        );
        assert_eq!(swimlane_disabled.1, elk_gap.1);
    }
}

#[test]
fn flowchart_start_stop_aliases_apply_effective_small_shadow() {
    for look in ["neo", "classic", "handDrawn"] {
        for (raw, enabled) in [
            ("true", true),
            ("false", false),
            ("'false'", true),
            ("1", true),
            ("0", false),
            ("''", false),
        ] {
            let svg = render_flowchart(&format!(
                "---\nconfig:\n  layout: dagre\n  look: {look}\n  theme: redux\n  themeVariables:\n    nodeShadow: {raw}\n---\nflowchart LR\nA@{{ shape: start }}\nB@{{ shape: sm-circ }}\nC@{{ shape: small-circle }}\nD@{{ shape: stop }}\nE@{{ shape: fr-circ }}\nF@{{ shape: framed-circle }}\n"
            ));
            let document = roxmltree::Document::parse(&svg).expect("valid SVG");
            let mut checked = 0;
            for node in document.descendants().filter(|node| {
                matches!(
                    node.attribute("class"),
                    Some("node default" | "rough-node default")
                )
            }) {
                let shape = node.children().find(|child| child.is_element()).unwrap();
                let shadow = shape.attribute("style").is_some_and(|style| {
                    style.contains("filter:url(#") && style.ends_with("-drop-shadow-small)")
                });
                assert_eq!(shadow, enabled && look != "handDrawn", "{look}, {enabled}");
                checked += 1;
            }
            assert_eq!(checked, 6);
        }
    }
}

use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::{Value, json};

const SOURCE: &str = "requirementDiagram\ndirection LR\nrequirement safety {\nid: R1\ntext: Stop safely\nrisk: low\nverifymethod: test\n}\nelement brake {\ntype: hardware\n}\nbrake - satisfies -> safety\n";

fn render(source: &str, algorithm: &str) -> (Value, String) {
    render_with_config(
        source,
        json!({ "layout": algorithm, "htmlLabels": false }),
        &SvgRenderOptions::default(),
    )
}

fn render_with_config(
    source: &str,
    config: Value,
    svg_options: &SvgRenderOptions,
) -> (Value, String) {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(config));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Requirement")
        .expect("detect Requirement");
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("render session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Requirement");
    let projection = artifact.layout_json().expect("layout projection");
    let svg = artifact
        .render_svg(svg_options, &SvgDebugOptions::default())
        .expect("render Requirement")
        .svg()
        .to_owned();
    (projection["layout"]["RequirementDiagram"].clone(), svg)
}

#[test]
fn requirement_registered_layout_keeps_direction_nodes_and_relationship_labels() {
    let (dagre, _) = render(SOURCE, "dagre");
    let (selected, svg) = render(SOURCE, "elk");
    assert_eq!(selected["nodes"].as_array().expect("nodes").len(), 2);
    let nodes = selected["nodes"].as_array().expect("nodes");
    let source = nodes
        .iter()
        .find(|node| node["id"] == "brake")
        .expect("source");
    let target = nodes
        .iter()
        .find(|node| node["id"] == "safety")
        .expect("target");
    assert!(source["x"].as_f64().unwrap() < target["x"].as_f64().unwrap());
    assert_eq!(selected["edges"][0]["from"], "brake");
    assert_eq!(selected["edges"][0]["to"], "safety");
    assert!(selected["edges"][0]["label"]["width"].as_f64().unwrap() > 0.0);
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    for text in ["Stop safely", "hardware", "satisfies"] {
        assert!(document.descendants().any(|node| node.is_text() && node.text().is_some_and(|value| value.contains(text))), "missing {text}");
    }
    #[cfg(feature = "layout-elk")]
    assert_ne!(selected, dagre, "ELK must execute its own provider");
    #[cfg(not(feature = "layout-elk"))]
    assert_eq!(
        selected, dagre,
        "an absent loader uses the declared lean fallback"
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn requirement_elk_self_loop_keeps_one_semantic_edge() {
    let source = format!("{SOURCE}safety - refines -> safety\n");
    let (dagre, _) = render(&source, "dagre");
    let (elk, svg) = render(&source, "elk");
    assert_eq!(dagre["nodes"].as_array().unwrap().len(), 4);
    assert_eq!(elk["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(elk["edges"].as_array().unwrap().len(), 2);
    let edge = &elk["edges"][1];
    assert_eq!(edge["id"], "safety-safety-1");
    assert_eq!(edge["from"], edge["to"]);
    assert!(edge["points"].as_array().unwrap().len() >= 3);
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let path = document
        .descendants()
        .find(|node| {
            node.attribute("data-id") == Some("safety-safety-1") && node.has_tag_name("path")
        })
        .expect("self-loop path");
    assert!(path.attribute("marker-end").is_some());
    assert!(
        path.attribute("d")
            .is_some_and(|path| path.starts_with('M'))
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn requirement_elk_packing_retains_raw_sections_and_paints_fallback() {
    for algorithm in ["elk.box", "elk.rectpacking"] {
        let (layout, svg) = render(SOURCE, algorithm);
        assert_eq!(layout["edges"][0]["points"], json!([]));
        let document = roxmltree::Document::parse(&svg).expect("valid SVG");
        let path = document
            .descendants()
            .find(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
            .expect("relationship path");
        let d = path.attribute("d").expect("path geometry");
        assert!(d.starts_with('M') && d.contains('L'), "{algorithm}: {d}");
        assert!(!svg.contains("NaN") && !svg.contains("undefined"));
        let label = document
            .descendants()
            .find(|node| node.attribute("class") == Some("edgeLabel") && node.has_tag_name("g"))
            .expect("relationship label");
        assert!(
            label
                .attribute("transform")
                .is_some_and(|value| value.starts_with("translate("))
        );
    }
}

#[test]
fn requirement_prototype_ids_keep_nodes_and_edges_in_default_and_dagre_layouts() {
    let fixture = include_str!(
        "../../../fixtures/requirement/upstream_requirement_direction_and_proto_ids_spec.mmd"
    );
    let requirement_proto = SOURCE.replace("safety", "__proto__");
    let element_proto = SOURCE.replace("brake", "__proto__");
    for config in [json!({}), json!({ "layout": "dagre" })] {
        for (source, node_ids, endpoints) in [
            (fixture, ["__proto__", "constructor"], None),
            (
                requirement_proto.as_str(),
                ["brake", "__proto__"],
                Some(("brake", "__proto__")),
            ),
            (
                element_proto.as_str(),
                ["__proto__", "safety"],
                Some(("__proto__", "safety")),
            ),
        ] {
            let (layout, svg) = render_with_config(
                source,
                config.clone(),
                &SvgRenderOptions {
                    diagram_id: Some("prototype-ids".to_string()),
                    ..SvgRenderOptions::default()
                },
            );
            let nodes = layout["nodes"].as_array().expect("nodes");
            assert_eq!(nodes.len(), 2, "{config}");
            let document = roxmltree::Document::parse(&svg).expect("valid SVG");
            let rendered_nodes: Vec<_> = document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("g")
                        && node.attribute("class").is_some_and(|classes| {
                            classes.split_whitespace().any(|class| class == "node")
                        })
                })
                .collect();
            assert_eq!(rendered_nodes.len(), 2, "{config}");
            for id in node_ids {
                assert!(nodes.iter().any(|node| node["id"] == id), "{id}: {config}");
                let dom_id = format!("prototype-ids-{id}");
                assert!(
                    rendered_nodes
                        .iter()
                        .any(|node| node.attribute("id") == Some(dom_id.as_str())),
                    "{dom_id}: {config}"
                );
            }
            let edges = layout["edges"].as_array().expect("edges");
            let rendered_edges: Vec<_> = document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("path") && node.attribute("data-edge") == Some("true")
                })
                .collect();
            assert_eq!(edges.len(), usize::from(endpoints.is_some()), "{config}");
            assert_eq!(rendered_edges.len(), edges.len(), "{config}");
            if let Some((from, to)) = endpoints {
                assert_eq!(edges[0]["from"], from);
                assert_eq!(edges[0]["to"], to);
                let edge_id = format!("{from}-{to}-0");
                assert_eq!(edges[0]["id"], edge_id);
                assert_eq!(
                    rendered_edges[0].attribute("data-id"),
                    Some(edge_id.as_str())
                );
                assert!(rendered_edges[0].attribute("marker-end").is_some());
                assert!(
                    rendered_edges[0]
                        .attribute("d")
                        .is_some_and(|path| path.starts_with('M'))
                );
            }
        }
    }
}

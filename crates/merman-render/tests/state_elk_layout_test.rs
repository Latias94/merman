use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::{Value, json};

fn render(source: &str, backend: &str) -> (Value, String) {
    let parsed = Engine::new()
        .with_site_config(MermaidConfig::from_value(json!({
            "layout": backend, "htmlLabels": false, "look": "classic",
            "state": { "minNodeWidth": 180 },
        })))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse State")
        .expect("detect State");
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare State");
    let layout = artifact.layout_json().expect("layout")["layout"]["StateDiagramV2"].clone();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State")
        .svg()
        .to_owned();
    (layout, svg)
}

#[test]
fn state_unregistered_layout_uses_dagre_and_lean_build_has_same_fallback() {
    let source = "stateDiagram-v2\nA --> B: move\nB --> B: wait\n";
    let dagre = render(source, "dagre");
    assert_eq!(dagre, render(source, "unregistered"));
    // The current drawRect intersection lands exactly on the center ray. Mermaid 11's
    // rounded-polygon adapter displaced both coordinates by a half pixel.
    let (simple, _) = render("stateDiagram-v2\nA --> B\n", "dagre");
    let nodes = simple["nodes"].as_array().unwrap();
    let a = nodes.iter().find(|node| node["id"] == "A").unwrap();
    let points = simple["edges"][0]["points"].as_array().unwrap();
    assert!((points[0]["x"].as_f64().unwrap() - a["x"].as_f64().unwrap()).abs() < 1e-9);
    assert!(
        (points[0]["y"].as_f64().unwrap()
            - a["y"].as_f64().unwrap()
            - a["height"].as_f64().unwrap() / 2.0)
            .abs()
            < 1e-9
    );
    #[cfg(not(feature = "layout-elk"))]
    assert_eq!(dagre, render(source, "elk"));
}

#[cfg(feature = "layout-elk")]
#[test]
fn state_elk_preserves_nested_concurrent_states_notes_and_native_self_loops() {
    // StateDB emits divider groups, leaf notes and native start/end nodes. The ELK
    // adapter must retain these identities rather than use Dagre cyclic-special nodes.
    let source = "stateDiagram-v2\nstate Active {\n[*] --> A\nA --> A: retry\n--\n[*] --> B\nB --> [*]\n}\nnote right of Active: composite note\nActive --> Done\n";
    let (layout, svg) = render(source, "elk");
    assert_eq!((layout.clone(), svg.clone()), render(source, "elk"));
    let nodes = layout["nodes"].as_array().expect("nodes");
    let active = nodes
        .iter()
        .find(|node| node["id"] == "Active")
        .expect("composite");
    assert_eq!(active["is_cluster"], true);
    for id in ["A", "B"] {
        let child = nodes
            .iter()
            .find(|node| node["id"] == id)
            .expect("child state");
        assert!(child["width"].as_f64().unwrap() >= 196.0);
        for (position, size) in [("x", "width"), ("y", "height")] {
            let delta =
                (child[position].as_f64().unwrap() - active[position].as_f64().unwrap()).abs();
            assert!(
                delta + child[size].as_f64().unwrap() / 2.0
                    <= active[size].as_f64().unwrap() / 2.0 + 1e-6
            );
        }
    }
    assert!(
        nodes
            .iter()
            .any(|node| node["id"].as_str().unwrap().starts_with("divider-id-"))
    );
    assert!(nodes.iter().any(|node| {
        node["id"]
            .as_str()
            .is_some_and(|id| id.contains("----parent"))
    }));
    assert!(nodes.iter().all(|node| {
        let Some(id) = node["id"].as_str() else {
            return true;
        };
        let Some((prefix, suffix)) = id.rsplit_once("---") else {
            return true;
        };
        if suffix != "1" && suffix != "2" {
            return true;
        }
        prefix
            .rsplit_once("---")
            .is_none_or(|(left, right)| left != right)
    }));
    let edges = layout["edges"].as_array().expect("edges");
    assert!(edges.iter().any(|edge| edge["from"] == "A"
        && edge["to"] == "A"
        && !edge["points"].as_array().unwrap().is_empty()));
    assert!(
        edges
            .iter()
            .all(|edge| !edge["id"].as_str().unwrap().contains("cyclic-special"))
    );
    assert!(svg.contains("note-cluster"));
}

#[cfg(feature = "layout-elk")]
#[test]
fn state_elk_packing_keeps_missing_sections_but_paints_edges_and_labels() {
    let (layout, svg) = render(
        "stateDiagram-v2\n[*] --> A\nA --> B: next\nB --> [*]\n",
        "elk.box",
    );
    let edges = layout["edges"].as_array().expect("edges");
    assert_eq!(edges.len(), 3);
    assert!(edges.iter().all(|edge| edge["points"] == json!([])));
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let paths: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("data-edge") == Some("true"))
        .collect();
    assert_eq!(paths.len(), 3);
    assert!(paths.iter().all(|node| {
        node.attribute("d")
            .is_some_and(|d| d.contains('L') && !d.contains("NaN"))
    }));
    let label = edges
        .iter()
        .find(|edge| edge["label"].is_object())
        .expect("transition label");
    assert!(label["label"]["x"].as_f64().unwrap().is_finite());
    assert!(svg.contains("next"));
}

#[cfg(feature = "layout-elk")]
#[test]
fn state_elk_cross_concurrency_edges_remain_orthogonal_through_parent_boundaries() {
    let source = include_str!(
        "../../../fixtures/state/stress_state_concurrency_with_external_edges_051.mmd"
    );
    let (layout, _) = render(source, "elk");
    let nodes = layout["nodes"].as_array().unwrap();
    let edges = layout["edges"].as_array().unwrap();
    for source in ["A2", "B2"] {
        let edge = edges
            .iter()
            .find(|edge| edge["from"] == source && edge["to"] == "End")
            .unwrap();
        let points = edge["points"].as_array().unwrap();
        assert!(points.len() >= 4);
        for segment in points.windows(2) {
            let dx = segment[0]["x"].as_f64().unwrap() - segment[1]["x"].as_f64().unwrap();
            let dy = segment[0]["y"].as_f64().unwrap() - segment[1]["y"].as_f64().unwrap();
            assert!(
                dx.abs() < 1e-8 || dy.abs() < 1e-8,
                "cross-container route has a diagonal: {segment:?}"
            );
        }
        let node = nodes.iter().find(|node| node["id"] == source).unwrap();
        for point in points.iter().take(3) {
            assert!(
                (point["x"].as_f64().unwrap() - node["x"].as_f64().unwrap()).abs() < 1e-8,
                "the compound route must keep its port column across both container boundaries"
            );
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn state_elk_fork_join_uses_painted_bounds_after_measurement() {
    for (direction, expected) in [("TB", (70.0, 10.0)), ("LR", (10.0, 70.0))] {
        let source = format!(
            "stateDiagram-v2\ndirection {direction}\nstate F <<fork>>\nstate J <<join>>\nF --> A\nA --> J\n"
        );
        for (backend, padding) in [("elk", 0.0), ("dagre", 4.0)] {
            let (layout, _) = render(&source, backend);
            for id in ["F", "J"] {
                let node = layout["nodes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|node| node["id"] == id)
                    .unwrap();
                assert_eq!(
                    node["width"],
                    expected.0 + padding,
                    "{backend} {direction} {id}"
                );
                assert_eq!(
                    node["height"],
                    expected.1 + padding,
                    "{backend} {direction} {id}"
                );
            }
        }
    }
}

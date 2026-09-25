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
fn state_elk_honors_default_and_explicit_model_order() {
    let source =
        include_str!("../../../fixtures/state/stress_state_cross_composite_transitions_007.mmd");
    let (default_layout, _) = render(source, "elk");
    let explicit_source = |strategy: &str| {
        format!("---\nconfig:\n  elk:\n    considerModelOrder: {strategy}\n---\n{source}")
    };
    let (model_order_layout, _) = render(&explicit_source("NODES_AND_EDGES"), "elk");
    let (unordered_layout, _) = render(&explicit_source("NONE"), "elk");

    // Mermaid 12's defaultConfig and StateDB preserve the shared model-order setting.
    // This compound graph changes its routes when the caller opts out, so an adapter
    // that silently forces either strategy cannot satisfy both assertions.
    assert_eq!(default_layout, model_order_layout);
    assert_ne!(default_layout["edges"], unordered_layout["edges"]);
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

#[cfg(feature = "layout-elk")]
#[test]
fn state_elk_subpixel_label_width_changes_route_topology() {
    use merman_render::environment::{
        MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity,
    };
    use merman_render::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode};
    use std::sync::Arc;

    struct FixtureMeasurements {
        selection_width: f64,
    }

    impl TextMeasurer for FixtureMeasurements {
        fn measure(&self, text: &str, _style: &TextStyle) -> TextMetrics {
            // Attribution experiment: widths and heights are the foreignObject bounds in
            // upstream_pkgtests_statediagram_spec_015.svg, not a production font profile.
            let width = match text {
                "Configuring" => 71.609375,
                "NewValueSelection" => self.selection_width,
                "NewValuePreview" => 120.0,
                "EvNewValue" => 79.125,
                "EvNewValueRejected" => 134.375,
                "EvNewValueSaved1" => 126.609375,
                "" => 0.0,
                other => panic!("unexpected measurement in the controlled fixture: {other:?}"),
            };
            TextMetrics {
                width,
                height: if text.is_empty() { 0.0 } else { 21.0 },
                line_count: usize::from(!text.is_empty()),
            }
        }

        fn measure_wrapped(
            &self,
            text: &str,
            style: &TextStyle,
            _max_width: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> TextMetrics {
            self.measure(text, style)
        }
    }

    let controlled_layout = |selection_width| {
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.state-015-browser-bounds").unwrap(),
                "fixture",
            )
            .unwrap(),
            Arc::new(FixtureMeasurements { selection_width }),
        );
        let environment = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
        let parsed = Engine::new()
            .with_site_config(MermaidConfig::from_value(json!({
                "layout": "elk", "htmlLabels": true,
            })))
            .parse_diagram_for_render_model_sync(
                include_str!("../../../fixtures/state/upstream_pkgtests_statediagram_spec_015.mmd"),
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = family::prepare(
            parsed,
            &LayoutOptions::headless_svg_defaults(),
            environment.begin_session().unwrap(),
        )
        .unwrap();
        artifact.layout_json().unwrap()["layout"]["StateDiagramV2"].clone()
    };

    for (selection_width, expected_points) in [(120.0, 2), (120.375, 4)] {
        let layout = controlled_layout(selection_width);
        let nodes = layout["nodes"].as_array().unwrap();
        let selection = nodes
            .iter()
            .find(|node| node["id"] == "NewValueSelection")
            .unwrap();
        let preview = nodes
            .iter()
            .find(|node| node["id"] == "NewValuePreview")
            .unwrap();
        assert_eq!(selection["width"], selection_width + 16.0);
        assert_eq!(preview["width"], 136.0);
        let edge = layout["edges"]
            .as_array()
            .unwrap()
            .iter()
            .find(|edge| edge["from"] == "NewValuePreview" && edge["to"] == "NewValueSelection")
            .unwrap();
        let points = edge["points"].as_array().unwrap();
        assert_eq!(
            points.len(),
            expected_points,
            "{selection_width}: {points:?}"
        );
        if expected_points == 4 {
            // The pinned SVG has x=250.0625 and x=250.109375 at these ports. ELK
            // retains their 0.046875px offset because its routing tolerance is 0.001.
            let x = |i: usize| points[i]["x"].as_f64().unwrap();
            let y = |i: usize| points[i]["y"].as_f64().unwrap();
            assert!((x(2) - x(1) - 0.046875).abs() < 1e-9, "{points:?}");
            assert_eq!(x(0), x(1));
            assert_eq!(y(1), y(2));
            assert_eq!(x(2), x(3));
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn state_elk_compound_routes_with_controlled_browser_measurements() {
    use merman_render::environment::{
        MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity,
    };
    use merman_render::text::{
        DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode,
    };
    use std::sync::Arc;

    struct FixtureMeasurements;

    impl TextMeasurer for FixtureMeasurements {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            // The pinned 040 fixture uses the same browser bounds for every group title,
            // transition label, and minimum-width child label, respectively.
            let width = if text.starts_with("State") && text.ends_with("_____________") {
                154.984375
            } else if text.starts_with("Transition") {
                120.515625
            } else if matches!(
                text,
                "c0" | "c1" | "c2" | "c3" | "c4" | "c5" | "c6" | "c7" | "c9"
            ) {
                120.0
            } else if text == "Multiple Transitions" || text.is_empty() {
                // The diagram title is outside the routed graph.
                return DeterministicTextMeasurer::default().measure(text, style);
            } else {
                panic!("unexpected measurement in the controlled fixture: {text:?}");
            };
            TextMetrics {
                width,
                height: 24.0,
                line_count: 1,
            }
        }

        fn measure_wrapped(
            &self,
            text: &str,
            style: &TextStyle,
            _max_width: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> TextMetrics {
            self.measure(text, style)
        }
    }

    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.state-040-browser-bounds").unwrap(),
            "fixture",
        )
        .unwrap(),
        Arc::new(FixtureMeasurements),
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    let parsed = Engine::new()
        .with_site_config(MermaidConfig::from_value(json!({ "layout": "elk", "htmlLabels": true })))
        .parse_diagram_for_render_model_sync(
            include_str!("../../../fixtures/state/upstream_cypress_statediagram_v2_spec_should_render_edge_labels_correctly_with_multiple_transitions_040.mmd"),
            ParseOptions::strict(),
        ).unwrap().unwrap();
    let artifact = family::prepare(
        parsed,
        &LayoutOptions::headless_svg_defaults(),
        environment.begin_session().unwrap(),
    )
    .unwrap();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let actual = roxmltree::Document::parse(rendered.svg()).unwrap();
    let upstream = roxmltree::Document::parse(include_str!(
        "../../../fixtures/upstream-svgs/state/upstream_cypress_statediagram_v2_spec_should_render_edge_labels_correctly_with_multiple_transitions_040.svg"
    )).unwrap();
    for id in ["edge3", "edge4", "edge7"] {
        let commands = |document: &roxmltree::Document<'_>| {
            document
                .descendants()
                .find(|node| node.attribute("data-id") == Some(id) && node.has_tag_name("path"))
                .unwrap()
                .attribute("d")
                .unwrap()
                .chars()
                .filter(char::is_ascii_uppercase)
                .collect::<String>()
        };
        // Compare only the previously failing route topology. A match here attributes
        // those extra corners to measurement inputs without relaxing the DOM comparator.
        assert_eq!(commands(&actual), commands(&upstream), "{id}");
    }
}

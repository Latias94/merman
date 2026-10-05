#![cfg(feature = "layout-elk")]

use merman_core::{Engine, ParseOptions};
use merman_layout_elk as elk;
use merman_render::flowchart::elk::build_flowchart_elk_graph;
use merman_render::text::DeterministicTextMeasurer;
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
struct BrowserMeasurements {
    schema_version: u32,
    entries: Vec<Fixture>,
}

#[derive(Deserialize)]
struct Fixture {
    fixture: String,
    nodes: Vec<MeasuredNode>,
    edges: Vec<MeasuredEdge>,
}

#[derive(Deserialize)]
struct MeasuredNode {
    id: String,
    size: Option<[f64; 2]>,
    label_size: Option<[f64; 2]>,
}

#[derive(Deserialize)]
struct MeasuredEdge {
    id: String,
    source: String,
    target: String,
    label_size: Option<[f64; 2]>,
    points: Vec<[f64; 2]>,
}

fn label([width, height]: [f64; 2]) -> elk::Label {
    elk::Label { width, height }
}

#[test]
fn flowchart_elk_routes_with_captured_browser_dimensions() {
    let measurements: BrowserMeasurements = serde_json::from_str(include_str!(
        "../../../fixtures/_verification/flowchart-elk-browser-measurements.json"
    ))
    .unwrap();
    assert_eq!(measurements.schema_version, 1);
    assert_eq!(measurements.entries.len(), 45);
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/flowchart");

    for fixture in measurements.entries {
        let name = &fixture.fixture;
        let source = std::fs::read_to_string(fixtures.join(format!("{name}.mmd"))).unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
            .unwrap()
            .unwrap();
        let mut graph =
            build_flowchart_elk_graph(&parsed, &DeterministicTextMeasurer::default(), None)
                .unwrap();
        assert_eq!(graph.nodes.len(), fixture.nodes.len(), "{name}: node count");
        assert_eq!(graph.edges.len(), fixture.edges.len(), "{name}: edge count");

        // Only replace measurements captured before the upstream ELK invocation. Keep the
        // parser, family adapter, hierarchy/options, and pure-Rust provider under test.
        for measured in &fixture.nodes {
            let node = graph
                .nodes
                .iter_mut()
                .find(|node| node.id == measured.id)
                .unwrap();
            if let Some([width, height]) = measured.size {
                node.width = width;
                node.height = height;
            }
            if let Some(size) = measured.label_size {
                node.label = Some(label(size));
            }
        }
        for measured in &fixture.edges {
            let edge = graph
                .edges
                .iter_mut()
                .find(|edge| edge.id == measured.id)
                .unwrap();
            assert_eq!(edge.source, measured.source, "{name}/{}", edge.id);
            assert_eq!(edge.target, measured.target, "{name}/{}", edge.id);
            // ELK ignores an empty browser label. The Rust adapter represents it as None.
            edge.label = measured.label_size.map(label);
        }

        let layout = elk::layout(&graph).unwrap();
        assert_eq!(
            layout.edges.len(),
            fixture.edges.len(),
            "{name}: routed edges"
        );
        for expected in &fixture.edges {
            let actual = layout
                .edges
                .iter()
                .find(|edge| edge.id == expected.id)
                .unwrap();
            assert_eq!(
                actual.points.len(),
                expected.points.len(),
                "{name}/{}: point count",
                expected.id
            );
            for (actual, [x, y]) in actual.points.iter().zip(&expected.points) {
                assert!(
                    (actual.x - x).abs() <= 1e-8 && (actual.y - y).abs() <= 1e-8,
                    "{name}/{}: expected ({x}, {y}), got {actual:?}",
                    expected.id
                );
            }
        }
    }
}

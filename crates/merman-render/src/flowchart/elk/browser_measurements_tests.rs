use super::build_flowchart_elk_graph;
use crate::text::DeterministicTextMeasurer;
use merman_core::{MermaidEngine, ParseOptions};
use merman_layout_elk as elk;
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
    // Provider output: center x/y and width/height, before endpoint projection.
    provider_geometry: Option<[f64; 4]>,
}

#[derive(Deserialize)]
struct MeasuredEdge {
    id: String,
    source: String,
    target: String,
    label_size: Option<[f64; 2]>,
    points: Vec<[f64; 2]>,
    // Provider center-label rectangles: top-left x/y and width/height.
    provider_labels: Option<Vec<[f64; 4]>>,
}

fn label([width, height]: [f64; 2]) -> elk::Label {
    elk::Label { width, height }
}

#[test]
fn flowchart_elk_routes_with_captured_browser_dimensions() {
    let measurements: BrowserMeasurements = serde_json::from_str(include_str!(
        "../../../../../fixtures/_verification/flowchart-elk-browser-measurements.json"
    ))
    .unwrap();
    assert_eq!(measurements.schema_version, 1);
    assert_eq!(measurements.entries.len(), 46);
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/flowchart");

    for fixture in measurements.entries {
        let name = &fixture.fixture;
        let source = std::fs::read_to_string(fixtures.join(format!("{name}.mmd"))).unwrap();
        let parsed = MermaidEngine::new()
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
        if fixture
            .nodes
            .iter()
            .any(|node| node.provider_geometry.is_some())
        {
            assert_eq!(
                layout.nodes.len(),
                fixture.nodes.len(),
                "{name}: placed nodes"
            );
        }
        for expected in &fixture.nodes {
            let Some(geometry) = expected.provider_geometry else {
                continue;
            };
            let actual = layout
                .nodes
                .iter()
                .find(|node| node.id == expected.id)
                .unwrap();
            let observed = [actual.x, actual.y, actual.width, actual.height];
            assert!(
                observed
                    .iter()
                    .zip(geometry)
                    .all(|(actual, expected)| (actual - expected).abs() <= 1e-8),
                "{name}/{}: expected node {geometry:?}, got {observed:?}",
                expected.id
            );
        }
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
            if let Some(labels) = &expected.provider_labels {
                assert_eq!(
                    actual.labels.len(),
                    labels.len(),
                    "{name}/{}: provider label count",
                    expected.id
                );
                for (actual, expected_label) in actual.labels.iter().zip(labels) {
                    assert_eq!(
                        actual.terminal, None,
                        "{name}/{}: provider center label",
                        expected.id
                    );
                    let observed = [actual.x, actual.y, actual.width, actual.height];
                    assert!(
                        observed
                            .iter()
                            .zip(expected_label)
                            .all(|(actual, expected)| (actual - expected).abs() <= 1e-8),
                        "{name}/{}: expected label {expected_label:?}, got {observed:?}",
                        expected.id
                    );
                }
            }
        }
    }
}

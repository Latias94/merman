#![cfg(feature = "diagram-flowchart")]

mod support;

use merman_ascii::{
    AsciiError, AsciiGraphPresentation, AsciiLayoutProfile, AsciiProjection, AsciiRenderOptions,
    AsciiRenderer, AsciiResourceLimitId, AsciiResourcePolicy, AsciiViewportPolicy, OverflowPolicy,
};
use merman_model::families::flowchart::{FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility};
use merman_model::presentation::RenderSemanticModel;
use sha2::{Digest, Sha256};
use support::{parse_model, render_model_with_resources};

const ORIGINAL: &[u8] =
    include_bytes!("testdata/local-semantic/flowchart/scene_planning/dense-work-budget.mmd");
const REDUCED: &[u8] =
    include_bytes!("testdata/local-semantic/flowchart/scene_planning/dense-minimized.mmd");

fn source(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).expect("the frozen sources are UTF-8")
}

fn cycle_ladder_pairs(node_count: usize) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for from in 0..node_count {
        pairs.push((from, from / 5 * 5 + (from + 1) % 5));
        if from + 5 < node_count {
            pairs.push((from, from + 5));
        }
    }
    pairs
}

fn assert_direct_facts(bytes: &[u8], node_count: usize, pairs: &[(usize, usize)]) {
    let RenderSemanticModel::Flowchart(model) = parse_model(source(bytes)) else {
        panic!("the frozen input must remain a flowchart");
    };
    // FlowDB normalizes authored TD to TB; neither spelling admits a rotation.
    assert_eq!(model.direction.as_deref(), Some("TB"));
    assert_eq!(model.nodes.len(), node_count);
    assert!(model.subgraphs.is_empty());
    for (index, node) in model.nodes.iter().enumerate() {
        assert_eq!(node.id, format!("C{index}"));
        assert_eq!(node.label.as_deref(), Some(node.id.as_str()));
        assert_eq!(model.node_label_source(node), Some(node.id.as_str()));
        assert!(!node.is_subgraph_anchor());
    }
    assert_eq!(model.edges.len(), pairs.len());
    for (edge, &(from, to)) in model.edges.iter().zip(pairs) {
        assert_eq!(edge.from, format!("C{from}"));
        assert_eq!(edge.to, format!("C{to}"));
        assert!(edge.label.as_deref().unwrap_or_default().is_empty());
        assert_eq!(edge.start_marker, FlowEdgeMarker::None);
        assert_eq!(edge.end_marker, FlowEdgeMarker::Point);
        assert_eq!(edge.stroke_kind, FlowEdgeStroke::Normal);
        assert_eq!(edge.visibility, FlowEdgeVisibility::Visible);
    }
}

#[test]
fn dense_sources_keep_the_exact_diagnostic_bytes() {
    for (bytes, expected) in [
        (
            ORIGINAL,
            "c39460ae177007d7d0bab13d5fc9ce37a9178b876d1d0655ac7f1bf4675bbc0f",
        ),
        (
            REDUCED,
            "51e61fbd87adea209d4314f64baa39a6030ed84475c22b47b7db92f180425ad7",
        ),
    ] {
        let actual: String = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, expected);
    }
}

#[test]
fn original_dense_source_keeps_all_twenty_cycles_and_ninety_five_forward_links() {
    let pairs = cycle_ladder_pairs(100);
    assert_eq!(pairs.len(), 195);
    assert_direct_facts(ORIGINAL, 100, &pairs);
}

#[test]
fn reduced_dense_source_keeps_the_original_order_and_only_the_recorded_omissions() {
    let mut pairs = cycle_ladder_pairs(20);
    pairs.retain(|pair| !matches!(*pair, (0, 5) | (1, 6)));
    assert_eq!(pairs.len(), 33);
    assert_direct_facts(REDUCED, 20, &pairs);
}

#[test]
fn reduced_strict_outcome_keeps_planning_diagnostics_distinct_from_resource_errors() {
    let model = parse_model(source(REDUCED));
    let options = AsciiRenderOptions::unicode().with_layout_profile(AsciiLayoutProfile::Canonical);
    match render_model_with_resources(&model, &options, AsciiResourcePolicy::unbounded()) {
        Ok(diagram) => {
            let labels: std::collections::BTreeSet<_> = diagram
                .split(|character: char| !character.is_ascii_alphanumeric())
                .collect();
            for index in 0..20 {
                assert!(labels.contains(format!("C{index}").as_str()));
            }
        }
        Err(AsciiError::RoutingSearchExhausted {
            diagram_type,
            edge_index,
        }) => {
            assert_eq!(diagram_type, "flowchart");
            assert!(edge_index < 33);
        }
        Err(AsciiError::PortCapacityExceeded {
            required,
            available,
            ..
        }) => assert!(required > available),
        Err(error) => panic!("strict planning must preserve its typed outcome: {error}"),
    }
}

#[test]
fn dense_work_exhaustion_stays_a_resource_error() {
    let resources = AsciiResourcePolicy::unbounded()
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
        .expect("one work unit is an admitted resource limit");
    for bytes in [ORIGINAL, REDUCED] {
        let model = parse_model(source(bytes));
        let error = render_model_with_resources(&model, &AsciiRenderOptions::unicode(), resources)
            .expect_err("the dense model cannot be planned in one work unit");
        assert!(
            matches!(
                error,
                AsciiError::ResourceLimitExceeded(ref details)
                    if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
            ),
            "resource exhaustion must not become a content miss: {error:?}"
        );
    }
}

#[test]
fn both_dense_graphs_have_complete_adaptive_documents_under_trusted_native_limits() {
    use merman_model::{OperationControl, resources::ResourceProfile, runtime::RuntimePolicy};
    for (bytes, node_count, edge_count) in [(REDUCED, 20, 33), (ORIGINAL, 100, 195)] {
        let parsed = merman_core::SemanticParser::new()
            .parse_diagram_sync(source(bytes))
            .unwrap();
        let context = RuntimePolicy::deterministic().begin_operation().unwrap();
        let output = AsciiRenderer::new(
            AsciiRenderOptions::ascii().with_graph_presentation(AsciiGraphPresentation::Adaptive),
        )
        .unwrap()
        .render_parsed_report(
            &parsed,
            AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Fallback),
            &OperationControl::new(),
            &context,
            AsciiResourcePolicy::for_profile(ResourceProfile::TrustedNative),
        )
        .expect("adaptive rendering must preserve the dense graph under unchanged native ceilings");
        assert!(output.emitted_extent.width <= 80);
        assert!(!output.emitted_overflowed);
        if output.projection == AsciiProjection::StructuredText {
            let text: String = output
                .text
                .lines()
                .map(|line| line.strip_prefix("  ").unwrap_or(line))
                .collect();
            for node in 0..node_count {
                assert!(
                    text.contains(&format!("\"C{node}\"")),
                    "missing full label C{node}"
                );
            }
            for edge in 1..=edge_count {
                assert!(
                    text.contains(&format!("E{edge}:")),
                    "missing relationship E{edge}"
                );
            }
            assert_eq!(text.matches(" --> ").count(), edge_count);
            let mut pairs = cycle_ladder_pairs(node_count);
            if node_count == 20 {
                pairs.retain(|pair| !matches!(*pair, (0, 5) | (1, 6)));
            }
            for (index, (from, to)) in pairs.into_iter().enumerate() {
                let prefix = format!("E{}: ", index + 1);
                let row = output
                    .text
                    .lines()
                    .find_map(|row| row.strip_prefix(&prefix))
                    .expect("each relationship starts a complete identifiable record");
                let (source, target) = row
                    .split_once(" --> ")
                    .expect("the edge keeps its authored marker direction");
                assert_eq!(
                    source.split_whitespace().next(),
                    Some(format!("N{}", from + 1).as_str())
                );
                assert_eq!(
                    target.split_whitespace().next(),
                    Some(format!("N{}", to + 1).as_str())
                );
            }
        }
    }
}

#![cfg(feature = "diagram-flowchart")]

mod support;

use merman_ascii::{
    AsciiError, AsciiGraphPresentation, AsciiGraphRoutingSpace, AsciiLayoutProfile,
    AsciiProjection, AsciiRenderOptions, AsciiRenderer, AsciiResourceLimitId, AsciiResourcePolicy,
    AsciiViewportPolicy, OverflowPolicy,
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
fn dense_strict_sources_have_complete_spatial_primaries_under_trusted_native_limits() {
    use merman_model::{OperationControl, resources::ResourceProfile, runtime::RuntimePolicy};
    for (bytes, node_count, max_height) in [(REDUCED, 20, 120), (ORIGINAL, 100, 300)] {
        let parsed = merman_core::SemanticParser::new()
            .parse_diagram_sync(source(bytes))
            .unwrap();
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            let context = RuntimePolicy::deterministic().begin_operation().unwrap();
            let output =
                AsciiRenderer::new(options.with_layout_profile(AsciiLayoutProfile::Canonical))
                    .unwrap()
                    .render_parsed_report(
                        &parsed,
                        AsciiViewportPolicy::with_max_width(120).overflow(OverflowPolicy::Allow),
                        &OperationControl::new(),
                        &context,
                        AsciiResourcePolicy::for_profile(ResourceProfile::TrustedNative),
                    )
                    .expect("the frozen cycle ladders must produce complete spatial primaries");
            assert_eq!(output.projection, AsciiProjection::Diagrammatic);
            assert_eq!(
                output.requested_graph_presentation,
                AsciiGraphPresentation::Strict
            );
            assert!(!output.fallback.attempted);
            assert!(!output.emitted_overflowed);
            assert_eq!(output.primary_extent, Some(output.emitted_extent));
            assert!(output.emitted_extent.width <= 120);
            assert!(
                output.emitted_extent.height <= max_height,
                "the dense primary must fit its spatial acceptance bound: {:?}",
                output.emitted_extent
            );
            let labels: std::collections::BTreeSet<_> = output
                .text
                .split(|character: char| !character.is_ascii_alphanumeric())
                .collect();
            for index in 0..node_count {
                assert!(labels.contains(format!("C{index}").as_str()));
            }
            assert!(!output.text.contains("family:") && !output.text.contains("Keys:"));
        }
    }
}

#[test]
fn issue_370_fixed_routing_space_preserves_the_requested_terminal_height() {
    use merman_model::{OperationControl, runtime::RuntimePolicy};

    let parsed = merman_core::SemanticParser::new()
        .parse_diagram_sync(include_str!(
            "testdata/local-semantic/flowchart/scene_planning/issue_370_terminal_height.mmd"
        ))
        .unwrap();
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    let options = AsciiRenderOptions::unicode()
        .with_layout_profile(AsciiLayoutProfile::Auto)
        .with_node_padding_y(0)
        .with_graph_padding_y(2);
    let render = |options| {
        AsciiRenderer::new(options)
            .unwrap()
            .render_parsed_report(
                &parsed,
                AsciiViewportPolicy::with_max_width(200).overflow(OverflowPolicy::Allow),
                &OperationControl::new(),
                &context,
                AsciiResourcePolicy::default(),
            )
            .unwrap()
    };

    let default = render(options);
    assert_eq!(default.emitted_extent.height, 48);
    let fixed = render(options.with_graph_routing_space(AsciiGraphRoutingSpace::Fixed));
    assert_eq!(fixed.projection, AsciiProjection::Diagrammatic);
    assert_eq!(fixed.layout_profile, Some(AsciiLayoutProfile::Canonical));
    assert!(!fixed.compact_attempted);
    assert!(!fixed.fallback.attempted);
    assert!(fixed.emitted_extent.height <= 33, "{}", fixed.text);
    assert!(fixed.emitted_extent.width <= 200, "{}", fixed.text);
    assert_eq!(fixed.text.lines().count(), fixed.emitted_extent.height);
    for label in [
        "User",
        "Interface",
        "Orchestrator",
        "Controller",
        "Model provider",
        "Model API",
        "Tool runner",
        "Worker",
        "Sandbox",
        "Events",
        "prompt",
        "JSON-RPC",
        "request",
        "stream",
        "tool call",
        "events",
        "broadcast",
    ] {
        assert_eq!(fixed.text.matches(label).count(), 1, "{}", fixed.text);
    }
    assert!(fixed.text.contains("╰ Events ╯"), "{}", fixed.text);
}

#[test]
fn fixed_routing_space_still_reports_resource_exhaustion_and_cancellation() {
    use merman_model::{CancelReason, OperationControl, runtime::RuntimePolicy};

    let parsed = merman_core::SemanticParser::new()
        .parse_diagram_sync(include_str!(
            "testdata/local-semantic/flowchart/scene_planning/issue_370_terminal_height.mmd"
        ))
        .unwrap();
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    let viewport = AsciiViewportPolicy::with_max_width(200).overflow(OverflowPolicy::Fallback);
    let renderer = AsciiRenderer::new(
        AsciiRenderOptions::unicode()
            .with_graph_padding_y(2)
            .with_graph_routing_space(AsciiGraphRoutingSpace::Fixed),
    )
    .unwrap();
    let limits = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 10_000)
        .unwrap();
    assert!(matches!(
        renderer.render_parsed_report(&parsed, viewport, &OperationControl::new(), &context, limits),
        Err(AsciiError::ResourceLimitExceeded(details))
            if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
    ));
    let control = OperationControl::new();
    control.cancel();
    assert!(matches!(
        renderer.render_parsed_report(&parsed, viewport, &control, &context, limits),
        Err(AsciiError::Cancelled(details)) if details.reason == CancelReason::Requested
    ));
}

#[test]
fn fixed_routing_space_rejects_subgraphs_and_adaptive_presentation() {
    let options =
        AsciiRenderOptions::ascii().with_graph_routing_space(AsciiGraphRoutingSpace::Fixed);
    assert!(matches!(
        AsciiRenderer::new(options.with_graph_presentation(AsciiGraphPresentation::Adaptive)),
        Err(AsciiError::InvalidOption {
            field: "graph_routing_space",
            ..
        })
    ));
    let subgraph = parse_model("flowchart TD\nsubgraph Group\nA --> B\nend");
    assert!(matches!(
        render_model_with_resources(&subgraph, &options, AsciiResourcePolicy::default()),
        Err(AsciiError::InvalidOption {
            field: "graph_routing_space",
            ..
        })
    ));
}

#[test]
fn capacity_recovery_keeps_narrow_cycles_and_all_edge_labels() {
    use merman_model::{OperationControl, resources::ResourceProfile, runtime::RuntimePolicy};

    for input in [
        include_str!(
            "testdata/local-semantic/flowchart/scene_planning/labelled-cycle-capacity.mmd"
        ),
        include_str!("testdata/local-semantic/flowchart/scene_planning/narrow-cycle-capacity.mmd"),
    ] {
        let parsed = merman_core::SemanticParser::new()
            .parse_diagram_sync(input)
            .unwrap();
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            let context = RuntimePolicy::deterministic().begin_operation().unwrap();
            let limits = AsciiResourcePolicy::for_profile(ResourceProfile::TrustedNative);
            let fixed =
                AsciiRenderer::new(options.with_graph_routing_space(AsciiGraphRoutingSpace::Fixed))
                    .unwrap()
                    .render_parsed_report(
                        &parsed,
                        AsciiViewportPolicy::with_max_width(50).overflow(OverflowPolicy::Fallback),
                        &OperationControl::new(),
                        &context,
                        limits,
                    );
            assert!(
                matches!(
                    fixed,
                    Err(AsciiError::RoutingSearchExhausted { .. }
                        | AsciiError::PortCapacityExceeded { .. })
                ),
                "the fixed request must surface its routing failure: {fixed:?}"
            );
            let output =
                AsciiRenderer::new(options.with_layout_profile(AsciiLayoutProfile::Canonical))
                    .unwrap()
                    .render_parsed_report(
                        &parsed,
                        AsciiViewportPolicy::with_max_width(50).overflow(OverflowPolicy::Error),
                        &OperationControl::new(),
                        &context,
                        limits,
                    )
                    .expect("component recovery must preserve existing 50-column spatial results");
            assert_eq!(output.projection, AsciiProjection::Diagrammatic);
            assert!(!output.fallback.attempted);
            assert!(!output.emitted_overflowed);
            assert!(output.emitted_extent.width <= 50);
            for label in (0..8).map(|index| format!("N{index}")) {
                assert!(output.text.contains(&label), "missing node {label}");
            }
            if input.contains("label1") {
                for label in ["label1", "label3", "label5", "label7", "label9", "label11"] {
                    assert!(output.text.contains(label), "missing edge label {label}");
                }
            }
        }
    }
}

#[test]
fn dense_recovery_keeps_reverse_root_directions_and_readable_labels() {
    for (direction, horizontal, marker) in [("BT", false, '▲'), ("RL", true, '◄')] {
        let input = source(REDUCED).replacen("flowchart TD", &format!("flowchart {direction}"), 1);
        let model = parse_model(&input);
        let text = render_model_with_resources(
            &model,
            &AsciiRenderOptions::unicode(),
            AsciiResourcePolicy::unbounded(),
        )
        .expect("the reverse view must preserve the complete cyclic graph");
        let position = |label: &str| {
            text.lines()
                .enumerate()
                .find_map(|(row, line)| line.find(label).map(|column| (column, row)))
                .expect("each full node label must remain readable")
        };
        let first = position(" C0 ");
        let last = position(" C15 ");
        assert!(
            if horizontal {
                first.0 > last.0
            } else {
                first.1 > last.1
            },
            "{direction}\n{text}"
        );
        assert!(
            text.contains(marker),
            "the main-axis arrows must point {direction}\n{text}"
        );
        let labels: std::collections::BTreeSet<_> = text
            .split(|character: char| !character.is_ascii_alphanumeric())
            .collect();
        for index in 0..20 {
            assert!(labels.contains(format!("C{index}").as_str()));
        }
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

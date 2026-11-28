use super::super::{
    Endpoint, Marker, MeasuredEdge, MeasuredGroup, MeasuredNode, RoutingStyle, TitlePosition,
};
use super::*;
use crate::resources::{RenderResourcePolicy, ResourceLimitId};

fn node(parent: Option<u32>) -> MeasuredNode {
    MeasuredNode {
        size: Size {
            width: 60.0,
            height: 40.0,
        },
        #[cfg(feature = "shoal-bridge")]
        contour: merman_layout_shoal::ShapeContourHint::default(),
        seq: 0,
        parent,
        port_span: None,
        perpendicular_port_span: None,
        anchor_y: None,
    }
}

fn edge(source: Endpoint, target: Endpoint) -> MeasuredEdge {
    MeasuredEdge {
        source,
        target,
        min_rank_span: 1,
        weight: 1.0,
        label: None,
        start_label: None,
        end_label: None,
        start_marker: Marker::default(),
        end_marker: Marker::default(),
    }
}

fn group() -> MeasuredGroup {
    MeasuredGroup {
        seq: 0,
        parent: None,
        direction: None,
        padding: kurbo::Insets::new(8.0, 11.0, 17.0, 13.0),
        min_size: Size::default(),
        title: None,
        title_position: TitlePosition::TopCenter,
    }
}

fn measured() -> MeasuredGraph {
    let n = Endpoint::Node;
    let g = Endpoint::Group;
    MeasuredGraph {
        direction: Direction::Down,
        routing: RoutingStyle::Orthogonal,
        node_spacing: 50.0,
        rank_spacing: 50.0,
        component_spacing: 50.0,
        inherit_crossed_group_direction: true,
        nodes: vec![node(Some(0)), node(None), node(Some(0))],
        groups: vec![group()],
        edges: vec![
            edge(g(0), n(2)),
            edge(n(0), n(1)),
            edge(n(2), n(0)),
            edge(n(2), n(2)),
        ],
    }
}

#[test]
fn shared_graph_keeps_occurrences_ports_loops_and_deferred_edges() {
    let input = measured();
    let meter = OperationWorkMeter::new(RenderResourcePolicy::trusted_native());
    let result = layout(&input, Options::default(), &meter).unwrap();
    assert_eq!(result.edge_order, [1, 2, 3, 0]);
    assert_eq!(result.graph.edges.len(), input.edges.len());
    assert_eq!(result.graph.nodes.len(), 3);
    assert_eq!(result.graph.groups.len(), 1);
    assert_eq!(result.graph.edges[3].points.len(), 4);
    let cross = &result.graph.edges[1];
    assert_eq!(
        (cross.source, cross.target),
        (Endpoint::Node(0), Endpoint::Node(1))
    );
    let [stitch] = result.graph.border_stitches.as_slice() else {
        panic!("one group crossing")
    };
    assert_eq!((stitch.edge, stitch.group), (1, 0));
    let group = result.graph.groups[0].rect;
    let knot = cross.points[stitch.point];
    let (normal, border) = match stitch.side {
        Side::Top => (knot.y, group.y),
        Side::Right => (knot.x, group.x + group.width),
        Side::Bottom => (knot.y, group.y + group.height),
        Side::Left => (knot.x, group.x),
    };
    assert!((normal - border).abs() < 0.01);
}

#[test]
fn shared_meter_is_cumulative_and_sticky() {
    let input = measured();
    let policy = RenderResourcePolicy::trusted_native();
    let baseline = OperationWorkMeter::new(policy);
    baseline.charge(5).unwrap();
    layout(&input, Options::default(), &baseline).unwrap();
    let total = baseline.used();
    for limit in [total, total - 1] {
        let meter = OperationWorkMeter::new(
            policy
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap(),
        );
        meter.charge(5).unwrap();
        let result = layout(&input, Options::default(), &meter);
        if limit == total {
            assert_eq!(result.unwrap().edge_order, [1, 2, 3, 0]);
            assert_eq!(meter.used(), total);
        } else {
            let error = result.unwrap_err();
            assert!(matches!(
                error,
                LayeredError::Work(OperationWorkError::ResourceLimitExceeded(_))
            ));
            assert_eq!(
                layout(&input, Options::default(), &meter).unwrap_err(),
                error
            );
        }
    }
    let cancelled = OperationWorkMeter::new(policy);
    cancelled.control().cancel();
    let error = layout(&input, Options::default(), &cancelled).unwrap_err();
    assert!(matches!(
        error,
        LayeredError::Work(OperationWorkError::Cancelled(_))
    ));
    assert_eq!(cancelled.used(), 0);
}

#[test]
fn invalid_slot_can_fall_back_and_feedback_stays_in_source_direction() {
    let mut input = measured();
    input.groups.clear();
    input.edges = vec![
        edge(Endpoint::Node(0), Endpoint::Node(1)),
        edge(Endpoint::Node(1), Endpoint::Node(0)),
    ];
    let meter = OperationWorkMeter::new(RenderResourcePolicy::trusted_native());
    let output = layout(&input, Options::default(), &meter).unwrap();
    assert!(!output.graph.edges[0].reversed_for_ranking);
    assert!(output.graph.edges[1].reversed_for_ranking);
    let [first, .., last] = output.graph.edges[1].points.as_slice() else {
        panic!("feedback route")
    };
    assert!(first.y > last.y);

    input.edges[0].source = Endpoint::Node(99);
    let error = layout(&input, Options::default(), &meter).unwrap_err();
    assert_eq!(
        error,
        LayeredError::InvalidInput(InputError::UnknownSource(0))
    );
    assert!(error.is_recoverable());
    assert!(meter.charge(0).is_ok());
}

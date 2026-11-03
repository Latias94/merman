use super::*;
use crate::model::SwimlaneDirection;
use working::{ProducerLaneRole, WorkingEdge, WorkingLayout, WorkingNode, WorkingNodeKind};

fn node(
    id: &str,
    parent: Option<&str>,
    kind: WorkingNodeKind,
    width: f64,
    height: f64,
) -> WorkingNode {
    WorkingNode {
        id: id.into(),
        label: id.into(),
        label_type: "text".into(),
        shape: "rect".into(),
        kind,
        producer_lane_role: None,
        parent_id: parent.map(str::to_string),
        top_lane_id: None,
        requested_dir: None,
        padding: 20.0,
        x: 0.0,
        y: 0.0,
        width,
        height,
        label_width: width,
        label_height: height,
        layer: 0,
        order: 0,
        content_top: None,
        title_rect: None,
    }
}

fn edge(id: &str, from: &str, to: &str, label: Option<&str>) -> WorkingEdge {
    WorkingEdge {
        id: id.into(),
        from: from.into(),
        to: to.into(),
        reference_id: id.into(),
        label_node_id: label.map(str::to_string),
        reversed_for_layout: false,
        points: Vec::new(),
    }
}

#[test]
fn lanes_resolve_nearest_lane_in_pool_and_guard_parent_cycles() {
    let mut nodes: indexmap::IndexMap<_, _> = [
        node("pool", None, WorkingNodeKind::Group, 0.0, 0.0),
        node("sales", Some("pool"), WorkingNodeKind::Group, 0.0, 0.0),
        node("warehouse", Some("pool"), WorkingNodeKind::Group, 0.0, 0.0),
        node("nested", Some("sales"), WorkingNodeKind::Group, 0.0, 0.0),
        node("task", Some("nested"), WorkingNodeKind::Content, 60.0, 40.0),
        node("plain", None, WorkingNodeKind::Group, 0.0, 0.0),
        node("solo", None, WorkingNodeKind::Group, 0.0, 0.0),
        node("cycleA", Some("cycleB"), WorkingNodeKind::Group, 0.0, 0.0),
        node("cycleB", Some("cycleA"), WorkingNodeKind::Group, 0.0, 0.0),
    ]
    .into_iter()
    .map(|node| (node.id.clone(), node))
    .collect();
    nodes["pool"].producer_lane_role = Some(ProducerLaneRole::Pool);
    nodes["pool"].shape = "swimlane".into();
    nodes["solo"].producer_lane_role = Some(ProducerLaneRole::Pool);
    let lanes = lanes::LaneModel::new(&nodes);
    assert!(lanes.is_pool("pool"));
    assert!(lanes.is_lane("sales") && lanes.is_lane("warehouse") && lanes.is_lane("plain"));
    assert!(!lanes.is_lane("nested") && !lanes.is_lane("pool"));
    assert!(lanes.is_lane("solo") && !lanes.is_pool("solo"));
    assert_eq!(lanes.lane_of("task"), Some("sales"));
    assert_eq!(lanes.lane_of("sales"), None);
    assert_eq!(lanes.lane_of("cycleA"), None);
    assert_eq!(lanes.lanes_by_pool["pool"], ["sales", "warehouse"]);
    assert_eq!(
        lanes.order(&nodes, &std::collections::HashMap::new()),
        ["solo", "plain", "warehouse", "sales"]
    );
    let partial = std::collections::HashMap::from([("sales", -1.0)]);
    assert_eq!(
        lanes.order(&nodes, &partial),
        ["solo", "plain", "warehouse", "sales"]
    );
    let complete = std::collections::HashMap::from([
        ("sales", 0.0),
        ("warehouse", 3.0),
        ("plain", 2.0),
        ("solo", 1.0),
    ]);
    assert_eq!(
        lanes.order(&nodes, &complete),
        ["sales", "solo", "plain", "warehouse"]
    );
}

fn labelled_layout(
    direction: SwimlaneDirection,
    label_width: f64,
    label_height: f64,
    count: usize,
) -> WorkingLayout {
    let mut nodes: indexmap::IndexMap<_, _> = [
        node("lane", None, WorkingNodeKind::Group, 0.0, 0.0),
        node("A", Some("lane"), WorkingNodeKind::Content, 60.0, 40.0),
        node("B", Some("lane"), WorkingNodeKind::Content, 60.0, 40.0),
        node("C", Some("lane"), WorkingNodeKind::Content, 60.0, 40.0),
    ]
    .into_iter()
    .map(|node| (node.id.clone(), node))
    .collect();
    let mut edges = vec![edge("A_B", "A", "B", None), edge("B_C", "B", "C", None)];
    for index in 0..count {
        let id = format!("label{index}");
        nodes.insert(
            id.clone(),
            node(
                &id,
                Some("lane"),
                WorkingNodeKind::EdgeLabel,
                label_width,
                label_height,
            ),
        );
        if index == 0 {
            edges[0].label_node_id = Some(id);
        } else {
            edges.push(edge(&format!("parallel{index}"), "A", "B", Some(&id)));
        }
    }
    let mut layout = WorkingLayout {
        direction,
        nodes,
        graph_edges: edges.clone(),
        original_edges: edges,
        top_lane_order: vec!["lane".into()],
    };
    layout.refresh_top_lane_ids();
    sugiyama::run(
        &mut layout,
        config::SwimlaneConfig {
            node_gap: 40.0,
            layer_gap: 100.0,
            ignore_cross_lane_edges: true,
            optimize_ranks_by_crossings: true,
            automatic_lane_ordering: false,
        },
    );
    layout
}

#[test]
fn labels_reserve_measured_flow_extent_once_without_affecting_ranks() {
    for direction in [
        SwimlaneDirection::Tb,
        SwimlaneDirection::Bt,
        SwimlaneDirection::Lr,
        SwimlaneDirection::Rl,
    ] {
        let labelled = labelled_layout(direction, 400.0, 130.0, 1);
        let plain = labelled_layout(direction, 0.0, 0.0, 0);
        for id in ["A", "B", "C"] {
            assert_eq!(labelled.nodes[id].layer, plain.nodes[id].layer);
        }
        let stretch = direction::flow_axis_stretch(&plain);
        assert_eq!(direction::flow_axis_stretch(&labelled), stretch);
        let delta = (labelled.nodes["C"].y - labelled.nodes["A"].y)
            - (plain.nodes["C"].y - plain.nodes["A"].y);
        let horizontal = matches!(direction, SwimlaneDirection::Lr | SwimlaneDirection::Rl);
        let (scale, extent) = if horizontal {
            (stretch, 400.0)
        } else {
            (1.0, 130.0)
        };
        assert!(
            (delta * scale - extent - 32.0).abs() < 1e-6,
            "{direction:?}: {delta}"
        );
    }
}

#[test]
fn labels_sharing_a_gap_spread_in_declaration_order() {
    let layout = labelled_layout(SwimlaneDirection::Tb, 100.0, 21.0, 3);
    for (left, right) in [("label0", "label1"), ("label1", "label2")] {
        assert_eq!(layout.nodes[left].y, layout.nodes[right].y);
        assert!(
            layout.nodes[left].x + layout.nodes[left].width / 2.0
                < layout.nodes[right].x - layout.nodes[right].width / 2.0
        );
    }
}

#[test]
fn bend_touch_conflicts_are_strictly_below_six_pixels() {
    let point = |x, y| crate::model::LayoutPoint { x, y };
    let first = [point(0.0, 0.0), point(20.0, 0.0), point(20.0, 20.0)];
    for (distance, expected) in [(5.9, true), (6.0, false), (6.1, false)] {
        let second = [point(10.0, -distance), point(30.0, -distance)];
        assert_eq!(
            direction::geometry::routes_touch_at_bend(
                &first,
                &second,
                config::ROUTE_TOUCH_TOLERANCE
            ),
            expected
        );
    }
    assert!(!direction::geometry::routes_touch_at_bend(
        &[point(0.0, 0.0), point(20.0, 0.0)],
        &[point(0.0, 1.0), point(20.0, 1.0)],
        config::ROUTE_TOUCH_TOLERANCE
    ));
}

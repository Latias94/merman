use super::*;
use crate::AsciiRenderOptions;
use crate::graph::charset::GraphCharset;
use crate::graph::layout::{CanvasCoord, GraphLayout, layout_graph};
use crate::graph::model::{AsciiGraph, GraphDirection, GraphEdgeAttrs};
use crate::graph::routing::label::RoutedLabelPlacement;
use crate::graph::routing::path::StepDirection;
use crate::graph::routing::plan::LabelAnchor;
use crate::graph::routing::plan::orthogonal::plan_terminal_path;
use crate::graph::routing::plan::{NodeAttachment, NodeAttachments, PlannedRouteLabel};
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};

fn constrained_scene() -> (AsciiGraph, GraphLayout, GraphCharset) {
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    for id in ["a", "b", "s", "t", "left", "above", "below"] {
        graph.add_node(id, id);
    }
    graph.add_edge_with_attrs(
        "a",
        "b",
        GraphEdgeAttrs {
            label: Some("q".to_owned()),
            ..Default::default()
        },
    );
    graph.add_edge("s", "t");
    let options = AsciiRenderOptions::ascii();
    let mut layout = layout_graph(&graph, &options);
    // The source has one open side. A legal label beside an earlier vertical
    // route occupies its only exit; moving that route's label opens the exit.
    for (node, (x, y)) in layout.nodes.iter_mut().zip([
        (14, 3),
        (14, 17),
        (10, 10),
        (25, 10),
        (6, 10),
        (10, 6),
        (10, 14),
    ]) {
        node.x = x;
        node.y = y;
        node.width = 3;
        node.height = 3;
    }
    layout.grid_aligned = false;
    (graph, layout, GraphCharset::for_options(&options))
}

fn request<'a>(
    graph: &'a AsciiGraph,
    layout: &'a GraphLayout,
    charset: &'a GraphCharset,
    index: usize,
) -> EdgeRouteRequest<'a> {
    let edge = &graph.edges[index];
    EdgeRouteRequest {
        graph,
        graph_layout: layout,
        edges: &graph.edges,
        from: layout
            .nodes
            .iter()
            .find(|node| node.id == edge.from)
            .unwrap(),
        to: layout.nodes.iter().find(|node| node.id == edge.to).unwrap(),
        edge_index: index,
        edge,
        charset,
    }
}

fn initial_routes<'layout>(
    graph: &AsciiGraph,
    layout: &'layout GraphLayout,
    charset: &GraphCharset,
    labels: &RoutedLabelCatalogPlan<'_>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> (Vec<PreparedRoute>, SceneOccupancy<'layout>) {
    let mut occupancy =
        SceneOccupancy::try_new_for_routes(layout, 2, 2, resources, execution).unwrap();
    let path: Vec<_> = (6..17).map(|y| CanvasCoord { x: 15, y }).collect();
    let attachments = NodeAttachments {
        start: NodeAttachment {
            contact: CanvasCoord { x: 15, y: 5 },
            outward: StepDirection::Down,
            connector: Some('v'),
        },
        end: NodeAttachment {
            contact: CanvasCoord { x: 15, y: 17 },
            outward: StepDirection::Up,
            connector: None,
        },
    };
    let mut label = PlannedRouteLabel::new(
        labels.descriptor(0).unwrap(),
        RoutedLabelPlacement::new(13, 11, 1),
    );
    label.anchor = LabelAnchor::Segment {
        start: path[0],
        end: path[path.len() - 1],
        route_segment: None,
    };
    let first = request(graph, layout, charset, 0);
    let owner = RouteOwner::for_test(0, "a", "b");
    let plan = plan_terminal_path(
        &path,
        attachments,
        first.edge,
        Some(label),
        charset,
        resources,
    )
    .unwrap()
    .unwrap();
    let (_, plan) = admit_route_candidate(
        first,
        RouteObstacles {
            scene: &occupancy,
            routes: &[],
            owner: &owner,
        },
        plan,
        resources,
    )
    .unwrap()
    .unwrap();
    let mut routes = Vec::with_capacity(2);
    append_route(first, &owner, plan, &mut routes, &mut occupancy, resources).unwrap();
    assert_eq!(
        routes[0].plan.labels[0].placement,
        RoutedLabelPlacement::new(13, 11, 1)
    );
    (routes, occupancy)
}

#[test]
fn reorder_opens_a_blocked_exit_and_preserves_both_relationships_and_the_condition() {
    let (graph, layout, charset) = constrained_scene();
    let policy = AsciiResourcePolicy::unbounded();
    let mut resources = ResourceContext::new(policy);
    let execution = AsciiExecution::for_test(&policy);
    let labels =
        RoutedLabelCatalogPlan::try_new(&graph.edges, &[0, 1], charset.width_profile, &resources)
            .unwrap();
    let (mut routes, mut occupancy) = initial_routes(
        &graph,
        &layout,
        &charset,
        &labels,
        &mut resources,
        execution,
    );
    let old = routes[0].plan.clone();
    let blocked = request(&graph, &layout, &charset, 1);
    let owner = RouteOwner::for_test(1, "s", "t");
    let mut budget = SceneRepairBudget::default();
    assert!(
        select_route(
            blocked,
            None,
            None,
            &owner,
            &routes,
            &occupancy,
            &mut budget,
            &mut resources,
            execution
        )
        .unwrap()
        .is_none()
    );
    assert!(
        reorder_failed_route(
            blocked,
            None,
            &labels,
            &owner,
            &mut routes,
            &mut occupancy,
            &mut budget,
            &mut resources,
            execution
        )
        .unwrap()
    );
    assert_eq!(routes.len(), 2);
    assert_eq!(
        routes
            .iter()
            .map(|route| route.owner.canonical_edge_index)
            .collect::<Vec<_>>(),
        [1, 0]
    );
    assert_ne!(
        routes[1].plan, old,
        "the earlier contribution must actually change"
    );
    assert_eq!(routes[1].plan.labels.len(), 1);
    assert_eq!(
        routes[1].plan.labels[0].descriptor,
        old.labels[0].descriptor
    );
    for (route, source, target) in [
        (&routes[0], &layout.nodes[2], &layout.nodes[3]),
        (&routes[1], &layout.nodes[0], &layout.nodes[1]),
    ] {
        let ports = route.plan.attachments.unwrap();
        assert!(ports.start.contact.x >= source.x && ports.start.contact.x <= source.right());
        assert!(ports.start.contact.y >= source.y && ports.start.contact.y <= source.bottom());
        assert!(ports.end.contact.x >= target.x && ports.end.contact.x <= target.right());
        assert!(ports.end.contact.y >= target.y && ports.end.contact.y <= target.bottom());
        assert!(
            route
                .plan
                .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
                .unwrap()
                .is_some()
        );
    }
    let restored = occupancy
        .rebuild_for_routes(&routes, None, &mut resources, "flowchart")
        .unwrap();
    assert_eq!(restored.occupied_extent(), occupancy.occupied_extent());
    assert_eq!(budget.attempts, MAX_REORDER_ATTEMPTS - 1);
}

#[test]
fn reorder_cannot_assign_a_different_path_to_a_joint_channel_contributor() {
    let (graph, layout, charset) = constrained_scene();
    let policy = AsciiResourcePolicy::unbounded();
    let mut resources = ResourceContext::new(policy);
    let execution = AsciiExecution::for_test(&policy);
    let labels =
        RoutedLabelCatalogPlan::try_new(&graph.edges, &[0, 1], charset.width_profile, &resources)
            .unwrap();
    let (mut routes, mut occupancy) = initial_routes(
        &graph,
        &layout,
        &charset,
        &labels,
        &mut resources,
        execution,
    );
    let start = CanvasCoord { x: 15, y: 6 };
    let end = CanvasCoord { x: 15, y: 16 };
    routes[0].owner.allocated_branch = Some(crate::graph::routing::allocation::AllocatedBranch {
        group: 0,
        shared_endpoint: MarkerEndpoint::Start,
        attachments: routes[0].plan.attachments.unwrap(),
        points: [start, end, end, end],
        label_host: Some([start, end]),
    });
    let old = routes[0].plan.clone();
    let allocation = routes[0].owner.allocated_branch;
    let owner = RouteOwner::for_test(1, "s", "t");
    // The ordinary version of this scene is repairable (tested above). Here the
    // required private label host crosses the proposed new edge, so this one-edge
    // reorder must fail rather than detach a branch from its retained allocation.
    assert!(
        !reorder_failed_route(
            request(&graph, &layout, &charset, 1),
            None,
            &labels,
            &owner,
            &mut routes,
            &mut occupancy,
            &mut SceneRepairBudget::default(),
            &mut resources,
            execution
        )
        .unwrap()
    );
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].plan, old);
    assert_eq!(routes[0].owner.allocated_branch, allocation);
}

#[test]
fn unsuccessful_reorder_keeps_the_original_routes_and_their_reservations() {
    let (mut graph, mut layout, charset) = constrained_scene();
    // These independent nodes surround the target. Removing an earlier edge
    // cannot solve this scene, so the proposal must be rolled back.
    for (index, (x, y)) in [(21, 10), (29, 10), (25, 6), (25, 14)]
        .into_iter()
        .enumerate()
    {
        let id = format!("target_neighbor_{index}");
        graph.add_node(&id, "N");
        let mut node = layout.nodes[3].clone();
        node.id = id;
        node.x = x;
        node.y = y;
        layout.nodes.push(node);
    }
    let policy = AsciiResourcePolicy::unbounded();
    let mut resources = ResourceContext::new(policy);
    let execution = AsciiExecution::for_test(&policy);
    let labels =
        RoutedLabelCatalogPlan::try_new(&graph.edges, &[0, 1], charset.width_profile, &resources)
            .unwrap();
    let (mut routes, mut occupancy) = initial_routes(
        &graph,
        &layout,
        &charset,
        &labels,
        &mut resources,
        execution,
    );
    let plan = routes[0].plan.clone();
    let before_cells = format!("{:?}", occupancy.route_cells);
    let before_extent = occupancy.occupied_extent();
    let before_work = resources.layout_work_used();
    let owner = RouteOwner::for_test(1, "s", "t");
    let mut budget = SceneRepairBudget::default();
    assert!(
        !reorder_failed_route(
            request(&graph, &layout, &charset, 1),
            None,
            &labels,
            &owner,
            &mut routes,
            &mut occupancy,
            &mut budget,
            &mut resources,
            execution
        )
        .unwrap()
    );
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].plan, plan);
    assert_eq!(routes[0].owner.canonical_edge_index, 0);
    assert_eq!(format!("{:?}", occupancy.route_cells), before_cells);
    assert_eq!(occupancy.occupied_extent(), before_extent);
    assert!(resources.layout_work_used() > before_work);
    assert_eq!(budget.attempts, MAX_REORDER_ATTEMPTS - 1);
}

#[test]
fn late_resource_failure_rolls_back_proposals_but_keeps_the_work_charged() {
    let (graph, layout, charset) = constrained_scene();
    let mut successful_work = None;
    for exact in [None, Some(true), Some(false)] {
        let mut policy = AsciiResourcePolicy::unbounded();
        if let Some(exact) = exact {
            policy = policy
                .with_limit(
                    AsciiResourceLimitId::MaxLayoutWorkUnits,
                    successful_work.unwrap() - usize::from(!exact),
                )
                .unwrap();
        }
        let mut resources = ResourceContext::new(policy);
        let execution = AsciiExecution::for_test(&policy);
        let labels = RoutedLabelCatalogPlan::try_new(
            &graph.edges,
            &[0, 1],
            charset.width_profile,
            &resources,
        )
        .unwrap();
        let (mut routes, mut occupancy) = initial_routes(
            &graph,
            &layout,
            &charset,
            &labels,
            &mut resources,
            execution,
        );
        let old = routes[0].plan.clone();
        let before_cells = format!("{:?}", occupancy.route_cells);
        let before_extent = occupancy.occupied_extent();
        let before_work = resources.layout_work_used();
        let owner = RouteOwner::for_test(1, "s", "t");
        let mut budget = SceneRepairBudget::default();
        let result = reorder_failed_route(
            request(&graph, &layout, &charset, 1),
            None,
            &labels,
            &owner,
            &mut routes,
            &mut occupancy,
            &mut budget,
            &mut resources,
            execution,
        );
        if exact == Some(false) {
            assert!(
                matches!(result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits)
            );
            assert_eq!(routes.len(), 1);
            assert_eq!(routes[0].plan, old);
            assert_eq!(format!("{:?}", occupancy.route_cells), before_cells);
            assert_eq!(occupancy.occupied_extent(), before_extent);
            assert!(resources.layout_work_used() > before_work);
        } else {
            assert!(result.unwrap());
            assert_eq!(routes.len(), 2);
            if successful_work.is_none() {
                successful_work = Some(resources.layout_work_used());
            }
        }
    }
}

#[test]
fn cancellation_before_scene_replacement_restores_the_incumbent_without_replay() {
    use merman_model::OperationControl;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::{Duration, Instant};

    let (graph, layout, charset) = constrained_scene();
    let mut checkpoints = None;
    for cancel in [false, true] {
        let calls = Arc::new(AtomicUsize::new(0));
        let clock_calls = calls.clone();
        let now = Instant::now();
        let control = OperationControl::with_test_clock(Arc::new(move || {
            clock_calls.fetch_add(1, Ordering::Relaxed);
            now
        }))
        .with_deadline(Duration::from_secs(60));
        let policy = AsciiResourcePolicy::unbounded();
        let mut resources = ResourceContext::new(policy);
        let execution = AsciiExecution::new(&control, &policy);
        execution.rebind_resource_context(&mut resources, OperationPhase::Layout);
        let labels = RoutedLabelCatalogPlan::try_new(
            &graph.edges,
            &[0, 1],
            charset.width_profile,
            &resources,
        )
        .unwrap();
        let (mut routes, mut occupancy) = initial_routes(
            &graph,
            &layout,
            &charset,
            &labels,
            &mut resources,
            execution,
        );
        let old = routes[0].plan.clone();
        let before_cells = format!("{:?}", occupancy.route_cells);
        let before_extent = occupancy.occupied_extent();
        let before_work = resources.layout_work_used();
        let before_calls = calls.load(Ordering::Relaxed);
        if cancel {
            control.cancel_after_checkpoints(checkpoints.unwrap() - 1);
        }
        let owner = RouteOwner::for_test(1, "s", "t");
        let result = reorder_failed_route(
            request(&graph, &layout, &charset, 1),
            None,
            &labels,
            &owner,
            &mut routes,
            &mut occupancy,
            &mut SceneRepairBudget::default(),
            &mut resources,
            execution,
        );
        if cancel {
            assert!(
                matches!(result, Err(AsciiError::Cancelled(cancelled)) if cancelled.phase == OperationPhase::Layout)
            );
            assert_eq!(routes.len(), 1);
            assert_eq!(routes[0].plan, old);
            assert_eq!(format!("{:?}", occupancy.route_cells), before_cells);
            assert_eq!(occupancy.occupied_extent(), before_extent);
            assert!(resources.layout_work_used() > before_work);
        } else {
            assert!(result.unwrap());
            checkpoints = Some(calls.load(Ordering::Relaxed) - before_calls);
        }
    }
}

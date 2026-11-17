use super::*;
use crate::AsciiRenderOptions;
use crate::graph::charset::GraphCharset;
use crate::graph::label::GraphLabel;
use crate::graph::layout::{CanvasCoord, GraphLayout, GridCoord, NodeLayout};
use crate::graph::model::{
    AsciiGraph, GraphDirection, GraphEdgeAttrs, GraphEdgeMarker, GraphEdgeStroke, GraphNodeShape,
    GraphNodeStyle,
};
use crate::graph::routing::cell::{DIR_DOWN, DIR_LEFT, DIR_RIGHT, DIR_UP};
use crate::graph::routing::conflict::ReservationKind;
use crate::graph::routing::label::RoutedLabelPlacement;
use crate::graph::routing::path::StepDirection;
use crate::graph::routing::plan::orthogonal::plan_terminal_path;
use crate::graph::routing::plan::{
    LabelAnchor, NodeAttachment, NodeAttachments, PlannedRouteCellKind, PlannedRouteLabel,
};
use crate::resource::AsciiResourcePolicy;
use std::collections::{BTreeMap, BTreeSet};

const CONDITIONS: [&str; 4] = ["a<br>a<br>a", "b<br>b<br>b", "c<br>c<br>c", "d<br>d<br>d"];
const PAIRS: [(&str, &str); 5] = [
    ("a0", "b0"),
    ("a1", "b1"),
    ("a2", "b2"),
    ("a3", "b3"),
    ("s", "t"),
];

fn constrained_four_contributor_scene() -> Result<(AsciiGraph, GraphLayout, GraphCharset)> {
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
    let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    graph.try_reserve_projection(22, 5, 0)?;
    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(22)
        .map_err(|_| super::super::layout_allocation_failed())?;
    // The walls have ordinary rectangular contours and retain their reading margins.
    // Endpoint rectangles replace parts of the walls rather than overlapping them.
    for (id, label, x, y, width, height) in [
        ("s", "s", 28, 44, 3, 3),
        ("t", "t", 90, 44, 3, 3),
        ("left", "w", 20, 43, 3, 5),
        ("right", "w", 98, 43, 3, 5),
        ("top0", "w", 20, 40, 21, 3),
        ("top1", "w", 44, 40, 7, 3),
        ("top2", "w", 54, 40, 7, 3),
        ("top3", "w", 64, 40, 7, 3),
        ("top4", "w", 74, 40, 27, 3),
        ("bottom0", "w", 20, 48, 21, 3),
        ("bottom1", "w", 44, 48, 7, 3),
        ("bottom2", "w", 54, 48, 7, 3),
        ("bottom3", "w", 64, 48, 7, 3),
        ("bottom4", "w", 74, 48, 27, 3),
        ("a0", "a", 41, 40, 3, 3),
        ("b0", "A", 41, 48, 3, 3),
        ("a1", "b", 51, 40, 3, 3),
        ("b1", "B", 51, 48, 3, 3),
        ("a2", "c", 61, 40, 3, 3),
        ("b2", "C", 61, 48, 3, 3),
        ("a3", "d", 71, 40, 3, 3),
        ("b3", "D", 71, 48, 3, 3),
    ] {
        graph.add_node(id, label);
        nodes.push(NodeLayout {
            id: id.to_owned(),
            label: GraphLabel::try_new_with_profile(label, charset.width_profile, &resources)?,
            shape: GraphNodeShape::Rect,
            style: GraphNodeStyle::default(),
            grid: GridCoord { x: 0, y: 0 },
            x,
            y,
            width,
            height,
        });
    }
    for (index, (from, to)) in PAIRS.into_iter().enumerate() {
        graph.add_edge_with_attrs(
            from,
            to,
            GraphEdgeAttrs {
                label: CONDITIONS.get(index).map(|label| (*label).to_owned()),
                ..Default::default()
            },
        );
    }
    Ok((
        graph,
        GraphLayout {
            nodes,
            groups: Vec::new(),
            group_background_order: Vec::new(),
        },
        charset,
    ))
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

fn unit_path(vertices: &[(usize, usize)]) -> Vec<CanvasCoord> {
    let mut path = vec![CanvasCoord {
        x: vertices[0].0,
        y: vertices[0].1,
    }];
    for segment in vertices.windows(2) {
        let (mut x, mut y) = segment[0];
        let (end_x, end_y) = segment[1];
        assert!(x == end_x || y == end_y, "fixture paths are orthogonal");
        while (x, y) != (end_x, end_y) {
            match (x.cmp(&end_x), y.cmp(&end_y)) {
                (std::cmp::Ordering::Less, _) => x += 1,
                (std::cmp::Ordering::Greater, _) => x -= 1,
                (_, std::cmp::Ordering::Less) => y += 1,
                (_, std::cmp::Ordering::Greater) => y -= 1,
                _ => unreachable!("the endpoint terminates the path"),
            }
            path.push(CanvasCoord { x, y });
        }
    }
    path
}

fn attachment(node: &NodeLayout, berth: CanvasCoord, charset: &GraphCharset) -> NodeAttachment {
    let inward = if berth.x < node.x {
        StepDirection::Right
    } else if berth.x > node.right() {
        StepDirection::Left
    } else if berth.y < node.y {
        StepDirection::Down
    } else {
        StepDirection::Up
    };
    NodeAttachment::resolve(node, berth, inward, charset)
        .expect("the independently chosen berth must resolve against the actual contour")
}

fn admit_fixed_route(
    request: EdgeRouteRequest<'_>,
    vertices: &[(usize, usize)],
    label: Option<PlannedRouteLabel>,
    routes: &mut Vec<PreparedRoute>,
    occupancy: &mut SceneOccupancy<'_>,
    resources: &mut ResourceContext,
) -> Result<()> {
    let path = unit_path(vertices);
    let attachments = NodeAttachments {
        start: attachment(request.from, path[0], request.charset),
        end: attachment(request.to, *path.last().unwrap(), request.charset),
    };
    let placement = label.as_ref().map(|label| label.placement);
    let miss = || AsciiError::RoutingSearchExhausted {
        diagram_type: "flowchart",
        edge_index: request.edge_index,
    };
    let plan = plan_terminal_path(
        &path,
        attachments,
        request.edge,
        label,
        request.charset,
        resources,
    )?
    .ok_or_else(miss)?;
    let owner = RouteOwner::for_test(request.edge_index, &request.edge.from, &request.edge.to);
    let (_, plan) = admit_route_candidate(
        request,
        RouteObstacles {
            scene: occupancy,
            routes,
            owner: &owner,
        },
        plan,
        resources,
    )?
    .ok_or_else(miss)?;
    append_route(request, &owner, plan, routes, occupancy, resources)?;
    assert_eq!(
        routes
            .last()
            .unwrap()
            .plan
            .labels
            .first()
            .map(|label| label.placement),
        placement,
        "the constructive fixture requires its exact admitted condition placement"
    );
    Ok(())
}

// A bounded independent oracle for this rectangular, ASCII, five-relationship witness.
// It does not call route scoring, contour resolution, canonicalization, or scene validation.
fn assert_independent_facts(
    graph: &AsciiGraph,
    layout: &GraphLayout,
    routes: &[PreparedRoute],
    labels: &RoutedLabelCatalogPlan<'_>,
    expected_count: usize,
    resources: &ResourceContext,
) -> Result<()> {
    assert!(layout.groups.is_empty() && graph.groups.is_empty());
    assert_eq!(graph.edges.len(), PAIRS.len());
    assert_eq!(routes.len(), expected_count);
    for (edge, &(from, to)) in graph.edges.iter().zip(&PAIRS) {
        assert_eq!((&*edge.from, &*edge.to), (from, to));
        assert_eq!(edge.start_marker, GraphEdgeMarker::Open);
        assert_eq!(edge.end_marker, GraphEdgeMarker::Point);
        assert_eq!(edge.stroke, GraphEdgeStroke::Normal);
    }
    for (index, node) in layout.nodes.iter().enumerate() {
        assert_eq!(node.shape, GraphNodeShape::Rect);
        assert!(node.width >= 3 && node.height >= 3);
        for other in &layout.nodes[..index] {
            assert!(
                node.right() < other.x
                    || other.right() < node.x
                    || node.bottom() < other.y
                    || other.bottom() < node.y
            );
        }
    }
    let catalog = labels.materialize(resources)?;
    let mut covered = BTreeSet::new();
    let mut occupied: BTreeMap<_, Vec<(usize, u8, bool)>> = BTreeMap::new();
    let mut label_boxes = Vec::new();
    let on_ray = |port: NodeAttachment, coord: CanvasCoord| match port.outward {
        StepDirection::Up => coord.x == port.contact.x && coord.y <= port.contact.y,
        StepDirection::Down => coord.x == port.contact.x && coord.y >= port.contact.y,
        StepDirection::Left => coord.y == port.contact.y && coord.x <= port.contact.x,
        StepDirection::Right => coord.y == port.contact.y && coord.x >= port.contact.x,
    };
    for route in routes {
        let index = route.owner.canonical_edge_index;
        assert!(index < expected_count && covered.insert(index));
        assert_eq!(route.owner.source_edge_index, index);
        assert_eq!(
            (route.owner.from.as_str(), route.owner.to.as_str()),
            PAIRS[index]
        );
        assert_eq!(route.owner.stroke, GraphEdgeStroke::Normal);
        assert!(route.owner.allocated_branch.is_none());
        let ports = route
            .plan
            .attachments
            .expect("strict routes retain both contacts");
        for (endpoint, id, port) in [
            (MarkerEndpoint::Start, &route.owner.from, ports.start),
            (MarkerEndpoint::End, &route.owner.to, ports.end),
        ] {
            let node = layout.nodes.iter().find(|node| &node.id == id).unwrap();
            let CanvasCoord { x, y } = port.contact;
            assert!(
                match port.outward {
                    StepDirection::Up => y == node.y && node.x < x && x < node.right(),
                    StepDirection::Down => y == node.bottom() && node.x < x && x < node.right(),
                    StepDirection::Left => x == node.x && node.y < y && y < node.bottom(),
                    StepDirection::Right => x == node.right() && node.y < y && y < node.bottom(),
                },
                "invalid rectangular contact for {id}"
            );
            let terminal = route.plan.terminal_candidate(endpoint, "flowchart")?;
            assert_eq!(terminal.point_direction, port.outward.opposite());
            let mut coord = port.contact;
            let distance = coord.x.abs_diff(terminal.coord.x) + coord.y.abs_diff(terminal.coord.y);
            assert!(distance > 0 && distance <= route.plan.cells.len());
            for _ in 0..distance {
                match port.outward {
                    StepDirection::Up => coord.y -= 1,
                    StepDirection::Down => coord.y += 1,
                    StepDirection::Left => coord.x -= 1,
                    StepDirection::Right => coord.x += 1,
                }
                assert!(route.plan.cells.iter().any(|cell| cell.coord == coord));
            }
            assert_eq!(
                coord, terminal.coord,
                "a terminal cannot leave its contact ray"
            );
        }
        let body: BTreeMap<_, _> = route
            .plan
            .active_cells()
            .filter(|(_, cell)| {
                cell.coord != ports.start.contact && cell.coord != ports.end.contact
            })
            .map(|(_, cell)| ((cell.coord.x, cell.coord.y), cell.directions))
            .collect();
        assert!(!body.is_empty());
        let first = *body.keys().next().unwrap();
        let mut visited = BTreeSet::from([first]);
        let mut pending = vec![first];
        while let Some((x, y)) = pending.pop() {
            for (next, heading, opposite) in [
                (x.checked_sub(1).map(|x| (x, y)), DIR_LEFT, DIR_RIGHT),
                (Some((x + 1, y)), DIR_RIGHT, DIR_LEFT),
                (y.checked_sub(1).map(|y| (x, y)), DIR_UP, DIR_DOWN),
                (Some((x, y + 1)), DIR_DOWN, DIR_UP),
            ] {
                if let Some(next) = next
                    && body[&(x, y)] & heading != 0
                    && body.get(&next).is_some_and(|mask| mask & opposite != 0)
                    && visited.insert(next)
                {
                    pending.push(next);
                }
            }
        }
        assert_eq!(
            visited.len(),
            body.len(),
            "disconnected canonical edge {index}"
        );
        assert!(!route.plan.requests_marker(MarkerEndpoint::Start));
        let marker = route
            .plan
            .materialized_marker_cell(MarkerEndpoint::End, "flowchart")?
            .expect("every directed relationship retains its pointed target");
        let expected_arrow = match ports.end.outward {
            StepDirection::Up => 'v',
            StepDirection::Down => '^',
            StepDirection::Left => '>',
            StepDirection::Right => '<',
        };
        assert_eq!(marker.ch, expected_arrow);
        assert_eq!(
            route
                .plan
                .active_cells()
                .filter(|(_, cell)| cell.kind == PlannedRouteCellKind::EdgeArrow)
                .count(),
            1
        );
        for (_, cell) in route.plan.active_cells() {
            assert_eq!(cell.stroke, GraphEdgeStroke::Normal);
            for node in &layout.nodes {
                let in_margin = node.x - 1 <= cell.coord.x
                    && cell.coord.x <= node.right() + 1
                    && node.y - 1 <= cell.coord.y
                    && cell.coord.y <= node.bottom() + 1;
                assert!(
                    !in_margin
                        || (node.id == route.owner.from && on_ray(ports.start, cell.coord))
                        || (node.id == route.owner.to && on_ray(ports.end, cell.coord)),
                    "a route entered an unrelated rectangular reading margin"
                );
            }
            let contributions = occupied.entry((cell.coord.x, cell.coord.y)).or_default();
            for &(previous, mask, arrow) in contributions.iter() {
                assert_ne!(previous, index, "duplicate route cell");
                assert!(
                    (mask == (DIR_LEFT | DIR_RIGHT) && cell.directions == (DIR_UP | DIR_DOWN))
                        || (mask == (DIR_UP | DIR_DOWN)
                            && cell.directions == (DIR_LEFT | DIR_RIGHT)),
                    "independent pairs may only cross transversely"
                );
                assert!(!arrow && cell.kind != PlannedRouteCellKind::EdgeArrow);
            }
            contributions.push((
                index,
                cell.directions,
                cell.kind == PlannedRouteCellKind::EdgeArrow,
            ));
        }
        assert_eq!(route.plan.labels.len(), usize::from(index < 4));
        for label in &route.plan.labels {
            assert_eq!(Some(label.descriptor), labels.descriptor(index));
            assert_eq!(label.descriptor.width(), 1);
            assert_eq!(label.descriptor.line_count(), 3);
            let text = catalog.get(label.descriptor)?;
            assert_eq!(text.lines().join("<br>"), CONDITIONS[index]);
            assert_eq!(graph.edges[index].label.as_deref(), Some(CONDITIONS[index]));
            let LabelAnchor::Segment { start, end, .. } = label.anchor else {
                panic!("complete conditions require an owned route segment");
            };
            assert!(start.x == end.x || start.y == end.y);
            assert!(body.contains_key(&(start.x, start.y)));
            assert!(body.contains_key(&(end.x, end.y)));
            let x = label.placement.x();
            let y = label.placement.y();
            label_boxes.push((index, x, y, start, end));
            for yy in y..y + 3 {
                assert!(layout.nodes.iter().all(|node| x < node.x
                    || x > node.right()
                    || yy < node.y
                    || yy > node.bottom()));
                assert!(
                    occupied
                        .get(&(x, yy))
                        .is_none_or(|owners| owners.iter().all(|&(owner, _, _)| owner == index))
                );
            }
        }
    }
    assert_eq!(covered, (0..expected_count).collect());
    let mut reservations = BTreeMap::new();
    for &(index, x, y, start, end) in &label_boxes {
        // Include the entire gap to the host, not only the visible label column.
        let (left, right, top, bottom) = if start.x == end.x {
            assert!(y <= start.y.max(end.y) && start.y.min(end.y) <= y + 2);
            (x.min(start.x), x.max(start.x), y, y + 2)
        } else {
            assert!(start.x.min(end.x) <= x && x <= start.x.max(end.x));
            (x, x, y.min(start.y), (y + 2).max(start.y))
        };
        for yy in top..=bottom {
            for xx in left..=right {
                assert!(
                    occupied
                        .get(&(xx, yy))
                        .is_none_or(|owners| owners.iter().all(|&(owner, _, _)| owner == index)),
                    "condition reservation meets another relationship"
                );
                assert!(
                    reservations.insert((xx, yy), index).is_none(),
                    "independent condition reservations cannot overlap"
                );
            }
        }
        for &(other, other_x, other_y, _, _) in &label_boxes {
            if index != other {
                assert!(
                    x.abs_diff(other_x) > 1 || y + 3 <= other_y || other_y + 3 <= y,
                    "complete conditions require a blank reading column"
                );
            }
        }
    }
    Ok(())
}

fn assert_singleton_cut(layout: &GraphLayout, retained: usize) {
    let mut blocked = BTreeSet::new();
    for node in &layout.nodes {
        if node.id == "s" || node.id == "t" {
            continue;
        }
        for x in node.x - 1..=node.x + node.width {
            for y in node.y - 1..=node.y + node.height {
                blocked.insert((x, y));
            }
        }
    }
    // Admission checks this exact column against the retained condition below.
    for y in 44..=46 {
        blocked.insert((41 + 10 * retained, y));
    }
    let start = (31usize, 45usize);
    let mut visited = BTreeSet::from([start]);
    let mut pending = vec![start];
    while let Some((x, y)) = pending.pop() {
        assert_ne!(
            (x, y),
            (89, 45),
            "one retained cut must separate the entire corridor"
        );
        for next in [
            x.checked_sub(1).map(|x| (x, y)),
            Some((x + 1, y)),
            y.checked_sub(1).map(|y| (x, y)),
            Some((x, y + 1)),
        ]
        .into_iter()
        .flatten()
        {
            if next.0 <= 106 && next.1 <= 56 && !blocked.contains(&next) && visited.insert(next) {
                pending.push(next);
            }
        }
    }
}

#[test]
fn four_label_cuts_require_complete_repair_and_preserve_strict_relationships() -> Result<()> {
    let (graph, layout, charset) = constrained_four_contributor_scene()?;
    let policy = AsciiResourcePolicy::default();
    let mut resources = ResourceContext::new(policy);
    let operation_budget = GraphPlanningBudget::default();
    let execution = AsciiExecution::for_test(&policy).with_graph_planning(&operation_budget);
    let labels = RoutedLabelCatalogPlan::try_new(
        &graph.edges,
        &[0, 1, 2, 3, 4],
        charset.width_profile,
        &resources,
    )?;
    let condition = |index, x, y, start, end| {
        let mut label = PlannedRouteLabel::new(
            labels.descriptor(index).unwrap(),
            RoutedLabelPlacement::new(x, y, 1),
        );
        label.anchor = LabelAnchor::Segment {
            start,
            end,
            route_segment: None,
        };
        label
    };
    let mut occupancy =
        SceneOccupancy::try_new_for_routes(&layout, 5, 2, &mut resources, execution)?;
    let mut routes = Vec::with_capacity(5);
    for index in 0..4 {
        let x = 42 + 10 * index;
        admit_fixed_route(
            request(&graph, &layout, &charset, index),
            &[(x, 43), (x, 47)],
            Some(condition(
                index,
                x - 2,
                44,
                CanvasCoord { x, y: 43 },
                CanvasCoord { x, y: 47 },
            )),
            &mut routes,
            &mut occupancy,
            &mut resources,
        )?;
    }
    assert_independent_facts(&graph, &layout, &routes, &labels, 4, &resources)?;
    assert_eq!(
        super::super::validate::first_invalid_routes(
            &graph,
            &layout,
            &graph.edges[..4],
            &routes,
            &labels,
            &charset,
            &mut resources,
            execution,
        )?,
        None,
        "the exact incumbent must already be valid for every admitted edge"
    );
    let incumbent: Vec<_> = routes.iter().map(|route| route.plan.clone()).collect();

    // Construct a full solution without calling any search or repair implementation.
    let mut target_occupancy =
        SceneOccupancy::try_new_for_routes(&layout, 5, 2, &mut resources, execution)?;
    let mut target = Vec::with_capacity(5);
    admit_fixed_route(
        request(&graph, &layout, &charset, 4),
        &[(31, 45), (89, 45)],
        None,
        &mut target,
        &mut target_occupancy,
        &mut resources,
    )?;
    for index in 0..4 {
        let x = 42 + 10 * index;
        let outside = 16 - 4 * index;
        let top = 38 - index;
        let bottom = 52 + index;
        admit_fixed_route(
            request(&graph, &layout, &charset, index),
            &[
                (x, 39),
                (x, top),
                (outside, top),
                (outside, bottom),
                (x, bottom),
                (x, 51),
            ],
            Some(condition(
                index,
                outside - 2,
                44,
                CanvasCoord { x: outside, y: top },
                CanvasCoord {
                    x: outside,
                    y: bottom,
                },
            )),
            &mut target,
            &mut target_occupancy,
            &mut resources,
        )?;
    }
    assert_independent_facts(&graph, &layout, &target, &labels, 5, &resources)?;
    assert_eq!(
        super::super::validate::first_invalid_routes(
            &graph,
            &layout,
            &graph.edges,
            &target,
            &labels,
            &charset,
            &mut resources,
            execution,
        )?,
        None,
        "the independent full construction must pass production admission and validation"
    );

    let blocked = request(&graph, &layout, &charset, 4);
    let owner = RouteOwner::for_test(4, "s", "t");
    let ports = NodeAttachments {
        start: attachment(blocked.from, CanvasCoord { x: 31, y: 45 }, &charset),
        end: attachment(blocked.to, CanvasCoord { x: 89, y: 45 }, &charset),
    };
    // A singleton is already a complete vertical cut. Adding retained reservations
    // cannot open a path, so these four controls cover every proper removal subset.
    // The former at-most-three contributor proposal could never remove all four.
    for retained in 0..4 {
        assert_singleton_cut(&layout, retained);
        let omitted: Vec<_> = (0..4).filter(|&index| index != retained).collect();
        let trial = occupancy.rebuild_for_routes(&routes, &omitted, &mut resources, "flowchart")?;
        let retained_routes = std::slice::from_ref(&routes[retained]);
        for y in 44..=46 {
            assert!(!trial.route_cell_is_clear(
                retained_routes,
                &owner,
                Some(ports),
                CanvasCoord {
                    x: 41 + 10 * retained,
                    y
                },
                DIR_LEFT | DIR_RIGHT,
                &mut resources
            )?);
        }
        let mut budget = SceneRepairBudget::default();
        assert!(
            select_route(
                blocked,
                None,
                None,
                &owner,
                retained_routes,
                &trial,
                &mut budget,
                RepairSearch::Missing,
                &mut resources,
                AsciiExecution::for_test(&policy)
            )?
            .is_none()
        );
        assert!(
            budget.expansions > 0,
            "a retained cut fails geometrically, not by search exhaustion"
        );
    }

    // Start with exactly the evidence the failed search actually discovered.
    // Failed repair trials must discover later cuts and grow the retained closure.
    occupancy.begin_blocker_collection(&resources)?;
    let mut initial_search = SceneRepairBudget::default();
    assert!(
        select_route(
            blocked,
            None,
            None,
            &owner,
            &routes,
            &occupancy,
            &mut initial_search,
            RepairSearch::Missing,
            &mut resources,
            AsciiExecution::for_test(&policy)
        )?
        .is_none()
    );
    assert!(
        initial_search.expansions > 0,
        "the incumbent miss must not depend on exhausted search work"
    );
    let witnesses = occupancy.take_blocker_collection(&resources)?.unwrap();
    assert!(!witnesses.blockers.is_empty());
    assert!(
        witnesses
            .blockers
            .iter()
            .all(|blocker| blocker.canonical_edge_index < 4)
    );
    assert!(
        witnesses
            .blockers
            .iter()
            .any(|blocker| blocker.contains(ReservationKind::Label))
    );
    assert!(!witnesses.truncated);
    let mut budget = SceneRepairBudget::for_execution(execution);
    let allowance_before = operation_budget.replacement.remaining();
    assert!(
        repair_blocked_scene(
            blocked,
            None,
            &labels,
            &owner,
            &witnesses,
            &mut routes,
            &mut occupancy,
            &mut budget,
            &mut resources,
            execution
        )?,
        "all four actual blockers must be repaired into a complete strict scene"
    );
    assert!(budget.attempts < MAX_REORDER_ATTEMPTS);
    let remaining_attempts = budget.attempts;
    drop(budget);
    assert_eq!(operation_budget.repair_attempts.get(), remaining_attempts);
    let allowance_after = operation_budget.replacement.remaining();
    assert!(
        allowance_after.0 < allowance_before.0 && allowance_after.1 < allowance_before.1,
        "replacement search must debit the unchanged operation allowance"
    );
    for (index, old_plan) in incumbent.iter().enumerate() {
        let replacement = routes
            .iter()
            .find(|route| route.owner.canonical_edge_index == index)
            .unwrap();
        assert_ne!(
            &replacement.plan, old_plan,
            "every necessary contribution must change"
        );
    }
    assert_independent_facts(&graph, &layout, &routes, &labels, 5, &resources)?;
    assert_eq!(
        super::super::validate::first_invalid_routes(
            &graph,
            &layout,
            &graph.edges,
            &routes,
            &labels,
            &charset,
            &mut resources,
            execution,
        )?,
        None
    );
    let rebuilt = occupancy.rebuild_for_routes(&routes, &[], &mut resources, "flowchart")?;
    assert_eq!(rebuilt.occupied_extent(), occupancy.occupied_extent());
    Ok(())
}

use super::*;
use crate::block::{BlockAllocatedBounds, prepare_block_layout};
use crate::resources::RenderResourcePolicy;
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::RenderSemanticModel;
use merman_render_core::text::DeterministicTextMeasurer;

fn placed(
    source: &str,
) -> (
    BlockDiagramRenderModel,
    BlockDiagramLayout,
    BlockTypographyThemePlan,
    Arc<OperationWorkMeter>,
) {
    let presentation = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let RenderSemanticModel::Block(model) = presentation.model() else {
        panic!("expected a Block diagram");
    };
    let meter = Arc::new(OperationWorkMeter::new(RenderResourcePolicy::constrained()));
    let (layout, _, typography) = prepare_block_layout(
        model,
        &presentation.metadata().effective_config,
        None,
        &DeterministicTextMeasurer::default(),
        &meter,
    )
    .unwrap();
    (model.clone(), layout, typography, meter)
}

fn facing_ports(connections: &str) {
    let source = format!(
        "block\n  columns 1\n  A[\"First wide block\"]\n  space\n  B[\"Second wide block\"]\n{connections}"
    );
    let (model, mut layout, typography, meter) = placed(&source);
    let routes = prepare(&model, &mut layout, &typography, &meter).unwrap();
    assert_eq!(routes.len(), model.edges.len());
    let [upper, lower] = layout.shape_geometries.as_slice() else {
        panic!("expected two placed shapes");
    };
    let mut rails = [Vec::new(), Vec::new()];
    for (edge, route) in layout.edges.iter().zip(routes) {
        assert!(route.is_some(), "{} fell back to its grid path", edge.id);
        let first = edge.points.first().unwrap();
        let last = edge.points.last().unwrap();
        let (a, b) = if edge.from == upper.id {
            (first, last)
        } else {
            (last, first)
        };
        assert_eq!(a.y, shape_box(upper).y1, "{} left the facing side", edge.id);
        assert_eq!(b.y, shape_box(lower).y0, "{} left the facing side", edge.id);
        rails[0].push(a.x);
        rails[1].push(b.x);
    }
    for rails in rails {
        assert!(
            rails.windows(2).all(|pair| pair[1] - pair[0] >= 8.0),
            "expected distinct ports in the same order at both blocks: {rails:?}"
        );
    }
}

#[test]
fn reciprocal_routes_keep_distinct_facing_ports() {
    facing_ports("  A --> B\n  B --> A\n");
}

#[test]
fn parallel_routes_keep_distinct_facing_ports() {
    facing_ports("  A --> B\n  A --> B\n  A --> B\n");
}

#[test]
fn top_face_fan_in_orders_equal_projections_outward() {
    let source = "block\n  columns 1\n  Far\n  space\n  Near\n  space\n  Hub[\"Wide target\"]\n  Far --> Hub\n  Near --> Hub\n";
    let (model, mut layout, typography, meter) = placed(source);
    let routes = prepare(&model, &mut layout, &typography, &meter).unwrap();
    assert!(routes.iter().all(Option::is_some));
    let top = shape_box(&layout.shape_geometries[2]).y0;
    let far = layout.edges[0].points.last().unwrap();
    let near = layout.edges[1].points.last().unwrap();
    assert_eq!([near.y, far.y], [top; 2]);
    assert!(far.x - near.x >= 8.0, "near {near:?}, far {far:?}");
}

#[test]
fn bottom_face_fanout_uses_the_outward_angle_for_peers_in_one_column() {
    let source = "block\n  columns 2\n  A[\"A wide source\"] space\n  space:2\n  space B\n  space:2\n  space C\n  A --> B\n  A --> C\n";
    let (model, mut layout, typography, meter) = placed(source);
    let routes = prepare(&model, &mut layout, &typography, &meter).unwrap();
    assert!(routes.iter().all(Option::is_some));
    let bottom = shape_box(&layout.shape_geometries[0]).y1;
    let near = &layout.edges[0].points[0];
    let far = &layout.edges[1].points[0];
    assert_eq!([near.y, far.y], [bottom; 2]);
    assert!(near.x - far.x >= 8.0, "near {near:?}, far {far:?}");
}

#[test]
fn crowded_parallel_routes_keep_spacing_on_retried_faces() {
    let source = format!(
        "block\n  columns 3\n  A space B\n{}",
        "  A --> B\n".repeat(5)
    );
    let (model, mut layout, typography, meter) = placed(&source);
    let routes = prepare(&model, &mut layout, &typography, &meter).unwrap();
    assert_eq!(routes.len(), 5);
    assert!(routes.iter().all(Option::is_some));
    let mut retried_face = false;
    for from in [true, false] {
        let bounds = shape_box(&layout.shape_geometries[usize::from(!from)]);
        let ports: Vec<_> = layout
            .edges
            .iter()
            .map(|edge| {
                to_point(if from {
                    &edge.points[0]
                } else {
                    edge.points.last().unwrap()
                })
            })
            .collect();
        let facing = if from { bounds.x1 } else { bounds.x0 };
        retried_face |= ports.iter().any(|port| port.x != facing);
        for (i, port) in ports.iter().enumerate() {
            assert!(
                ports[..i]
                    .iter()
                    .all(|other| (*port - *other).hypot() >= 8.0),
                "ports {ports:?}"
            );
        }
    }
    assert!(retried_face);
}

#[test]
fn blocked_label_fallback_preserves_its_grid_path_and_marker_clearance() {
    let source = "block\n  columns 5\n  space:2 C space:2\n  space:5\n  A space X space B\n  C --\"Blocked\"--x X\n  A --> B\n";
    let (mut model, mut layout, typography, meter) = placed(source);
    let body = layout.nodes.iter().find(|node| node.id == "X").unwrap();
    let label = layout.edges[0].label.as_mut().unwrap();
    label.x = body.x;
    label.y = body.y;
    let original: Vec<_> = layout.edges[0].points.iter().map(to_point).collect();
    let geometries: HashMap<_, _> = layout
        .shape_geometries
        .iter()
        .map(|shape| (shape.id.as_str(), shape))
        .collect();
    let points = block_render_edge_points(&model.edges[0], &layout.edges[0], &geometries, None);
    let tip = to_point(points.rendered_points.last().unwrap());
    let footprint = Rect::new(tip.x - 5.8, tip.y - 12.0, tip.x + 5.8, tip.y - 1.0);
    let mut marked = layout.clone();
    let routes = prepare(&model, &mut marked, &typography, &meter).unwrap();
    assert!(routes[0].is_none());
    assert!(routes[1].is_some());
    assert_eq!(
        marked.edges[0]
            .points
            .iter()
            .map(to_point)
            .collect::<Vec<_>>(),
        original
    );
    let crosses_marker = |layout: &BlockDiagramLayout| {
        layout.edges[1].points.windows(2).any(|leg| {
            intersects(
                Rect::from_points(to_point(&leg[0]), to_point(&leg[1])),
                footprint,
            )
        })
    };
    assert!(!crosses_marker(&marked));
    model.edges[0].arrow_type_end = None;
    let routes = prepare(&model, &mut layout, &typography, &meter).unwrap();
    assert!(routes[1].is_some());
    assert!(crosses_marker(&layout));
}

#[test]
fn restoring_failed_routes_evicts_conflicts_without_stale_markers() {
    let (_, layout, _, meter) = placed("block\n  A B\n  A --> B\n");
    let make_route = |occurrence, offset| {
        let ends = [0, 1].map(|index| {
            let mut shape = layout.shape_geometries[index].clone();
            shape.allocated.y += offset;
            Terminal::new(&shape, shape_box(&shape), Side::Top, None, 2.0, None).unwrap()
        });
        let anchors = ends.each_ref().map(|end| end.anchor);
        Request {
            placement: EdgePlacement {
                index: 0,
                occurrence,
                bodies: [0, 1],
            },
            ends,
            label: None,
        }
        .solve(anchors.into_iter())
    };
    let middle = make_route(1, 50.0);
    let next = make_route(2, 100.0);
    let last = make_route(3, 150.0);
    let blocker = |route: &SolvedRequest, occurrence| {
        let point = knot_point(route.points[1]);
        Wall::marker(
            Rect::from_center_size(point, (4.0, 4.0)),
            occurrence,
            0,
            route.points[1],
        )
    };
    let grid = [
        [Some(blocker(&middle, 0)), None],
        [Some(blocker(&next, 1)), None],
        [None; 2],
        [Some(blocker(&last, 3)), None],
    ];
    let mut solved = vec![None, Some(middle), Some(next), Some(last)];
    let mut work = OperationLayoutWorkControl::new(Arc::clone(&meter));
    fallback::restore(&mut solved, &grid, 2.0, &mut work).unwrap();
    assert!(solved[1].is_none());
    assert!(solved[2].is_none());
    assert!(solved[3].is_some());
}

#[test]
fn off_center_circle_anchors_clear_the_inflated_bounding_box() {
    let shape = BlockShapeGeometry {
        id: "circle".into(),
        allocated: BlockAllocatedBounds {
            x: 40.0,
            y: 60.0,
            width: 80.0,
            height: 80.0,
        },
        boundary: BlockShapeBoundary::Circle {
            radius: 25.0,
            width_attribute: 50.0,
            height_attribute: 50.0,
        },
    };
    let margin = 2.0;
    let body = shape_box(&shape);
    let bounds = body.inflate(margin, margin);
    for (side, across) in [
        (Side::Top, 64.0),
        (Side::Right, 84.0),
        (Side::Bottom, 64.0),
        (Side::Left, 84.0),
    ] {
        let end = Terminal::new(&shape, body, side, None, margin, Some(across)).unwrap();
        let anchor = knot_point(end.anchor);
        let expected = match side {
            Side::Top => Point::new(across, bounds.y0.next_down()),
            Side::Right => Point::new(bounds.x1.next_up(), across),
            Side::Bottom => Point::new(across, bounds.y1.next_up()),
            Side::Left => Point::new(bounds.x0.next_down(), across),
        };
        assert_eq!(anchor, expected, "{side:?} anchor touches the padded box");
    }
}

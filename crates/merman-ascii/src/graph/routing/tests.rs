use super::plan::{PlannedRouteCell, PlannedRouteLabel, PlannedRoutePaint};
use super::*;
use crate::color::AsciiColorRole;
use crate::graph::layout::layout_graph;
use crate::graph::model::{GraphDirection, GraphEdgeAttrs, GraphEdgeStyle};
use crate::graph::routing::label::{RoutedLabelCatalog, RoutedLabelPlacement, RoutedLabelText};
use crate::graph::routing::plan::{MarkerAnchor, MarkerAnchors, PlannedCellId};
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
use crate::{AsciiRenderOptions, TerminalWidthProfile};
use merman_core::resources::ResourceProfile;

mod labels;
mod markers;
mod scene;

#[test]
fn edge_style_is_applied_to_route_plan_cells_and_labels() {
    let line = AsciiRgb::new(1, 2, 3);
    let arrow = AsciiRgb::new(4, 5, 6);
    let label = AsciiRgb::new(7, 8, 9);
    let plan = RoutePlan::new_without_markers_for_test(
        vec![
            planned_cell(0, 0, '-', PlannedRouteCellKind::EdgeLine),
            planned_cell(1, 0, '-', PlannedRouteCellKind::RouteCell),
            planned_cell(2, 0, '>', PlannedRouteCellKind::EdgeArrow),
        ],
        vec![PlannedRouteLabel::new(
            RoutedLabelText::new("label").expect("single-line label should exist"),
            RoutedLabelPlacement::new(0, 0, 5),
        )],
    );

    let mut canvas = RawCanvas::with_width_profile(5, 1, TerminalWidthProfile::Unicode);
    let mut route_cells = RouteCells::new();
    let mut drawing = RouteDrawing::new(&mut canvas, &mut route_cells);
    let plan = plan.with_style(GraphEdgeStyle {
        line: Some(line),
        arrow: Some(arrow),
        label: Some(label),
    });

    paint_route_plan(&mut drawing, &plan)
        .expect("test route should fit the unbounded resource policy");

    assert_eq!(
        canvas.get_color(0, 0),
        Some(crate::terminal::CanvasColor::Direct(line))
    );
    assert_eq!(
        canvas.get_color(1, 0),
        Some(crate::terminal::CanvasColor::Direct(line))
    );
    assert_eq!(
        canvas.get_color(2, 0),
        Some(crate::terminal::CanvasColor::Direct(arrow))
    );

    let scene = RouteScene {
        routes: vec![PreparedRoute::for_test(plan, 0)],
        extent: (5, 1),
        planned_cell_count: 3,
        labels: RoutedLabelCatalog::for_test(vec![Some(
            RoutedLabelText::new("label").expect("single-line label should exist"),
        )]),
    };
    scene
        .draw_labels(&mut canvas, RouteLabelTransform::Identity)
        .expect("test route label should fit the unbounded resource policy");

    assert_eq!(canvas.get_color(0, 0), Some(CanvasColor::Direct(label)));
}

#[test]
fn route_label_transform_mirrors_horizontal_label_placement() {
    let text = RoutedLabelText::new("north<br>south").expect("label should exist");
    let label = EdgeLabel {
        text: &text,
        placement: RoutedLabelPlacement::new(2, 4, 5),
        color: CanvasColor::Role(AsciiColorRole::EdgeLabel),
    };

    let transformed = RouteLabelTransform::HorizontalMirror { width: 20 }.apply(label);

    assert_eq!(transformed.text.lines(), ["north", "south"]);
    assert_eq!(transformed.placement, RoutedLabelPlacement::new(13, 4, 5));
}

#[test]
fn route_label_transform_preserves_vertical_mirrored_multiline_label_order() {
    let text = RoutedLabelText::new("north<br>south").expect("label should exist");
    let label = EdgeLabel {
        text: &text,
        placement: RoutedLabelPlacement::new(2, 4, 5),
        color: CanvasColor::Role(AsciiColorRole::EdgeLabel),
    };

    let transformed = RouteLabelTransform::VerticalMirror { height: 20 }.apply(label);

    assert_eq!(transformed.text.lines(), ["north", "south"]);
    assert_eq!(transformed.placement, RoutedLabelPlacement::new(2, 14, 5));
}

#[test]
fn edge_arrow_style_falls_back_to_line_style() {
    let line = AsciiRgb::new(10, 11, 12);
    let plan = RoutePlan::new_without_markers_for_test(
        vec![planned_cell(0, 0, '>', PlannedRouteCellKind::EdgeArrow)],
        Vec::new(),
    );

    let mut canvas = RawCanvas::with_width_profile(1, 1, TerminalWidthProfile::Unicode);
    let mut route_cells = RouteCells::new();
    let mut drawing = RouteDrawing::new(&mut canvas, &mut route_cells);

    paint_route_plan(
        &mut drawing,
        &plan.with_style(GraphEdgeStyle {
            line: Some(line),
            arrow: None,
            label: None,
        }),
    )
    .expect("test edge arrow should fit the unbounded resource policy");

    assert_eq!(
        canvas.get_color(0, 0),
        Some(crate::terminal::CanvasColor::Direct(line))
    );
}

#[test]
fn route_body_admission_rejects_reserved_node_cells_before_commit() {
    let options = AsciiRenderOptions::ascii();
    let graph_layout = simple_graph_layout(&options);
    let blocker = &graph_layout.nodes[0];
    let coord = CanvasCoord {
        x: blocker.center_x(),
        y: blocker.center_y(),
    };
    let cell = planned_cell(coord.x, coord.y, '-', PlannedRouteCellKind::EdgeLine);
    let anchor = MarkerAnchor::new(PlannedCellId::new(0), StepDirection::Right);
    let route = PreparedRoute::for_test_with_endpoints(
        RoutePlan::new(vec![cell], Vec::new(), MarkerAnchors::new(anchor, anchor)),
        0,
        "source",
        "target",
    );
    let mut resources = unbounded_resources();
    let mut occupancy =
        SceneOccupancy::try_new_for_routes(&graph_layout, 1, &mut resources).unwrap();

    assert!(
        !occupancy
            .try_admit_route(0, &route, &mut resources, "flowchart")
            .unwrap()
    );
    assert!(occupancy.route_cells.is_empty());
    assert!(occupancy.route_bounds.is_empty());
}

#[test]
fn route_scene_selects_an_alternate_route_when_the_primary_lane_is_reserved() {
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_edge("a", "b");
    graph.add_edge("a", "b");
    graph.add_edge("a", "b");
    let mut graph_layout = layout_graph(&graph, &options);
    let from = graph_layout.nodes[0].clone();
    let to = graph_layout.nodes[1].clone();
    let primary = plan_edge_route(EdgeRouteRequest {
        graph: &graph,
        graph_layout: &graph_layout,
        edges: &graph.edges,
        from: &from,
        to: &to,
        edge_index: 1,
        edge: &graph.edges[1],
        charset: &charset,
    })
    .unwrap();
    let primary_lane_y = primary
        .cells
        .iter()
        .map(|cell| cell.coord.y)
        .max()
        .expect("parallel route should have a bottom lane");
    let blocked_coord = primary
        .cells
        .iter()
        .find(|cell| {
            cell.coord.y == primary_lane_y
                && cell.coord.x > from.center_x()
                && cell.coord.x < to.center_x()
        })
        .expect("parallel route should have a horizontal lane cell")
        .coord;
    graph_layout.nodes.push(NodeLayout {
        id: "route-blocker".to_string(),
        label: GraphLabel::new(""),
        shape: GraphNodeShape::Rect,
        style: GraphNodeStyle::default(),
        grid: GridCoord { x: 0, y: 0 },
        x: blocked_coord.x,
        y: blocked_coord.y,
        width: 1,
        height: 1,
    });

    let scene = prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset)
        .expect("a bounded outer-lane candidate should avoid the reserved cell");

    assert_eq!(scene.routes.len(), 3);
    assert!(
        scene.routes[1]
            .plan
            .cells
            .iter()
            .all(|cell| cell.coord != blocked_coord)
    );
    assert_ne!(scene.routes[1].plan.cells, primary.cells);
}

fn marker_request_plan(marker: GraphEdgeMarker, length: usize) -> RoutePlan {
    marker_request_plan_at_y(marker, length, 0)
}

fn marker_request_plan_at_y(marker: GraphEdgeMarker, length: usize, y: usize) -> RoutePlan {
    let cells = (0..length)
        .map(|x| planned_cell(x, y, '-', PlannedRouteCellKind::EdgeLine))
        .collect::<Vec<_>>();
    let start = MarkerAnchor::new(PlannedCellId::new(0), StepDirection::Left);
    let end = MarkerAnchor::new(
        PlannedCellId::new(length.saturating_sub(1)),
        StepDirection::Right,
    );
    RoutePlan::new(cells, Vec::new(), MarkerAnchors::new(start, end))
        .with_marker_requests(GraphEdgeMarker::Open, marker, "flowchart")
        .unwrap()
}

fn vertical_route_plan_at_x(x: usize) -> RoutePlan {
    RoutePlan::new(
        (0..=2)
            .map(|y| planned_cell(x, y, '|', PlannedRouteCellKind::EdgeLine))
            .collect(),
        Vec::new(),
        MarkerAnchors::new(
            MarkerAnchor::new(PlannedCellId::new(0), StepDirection::Up),
            MarkerAnchor::new(PlannedCellId::new(2), StepDirection::Down),
        ),
    )
}

fn labeled_plan(x: usize, y: usize, text: &str) -> RoutePlan {
    RoutePlan::new_without_markers_for_test(
        vec![planned_cell(x, y + 1, '-', PlannedRouteCellKind::EdgeLine)],
        vec![PlannedRouteLabel::new(
            RoutedLabelText::new(text).unwrap(),
            RoutedLabelPlacement::new(x, y, text.len()),
        )],
    )
}

fn simple_graph_layout(options: &AsciiRenderOptions) -> GraphLayout {
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    layout_graph(&graph, options)
}

fn grouped_graph_layout(options: &AsciiRenderOptions) -> GraphLayout {
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_group_with_style(
        "group",
        "Group",
        None,
        vec!["a".to_string()],
        Default::default(),
    );
    layout_graph(&graph, options)
}

fn unbounded_resources() -> ResourceContext {
    ResourceContext::new(AsciiResourcePolicy::for_profile(
        ResourceProfile::UnboundedForTrustedInput,
    ))
}

fn allocate_test_marker_berths(
    routes: &mut [PreparedRoute],
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<()> {
    let options = AsciiRenderOptions::ascii();
    let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
    let mut occupancy = SceneOccupancy::try_new(routes, &graph_layout, resources, "flowchart")?;
    allocate_marker_berths(routes, &mut occupancy, charset, resources, "flowchart")
}

fn allocate_test_label_placements(
    routes: &mut [PreparedRoute],
    graph_layout: &GraphLayout,
    resources: &mut ResourceContext,
) -> Result<()> {
    let mut occupancy = SceneOccupancy::try_new(routes, graph_layout, resources, "flowchart")?;
    allocate_route_label_placements(routes, &mut occupancy, resources, "flowchart")
}

fn route_scene_signature(scene: &RouteScene) -> Vec<(String, String, RoutePlan)> {
    scene
        .routes
        .iter()
        .map(|route| {
            (
                route.owner.from.clone(),
                route.owner.to.clone(),
                route.plan.clone(),
            )
        })
        .collect()
}

fn planned_cell(x: usize, y: usize, ch: char, kind: PlannedRouteCellKind) -> PlannedRouteCell {
    PlannedRouteCell {
        coord: CanvasCoord { x, y },
        ch,
        kind,
        segment: PlannedRouteSegment::Direct,
        paint: PlannedRoutePaint::role(match kind {
            PlannedRouteCellKind::EdgeArrow => AsciiColorRole::EdgeArrow,
            PlannedRouteCellKind::EdgeLine | PlannedRouteCellKind::RouteCell => {
                AsciiColorRole::EdgeLine
            }
        }),
    }
}

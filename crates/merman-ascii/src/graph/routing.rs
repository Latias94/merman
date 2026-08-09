use super::charset::GraphCharset;
use super::label::GraphLabel;
use super::layout::{GraphLayout, GridCoord, GroupLayout, NodeLayout};
use super::model::{AsciiGraph, AsciiGraphEdge, GraphNodeShape, GraphNodeStyle};
use super::surface::GraphSurface;
use super::topology::GraphGroupTopology;
use crate::canvas::Canvas as RawCanvas;
use crate::canvas::CanvasColor;
use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use std::collections::HashMap;

mod cell;
mod label;
mod path;
mod plan;

pub(super) use cell::RouteCells;
use cell::{set_edge_cell_with_paint, set_route_cell_with_paint};
use label::{EdgeLabel, draw_routed_label};
#[cfg(test)]
use plan::plan_edge_route;
use plan::{
    EdgeRoutePlan, EdgeRouteRequest, PlannedRouteCellKind, RoutePlan, plan_edge_route_with_topology,
};

type Canvas<'surface> = dyn GraphSurface + 'surface;

pub(super) struct RouteDrawing<'a> {
    canvas: &'a mut Canvas<'a>,
    route_cells: &'a mut RouteCells,
}

impl<'a> RouteDrawing<'a> {
    pub(super) fn new(canvas: &'a mut Canvas<'a>, route_cells: &'a mut RouteCells) -> Self {
        Self {
            canvas,
            route_cells,
        }
    }
}

pub(super) struct RouteScene {
    routes: Vec<PreparedRoute>,
    extent: (usize, usize),
    planned_cell_count: usize,
}

struct PreparedRoute {
    plan: RoutePlan,
}

impl PreparedRoute {
    fn paint_body(&self, drawing: &mut RouteDrawing<'_>) -> Result<()> {
        paint_route_plan_body(drawing, &self.plan)
    }

    fn paint_markers(&self, drawing: &mut RouteDrawing<'_>) -> Result<()> {
        paint_route_plan_markers(drawing, &self.plan)
    }
}

impl RouteScene {
    pub(super) fn canvas_extent(&self) -> (usize, usize) {
        self.extent
    }

    pub(super) fn planned_cell_count(&self) -> usize {
        self.planned_cell_count
    }

    pub(super) fn paint_routes(&self, drawing: &mut RouteDrawing<'_>) -> Result<()> {
        for route in &self.routes {
            route.paint_body(drawing)?;
        }
        for route in &self.routes {
            route.paint_markers(drawing)?;
        }
        Ok(())
    }

    pub(super) fn draw_labels(
        &self,
        canvas: &mut RawCanvas,
        transform: RouteLabelTransform,
    ) -> Result<()> {
        for route in &self.routes {
            for label in &route.plan.labels {
                let label = transform.apply(EdgeLabel {
                    text: label.text.clone(),
                    placement: label.placement,
                    color: label.paint.color,
                });
                draw_routed_label(canvas, &label)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RouteLabelTransform {
    Identity,
    HorizontalMirror { width: usize },
    VerticalMirror { height: usize },
}

impl RouteLabelTransform {
    fn apply(self, label: EdgeLabel) -> EdgeLabel {
        match self {
            Self::Identity => label,
            Self::HorizontalMirror { width } => EdgeLabel {
                placement: label.placement.with_position(
                    width
                        .saturating_sub(label.placement.x())
                        .saturating_sub(label.placement.width()),
                    label.placement.y(),
                ),
                ..label
            },
            Self::VerticalMirror { height } => {
                let line_count = label.text.line_count();
                EdgeLabel {
                    text: label.text.reversed(),
                    placement: label.placement.with_position(
                        label.placement.x(),
                        height.saturating_sub(label.placement.y().saturating_add(line_count)),
                    ),
                    color: label.color,
                }
            }
        }
    }
}

pub(super) fn prepare_route_scene_with_resources(
    graph: &AsciiGraph,
    graph_layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<RouteScene> {
    let topology = if graph.groups.is_empty() {
        None
    } else {
        Some(GraphGroupTopology::try_new(graph, resources)?)
    };
    let mut routes = Vec::new();
    routes
        .try_reserve(edges.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    let mut width = 0;
    let mut height = 0;
    let mut planned_cell_count = 0usize;
    let marker_capacity = edges.len().checked_mul(2).ok_or_else(|| {
        resources
            .policy()
            .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
    })?;
    let mut marker_ownership = HashMap::new();
    marker_ownership
        .try_reserve(marker_capacity)
        .map_err(|_| AsciiError::AllocationFailed {
            phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    let route_scan_width = graph_layout
        .nodes
        .len()
        .checked_add(graph_layout.groups.len())
        .and_then(|count| count.checked_add(edges.len()))
        .ok_or_else(|| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
        })?;

    for (edge_index, edge) in edges.iter().enumerate() {
        if edge.stroke == super::model::GraphEdgeStroke::Invisible {
            continue;
        }
        resources.charge_layout_work(route_scan_width)?;
        let Some(from) = endpoint_layout(graph_layout, &edge.from, charset) else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "edges with missing endpoint layouts",
            });
        };
        let Some(to) = endpoint_layout(graph_layout, &edge.to, charset) else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "edges with missing endpoint layouts",
            });
        };
        let plan = match plan_edge_route_with_topology(
            EdgeRouteRequest {
                graph,
                graph_layout,
                edges,
                from: &from,
                to: &to,
                edge_index,
                edge,
                charset,
            },
            topology.as_ref(),
            resources,
        )? {
            EdgeRoutePlan::Routed(plan) => plan,
            EdgeRoutePlan::Unsupported(route) => {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: graph.diagram_type(),
                    feature: route.feature(),
                });
            }
        };

        let plan = plan
            .with_markers(
                edge.start_marker,
                edge.end_marker,
                charset,
                graph.diagram_type(),
            )?
            .with_style(edge.style);
        validate_marker_ownership(&plan, &mut marker_ownership, graph.diagram_type())?;
        planned_cell_count = planned_cell_count
            .checked_add(plan.cells.len())
            .ok_or_else(|| {
                resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
            })?;
        let (plan_width, plan_height) = plan.canvas_extent_with_resources(resources)?;
        width = width.max(plan_width);
        height = height.max(plan_height);
        routes.push(PreparedRoute { plan });
    }

    Ok(RouteScene {
        routes,
        extent: (width, height),
        planned_cell_count,
    })
}

#[cfg(test)]
pub(super) fn prepare_route_scene(
    graph: &AsciiGraph,
    graph_layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    charset: &GraphCharset,
) -> Result<RouteScene> {
    let mut resources = ResourceContext::new(crate::resource::AsciiResourcePolicy::for_profile(
        merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
    ));
    prepare_route_scene_with_resources(graph, graph_layout, edges, charset, &mut resources)
}

fn endpoint_layout(
    graph_layout: &GraphLayout,
    endpoint_id: &str,
    charset: &GraphCharset,
) -> Option<NodeLayout> {
    graph_layout
        .nodes
        .iter()
        .find(|layout| layout.id == endpoint_id)
        .cloned()
        .or_else(|| {
            graph_layout
                .groups
                .iter()
                .find(|layout| layout.id == endpoint_id)
                .map(|group| group_endpoint_layout(group, charset))
        })
}

fn group_endpoint_layout(group: &GroupLayout, charset: &GraphCharset) -> NodeLayout {
    NodeLayout {
        id: group.id.clone(),
        label: GraphLabel::new_with_profile("", charset.width_profile),
        shape: GraphNodeShape::Rect,
        style: GraphNodeStyle::default(),
        grid: GridCoord { x: 0, y: 0 },
        x: group.x,
        y: group.y,
        width: group.width,
        height: group.height,
    }
}

#[cfg(test)]
fn paint_route_plan(drawing: &mut RouteDrawing<'_>, plan: &RoutePlan) -> Result<()> {
    paint_route_plan_body(drawing, plan)?;
    paint_route_plan_markers(drawing, plan)
}

fn paint_route_plan_body(drawing: &mut RouteDrawing<'_>, plan: &RoutePlan) -> Result<()> {
    for cell in &plan.cells {
        match cell.kind {
            PlannedRouteCellKind::EdgeLine => set_edge_cell_with_paint(
                drawing.canvas,
                cell.coord.x,
                cell.coord.y,
                cell.ch,
                cell.paint.color,
            )?,
            PlannedRouteCellKind::RouteCell => set_route_cell_with_paint(
                drawing.canvas,
                drawing.route_cells,
                cell.coord.x,
                cell.coord.y,
                cell.ch,
                cell.paint.color,
            )?,
            PlannedRouteCellKind::EdgeArrow => {}
        }
    }
    Ok(())
}

fn paint_route_plan_markers(drawing: &mut RouteDrawing<'_>, plan: &RoutePlan) -> Result<()> {
    for cell in &plan.cells {
        if cell.kind == PlannedRouteCellKind::EdgeArrow {
            set_edge_cell_with_paint(
                drawing.canvas,
                cell.coord.x,
                cell.coord.y,
                cell.ch,
                cell.paint.color,
            )?;
        }
    }
    Ok(())
}

fn validate_marker_ownership(
    plan: &RoutePlan,
    occupancy: &mut HashMap<(usize, usize), (char, CanvasColor)>,
    diagram_type: &'static str,
) -> Result<()> {
    for cell in &plan.cells {
        if cell.kind != PlannedRouteCellKind::EdgeArrow {
            continue;
        }
        let marker = (cell.ch, cell.paint.color);
        match occupancy.get(&(cell.coord.x, cell.coord.y)) {
            Some(existing) if *existing != marker => {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type,
                    feature: "conflicting edge marker ownership",
                });
            }
            Some(_) => {}
            None => {
                occupancy.insert((cell.coord.x, cell.coord.y), marker);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::plan::{
        PlannedRouteCell, PlannedRouteLabel, PlannedRoutePaint, PlannedRouteSegment,
    };
    use super::*;
    use crate::color::{AsciiColorRole, AsciiRgb};
    use crate::graph::layout::CanvasCoord;
    use crate::graph::layout::layout_graph;
    use crate::graph::model::{GraphDirection, GraphEdgeAttrs, GraphEdgeStyle};
    use crate::graph::routing::label::{RoutedLabelPlacement, RoutedLabelText};
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use crate::{AsciiRenderOptions, TerminalWidthProfile};
    use merman_core::resources::ResourceProfile;

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
            routes: vec![PreparedRoute { plan }],
            extent: (5, 1),
            planned_cell_count: 3,
        };
        scene
            .draw_labels(&mut canvas, RouteLabelTransform::Identity)
            .expect("test route label should fit the unbounded resource policy");

        assert_eq!(canvas.get_color(0, 0), Some(CanvasColor::Direct(label)));
    }

    #[test]
    fn route_label_transform_mirrors_horizontal_label_placement() {
        let label = EdgeLabel {
            text: RoutedLabelText::new("north<br>south").expect("label should exist"),
            placement: RoutedLabelPlacement::new(2, 4, 5),
            color: CanvasColor::Role(AsciiColorRole::EdgeLabel),
        };

        let transformed = RouteLabelTransform::HorizontalMirror { width: 20 }.apply(label);

        assert_eq!(transformed.text.lines(), ["north", "south"]);
        assert_eq!(transformed.placement, RoutedLabelPlacement::new(13, 4, 5));
    }

    #[test]
    fn route_label_transform_reverses_vertical_mirrored_multiline_labels() {
        let label = EdgeLabel {
            text: RoutedLabelText::new("north<br>south").expect("label should exist"),
            placement: RoutedLabelPlacement::new(2, 4, 5),
            color: CanvasColor::Role(AsciiColorRole::EdgeLabel),
        };

        let transformed = RouteLabelTransform::VerticalMirror { height: 20 }.apply(label);

        assert_eq!(transformed.text.lines(), ["south", "north"]);
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
    fn conflicting_marker_ownership_is_rejected_instead_of_using_paint_order() {
        let first = RoutePlan::new_without_markers_for_test(
            vec![planned_cell(0, 0, 'o', PlannedRouteCellKind::EdgeArrow)],
            Vec::new(),
        );
        let second = RoutePlan::new_without_markers_for_test(
            vec![planned_cell(0, 0, 'x', PlannedRouteCellKind::EdgeArrow)],
            Vec::new(),
        );
        let mut occupancy = HashMap::new();
        validate_marker_ownership(&first, &mut occupancy, "flowchart").unwrap();

        let error = validate_marker_ownership(&second, &mut occupancy, "flowchart")
            .expect_err("different markers cannot own the same terminal cell");

        assert_eq!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "conflicting edge marker ownership",
            }
        );
    }

    #[test]
    fn edge_canvas_extent_accounts_for_boundary_grid_path_label_width() {
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_node("y", "Y");
        graph.add_group_with_style(
            "one",
            "LR Group",
            Some(GraphDirection::LeftRight),
            vec!["a".to_string(), "b".to_string()],
            Default::default(),
        );
        graph.add_edge("a", "b");
        graph.add_edge_with_attrs(
            "b",
            "y",
            GraphEdgeAttrs {
                label: Some("boundary label with enough width".to_string()),
                ..Default::default()
            },
        );
        let graph_layout = layout_graph(&graph, &options);
        let edge = &graph.edges[1];
        let from = endpoint_layout(&graph_layout, &edge.from, &charset)
            .expect("source layout should exist");
        let to =
            endpoint_layout(&graph_layout, &edge.to, &charset).expect("target layout should exist");
        let plan = plan_edge_route(EdgeRouteRequest {
            graph: &graph,
            graph_layout: &graph_layout,
            edges: &graph.edges,
            from: &from,
            to: &to,
            edge_index: 1,
            edge,
            charset: &charset,
        })
        .expect("boundary route should plan");
        let label = plan.labels.first().expect("boundary route should label");
        let (required_width, _) = label.placement.canvas_extent();

        let scene = prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset)
            .expect("boundary scene should render");
        let (edge_width, _) = scene.canvas_extent();

        assert!(
            edge_width >= required_width,
            "edge canvas extent should reserve boundary label width {required_width}, got {edge_width}; plan: {plan:?}"
        );
    }

    #[test]
    fn prepare_route_scene_reports_missing_endpoint_layouts_before_painting() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_edge("a", "missing");
        let options = AsciiRenderOptions::ascii();
        let graph_layout = layout_graph(&graph, &options);
        let charset = GraphCharset::for_options(&options);

        let error = match prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset) {
            Ok(_) => panic!("scene planning should fail on missing endpoint layouts"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "edges with missing endpoint layouts",
            }
        );
    }

    #[test]
    fn prepare_route_scene_tracks_canvas_extent_for_each_route_plan() {
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_node("c", "C");
        graph.add_edge("a", "b");
        graph.add_edge_with_attrs(
            "b",
            "c",
            GraphEdgeAttrs {
                label: Some("wide label".to_string()),
                ..Default::default()
            },
        );
        let graph_layout = layout_graph(&graph, &options);

        let scene = prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset)
            .expect("supported graph should produce a prepared route scene");

        let mut expected_width = 0;
        let mut expected_height = 0;
        for (edge_index, edge) in graph.edges.iter().enumerate() {
            let from = endpoint_layout(&graph_layout, &edge.from, &charset)
                .expect("source layout should exist");
            let to = endpoint_layout(&graph_layout, &edge.to, &charset)
                .expect("target layout should exist");
            let plan = plan_edge_route(EdgeRouteRequest {
                graph: &graph,
                graph_layout: &graph_layout,
                edges: &graph.edges,
                from: &from,
                to: &to,
                edge_index,
                edge,
                charset: &charset,
            })
            .expect("supported graph should route");
            let (plan_width, plan_height) = plan.canvas_extent();
            expected_width = expected_width.max(plan_width);
            expected_height = expected_height.max(plan_height);
        }

        assert_eq!(scene.canvas_extent(), (expected_width, expected_height));
    }

    #[test]
    fn overlapping_route_cells_accept_exact_work_limit_and_reject_max_minus_one() {
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_edge("a", "b");
        graph.add_edge("a", "b");
        let graph_layout = layout_graph(&graph, &options);
        let scene = prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset)
            .expect("overlapping routes should plan");
        let route_scan_width =
            graph_layout.nodes.len() + graph_layout.groups.len() + graph.edges.len();
        let exact = graph.edges.len() * route_scan_width + scene.planned_cell_count();
        assert!(exact > 1, "test graph should plan overlapping route cells");

        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact)
            .expect("exact layout-work limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        prepare_route_scene_with_resources(
            &graph,
            &graph_layout,
            &graph.edges,
            &charset,
            &mut exact_resources,
        )
        .expect("exact planned-cell work limit should pass");

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact - 1)
            .expect("max-minus-one layout-work limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        let error = match prepare_route_scene_with_resources(
            &graph,
            &graph_layout,
            &graph.edges,
            &charset,
            &mut below_resources,
        ) {
            Ok(_) => panic!("max-minus-one planned-cell work limit should fail"),
            Err(error) => error,
        };
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact);
        assert_eq!(details.max, exact - 1);
    }

    #[test]
    fn route_extent_reports_checked_cell_geometry_overflow() {
        let plan = RoutePlan::new_without_markers_for_test(
            vec![planned_cell(
                usize::MAX,
                0,
                '-',
                PlannedRouteCellKind::RouteCell,
            )],
            Vec::new(),
        );
        let resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ));

        let error = plan
            .canvas_extent_with_resources(&resources)
            .expect_err("overflowing route-cell geometry should fail");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a grid resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
        assert_eq!(details.actual, usize::MAX);
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
}

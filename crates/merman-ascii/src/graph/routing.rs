use super::charset::GraphCharset;
use super::label::GraphLabel;
use super::layout::{GraphLayout, GridCoord, GroupLayout, NodeLayout};
use super::model::{AsciiGraph, AsciiGraphEdge, GraphNodeShape, GraphNodeStyle};
use crate::canvas::Canvas;
use crate::error::{AsciiError, Result};

mod cell;
mod label;
mod path;
mod plan;

pub(super) use cell::RouteCells;
use cell::{set_edge_cell_with_paint, set_route_cell_with_paint};
pub(super) use label::{EdgeLabel, RoutedLabelPlacement, draw_routed_label};
use plan::{EdgeRouteRequest, PlannedRouteCellKind, RoutePlan, plan_edge_route};

pub(super) struct RouteDrawing<'a> {
    canvas: &'a mut Canvas,
    route_cells: &'a mut RouteCells,
    labels: &'a mut Vec<EdgeLabel>,
}

impl<'a> RouteDrawing<'a> {
    pub(super) fn new(
        canvas: &'a mut Canvas,
        route_cells: &'a mut RouteCells,
        labels: &'a mut Vec<EdgeLabel>,
    ) -> Self {
        Self {
            canvas,
            route_cells,
            labels,
        }
    }
}

pub(super) struct RouteScene {
    routes: Vec<PreparedRoute>,
    extent: (usize, usize),
}

struct PreparedRoute {
    plan: RoutePlan,
}

impl PreparedRoute {
    fn paint(&self, drawing: &mut RouteDrawing<'_>) {
        paint_route_plan(drawing, &self.plan);
    }
}

impl RouteScene {
    pub(super) fn canvas_extent(&self) -> (usize, usize) {
        self.extent
    }

    pub(super) fn paint(&self, drawing: &mut RouteDrawing<'_>) {
        for route in &self.routes {
            route.paint(drawing);
        }
    }
}

pub(super) fn prepare_route_scene(
    graph: &AsciiGraph,
    graph_layout: &GraphLayout,
    edges: &[AsciiGraphEdge],
    charset: &GraphCharset,
) -> Result<RouteScene> {
    let mut routes = Vec::with_capacity(edges.len());
    let mut width = 0;
    let mut height = 0;

    for (edge_index, edge) in edges.iter().enumerate() {
        let Some(from) = endpoint_layout(graph_layout, &edge.from) else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "edges with missing endpoint layouts",
            });
        };
        let Some(to) = endpoint_layout(graph_layout, &edge.to) else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "edges with missing endpoint layouts",
            });
        };
        let Some(plan) = plan_edge_route(EdgeRouteRequest {
            graph,
            graph_layout,
            edges,
            from: &from,
            to: &to,
            edge_index,
            edge,
            charset,
        }) else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: graph.diagram_type(),
                feature: "unroutable graph edges",
            });
        };

        let plan = plan.with_style(edge.style);
        let (plan_width, plan_height) = plan.canvas_extent();
        width = width.max(plan_width);
        height = height.max(plan_height);
        routes.push(PreparedRoute { plan });
    }

    Ok(RouteScene {
        routes,
        extent: (width, height),
    })
}

pub(super) fn transform_routed_label(
    label: &EdgeLabel,
    mut transform: impl FnMut(RoutedLabelPlacement, usize) -> RoutedLabelPlacement,
    reverse_lines: bool,
) -> EdgeLabel {
    EdgeLabel {
        text: if reverse_lines {
            label.text.reversed()
        } else {
            label.text.clone()
        },
        placement: transform(label.placement, label.text.line_count()),
        color: label.color,
    }
}

fn endpoint_layout(graph_layout: &GraphLayout, endpoint_id: &str) -> Option<NodeLayout> {
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
                .map(group_endpoint_layout)
        })
}

fn group_endpoint_layout(group: &GroupLayout) -> NodeLayout {
    NodeLayout {
        id: group.id.clone(),
        label: GraphLabel::new(""),
        shape: GraphNodeShape::Rect,
        style: GraphNodeStyle::default(),
        grid: GridCoord { x: 0, y: 0 },
        x: group.x,
        y: group.y,
        width: group.width,
        height: group.height,
    }
}

fn paint_route_plan(drawing: &mut RouteDrawing<'_>, plan: &RoutePlan) {
    for cell in &plan.cells {
        match cell.kind {
            PlannedRouteCellKind::EdgeLine | PlannedRouteCellKind::EdgeArrow => {
                set_edge_cell_with_paint(
                    drawing.canvas,
                    cell.coord.x,
                    cell.coord.y,
                    cell.ch,
                    cell.paint.color,
                )
            }
            PlannedRouteCellKind::RouteCell => set_route_cell_with_paint(
                drawing.canvas,
                drawing.route_cells,
                cell.coord.x,
                cell.coord.y,
                cell.ch,
                cell.paint.color,
            ),
        }
    }

    drawing
        .labels
        .extend(plan.labels.iter().map(|label| EdgeLabel {
            text: label.text.clone(),
            placement: label.placement,
            color: label.paint.color,
        }));
}

#[cfg(test)]
mod tests {
    use super::plan::{
        PlannedRouteCell, PlannedRouteLabel, PlannedRoutePaint, PlannedRouteSegment,
    };
    use super::*;
    use crate::AsciiRenderOptions;
    use crate::canvas::CanvasColor;
    use crate::color::{AsciiColorRole, AsciiRgb};
    use crate::graph::layout::CanvasCoord;
    use crate::graph::layout::layout_graph;
    use crate::graph::model::{GraphDirection, GraphEdgeAttrs, GraphEdgeStyle};
    use crate::graph::routing::label::RoutedLabelText;

    #[test]
    fn edge_style_is_applied_to_route_plan_cells_and_labels() {
        let line = AsciiRgb::new(1, 2, 3);
        let arrow = AsciiRgb::new(4, 5, 6);
        let label = AsciiRgb::new(7, 8, 9);
        let plan = RoutePlan::new(
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

        let mut canvas = Canvas::new(3, 1);
        let mut route_cells = RouteCells::new();
        let mut labels = Vec::new();
        let mut drawing = RouteDrawing::new(&mut canvas, &mut route_cells, &mut labels);

        paint_route_plan(
            &mut drawing,
            &plan.with_style(GraphEdgeStyle {
                line: Some(line),
                arrow: Some(arrow),
                label: Some(label),
            }),
        );

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
        assert_eq!(labels[0].color, CanvasColor::Direct(label));
    }

    #[test]
    fn transform_routed_label_reverses_vertical_mirrored_multiline_labels() {
        let label = EdgeLabel {
            text: RoutedLabelText::new("north<br>south").expect("label should exist"),
            placement: RoutedLabelPlacement::new(2, 4, 5),
            color: CanvasColor::Role(AsciiColorRole::EdgeLabel),
        };

        let transformed = transform_routed_label(
            &label,
            |placement, line_count| {
                placement.with_position(
                    20usize
                        .saturating_sub(placement.x())
                        .saturating_sub(placement.width()),
                    20usize.saturating_sub(placement.y().saturating_add(line_count)),
                )
            },
            true,
        );

        assert_eq!(transformed.text.lines(), ["south", "north"]);
        assert_eq!(transformed.placement, RoutedLabelPlacement::new(13, 14, 5));
    }

    #[test]
    fn edge_arrow_style_falls_back_to_line_style() {
        let line = AsciiRgb::new(10, 11, 12);
        let plan = RoutePlan::new(
            vec![planned_cell(0, 0, '>', PlannedRouteCellKind::EdgeArrow)],
            Vec::new(),
        );

        let mut canvas = Canvas::new(1, 1);
        let mut route_cells = RouteCells::new();
        let mut labels = Vec::new();
        let mut drawing = RouteDrawing::new(&mut canvas, &mut route_cells, &mut labels);

        paint_route_plan(
            &mut drawing,
            &plan.with_style(GraphEdgeStyle {
                line: Some(line),
                arrow: None,
                label: None,
            }),
        );

        assert_eq!(
            canvas.get_color(0, 0),
            Some(crate::terminal::CanvasColor::Direct(line))
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
        let from = endpoint_layout(&graph_layout, &edge.from).expect("source layout should exist");
        let to = endpoint_layout(&graph_layout, &edge.to).expect("target layout should exist");
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
            let from =
                endpoint_layout(&graph_layout, &edge.from).expect("source layout should exist");
            let to = endpoint_layout(&graph_layout, &edge.to).expect("target layout should exist");
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

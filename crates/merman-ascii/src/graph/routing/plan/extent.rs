use super::super::super::layout::NodeLayout;
use super::super::super::model::{AsciiGraph, AsciiGraphEdge, GraphDirection};
use super::boundary::edge_boundary_context;
use super::edges::parallel_edge_index;
use super::left_right::{
    left_right_back_edge_bottom_y, self_loop_bottom_y_for_edges, self_loop_right_x,
};
use super::top_down::top_down_back_edge_lane_x;
use crate::text::display_width;

pub(in crate::graph::routing) fn route_canvas_extent(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    _direction: GraphDirection,
) -> (usize, usize) {
    let mut width = 0;
    let mut height = 0;

    for edge in edges.iter().filter(|edge| edge.from == edge.to) {
        let Some(layout) = layouts.iter().find(|layout| layout.id == edge.from) else {
            continue;
        };
        width = width.max(self_loop_right_x(layouts, layout) + 1);
        height = height.max(self_loop_bottom_y_for_edges(layouts, edges, layout) + 1);
    }
    for (edge_index, edge) in edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| edge.from != edge.to)
    {
        let Some(from) = layouts.iter().find(|layout| layout.id == edge.from) else {
            continue;
        };
        let Some(to) = layouts.iter().find(|layout| layout.id == edge.to) else {
            continue;
        };
        match edge_boundary_context(graph, edge).direction().canonical() {
            GraphDirection::LeftRight => {
                if from.center_y() == to.center_y()
                    && (from.x > to.x || parallel_edge_index(edges, edge_index) > 0)
                {
                    width = width.max(from.center_x().max(to.center_x()) + 3);
                    height = height.max(left_right_back_edge_bottom_y(from) + 1);
                }
            }
            GraphDirection::TopDown => {
                if from.center_y() > to.center_y() {
                    let lane_x = top_down_back_edge_lane_x(from, to);
                    width = width.max(lane_x + 3);
                    if let Some(label) = edge.label.as_deref() {
                        let label_start = lane_x.saturating_sub(display_width(label) / 2);
                        width = width.max(label_start + display_width(label) + 1);
                    }
                }
            }
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        }
    }

    (width, height)
}

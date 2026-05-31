use super::super::charset::GraphCharset;
use super::super::layout::{CanvasCoord, NodeLayout};
use super::super::model::{AsciiGraphEdge, GraphDirection, GraphEdgeArrow};
use super::cell::edge_line_char;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RoutePlan {
    pub(super) cells: Vec<PlannedRouteCell>,
    pub(super) labels: Vec<PlannedRouteLabel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PlannedRouteCell {
    pub(super) coord: CanvasCoord,
    pub(super) ch: char,
    pub(super) kind: PlannedRouteCellKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlannedRouteCellKind {
    EdgeLine,
    RouteCell,
    EdgeArrow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PlannedRouteLabel {
    pub(super) start: CanvasCoord,
    pub(super) end: CanvasCoord,
    pub(super) text: String,
}

pub(super) fn plan_left_right_direct_route(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    if from.center_y() != to.center_y() || to.x <= from.right() + 1 {
        return None;
    }
    if !left_right_direct_route_is_clear(layouts, from, to) {
        return None;
    }

    let y = from.center_y();
    let start = from.right() + 1;
    let end = to.x - 1;
    let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let mut cells = Vec::new();
    if charset.unicode {
        cells.push(PlannedRouteCell {
            coord: CanvasCoord { x: from.right(), y },
            ch: charset.right_connector,
            kind: PlannedRouteCellKind::EdgeLine,
        });
    }
    for x in start..end {
        cells.push(PlannedRouteCell {
            coord: CanvasCoord { x, y },
            ch: line,
            kind: PlannedRouteCellKind::RouteCell,
        });
    }
    cells.push(PlannedRouteCell {
        coord: CanvasCoord { x: end, y },
        ch: match edge.arrow {
            GraphEdgeArrow::Open => line,
            GraphEdgeArrow::Point => charset.arrow_right,
        },
        kind: match edge.arrow {
            GraphEdgeArrow::Open => PlannedRouteCellKind::RouteCell,
            GraphEdgeArrow::Point => PlannedRouteCellKind::EdgeArrow,
        },
    });

    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord { x: start, y },
        CanvasCoord { x: end, y },
    )
    .into_iter()
    .collect();

    Some(RoutePlan { cells, labels })
}

pub(super) fn plan_top_down_direct_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    if to.y <= from.bottom() + 1 {
        return None;
    }

    let x = from.center_x();
    let start = from.bottom() + 1;
    let end = to.y - 1;
    let line = edge_line_char(edge, charset, GraphDirection::TopDown);
    let mut cells = Vec::new();
    cells.push(PlannedRouteCell {
        coord: CanvasCoord {
            x,
            y: from.bottom(),
        },
        ch: charset.down_connector,
        kind: PlannedRouteCellKind::EdgeLine,
    });
    for y in start..end {
        cells.push(PlannedRouteCell {
            coord: CanvasCoord { x, y },
            ch: line,
            kind: PlannedRouteCellKind::RouteCell,
        });
    }
    cells.push(PlannedRouteCell {
        coord: CanvasCoord { x, y: end },
        ch: match edge.arrow {
            GraphEdgeArrow::Open => line,
            GraphEdgeArrow::Point => charset.arrow_down,
        },
        kind: match edge.arrow {
            GraphEdgeArrow::Open => PlannedRouteCellKind::RouteCell,
            GraphEdgeArrow::Point => PlannedRouteCellKind::EdgeArrow,
        },
    });

    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord { x, y: start },
        CanvasCoord { x, y: end },
    )
    .into_iter()
    .collect();

    Some(RoutePlan { cells, labels })
}

fn left_right_direct_route_is_clear(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
) -> bool {
    let y = from.center_y();
    let start = from.right() + 1;
    let end = to.x - 1;
    layouts
        .iter()
        .filter(|layout| layout.id != from.id && layout.id != to.id)
        .all(|layout| {
            y < layout.y || y > layout.bottom() || end < layout.x || start > layout.right()
        })
}

fn planned_label(
    label: Option<&str>,
    start: CanvasCoord,
    end: CanvasCoord,
) -> Option<PlannedRouteLabel> {
    label
        .filter(|label| !label.is_empty())
        .map(|label| PlannedRouteLabel {
            start,
            end,
            text: label.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiRenderOptions;
    use crate::graph::label::GraphLabel;
    use crate::graph::layout::GridCoord;
    use crate::graph::model::{GraphEdgeStroke, GraphEdgeStyle, GraphNodeShape, GraphNodeStyle};

    #[test]
    fn left_right_direct_route_plans_ascii_line_arrow_and_label_without_connector() {
        let from = node("a", 0, 0, 5, 3);
        let to = node("b", 10, 0, 5, 3);
        let layouts = vec![from.clone(), to.clone()];
        let edge = edge(Some("label"), GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan = plan_left_right_direct_route(&layouts, &from, &to, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(5, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(6, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(7, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(8, 1, '-', PlannedRouteCellKind::RouteCell),
                cell(9, 1, '>', PlannedRouteCellKind::EdgeArrow),
            ]
        );
        assert_eq!(
            plan.labels,
            vec![PlannedRouteLabel {
                start: CanvasCoord { x: 5, y: 1 },
                end: CanvasCoord { x: 9, y: 1 },
                text: "label".to_string(),
            }]
        );
    }

    #[test]
    fn left_right_direct_route_plans_unicode_connector() {
        let from = node("a", 0, 0, 5, 3);
        let to = node("b", 10, 0, 5, 3);
        let layouts = vec![from.clone(), to.clone()];
        let edge = edge(None, GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::unicode());

        let plan = plan_left_right_direct_route(&layouts, &from, &to, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(4, 1, '├', PlannedRouteCellKind::EdgeLine),
                cell(5, 1, '─', PlannedRouteCellKind::RouteCell),
                cell(6, 1, '─', PlannedRouteCellKind::RouteCell),
                cell(7, 1, '─', PlannedRouteCellKind::RouteCell),
                cell(8, 1, '─', PlannedRouteCellKind::RouteCell),
                cell(9, 1, '►', PlannedRouteCellKind::EdgeArrow),
            ]
        );
        assert!(plan.labels.is_empty());
    }

    #[test]
    fn left_right_direct_open_route_plans_line_endpoint_without_arrow() {
        let from = node("a", 0, 0, 3, 3);
        let to = node("b", 6, 0, 3, 3);
        let layouts = vec![from.clone(), to.clone()];
        let edge = edge(None, GraphEdgeArrow::Open);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan = plan_left_right_direct_route(&layouts, &from, &to, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells.last(),
            Some(&cell(5, 1, '-', PlannedRouteCellKind::RouteCell))
        );
    }

    #[test]
    fn left_right_direct_route_rejects_blocked_same_row_path() {
        let from = node("a", 0, 0, 3, 3);
        let blocker = node("blocker", 5, 0, 3, 3);
        let to = node("b", 10, 0, 3, 3);
        let layouts = vec![from.clone(), blocker, to.clone()];
        let edge = edge(None, GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        assert!(plan_left_right_direct_route(&layouts, &from, &to, &edge, &charset).is_none());
    }

    #[test]
    fn top_down_direct_route_plans_connector_line_arrow_and_label() {
        let from = node("a", 2, 0, 5, 3);
        let to = node("b", 2, 6, 5, 3);
        let edge = edge(Some("label"), GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan = plan_top_down_direct_route(&from, &to, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells,
            vec![
                cell(4, 2, '-', PlannedRouteCellKind::EdgeLine),
                cell(4, 3, '|', PlannedRouteCellKind::RouteCell),
                cell(4, 4, '|', PlannedRouteCellKind::RouteCell),
                cell(4, 5, 'v', PlannedRouteCellKind::EdgeArrow),
            ]
        );
        assert_eq!(
            plan.labels,
            vec![PlannedRouteLabel {
                start: CanvasCoord { x: 4, y: 3 },
                end: CanvasCoord { x: 4, y: 5 },
                text: "label".to_string(),
            }]
        );
    }

    #[test]
    fn top_down_direct_open_route_plans_line_endpoint_without_arrow() {
        let from = node("a", 0, 0, 3, 3);
        let to = node("b", 0, 5, 3, 3);
        let edge = edge(None, GraphEdgeArrow::Open);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        let plan = plan_top_down_direct_route(&from, &to, &edge, &charset).unwrap();

        assert_eq!(
            plan.cells.last(),
            Some(&cell(1, 4, '|', PlannedRouteCellKind::RouteCell))
        );
        assert!(plan.labels.is_empty());
    }

    #[test]
    fn top_down_direct_route_rejects_adjacent_boxes() {
        let from = node("a", 0, 0, 3, 3);
        let to = node("b", 0, 3, 3, 3);
        let edge = edge(None, GraphEdgeArrow::Point);
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());

        assert!(plan_top_down_direct_route(&from, &to, &edge, &charset).is_none());
    }

    fn cell(x: usize, y: usize, ch: char, kind: PlannedRouteCellKind) -> PlannedRouteCell {
        PlannedRouteCell {
            coord: CanvasCoord { x, y },
            ch,
            kind,
        }
    }

    fn edge(label: Option<&str>, arrow: GraphEdgeArrow) -> AsciiGraphEdge {
        AsciiGraphEdge {
            from: "a".to_string(),
            to: "b".to_string(),
            label: label.map(ToOwned::to_owned),
            stroke: GraphEdgeStroke::Normal,
            arrow,
            length: 1,
            style: GraphEdgeStyle::default(),
        }
    }

    fn node(id: &str, x: usize, y: usize, width: usize, height: usize) -> NodeLayout {
        NodeLayout {
            id: id.to_string(),
            label: GraphLabel::new(id),
            shape: GraphNodeShape::Rect,
            style: GraphNodeStyle::default(),
            grid: GridCoord { x: 0, y: 0 },
            x,
            y,
            width,
            height,
        }
    }
}

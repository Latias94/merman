use super::label::GraphLabel;
use super::model::{AsciiGraph, GraphGroupKind, GraphGroupStyle, GraphNodeShape, GraphNodeStyle};
use crate::options::AsciiRenderOptions;
use std::collections::BTreeMap;

mod grid;
mod groups;

pub(crate) use self::grid::reserve_grid_spot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GraphLayout {
    pub(super) nodes: Vec<NodeLayout>,
    pub(super) groups: Vec<GroupLayout>,
    column_widths: BTreeMap<usize, usize>,
    row_heights: BTreeMap<usize, usize>,
    offset_x: usize,
    offset_y: usize,
}

impl GraphLayout {
    pub(super) fn grid_to_canvas(&self, coord: GridCoord) -> CanvasCoord {
        CanvasCoord {
            x: self.offset_x + grid::axis_position(&self.column_widths, coord.x),
            y: self.offset_y + grid::axis_position(&self.row_heights, coord.y),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NodeLayout {
    pub(super) id: String,
    pub(super) label: GraphLabel,
    pub(super) shape: GraphNodeShape,
    pub(super) style: GraphNodeStyle,
    pub(super) grid: GridCoord,
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

impl NodeLayout {
    pub(super) fn center_x(&self) -> usize {
        self.x + self.width / 2
    }

    pub(super) fn center_y(&self) -> usize {
        self.y + self.height / 2
    }

    pub(super) fn right(&self) -> usize {
        self.x + self.width - 1
    }

    pub(super) fn bottom(&self) -> usize {
        self.y + self.height - 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct GridCoord {
    pub(super) x: usize,
    pub(super) y: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CanvasCoord {
    pub(super) x: usize,
    pub(super) y: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GroupLayout {
    pub(super) id: String,
    pub(super) kind: GraphGroupKind,
    pub(super) title: GraphLabel,
    pub(super) style: GraphGroupStyle,
    pub(super) divider_span: Option<DividerSpan>,
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

impl GroupLayout {
    pub(super) fn right(&self) -> usize {
        self.x + self.width - 1
    }

    pub(super) fn bottom(&self) -> usize {
        self.y + self.height - 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DividerSpan {
    pub(super) x_start: usize,
    pub(super) x_end: usize,
}

pub(super) fn layout_graph(graph: &AsciiGraph, options: &AsciiRenderOptions) -> GraphLayout {
    let (mut nodes, column_widths, row_heights) = grid::layout_nodes(graph, options);
    let (group_offset_x, group_offset_y) = groups::subgraph_offsets(graph, &nodes);
    for node in &mut nodes {
        node.x += group_offset_x;
        node.y += group_offset_y;
    }
    let offset_x = nodes
        .first()
        .map(|node| {
            node.x
                .saturating_sub(grid::axis_position(&column_widths, node.grid.x))
        })
        .unwrap_or_default();
    let offset_y = nodes
        .first()
        .map(|node| {
            node.y
                .saturating_sub(grid::axis_position(&row_heights, node.grid.y))
        })
        .unwrap_or_default();
    let groups = groups::layout_groups(graph, &nodes);
    GraphLayout {
        nodes,
        groups,
        column_widths,
        row_heights,
        offset_x,
        offset_y,
    }
}

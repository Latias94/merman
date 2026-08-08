use super::label::GraphLabel;
use super::model::{AsciiGraph, GraphGroupKind, GraphGroupStyle, GraphNodeShape, GraphNodeStyle};
use super::topology::GraphGroupTopology;
use crate::error::Result;
use crate::options::AsciiRenderOptions;
use crate::resource::ResourceContext;
use std::collections::HashMap;

mod grid;
mod groups;

pub(crate) use self::grid::reserve_grid_spot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GraphLayout {
    pub(super) nodes: Vec<NodeLayout>,
    pub(super) groups: Vec<GroupLayout>,
    column_widths: HashMap<usize, usize>,
    row_heights: HashMap<usize, usize>,
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

pub(super) fn layout_graph_with_resources(
    graph: &AsciiGraph,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<GraphLayout> {
    grid::preflight_minimum_grid_extent(graph, options, resources)?;
    charge_graph_layout_work(graph, resources)?;
    let topology = if graph.groups.is_empty() {
        None
    } else {
        Some(GraphGroupTopology::try_new(graph, resources)?)
    };
    check_graph_nesting_depth(graph, topology.as_ref(), resources)?;
    let (mut nodes, column_widths, row_heights) =
        grid::layout_nodes(graph, options, topology.as_ref(), resources)?;
    let (group_offset_x, group_offset_y) = if graph.groups.is_empty() {
        (0, 0)
    } else {
        groups::subgraph_offsets(
            graph,
            &nodes,
            topology
                .as_ref()
                .expect("non-empty graph groups must have topology"),
            options.terminal_width_profile,
            resources,
        )?
    };
    for node in &mut nodes {
        node.x = resources.checked_grid_add(node.x, group_offset_x)?;
        node.y = resources.checked_grid_add(node.y, group_offset_y)?;
    }
    check_node_layout_extent(&nodes, resources)?;
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
    let groups = if graph.groups.is_empty() {
        Vec::new()
    } else {
        groups::layout_groups(
            graph,
            &nodes,
            topology
                .as_ref()
                .expect("non-empty graph groups must have topology"),
            options.terminal_width_profile,
            resources,
        )?
    };
    Ok(GraphLayout {
        nodes,
        groups,
        column_widths,
        row_heights,
        offset_x,
        offset_y,
    })
}

fn check_node_layout_extent(nodes: &[NodeLayout], resources: &ResourceContext) -> Result<()> {
    let mut width = 0;
    let mut height = 0;
    for node in nodes {
        width = width.max(resources.checked_grid_add(node.x, node.width)?);
        height = height.max(resources.checked_grid_add(node.y, node.height)?);
    }
    resources.grid_extent(width, height)?;
    Ok(())
}

fn check_graph_nesting_depth(
    graph: &AsciiGraph,
    topology: Option<&GraphGroupTopology<'_>>,
    resources: &mut ResourceContext,
) -> Result<()> {
    let Some(topology) = topology else {
        return Ok(());
    };
    for group_index in 0..graph.groups.len() {
        topology.group_depth(group_index, resources)?;
    }
    Ok(())
}

fn charge_graph_layout_work(graph: &AsciiGraph, resources: &mut ResourceContext) -> Result<()> {
    let graph_items = resources.checked_work_add(
        resources.checked_work_add(graph.nodes.len(), graph.edges.len())?,
        graph.groups.len(),
    )?;
    resources.charge_layout_work(graph_items)?;

    let adjacency_scans = resources.checked_work_mul(graph.nodes.len(), graph.edges.len())?;
    resources.charge_layout_work(adjacency_scans)?;

    let group_scan_width = resources.checked_work_add(
        resources.checked_work_add(graph.nodes.len(), graph.edges.len())?,
        graph.groups.len(),
    )?;
    let group_scans = resources.checked_work_mul(graph.groups.len(), group_scan_width)?;
    resources.charge_layout_work(group_scans)
}

#[cfg(test)]
pub(super) fn layout_graph(graph: &AsciiGraph, options: &AsciiRenderOptions) -> GraphLayout {
    let mut resources = ResourceContext::new(crate::resource::AsciiResourcePolicy::for_profile(
        merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
    ));
    layout_graph_with_resources(graph, options, &mut resources)
        .expect("test graph layout work must remain representable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::model::GraphDirection;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;

    #[test]
    fn nested_graph_groups_accept_exact_depth_and_reject_max_minus_one() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_group_with_style(
            "inner",
            "Inner",
            None,
            vec!["a".to_string()],
            GraphGroupStyle::default(),
        );
        graph.add_group_with_style(
            "outer",
            "Outer",
            None,
            vec!["inner".to_string()],
            GraphGroupStyle::default(),
        );
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxNestingDepth, 2)
            .expect("exact nesting limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        let exact_topology = GraphGroupTopology::try_new(&graph, &mut exact_resources)
            .expect("exact topology construction should pass");
        check_graph_nesting_depth(&graph, Some(&exact_topology), &mut exact_resources)
            .expect("exact nesting limit should pass");

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxNestingDepth, 1)
            .expect("max-minus-one nesting limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        let below_topology = GraphGroupTopology::try_new(&graph, &mut below_resources)
            .expect("max-minus-one topology construction should pass");
        let error = check_graph_nesting_depth(&graph, Some(&below_topology), &mut below_resources)
            .expect_err("max-minus-one nesting limit should fail");
        let crate::AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a nesting resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxNestingDepth);
        assert_eq!(details.actual, 2);
        assert_eq!(details.max, 1);
    }

    #[test]
    fn disconnected_graph_grid_preflight_is_exact_and_precedes_layout_work() {
        const NODE_COUNT: usize = 1024;
        const MINIMUM_CELLS_PER_NODE: usize = 9;

        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        for index in 0..NODE_COUNT {
            graph.add_node(format!("node-{index}"), "Node");
        }
        let options = AsciiRenderOptions::unicode();
        let minimum_cells = NODE_COUNT * MINIMUM_CELLS_PER_NODE;
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxGridCells, minimum_cells)
            .expect("exact grid limit should be valid");
        let exact_resources = ResourceContext::new(exact_policy);
        grid::preflight_minimum_grid_extent(&graph, &options, &exact_resources)
            .expect("the exact minimum grid extent should pass preflight");
        assert_eq!(exact_resources.layout_work_used(), 0);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxGridCells, minimum_cells - 1)
            .expect("max-minus-one grid limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        let error = layout_graph_with_resources(&graph, &options, &mut below_resources)
            .expect_err("max-minus-one grid limit should fail before layout allocation");
        let crate::AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a grid resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
        assert_eq!(details.actual, minimum_cells);
        assert_eq!(details.max, minimum_cells - 1);
        assert_eq!(below_resources.layout_work_used(), 0);
    }
}

use super::label::GraphLabel;
use super::model::{AsciiGraph, GraphGroupKind, GraphGroupStyle, GraphNodeShape, GraphNodeStyle};
use super::topology::GraphGroupTopology;
use crate::error::Result;
use crate::operation::AsciiExecution;
#[cfg(test)]
use crate::options::AsciiRenderOptions;
use crate::resource::{LogicalExtent, ResourceContext};

mod grid;
mod groups;

/// Additional coarse-grid coordinates available to bounded graph routing.
pub(super) const ROUTING_GRID_MARGIN: usize = 6;

pub(super) fn charge_sort_work(len: usize, resources: &ResourceContext) -> Result<()> {
    let comparison_height = if len <= 1 {
        1
    } else {
        usize::BITS as usize - (len - 1).leading_zeros() as usize
    };
    resources.charge_layout_work(resources.checked_work_mul(len, comparison_height)?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GraphLayout {
    pub(super) nodes: Vec<NodeLayout>,
    pub(super) groups: Vec<GroupLayout>,
    /// Group indices ordered from containing backgrounds to nested backgrounds.
    pub(super) group_background_order: Vec<usize>,
    column_widths: grid::AxisProjection,
    row_heights: grid::AxisProjection,
    offset_x: usize,
    offset_y: usize,
}

impl GraphLayout {
    pub(super) fn grid_to_canvas(&self, coord: GridCoord) -> CanvasCoord {
        CanvasCoord {
            x: self.offset_x + self.column_widths.position(coord.x),
            y: self.offset_y + self.row_heights.position(coord.y),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[cfg(test)]
pub(super) fn layout_graph_with_resources(
    graph: &AsciiGraph,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<GraphLayout> {
    let policy = resources.policy();
    let layout_policy = options.flowchart_layout().graph_policy();
    layout_graph_with_resources_and_execution(
        graph,
        &layout_policy,
        resources,
        AsciiExecution::for_test(&policy),
    )
}

pub(super) fn layout_graph_with_resources_and_execution(
    graph: &AsciiGraph,
    layout_policy: &crate::options::GraphLayoutPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<GraphLayout> {
    execution.rebind_resource_context(resources, merman_core::OperationPhase::Layout);
    checkpoint_layout(execution)?;
    grid::preflight_minimum_grid_extent(graph, layout_policy, resources)?;
    charge_graph_layout_work(graph, resources)?;
    checkpoint_layout(execution)?;
    let label_plans =
        grid::plan_node_labels(graph, layout_policy.terminal_width_profile, resources)?;
    let topology = if graph.groups.is_empty() {
        None
    } else {
        Some(GraphGroupTopology::try_new(graph, resources)?)
    };
    check_graph_nesting_depth(graph, topology.as_ref(), resources)?;
    checkpoint_layout(execution)?;
    let (mut nodes, column_widths, row_heights) = grid::layout_nodes(
        graph,
        layout_policy,
        topology.as_ref(),
        &label_plans,
        resources,
        execution,
    )?;
    checkpoint_layout(execution)?;
    let (laid_out_groups, group_offset_x, group_offset_y) = if graph.groups.is_empty() {
        (
            groups::LaidOutGroups {
                items: Vec::new(),
                background_order: Vec::new(),
            },
            0,
            0,
        )
    } else {
        groups::layout_scene_groups(
            graph,
            &mut nodes,
            topology
                .as_ref()
                .expect("non-empty graph groups must have topology"),
            layout_policy,
            resources,
            execution,
        )?
    };
    let offset_x = nodes
        .first()
        .map(|node| node.x.saturating_sub(column_widths.position(node.grid.x)))
        .unwrap_or(group_offset_x);
    let offset_y = nodes
        .first()
        .map(|node| node.y.saturating_sub(row_heights.position(node.grid.y)))
        .unwrap_or(group_offset_y);
    checkpoint_layout(execution)?;
    let groups = laid_out_groups.items;
    graph_canvas_extent(&nodes, &groups, 0, 0, resources)?;
    grid::materialize_node_labels(&mut nodes, graph, &label_plans, resources)?;
    checkpoint_layout(execution)?;
    Ok(GraphLayout {
        nodes,
        groups,
        group_background_order: laid_out_groups.background_order,
        column_widths,
        row_heights,
        offset_x,
        offset_y,
    })
}

fn checkpoint_layout(execution: AsciiExecution<'_>) -> Result<()> {
    execution.checkpoint(merman_core::OperationPhase::Layout)
}

pub(super) fn graph_canvas_extent(
    nodes: &[NodeLayout],
    groups: &[GroupLayout],
    edge_width: usize,
    edge_height: usize,
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let mut width = edge_width;
    let mut height = edge_height;
    for node in nodes {
        width = width.max(resources.checked_grid_add(node.x, node.width)?);
        height = height.max(resources.checked_grid_add(node.y, node.height)?);
    }
    for group in groups {
        width = width.max(resources.checked_grid_add(group.x, group.width)?);
        height = height.max(resources.checked_grid_add(group.y, group.height)?);
    }
    resources.grid_extent(width, height)
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
    // Charge caller-owned graph projection once. Dagre ranking now meters its own adjacency,
    // cycle-breaking, nesting, and ranker work through the shared ResourceContext, so the former
    // coarse `nodes × edges` surcharge would double-count real work and reject sparse large graphs
    // before the source-backed algorithm can apply its bounded tranche accounting.
    let graph_items = resources.checked_work_add(
        resources.checked_work_add(graph.nodes.len(), graph.edges.len())?,
        graph.groups.len(),
    )?;
    resources.charge_layout_work(graph_items)?;

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
    use crate::graph::model::{GraphDirection, GraphEdgeAttrs, GraphEdgeStroke, GraphGroupStyle};
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;
    use merman_core::{CancelReason, OperationControl, OperationPhase};

    fn compact_two_groups(direction: GraphDirection, nested: bool) -> GraphLayout {
        let mut graph = AsciiGraph::new(direction);
        for (id, label) in [
            ("L", "Lint"),
            ("T", "Test"),
            ("S", "Staging"),
            ("P", "Prod"),
        ] {
            graph.add_node(id, label);
        }
        for (from, to) in [("L", "T"), ("T", "S"), ("S", "P")] {
            graph.add_edge(from, to);
        }
        graph.add_group_with_style(
            "CI",
            "CI",
            None,
            vec!["L".into(), "T".into()],
            GraphGroupStyle::default(),
        );
        graph.add_group_with_style(
            "Deploy",
            "Deploy",
            None,
            vec!["S".into(), "P".into()],
            GraphGroupStyle::default(),
        );
        if nested {
            graph.add_group_with_style(
                "CI outer",
                "CI outer",
                None,
                vec!["CI".into()],
                GraphGroupStyle::default(),
            );
            graph.add_group_with_style(
                "Deploy outer",
                "Deploy outer",
                None,
                vec!["Deploy".into()],
                GraphGroupStyle::default(),
            );
        }
        let mut options = AsciiRenderOptions::unicode();
        options.layout_profile = crate::AsciiLayoutProfile::Compact;
        layout_graph(&graph, &options)
    }

    fn assert_contains(group: &GroupLayout, x: usize, y: usize, right: usize, bottom: usize) {
        assert!(
            group.x < x && right < group.right(),
            "horizontal frame containment: {group:?}"
        );
        assert!(
            group.y < y && bottom < group.bottom(),
            "vertical frame containment: {group:?}"
        );
    }

    #[test]
    fn compact_sibling_frames_expand_the_shared_member_gap_only() {
        let layout = compact_two_groups(GraphDirection::LeftRight, false);
        let ci = &layout.groups[0];
        let deploy = &layout.groups[1];
        assert_eq!(layout.nodes[1].x - layout.nodes[0].right() - 1, 3);
        assert_eq!(layout.nodes[3].x - layout.nodes[2].right() - 1, 3);
        assert_eq!(layout.nodes[2].x - layout.nodes[1].right() - 1, 5);
        assert_eq!(deploy.x - ci.right() - 1, 1);
        assert_eq!(deploy.right() + 1, 50);
        for (node, group) in layout.nodes.iter().zip([ci, ci, deploy, deploy]) {
            assert_contains(group, node.x, node.y, node.right(), node.bottom());
        }
    }

    #[test]
    fn nested_group_overhang_preserves_member_containment_and_sibling_gutters() {
        for direction in [
            GraphDirection::LeftRight,
            GraphDirection::RightLeft,
            GraphDirection::TopDown,
            GraphDirection::BottomTop,
        ] {
            let layout = compact_two_groups(direction, true);
            let left = &layout.groups[2];
            let right = &layout.groups[3];
            match direction.canonical() {
                GraphDirection::LeftRight => assert!(left.right() + 1 < right.x),
                GraphDirection::TopDown => assert!(left.bottom() + 1 < right.y),
                GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
            }
            for (outer, inner) in [(left, &layout.groups[0]), (right, &layout.groups[1])] {
                assert_contains(outer, inner.x, inner.y, inner.right(), inner.bottom());
            }
            for (node, group) in layout.nodes.iter().zip([
                &layout.groups[0],
                &layout.groups[0],
                &layout.groups[1],
                &layout.groups[1],
            ]) {
                assert_contains(group, node.x, node.y, node.right(), node.bottom());
            }
        }
    }

    #[test]
    fn nested_empty_sibling_layout_keeps_complete_children_inside_parent() {
        for direction in [
            GraphDirection::LeftRight,
            GraphDirection::RightLeft,
            GraphDirection::TopDown,
            GraphDirection::BottomTop,
        ] {
            for profile in [
                crate::AsciiLayoutProfile::Canonical,
                crate::AsciiLayoutProfile::Compact,
            ] {
                for recursive_empty in [false, true] {
                    for empty_first in [false, true] {
                        let mut graph = AsciiGraph::new(direction);
                        graph.add_node("A", "AAA");
                        graph.add_node("B", "BBB");
                        graph.add_edge("A", "B");
                        let group_style = GraphGroupStyle::default();
                        if empty_first {
                            graph.add_group_with_style("E", "E", None, Vec::new(), group_style);
                        }
                        let mut populated_members = vec!["A".into()];
                        if recursive_empty {
                            graph.add_group_with_style("F", "F", None, Vec::new(), group_style);
                            populated_members.push("F".into());
                        }
                        graph.add_group_with_style("X", "X", None, populated_members, group_style);
                        if !empty_first {
                            graph.add_group_with_style("E", "E", None, Vec::new(), group_style);
                        }
                        let children = if empty_first {
                            vec!["E".into(), "X".into()]
                        } else {
                            vec!["X".into(), "E".into()]
                        };
                        graph.add_group_with_style("G1", "G1", None, children, group_style);
                        graph.add_group_with_style("G2", "G2", None, vec!["B".into()], group_style);
                        let options = AsciiRenderOptions::unicode().with_layout_profile(profile);
                        let layout = layout_graph(&graph, &options);
                        let group =
                            |id: &str| layout.groups.iter().find(|group| group.id == id).unwrap();
                        let populated = group("X");
                        let empty = group("E");
                        let parent = group("G1");
                        for child in [populated, empty] {
                            assert_contains(
                                parent,
                                child.x,
                                child.y,
                                child.right(),
                                child.bottom(),
                            );
                        }
                        assert!(
                            populated.right() + 1 < empty.x
                                || empty.right() + 1 < populated.x
                                || populated.bottom() + 1 < empty.y
                                || empty.bottom() + 1 < populated.y,
                            "sibling gutter must precede routing: {direction:?} {profile:?}: {layout:?}"
                        );
                        if recursive_empty {
                            let nested_empty = group("F");
                            assert_contains(
                                populated,
                                nested_empty.x,
                                nested_empty.y,
                                nested_empty.right(),
                                nested_empty.bottom(),
                            );
                        }
                        let a = &layout.nodes[0];
                        assert_contains(populated, a.x, a.y, a.right(), a.bottom());
                        let next = group("G2");
                        match direction.canonical() {
                            GraphDirection::LeftRight => assert!(parent.right() + 1 < next.x),
                            GraphDirection::TopDown => assert!(parent.bottom() + 1 < next.y),
                            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn scoped_empty_subtrees_keep_node_projection_and_unrelated_frame_gutters() {
        for direction in [
            GraphDirection::LeftRight,
            GraphDirection::RightLeft,
            GraphDirection::TopDown,
            GraphDirection::BottomTop,
        ] {
            for profile in [
                crate::AsciiLayoutProfile::Canonical,
                crate::AsciiLayoutProfile::Compact,
            ] {
                for zero_padding in [false, true] {
                    let mut graph = AsciiGraph::new(direction);
                    for node in ["A", "B", "C"] {
                        graph.add_node(node, node);
                    }
                    let style = GraphGroupStyle::default();
                    graph.add_group_with_style("E", "Empty", None, Vec::new(), style);
                    graph.add_group_with_style(
                        "L",
                        "Left",
                        None,
                        vec!["A".into(), "E".into()],
                        style,
                    );
                    graph.add_group_with_style("R", "Right", None, vec!["B".into()], style);
                    let mut options = AsciiRenderOptions::unicode().with_layout_profile(profile);
                    if zero_padding {
                        options = options
                            .with_node_padding_x(0)
                            .with_node_padding_y(0)
                            .with_graph_padding_x(0)
                            .with_graph_padding_y(0);
                    }
                    let layout = layout_graph(&graph, &options);
                    for node in &layout.nodes {
                        let projected = layout.grid_to_canvas(node.grid);
                        assert_eq!(
                            (projected.x, projected.y),
                            (node.x, node.y),
                            "scope placement must preserve the routing projection contract"
                        );
                    }
                    let left = layout.groups.iter().find(|group| group.id == "L").unwrap();
                    let right = layout.groups.iter().find(|group| group.id == "R").unwrap();
                    let empty = layout.groups.iter().find(|group| group.id == "E").unwrap();
                    assert_contains(left, empty.x, empty.y, empty.right(), empty.bottom());
                    assert!(
                        left.right() + 1 < right.x
                            || right.right() + 1 < left.x
                            || left.bottom() + 1 < right.y
                            || right.bottom() + 1 < left.y,
                        "unrelated complete frames need a reading gutter: {layout:?}"
                    );
                    let outside = layout.nodes.iter().find(|node| node.id == "C").unwrap();
                    for group in [left, right] {
                        assert!(
                            group.right() + 1 < outside.x
                                || outside.right() + 1 < group.x
                                || group.bottom() + 1 < outside.y
                                || outside.bottom() + 1 < group.y,
                            "outside nodes must not be swallowed by a complete subtree envelope: {layout:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn graph_layout_cancellation_precedes_grid_and_work_admission() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("a", "A");
        let options = AsciiRenderOptions::unicode();
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 1)
            .expect("one grid cell should be a valid limit")
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("one work unit should be a valid limit");
        let mut resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel();
        let layout_policy = options.flowchart_layout().graph_policy();

        let error = layout_graph_with_resources_and_execution(
            &graph,
            &layout_policy,
            &mut resources,
            AsciiExecution::new(&control, &policy),
        )
        .expect_err("cancellation should win before graph resource admission");

        assert!(matches!(
            error,
            crate::AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }

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
        grid::preflight_minimum_grid_extent(
            &graph,
            &options.flowchart_layout().graph_policy(),
            &exact_resources,
        )
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

    fn node_grid(layout: &GraphLayout, id: &str) -> GridCoord {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map(|node| node.grid)
            .unwrap_or_else(|| panic!("missing layout node {id}"))
    }

    #[test]
    fn dagre_ranks_out_of_order_chain_by_connectivity_not_declaration_order() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("c", "C");
        graph.add_node("b", "B");
        graph.add_node("a", "A");
        graph.add_edge("a", "b");
        graph.add_edge("b", "c");

        let layout = layout_graph(&graph, &AsciiRenderOptions::unicode());
        let a = node_grid(&layout, "a");
        let b = node_grid(&layout, "b");
        let c = node_grid(&layout, "c");

        assert!(a.y < b.y, "A must rank before B: {a:?} vs {b:?}");
        assert!(b.y < c.y, "B must rank before C: {b:?} vs {c:?}");
    }

    #[test]
    fn dagre_rank_plan_honors_terminal_edge_minlen_on_both_axes() {
        fn physical_span(direction: GraphDirection, length: usize) -> usize {
            let mut graph = AsciiGraph::new(direction);
            graph.add_node("a", "A");
            graph.add_node("b", "B");
            graph.add_edge_with_attrs(
                "a",
                "b",
                GraphEdgeAttrs {
                    length,
                    ..GraphEdgeAttrs::default()
                },
            );
            let layout = layout_graph(&graph, &AsciiRenderOptions::unicode());
            let a = layout.nodes.iter().find(|node| node.id == "a").unwrap();
            let b = layout.nodes.iter().find(|node| node.id == "b").unwrap();
            match direction.canonical() {
                GraphDirection::LeftRight => b.x - a.x,
                GraphDirection::TopDown => b.y - a.y,
                GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
            }
        }

        for direction in [
            GraphDirection::LeftRight,
            GraphDirection::RightLeft,
            GraphDirection::TopDown,
            GraphDirection::BottomTop,
        ] {
            assert!(
                physical_span(direction, 3) > physical_span(direction, 1),
                "edge minlen must expand the canonical terminal axis for {direction:?}"
            );
        }
    }

    #[test]
    fn dagre_rank_plan_honors_minlen_inside_perpendicular_group_direction_overrides() {
        fn physical_span(
            graph_direction: GraphDirection,
            group_direction: GraphDirection,
            length: usize,
        ) -> usize {
            let mut graph = AsciiGraph::new(graph_direction);
            graph.add_node("a", "A");
            graph.add_node("b", "B");
            graph.add_group_with_style(
                "group",
                "Group",
                Some(group_direction),
                vec!["a".to_string(), "b".to_string()],
                GraphGroupStyle::default(),
            );
            graph.add_edge_with_attrs(
                "a",
                "b",
                GraphEdgeAttrs {
                    length,
                    ..GraphEdgeAttrs::default()
                },
            );
            let layout = layout_graph(&graph, &AsciiRenderOptions::unicode());
            let a = layout.nodes.iter().find(|node| node.id == "a").unwrap();
            let b = layout.nodes.iter().find(|node| node.id == "b").unwrap();
            match group_direction.canonical() {
                GraphDirection::LeftRight => b.x - a.x,
                GraphDirection::TopDown => b.y - a.y,
                GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
            }
        }

        for (graph_direction, group_direction) in [
            (GraphDirection::TopDown, GraphDirection::LeftRight),
            (GraphDirection::LeftRight, GraphDirection::TopDown),
        ] {
            assert!(
                physical_span(graph_direction, group_direction, 3)
                    > physical_span(graph_direction, group_direction, 1),
                "local {group_direction:?} minlen must survive global {graph_direction:?} layout"
            );
        }
    }

    #[test]
    fn dagre_ranks_are_invariant_to_equivalent_edge_permutations() {
        fn graph_with_edges(edges: &[(&str, &str)]) -> AsciiGraph {
            let mut graph = AsciiGraph::new(GraphDirection::TopDown);
            for id in ["a", "b", "c", "d"] {
                graph.add_node(id, id.to_uppercase());
            }
            for (from, to) in edges {
                graph.add_edge(*from, *to);
            }
            graph
        }

        fn ranks(graph: &AsciiGraph) -> Vec<(String, usize)> {
            let layout = layout_graph(graph, &AsciiRenderOptions::unicode());
            let mut ranks = layout
                .nodes
                .into_iter()
                .map(|node| (node.id, node.grid.y))
                .collect::<Vec<_>>();
            ranks.sort_by(|left, right| left.0.cmp(&right.0));
            ranks
        }

        let first = graph_with_edges(&[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")]);
        let second = graph_with_edges(&[("c", "d"), ("a", "c"), ("b", "d"), ("a", "b")]);

        assert_eq!(ranks(&first), ranks(&second));
    }

    #[test]
    fn dagre_cycle_breaking_is_stable_and_keeps_original_nodes_ranked() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        for id in ["a", "b", "c"] {
            graph.add_node(id, id.to_uppercase());
        }
        graph.add_edge("a", "b");
        graph.add_edge("b", "c");
        graph.add_edge("c", "a");

        let first = layout_graph(&graph, &AsciiRenderOptions::unicode());
        let second = layout_graph(&graph, &AsciiRenderOptions::unicode());
        let first_grids = first
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node.grid))
            .collect::<Vec<_>>();
        let second_grids = second
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node.grid))
            .collect::<Vec<_>>();

        assert_eq!(first_grids, second_grids);
        assert!(first.nodes.iter().any(|node| node.grid.y > 0));
    }

    #[test]
    fn dagre_cycle_feedback_edge_minlen_survives_restored_terminal_direction() {
        fn physical_span(feedback_length: usize) -> usize {
            let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
            for id in ["a", "b", "c"] {
                graph.add_node(id, id.to_uppercase());
            }
            graph.add_edge("a", "b");
            graph.add_edge("b", "c");
            graph.add_edge_with_attrs(
                "c",
                "a",
                GraphEdgeAttrs {
                    length: feedback_length,
                    ..GraphEdgeAttrs::default()
                },
            );

            let layout = layout_graph(&graph, &AsciiRenderOptions::unicode());
            let min_x = layout.nodes.iter().map(|node| node.x).min().unwrap();
            let max_x = layout.nodes.iter().map(|node| node.x).max().unwrap();
            max_x - min_x
        }

        assert!(physical_span(5) > physical_span(1));
    }

    #[test]
    fn dagre_rank_projection_keeps_invisible_edges_as_constraints() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_edge_with_attrs(
            "a",
            "b",
            GraphEdgeAttrs {
                stroke: GraphEdgeStroke::Invisible,
                ..GraphEdgeAttrs::default()
            },
        );

        let layout = layout_graph(&graph, &AsciiRenderOptions::unicode());
        assert!(node_grid(&layout, "a").y < node_grid(&layout, "b").y);
    }

    #[test]
    fn dagre_ranks_map_to_the_canonical_axis_before_surface_mirroring() {
        for direction in [GraphDirection::LeftRight, GraphDirection::RightLeft] {
            let mut graph = AsciiGraph::new(direction);
            graph.add_node("a", "A");
            graph.add_node("b", "B");
            graph.add_edge("a", "b");
            let layout = layout_graph(&graph, &AsciiRenderOptions::unicode());
            assert!(node_grid(&layout, "a").x < node_grid(&layout, "b").x);
        }
        for direction in [GraphDirection::TopDown, GraphDirection::BottomTop] {
            let mut graph = AsciiGraph::new(direction);
            graph.add_node("a", "A");
            graph.add_node("b", "B");
            graph.add_edge("a", "b");
            let layout = layout_graph(&graph, &AsciiRenderOptions::unicode());
            assert!(node_grid(&layout, "a").y < node_grid(&layout, "b").y);
        }
    }

    #[test]
    fn dagre_rank_work_uses_the_shared_exact_layout_budget() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        for id in ["a", "b", "c", "d"] {
            graph.add_node(id, id.to_uppercase());
        }
        graph.add_edge("a", "b");
        graph.add_edge("a", "c");
        graph.add_edge("b", "d");
        graph.add_edge("c", "d");
        let options = AsciiRenderOptions::unicode();
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut measured_resources = ResourceContext::new(unbounded);
        layout_graph_with_resources(&graph, &options, &mut measured_resources)
            .expect("unbounded rank layout should succeed");
        let exact_work = measured_resources.layout_work_used();

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("measured exact work should be a valid limit");
        let mut exact_resources = ResourceContext::new(exact_policy);
        layout_graph_with_resources(&graph, &options, &mut exact_resources)
            .expect("the exact shared Dagre rank budget should pass");
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("max-minus-one work should be a valid limit");
        let mut below_resources = ResourceContext::new(below_policy);
        let error = layout_graph_with_resources(&graph, &options, &mut below_resources)
            .expect_err("max-minus-one Dagre rank budget should fail");
        let crate::AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.max, exact_work - 1);
    }
}

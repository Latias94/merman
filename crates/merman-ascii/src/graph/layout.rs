use super::label::GraphLabel;
use super::model::{
    AsciiGraph, AsciiGraphEdge, AsciiGraphGroup, AsciiGraphNode, GraphDirection, GraphEdgeArrow,
    GraphEdgeStroke, GraphEdgeStyle, GraphGroupKind, GraphGroupStyle, GraphNodeShape,
    GraphNodeStyle, GraphRootPolicy,
};
use crate::options::AsciiRenderOptions;
use crate::text::display_width;
use std::collections::{BTreeMap, HashMap, HashSet};

mod groups;

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
            x: self.offset_x + axis_position(&self.column_widths, coord.x),
            y: self.offset_y + axis_position(&self.row_heights, coord.y),
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
    let (mut nodes, column_widths, row_heights) = layout_nodes(graph, options);
    let (group_offset_x, group_offset_y) = groups::subgraph_offsets(graph, &nodes);
    for node in &mut nodes {
        node.x += group_offset_x;
        node.y += group_offset_y;
    }
    let offset_x = nodes
        .first()
        .map(|node| {
            node.x
                .saturating_sub(axis_position(&column_widths, node.grid.x))
        })
        .unwrap_or_default();
    let offset_y = nodes
        .first()
        .map(|node| {
            node.y
                .saturating_sub(axis_position(&row_heights, node.grid.y))
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

fn layout_nodes(
    graph: &AsciiGraph,
    options: &AsciiRenderOptions,
) -> (
    Vec<NodeLayout>,
    BTreeMap<usize, usize>,
    BTreeMap<usize, usize>,
) {
    match graph.direction.canonical() {
        GraphDirection::LeftRight => layout_left_right_grid_nodes(graph, options),
        GraphDirection::TopDown => layout_top_down_grid_nodes(graph, options),
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    }
}

fn layout_left_right_grid_nodes(
    graph: &AsciiGraph,
    options: &AsciiRenderOptions,
) -> (
    Vec<NodeLayout>,
    BTreeMap<usize, usize>,
    BTreeMap<usize, usize>,
) {
    let placements = place_left_right_grid_nodes(graph);
    let mut column_widths = BTreeMap::new();
    let mut row_heights = BTreeMap::new();

    for (index, coord) in placements.iter().copied().enumerate() {
        let node = &graph.nodes[index];
        set_axis_size(&mut column_widths, coord.x, 1);
        set_axis_size(
            &mut column_widths,
            coord.x + 1,
            node_width(node, options).saturating_sub(2),
        );
        set_axis_size(&mut column_widths, coord.x + 2, 1);
        if coord.x > 0 {
            set_axis_size(&mut column_widths, coord.x - 1, options.graph_padding_x);
        }

        let height = node_height(node, options);
        set_axis_size(&mut row_heights, coord.y, 1);
        set_axis_size(&mut row_heights, coord.y + 1, height.saturating_sub(2));
        set_axis_size(&mut row_heights, coord.y + 2, 1);
        if coord.y > 0 {
            set_axis_size(
                &mut row_heights,
                coord.y - 1,
                node_padding_y(graph, &placements, index, options),
            );
        }
    }

    let coord_by_id = graph
        .nodes
        .iter()
        .zip(placements.iter().copied())
        .map(|(node, coord)| (node.id.as_str(), coord))
        .collect::<HashMap<_, _>>();
    for edge in &graph.edges {
        let (Some(from), Some(to)) = (
            coord_by_id.get(edge.from.as_str()).copied(),
            coord_by_id.get(edge.to.as_str()).copied(),
        ) else {
            continue;
        };
        if to.x <= from.x {
            continue;
        }

        let length_gap = options
            .graph_padding_x
            .saturating_add(edge.length.saturating_sub(1) * 2);
        let label_gap = edge
            .label
            .as_deref()
            .map(|label| display_width(label) + 2)
            .unwrap_or_default();
        set_axis_size(&mut column_widths, to.x - 1, length_gap.max(label_gap));
    }

    let layouts = placements
        .into_iter()
        .zip(graph.nodes.iter())
        .map(|(coord, node)| NodeLayout {
            id: node.id.clone(),
            label: GraphLabel::new(&node.label),
            shape: node.shape,
            style: node.style,
            grid: coord,
            x: axis_position(&column_widths, coord.x),
            y: axis_position(&row_heights, coord.y),
            width: axis_span(&column_widths, coord.x, 3),
            height: axis_span(&row_heights, coord.y, 3),
        })
        .collect();

    (layouts, column_widths, row_heights)
}

fn place_left_right_grid_nodes(graph: &AsciiGraph) -> Vec<GridCoord> {
    let index_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut placements = vec![None; graph.nodes.len()];
    let mut occupied = HashSet::new();
    let mut highest_position_per_level = BTreeMap::<usize, usize>::new();

    let root_indices = graph_root_indices(graph);
    let should_separate_roots =
        should_separate_left_right_roots(graph, &root_indices, &index_by_id);
    let (external_roots, subgraph_roots): (Vec<_>, Vec<_>) =
        root_indices.into_iter().partition(|root_index| {
            !should_separate_roots
                || node_group_index(graph, &graph.nodes[*root_index].id).is_none()
        });

    for root_index in external_roots {
        place_left_right_node(
            root_index,
            0,
            &mut placements,
            &mut occupied,
            &mut highest_position_per_level,
        );
    }
    for root_index in subgraph_roots {
        place_left_right_node(
            root_index,
            4,
            &mut placements,
            &mut occupied,
            &mut highest_position_per_level,
        );
    }

    for node_index in 0..graph.nodes.len() {
        if placements[node_index].is_none() {
            place_left_right_node(
                node_index,
                0,
                &mut placements,
                &mut occupied,
                &mut highest_position_per_level,
            );
        }

        let Some(parent_coord) = placements[node_index] else {
            continue;
        };
        let child_level = parent_coord.x + 4;
        for child_index in child_indices(graph, node_index, &index_by_id) {
            if placements[child_index].is_some() {
                continue;
            }
            place_left_right_node(
                child_index,
                child_level,
                &mut placements,
                &mut occupied,
                &mut highest_position_per_level,
            );
        }
    }

    let mut placements = placements
        .into_iter()
        .map(|coord| coord.unwrap_or(GridCoord { x: 0, y: 0 }))
        .collect::<Vec<_>>();
    groups::apply_group_placement_adjustments(graph, &mut placements);
    placements
}

fn should_separate_left_right_roots(
    graph: &AsciiGraph,
    root_indices: &[usize],
    index_by_id: &HashMap<&str, usize>,
) -> bool {
    let has_external_roots = root_indices
        .iter()
        .any(|index| node_group_index(graph, &graph.nodes[*index].id).is_none());
    let has_subgraph_roots_with_edges = root_indices.iter().any(|index| {
        node_group_index(graph, &graph.nodes[*index].id).is_some()
            && !child_indices(graph, *index, index_by_id).is_empty()
    });

    has_external_roots && has_subgraph_roots_with_edges
}

fn graph_root_indices(graph: &AsciiGraph) -> Vec<usize> {
    let nodes_with_incoming = graph
        .edges
        .iter()
        .map(|edge| edge.to.as_str())
        .collect::<HashSet<_>>();

    let declared_first =
        graph.root_policy == GraphRootPolicy::DeclaredFirst && !graph.nodes.is_empty();
    let mut roots = Vec::new();
    if declared_first {
        roots.push(0);
    }

    let incoming_roots = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            (!nodes_with_incoming.contains(node.id.as_str()) && (!declared_first || index != 0))
                .then_some(index)
        })
        .collect::<Vec<_>>();
    roots.extend(incoming_roots);
    if roots.is_empty() && !graph.nodes.is_empty() {
        roots.push(0);
    }

    roots
}

fn child_indices<'a>(
    graph: &'a AsciiGraph,
    node_index: usize,
    index_by_id: &HashMap<&'a str, usize>,
) -> Vec<usize> {
    let node = &graph.nodes[node_index];
    graph
        .edges
        .iter()
        .filter(|edge| edge.from == node.id)
        .filter_map(|edge| index_by_id.get(edge.to.as_str()).copied())
        .collect()
}

fn place_left_right_node(
    node_index: usize,
    level: usize,
    placements: &mut [Option<GridCoord>],
    occupied: &mut HashSet<(usize, usize)>,
    highest_position_per_level: &mut BTreeMap<usize, usize>,
) {
    let requested_y = highest_position_per_level
        .get(&level)
        .copied()
        .unwrap_or_default();
    let coord = reserve_grid_spot(
        occupied,
        GridCoord {
            x: level,
            y: requested_y,
        },
        GraphDirection::LeftRight,
    );
    placements[node_index] = Some(coord);
    highest_position_per_level.insert(level, coord.y + 4);
}

fn reserve_grid_spot(
    occupied: &mut HashSet<(usize, usize)>,
    requested_coord: GridCoord,
    direction: GraphDirection,
) -> GridCoord {
    let mut coord = requested_coord;
    while grid_spot_occupied(occupied, coord) {
        match direction.canonical() {
            GraphDirection::LeftRight => coord.y += 4,
            GraphDirection::TopDown => coord.x += 4,
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        }
    }

    for x in coord.x..(coord.x + 3) {
        for y in coord.y..(coord.y + 3) {
            occupied.insert((x, y));
        }
    }

    coord
}

fn grid_spot_occupied(occupied: &HashSet<(usize, usize)>, coord: GridCoord) -> bool {
    (coord.x..(coord.x + 3)).any(|x| (coord.y..(coord.y + 3)).any(|y| occupied.contains(&(x, y))))
}

fn layout_top_down_grid_nodes(
    graph: &AsciiGraph,
    options: &AsciiRenderOptions,
) -> (
    Vec<NodeLayout>,
    BTreeMap<usize, usize>,
    BTreeMap<usize, usize>,
) {
    let placements = place_top_down_grid_nodes(graph);
    let mut column_widths = BTreeMap::new();
    let mut row_heights = BTreeMap::new();

    for (index, coord) in placements.iter().copied().enumerate() {
        let node = &graph.nodes[index];
        set_axis_size(&mut column_widths, coord.x, 1);
        set_axis_size(
            &mut column_widths,
            coord.x + 1,
            node_width(node, options).saturating_sub(2),
        );
        set_axis_size(&mut column_widths, coord.x + 2, 1);
        if coord.x > 0 {
            set_axis_size(&mut column_widths, coord.x - 1, options.graph_padding_x);
        }

        let height = node_height(node, options);
        set_axis_size(&mut row_heights, coord.y, 1);
        set_axis_size(&mut row_heights, coord.y + 1, height.saturating_sub(2));
        set_axis_size(&mut row_heights, coord.y + 2, 1);
        if coord.y > 0 {
            set_axis_size(
                &mut row_heights,
                coord.y - 1,
                node_padding_y(graph, &placements, index, options),
            );
        }
    }

    let index_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    for edge in &graph.edges {
        let (Some(from_index), Some(to_index)) = (
            index_by_id.get(edge.from.as_str()).copied(),
            index_by_id.get(edge.to.as_str()).copied(),
        ) else {
            continue;
        };
        let from = placements[from_index];
        let to = placements[to_index];
        if to.y <= from.y || from.x != to.x {
            continue;
        }
        if let Some(label) = edge.label.as_deref() {
            set_axis_size(&mut column_widths, from.x + 1, display_width(label) + 2);
        }
    }

    let layouts = placements
        .into_iter()
        .zip(graph.nodes.iter())
        .map(|(coord, node)| NodeLayout {
            id: node.id.clone(),
            label: GraphLabel::new(&node.label),
            shape: node.shape,
            style: node.style,
            grid: coord,
            x: axis_position(&column_widths, coord.x),
            y: axis_position(&row_heights, coord.y),
            width: axis_span(&column_widths, coord.x, 3),
            height: axis_span(&row_heights, coord.y, 3),
        })
        .collect();

    (layouts, column_widths, row_heights)
}

fn place_top_down_grid_nodes(graph: &AsciiGraph) -> Vec<GridCoord> {
    let index_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut placements = vec![None; graph.nodes.len()];
    let mut occupied = HashSet::new();
    let mut highest_position_per_level = BTreeMap::<usize, usize>::new();

    for root_index in graph_root_indices(graph) {
        place_top_down_node(
            root_index,
            0,
            &mut placements,
            &mut occupied,
            &mut highest_position_per_level,
        );
    }

    for node_index in 0..graph.nodes.len() {
        if placements[node_index].is_none() {
            place_top_down_node(
                node_index,
                0,
                &mut placements,
                &mut occupied,
                &mut highest_position_per_level,
            );
        }

        let Some(parent_coord) = placements[node_index] else {
            continue;
        };
        let child_level = parent_coord.y + 4;
        for child_index in child_indices(graph, node_index, &index_by_id) {
            if placements[child_index].is_some() {
                continue;
            }
            place_top_down_node(
                child_index,
                child_level,
                &mut placements,
                &mut occupied,
                &mut highest_position_per_level,
            );
        }
    }

    let mut placements = placements
        .into_iter()
        .map(|coord| coord.unwrap_or(GridCoord { x: 0, y: 0 }))
        .collect::<Vec<_>>();
    groups::apply_group_placement_adjustments(graph, &mut placements);
    placements
}

fn graph_endpoint_group_ids(graph: &AsciiGraph) -> HashSet<&str> {
    let group_ids = graph
        .groups
        .iter()
        .map(|group| group.id.as_str())
        .collect::<HashSet<_>>();
    graph
        .edges
        .iter()
        .flat_map(|edge| [edge.from.as_str(), edge.to.as_str()])
        .filter(|endpoint| group_ids.contains(endpoint))
        .collect()
}

fn group_member_indices(graph: &AsciiGraph, group_index: usize) -> Vec<usize> {
    let group_index_by_id = graph
        .groups
        .iter()
        .enumerate()
        .map(|(index, group)| (group.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let node_index_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut indices = HashSet::new();
    let mut visited_groups = HashSet::new();
    let mut stack = vec![group_index];

    while let Some(index) = stack.pop() {
        if !visited_groups.insert(index) {
            continue;
        }
        let Some(group) = graph.groups.get(index) else {
            continue;
        };

        for member in &group.nodes {
            if let Some(node_index) = node_index_by_id.get(member.as_str()).copied() {
                indices.insert(node_index);
            } else if let Some(child_group_index) = group_index_by_id.get(member.as_str()).copied()
            {
                stack.push(child_group_index);
            }
        }
    }

    let mut indices = indices.into_iter().collect::<Vec<_>>();
    indices.sort_unstable();
    indices
}

fn group_bounds_for_placements(
    graph: &AsciiGraph,
    group_index: usize,
    member_indices: &[usize],
    placements: &[GridCoord],
) -> Option<RawBounds> {
    let group = graph.groups.get(group_index)?;
    let mut member_bounds = None::<RawBounds>;

    for index in member_indices {
        let bounds = node_bounds(*placements.get(*index)?);
        if let Some(current) = &mut member_bounds {
            current.include(bounds);
        } else {
            member_bounds = Some(bounds);
        }
    }

    Some(raw_group_bounds_for_members(group, member_bounds?))
}

#[derive(Debug, Clone)]
struct GroupPlacementMember {
    id: String,
    node_indices: Vec<usize>,
}

fn group_placement_members(graph: &AsciiGraph, group_index: usize) -> Vec<GroupPlacementMember> {
    let Some(group) = graph.groups.get(group_index) else {
        return Vec::new();
    };
    let group_index_by_id = graph
        .groups
        .iter()
        .enumerate()
        .map(|(index, group)| (group.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let node_index_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();

    let mut members = Vec::new();
    for member in &group.nodes {
        if let Some(node_index) = node_index_by_id.get(member.as_str()).copied() {
            members.push(GroupPlacementMember {
                id: member.clone(),
                node_indices: vec![node_index],
            });
        } else if let Some(child_group_index) = group_index_by_id.get(member.as_str()).copied() {
            let node_indices = group_member_indices(graph, child_group_index);
            if node_indices.is_empty() {
                continue;
            }
            members.push(GroupPlacementMember {
                id: member.clone(),
                node_indices,
            });
        }
    }

    members
}

fn build_group_override_graph(graph: &AsciiGraph, members: &[GroupPlacementMember]) -> AsciiGraph {
    let mut override_graph = AsciiGraph::new_for_diagram(graph.diagram_type(), graph.direction);
    override_graph.root_policy = graph.root_policy;
    override_graph.nodes = members
        .iter()
        .map(|member| AsciiGraphNode {
            id: member.id.clone(),
            label: member.id.clone(),
            shape: GraphNodeShape::Rect,
            style: GraphNodeStyle::default(),
        })
        .collect();

    let mut node_to_member = HashMap::<&str, usize>::new();
    for (member_index, member) in members.iter().enumerate() {
        for node_index in &member.node_indices {
            let Some(node) = graph.nodes.get(*node_index) else {
                continue;
            };
            node_to_member
                .entry(node.id.as_str())
                .or_insert(member_index);
        }
    }

    let mut seen_edges = HashSet::<(usize, usize)>::new();
    for edge in &graph.edges {
        let Some(from_member_index) = node_to_member.get(edge.from.as_str()).copied() else {
            continue;
        };
        let Some(to_member_index) = node_to_member.get(edge.to.as_str()).copied() else {
            continue;
        };
        if from_member_index == to_member_index {
            continue;
        }
        if !seen_edges.insert((from_member_index, to_member_index)) {
            continue;
        }
        let from = override_graph.nodes[from_member_index].id.clone();
        let to = override_graph.nodes[to_member_index].id.clone();
        override_graph.edges.push(AsciiGraphEdge {
            from,
            to,
            label: None,
            stroke: GraphEdgeStroke::Normal,
            arrow: GraphEdgeArrow::Point,
            length: 1,
            style: GraphEdgeStyle::default(),
        });
    }

    override_graph
}

fn member_origin(placements: &[GridCoord], member_indices: &[usize]) -> Option<GridCoord> {
    let bounds = member_grid_bounds(member_indices, placements)?;
    Some(GridCoord {
        x: bounds.x.max(0) as usize,
        y: bounds.y.max(0) as usize,
    })
}

fn shift_member_indices(
    placements: &mut [GridCoord],
    member_indices: &[usize],
    delta_x: isize,
    delta_y: isize,
) {
    if delta_x == 0 && delta_y == 0 {
        return;
    }

    for index in member_indices {
        if let Some(coord) = placements.get_mut(*index) {
            if delta_x.is_positive() {
                coord.x += delta_x as usize;
            } else {
                coord.x = coord.x.saturating_sub(delta_x.unsigned_abs());
            }
            if delta_y.is_positive() {
                coord.y += delta_y as usize;
            } else {
                coord.y = coord.y.saturating_sub(delta_y.unsigned_abs());
            }
        }
    }
}

fn member_grid_bounds(member_indices: &[usize], placements: &[GridCoord]) -> Option<RawBounds> {
    let mut bounds = None::<RawBounds>;

    for index in member_indices {
        let current = node_bounds(*placements.get(*index)?);
        if let Some(existing) = &mut bounds {
            existing.include(current);
        } else {
            bounds = Some(current);
        }
    }

    bounds
}

fn shift_member_indices_y(placements: &mut [GridCoord], member_indices: &[usize], delta: usize) {
    if delta == 0 {
        return;
    }

    for index in member_indices {
        if let Some(coord) = placements.get_mut(*index) {
            coord.y += delta;
        }
    }
}

fn shift_member_indices_x(placements: &mut [GridCoord], member_indices: &[usize], delta: isize) {
    if delta == 0 {
        return;
    }

    for index in member_indices {
        if let Some(coord) = placements.get_mut(*index) {
            if delta.is_positive() {
                coord.x += delta as usize;
            } else {
                coord.x = coord.x.saturating_sub(delta.unsigned_abs());
            }
        }
    }
}

fn group_root_indices(graph: &AsciiGraph, member_indices: &[usize]) -> Vec<usize> {
    let member_ids = member_indices
        .iter()
        .filter_map(|index| graph.nodes.get(*index))
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();

    member_indices
        .iter()
        .copied()
        .filter(|index| {
            let Some(node) = graph.nodes.get(*index) else {
                return false;
            };
            !graph
                .edges
                .iter()
                .any(|edge| edge.to == node.id && member_ids.contains(edge.from.as_str()))
        })
        .collect()
}

fn place_group_nodes(
    graph: &AsciiGraph,
    member_indices: &[usize],
    root_indices: &[usize],
    direction: GraphDirection,
) -> HashMap<usize, GridCoord> {
    let member_ids = member_indices
        .iter()
        .filter_map(|index| graph.nodes.get(*index))
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    let index_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut placements = HashMap::new();
    let mut occupied = HashSet::new();
    let mut highest_position_per_level = BTreeMap::<usize, usize>::new();
    let mut visit_order = Vec::new();
    let mut cursor = 0usize;

    for root_index in root_indices {
        place_group_node(
            *root_index,
            0,
            direction,
            &mut placements,
            &mut occupied,
            &mut highest_position_per_level,
        );
        visit_order.push(*root_index);
    }

    process_group_visit_order(
        graph,
        &member_ids,
        &index_by_id,
        direction,
        &mut placements,
        &mut occupied,
        &mut highest_position_per_level,
        &mut visit_order,
        &mut cursor,
    );

    let remaining_members = member_indices
        .iter()
        .copied()
        .filter(|index| !placements.contains_key(index))
        .collect::<Vec<_>>();
    if !remaining_members.is_empty() {
        for node_index in remaining_members {
            place_group_node(
                node_index,
                0,
                direction,
                &mut placements,
                &mut occupied,
                &mut highest_position_per_level,
            );
            visit_order.push(node_index);
        }
        process_group_visit_order(
            graph,
            &member_ids,
            &index_by_id,
            direction,
            &mut placements,
            &mut occupied,
            &mut highest_position_per_level,
            &mut visit_order,
            &mut cursor,
        );
    }

    placements
}

fn process_group_visit_order<'a>(
    graph: &'a AsciiGraph,
    member_ids: &HashSet<&'a str>,
    index_by_id: &HashMap<&'a str, usize>,
    direction: GraphDirection,
    placements: &mut HashMap<usize, GridCoord>,
    occupied: &mut HashSet<(usize, usize)>,
    highest_position_per_level: &mut BTreeMap<usize, usize>,
    visit_order: &mut Vec<usize>,
    cursor: &mut usize,
) {
    while *cursor < visit_order.len() {
        let node_index = visit_order[*cursor];
        *cursor += 1;

        let Some(parent_coord) = placements.get(&node_index).copied() else {
            continue;
        };
        let child_level = match direction {
            GraphDirection::LeftRight => parent_coord.x + 4,
            GraphDirection::TopDown => parent_coord.y + 4,
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        };
        for child_index in graph
            .edges
            .iter()
            .filter(|edge| {
                graph.nodes[node_index].id == edge.from && member_ids.contains(edge.to.as_str())
            })
            .filter_map(|edge| index_by_id.get(edge.to.as_str()).copied())
        {
            if placements.contains_key(&child_index) {
                continue;
            }
            place_group_node(
                child_index,
                child_level,
                direction,
                placements,
                occupied,
                highest_position_per_level,
            );
            visit_order.push(child_index);
        }
    }
}

fn shift_external_nodes_away_from_group(
    graph: &AsciiGraph,
    member_indices: &[usize],
    group_bounds: RawBounds,
    placements: &mut [GridCoord],
) -> bool {
    let member_indices = member_indices.iter().copied().collect::<HashSet<_>>();
    let graph_direction = graph.direction.canonical();
    let mut changed = false;

    for index in 0..placements.len() {
        if member_indices.contains(&index) {
            continue;
        }
        if !raw_bounds_intersects(group_bounds, node_bounds(placements[index])) {
            continue;
        }

        while raw_bounds_intersects(group_bounds, node_bounds(placements[index]))
            || node_overlaps_any_other(index, placements)
        {
            changed = true;
            match graph_direction {
                GraphDirection::LeftRight => placements[index].y += 4,
                GraphDirection::TopDown => placements[index].x += 4,
                GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
            }
        }
    }

    changed
}

fn node_overlaps_any_other(index: usize, placements: &[GridCoord]) -> bool {
    let bounds = node_bounds(placements[index]);
    placements
        .iter()
        .enumerate()
        .any(|(other_index, other_coord)| {
            index != other_index && raw_bounds_intersects(bounds, node_bounds(*other_coord))
        })
}

fn node_bounds(coord: GridCoord) -> RawBounds {
    RawBounds {
        x: coord.x as isize,
        y: coord.y as isize,
        right: coord.x as isize + 2,
        bottom: coord.y as isize + 2,
    }
}

fn raw_bounds_intersects(left: RawBounds, right: RawBounds) -> bool {
    !(left.right < right.x
        || right.right < left.x
        || left.bottom < right.y
        || right.bottom < left.y)
}

fn place_group_node(
    node_index: usize,
    level: usize,
    direction: GraphDirection,
    placements: &mut HashMap<usize, GridCoord>,
    occupied: &mut HashSet<(usize, usize)>,
    highest_position_per_level: &mut BTreeMap<usize, usize>,
) {
    let requested = highest_position_per_level
        .get(&level)
        .copied()
        .unwrap_or_default();
    let coord = match direction {
        GraphDirection::LeftRight => reserve_grid_spot(
            occupied,
            GridCoord {
                x: level,
                y: requested,
            },
            direction,
        ),
        GraphDirection::TopDown => reserve_grid_spot(
            occupied,
            GridCoord {
                x: requested,
                y: level,
            },
            direction,
        ),
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    };
    placements.insert(node_index, coord);
    match direction {
        GraphDirection::LeftRight => {
            highest_position_per_level.insert(level, coord.y + 4);
        }
        GraphDirection::TopDown => {
            highest_position_per_level.insert(level, coord.x + 4);
        }
        GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
    }
}

fn place_top_down_node(
    node_index: usize,
    level: usize,
    placements: &mut [Option<GridCoord>],
    occupied: &mut HashSet<(usize, usize)>,
    highest_position_per_level: &mut BTreeMap<usize, usize>,
) {
    let requested_x = highest_position_per_level
        .get(&level)
        .copied()
        .unwrap_or_default();
    let coord = reserve_grid_spot(
        occupied,
        GridCoord {
            x: requested_x,
            y: level,
        },
        GraphDirection::TopDown,
    );
    placements[node_index] = Some(coord);
    highest_position_per_level.insert(level, coord.x + 4);
}

fn set_axis_size(axis_sizes: &mut BTreeMap<usize, usize>, index: usize, size: usize) {
    axis_sizes
        .entry(index)
        .and_modify(|current| *current = (*current).max(size))
        .or_insert(size);
}

fn node_padding_y(
    graph: &AsciiGraph,
    placements: &[GridCoord],
    node_index: usize,
    options: &AsciiRenderOptions,
) -> usize {
    const SUBGRAPH_EXTERNAL_INCOMING_OVERHEAD: usize = 4;

    let Some(node) = graph.nodes.get(node_index) else {
        return options.graph_padding_y;
    };
    let Some(group_index) = node_group_index(graph, &node.id) else {
        return options.graph_padding_y;
    };
    if !has_incoming_edge_from_outside_group(graph, &node.id, group_index) {
        return options.graph_padding_y;
    }

    let node_y = placements
        .get(node_index)
        .map(|coord| coord.y)
        .unwrap_or_default();
    let has_higher_external_entry = graph.groups[group_index].nodes.iter().any(|other_id| {
        if other_id == &node.id
            || !has_incoming_edge_from_outside_group(graph, other_id, group_index)
        {
            return false;
        }
        let Some(other_index) = graph.nodes.iter().position(|other| other.id == *other_id) else {
            return false;
        };
        placements
            .get(other_index)
            .is_some_and(|coord| coord.y < node_y)
    });
    if has_higher_external_entry {
        return options.graph_padding_y;
    }

    options.graph_padding_y + SUBGRAPH_EXTERNAL_INCOMING_OVERHEAD
}

fn has_incoming_edge_from_outside_group(
    graph: &AsciiGraph,
    node_id: &str,
    group_index: usize,
) -> bool {
    graph
        .edges
        .iter()
        .any(|edge| edge.to == node_id && node_group_index(graph, &edge.from) != Some(group_index))
}

fn node_group_index(graph: &AsciiGraph, node_id: &str) -> Option<usize> {
    graph
        .groups
        .iter()
        .position(|group| group.nodes.iter().any(|member| member == node_id))
}

fn axis_position(axis_sizes: &BTreeMap<usize, usize>, index: usize) -> usize {
    axis_sizes
        .range(..index)
        .map(|(_, size)| *size)
        .sum::<usize>()
        + axis_sizes.get(&index).copied().unwrap_or_default() / 2
}

fn axis_span(axis_sizes: &BTreeMap<usize, usize>, start: usize, len: usize) -> usize {
    (start..(start + len))
        .map(|index| axis_sizes.get(&index).copied().unwrap_or_default())
        .sum()
}

fn node_height(node: &AsciiGraphNode, options: &AsciiRenderOptions) -> usize {
    match node.shape {
        GraphNodeShape::StateStart
        | GraphNodeShape::StateEnd
        | GraphNodeShape::ForkJoinHorizontal
        | GraphNodeShape::Choice => 3,
        GraphNodeShape::ForkJoinVertical => 7,
        GraphNodeShape::Rect
        | GraphNodeShape::Rounded
        | GraphNodeShape::Diamond
        | GraphNodeShape::Subroutine
        | GraphNodeShape::Cylinder => {
            2 + GraphLabel::new(&node.label).content_height() + options.box_border_padding * 2
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RawBounds {
    x: isize,
    y: isize,
    right: isize,
    bottom: isize,
}

impl RawBounds {
    fn include(&mut self, other: RawBounds) {
        self.x = self.x.min(other.x);
        self.y = self.y.min(other.y);
        self.right = self.right.max(other.right);
        self.bottom = self.bottom.max(other.bottom);
    }
}

fn raw_group_bounds_for_members(group: &AsciiGraphGroup, member_bounds: RawBounds) -> RawBounds {
    let x = member_bounds.x - 2;
    let right = member_bounds.right + 2;

    match group.kind {
        GraphGroupKind::Container => {
            let title_width = (member_bounds.right - member_bounds.x + 3).max(1) as usize;
            let title = GraphLabel::wrapped(&group.title, title_width);
            let title_space = title.content_height() + 3;
            RawBounds {
                x,
                y: member_bounds.y - title_space as isize,
                right,
                bottom: member_bounds.bottom + 2,
            }
        }
        GraphGroupKind::Divider => RawBounds {
            x,
            y: member_bounds.y - 1,
            right,
            bottom: member_bounds.bottom,
        },
    }
}

fn raw_group_bounds(
    graph: &AsciiGraph,
    layouts: &[NodeLayout],
    group_index: usize,
) -> Option<RawBounds> {
    graph.groups.get(group_index)?;

    let mut layout_bounds_by_id = HashMap::new();
    for layout in layouts {
        layout_bounds_by_id
            .entry(layout.id.as_str())
            .or_insert(RawBounds {
                x: layout.x as isize,
                y: layout.y as isize,
                right: layout.right() as isize,
                bottom: layout.bottom() as isize,
            });
    }
    let mut group_index_by_id = HashMap::new();
    for (index, group) in graph.groups.iter().enumerate() {
        group_index_by_id.entry(group.id.as_str()).or_insert(index);
    }
    let mut completed = HashMap::<usize, Option<RawBounds>>::new();
    let mut visiting = HashSet::<usize>::new();
    let mut stack = vec![(group_index, false)];

    while let Some((index, exiting)) = stack.pop() {
        if completed.contains_key(&index) {
            continue;
        }
        let Some(group) = graph.groups.get(index) else {
            completed.insert(index, None);
            continue;
        };

        if exiting {
            visiting.remove(&index);
            completed.insert(
                index,
                raw_group_bounds_from_completed_children(
                    index,
                    group,
                    &layout_bounds_by_id,
                    &group_index_by_id,
                    &completed,
                ),
            );
            continue;
        }

        if !visiting.insert(index) {
            completed.insert(index, None);
            continue;
        }

        stack.push((index, true));
        for member in group.nodes.iter().rev() {
            if let Some(child_index) = group_index_by_id
                .get(member.as_str())
                .copied()
                .filter(|child_index| *child_index != index)
                && !completed.contains_key(&child_index)
                && !visiting.contains(&child_index)
            {
                stack.push((child_index, false));
            }
        }
    }

    completed.remove(&group_index).flatten()
}

fn raw_group_bounds_from_completed_children(
    group_index: usize,
    group: &super::model::AsciiGraphGroup,
    layout_bounds_by_id: &HashMap<&str, RawBounds>,
    group_index_by_id: &HashMap<&str, usize>,
    completed: &HashMap<usize, Option<RawBounds>>,
) -> Option<RawBounds> {
    let mut member_bounds = None::<RawBounds>;

    for member in &group.nodes {
        let bounds = if let Some(bounds) = layout_bounds_by_id.get(member.as_str()).copied() {
            Some(bounds)
        } else if let Some(child_index) = group_index_by_id
            .get(member.as_str())
            .copied()
            .filter(|child_index| *child_index != group_index)
        {
            completed.get(&child_index).copied().flatten()
        } else {
            None
        };

        let Some(bounds) = bounds else {
            continue;
        };
        if let Some(current) = &mut member_bounds {
            current.include(bounds);
        } else {
            member_bounds = Some(bounds);
        };
    }

    Some(raw_group_bounds_for_members(group, member_bounds?))
}

#[derive(Debug, Clone, Copy)]
struct GroupLayoutBounds {
    x: usize,
    y: usize,
    right: usize,
    bottom: usize,
}

fn group_title_for_layout(group: &AsciiGraphGroup, min_x: usize, max_right: usize) -> GraphLabel {
    match group.kind {
        GraphGroupKind::Container => {
            GraphLabel::wrapped(&group.title, (max_right.saturating_sub(min_x) + 3).max(1))
        }
        GraphGroupKind::Divider => GraphLabel::new(""),
    }
}

fn group_layout_bounds_for_members(
    group: &AsciiGraphGroup,
    title: &GraphLabel,
    min_x: usize,
    min_y: usize,
    max_right: usize,
    max_bottom: usize,
) -> GroupLayoutBounds {
    let x = min_x.saturating_sub(2);
    let right = max_right.saturating_add(2);

    match group.kind {
        GraphGroupKind::Container => {
            let title_space = title.content_height() + 3;
            GroupLayoutBounds {
                x,
                y: min_y.saturating_sub(title_space),
                right,
                bottom: max_bottom.saturating_add(2),
            }
        }
        GraphGroupKind::Divider => GroupLayoutBounds {
            x,
            y: min_y.saturating_sub(1),
            right,
            bottom: max_bottom,
        },
    }
}

fn assign_divider_spans(graph: &AsciiGraph, groups: &mut [GroupLayout]) {
    for graph_group in graph
        .groups
        .iter()
        .filter(|group| group.kind == GraphGroupKind::Divider)
    {
        let Some(layout_index) = groups.iter().position(|layout| layout.id == graph_group.id)
        else {
            continue;
        };
        let span = divider_parent_span(graph, groups, &graph_group.id)
            .or_else(|| divider_inner_span(&groups[layout_index]));
        groups[layout_index].divider_span = span;
    }
}

fn divider_parent_span(
    graph: &AsciiGraph,
    groups: &[GroupLayout],
    divider_id: &str,
) -> Option<DividerSpan> {
    let parent = graph
        .groups
        .iter()
        .find(|group| group.nodes.iter().any(|member| member == divider_id))?;
    groups
        .iter()
        .find(|layout| layout.id == parent.id)
        .and_then(divider_inner_span)
}

fn divider_inner_span(group: &GroupLayout) -> Option<DividerSpan> {
    let x_start = group.x.saturating_add(1);
    let x_end = group.right().saturating_sub(1);
    (x_start <= x_end).then_some(DividerSpan { x_start, x_end })
}

fn node_width(node: &AsciiGraphNode, options: &AsciiRenderOptions) -> usize {
    let base = GraphLabel::new(&node.label).width() + options.box_border_padding * 2 + 2;
    match node.shape {
        GraphNodeShape::StateStart | GraphNodeShape::StateEnd | GraphNodeShape::Choice => 5,
        GraphNodeShape::ForkJoinHorizontal => 7,
        GraphNodeShape::ForkJoinVertical => 3,
        GraphNodeShape::Subroutine => base + 2,
        GraphNodeShape::Cylinder => base + 2,
        GraphNodeShape::Rect | GraphNodeShape::Rounded | GraphNodeShape::Diamond => base,
    }
}

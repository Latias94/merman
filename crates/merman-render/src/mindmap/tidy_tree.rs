use super::{MindmapEdgeModel, MindmapNodeModel};
use crate::model::{LayoutEdge, LayoutNode, LayoutPoint};
use crate::{Error, Result};

const SIBLING_GAP: f64 = 20.0;
const CONNECTION_PADDING: f64 = 40.0;
const TREE_GAP: f64 = 30.0;

#[derive(Debug)]
struct TidyNode {
    original_index: Option<usize>,
    width: f64,
    height: f64,
    y: f64,
    children: Vec<usize>,
    x: f64,
    prelim: f64,
    modifier: f64,
    shift: f64,
    change: f64,
    left_thread: Option<usize>,
    right_thread: Option<usize>,
    extreme_left: usize,
    extreme_right: usize,
    modifier_sum_extreme_left: f64,
    modifier_sum_extreme_right: f64,
}

impl TidyNode {
    fn new(original_index: Option<usize>, width: f64, height: f64, y: f64) -> Self {
        Self {
            original_index,
            width,
            height,
            y,
            children: Vec::new(),
            x: 0.0,
            prelim: 0.0,
            modifier: 0.0,
            shift: 0.0,
            change: 0.0,
            left_thread: None,
            right_thread: None,
            extreme_left: 0,
            extreme_right: 0,
            modifier_sum_extreme_left: 0.0,
            modifier_sum_extreme_right: 0.0,
        }
    }
}

#[derive(Debug)]
struct IndexedYList {
    low_y: f64,
    index: usize,
    next: Option<Box<IndexedYList>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TreeSection {
    Root,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
struct PositionedNode {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    section: TreeSection,
}

fn bottom(nodes: &[TidyNode], index: usize) -> f64 {
    nodes[index].y + nodes[index].height
}

fn set_extremes(nodes: &mut [TidyNode], index: usize) {
    let children = &nodes[index].children;
    if children.is_empty() {
        nodes[index].extreme_left = index;
        nodes[index].extreme_right = index;
        nodes[index].modifier_sum_extreme_left = 0.0;
        nodes[index].modifier_sum_extreme_right = 0.0;
        return;
    }

    let first = children[0];
    let last = children[children.len() - 1];
    let extreme_left = nodes[first].extreme_left;
    let modifier_sum_extreme_left = nodes[first].modifier_sum_extreme_left;
    let extreme_right = nodes[last].extreme_right;
    let modifier_sum_extreme_right = nodes[last].modifier_sum_extreme_right;
    nodes[index].extreme_left = extreme_left;
    nodes[index].modifier_sum_extreme_left = modifier_sum_extreme_left;
    nodes[index].extreme_right = extreme_right;
    nodes[index].modifier_sum_extreme_right = modifier_sum_extreme_right;
}

fn update_indexed_y_list(
    min_y: f64,
    index: usize,
    mut head: Option<Box<IndexedYList>>,
) -> Box<IndexedYList> {
    while head.as_ref().is_some_and(|item| min_y >= item.low_y) {
        head = head.and_then(|item| item.next);
    }
    Box::new(IndexedYList {
        low_y: min_y,
        index,
        next: head,
    })
}

fn distribute_extra(nodes: &mut [TidyNode], parent: usize, index: usize, sibling: usize, d: f64) {
    if sibling == index - 1 {
        return;
    }
    let child_count = (index - sibling) as f64;
    let after_sibling = nodes[parent].children[sibling + 1];
    let current = nodes[parent].children[index];
    nodes[after_sibling].shift += d / child_count;
    nodes[current].shift -= d / child_count;
    nodes[current].change -= d - d / child_count;
}

fn move_subtree(nodes: &mut [TidyNode], parent: usize, index: usize, sibling: usize, d: f64) {
    let current = nodes[parent].children[index];
    nodes[current].modifier += d;
    nodes[current].modifier_sum_extreme_left += d;
    nodes[current].modifier_sum_extreme_right += d;
    distribute_extra(nodes, parent, index, sibling, d);
}

fn next_left_contour(nodes: &[TidyNode], index: usize) -> Option<usize> {
    nodes[index]
        .children
        .first()
        .copied()
        .or(nodes[index].left_thread)
}

fn next_right_contour(nodes: &[TidyNode], index: usize) -> Option<usize> {
    nodes[index]
        .children
        .last()
        .copied()
        .or(nodes[index].right_thread)
}

fn set_left_thread(
    nodes: &mut [TidyNode],
    parent: usize,
    index: usize,
    contour_left: usize,
    modifier_sum_contour_left: f64,
) {
    let first = nodes[parent].children[0];
    let current = nodes[parent].children[index];
    let left = nodes[first].extreme_left;
    let diff = (modifier_sum_contour_left - nodes[contour_left].modifier)
        - nodes[first].modifier_sum_extreme_left;
    nodes[left].left_thread = Some(contour_left);
    nodes[left].modifier += diff;
    nodes[left].prelim -= diff;
    nodes[first].extreme_left = nodes[current].extreme_left;
    nodes[first].modifier_sum_extreme_left = nodes[current].modifier_sum_extreme_left;
}

fn set_right_thread(
    nodes: &mut [TidyNode],
    parent: usize,
    index: usize,
    sibling_right: usize,
    modifier_sum_sibling_right: f64,
) {
    let current = nodes[parent].children[index];
    let previous = nodes[parent].children[index - 1];
    let right = nodes[current].extreme_right;
    let diff = (modifier_sum_sibling_right - nodes[sibling_right].modifier)
        - nodes[current].modifier_sum_extreme_right;
    nodes[right].right_thread = Some(sibling_right);
    nodes[right].modifier += diff;
    nodes[right].prelim -= diff;
    nodes[current].extreme_right = nodes[previous].extreme_right;
    nodes[current].modifier_sum_extreme_right = nodes[previous].modifier_sum_extreme_right;
}

fn separate(
    nodes: &mut [TidyNode],
    parent: usize,
    index: usize,
    indexed_y_list: &IndexedYList,
) -> Result<()> {
    let initial_sibling_right = nodes[parent].children[index - 1];
    let initial_contour_left = nodes[parent].children[index];
    let mut sibling_right = Some(initial_sibling_right);
    let mut modifier_sum_sibling_right = nodes[initial_sibling_right].modifier;
    let mut contour_left = Some(initial_contour_left);
    let mut modifier_sum_contour_left = nodes[initial_contour_left].modifier;
    let mut indexed_y = Some(indexed_y_list);

    while let (Some(sr), Some(cl)) = (sibling_right, contour_left) {
        if indexed_y.is_some_and(|item| bottom(nodes, sr) > item.low_y) {
            indexed_y = indexed_y.and_then(|item| item.next.as_deref());
        }
        let Some(indexed_y) = indexed_y else {
            return Err(Error::InvalidModel {
                message: "tidy-tree contour index invariant was violated".to_string(),
            });
        };

        let distance = modifier_sum_sibling_right + nodes[sr].prelim + nodes[sr].width
            - (modifier_sum_contour_left + nodes[cl].prelim);
        if distance > 0.0 {
            modifier_sum_contour_left += distance;
            move_subtree(nodes, parent, index, indexed_y.index, distance);
        }

        let sibling_y = bottom(nodes, sr);
        let contour_y = bottom(nodes, cl);
        if sibling_y <= contour_y {
            sibling_right = next_right_contour(nodes, sr);
            if let Some(next) = sibling_right {
                modifier_sum_sibling_right += nodes[next].modifier;
            }
        }
        if sibling_y >= contour_y {
            contour_left = next_left_contour(nodes, cl);
            if let Some(next) = contour_left {
                modifier_sum_contour_left += nodes[next].modifier;
            }
        }
    }

    match (sibling_right, contour_left) {
        (None, Some(cl)) => set_left_thread(nodes, parent, index, cl, modifier_sum_contour_left),
        (Some(sr), None) => set_right_thread(nodes, parent, index, sr, modifier_sum_sibling_right),
        _ => {}
    }
    Ok(())
}

fn position_root(nodes: &mut [TidyNode], index: usize) {
    let first = nodes[index].children[0];
    let last = nodes[index].children[nodes[index].children.len() - 1];
    nodes[index].prelim = (nodes[first].prelim
        + nodes[first].modifier
        + nodes[last].modifier
        + nodes[last].prelim
        + nodes[last].width)
        / 2.0
        - nodes[index].width / 2.0;
}

fn first_walk(nodes: &mut [TidyNode], index: usize) -> Result<()> {
    let children = nodes[index].children.clone();
    if children.is_empty() {
        set_extremes(nodes, index);
        return Ok(());
    }

    first_walk(nodes, children[0])?;
    let first_extreme_left = nodes[children[0]].extreme_left;
    let mut indexed_y = update_indexed_y_list(bottom(nodes, first_extreme_left), 0, None);
    for (child_index, child) in children.iter().copied().enumerate().skip(1) {
        first_walk(nodes, child)?;
        let min_y = bottom(nodes, nodes[child].extreme_right);
        separate(nodes, index, child_index, &indexed_y)?;
        indexed_y = update_indexed_y_list(min_y, child_index, Some(indexed_y));
    }
    position_root(nodes, index);
    set_extremes(nodes, index);
    Ok(())
}

fn add_child_spacing(nodes: &mut [TidyNode], index: usize) {
    let mut distance = 0.0;
    let mut modifier_delta = 0.0;
    let children = nodes[index].children.clone();
    for child in children {
        distance += nodes[child].shift;
        modifier_delta += distance + nodes[child].change;
        nodes[child].modifier += modifier_delta;
    }
}

fn second_walk(nodes: &mut [TidyNode], index: usize, parent_modifier_sum: f64) {
    let modifier_sum = parent_modifier_sum + nodes[index].modifier;
    nodes[index].x = nodes[index].prelim + modifier_sum;
    add_child_spacing(nodes, index);
    let children = nodes[index].children.clone();
    for child in children {
        second_walk(nodes, child, modifier_sum);
    }
}

fn run_layout(nodes: &mut [TidyNode], root: usize) -> Result<()> {
    first_walk(nodes, root)?;
    second_walk(nodes, root, 0.0);
    Ok(())
}

fn build_tidy_node(
    original_index: usize,
    y: f64,
    layout_nodes: &[LayoutNode],
    child_indices: &[Vec<usize>],
    ancestors: &mut [bool],
    tidy_nodes: &mut Vec<TidyNode>,
) -> Result<usize> {
    if ancestors[original_index] {
        return Err(Error::InvalidModel {
            message: format!(
                "tidy-tree layout cannot contain a cycle through node {}",
                layout_nodes[original_index].id
            ),
        });
    }
    ancestors[original_index] = true;

    // The upstream plugin transposes dimensions before rotating the vertical tidy tree.
    let width = layout_nodes[original_index].height + SIBLING_GAP;
    let height = layout_nodes[original_index].width + CONNECTION_PADDING;
    let index = tidy_nodes.len();
    tidy_nodes.push(TidyNode::new(Some(original_index), width, height, y));

    let mut children = Vec::with_capacity(child_indices[original_index].len());
    for child in child_indices[original_index].iter().copied() {
        children.push(build_tidy_node(
            child,
            y + height,
            layout_nodes,
            child_indices,
            ancestors,
            tidy_nodes,
        )?);
    }
    tidy_nodes[index].children = children;
    ancestors[original_index] = false;
    Ok(index)
}

fn build_virtual_tree(
    roots: &[usize],
    layout_nodes: &[LayoutNode],
    child_indices: &[Vec<usize>],
    ancestors: &mut [bool],
    tidy_nodes: &mut Vec<TidyNode>,
) -> Result<usize> {
    let root = tidy_nodes.len();
    tidy_nodes.push(TidyNode::new(
        None,
        1.0 + SIBLING_GAP,
        1.0 + CONNECTION_PADDING,
        0.0,
    ));
    let child_y = 1.0 + CONNECTION_PADDING;
    let mut children = Vec::with_capacity(roots.len());
    for child in roots.iter().copied() {
        children.push(build_tidy_node(
            child,
            child_y,
            layout_nodes,
            child_indices,
            ancestors,
            tidy_nodes,
        )?);
    }
    tidy_nodes[root].children = children;
    Ok(root)
}

fn collect_positioned_nodes(
    tidy_nodes: &[TidyNode],
    index: usize,
    section: TreeSection,
    offset_x: f64,
    out: &mut Vec<(usize, PositionedNode)>,
) {
    let node = &tidy_nodes[index];
    if let Some(original_index) = node.original_index {
        let distance_from_root = node.y;
        let vertical_position = node.x + SIBLING_GAP / 2.0;
        let x = match section {
            TreeSection::Left => offset_x - distance_from_root,
            TreeSection::Right => offset_x + distance_from_root,
            TreeSection::Root => offset_x,
        };
        out.push((
            original_index,
            PositionedNode {
                x,
                y: vertical_position,
                width: 0.0,
                height: 0.0,
                section,
            },
        ));
    }
    for child in node.children.iter().copied() {
        collect_positioned_nodes(tidy_nodes, child, section, offset_x, out);
    }
}

fn center_tree(
    positioned: &mut [(usize, PositionedNode)],
    first_level: &[usize],
    layout_nodes: &[LayoutNode],
) {
    if first_level.is_empty() {
        return;
    }

    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (original_index, node) in positioned.iter() {
        if first_level.contains(original_index) {
            let height = layout_nodes[*original_index].height;
            min_y = min_y.min(node.y - height / 2.0);
            max_y = max_y.max(node.y + height / 2.0);
        }
    }
    let offset = if min_y.is_finite() && max_y.is_finite() {
        -(min_y + max_y) / 2.0
    } else {
        0.0
    };

    for (original_index, node) in positioned {
        let source = &layout_nodes[*original_index];
        node.x += match node.section {
            TreeSection::Left => -source.width / 2.0,
            TreeSection::Right => source.width / 2.0,
            TreeSection::Root => 0.0,
        };
        node.y += offset + source.height / 2.0;
        node.width = source.width;
        node.height = source.height;
    }
}

fn circle_intersection(
    node: PositionedNode,
    outside: &LayoutPoint,
    inside: &LayoutPoint,
) -> LayoutPoint {
    let radius = node.width.min(node.height) / 2.0;
    let dx = inside.x - outside.x;
    let dy = inside.y - outside.y;
    let length = (dx * dx + dy * dy).sqrt();
    if length == 0.0 {
        return outside.clone();
    }
    LayoutPoint {
        x: node.x - dx / length * radius,
        y: node.y - dy / length * radius,
    }
}

fn rectangle_intersection(
    node: PositionedNode,
    outside: &LayoutPoint,
    inside: &LayoutPoint,
) -> LayoutPoint {
    let half_width = node.width / 2.0;
    let half_height = node.height / 2.0;
    let q_abs = (outside.y - inside.y).abs();
    let r_abs = (outside.x - inside.x).abs();

    if (node.y - outside.y).abs() * half_width > (node.x - outside.x).abs() * half_height {
        let q = if inside.y < outside.y {
            outside.y - half_height - node.y
        } else {
            node.y - half_height - outside.y
        };
        let r = r_abs * q / q_abs;
        let mut result = LayoutPoint {
            x: if inside.x < outside.x {
                inside.x + r
            } else {
                inside.x - r_abs + r
            },
            y: if inside.y < outside.y {
                inside.y + q_abs - q
            } else {
                inside.y - q_abs + q
            },
        };
        if r == 0.0 {
            result = outside.clone();
        }
        if r_abs == 0.0 {
            result.x = outside.x;
        }
        if q_abs == 0.0 {
            result.y = outside.y;
        }
        result
    } else {
        let r = if inside.x < outside.x {
            outside.x - half_width - node.x
        } else {
            node.x - half_width - outside.x
        };
        let q = q_abs * r / r_abs;
        let mut x = if inside.x < outside.x {
            inside.x + r_abs - r
        } else {
            inside.x - r_abs + r
        };
        let mut y = if inside.y < outside.y {
            inside.y + q
        } else {
            inside.y - q
        };
        if r == 0.0 {
            x = outside.x;
            y = outside.y;
        }
        if r_abs == 0.0 {
            x = outside.x;
        }
        if q_abs == 0.0 {
            y = outside.y;
        }
        LayoutPoint { x, y }
    }
}

fn edge_intersection(
    node: PositionedNode,
    outside: &LayoutPoint,
    inside: &LayoutPoint,
    round: bool,
) -> LayoutPoint {
    if round {
        circle_intersection(node, outside, inside)
    } else {
        rectangle_intersection(node, outside, inside)
    }
}

fn build_edge(
    edge: &MindmapEdgeModel,
    source: PositionedNode,
    target: PositionedNode,
    source_round: bool,
    target_round: bool,
) -> LayoutEdge {
    let source_center = LayoutPoint {
        x: source.x,
        y: source.y,
    };
    let target_center = LayoutPoint {
        x: target.x,
        y: target.y,
    };
    let mut points = vec![source_center.clone()];

    match source.section {
        TreeSection::Left => points.push(LayoutPoint {
            x: source.x - source.width / 2.0 - TREE_GAP,
            y: source.y,
        }),
        TreeSection::Right => points.push(LayoutPoint {
            x: source.x + source.width / 2.0 + TREE_GAP,
            y: source.y,
        }),
        TreeSection::Root => match target.section {
            TreeSection::Left => points.push(LayoutPoint {
                x: source.x - source.width / 2.0 - TREE_GAP,
                y: source.y,
            }),
            TreeSection::Right => points.push(LayoutPoint {
                x: source.x + source.width / 2.0 + TREE_GAP,
                y: source.y,
            }),
            TreeSection::Root => {}
        },
    }

    match target.section {
        TreeSection::Left => points.push(LayoutPoint {
            x: target.x + target.width / 2.0 + TREE_GAP,
            y: target.y,
        }),
        TreeSection::Right => points.push(LayoutPoint {
            x: target.x - target.width / 2.0 - TREE_GAP,
            y: target.y,
        }),
        TreeSection::Root => match source.section {
            TreeSection::Left => points.push(LayoutPoint {
                x: target.x - target.width / 2.0 - TREE_GAP,
                y: target.y,
            }),
            TreeSection::Right => points.push(LayoutPoint {
                x: target.x + target.width / 2.0 + TREE_GAP,
                y: target.y,
            }),
            TreeSection::Root => {}
        },
    }
    points.push(target_center.clone());

    let source_outside = points
        .get(1)
        .cloned()
        .unwrap_or_else(|| target_center.clone());
    points[0] = edge_intersection(source, &source_outside, &source_center, source_round);
    let target_outside = points
        .get(points.len().saturating_sub(2))
        .cloned()
        .unwrap_or_else(|| source_center.clone());
    let last = points.len() - 1;
    points[last] = edge_intersection(target, &target_outside, &target_center, target_round);

    LayoutEdge {
        id: edge.id.clone(),
        from: edge.start.clone(),
        to: edge.end.clone(),
        from_cluster: None,
        to_cluster: None,
        points,
        label: None,
        start_label_left: None,
        start_label_right: None,
        end_label_left: None,
        end_label_right: None,
        start_marker: None,
        end_marker: None,
        stroke_dasharray: None,
    }
}

fn is_round_shape(shape: &str) -> bool {
    matches!(shape, "circle" | "mindmapCircle" | "cloud" | "bang")
}

pub(super) fn layout(
    layout_nodes: &mut [LayoutNode],
    model_nodes: &[MindmapNodeModel],
    model_edges: &[MindmapEdgeModel],
    edge_indices: &[(usize, usize)],
) -> Result<Vec<LayoutEdge>> {
    if layout_nodes.is_empty() {
        return Err(Error::InvalidModel {
            message: "tidy-tree layout requires at least one node".to_string(),
        });
    }

    let mut child_indices = vec![Vec::new(); layout_nodes.len()];
    let mut has_parent = vec![false; layout_nodes.len()];
    for (source, target) in edge_indices.iter().copied() {
        child_indices[source].push(target);
        has_parent[target] = true;
    }
    let root = has_parent
        .iter()
        .position(|has_parent| !has_parent)
        .unwrap_or(0);
    let root_children = &child_indices[root];
    let mut left_roots = Vec::new();
    let mut right_roots = Vec::new();
    for (index, child) in root_children.iter().copied().enumerate() {
        if index % 2 == 0 {
            left_roots.push(child);
        } else {
            right_roots.push(child);
        }
    }

    let mut tidy_nodes = Vec::with_capacity(layout_nodes.len() + 2);
    let mut ancestors = vec![false; layout_nodes.len()];
    let left_tree = (!left_roots.is_empty())
        .then(|| {
            build_virtual_tree(
                &left_roots,
                layout_nodes,
                &child_indices,
                &mut ancestors,
                &mut tidy_nodes,
            )
        })
        .transpose()?;
    let right_tree = (!right_roots.is_empty())
        .then(|| {
            build_virtual_tree(
                &right_roots,
                layout_nodes,
                &child_indices,
                &mut ancestors,
                &mut tidy_nodes,
            )
        })
        .transpose()?;

    if let Some(tree) = left_tree {
        run_layout(&mut tidy_nodes, tree)?;
    }
    if let Some(tree) = right_tree {
        run_layout(&mut tidy_nodes, tree)?;
    }

    let tree_spacing = layout_nodes[root].width / 2.0 + TREE_GAP;
    let mut positioned_pairs = Vec::with_capacity(layout_nodes.len());
    if let Some(tree) = left_tree {
        for child in tidy_nodes[tree].children.iter().copied() {
            collect_positioned_nodes(
                &tidy_nodes,
                child,
                TreeSection::Left,
                -tree_spacing,
                &mut positioned_pairs,
            );
        }
        center_tree(&mut positioned_pairs, &left_roots, layout_nodes);
    }
    let left_len = positioned_pairs.len();
    if let Some(tree) = right_tree {
        for child in tidy_nodes[tree].children.iter().copied() {
            collect_positioned_nodes(
                &tidy_nodes,
                child,
                TreeSection::Right,
                tree_spacing,
                &mut positioned_pairs,
            );
        }
        center_tree(
            &mut positioned_pairs[left_len..],
            &right_roots,
            layout_nodes,
        );
    }

    let mut positioned = vec![None; layout_nodes.len()];
    let root_position = PositionedNode {
        x: 0.0,
        y: 20.0,
        width: layout_nodes[root].width,
        height: layout_nodes[root].height,
        section: TreeSection::Root,
    };
    positioned[root] = Some(root_position);
    layout_nodes[root].x = root_position.x;
    layout_nodes[root].y = root_position.y;
    for (original_index, node) in positioned_pairs {
        layout_nodes[original_index].x = node.x;
        layout_nodes[original_index].y = node.y;
        positioned[original_index] = Some(node);
    }

    let mut shape_by_id = rustc_hash::FxHashMap::default();
    for node in model_nodes {
        shape_by_id.insert(node.id.as_str(), node.shape.as_str());
    }

    let mut edges = Vec::with_capacity(model_edges.len());
    for (edge, (source_index, target_index)) in model_edges.iter().zip(edge_indices.iter().copied())
    {
        let Some(source) = positioned[source_index] else {
            continue;
        };
        let Some(target) = positioned[target_index] else {
            continue;
        };
        let source_round = shape_by_id
            .get(layout_nodes[source_index].id.as_str())
            .is_some_and(|shape| is_round_shape(shape));
        let target_round = shape_by_id
            .get(layout_nodes[target_index].id.as_str())
            .is_some_and(|shape| is_round_shape(shape));
        edges.push(build_edge(edge, source, target, source_round, target_round));
    }
    Ok(edges)
}

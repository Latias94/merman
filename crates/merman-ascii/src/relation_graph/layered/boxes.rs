use super::super::{RelationGraphBox, find_box};
use super::lanes::parallel_lane_margin;
use crate::canvas::Canvas;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayeredRelationEdge {
    from_id: String,
    to_id: String,
    label_half_width: usize,
    label_line_count: usize,
}

impl LayeredRelationEdge {
    pub(crate) fn new(
        from_id: impl Into<String>,
        to_id: impl Into<String>,
        label_half_width: usize,
        label_line_count: usize,
    ) -> Self {
        Self {
            from_id: from_id.into(),
            to_id: to_id.into(),
            label_half_width,
            label_line_count,
        }
    }

    pub(crate) fn source_id(&self) -> &str {
        self.from_id.as_str()
    }

    pub(crate) fn target_id(&self) -> &str {
        self.to_id.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayeredRelationError {
    MissingEndpoint,
    UnrelatedBoxes,
    Crossing,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PlacedRelationGraphBox<'a> {
    pub(crate) id: &'a str,
    pub(crate) relation_box: &'a RelationGraphBox,
    pub(crate) x: usize,
    pub(crate) y: usize,
}

impl PlacedRelationGraphBox<'_> {
    pub(crate) fn id(&self) -> &str {
        self.id
    }

    pub(crate) fn x(&self) -> usize {
        self.x
    }

    pub(crate) fn width(&self) -> usize {
        self.relation_box.width()
    }

    pub(crate) fn height(&self) -> usize {
        self.relation_box.height()
    }

    pub(crate) fn y(&self) -> usize {
        self.y
    }

    pub(crate) fn center_x(&self) -> usize {
        self.x + self.width() / 2
    }

    pub(crate) fn right(&self) -> usize {
        self.x + self.width().saturating_sub(1)
    }

    pub(crate) fn bottom(&self) -> usize {
        self.y + self.height().saturating_sub(1)
    }

    pub(crate) fn draw_at(&self, canvas: &mut Canvas) {
        self.relation_box.draw_at(canvas, self.x, self.y);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LayeredRelationPlan<'a> {
    placed: Vec<PlacedRelationGraphBox<'a>>,
    width: usize,
}

impl<'a> LayeredRelationPlan<'a> {
    pub(crate) fn placed_boxes(&self) -> &[PlacedRelationGraphBox<'a>] {
        &self.placed
    }

    pub(crate) fn width(&self) -> usize {
        self.width.max(
            self.placed
                .iter()
                .map(|relation_box| relation_box.x + relation_box.width())
                .max()
                .unwrap_or(0),
        )
    }

    pub(crate) fn height(&self) -> usize {
        self.placed
            .iter()
            .map(|relation_box| relation_box.y + relation_box.height())
            .max()
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RelationGraphComponent<'a> {
    boxes: Vec<&'a RelationGraphBox>,
    edge_indices: Vec<usize>,
}

impl<'a> RelationGraphComponent<'a> {
    pub(crate) fn boxes(&self) -> &[&'a RelationGraphBox] {
        &self.boxes
    }

    pub(crate) fn edge_indices(&self) -> &[usize] {
        &self.edge_indices
    }
}

pub(crate) fn relation_components<'a>(
    boxes: &'a [RelationGraphBox],
    edges: &[LayeredRelationEdge],
) -> std::result::Result<Vec<RelationGraphComponent<'a>>, LayeredRelationError> {
    let mut incident_ids = HashSet::new();
    let mut neighbors = HashMap::<&str, Vec<&str>>::new();
    for edge in edges {
        if find_box(boxes, edge.source_id()).is_none()
            || find_box(boxes, edge.target_id()).is_none()
        {
            return Err(LayeredRelationError::MissingEndpoint);
        }

        incident_ids.insert(edge.source_id());
        incident_ids.insert(edge.target_id());
        neighbors
            .entry(edge.source_id())
            .or_default()
            .push(edge.target_id());
        neighbors
            .entry(edge.target_id())
            .or_default()
            .push(edge.source_id());
    }

    let mut components = Vec::new();
    let mut visited = HashSet::new();

    for edge in edges {
        let start_id = edge.source_id();
        if visited.contains(start_id) {
            continue;
        }

        let mut component_ids = HashSet::new();
        let mut queue = VecDeque::new();
        visited.insert(start_id);
        component_ids.insert(start_id);
        queue.push_back(start_id);

        while let Some(id) = queue.pop_front() {
            for neighbor_id in neighbors.get(id).into_iter().flatten() {
                if visited.insert(*neighbor_id) {
                    component_ids.insert(*neighbor_id);
                    queue.push_back(*neighbor_id);
                }
            }
        }

        let component_boxes = boxes
            .iter()
            .filter(|relation_box| component_ids.contains(relation_box.id()))
            .collect::<Vec<_>>();
        let component_edge_indices = edges
            .iter()
            .enumerate()
            .filter_map(|(index, edge)| {
                (component_ids.contains(edge.source_id())
                    && component_ids.contains(edge.target_id()))
                .then_some(index)
            })
            .collect::<Vec<_>>();

        components.push(RelationGraphComponent {
            boxes: component_boxes,
            edge_indices: component_edge_indices,
        });
    }

    components.extend(
        boxes
            .iter()
            .filter(|relation_box| !incident_ids.contains(relation_box.id()))
            .map(|relation_box| RelationGraphComponent {
                boxes: vec![relation_box],
                edge_indices: Vec::new(),
            }),
    );

    Ok(components)
}

pub(crate) fn plan_layered_relation_boxes<'a>(
    boxes: &'a [RelationGraphBox],
    edges: &[LayeredRelationEdge],
    horizontal_gap: usize,
) -> std::result::Result<LayeredRelationPlan<'a>, LayeredRelationError> {
    let levels = layered_relation_levels(boxes, edges)?;
    let level_groups = choose_ordered_layered_groups(boxes, edges, &levels)?;
    let (placed, width) = place_layered_boxes(&level_groups, edges, &levels, horizontal_gap);

    Ok(LayeredRelationPlan { placed, width })
}

fn layered_relation_levels(
    boxes: &[RelationGraphBox],
    edges: &[LayeredRelationEdge],
) -> std::result::Result<HashMap<String, usize>, LayeredRelationError> {
    let mut incident = HashSet::new();
    let mut outgoing = HashMap::<String, Vec<String>>::new();

    for edge in edges {
        if find_box(boxes, edge.source_id()).is_none()
            || find_box(boxes, edge.target_id()).is_none()
        {
            return Err(LayeredRelationError::MissingEndpoint);
        }

        incident.insert(edge.source_id().to_string());
        incident.insert(edge.target_id().to_string());
        outgoing
            .entry(edge.source_id().to_string())
            .or_default()
            .push(edge.target_id().to_string());
    }

    if incident.len() != boxes.len() {
        return Err(LayeredRelationError::UnrelatedBoxes);
    }

    let mut levels = HashMap::<String, usize>::new();
    let mut queue = VecDeque::new();
    for relation_box in boxes {
        let id = relation_box.id().to_string();
        levels.insert(id.clone(), 0);
        queue.push_back(id);
    }

    let level_cap = boxes.len().saturating_sub(1);
    while let Some(id) = queue.pop_front() {
        let current_level = levels.get(&id).copied().unwrap_or(0);
        let Some(children) = outgoing.get(&id) else {
            continue;
        };
        for child_id in children {
            let next_level = current_level + 1;
            if next_level > level_cap {
                continue;
            }
            let should_update = match levels.get(child_id) {
                Some(existing_level) => *existing_level < next_level,
                None => true,
            };
            if should_update {
                levels.insert(child_id.clone(), next_level);
                queue.push_back(child_id.clone());
            }
        }
    }

    Ok(levels)
}

fn choose_ordered_layered_groups<'a>(
    boxes: &'a [RelationGraphBox],
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
) -> std::result::Result<Vec<Vec<&'a RelationGraphBox>>, LayeredRelationError> {
    let base = initial_layered_groups(boxes, levels);
    let downward = order_layered_groups_downward(base.clone(), edges, levels);
    let upward = order_layered_groups_upward(base.clone(), edges, levels);
    let candidates = [
        downward.clone(),
        order_layered_groups_downward(upward.clone(), edges, levels),
        order_layered_groups_upward(downward, edges, levels),
        upward,
        base,
    ];

    let mut best: Option<(usize, Vec<Vec<&RelationGraphBox>>)> = None;
    for candidate in candidates {
        let crossings = crossing_layered_relation_count(edges, levels, &candidate);
        let should_replace = best
            .as_ref()
            .is_none_or(|(best_crossings, _)| crossings < *best_crossings);
        if should_replace {
            best = Some((crossings, candidate));
        }
    }

    let Some((crossings, level_groups)) = best else {
        return Ok(Vec::new());
    };
    if crossings == 0 {
        Ok(level_groups)
    } else {
        Err(LayeredRelationError::Crossing)
    }
}

fn crossing_layered_relation_count(
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
    level_groups: &[Vec<&RelationGraphBox>],
) -> usize {
    let mut order_by_id = HashMap::new();
    for group in level_groups {
        for (index, relation_box) in group.iter().enumerate() {
            order_by_id.insert(relation_box.id().to_string(), index);
        }
    }

    let mut crossings = 0;
    for (left_index, left) in edges.iter().enumerate() {
        let left_from_level = levels.get(left.source_id()).copied().unwrap_or(0);
        let left_to_level = levels.get(left.target_id()).copied().unwrap_or(0);
        for right in edges.iter().skip(left_index + 1) {
            if levels.get(right.source_id()).copied().unwrap_or(0) != left_from_level
                || levels.get(right.target_id()).copied().unwrap_or(0) != left_to_level
            {
                continue;
            }

            let left_from_order = order_by_id.get(left.source_id()).copied().unwrap_or(0);
            let left_to_order = order_by_id.get(left.target_id()).copied().unwrap_or(0);
            let right_from_order = order_by_id.get(right.source_id()).copied().unwrap_or(0);
            let right_to_order = order_by_id.get(right.target_id()).copied().unwrap_or(0);

            let crosses_left_to_right =
                left_from_order < right_from_order && left_to_order > right_to_order;
            let crosses_right_to_left =
                left_from_order > right_from_order && left_to_order < right_to_order;
            if crosses_left_to_right || crosses_right_to_left {
                crossings += 1;
            }
        }
    }

    crossings
}

fn initial_layered_groups<'a>(
    boxes: &'a [RelationGraphBox],
    levels: &HashMap<String, usize>,
) -> Vec<Vec<&'a RelationGraphBox>> {
    let max_level = levels.values().copied().max().unwrap_or(0);
    let mut level_groups = vec![Vec::<&RelationGraphBox>::new(); max_level + 1];
    for relation_box in boxes {
        if let Some(level) = levels.get(relation_box.id()).copied() {
            level_groups[level].push(relation_box);
        }
    }

    level_groups
}

fn order_layered_groups_downward<'a>(
    mut level_groups: Vec<Vec<&'a RelationGraphBox>>,
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
) -> Vec<Vec<&'a RelationGraphBox>> {
    let max_level = level_groups.len().saturating_sub(1);
    for level in 1..=max_level {
        let previous_order = level_groups[level - 1]
            .iter()
            .enumerate()
            .map(|(index, relation_box)| (relation_box.id(), index))
            .collect::<HashMap<_, _>>();
        let original_order = level_groups[level]
            .iter()
            .enumerate()
            .map(|(index, relation_box)| (relation_box.id(), index))
            .collect::<HashMap<_, _>>();

        level_groups[level].sort_by_key(|relation_box| {
            let neighbor_orders = edges
                .iter()
                .filter(|edge| {
                    edge.target_id() == relation_box.id()
                        && levels.get(edge.source_id()).copied() == Some(level - 1)
                })
                .filter_map(|edge| previous_order.get(edge.source_id()).copied())
                .collect::<Vec<_>>();
            let neighbor_order = barycenter_order(&neighbor_orders);
            let original_order = original_order
                .get(relation_box.id())
                .copied()
                .unwrap_or(usize::MAX);
            (neighbor_order, original_order)
        });
    }

    level_groups
}

fn order_layered_groups_upward<'a>(
    mut level_groups: Vec<Vec<&'a RelationGraphBox>>,
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
) -> Vec<Vec<&'a RelationGraphBox>> {
    let max_level = level_groups.len().saturating_sub(1);
    for level in (0..max_level).rev() {
        let next_order = level_groups[level + 1]
            .iter()
            .enumerate()
            .map(|(index, relation_box)| (relation_box.id(), index))
            .collect::<HashMap<_, _>>();
        let original_order = level_groups[level]
            .iter()
            .enumerate()
            .map(|(index, relation_box)| (relation_box.id(), index))
            .collect::<HashMap<_, _>>();

        level_groups[level].sort_by_key(|relation_box| {
            let neighbor_orders = edges
                .iter()
                .filter(|edge| {
                    edge.source_id() == relation_box.id()
                        && levels.get(edge.target_id()).copied() == Some(level + 1)
                })
                .filter_map(|edge| next_order.get(edge.target_id()).copied())
                .collect::<Vec<_>>();
            let neighbor_order = barycenter_order(&neighbor_orders);
            let original_order = original_order
                .get(relation_box.id())
                .copied()
                .unwrap_or(usize::MAX);
            (neighbor_order, original_order)
        });
    }

    level_groups
}

fn barycenter_order(neighbor_orders: &[usize]) -> usize {
    if neighbor_orders.is_empty() {
        usize::MAX
    } else {
        neighbor_orders.iter().sum::<usize>() * 2 / neighbor_orders.len()
    }
}

fn place_layered_boxes<'a>(
    level_groups: &[Vec<&'a RelationGraphBox>],
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
    horizontal_gap: usize,
) -> (Vec<PlacedRelationGraphBox<'a>>, usize) {
    let max_level = level_groups.len().saturating_sub(1);

    let group_widths = level_groups
        .iter()
        .map(|group| {
            let boxes_width = group
                .iter()
                .map(|relation_box| relation_box.width())
                .sum::<usize>();
            let gaps_width = horizontal_gap.saturating_mul(group.len().saturating_sub(1));
            boxes_width + gaps_width
        })
        .collect::<Vec<_>>();
    let max_label_half_width = edges
        .iter()
        .map(|edge| edge.label_half_width)
        .max()
        .unwrap_or(0);
    let content_width = group_widths
        .iter()
        .copied()
        .max()
        .unwrap_or(0)
        .max(max_label_half_width.saturating_mul(2).saturating_add(1))
        .saturating_add(spanning_lane_margin(level_groups, edges, levels).saturating_mul(2))
        .saturating_add(
            parallel_lane_margin(
                edges
                    .iter()
                    .map(|edge| (edge.source_id(), edge.target_id())),
            )
            .saturating_mul(2),
        );
    let global_center = content_width / 2;

    let mut placed = Vec::new();
    let mut y = 0;
    for (level, group) in level_groups.iter().enumerate() {
        let group_width = group_widths[level];
        let mut x = global_center.saturating_sub(group_width / 2);
        for relation_box in group {
            placed.push(PlacedRelationGraphBox {
                id: relation_box.id(),
                relation_box,
                x,
                y,
            });
            x += relation_box.width() + horizontal_gap;
        }

        let row_height = group
            .iter()
            .map(|relation_box| relation_box.height())
            .max()
            .unwrap_or(0);
        y += row_height;
        if level < max_level {
            y += layered_relation_gap_height(edges, levels, level);
        }
    }

    (placed, content_width)
}

fn spanning_lane_margin(
    level_groups: &[Vec<&RelationGraphBox>],
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
) -> usize {
    let has_spanning_edge = edges.iter().any(|edge| {
        let from_level = levels.get(edge.source_id()).copied().unwrap_or(0);
        let to_level = levels.get(edge.target_id()).copied().unwrap_or(0);
        from_level.abs_diff(to_level) > 1
    });
    if !has_spanning_edge {
        return 0;
    }

    level_groups
        .iter()
        .flatten()
        .map(|relation_box| relation_box.width() / 2)
        .max()
        .unwrap_or(0)
        .saturating_add(3)
}

fn layered_relation_gap_height(
    edges: &[LayeredRelationEdge],
    levels: &HashMap<String, usize>,
    level: usize,
) -> usize {
    let max_label_lines = edges
        .iter()
        .filter(|edge| relation_edge_crosses_level_gap(edge, levels, level))
        .map(|edge| edge.label_line_count)
        .max()
        .unwrap_or(0);
    if max_label_lines > 0 {
        max_label_lines + 3
    } else {
        3
    }
}

fn relation_edge_crosses_level_gap(
    edge: &LayeredRelationEdge,
    levels: &HashMap<String, usize>,
    level: usize,
) -> bool {
    let from_level = levels.get(edge.source_id()).copied().unwrap_or(0);
    let to_level = levels.get(edge.target_id()).copied().unwrap_or(0);
    let min_level = from_level.min(to_level);
    let max_level = from_level.max(to_level);

    min_level <= level && level < max_level
}

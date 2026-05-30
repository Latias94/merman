use crate::canvas::Canvas;
use crate::text::display_width;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelationGraphBox {
    id: String,
    lines: Vec<String>,
    width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LayeredRelationEdge<'a> {
    top_id: &'a str,
    bottom_id: &'a str,
    has_label: bool,
    label_half_width: usize,
}

impl<'a> LayeredRelationEdge<'a> {
    pub(crate) fn new(
        top_id: &'a str,
        bottom_id: &'a str,
        has_label: bool,
        label_half_width: usize,
    ) -> Self {
        Self {
            top_id,
            bottom_id,
            has_label,
            label_half_width,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayeredRelationError {
    MissingEndpoint,
    UnrelatedBoxes,
    Cyclic,
    Crossing,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PlacedRelationGraphBox<'a> {
    id: &'a str,
    relation_box: &'a RelationGraphBox,
    x: usize,
    y: usize,
}

impl PlacedRelationGraphBox<'_> {
    pub(crate) fn id(&self) -> &str {
        self.id
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

impl RelationGraphBox {
    pub(crate) fn new(id: String, lines: Vec<String>, width: usize) -> Self {
        Self { id, lines, width }
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn height(&self) -> usize {
        self.lines.len()
    }

    pub(crate) fn draw_at(&self, canvas: &mut Canvas, x: usize, y: usize) {
        for (row_index, line) in self.lines.iter().enumerate() {
            canvas.write_text(x, y + row_index, line);
        }
    }
}

pub(crate) fn render_stacked_boxes(boxes: &[RelationGraphBox]) -> String {
    boxes.iter().map(render_box).collect::<Vec<_>>().join("\n")
}

pub(crate) fn find_box<'a>(
    boxes: &'a [RelationGraphBox],
    id: &str,
) -> Option<&'a RelationGraphBox> {
    boxes.iter().find(|relation_box| relation_box.id == id)
}

pub(crate) fn vertical_center(
    top: &RelationGraphBox,
    bottom: &RelationGraphBox,
    extra_half_widths: &[usize],
) -> usize {
    extra_half_widths
        .iter()
        .copied()
        .fold((top.width / 2).max(bottom.width / 2), usize::max)
}

pub(crate) fn render_vertical_stack(
    top: &RelationGraphBox,
    bottom: &RelationGraphBox,
    center: usize,
    relation_lines: Vec<String>,
) -> String {
    let mut lines = Vec::new();
    lines.extend(align_box(top, center));
    lines.extend(relation_lines);
    lines.extend(align_box(bottom, center));

    let mut rendered = lines.join("\n");
    rendered.push('\n');
    rendered
}

pub(crate) fn render_parallel_vertical_stack(
    top: &RelationGraphBox,
    bottom: &RelationGraphBox,
    lanes: &[Vec<String>],
    lane_gap: usize,
) -> String {
    let lane_widths = lanes
        .iter()
        .map(|lane| {
            lane.iter()
                .map(|text| display_width(text))
                .max()
                .unwrap_or(1)
                .max(1)
        })
        .collect::<Vec<_>>();
    let lanes_width = lane_widths.iter().sum::<usize>()
        + lane_gap.saturating_mul(lane_widths.len().saturating_sub(1));
    let lane_center = lanes_width / 2;
    let center = (top.width / 2).max(bottom.width / 2).max(lane_center);
    let lane_left = center.saturating_sub(lane_center);
    let row_count = lanes.iter().map(Vec::len).max().unwrap_or(0);

    let mut relation_lines = Vec::new();
    for row_index in 0..row_count {
        let mut line = String::new();
        line.extend(std::iter::repeat_n(' ', lane_left));
        for (lane_index, lane) in lanes.iter().enumerate() {
            if lane_index > 0 {
                line.extend(std::iter::repeat_n(' ', lane_gap));
            }
            let text = lane.get(row_index).map(String::as_str).unwrap_or("");
            line.push_str(&centered_cell(text, lane_widths[lane_index]));
        }
        while line.ends_with(' ') {
            line.pop();
        }
        relation_lines.push(line);
    }

    render_vertical_stack(top, bottom, center, relation_lines)
}

pub(crate) fn parallel_lane_offset(index: usize, count: usize) -> isize {
    if count <= 1 {
        return 0;
    }
    (index as isize * 2 - (count as isize - 1)) * 3
}

pub(crate) fn offset_center(center: usize, offset: isize) -> usize {
    if offset < 0 {
        center.saturating_sub(offset.unsigned_abs())
    } else {
        center.saturating_add(offset as usize)
    }
}

pub(crate) fn spanning_lane_offset(top_width: usize, bottom_width: usize) -> isize {
    (top_width.max(bottom_width) / 2).saturating_add(3) as isize
}

pub(crate) fn marker_line(marker: char, center: usize) -> String {
    let mut line = String::new();
    line.extend(std::iter::repeat_n(' ', center));
    line.push(marker);
    line
}

pub(crate) fn centered_text_line(text: &str, center: usize) -> String {
    let mut line = String::new();
    let half_width = display_width(text) / 2;
    line.extend(std::iter::repeat_n(' ', center.saturating_sub(half_width)));
    line.push_str(text);
    line
}

pub(crate) fn relation_components<'a>(
    boxes: &'a [RelationGraphBox],
    edges: &[LayeredRelationEdge<'_>],
) -> std::result::Result<Vec<RelationGraphComponent<'a>>, LayeredRelationError> {
    // Keep every related box in one planning domain so layer reordering can still solve
    // disjoint adjacent-layer crossings; only truly isolated boxes become standalone components.
    let mut incident_ids = HashSet::new();
    for edge in edges {
        if find_box(boxes, edge.top_id).is_none() || find_box(boxes, edge.bottom_id).is_none() {
            return Err(LayeredRelationError::MissingEndpoint);
        }

        incident_ids.insert(edge.top_id);
        incident_ids.insert(edge.bottom_id);
    }

    let mut components = Vec::new();

    if !edges.is_empty() {
        let relation_boxes = boxes
            .iter()
            .filter(|relation_box| incident_ids.contains(relation_box.id()))
            .collect::<Vec<_>>();

        components.push(RelationGraphComponent {
            boxes: relation_boxes,
            edge_indices: (0..edges.len()).collect(),
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
    edges: &[LayeredRelationEdge<'_>],
    horizontal_gap: usize,
) -> std::result::Result<LayeredRelationPlan<'a>, LayeredRelationError> {
    let levels = layered_relation_levels(boxes, edges)?;
    let level_groups = ordered_layered_groups(boxes, edges, &levels);
    reject_crossing_layered_relations(edges, &levels, &level_groups)?;
    let (placed, width) = place_layered_boxes(&level_groups, edges, &levels, horizontal_gap);

    Ok(LayeredRelationPlan { placed, width })
}

fn layered_relation_levels(
    boxes: &[RelationGraphBox],
    edges: &[LayeredRelationEdge<'_>],
) -> std::result::Result<HashMap<String, usize>, LayeredRelationError> {
    let mut incident = HashSet::new();
    let mut incoming_count = boxes
        .iter()
        .map(|relation_box| (relation_box.id().to_string(), 0usize))
        .collect::<HashMap<_, _>>();
    let mut outgoing = HashMap::<String, Vec<String>>::new();

    for edge in edges {
        if find_box(boxes, edge.top_id).is_none() || find_box(boxes, edge.bottom_id).is_none() {
            return Err(LayeredRelationError::MissingEndpoint);
        }

        incident.insert(edge.top_id.to_string());
        incident.insert(edge.bottom_id.to_string());
        *incoming_count
            .entry(edge.bottom_id.to_string())
            .or_insert(0) += 1;
        outgoing
            .entry(edge.top_id.to_string())
            .or_default()
            .push(edge.bottom_id.to_string());
    }

    if incident.len() != boxes.len() {
        return Err(LayeredRelationError::UnrelatedBoxes);
    }

    let mut levels = HashMap::<String, usize>::new();
    let mut queue = boxes
        .iter()
        .filter(|relation_box| incoming_count.get(relation_box.id()).copied().unwrap_or(0) == 0)
        .map(|relation_box| relation_box.id().to_string())
        .collect::<VecDeque<_>>();

    if queue.is_empty() {
        return Err(LayeredRelationError::Cyclic);
    }

    for id in &queue {
        levels.insert(id.clone(), 0);
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
                return Err(LayeredRelationError::Cyclic);
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

    if levels.len() != boxes.len() {
        return Err(LayeredRelationError::Cyclic);
    }

    for edge in edges {
        let top_level = levels.get(edge.top_id).copied().unwrap_or(0);
        let bottom_level = levels.get(edge.bottom_id).copied().unwrap_or(0);
        if bottom_level <= top_level {
            return Err(LayeredRelationError::Cyclic);
        }
    }

    Ok(levels)
}

fn reject_crossing_layered_relations(
    edges: &[LayeredRelationEdge<'_>],
    levels: &HashMap<String, usize>,
    level_groups: &[Vec<&RelationGraphBox>],
) -> std::result::Result<(), LayeredRelationError> {
    let mut order_by_id = HashMap::new();
    for group in level_groups {
        for (index, relation_box) in group.iter().enumerate() {
            order_by_id.insert(relation_box.id().to_string(), index);
        }
    }

    for (left_index, left) in edges.iter().enumerate() {
        let left_top_level = levels.get(left.top_id).copied().unwrap_or(0);
        let left_bottom_level = levels.get(left.bottom_id).copied().unwrap_or(0);
        for right in edges.iter().skip(left_index + 1) {
            if levels.get(right.top_id).copied().unwrap_or(0) != left_top_level
                || levels.get(right.bottom_id).copied().unwrap_or(0) != left_bottom_level
            {
                continue;
            }

            let left_top_order = order_by_id.get(left.top_id).copied().unwrap_or(0);
            let left_bottom_order = order_by_id.get(left.bottom_id).copied().unwrap_or(0);
            let right_top_order = order_by_id.get(right.top_id).copied().unwrap_or(0);
            let right_bottom_order = order_by_id.get(right.bottom_id).copied().unwrap_or(0);

            let crosses_left_to_right =
                left_top_order < right_top_order && left_bottom_order > right_bottom_order;
            let crosses_right_to_left =
                left_top_order > right_top_order && left_bottom_order < right_bottom_order;
            if crosses_left_to_right || crosses_right_to_left {
                return Err(LayeredRelationError::Crossing);
            }
        }
    }

    Ok(())
}

fn ordered_layered_groups<'a>(
    boxes: &'a [RelationGraphBox],
    edges: &[LayeredRelationEdge<'_>],
    levels: &HashMap<String, usize>,
) -> Vec<Vec<&'a RelationGraphBox>> {
    let max_level = levels.values().copied().max().unwrap_or(0);
    let mut level_groups = vec![Vec::<&RelationGraphBox>::new(); max_level + 1];
    for relation_box in boxes {
        if let Some(level) = levels.get(relation_box.id()).copied() {
            level_groups[level].push(relation_box);
        }
    }

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
            let parent_order = edges
                .iter()
                .filter(|edge| {
                    edge.bottom_id == relation_box.id()
                        && levels.get(edge.top_id).copied() == Some(level - 1)
                })
                .filter_map(|edge| previous_order.get(edge.top_id).copied())
                .min()
                .unwrap_or(usize::MAX);
            let original_order = original_order
                .get(relation_box.id())
                .copied()
                .unwrap_or(usize::MAX);
            (parent_order, original_order)
        });
    }

    level_groups
}

fn place_layered_boxes<'a>(
    level_groups: &[Vec<&'a RelationGraphBox>],
    edges: &[LayeredRelationEdge<'_>],
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
        .saturating_add(spanning_lane_margin(level_groups, edges, levels).saturating_mul(2));
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
    edges: &[LayeredRelationEdge<'_>],
    levels: &HashMap<String, usize>,
) -> usize {
    let has_spanning_edge = edges.iter().any(|edge| {
        let top_level = levels.get(edge.top_id).copied().unwrap_or(0);
        let bottom_level = levels.get(edge.bottom_id).copied().unwrap_or(0);
        bottom_level > top_level + 1
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
    edges: &[LayeredRelationEdge<'_>],
    levels: &HashMap<String, usize>,
    level: usize,
) -> usize {
    let has_label = edges.iter().any(|edge| {
        levels.get(edge.top_id).copied() == Some(level)
            && levels.get(edge.bottom_id).copied() == Some(level + 1)
            && edge.has_label
    });
    if has_label { 4 } else { 3 }
}

fn render_box(relation_box: &RelationGraphBox) -> String {
    let mut rendered = relation_box.lines.join("\n");
    rendered.push('\n');
    rendered
}

fn centered_cell(text: &str, width: usize) -> String {
    let text_width = display_width(text);
    let left_padding = width.saturating_sub(text_width) / 2;
    let right_padding = width.saturating_sub(text_width + left_padding);
    let mut cell = String::new();
    cell.extend(std::iter::repeat_n(' ', left_padding));
    cell.push_str(text);
    cell.extend(std::iter::repeat_n(' ', right_padding));
    cell
}

fn align_box(relation_box: &RelationGraphBox, center: usize) -> Vec<String> {
    let left_padding = center.saturating_sub(relation_box.width / 2);
    let padding = " ".repeat(left_padding);
    relation_box
        .lines
        .iter()
        .map(|line| format!("{padding}{line}"))
        .collect()
}

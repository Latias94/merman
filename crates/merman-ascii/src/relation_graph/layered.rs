use super::{RelationGraphBox, RelationGraphLine, find_box};
use crate::canvas::Canvas;
use crate::color::AsciiColorRole;
use crate::text::display_width;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RelationOverlay {
    Glyph {
        x: usize,
        y: usize,
        ch: char,
        role: AsciiColorRole,
    },
    Text {
        center_x: usize,
        y: usize,
        text: String,
        role: AsciiColorRole,
    },
}

impl RelationOverlay {
    pub(crate) fn glyph(x: usize, y: usize, ch: char, role: AsciiColorRole) -> Self {
        Self::Glyph { x, y, ch, role }
    }

    pub(crate) fn text(center_x: usize, y: usize, text: String, role: AsciiColorRole) -> Self {
        Self::Text {
            center_x,
            y,
            text,
            role,
        }
    }

    fn draw_at(&self, canvas: &mut Canvas) {
        match self {
            RelationOverlay::Glyph { x, y, ch, role } => canvas.set_role(*x, *y, *ch, *role),
            RelationOverlay::Text {
                center_x,
                y,
                text,
                role,
            } => write_centered_relation_text(canvas, *center_x, *y, text, *role),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayeredRelationRouteGeometry {
    from_x: usize,
    to_x: usize,
    source_path_start_y: usize,
    source_marker_y: usize,
    route_y: usize,
    target_marker_y: usize,
    target_path_end_y: usize,
}

impl LayeredRelationRouteGeometry {
    pub(crate) fn from_x(&self) -> usize {
        self.from_x
    }

    pub(crate) fn to_x(&self) -> usize {
        self.to_x
    }

    pub(crate) fn route_y(&self) -> usize {
        self.route_y
    }

    pub(crate) fn source_marker_y(&self) -> usize {
        self.source_marker_y
    }

    pub(crate) fn target_marker_y(&self) -> usize {
        self.target_marker_y
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayeredRelationRoutePlan {
    geometry: LayeredRelationRouteGeometry,
    vertical_char: char,
    horizontal_char: char,
    relation_chars: RelationLineChars,
    overlays: Vec<RelationOverlay>,
}

impl LayeredRelationRoutePlan {
    pub(crate) fn new(
        geometry: LayeredRelationRouteGeometry,
        vertical_char: char,
        horizontal_char: char,
        relation_chars: RelationLineChars,
        overlays: Vec<RelationOverlay>,
    ) -> Self {
        Self {
            geometry,
            vertical_char,
            horizontal_char,
            relation_chars,
            overlays,
        }
    }

    pub(crate) fn draw_at(&self, canvas: &mut Canvas) {
        draw_relation_span_inclusive(
            canvas,
            self.geometry.from_x,
            self.geometry.source_path_start_y,
            self.geometry.route_y,
            self.vertical_char,
            self.relation_chars,
        );
        if self.geometry.from_x != self.geometry.to_x {
            let left = self.geometry.from_x.min(self.geometry.to_x);
            let right = self.geometry.from_x.max(self.geometry.to_x);
            for x in left..=right {
                put_relation_char(
                    canvas,
                    x,
                    self.geometry.route_y,
                    self.horizontal_char,
                    self.relation_chars,
                );
            }
        }
        draw_relation_span_exclusive(
            canvas,
            self.geometry.to_x,
            self.geometry.route_y,
            self.geometry.target_path_end_y,
            self.vertical_char,
            self.relation_chars,
        );

        for overlay in &self.overlays {
            overlay.draw_at(canvas);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LayeredRelationRouteProfile {
    min_vertical_gap: usize,
    source_path_start_offset: usize,
    route_y_offset_from_target: usize,
    target_path_end_offset_from_target: usize,
}

impl LayeredRelationRouteProfile {
    pub(crate) const fn new(
        min_vertical_gap: usize,
        source_path_start_offset: usize,
        route_y_offset_from_target: usize,
        target_path_end_offset_from_target: usize,
    ) -> Self {
        Self {
            min_vertical_gap,
            source_path_start_offset,
            route_y_offset_from_target,
            target_path_end_offset_from_target,
        }
    }

    pub(crate) const fn class() -> Self {
        Self::new(1, 1, 1, 0)
    }

    pub(crate) const fn er() -> Self {
        Self::new(2, 2, 2, 1)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LayeredRelationRouteRequest<'boxes, 'graph> {
    placed_boxes: &'boxes [PlacedRelationGraphBox<'graph>],
    top: &'boxes PlacedRelationGraphBox<'graph>,
    bottom: &'boxes PlacedRelationGraphBox<'graph>,
    lane_offset: isize,
    profile: LayeredRelationRouteProfile,
}

impl<'boxes, 'graph> LayeredRelationRouteRequest<'boxes, 'graph> {
    pub(crate) fn new(
        placed_boxes: &'boxes [PlacedRelationGraphBox<'graph>],
        top: &'boxes PlacedRelationGraphBox<'graph>,
        bottom: &'boxes PlacedRelationGraphBox<'graph>,
        lane_offset: isize,
        profile: LayeredRelationRouteProfile,
    ) -> Self {
        Self {
            placed_boxes,
            top,
            bottom,
            lane_offset,
            profile,
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RelationLineChars {
    line_chars: [char; 4],
    junction: char,
}

impl RelationLineChars {
    pub(crate) fn new(line_chars: [char; 4], junction: char) -> Self {
        Self {
            line_chars,
            junction,
        }
    }

    fn contains(self, ch: char) -> bool {
        self.line_chars.contains(&ch) || ch == self.junction
    }
}

impl<'a> RelationGraphComponent<'a> {
    pub(crate) fn boxes(&self) -> &[&'a RelationGraphBox] {
        &self.boxes
    }

    pub(crate) fn edge_indices(&self) -> &[usize] {
        &self.edge_indices
    }
}

pub(crate) fn parallel_lane_offset(index: usize, count: usize) -> isize {
    if count <= 1 {
        return 0;
    }
    (index as isize * 2 - (count as isize - 1)) * 3
}

pub(crate) fn parallel_relation_lane_offsets<'a>(
    endpoints: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<isize> {
    let endpoints = endpoints.into_iter().collect::<Vec<_>>();
    let mut counts = HashMap::<(&str, &str), usize>::new();
    for endpoint in &endpoints {
        *counts.entry(*endpoint).or_insert(0) += 1;
    }

    let mut seen = HashMap::<(&str, &str), usize>::new();
    endpoints
        .into_iter()
        .map(|endpoint| {
            let index = seen.entry(endpoint).or_insert(0);
            let offset = parallel_lane_offset(*index, counts[&endpoint]);
            *index += 1;
            offset
        })
        .collect()
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

pub(crate) fn spanning_lane_offset_around_intermediate_boxes(
    placed_boxes: &[PlacedRelationGraphBox<'_>],
    top: &PlacedRelationGraphBox<'_>,
    bottom: &PlacedRelationGraphBox<'_>,
    lane_offset: isize,
) -> isize {
    let lower_bound = top.y().min(bottom.y());
    let upper_bound = top.bottom().max(bottom.bottom());
    let intermediate_boxes = placed_boxes
        .iter()
        .filter(|placed_box| placed_box.y() > lower_bound && placed_box.bottom() < upper_bound)
        .collect::<Vec<_>>();
    if intermediate_boxes.is_empty() {
        return lane_offset;
    }

    let route_clearance = intermediate_boxes
        .iter()
        .map(|placed_box| placed_box.width() / 2)
        .max()
        .unwrap_or(0);
    let spanning_offset = spanning_lane_offset(
        top.width().max(route_clearance.saturating_mul(2)),
        bottom.width(),
    );
    let left_offset = lane_offset - spanning_offset;
    let right_offset = lane_offset + spanning_offset;
    let left_is_clear =
        !route_column_crosses_any_box(top.center_x(), left_offset, &intermediate_boxes);
    let right_is_clear =
        !route_column_crosses_any_box(top.center_x(), right_offset, &intermediate_boxes);

    match (left_is_clear, right_is_clear) {
        (true, false) => left_offset,
        (false, true) => right_offset,
        (true, true) if lane_offset < 0 => left_offset,
        (true, true) if top_is_left_of_intermediate_boxes(top, &intermediate_boxes) => left_offset,
        (true, true) => right_offset,
        (false, false) if top_is_left_of_intermediate_boxes(top, &intermediate_boxes) => {
            route_column_left_of_intermediate_boxes(top, &intermediate_boxes)
        }
        (false, false) => route_column_right_of_intermediate_boxes(top, &intermediate_boxes),
    }
}

pub(crate) fn plan_layered_relation_route(
    request: LayeredRelationRouteRequest<'_, '_>,
) -> LayeredRelationRouteGeometry {
    let lane_offset = spanning_lane_offset_around_intermediate_boxes(
        request.placed_boxes,
        request.top,
        request.bottom,
        request.lane_offset,
    );
    let from_x = offset_center(request.top.center_x(), lane_offset);
    let to_x = offset_center(request.bottom.center_x(), lane_offset);
    let source_top = request.top.y();
    let source_bottom = request.top.bottom();
    let target_top = request.bottom.y();
    let target_bottom = request.bottom.bottom();

    if target_top > source_bottom.saturating_add(request.profile.min_vertical_gap) {
        return LayeredRelationRouteGeometry {
            from_x,
            to_x,
            source_path_start_y: source_bottom
                .saturating_add(request.profile.source_path_start_offset),
            source_marker_y: source_bottom.saturating_add(1),
            route_y: target_top.saturating_sub(request.profile.route_y_offset_from_target),
            target_marker_y: target_top.saturating_sub(1),
            target_path_end_y: target_top
                .saturating_sub(request.profile.target_path_end_offset_from_target),
        };
    }

    if source_top > target_bottom.saturating_add(request.profile.min_vertical_gap) {
        return LayeredRelationRouteGeometry {
            from_x,
            to_x,
            source_path_start_y: source_top
                .saturating_sub(request.profile.source_path_start_offset),
            source_marker_y: source_top.saturating_sub(1),
            route_y: target_bottom.saturating_add(request.profile.route_y_offset_from_target),
            target_marker_y: target_bottom.saturating_add(1),
            target_path_end_y: target_bottom
                .saturating_add(request.profile.target_path_end_offset_from_target),
        };
    }

    LayeredRelationRouteGeometry {
        from_x,
        to_x,
        source_path_start_y: source_bottom.saturating_add(request.profile.source_path_start_offset),
        source_marker_y: source_bottom.saturating_add(1),
        route_y: source_bottom
            .max(target_bottom)
            .saturating_add(request.profile.route_y_offset_from_target),
        target_marker_y: target_bottom.saturating_add(1),
        target_path_end_y: target_bottom
            .saturating_add(request.profile.target_path_end_offset_from_target),
    }
}

fn route_column_crosses_any_box(
    center_x: usize,
    lane_offset: isize,
    boxes: &[&PlacedRelationGraphBox<'_>],
) -> bool {
    let column = offset_center(center_x, lane_offset);
    boxes
        .iter()
        .any(|placed_box| column >= placed_box.x() && column <= placed_box.right())
}

fn draw_relation_span_inclusive(
    canvas: &mut Canvas,
    x: usize,
    start_y: usize,
    end_y: usize,
    ch: char,
    chars: RelationLineChars,
) {
    let start = start_y.min(end_y);
    let end = start_y.max(end_y);
    for y in start..=end {
        put_relation_char(canvas, x, y, ch, chars);
    }
}

fn draw_relation_span_exclusive(
    canvas: &mut Canvas,
    x: usize,
    start_y: usize,
    end_y: usize,
    ch: char,
    chars: RelationLineChars,
) {
    if start_y <= end_y {
        for y in start_y..end_y {
            put_relation_char(canvas, x, y, ch, chars);
        }
        return;
    }

    for y in (end_y + 1)..=start_y {
        put_relation_char(canvas, x, y, ch, chars);
    }
}

fn top_is_left_of_intermediate_boxes(
    top: &PlacedRelationGraphBox<'_>,
    intermediate_boxes: &[&PlacedRelationGraphBox<'_>],
) -> bool {
    intermediate_boxes
        .iter()
        .any(|placed_box| top.center_x() < placed_box.center_x())
}

fn route_column_left_of_intermediate_boxes(
    top: &PlacedRelationGraphBox<'_>,
    intermediate_boxes: &[&PlacedRelationGraphBox<'_>],
) -> isize {
    let target = intermediate_boxes
        .iter()
        .map(|placed_box| placed_box.x())
        .min()
        .unwrap_or(0)
        .saturating_sub(2);
    target as isize - top.center_x() as isize
}

fn route_column_right_of_intermediate_boxes(
    top: &PlacedRelationGraphBox<'_>,
    intermediate_boxes: &[&PlacedRelationGraphBox<'_>],
) -> isize {
    let target = intermediate_boxes
        .iter()
        .map(|placed_box| placed_box.right())
        .max()
        .unwrap_or(top.center_x())
        .saturating_add(2);
    target as isize - top.center_x() as isize
}

pub(crate) fn marker_line_with_role(
    marker: char,
    center: usize,
    role: AsciiColorRole,
) -> RelationGraphLine {
    let mut line = String::new();
    line.extend(std::iter::repeat_n(' ', center));
    line.push(marker);
    let mut roles = vec![None; center];
    roles.push(Some(role));
    RelationGraphLine::new(line, roles)
}

pub(crate) fn centered_text_line_with_role(
    text: &str,
    center: usize,
    role: AsciiColorRole,
) -> RelationGraphLine {
    let mut line = String::new();
    let half_width = display_width(text) / 2;
    let left_padding = center.saturating_sub(half_width);
    line.extend(std::iter::repeat_n(' ', left_padding));
    line.push_str(text);

    let mut roles = vec![None; left_padding];
    roles.extend(std::iter::repeat_n(Some(role), text.chars().count()));
    RelationGraphLine::new(line, roles)
}

pub(crate) fn put_relation_char(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    ch: char,
    chars: RelationLineChars,
) {
    let next = match canvas.get(x, y) {
        Some(existing) if existing == ' ' || existing == ch => ch,
        Some(existing) if chars.contains(existing) && chars.contains(ch) => chars.junction,
        _ => ch,
    };
    let role = if next == chars.junction {
        AsciiColorRole::Junction
    } else {
        AsciiColorRole::EdgeLine
    };
    canvas.set_role(x, y, next, role);
}

pub(crate) fn write_centered_relation_text(
    canvas: &mut Canvas,
    center_x: usize,
    y: usize,
    text: &str,
    role: AsciiColorRole,
) {
    let text_half_width = display_width(text) / 2;
    canvas.write_text_role(center_x.saturating_sub(text_half_width), y, text, role);
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
    let mut outgoing = HashMap::<String, Vec<String>>::new();

    for edge in edges {
        if find_box(boxes, edge.top_id).is_none() || find_box(boxes, edge.bottom_id).is_none() {
            return Err(LayeredRelationError::MissingEndpoint);
        }

        incident.insert(edge.top_id.to_string());
        incident.insert(edge.bottom_id.to_string());
        outgoing
            .entry(edge.top_id.to_string())
            .or_default()
            .push(edge.bottom_id.to_string());
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

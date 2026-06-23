use crate::canvas::Canvas;
use crate::color::{AsciiColorMode, AsciiColorRole};
use crate::options::AsciiRenderOptions;
use crate::text::{StyledLine, display_width};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelationGraphLine {
    text: String,
    line: StyledLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelationGraphBox {
    id: String,
    lines: Vec<RelationGraphLine>,
    width: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct RelationStackPlan<'a> {
    top: &'a RelationGraphBox,
    bottom: &'a RelationGraphBox,
    center: usize,
    relation_lines: Vec<RelationGraphLine>,
}

impl<'a> RelationStackPlan<'a> {
    pub(crate) fn from_centered_rows(
        top: &'a RelationGraphBox,
        bottom: &'a RelationGraphBox,
        extra_half_widths: &[usize],
        build_rows: impl FnOnce(usize) -> Vec<RelationGraphLine>,
    ) -> Self {
        let center = vertical_center(top, bottom, extra_half_widths);
        let relation_lines = build_rows(center);
        Self {
            top,
            bottom,
            center,
            relation_lines,
        }
    }

    pub(crate) fn render_with_options(&self, options: &AsciiRenderOptions) -> String {
        render_vertical_stack_with_options(
            self.top,
            self.bottom,
            self.center,
            self.relation_lines.clone(),
            options,
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RelationParallelPlan<'a> {
    top: &'a RelationGraphBox,
    bottom: &'a RelationGraphBox,
    center: usize,
    lane_left: usize,
    lane_gap: usize,
    lane_widths: Vec<usize>,
    lanes: Vec<Vec<RelationGraphLine>>,
}

impl<'a> RelationParallelPlan<'a> {
    pub(crate) fn new(
        top: &'a RelationGraphBox,
        bottom: &'a RelationGraphBox,
        lanes: Vec<Vec<RelationGraphLine>>,
        lane_gap: usize,
    ) -> Self {
        let lane_widths = lanes
            .iter()
            .map(|lane| {
                lane.iter()
                    .map(|line| display_width(line.text()))
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

        Self {
            top,
            bottom,
            center,
            lane_left,
            lane_gap,
            lane_widths,
            lanes,
        }
    }

    pub(crate) fn render_with_options(&self, options: &AsciiRenderOptions) -> String {
        if options.color_mode == AsciiColorMode::Plain {
            let mut relation_lines = Vec::new();
            let row_count = self.lanes.iter().map(Vec::len).max().unwrap_or(0);
            for row_index in 0..row_count {
                let mut line = String::new();
                line.extend(std::iter::repeat_n(' ', self.lane_left));
                for (lane_index, lane) in self.lanes.iter().enumerate() {
                    if lane_index > 0 {
                        line.extend(std::iter::repeat_n(' ', self.lane_gap));
                    }
                    let text = lane.get(row_index).map(|line| line.text()).unwrap_or("");
                    line.push_str(&centered_cell(text, self.lane_widths[lane_index]));
                }
                while line.ends_with(' ') {
                    line.pop();
                }
                relation_lines.push(line);
            }

            return render_vertical_stack(self.top, self.bottom, self.center, relation_lines);
        }

        let mut relation_lines = Vec::new();
        let row_count = self.lanes.iter().map(Vec::len).max().unwrap_or(0);
        for row_index in 0..row_count {
            let mut parts = Vec::new();
            parts.push(RelationGraphLine::plain(" ".repeat(self.lane_left)));
            for (lane_index, lane) in self.lanes.iter().enumerate() {
                if lane_index > 0 {
                    parts.push(RelationGraphLine::plain(" ".repeat(self.lane_gap)));
                }
                let cell = lane
                    .get(row_index)
                    .cloned()
                    .unwrap_or_else(|| RelationGraphLine::plain(String::new()));
                parts.push(centered_cell_line(&cell, self.lane_widths[lane_index]));
            }
            relation_lines.push(concat_relation_lines(parts));
        }

        render_vertical_stack_with_options(
            self.top,
            self.bottom,
            self.center,
            relation_lines,
            options,
        )
    }

    #[allow(dead_code)]
    fn render_plain(&self) -> String {
        let mut relation_lines = Vec::new();
        let row_count = self.lanes.iter().map(Vec::len).max().unwrap_or(0);
        for row_index in 0..row_count {
            let mut line = String::new();
            line.extend(std::iter::repeat_n(' ', self.lane_left));
            for (lane_index, lane) in self.lanes.iter().enumerate() {
                if lane_index > 0 {
                    line.extend(std::iter::repeat_n(' ', self.lane_gap));
                }
                let text = lane.get(row_index).map(|line| line.text()).unwrap_or("");
                line.push_str(&centered_cell(text, self.lane_widths[lane_index]));
            }
            while line.ends_with(' ') {
                line.pop();
            }
            relation_lines.push(line);
        }

        render_vertical_stack(self.top, self.bottom, self.center, relation_lines)
    }
}

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
    from_y: usize,
    to_x: usize,
    to_y: usize,
    source_path_start_y: usize,
    route_y: usize,
    target_path_end_y: usize,
}

impl LayeredRelationRouteGeometry {
    pub(crate) fn from_x(&self) -> usize {
        self.from_x
    }

    pub(crate) fn from_y(&self) -> usize {
        self.from_y
    }

    pub(crate) fn to_x(&self) -> usize {
        self.to_x
    }

    pub(crate) fn to_y(&self) -> usize {
        self.to_y
    }

    pub(crate) fn route_y(&self) -> usize {
        self.route_y
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
        if self.geometry.source_path_start_y <= self.geometry.route_y {
            for y in self.geometry.source_path_start_y..=self.geometry.route_y {
                put_relation_char(
                    canvas,
                    self.geometry.from_x,
                    y,
                    self.vertical_char,
                    self.relation_chars,
                );
            }
        }
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
        if self.geometry.route_y < self.geometry.target_path_end_y {
            for y in self.geometry.route_y..self.geometry.target_path_end_y {
                put_relation_char(
                    canvas,
                    self.geometry.to_x,
                    y,
                    self.vertical_char,
                    self.relation_chars,
                );
            }
        }

        for overlay in &self.overlays {
            overlay.draw_at(canvas);
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

impl RelationGraphLine {
    pub(crate) fn new(text: String, roles: Vec<Option<AsciiColorRole>>) -> Self {
        let line = StyledLine::text_with_roles(&text, roles);
        Self { text, line }
    }

    pub(crate) fn plain(text: String) -> Self {
        let line = StyledLine::plain_text(&text);
        Self { text, line }
    }

    pub(crate) fn with_role(text: String, role: AsciiColorRole) -> Self {
        let line = StyledLine::role_text(&text, role);
        Self { text, line }
    }

    pub(crate) fn box_border(
        left: char,
        right: char,
        horizontal: char,
        content_width: usize,
        role: AsciiColorRole,
    ) -> Self {
        let mut line = StyledLine::new();
        line.push_role_char(left, role);
        line.push_role_repeat(horizontal, content_width, role);
        line.push_role_char(right, role);
        Self::from_styled(line)
    }

    pub(crate) fn box_content(
        text: &str,
        content_width: usize,
        padding: usize,
        vertical: char,
        border_role: AsciiColorRole,
        text_role: AsciiColorRole,
    ) -> Self {
        let text_width = display_width(text);
        let trailing = content_width.saturating_sub(padding + text_width);

        let mut line = StyledLine::new();
        line.push_role_char(vertical, border_role);
        line.push_spaces(padding);
        line.push_role_text(text, text_role);
        line.push_spaces(trailing);
        line.push_role_char(vertical, border_role);
        Self::from_styled(line)
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn draw_at(&self, canvas: &mut Canvas, x: usize, y: usize) {
        self.line.write_to_at(canvas, x, y);
    }

    fn from_styled(line: StyledLine) -> Self {
        let text = line.text();
        Self { text, line }
    }
}

impl RelationGraphBox {
    #[allow(dead_code)]
    pub(crate) fn new(id: String, lines: Vec<String>, width: usize) -> Self {
        Self {
            id,
            lines: lines.into_iter().map(RelationGraphLine::plain).collect(),
            width,
        }
    }

    pub(crate) fn new_with_lines(id: String, lines: Vec<RelationGraphLine>, width: usize) -> Self {
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
            line.draw_at(canvas, x, y + row_index);
        }
    }
}

pub(crate) fn render_stacked_boxes(boxes: &[RelationGraphBox]) -> String {
    boxes.iter().map(render_box).collect::<Vec<_>>().join("\n")
}

pub(crate) fn render_stacked_boxes_with_options(
    boxes: &[RelationGraphBox],
    options: &AsciiRenderOptions,
) -> String {
    if options.color_mode == AsciiColorMode::Plain {
        return render_stacked_boxes(boxes);
    }

    let mut lines = Vec::new();
    for (index, relation_box) in boxes.iter().enumerate() {
        if index > 0 {
            lines.push(RelationGraphLine::plain(String::new()));
        }
        lines.extend(relation_box.lines.iter().cloned());
    }

    render_lines_with_options(&lines, options)
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

pub(crate) fn render_vertical_stack_with_options(
    top: &RelationGraphBox,
    bottom: &RelationGraphBox,
    center: usize,
    relation_lines: Vec<RelationGraphLine>,
    options: &AsciiRenderOptions,
) -> String {
    if options.color_mode == AsciiColorMode::Plain {
        return render_vertical_stack(
            top,
            bottom,
            center,
            relation_lines
                .into_iter()
                .map(|line| line.text().to_string())
                .collect(),
        );
    }

    let mut lines = Vec::new();
    lines.extend(align_box_lines(top, center));
    lines.extend(relation_lines);
    lines.extend(align_box_lines(bottom, center));

    render_lines_with_options(&lines, options)
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
    let intermediate_boxes = placed_boxes
        .iter()
        .filter(|placed_box| placed_box.y() > top.y() && placed_box.y() < bottom.y())
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
    placed_boxes: &[PlacedRelationGraphBox<'_>],
    top: &PlacedRelationGraphBox<'_>,
    bottom: &PlacedRelationGraphBox<'_>,
    lane_offset: isize,
    min_vertical_gap: usize,
    source_path_start_offset: usize,
    route_y_offset_from_target: usize,
    target_path_end_offset_from_target: usize,
) -> Option<LayeredRelationRouteGeometry> {
    let lane_offset =
        spanning_lane_offset_around_intermediate_boxes(placed_boxes, top, bottom, lane_offset);
    let from_x = offset_center(top.center_x(), lane_offset);
    let from_y = top.bottom();
    let to_x = offset_center(bottom.center_x(), lane_offset);
    let to_y = bottom.y();
    if to_y <= from_y.saturating_add(min_vertical_gap) {
        return None;
    }

    Some(LayeredRelationRouteGeometry {
        from_x,
        from_y,
        to_x,
        to_y,
        source_path_start_y: from_y.saturating_add(source_path_start_offset),
        route_y: to_y.saturating_sub(route_y_offset_from_target),
        target_path_end_y: to_y.saturating_sub(target_path_end_offset_from_target),
    })
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
    let mut rendered = relation_box
        .lines
        .iter()
        .map(RelationGraphLine::text)
        .collect::<Vec<_>>()
        .join("\n");
    rendered.push('\n');
    rendered
}

fn render_lines_with_options(lines: &[RelationGraphLine], options: &AsciiRenderOptions) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let width = lines.iter().map(line_char_width).max().unwrap_or(0);
    if width == 0 {
        return "\n".repeat(lines.len());
    }

    let mut canvas = Canvas::new(width, lines.len());
    for (y, line) in lines.iter().enumerate() {
        line.draw_at(&mut canvas, 0, y);
    }

    canvas.finish_trimmed_with_options(options)
}

fn line_char_width(line: &RelationGraphLine) -> usize {
    display_width(line.text())
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

fn centered_cell_line(line: &RelationGraphLine, width: usize) -> RelationGraphLine {
    let text_width = display_width(line.text());
    let left_padding = width.saturating_sub(text_width) / 2;
    let right_padding = width.saturating_sub(text_width + left_padding);
    padded_line(line, left_padding, right_padding)
}

fn align_box(relation_box: &RelationGraphBox, center: usize) -> Vec<String> {
    let left_padding = center.saturating_sub(relation_box.width / 2);
    let padding = " ".repeat(left_padding);
    relation_box
        .lines
        .iter()
        .map(|line| format!("{padding}{}", line.text()))
        .collect()
}

fn align_box_lines(relation_box: &RelationGraphBox, center: usize) -> Vec<RelationGraphLine> {
    let left_padding = center.saturating_sub(relation_box.width / 2);
    relation_box
        .lines
        .iter()
        .map(|line| padded_line(line, left_padding, 0))
        .collect()
}

fn padded_line(line: &RelationGraphLine, left: usize, right: usize) -> RelationGraphLine {
    let mut padded = StyledLine::blank(left);
    padded.push_line(&line.line);
    padded.push_spaces(right);
    RelationGraphLine::from_styled(padded)
}

fn concat_relation_lines(parts: Vec<RelationGraphLine>) -> RelationGraphLine {
    let mut line = StyledLine::new();
    for part in parts {
        line.push_line(&part.line);
    }
    RelationGraphLine::from_styled(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::Canvas;
    use crate::{AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiRenderOptions, AsciiRgb};

    #[test]
    fn render_stacked_boxes_preserves_plain_text() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string(), "|".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string(), "|".to_string()], 1),
        ];

        assert_eq!(render_stacked_boxes(&boxes), "A\n|\n\nB\n|\n");
    }

    #[test]
    fn relation_graph_box_draws_role_lines_to_trimmed_canvas() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 2, 3));
        let line = RelationGraphLine::with_role("AB".to_string(), AsciiColorRole::Text);
        let relation_box = RelationGraphBox::new_with_lines("box".to_string(), vec![line], 2);
        let mut canvas = Canvas::new(4, 1);
        relation_box.draw_at(&mut canvas, 0, 0);

        let output = canvas.finish_trimmed_with_options(
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::TrueColor)
                .with_color_theme(theme),
        );

        assert_eq!(output, "\u{1b}[38;2;1;2;3mAB\u{1b}[0m\n");
    }

    #[test]
    fn relation_graph_box_content_line_preserves_border_and_text_roles() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::NodeBorder, AsciiRgb::from_hex24(0x111111))
            .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x222222));
        let line = RelationGraphLine::box_content(
            "A",
            3,
            1,
            '|',
            AsciiColorRole::NodeBorder,
            AsciiColorRole::Text,
        );
        let mut canvas = Canvas::new(5, 1);

        line.draw_at(&mut canvas, 0, 0);

        assert_eq!(line.text(), "| A |");
        assert_eq!(
            canvas.finish_trimmed_with_options(
                &AsciiRenderOptions::ascii()
                    .with_color_mode(AsciiColorMode::Html)
                    .with_color_theme(theme),
            ),
            "<span style=\"color:#111111\">|</span> <span style=\"color:#222222\">A</span> <span style=\"color:#111111\">|</span>\n"
        );
    }

    #[test]
    fn relation_line_chars_merge_crossing_relation_lines_to_junction() {
        let chars = RelationLineChars::new(['-', '|', '.', ':'], '+');
        let mut canvas = Canvas::new(1, 1);
        canvas.set_role(0, 0, '-', AsciiColorRole::EdgeLine);

        put_relation_char(&mut canvas, 0, 0, '|', chars);

        assert_eq!(canvas.get(0, 0), Some('+'));
        assert_eq!(
            canvas.get_color(0, 0),
            Some(crate::canvas::CanvasColor::Role(AsciiColorRole::Junction))
        );
    }

    #[test]
    fn parallel_relation_lane_offsets_group_by_endpoint_pair() {
        let offsets =
            parallel_relation_lane_offsets([("A", "B"), ("A", "B"), ("A", "C"), ("A", "B")]);

        assert_eq!(offsets, vec![-6, 0, 0, 6]);
    }

    #[test]
    fn layered_relation_route_plan_draws_route_and_overlays() {
        let top_box = RelationGraphBox::new("top".to_string(), vec!["AAA".to_string()], 3);
        let bottom_box = RelationGraphBox::new("bottom".to_string(), vec!["BBB".to_string()], 3);
        let placed = vec![
            PlacedRelationGraphBox {
                id: "top",
                relation_box: &top_box,
                x: 0,
                y: 0,
            },
            PlacedRelationGraphBox {
                id: "bottom",
                relation_box: &bottom_box,
                x: 0,
                y: 4,
            },
        ];
        let geometry = plan_layered_relation_route(&placed, &placed[0], &placed[1], 0, 1, 1, 1, 0)
            .expect("route has enough vertical space");
        let route = LayeredRelationRoutePlan::new(
            geometry.clone(),
            '|',
            '-',
            RelationLineChars::new(['-', '|', '.', ':'], '+'),
            vec![
                RelationOverlay::text(
                    geometry.from_x(),
                    geometry.from_y() + 1,
                    "T".to_string(),
                    AsciiColorRole::EdgeArrow,
                ),
                RelationOverlay::text(
                    (geometry.from_x() + geometry.to_x()) / 2,
                    geometry.route_y().saturating_sub(1),
                    "L".to_string(),
                    AsciiColorRole::EdgeLabel,
                ),
                RelationOverlay::text(
                    geometry.to_x(),
                    geometry.to_y().saturating_sub(1),
                    "B".to_string(),
                    AsciiColorRole::EdgeArrow,
                ),
            ],
        );
        let mut canvas = Canvas::new(3, 5);

        route.draw_at(&mut canvas);

        assert_eq!(canvas.get(1, 1), Some('T'));
        assert_eq!(canvas.get(1, 2), Some('L'));
        assert_eq!(canvas.get(1, 3), Some('B'));
        assert_eq!(
            canvas.get_color(1, 1),
            Some(crate::canvas::CanvasColor::Role(AsciiColorRole::EdgeArrow))
        );
        assert_eq!(
            canvas.get_color(1, 2),
            Some(crate::canvas::CanvasColor::Role(AsciiColorRole::EdgeLabel))
        );
    }
}

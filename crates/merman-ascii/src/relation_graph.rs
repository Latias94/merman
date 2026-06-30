use crate::canvas::Canvas;
use crate::color::{AsciiColorMode, AsciiColorRole};
use crate::options::AsciiRenderOptions;
use crate::text::{StyledLine, display_width, split_label_lines};
use crate::{AsciiError, Result};
use std::collections::HashSet;
mod layered;
mod summary;

pub(crate) use self::layered::*;
pub(crate) use self::summary::*;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RelationGraphBoxStyle {
    pub(crate) top_left: char,
    pub(crate) top_right: char,
    pub(crate) bottom_left: char,
    pub(crate) bottom_right: char,
    pub(crate) horizontal: char,
    pub(crate) vertical: char,
    pub(crate) separator_left: char,
    pub(crate) separator_right: char,
    pub(crate) border_role: AsciiColorRole,
    pub(crate) text_role: AsciiColorRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelationGraphLabel {
    lines: Vec<String>,
    width: usize,
}

pub(crate) trait RelationComponentAdapter<R> {
    fn build_edges(&self, relation: &R) -> LayeredRelationEdge;

    fn is_same_endpoint_parallel(&self, relations: &[R]) -> bool;

    fn is_self_relation(&self, relation: &R) -> bool;

    fn render_self_relation(
        &self,
        relation_box: &RelationGraphBox,
        relation: &R,
        options: &AsciiRenderOptions,
    ) -> Result<String>;

    fn render_self_relations(
        &self,
        relation_box: &RelationGraphBox,
        relations: &[R],
        options: &AsciiRenderOptions,
    ) -> Result<String>;

    fn layered_horizontal_gap(&self) -> usize;

    fn layered_route_style(&self, relation: &R) -> Result<LayeredRelationRouteStyle>;

    fn layered_relation_overlays(
        &self,
        relation: &R,
        geometry: &LayeredRelationRouteGeometry,
    ) -> Result<Vec<RelationOverlay>>;

    fn render_vertical(
        &self,
        boxes: &[RelationGraphBox],
        relation: &R,
        options: &AsciiRenderOptions,
    ) -> Result<String>;

    fn render_parallel(
        &self,
        boxes: &[RelationGraphBox],
        relations: &[R],
        options: &AsciiRenderOptions,
    ) -> Result<String>;

    fn build_summary_row(
        &self,
        relation: &R,
        reason: LayeredRelationSummaryReason,
    ) -> Result<RelationGraphSummaryRow>;

    fn layered_error(&self, error: LayeredRelationError) -> AsciiError;
}

impl RelationGraphLabel {
    pub(crate) fn new(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }

        let lines = split_label_lines(trimmed);
        let width = lines
            .iter()
            .map(|line| display_width(line))
            .max()
            .unwrap_or_default();

        Some(Self { lines, width })
    }

    pub(crate) fn lines(&self) -> &[String] {
        &self.lines
    }

    pub(crate) fn half_width(&self) -> usize {
        self.width / 2
    }

    pub(crate) fn line_count(&self) -> usize {
        self.lines.len()
    }
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
    #[cfg(test)]
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

    pub(crate) fn from_sections(
        id: String,
        sections: &[Vec<String>],
        padding: usize,
        style: RelationGraphBoxStyle,
    ) -> Self {
        let content_width = sectioned_box_content_width(sections, padding);
        let mut lines = Vec::new();

        lines.push(RelationGraphLine::box_border(
            style.top_left,
            style.top_right,
            style.horizontal,
            content_width,
            style.border_role,
        ));
        for (section_index, section) in sections.iter().enumerate() {
            if section_index > 0 {
                lines.push(RelationGraphLine::box_border(
                    style.separator_left,
                    style.separator_right,
                    style.horizontal,
                    content_width,
                    style.border_role,
                ));
            }
            for line in section {
                lines.push(RelationGraphLine::box_content(
                    line,
                    content_width,
                    padding,
                    style.vertical,
                    style.border_role,
                    style.text_role,
                ));
            }
        }
        lines.push(RelationGraphLine::box_border(
            style.bottom_left,
            style.bottom_right,
            style.horizontal,
            content_width,
            style.border_role,
        ));

        Self::new_with_lines(id, lines, content_width + 2)
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

fn sectioned_box_content_width(sections: &[Vec<String>], padding: usize) -> usize {
    let max_line_width = sections
        .iter()
        .flat_map(|section| section.iter())
        .map(|line| display_width(line))
        .max()
        .unwrap_or(0)
        .max(1);
    max_line_width + padding.saturating_mul(2)
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

pub(crate) fn render_stacked_boxes_with_section(
    boxes: &[RelationGraphBox],
    section_title: RelationGraphLine,
    section_lines: &[RelationGraphLine],
    options: &AsciiRenderOptions,
) -> String {
    let mut lines = Vec::new();
    for (index, relation_box) in boxes.iter().enumerate() {
        if index > 0 {
            lines.push(RelationGraphLine::plain(String::new()));
        }
        lines.extend(relation_box.lines.iter().cloned());
    }

    if !section_lines.is_empty() {
        if !lines.is_empty() {
            lines.push(RelationGraphLine::plain(String::new()));
        }
        lines.push(section_title);
        lines.extend(section_lines.iter().cloned());
    }

    if lines.is_empty() {
        return String::new();
    }

    render_lines_with_options(&lines, options)
}

pub(crate) fn render_relation_components<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    adapter: &A,
) -> Result<String>
where
    A: RelationComponentAdapter<R>,
    R: Clone,
{
    let edges = relations
        .iter()
        .map(|relation| adapter.build_edges(relation))
        .collect::<Vec<_>>();
    let layered_error = |error| adapter.layered_error(error);
    let components = relation_components(boxes, &edges).map_err(layered_error)?;
    if components.len() == 1 {
        return render_relation_component(boxes, relations, options, adapter);
    }
    if let Some(rendered) = render_combined_relation_components(
        boxes,
        relations,
        options,
        adapter,
        &components,
        &edges,
    )? {
        return Ok(rendered);
    }

    let mut rendered = Vec::new();
    for component in components {
        let component_boxes = component
            .boxes()
            .iter()
            .map(|relation_box| (*relation_box).clone())
            .collect::<Vec<_>>();
        let component_relations = component
            .edge_indices()
            .iter()
            .map(|index| relations[*index].clone())
            .collect::<Vec<_>>();
        rendered.push(render_relation_component(
            &component_boxes,
            &component_relations,
            options,
            adapter,
        )?);
    }

    Ok(rendered.join("\n"))
}

fn render_combined_relation_components<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    adapter: &A,
    components: &[RelationGraphComponent<'_>],
    edges: &[LayeredRelationEdge],
) -> Result<Option<String>>
where
    A: RelationComponentAdapter<R>,
    R: Clone,
{
    let relation_component_count = components
        .iter()
        .filter(|component| !component.edge_indices().is_empty())
        .count();
    if relation_component_count < 2 {
        return Ok(None);
    }

    let relation_ids = components
        .iter()
        .filter(|component| !component.edge_indices().is_empty())
        .flat_map(|component| {
            component
                .boxes()
                .iter()
                .map(|relation_box| relation_box.id())
        })
        .collect::<HashSet<_>>();
    let relation_boxes = boxes
        .iter()
        .filter(|relation_box| relation_ids.contains(relation_box.id()))
        .cloned()
        .collect::<Vec<_>>();
    let mut relation_indices = components
        .iter()
        .flat_map(|component| component.edge_indices().iter().copied())
        .collect::<Vec<_>>();
    relation_indices.sort_unstable();
    relation_indices.dedup();
    let component_relations = relation_indices
        .iter()
        .map(|index| relations[*index].clone())
        .collect::<Vec<_>>();

    let combined = match render_layered_relation_component_result(
        &relation_boxes,
        &component_relations,
        options,
        adapter.layered_horizontal_gap(),
        options.max_grid_cells,
        adapter,
    )? {
        Ok(rendered) => rendered,
        Err(reason) => {
            if split_summary_fallback_is_safe(components, edges) {
                return Ok(None);
            }
            render_relation_summary_rows(
                &relation_boxes,
                &component_relations,
                options,
                |relation| adapter.build_summary_row(relation, reason),
            )?
        }
    };

    let mut rendered = vec![combined];
    for component in components
        .iter()
        .filter(|component| component.edge_indices().is_empty())
    {
        let component_boxes = component
            .boxes()
            .iter()
            .map(|relation_box| (*relation_box).clone())
            .collect::<Vec<_>>();
        rendered.push(render_relation_component(
            &component_boxes,
            &[],
            options,
            adapter,
        )?);
    }

    Ok(Some(rendered.join("\n")))
}

fn split_summary_fallback_is_safe(
    components: &[RelationGraphComponent<'_>],
    edges: &[LayeredRelationEdge],
) -> bool {
    components
        .iter()
        .filter(|component| !component.edge_indices().is_empty())
        .all(|component| {
            let [edge_index] = component.edge_indices() else {
                return false;
            };
            let Some(edge) = edges.get(*edge_index) else {
                return false;
            };
            edge.source_id() != edge.target_id()
        })
}

fn render_relation_component<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    adapter: &A,
) -> Result<String>
where
    A: RelationComponentAdapter<R>,
{
    if relations.is_empty() {
        return Ok(render_stacked_boxes_with_options(boxes, options));
    }
    if relations.len() > 1
        && relations
            .iter()
            .all(|relation| adapter.is_self_relation(relation))
    {
        let edge = adapter.build_edges(&relations[0]);
        let same_endpoint = relations.iter().all(|relation| {
            let next_edge = adapter.build_edges(relation);
            next_edge.source_id() == edge.source_id() && next_edge.target_id() == edge.target_id()
        });
        if same_endpoint {
            let relation_box = find_box(boxes, edge.source_id())
                .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
            return adapter.render_self_relations(relation_box, relations, options);
        }
    }
    if relations.len() == 1 && adapter.is_self_relation(&relations[0]) {
        let edge = adapter.build_edges(&relations[0]);
        let relation_box = find_box(boxes, edge.source_id())
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        return adapter.render_self_relation(relation_box, &relations[0], options);
    }
    if adapter.is_same_endpoint_parallel(relations) {
        return adapter.render_parallel(boxes, relations, options);
    }
    if relations.len() == 1 {
        return adapter.render_vertical(boxes, &relations[0], options);
    }
    render_layered_relation_component(
        boxes,
        relations,
        options,
        adapter.layered_horizontal_gap(),
        options.max_grid_cells,
        adapter,
    )
}

pub(crate) fn render_layered_relation_component<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    max_grid_cells: usize,
    adapter: &A,
) -> Result<String>
where
    A: RelationComponentAdapter<R>,
{
    match render_layered_relation_component_result(
        boxes,
        relations,
        options,
        horizontal_gap,
        max_grid_cells,
        adapter,
    )? {
        Ok(rendered) => Ok(rendered),
        Err(reason) => render_relation_summary_rows(boxes, relations, options, |relation| {
            adapter.build_summary_row(relation, reason)
        }),
    }
}

fn render_layered_relation_component_result<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    max_grid_cells: usize,
    adapter: &A,
) -> Result<std::result::Result<String, LayeredRelationSummaryReason>>
where
    A: RelationComponentAdapter<R>,
{
    let edges = relations
        .iter()
        .map(|relation| adapter.build_edges(relation))
        .collect::<Vec<_>>();
    let scene = match plan_layered_relation_scene(boxes, edges, horizontal_gap, max_grid_cells)
        .map_err(|error| adapter.layered_error(error))?
    {
        LayeredRelationScenePlan::Routed(scene) => scene,
        LayeredRelationScenePlan::Summary(reason) => {
            return Ok(Err(reason));
        }
    };

    let mut canvas = scene.canvas_with_boxes();
    let box_snapshot = scene.capture_box_snapshot(&canvas);
    let mut route_plans = Vec::new();
    for (edge_index, lane_offset) in scene.draw_order().iter().copied() {
        let relation = &relations[edge_index];
        let style = adapter.layered_route_style(relation)?;
        let Some(route_plan) =
            scene.plan_edge_draw(edge_index, lane_offset, style, |geometry| {
                adapter.layered_relation_overlays(relation, geometry)
            })?
        else {
            continue;
        };

        route_plans.push(route_plan);
    }

    for route_plan in &route_plans {
        route_plan.draw_route_at(&mut canvas);
    }
    if !scene.box_snapshot_matches(&canvas, &box_snapshot) {
        return Ok(Err(LayeredRelationSummaryReason::RouteCollision));
    }

    for route_plan in &route_plans {
        route_plan.draw_overlays_at(&mut canvas);
    }
    if !scene.box_snapshot_matches(&canvas, &box_snapshot) {
        return Ok(Err(LayeredRelationSummaryReason::OverlayCollision));
    }

    Ok(Ok(canvas.finish_trimmed_with_options(options)))
}

pub(crate) fn render_parallel_self_loops_with_options(
    relation_box: &RelationGraphBox,
    loops: Vec<RelationSelfLoopRows>,
    options: &AsciiRenderOptions,
) -> String {
    if loops.is_empty() {
        return render_lines_with_options(&relation_box.lines, options);
    }

    let geometry = SelfLoopGeometry::for_loops(relation_box, &loops);
    let mut loop_iter = loops.into_iter();
    let mut lines = loop_iter
        .next()
        .map(|first_loop| first_self_loop_lines(relation_box, first_loop, &geometry))
        .unwrap_or_else(|| relation_box.lines.clone());
    for loop_rows in loop_iter {
        lines.extend(tail_self_loop_lines(relation_box, loop_rows, &geometry));
    }

    render_lines_with_options(&lines, options)
}

pub(crate) struct RelationSelfLoopRows {
    top_marker: RelationGraphLine,
    label_lines: Vec<RelationGraphLine>,
    bottom_marker: RelationGraphLine,
    tail_prefix: Option<RelationGraphLine>,
    horizontal: char,
    vertical: char,
}

impl RelationSelfLoopRows {
    pub(crate) fn new(
        top_marker: RelationGraphLine,
        label_lines: Vec<RelationGraphLine>,
        bottom_marker: RelationGraphLine,
        horizontal: char,
        vertical: char,
    ) -> Self {
        Self {
            top_marker,
            label_lines,
            bottom_marker,
            tail_prefix: None,
            horizontal,
            vertical,
        }
    }

    pub(crate) fn with_tail_prefix(mut self, tail_prefix: RelationGraphLine) -> Self {
        self.tail_prefix = Some(tail_prefix);
        self
    }
}

struct SelfLoopGeometry {
    bottom_start: usize,
    loop_col: usize,
}

impl SelfLoopGeometry {
    fn for_loops(relation_box: &RelationGraphBox, loops: &[RelationSelfLoopRows]) -> Self {
        let bottom_start = relation_box.width().saturating_div(2);
        let loop_col = loops.iter().enumerate().fold(
            relation_box.width().saturating_add(3),
            |loop_col, (loop_index, loop_rows)| {
                let label_width = max_self_loop_label_width(&loop_rows.label_lines);
                let label_start = self_loop_label_start(
                    relation_box,
                    label_width,
                    loop_rows.tail_prefix.as_ref().filter(|_| loop_index > 0),
                );
                let bottom_marker_width = display_width(loop_rows.bottom_marker.text());
                loop_col
                    .max(label_start.saturating_add(label_width).saturating_add(2))
                    .max(
                        bottom_start
                            .saturating_add(bottom_marker_width)
                            .saturating_add(3),
                    )
            },
        );

        Self {
            bottom_start,
            loop_col,
        }
    }
}

fn max_self_loop_label_width(label_lines: &[RelationGraphLine]) -> usize {
    label_lines
        .iter()
        .map(|line| display_width(line.text()))
        .max()
        .unwrap_or(0)
}

fn self_loop_label_start(
    relation_box: &RelationGraphBox,
    label_width: usize,
    prefix: Option<&RelationGraphLine>,
) -> usize {
    let centered_start = if label_width >= relation_box.width() {
        1
    } else {
        relation_box
            .width()
            .saturating_sub(label_width)
            .saturating_div(2)
            .saturating_add(1)
    };
    let prefix_start = prefix
        .map(|prefix| display_width(prefix.text()).saturating_add(1))
        .unwrap_or(0);
    centered_start.max(prefix_start)
}

fn first_self_loop_lines(
    relation_box: &RelationGraphBox,
    loop_rows: RelationSelfLoopRows,
    geometry: &SelfLoopGeometry,
) -> Vec<RelationGraphLine> {
    let RelationSelfLoopRows {
        top_marker,
        label_lines,
        bottom_marker,
        tail_prefix: _,
        horizontal,
        vertical,
    } = loop_rows;
    let label_start_row = relation_box.height();
    let bottom_row = label_start_row.saturating_add(label_lines.len());
    let row_count = bottom_row.saturating_add(1).max(3);
    let mut lines = Vec::new();
    lines.extend(relation_box.lines.clone());
    lines.resize_with(row_count, || {
        RelationGraphLine::plain(" ".repeat(relation_box.width()))
    });

    lines[1] = concat_relation_lines(vec![
        lines[1].clone(),
        repeated_line(
            horizontal,
            geometry.loop_col.saturating_sub(relation_box.width()),
            AsciiColorRole::EdgeLine,
        ),
        top_marker,
    ]);

    for row_index in 2..label_start_row {
        lines[row_index] = concat_relation_lines(vec![
            lines[row_index].clone(),
            RelationGraphLine::plain(
                " ".repeat(geometry.loop_col.saturating_sub(relation_box.width())),
            ),
            RelationGraphLine::with_role(vertical.to_string(), AsciiColorRole::EdgeLine),
        ]);
    }

    for (label_index, label_line) in label_lines.into_iter().enumerate() {
        let row_index = label_start_row + label_index;
        lines[row_index] = self_loop_label_line(relation_box, None, label_line, vertical, geometry);
    }

    lines[bottom_row] = self_loop_bottom_line(bottom_marker, horizontal, geometry);
    lines
}

fn tail_self_loop_lines(
    relation_box: &RelationGraphBox,
    loop_rows: RelationSelfLoopRows,
    geometry: &SelfLoopGeometry,
) -> Vec<RelationGraphLine> {
    let RelationSelfLoopRows {
        top_marker: _,
        label_lines,
        bottom_marker,
        tail_prefix,
        horizontal,
        vertical,
    } = loop_rows;
    let mut lines = label_lines
        .into_iter()
        .enumerate()
        .map(|(label_index, label_line)| {
            let prefix = if label_index == 0 {
                tail_prefix.clone()
            } else {
                None
            };
            self_loop_label_line(relation_box, prefix, label_line, vertical, geometry)
        })
        .collect::<Vec<_>>();
    lines.push(self_loop_bottom_line(bottom_marker, horizontal, geometry));
    lines
}

fn self_loop_label_line(
    relation_box: &RelationGraphBox,
    prefix: Option<RelationGraphLine>,
    label_line: RelationGraphLine,
    vertical: char,
    geometry: &SelfLoopGeometry,
) -> RelationGraphLine {
    let label_width = display_width(label_line.text());
    let prefix_width = prefix
        .as_ref()
        .map(|prefix| display_width(prefix.text()))
        .unwrap_or(0);
    let label_start = self_loop_label_start(relation_box, label_width, prefix.as_ref());
    let prefix_start = label_start.saturating_sub(prefix_width.saturating_add(1));
    let gap_after_prefix = label_start
        .saturating_sub(prefix_start)
        .saturating_sub(prefix_width);
    let right_padding = geometry
        .loop_col
        .saturating_sub(label_start.saturating_add(label_width));

    let mut segments = Vec::new();
    match prefix {
        Some(prefix) => {
            segments.push(RelationGraphLine::plain(" ".repeat(prefix_start)));
            segments.push(prefix);
            segments.push(RelationGraphLine::plain(" ".repeat(gap_after_prefix)));
        }
        None => {
            segments.push(RelationGraphLine::plain(" ".repeat(label_start)));
        }
    }
    segments.push(label_line);
    segments.push(RelationGraphLine::plain(" ".repeat(right_padding)));
    segments.push(RelationGraphLine::with_role(
        vertical.to_string(),
        AsciiColorRole::EdgeLine,
    ));

    concat_relation_lines(segments)
}

fn self_loop_bottom_line(
    bottom_marker: RelationGraphLine,
    horizontal: char,
    geometry: &SelfLoopGeometry,
) -> RelationGraphLine {
    let bottom_marker_width = display_width(bottom_marker.text());
    concat_relation_lines(vec![
        RelationGraphLine::plain(" ".repeat(geometry.bottom_start)),
        bottom_marker,
        repeated_line(
            horizontal,
            geometry
                .loop_col
                .saturating_sub(geometry.bottom_start + bottom_marker_width),
            AsciiColorRole::EdgeLine,
        ),
        RelationGraphLine::with_role("+".to_string(), AsciiColorRole::EdgeLine),
    ])
}

fn repeated_line(ch: char, count: usize, role: AsciiColorRole) -> RelationGraphLine {
    RelationGraphLine::with_role(std::iter::repeat_n(ch, count).collect(), role)
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
    use std::cell::Cell;

    struct TestRelationAdapter {
        summary_reason: Cell<Option<LayeredRelationSummaryReason>>,
        overlap: TestRelationOverlap,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TestRelationOverlap {
        None,
        Route,
        Overlay,
    }

    impl RelationComponentAdapter<(&'static str, &'static str)> for TestRelationAdapter {
        fn build_edges(&self, relation: &(&'static str, &'static str)) -> LayeredRelationEdge {
            LayeredRelationEdge::new(relation.0, relation.1, 0, 0)
        }

        fn is_same_endpoint_parallel(&self, _relations: &[(&'static str, &'static str)]) -> bool {
            false
        }

        fn is_self_relation(&self, relation: &(&'static str, &'static str)) -> bool {
            relation.0 == relation.1
        }

        fn render_self_relation(
            &self,
            _relation_box: &RelationGraphBox,
            _relation: &(&'static str, &'static str),
            _options: &AsciiRenderOptions,
        ) -> Result<String> {
            Ok(String::new())
        }

        fn render_self_relations(
            &self,
            _relation_box: &RelationGraphBox,
            _relations: &[(&'static str, &'static str)],
            _options: &AsciiRenderOptions,
        ) -> Result<String> {
            Ok(String::new())
        }

        fn layered_horizontal_gap(&self) -> usize {
            1
        }

        fn layered_route_style(
            &self,
            _relation: &(&'static str, &'static str),
        ) -> Result<LayeredRelationRouteStyle> {
            if self.overlap == TestRelationOverlap::Route {
                return Ok(LayeredRelationRouteStyle::new(
                    'X',
                    'X',
                    RelationLineChars::new(['X', 'X', 'X', 'X'], 'X'),
                    LayeredRelationRouteProfile::new(1, 0, 1, 0, 0),
                ));
            }

            Ok(LayeredRelationRouteStyle::new(
                '-',
                '-',
                RelationLineChars::new(['-', '-', '-', '-'], '+'),
                LayeredRelationRouteProfile::class(),
            ))
        }

        fn layered_relation_overlays(
            &self,
            _relation: &(&'static str, &'static str),
            _geometry: &LayeredRelationRouteGeometry,
        ) -> Result<Vec<RelationOverlay>> {
            if self.overlap == TestRelationOverlap::Overlay {
                return Ok(vec![RelationOverlay::glyph(
                    _geometry.source_x(),
                    0,
                    'X',
                    AsciiColorRole::EdgeLine,
                )]);
            }

            Ok(Vec::new())
        }

        fn render_vertical(
            &self,
            _boxes: &[RelationGraphBox],
            _relation: &(&'static str, &'static str),
            _options: &AsciiRenderOptions,
        ) -> Result<String> {
            Ok(String::new())
        }

        fn render_parallel(
            &self,
            _boxes: &[RelationGraphBox],
            _relations: &[(&'static str, &'static str)],
            _options: &AsciiRenderOptions,
        ) -> Result<String> {
            Ok(String::new())
        }

        fn build_summary_row(
            &self,
            _relation: &(&'static str, &'static str),
            reason: LayeredRelationSummaryReason,
        ) -> Result<RelationGraphSummaryRow> {
            self.summary_reason.set(Some(reason));
            Ok(RelationGraphSummaryRow::new("A", "-->", "B"))
        }

        fn layered_error(&self, error: LayeredRelationError) -> AsciiError {
            AsciiError::UnsupportedFeature {
                diagram_type: "test",
                feature: match error {
                    LayeredRelationError::MissingEndpoint => "missing endpoint",
                    LayeredRelationError::UnrelatedBoxes => "unrelated boxes",
                    LayeredRelationError::Crossing => "crossing",
                },
            }
        }
    }

    #[test]
    fn render_stacked_boxes_preserves_plain_text() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string(), "|".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string(), "|".to_string()], 1),
        ];

        assert_eq!(render_stacked_boxes(&boxes), "A\n|\n\nB\n|\n");
    }

    #[test]
    fn render_stacked_boxes_with_section_appends_summary() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        ];
        let section_lines = vec![
            RelationGraphLine::plain("A --> B".to_string()),
            RelationGraphLine::plain("B --> A".to_string()),
        ];

        assert_eq!(
            render_stacked_boxes_with_section(
                &boxes,
                RelationGraphLine::plain("relations:".to_string()),
                &section_lines,
                &AsciiRenderOptions::ascii(),
            ),
            "A\n\nB\n\nrelations:\nA --> B\nB --> A\n"
        );
    }

    #[test]
    fn render_stacked_boxes_with_section_colors_title_and_summary_lines() {
        let boxes = vec![RelationGraphBox::new_with_lines(
            "a".to_string(),
            vec![RelationGraphLine::with_role(
                "A".to_string(),
                AsciiColorRole::Text,
            )],
            1,
        )];
        let section_lines = vec![RelationGraphLine::with_role(
            "A --> B".to_string(),
            AsciiColorRole::EdgeLabel,
        )];
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x111111))
            .with_role(AsciiColorRole::MutedText, AsciiRgb::from_hex24(0x222222))
            .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::from_hex24(0x333333));

        let rendered = render_stacked_boxes_with_section(
            &boxes,
            RelationGraphLine::with_role("relations:".to_string(), AsciiColorRole::MutedText),
            &section_lines,
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::Html)
                .with_color_theme(theme),
        );

        assert_eq!(
            rendered,
            concat!(
                "<span style=\"color:#111111\">A</span>\n",
                "\n",
                "<span style=\"color:#222222\">relations:</span>\n",
                "<span style=\"color:#333333\">A --&gt; B</span>\n",
            )
        );
    }

    #[test]
    fn relation_graph_box_from_sections_builds_shared_sectioned_boxes() {
        let style = RelationGraphBoxStyle {
            top_left: '+',
            top_right: '+',
            bottom_left: '+',
            bottom_right: '+',
            horizontal: '-',
            vertical: '|',
            separator_left: '+',
            separator_right: '+',
            border_role: AsciiColorRole::NodeBorder,
            text_role: AsciiColorRole::Text,
        };
        let relation_box = RelationGraphBox::from_sections(
            "box".to_string(),
            &[vec!["A".to_string()], vec!["B".to_string()]],
            1,
            style,
        );
        let mut canvas = Canvas::new(relation_box.width(), relation_box.height());

        relation_box.draw_at(&mut canvas, 0, 0);

        assert_eq!(relation_box.width(), 5);
        assert_eq!(relation_box.height(), 5);
        assert_eq!(
            canvas.finish_trimmed_with_options(&AsciiRenderOptions::ascii()),
            "+---+\n| A |\n+---+\n| B |\n+---+\n"
        );
    }

    #[test]
    fn relation_components_split_disconnected_relation_subgraphs() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
            RelationGraphBox::new("c".to_string(), vec!["C".to_string()], 1),
            RelationGraphBox::new("d".to_string(), vec!["D".to_string()], 1),
            RelationGraphBox::new("isolated".to_string(), vec!["I".to_string()], 1),
        ];
        let edges = vec![
            LayeredRelationEdge::new("a", "b", 0, 0),
            LayeredRelationEdge::new("c", "d", 0, 0),
        ];

        let components = relation_components(&boxes, &edges).expect("components should split");
        let component_box_ids = components
            .iter()
            .map(|component| {
                component
                    .boxes()
                    .iter()
                    .map(|relation_box| relation_box.id())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let component_edge_indices = components
            .iter()
            .map(|component| component.edge_indices().to_vec())
            .collect::<Vec<_>>();

        assert_eq!(
            component_box_ids,
            vec![vec!["a", "b"], vec!["c", "d"], vec!["isolated"]]
        );
        assert_eq!(component_edge_indices, vec![vec![0], vec![1], vec![]]);
    }

    #[test]
    fn render_layered_relation_component_passes_grid_budget_reason_to_row_builder() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        ];
        let relations = vec![("a", "b")];
        let adapter = TestRelationAdapter {
            summary_reason: Cell::new(None),
            overlap: TestRelationOverlap::None,
        };

        let rendered = render_layered_relation_component(
            &boxes,
            &relations,
            &AsciiRenderOptions::ascii(),
            1,
            1,
            &adapter,
        )
        .expect("grid budget fallback should render");

        assert_eq!(
            adapter.summary_reason.get(),
            Some(LayeredRelationSummaryReason::GridBudget {
                actual: 5,
                limit: 1,
            })
        );
        assert!(rendered.contains("relations:\nA --> B\n"));
    }

    #[test]
    fn render_layered_relation_component_uses_summary_when_route_path_overlaps_box() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        ];
        let relations = vec![("a", "b")];
        let adapter = TestRelationAdapter {
            summary_reason: Cell::new(None),
            overlap: TestRelationOverlap::Route,
        };

        let rendered = render_layered_relation_component(
            &boxes,
            &relations,
            &AsciiRenderOptions::ascii(),
            1,
            10_000,
            &adapter,
        )
        .expect("route-overlapping layered relation should render as a summary");

        assert_eq!(
            adapter.summary_reason.get(),
            Some(LayeredRelationSummaryReason::RouteCollision)
        );
        assert!(rendered.contains("relations:\nA --> B\n"));
    }

    #[test]
    fn render_layered_relation_component_uses_summary_when_overlay_overlaps_box() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
            RelationGraphBox::new("c".to_string(), vec!["C".to_string()], 1),
        ];
        let relations = vec![("a", "b"), ("a", "c")];
        let adapter = TestRelationAdapter {
            summary_reason: Cell::new(None),
            overlap: TestRelationOverlap::Overlay,
        };

        let rendered = render_layered_relation_component(
            &boxes,
            &relations,
            &AsciiRenderOptions::ascii(),
            1,
            10_000,
            &adapter,
        )
        .expect("overlay-overlapping layered relation should render as a summary");

        assert_eq!(
            adapter.summary_reason.get(),
            Some(LayeredRelationSummaryReason::OverlayCollision)
        );
        assert!(rendered.contains("relations:\nA --> B\nA --> B\n"));
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
    fn parallel_relation_lane_offsets_group_reverse_endpoint_pairs() {
        let offsets = parallel_relation_lane_offsets([("A", "B"), ("B", "A"), ("A", "B")]);

        assert_eq!(offsets, vec![-6, 0, 6]);
    }

    #[test]
    fn relation_graph_label_splits_breaks_and_tracks_line_count() {
        let label = RelationGraphLabel::new("north<br>south").expect("label should be present");

        assert_eq!(label.lines(), ["north", "south"]);
        assert_eq!(label.half_width(), 2);
        assert_eq!(label.line_count(), 2);
    }

    #[test]
    fn write_centered_relation_label_draws_each_line() {
        let label = RelationGraphLabel::new("A<br>B").expect("label should be present");
        let mut canvas = Canvas::new(3, 3);

        write_centered_relation_label(&mut canvas, 1, 1, &label, AsciiColorRole::EdgeLabel);

        assert_eq!(canvas.get(1, 1), Some('A'));
        assert_eq!(canvas.get(1, 2), Some('B'));
        assert_eq!(
            canvas.get_color(1, 1),
            Some(crate::canvas::CanvasColor::Role(AsciiColorRole::EdgeLabel))
        );
    }

    #[test]
    fn layered_relation_gap_grows_with_label_line_count() {
        let boxes = vec![
            RelationGraphBox::new("top".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("bottom".to_string(), vec!["B".to_string()], 1),
        ];
        let no_label_edges = vec![LayeredRelationEdge::new("top", "bottom", 0, 0)];
        let one_line_edges = vec![LayeredRelationEdge::new("top", "bottom", 0, 1)];
        let two_line_edges = vec![LayeredRelationEdge::new("top", "bottom", 0, 2)];

        let no_label_plan = plan_layered_relation_boxes(&boxes, &no_label_edges, 1)
            .expect("unlabeled layered relation should plan");
        let one_line_plan = plan_layered_relation_boxes(&boxes, &one_line_edges, 1)
            .expect("single-line labeled relation should plan");
        let two_line_plan = plan_layered_relation_boxes(&boxes, &two_line_edges, 1)
            .expect("multiline labeled relation should plan");

        assert_eq!(no_label_plan.height(), 5);
        assert_eq!(one_line_plan.height(), 6);
        assert_eq!(two_line_plan.height(), 7);
    }

    #[test]
    fn layered_relation_plan_reserves_width_for_reverse_spanning_edges() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
            RelationGraphBox::new("c".to_string(), vec!["C".to_string()], 1),
        ];
        let edges = vec![
            LayeredRelationEdge::new("a", "b", 0, 0),
            LayeredRelationEdge::new("b", "c", 0, 0),
            LayeredRelationEdge::new("c", "a", 0, 0),
        ];

        let plan =
            plan_layered_relation_boxes(&boxes, &edges, 1).expect("cyclic plan should render");

        assert_eq!(plan.width(), 7);
    }

    #[test]
    fn layered_relation_plan_reserves_width_for_reverse_parallel_lanes() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        ];
        let edges = vec![
            LayeredRelationEdge::new("a", "b", 0, 0),
            LayeredRelationEdge::new("b", "a", 0, 0),
        ];

        let plan = plan_layered_relation_boxes(&boxes, &edges, 1)
            .expect("bidirectional plan should render");

        assert_eq!(plan.width(), 7);
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
        let geometry = plan_layered_relation_route(LayeredRelationRouteRequest::new(
            &placed,
            &placed[0],
            &placed[1],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0, 0),
        ));
        let route = LayeredRelationRoutePlan::new(
            geometry.clone(),
            '|',
            '-',
            RelationLineChars::new(['-', '|', '.', ':'], '+'),
            vec![
                RelationOverlay::text(
                    geometry.source_x(),
                    geometry.source_marker_y(),
                    "T".to_string(),
                    AsciiColorRole::EdgeArrow,
                ),
                RelationOverlay::text(
                    (geometry.source_x() + geometry.target_x()) / 2,
                    geometry.route_y().saturating_sub(1),
                    "L".to_string(),
                    AsciiColorRole::EdgeLabel,
                ),
                RelationOverlay::text(
                    geometry.target_x(),
                    geometry.target_marker_y(),
                    "B".to_string(),
                    AsciiColorRole::EdgeArrow,
                ),
            ],
        );
        let mut canvas = Canvas::new(3, 5);

        route.draw_route_at(&mut canvas);
        route.draw_overlays_at(&mut canvas);

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

    #[test]
    fn layered_relation_route_label_y_follows_source_to_target_direction() {
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
                y: 10,
            },
        ];

        let downward = plan_layered_relation_route(LayeredRelationRouteRequest::new(
            &placed,
            &placed[0],
            &placed[1],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0, 0),
        ));
        let upward = plan_layered_relation_route(LayeredRelationRouteRequest::new(
            &placed,
            &placed[1],
            &placed[0],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0, 0),
        ));

        assert_eq!(downward.label_y_after_source(), 2);
        assert_eq!(upward.label_y_after_source(), 8);
    }

    #[test]
    fn layered_relation_route_profile_reserves_rows_for_multiline_endpoint_labels() {
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
                y: 10,
            },
        ];

        let geometry = plan_layered_relation_route(LayeredRelationRouteRequest::new(
            &placed,
            &placed[0],
            &placed[1],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0, 2),
        ));

        assert_eq!(geometry.source_marker_y(), 3);
        assert_eq!(geometry.label_y_after_source(), 4);
        assert_eq!(geometry.route_y(), 7);
        assert_eq!(geometry.target_marker_y(), 7);
    }

    #[test]
    fn layered_relation_route_plan_avoids_intermediate_boxes() {
        let top_box = RelationGraphBox::new("top".to_string(), vec!["AAA".to_string()], 3);
        let middle_box =
            RelationGraphBox::new("middle".to_string(), vec!["MMMMMMM".to_string()], 7);
        let bottom_box = RelationGraphBox::new("bottom".to_string(), vec!["BBB".to_string()], 3);
        let placed = vec![
            PlacedRelationGraphBox {
                id: "top",
                relation_box: &top_box,
                x: 0,
                y: 0,
            },
            PlacedRelationGraphBox {
                id: "middle",
                relation_box: &middle_box,
                x: 0,
                y: 4,
            },
            PlacedRelationGraphBox {
                id: "bottom",
                relation_box: &bottom_box,
                x: 0,
                y: 10,
            },
        ];

        let geometry = plan_layered_relation_route(LayeredRelationRouteRequest::new(
            &placed,
            &placed[0],
            &placed[2],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0, 0),
        ));

        assert_eq!(geometry.source_x(), 7);
        assert_eq!(geometry.target_x(), 7);
        assert_eq!(geometry.route_y(), 9);
    }
}

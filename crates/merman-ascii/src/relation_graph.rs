use crate::canvas::{Canvas, finish_styled_line_iter_with_resources};
use crate::color::AsciiColorRole;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
#[cfg(test)]
use crate::resource::AsciiResourceLimitId;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
#[cfg(test)]
use crate::safe_text::normalize_terminal_text;
use crate::safe_text::try_build_normalized_label_lines;
#[cfg(test)]
use crate::text::split_label_lines;
use crate::text::{StyledLine, display_width_with_profile};
use crate::{AsciiError, Result};
use std::collections::HashSet;
use std::rc::Rc;
mod layered;
mod summary;

pub(crate) use self::layered::*;
pub(crate) use self::summary::*;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RelationGraphLine {
    line: Rc<StyledLine>,
}

impl Clone for RelationGraphLine {
    fn clone(&self) -> Self {
        self.shared()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RelationGraphBox {
    id: Rc<String>,
    lines: Rc<Vec<RelationGraphLine>>,
    width: usize,
    width_profile: TerminalWidthProfile,
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
    width_profile: TerminalWidthProfile,
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
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationGraphLine>>;

    fn render_self_relations(
        &self,
        relation_box: &RelationGraphBox,
        relations: &[R],
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationGraphLine>>;

    fn layered_horizontal_gap(&self) -> usize;

    fn layered_route_style(&self, relation: &R) -> Result<LayeredRelationRouteStyle>;

    fn layered_relation_overlays(
        &self,
        relation: &R,
        geometry: &LayeredRelationRouteGeometry,
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationOverlay>>;

    fn render_vertical(
        &self,
        boxes: &[RelationGraphBox],
        relation: &R,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationGraphLine>>;

    fn render_parallel(
        &self,
        boxes: &[RelationGraphBox],
        relations: &[R],
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationGraphLine>>;

    fn build_summary_row(
        &self,
        relation: &R,
        reason: LayeredRelationSummaryReason,
    ) -> Result<RelationGraphSummaryRow>;

    fn layered_error(&self, error: LayeredRelationError) -> AsciiError;
}

impl RelationGraphLabel {
    pub(crate) fn try_new(
        raw: &str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Option<Self>> {
        let Some(normalized) =
            try_build_normalized_label_lines(raw, width_profile, true, None, resources)?
        else {
            return Ok(None);
        };
        let (lines, width) = normalized.into_parts();
        Ok(Some(Self {
            lines,
            width,
            width_profile,
        }))
    }

    #[cfg(test)]
    pub(crate) fn new(raw: &str, width_profile: TerminalWidthProfile) -> Option<Self> {
        let normalized = normalize_terminal_text(raw);
        let trimmed = normalized.trim();
        if trimmed.is_empty() {
            return None;
        }

        let lines = split_label_lines(trimmed);
        let width = lines
            .iter()
            .map(|line| display_width_with_profile(line, width_profile))
            .max()
            .unwrap_or_default();

        Some(Self {
            lines,
            width,
            width_profile,
        })
    }

    pub(crate) fn lines(&self) -> &[String] {
        &self.lines
    }

    pub(crate) fn half_width(&self) -> usize {
        self.width / 2
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub(crate) fn width_profile(&self) -> TerminalWidthProfile {
        self.width_profile
    }
}

#[derive(Debug)]
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
        build_rows: impl FnOnce(usize) -> Result<Vec<RelationGraphLine>>,
    ) -> Result<Self> {
        debug_assert_eq!(
            top.width_profile(),
            bottom.width_profile(),
            "stacked relation boxes must share one terminal width profile"
        );
        let center = vertical_center(top, bottom, extra_half_widths);
        let relation_lines = build_rows(center)?;
        debug_assert!(
            relation_lines
                .iter()
                .all(|line| line.width_profile() == top.width_profile())
        );
        Ok(Self {
            top,
            bottom,
            center,
            relation_lines,
        })
    }

    pub(crate) fn render_lines(
        self,
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationGraphLine>> {
        vertical_stack_lines(
            self.top,
            self.bottom,
            self.center,
            self.relation_lines,
            resources,
        )
    }
}

#[derive(Debug)]
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
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        debug_assert_eq!(
            top.width_profile(),
            bottom.width_profile(),
            "parallel relation boxes must share one terminal width profile"
        );
        debug_assert!(
            lanes
                .iter()
                .flatten()
                .all(|line| { line.width_profile() == top.width_profile() })
        );
        resources.charge_layout_work(lanes.len().max(1))?;
        let mut lane_widths = Vec::new();
        lane_widths
            .try_reserve_exact(lanes.len())
            .map_err(|_| layout_allocation_failed())?;
        for lane in &lanes {
            lane_widths.push(
                lane.iter()
                    .map(RelationGraphLine::width)
                    .max()
                    .unwrap_or(1)
                    .max(1),
            );
        }
        let lanes_content_width = lane_widths.iter().try_fold(0usize, |total, width| {
            resources.checked_grid_add(total, *width)
        })?;
        let gap_count = lane_widths.len().saturating_sub(1);
        let gaps_width = resources.checked_grid_mul(lane_gap, gap_count)?;
        let lanes_width = resources.checked_grid_add(lanes_content_width, gaps_width)?;
        let lane_center = lanes_width / 2;
        let center = (top.width / 2).max(bottom.width / 2).max(lane_center);
        let lane_left = center - lane_center;

        Ok(Self {
            top,
            bottom,
            center,
            lane_left,
            lane_gap,
            lane_widths,
            lanes,
        })
    }

    pub(crate) fn render_lines(
        &self,
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationGraphLine>> {
        let mut relation_lines = Vec::new();
        let row_count = self.lanes.iter().map(Vec::len).max().unwrap_or(0);
        let height = resources.checked_grid_add(
            resources.checked_grid_add(self.top.height(), row_count)?,
            self.bottom.height(),
        )?;
        let lanes_width = self.lane_widths.iter().try_fold(0usize, |total, width| {
            resources.checked_grid_add(total, *width)
        })?;
        let gaps_width =
            resources.checked_grid_mul(self.lane_gap, self.lane_widths.len().saturating_sub(1))?;
        let relation_width = resources.checked_grid_add(
            self.lane_left,
            resources.checked_grid_add(lanes_width, gaps_width)?,
        )?;
        let box_width = resources.checked_grid_add(self.center, self.center.max(1))?;
        let extent = resources.grid_extent(relation_width.max(box_width), height)?;
        resources.charge_layout_work(extent.cells())?;
        relation_lines
            .try_reserve_exact(row_count)
            .map_err(|_| layout_allocation_failed())?;
        for row_index in 0..row_count {
            let mut line = StyledLine::with_resources(self.top.width_profile(), resources);
            line.try_push_spaces(self.lane_left)?;
            for (lane_index, lane) in self.lanes.iter().enumerate() {
                if lane_index > 0 {
                    line.try_push_spaces(self.lane_gap)?;
                }
                let lane_width = self.lane_widths[lane_index];
                let Some(cell) = lane.get(row_index) else {
                    line.try_push_spaces(lane_width)?;
                    continue;
                };
                let remaining = lane_width
                    .checked_sub(cell.width())
                    .ok_or_else(|| grid_overflow(resources))?;
                let left_padding = remaining / 2;
                let right_padding = remaining
                    .checked_sub(left_padding)
                    .ok_or_else(|| grid_overflow(resources))?;
                line.try_push_spaces(left_padding)?;
                line.try_push_line(&cell.line)?;
                line.try_push_spaces(right_padding)?;
            }
            relation_lines.push(RelationGraphLine::from_styled(line));
        }

        vertical_stack_lines(
            self.top,
            self.bottom,
            self.center,
            relation_lines,
            resources,
        )
    }
}

impl RelationGraphLine {
    #[cfg(test)]
    pub(crate) fn plain(text: String, width_profile: TerminalWidthProfile) -> Self {
        let line = StyledLine::plain_text_with_profile(&text, width_profile);
        Self::from_styled(line)
    }

    pub(crate) fn try_plain(
        text: &str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let mut line = StyledLine::with_resources(width_profile, resources);
        line.try_push_plain_text(text)?;
        Ok(Self::from_styled(line))
    }

    pub(crate) fn try_blank(
        width: usize,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let line = StyledLine::try_blank_with_resources(width, width_profile, resources)?;
        Ok(Self::from_styled(line))
    }

    #[cfg(test)]
    pub(crate) fn with_role(
        text: String,
        role: AsciiColorRole,
        width_profile: TerminalWidthProfile,
    ) -> Self {
        let line = StyledLine::role_text_with_profile(&text, role, width_profile);
        Self::from_styled(line)
    }

    pub(crate) fn try_with_role(
        text: &str,
        role: AsciiColorRole,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let mut line = StyledLine::with_resources(width_profile, resources);
        line.try_push_role_text(text, role)?;
        Ok(Self::from_styled(line))
    }

    pub(crate) fn try_role_char(
        ch: char,
        role: AsciiColorRole,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let mut line = StyledLine::with_resources(width_profile, resources);
        line.try_push_role_char(ch, role)?;
        Ok(Self::from_styled(line))
    }

    pub(crate) fn try_role_repeat(
        ch: char,
        count: usize,
        role: AsciiColorRole,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let mut line = StyledLine::with_resources(width_profile, resources);
        line.try_push_role_repeat(ch, count, role)?;
        Ok(Self::from_styled(line))
    }

    pub(crate) fn try_box_border(
        left: char,
        right: char,
        horizontal: char,
        content_width: usize,
        role: AsciiColorRole,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let mut line = StyledLine::with_resources(width_profile, resources);
        line.try_push_role_char(left, role)?;
        line.try_push_role_repeat(horizontal, content_width, role)?;
        line.try_push_role_char(right, role)?;
        Ok(Self::from_styled(line))
    }

    pub(crate) fn box_content(
        text: &str,
        content_width: usize,
        padding: usize,
        style: RelationGraphBoxStyle,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let text_width = display_width_with_profile(text, width_profile);
        let used_width = resources.checked_grid_add(padding, text_width)?;
        let trailing = content_width
            .checked_sub(used_width)
            .ok_or_else(|| grid_overflow(resources))?;

        let mut line = StyledLine::with_resources(width_profile, resources);
        line.try_push_role_char(style.vertical, style.border_role)?;
        line.try_push_spaces(padding)?;
        line.try_push_role_text(text, style.text_role)?;
        line.try_push_spaces(trailing)?;
        line.try_push_role_char(style.vertical, style.border_role)?;
        Ok(Self::from_styled(line))
    }

    #[cfg(test)]
    pub(crate) fn text(&self) -> String {
        self.line.text()
    }

    pub(crate) fn draw_at(&self, canvas: &mut Canvas, x: usize, y: usize) -> Result<()> {
        self.line.try_write_to_at(canvas, x, y)
    }

    pub(crate) fn width(&self) -> usize {
        self.line.len()
    }

    pub(crate) fn width_profile(&self) -> TerminalWidthProfile {
        self.line.width_profile()
    }

    pub(crate) fn from_styled(line: StyledLine) -> Self {
        Self {
            line: Rc::new(line),
        }
    }

    fn styled(&self) -> &StyledLine {
        &self.line
    }

    fn shared(&self) -> Self {
        Self {
            line: Rc::clone(&self.line),
        }
    }
}

impl RelationGraphBox {
    #[cfg(test)]
    pub(crate) fn new(id: String, lines: Vec<String>, width: usize) -> Self {
        let width_profile = TerminalWidthProfile::Unicode;
        let lines = lines
            .into_iter()
            .map(|line| RelationGraphLine::plain(line, width_profile))
            .collect::<Vec<_>>();
        Self {
            id: Rc::new(id),
            lines: Rc::new(lines),
            width,
            width_profile,
        }
    }

    pub(crate) fn new_with_lines(
        id: String,
        lines: Vec<RelationGraphLine>,
        width: usize,
        width_profile: TerminalWidthProfile,
    ) -> Self {
        debug_assert!(
            lines
                .iter()
                .all(|line| line.width_profile() == width_profile),
            "relation graph box lines must share one terminal width profile"
        );
        Self {
            id: Rc::new(id),
            lines: Rc::new(lines),
            width,
            width_profile,
        }
    }

    pub(crate) fn from_rendered_lines(
        id: String,
        lines: Vec<RelationGraphLine>,
        width_profile: TerminalWidthProfile,
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        let width = lines.iter().map(line_char_width).max().unwrap_or(0);
        let extent = resources.grid_extent(width, lines.len())?;
        resources.charge_layout_work(extent.cells())?;
        Ok(Self::new_with_lines(id, lines, width, width_profile))
    }

    pub(crate) fn from_sections(
        id: String,
        sections: &[Vec<String>],
        padding: usize,
        style: RelationGraphBoxStyle,
        width_profile: TerminalWidthProfile,
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        let content_width =
            sectioned_box_content_width(sections, padding, width_profile, resources)?;
        let separator_count = sections.len().saturating_sub(1);
        let text_line_count = sections.iter().try_fold(0usize, |total, section| {
            resources.checked_grid_add(total, section.len())
        })?;
        let height = resources.checked_grid_add(
            resources.checked_grid_add(text_line_count, separator_count)?,
            2,
        )?;
        let width = resources.checked_grid_add(content_width, 2)?;
        let extent = resources.grid_extent(width, height)?;
        resources.charge_layout_work(extent.cells())?;
        let mut lines = Vec::new();
        lines
            .try_reserve_exact(height)
            .map_err(|_| layout_allocation_failed())?;

        lines.push(RelationGraphLine::try_box_border(
            style.top_left,
            style.top_right,
            style.horizontal,
            content_width,
            style.border_role,
            width_profile,
            resources,
        )?);
        for (section_index, section) in sections.iter().enumerate() {
            if section_index > 0 {
                lines.push(RelationGraphLine::try_box_border(
                    style.separator_left,
                    style.separator_right,
                    style.horizontal,
                    content_width,
                    style.border_role,
                    width_profile,
                    resources,
                )?);
            }
            for line in section {
                lines.push(RelationGraphLine::box_content(
                    line,
                    content_width,
                    padding,
                    style,
                    width_profile,
                    resources,
                )?);
            }
        }
        lines.push(RelationGraphLine::try_box_border(
            style.bottom_left,
            style.bottom_right,
            style.horizontal,
            content_width,
            style.border_role,
            width_profile,
            resources,
        )?);

        Ok(Self::new_with_lines(id, lines, width, width_profile))
    }

    pub(crate) fn id(&self) -> &str {
        self.id.as_str()
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn lines(&self) -> &[RelationGraphLine] {
        self.lines.as_slice()
    }

    pub(crate) fn height(&self) -> usize {
        self.lines.len()
    }

    pub(crate) fn width_profile(&self) -> TerminalWidthProfile {
        self.width_profile
    }

    fn shared_projection(&self) -> Self {
        Self {
            id: Rc::clone(&self.id),
            lines: Rc::clone(&self.lines),
            width: self.width,
            width_profile: self.width_profile,
        }
    }

    pub(crate) fn draw_at(
        &self,
        canvas: &mut Canvas,
        x: usize,
        y: usize,
        resources: &ResourceContext,
    ) -> Result<()> {
        for (row_index, line) in self.lines.iter().enumerate() {
            let row_y = y
                .checked_add(row_index)
                .ok_or_else(|| grid_overflow(resources))?;
            line.draw_at(canvas, x, row_y)?;
        }
        Ok(())
    }
}

fn sectioned_box_content_width(
    sections: &[Vec<String>],
    padding: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<usize> {
    let max_line_width = sections
        .iter()
        .flat_map(|section| section.iter())
        .map(|line| display_width_with_profile(line, width_profile))
        .max()
        .unwrap_or(0)
        .max(1);
    let total_padding = resources.checked_grid_mul(padding, 2)?;
    resources.checked_grid_add(max_line_width, total_padding)
}

#[cfg(test)]
pub(crate) fn render_stacked_boxes(boxes: &[RelationGraphBox]) -> String {
    boxes.iter().map(render_box).collect::<Vec<_>>().join("\n")
}

pub(crate) fn render_stacked_boxes_with_options(
    boxes: &[RelationGraphBox],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<String> {
    let lines = stacked_box_lines(boxes, options.terminal_width_profile, resources)?;
    render_lines_with_options(&lines, options, resources)
}

pub(crate) fn render_stacked_boxes_with_section(
    boxes: &[RelationGraphBox],
    section_title: RelationGraphLine,
    section_lines: &[RelationGraphLine],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<String> {
    let additional_lines = resources.checked_grid_add(
        usize::from(!boxes.is_empty() && !section_lines.is_empty()),
        resources.checked_grid_add(usize::from(!section_lines.is_empty()), section_lines.len())?,
    )?;
    let base_height = stacked_boxes_height(boxes, resources)?;
    let height = resources.checked_grid_add(base_height, additional_lines)?;
    let width = boxes
        .iter()
        .map(RelationGraphBox::width)
        .chain(std::iter::once(section_title.width()))
        .chain(section_lines.iter().map(RelationGraphLine::width))
        .max()
        .unwrap_or(0);
    let extent = resources.grid_extent(width, height)?;
    resources.charge_layout_work(extent.cells())?;

    let mut lines = Vec::new();
    lines
        .try_reserve_exact(height)
        .map_err(|_| layout_allocation_failed())?;
    for (index, relation_box) in boxes.iter().enumerate() {
        if index > 0 {
            lines.push(RelationGraphLine::try_plain(
                "",
                options.terminal_width_profile,
                resources,
            )?);
        }
        lines.extend(relation_box.lines.iter().map(RelationGraphLine::shared));
    }

    if !section_lines.is_empty() {
        if !lines.is_empty() {
            lines.push(RelationGraphLine::try_plain(
                "",
                options.terminal_width_profile,
                resources,
            )?);
        }
        lines.push(section_title);
        lines.extend(section_lines.iter().map(RelationGraphLine::shared));
    }

    if lines.is_empty() {
        return Ok(String::new());
    }

    render_lines_with_options(&lines, options, resources)
}

pub(crate) fn stacked_box_lines(
    boxes: &[RelationGraphBox],
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let height = stacked_boxes_height(boxes, resources)?;
    let width = boxes.iter().map(RelationGraphBox::width).max().unwrap_or(0);
    let extent = resources.grid_extent(width, height)?;
    resources.charge_layout_work(extent.cells())?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(height)
        .map_err(|_| layout_allocation_failed())?;
    for (index, relation_box) in boxes.iter().enumerate() {
        if index > 0 {
            lines.push(RelationGraphLine::try_plain("", width_profile, resources)?);
        }
        lines.extend(relation_box.lines.iter().map(RelationGraphLine::shared));
    }
    Ok(lines)
}

fn stacked_box_ref_lines(
    boxes: &[&RelationGraphBox],
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let height = boxes
        .iter()
        .try_fold(boxes.len().saturating_sub(1), |height, relation_box| {
            resources.checked_grid_add(height, relation_box.height())
        })?;
    let width = boxes
        .iter()
        .map(|relation_box| relation_box.width())
        .max()
        .unwrap_or(0);
    let extent = resources.grid_extent(width, height)?;
    resources.charge_layout_work(extent.cells())?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(height)
        .map_err(|_| layout_allocation_failed())?;
    for (index, relation_box) in boxes.iter().enumerate() {
        if index > 0 {
            lines.push(RelationGraphLine::try_plain("", width_profile, resources)?);
        }
        lines.extend(relation_box.lines.iter().map(RelationGraphLine::shared));
    }
    Ok(lines)
}

fn join_component_line_groups(
    groups: Vec<Vec<RelationGraphLine>>,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let separators = groups.len().saturating_sub(1);
    let height = groups.iter().try_fold(separators, |total, group| {
        resources.checked_grid_add(total, group.len())
    })?;
    let width = groups
        .iter()
        .flatten()
        .map(RelationGraphLine::width)
        .max()
        .unwrap_or(0);
    let extent = resources.grid_extent(width, height)?;
    resources.charge_layout_work(extent.cells())?;
    let mut joined = Vec::new();
    joined
        .try_reserve_exact(height)
        .map_err(|_| layout_allocation_failed())?;
    for group in groups {
        if !joined.is_empty() {
            joined.push(RelationGraphLine::try_plain("", width_profile, resources)?);
        }
        joined.extend(group);
    }
    Ok(joined)
}

fn stacked_boxes_height(boxes: &[RelationGraphBox], resources: &ResourceContext) -> Result<usize> {
    boxes
        .iter()
        .try_fold(boxes.len().saturating_sub(1), |height, relation_box| {
            resources.checked_grid_add(height, relation_box.height())
        })
}

fn build_layered_edges<R, A>(
    relations: &[R],
    adapter: &A,
    resources: &mut ResourceContext,
) -> Result<Vec<LayeredRelationEdge>>
where
    A: RelationComponentAdapter<R>,
{
    resources.charge_layout_work(relations.len().max(1))?;
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(relations.len())
        .map_err(|_| layout_allocation_failed())?;
    edges.extend(
        relations
            .iter()
            .map(|relation| adapter.build_edges(relation)),
    );
    Ok(edges)
}

pub(crate) fn render_relation_components<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<String>
where
    A: RelationComponentAdapter<R>,
{
    match render_relation_component_lines(boxes, relations, options, resources, adapter)? {
        Some(lines) => render_lines_with_options(&lines, options, resources),
        None => Ok(String::new()),
    }
}

pub(crate) fn render_relation_component_lines<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<Option<Vec<RelationGraphLine>>>
where
    A: RelationComponentAdapter<R>,
{
    let edges = build_layered_edges(relations, adapter, resources)?;
    let layered_error = |error| adapter.layered_error(error);
    let components = relation_components(boxes, &edges, resources)
        .map_err(|error| error.into_ascii_error(layered_error))?;
    if components.len() == 1 {
        return render_relation_component(boxes, relations, options, resources, adapter).map(Some);
    }
    if let Some(rendered) = render_combined_relation_components(
        boxes,
        relations,
        options,
        resources,
        adapter,
        &components,
        &edges,
    )? {
        return Ok(Some(rendered));
    }

    let mut rendered = Vec::new();
    resources.charge_layout_work(components.len().max(1))?;
    rendered
        .try_reserve_exact(components.len())
        .map_err(|_| layout_allocation_failed())?;
    for component in &components {
        rendered.push(render_relation_component_from_plan(
            boxes, relations, component, options, resources, adapter,
        )?);
    }

    Ok(Some(join_component_line_groups(
        rendered,
        options.terminal_width_profile,
        resources,
    )?))
}

fn render_relation_component_from_plan<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    component: &RelationGraphComponent<'_>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<Vec<RelationGraphLine>>
where
    A: RelationComponentAdapter<R>,
{
    let relation_indices = component.edge_indices();
    let selected_relations = match relation_indices {
        [] => &relations[0..0],
        [index] => std::slice::from_ref(
            relations
                .get(*index)
                .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?,
        ),
        indices if indices.iter().copied().eq(0..relations.len()) => relations,
        _ => {
            return Err(adapter.layered_error(LayeredRelationError::UnrelatedBoxes));
        }
    };

    render_relation_component_with_box_refs(
        boxes,
        component.boxes(),
        selected_relations,
        options,
        resources,
        adapter,
    )
}

fn render_relation_component_with_box_refs<R, A>(
    all_boxes: &[RelationGraphBox],
    boxes: &[&RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<Vec<RelationGraphLine>>
where
    A: RelationComponentAdapter<R>,
{
    if relations.is_empty() {
        return stacked_box_ref_lines(boxes, options.terminal_width_profile, resources);
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
            let relation_box = find_box_ref(boxes, edge.source_id())
                .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
            return adapter.render_self_relations(relation_box, relations, options, resources);
        }
    }
    if relations.len() == 1 && adapter.is_self_relation(&relations[0]) {
        let edge = adapter.build_edges(&relations[0]);
        let relation_box = find_box_ref(boxes, edge.source_id())
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        return adapter.render_self_relation(relation_box, &relations[0], options, resources);
    }
    if adapter.is_same_endpoint_parallel(relations) {
        return adapter.render_parallel(all_boxes, relations, options, resources);
    }
    if relations.len() == 1 {
        return adapter.render_vertical(all_boxes, &relations[0], options, resources);
    }
    render_layered_relation_component_ref_lines(
        boxes,
        relations,
        options,
        adapter.layered_horizontal_gap(),
        resources,
        adapter,
    )
}

fn render_combined_relation_components<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &A,
    components: &[RelationGraphComponent<'_>],
    edges: &[LayeredRelationEdge],
) -> Result<Option<Vec<RelationGraphLine>>>
where
    A: RelationComponentAdapter<R>,
{
    let relation_component_count = components
        .iter()
        .filter(|component| !component.edge_indices().is_empty())
        .count();
    if relation_component_count < 2 {
        return Ok(None);
    }

    let relation_box_count = components
        .iter()
        .filter(|component| !component.edge_indices().is_empty())
        .try_fold(0usize, |total, component| {
            total
                .checked_add(component.boxes().len())
                .ok_or_else(|| work_overflow(resources))
        })?;
    resources.charge_layout_work(relation_box_count.max(1))?;
    let mut relation_ids = HashSet::new();
    relation_ids
        .try_reserve(relation_box_count)
        .map_err(|_| layout_allocation_failed())?;
    for relation_box in components
        .iter()
        .filter(|component| !component.edge_indices().is_empty())
        .flat_map(RelationGraphComponent::boxes)
    {
        relation_ids.insert(relation_box.id());
    }
    resources.charge_layout_work(relation_ids.len().max(1))?;
    let mut relation_boxes = Vec::new();
    relation_boxes
        .try_reserve_exact(relation_ids.len())
        .map_err(|_| layout_allocation_failed())?;
    relation_boxes.extend(
        boxes
            .iter()
            .filter(|relation_box| relation_ids.contains(relation_box.id())),
    );

    let combined = match render_layered_relation_component_ref_result(
        &relation_boxes,
        relations,
        options,
        adapter.layered_horizontal_gap(),
        resources,
        adapter,
    )? {
        Ok(rendered) => rendered,
        Err(reason) => {
            if split_summary_fallback_is_safe(components, edges) {
                return Ok(None);
            }
            let mut lines =
                stacked_box_ref_lines(&relation_boxes, options.terminal_width_profile, resources)?;
            let summary_lines = relation_summary_rows_lines(
                relations,
                options,
                Some(reason),
                resources,
                |relation| adapter.build_summary_row(relation, reason),
            )?;
            if !summary_lines.is_empty() {
                if !lines.is_empty() {
                    lines.push(RelationGraphLine::try_plain(
                        "",
                        options.terminal_width_profile,
                        resources,
                    )?);
                }
                lines.push(RelationGraphLine::try_with_role(
                    "relations:",
                    AsciiColorRole::MutedText,
                    options.terminal_width_profile,
                    resources,
                )?);
                lines.extend(summary_lines);
            }
            lines
        }
    };

    let mut rendered = Vec::new();
    resources.charge_layout_work(components.len().max(1))?;
    rendered
        .try_reserve_exact(components.len())
        .map_err(|_| layout_allocation_failed())?;
    rendered.push(combined);
    for component in components
        .iter()
        .filter(|component| component.edge_indices().is_empty())
    {
        rendered.push(stacked_box_ref_lines(
            component.boxes(),
            options.terminal_width_profile,
            resources,
        )?);
    }

    Ok(Some(join_component_line_groups(
        rendered,
        options.terminal_width_profile,
        resources,
    )?))
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
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<Vec<RelationGraphLine>>
where
    A: RelationComponentAdapter<R>,
{
    if relations.is_empty() {
        return stacked_box_lines(boxes, options.terminal_width_profile, resources);
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
            return adapter.render_self_relations(relation_box, relations, options, resources);
        }
    }
    if relations.len() == 1 && adapter.is_self_relation(&relations[0]) {
        let edge = adapter.build_edges(&relations[0]);
        let relation_box = find_box(boxes, edge.source_id())
            .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
        return adapter.render_self_relation(relation_box, &relations[0], options, resources);
    }
    if adapter.is_same_endpoint_parallel(relations) {
        return adapter.render_parallel(boxes, relations, options, resources);
    }
    if relations.len() == 1 {
        return adapter.render_vertical(boxes, &relations[0], options, resources);
    }
    render_layered_relation_component_lines(
        boxes,
        relations,
        options,
        adapter.layered_horizontal_gap(),
        resources,
        adapter,
    )
}

#[cfg(test)]
pub(crate) fn render_layered_relation_component<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    adapter: &A,
) -> Result<String>
where
    A: RelationComponentAdapter<R>,
{
    let mut resources = ResourceContext::new(options.resources);
    let lines = render_layered_relation_component_lines(
        boxes,
        relations,
        options,
        horizontal_gap,
        &mut resources,
        adapter,
    )?;
    render_lines_with_options(&lines, options, &mut resources)
}

pub(crate) fn render_layered_relation_component_lines<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<Vec<RelationGraphLine>>
where
    A: RelationComponentAdapter<R>,
{
    match render_layered_relation_component_result(
        boxes,
        relations,
        options,
        horizontal_gap,
        resources,
        adapter,
    )? {
        Ok(rendered) => Ok(rendered),
        Err(reason) => {
            let mut lines = stacked_box_lines(boxes, options.terminal_width_profile, resources)?;
            let summary_lines = relation_summary_rows_lines(
                relations,
                options,
                Some(reason),
                resources,
                |relation| adapter.build_summary_row(relation, reason),
            )?;
            if !summary_lines.is_empty() {
                if !lines.is_empty() {
                    lines.push(RelationGraphLine::try_plain(
                        "",
                        options.terminal_width_profile,
                        resources,
                    )?);
                }
                lines.push(RelationGraphLine::try_with_role(
                    "relations:",
                    AsciiColorRole::MutedText,
                    options.terminal_width_profile,
                    resources,
                )?);
                lines.extend(summary_lines);
            }
            Ok(lines)
        }
    }
}

fn render_layered_relation_component_ref_lines<R, A>(
    boxes: &[&RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<Vec<RelationGraphLine>>
where
    A: RelationComponentAdapter<R>,
{
    match render_layered_relation_component_ref_result(
        boxes,
        relations,
        options,
        horizontal_gap,
        resources,
        adapter,
    )? {
        Ok(rendered) => Ok(rendered),
        Err(reason) => {
            let mut lines =
                stacked_box_ref_lines(boxes, options.terminal_width_profile, resources)?;
            let summary_lines = relation_summary_rows_lines(
                relations,
                options,
                Some(reason),
                resources,
                |relation| adapter.build_summary_row(relation, reason),
            )?;
            if !summary_lines.is_empty() {
                if !lines.is_empty() {
                    lines.push(RelationGraphLine::try_plain(
                        "",
                        options.terminal_width_profile,
                        resources,
                    )?);
                }
                lines.push(RelationGraphLine::try_with_role(
                    "relations:",
                    AsciiColorRole::MutedText,
                    options.terminal_width_profile,
                    resources,
                )?);
                lines.extend(summary_lines);
            }
            Ok(lines)
        }
    }
}

fn render_layered_relation_component_ref_result<R, A>(
    boxes: &[&RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<std::result::Result<Vec<RelationGraphLine>, LayeredRelationSummaryReason>>
where
    A: RelationComponentAdapter<R>,
{
    resources.charge_layout_work(boxes.len().max(1))?;
    let mut projected_boxes = Vec::new();
    projected_boxes
        .try_reserve_exact(boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    projected_boxes.extend(
        boxes
            .iter()
            .map(|relation_box| relation_box.shared_projection()),
    );
    render_layered_relation_component_result(
        &projected_boxes,
        relations,
        options,
        horizontal_gap,
        resources,
        adapter,
    )
}

fn render_layered_relation_component_result<R, A>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<std::result::Result<Vec<RelationGraphLine>, LayeredRelationSummaryReason>>
where
    A: RelationComponentAdapter<R>,
{
    let edges = build_layered_edges(relations, adapter, resources)?;
    let scene = match plan_layered_relation_scene(
        boxes,
        edges,
        horizontal_gap,
        options.terminal_width_profile,
        resources,
    )
    .map_err(|error| error.into_ascii_error(|semantic| adapter.layered_error(semantic)))?
    {
        LayeredRelationScenePlan::Routed(scene) => scene,
        LayeredRelationScenePlan::Summary(reason) => {
            return Ok(Err(reason));
        }
    };

    let mut route_plans = Vec::new();
    resources.charge_layout_work(scene.draw_order().len().max(1))?;
    route_plans
        .try_reserve_exact(scene.draw_order().len())
        .map_err(|_| layout_allocation_failed())?;
    for (edge_index, lane_offset) in scene.draw_order().iter().copied() {
        let relation = &relations[edge_index];
        let style = adapter.layered_route_style(relation)?;
        let Some(route_plan) = scene.plan_edge_draw(
            edge_index,
            lane_offset,
            style,
            resources,
            |geometry, resources| adapter.layered_relation_overlays(relation, geometry, resources),
        )?
        else {
            continue;
        };

        route_plans.push(route_plan);
    }

    if route_plans
        .iter()
        .any(|route_plan| !route_plan.route_fits(scene.width(), scene.height()))
    {
        return Ok(Err(LayeredRelationSummaryReason::RouteCollision));
    }
    if route_plans
        .iter()
        .any(|route_plan| !route_plan.overlays_fit(scene.width(), scene.height()))
    {
        return Ok(Err(LayeredRelationSummaryReason::OverlayCollision));
    }

    let mut canvas = scene.canvas_with_boxes(options, resources)?;
    let box_snapshot = scene.capture_box_snapshot(&canvas, resources)?;

    for route_plan in &route_plans {
        route_plan.draw_route_at(&mut canvas)?;
    }
    if !scene.box_snapshot_matches(&canvas, &box_snapshot, resources)? {
        return Ok(Err(LayeredRelationSummaryReason::RouteCollision));
    }

    for route_plan in &route_plans {
        route_plan.draw_overlays_at(&mut canvas)?;
    }
    if !scene.box_snapshot_matches(&canvas, &box_snapshot, resources)? {
        return Ok(Err(LayeredRelationSummaryReason::OverlayCollision));
    }

    drop(box_snapshot);
    let styled_lines = canvas.into_styled_lines_trimmed()?;
    let mut rendered = Vec::new();
    resources.charge_layout_work(styled_lines.len().max(1))?;
    rendered
        .try_reserve_exact(styled_lines.len())
        .map_err(|_| layout_allocation_failed())?;
    rendered.extend(styled_lines.into_iter().map(RelationGraphLine::from_styled));
    Ok(Ok(rendered))
}

pub(crate) fn render_parallel_self_loops(
    relation_box: &RelationGraphBox,
    loops: Vec<RelationSelfLoopRows>,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    if loops.is_empty() {
        let extent = resources.grid_extent(relation_box.width(), relation_box.height())?;
        resources.charge_layout_work(extent.cells())?;
        return try_share_relation_box_lines(relation_box);
    }

    let geometry = SelfLoopGeometry::for_loops(relation_box, &loops, resources)?;
    let height = loops
        .iter()
        .enumerate()
        .try_fold(0usize, |height, (index, rows)| {
            let loop_height = if index == 0 {
                resources.checked_grid_add(
                    relation_box.height(),
                    resources.checked_grid_add(rows.label_lines.len(), 1)?,
                )?
            } else {
                resources.checked_grid_add(rows.label_lines.len(), 1)?
            };
            resources.checked_grid_add(height, loop_height)
        })?;
    let max_top_marker_width = loops
        .iter()
        .map(|rows| rows.top_marker.width())
        .max()
        .unwrap_or(1)
        .max(1);
    let width = resources.checked_grid_add(geometry.loop_col, max_top_marker_width)?;
    let extent = resources.grid_extent(width, height)?;
    resources.charge_layout_work(extent.cells())?;
    let mut loop_iter = loops.into_iter();
    let Some(first_loop) = loop_iter.next() else {
        return try_share_relation_box_lines(relation_box);
    };
    let mut lines = first_self_loop_lines(relation_box, first_loop, &geometry, resources)?;
    for loop_rows in loop_iter {
        lines.extend(tail_self_loop_lines(
            relation_box,
            loop_rows,
            &geometry,
            resources,
        )?);
    }

    Ok(lines)
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
    fn for_loops(
        relation_box: &RelationGraphBox,
        loops: &[RelationSelfLoopRows],
        resources: &ResourceContext,
    ) -> Result<Self> {
        let bottom_start = relation_box.width() / 2;
        let mut loop_col = resources.checked_grid_add(relation_box.width(), 3)?;
        for (loop_index, loop_rows) in loops.iter().enumerate() {
            let label_width = max_self_loop_label_width(&loop_rows.label_lines);
            let label_start = self_loop_label_start(
                relation_box,
                label_width,
                loop_rows.tail_prefix.as_ref().filter(|_| loop_index > 0),
                resources,
            )?;
            let label_end = resources
                .checked_grid_add(resources.checked_grid_add(label_start, label_width)?, 2)?;
            let marker_end = resources.checked_grid_add(
                resources.checked_grid_add(bottom_start, loop_rows.bottom_marker.width())?,
                3,
            )?;
            loop_col = loop_col.max(label_end).max(marker_end);
        }

        Ok(Self {
            bottom_start,
            loop_col,
        })
    }
}

fn max_self_loop_label_width(label_lines: &[RelationGraphLine]) -> usize {
    label_lines
        .iter()
        .map(RelationGraphLine::width)
        .max()
        .unwrap_or(0)
}

fn self_loop_label_start(
    relation_box: &RelationGraphBox,
    label_width: usize,
    prefix: Option<&RelationGraphLine>,
    resources: &ResourceContext,
) -> Result<usize> {
    let centered_start = if label_width >= relation_box.width() {
        1
    } else {
        resources.checked_grid_add((relation_box.width() - label_width) / 2, 1)?
    };
    let prefix_start = match prefix {
        Some(prefix) => resources.checked_grid_add(prefix.width(), 1)?,
        None => 0,
    };
    Ok(centered_start.max(prefix_start))
}

fn first_self_loop_lines(
    relation_box: &RelationGraphBox,
    loop_rows: RelationSelfLoopRows,
    geometry: &SelfLoopGeometry,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let RelationSelfLoopRows {
        top_marker,
        label_lines,
        bottom_marker,
        tail_prefix: _,
        horizontal,
        vertical,
    } = loop_rows;
    let label_start_row = relation_box.height();
    let bottom_row = resources.checked_grid_add(label_start_row, label_lines.len())?;
    let row_count = resources.checked_grid_add(bottom_row, 1)?.max(3);
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(row_count)
        .map_err(|_| layout_allocation_failed())?;
    lines.extend(relation_box.lines.iter().map(RelationGraphLine::shared));
    while lines.len() < row_count {
        lines.push(RelationGraphLine::try_blank(
            relation_box.width(),
            relation_box.width_profile(),
            resources,
        )?);
    }

    lines[1] = try_concat_relation_lines(
        vec![
            lines[1].shared(),
            repeated_line(
                horizontal,
                geometry
                    .loop_col
                    .checked_sub(relation_box.width())
                    .ok_or_else(|| grid_overflow(resources))?,
                AsciiColorRole::EdgeLine,
                relation_box.width_profile(),
                resources,
            )?,
            top_marker,
        ],
        relation_box.width_profile(),
        resources,
    )?;

    for line in lines.iter_mut().take(label_start_row).skip(2) {
        *line = try_concat_relation_lines(
            vec![
                line.shared(),
                RelationGraphLine::try_blank(
                    geometry
                        .loop_col
                        .checked_sub(relation_box.width())
                        .ok_or_else(|| grid_overflow(resources))?,
                    relation_box.width_profile(),
                    resources,
                )?,
                RelationGraphLine::try_role_char(
                    vertical,
                    AsciiColorRole::EdgeLine,
                    relation_box.width_profile(),
                    resources,
                )?,
            ],
            relation_box.width_profile(),
            resources,
        )?;
    }

    for (label_index, label_line) in label_lines.into_iter().enumerate() {
        let row_index = resources.checked_grid_add(label_start_row, label_index)?;
        lines[row_index] = self_loop_label_line(
            relation_box,
            None,
            label_line,
            vertical,
            geometry,
            resources,
        )?;
    }

    lines[bottom_row] = self_loop_bottom_line(bottom_marker, horizontal, geometry, resources)?;
    Ok(lines)
}

fn tail_self_loop_lines(
    relation_box: &RelationGraphBox,
    loop_rows: RelationSelfLoopRows,
    geometry: &SelfLoopGeometry,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let RelationSelfLoopRows {
        top_marker: _,
        label_lines,
        bottom_marker,
        tail_prefix,
        horizontal,
        vertical,
    } = loop_rows;
    let mut lines = Vec::new();
    let capacity = label_lines
        .len()
        .checked_add(1)
        .ok_or_else(|| work_overflow(resources))?;
    lines
        .try_reserve_exact(capacity)
        .map_err(|_| layout_allocation_failed())?;
    for (label_index, label_line) in label_lines.into_iter().enumerate() {
        let prefix = if label_index == 0 {
            tail_prefix.as_ref().map(RelationGraphLine::shared)
        } else {
            None
        };
        lines.push(self_loop_label_line(
            relation_box,
            prefix,
            label_line,
            vertical,
            geometry,
            resources,
        )?);
    }
    lines.push(self_loop_bottom_line(
        bottom_marker,
        horizontal,
        geometry,
        resources,
    )?);
    Ok(lines)
}

fn self_loop_label_line(
    relation_box: &RelationGraphBox,
    prefix: Option<RelationGraphLine>,
    label_line: RelationGraphLine,
    vertical: char,
    geometry: &SelfLoopGeometry,
    resources: &ResourceContext,
) -> Result<RelationGraphLine> {
    let label_width = label_line.width();
    let prefix_width = prefix.as_ref().map(RelationGraphLine::width).unwrap_or(0);
    let label_start = if label_width >= relation_box.width() {
        1.max(resources.checked_grid_add(prefix_width, usize::from(prefix.is_some()))?)
    } else {
        resources
            .checked_grid_add((relation_box.width() - label_width) / 2, 1)?
            .max(resources.checked_grid_add(prefix_width, usize::from(prefix.is_some()))?)
    };
    let prefix_start = label_start
        .checked_sub(prefix_width)
        .and_then(|value| value.checked_sub(usize::from(prefix.is_some())))
        .ok_or_else(|| grid_overflow(resources))?;
    let gap_after_prefix = label_start
        .checked_sub(prefix_start)
        .and_then(|value| value.checked_sub(prefix_width))
        .ok_or_else(|| grid_overflow(resources))?;
    let right_padding = geometry
        .loop_col
        .checked_sub(label_start)
        .and_then(|value| value.checked_sub(label_width))
        .ok_or_else(|| grid_overflow(resources))?;

    let mut segments = Vec::new();
    match prefix {
        Some(prefix) => {
            segments.push(RelationGraphLine::try_blank(
                prefix_start,
                relation_box.width_profile(),
                resources,
            )?);
            segments.push(prefix);
            segments.push(RelationGraphLine::try_blank(
                gap_after_prefix,
                relation_box.width_profile(),
                resources,
            )?);
        }
        None => {
            segments.push(RelationGraphLine::try_blank(
                label_start,
                relation_box.width_profile(),
                resources,
            )?);
        }
    }
    segments.push(label_line);
    segments.push(RelationGraphLine::try_blank(
        right_padding,
        relation_box.width_profile(),
        resources,
    )?);
    segments.push(RelationGraphLine::try_role_char(
        vertical,
        AsciiColorRole::EdgeLine,
        relation_box.width_profile(),
        resources,
    )?);

    try_concat_relation_lines(segments, relation_box.width_profile(), resources)
}

fn self_loop_bottom_line(
    bottom_marker: RelationGraphLine,
    horizontal: char,
    geometry: &SelfLoopGeometry,
    resources: &ResourceContext,
) -> Result<RelationGraphLine> {
    let width_profile = bottom_marker.width_profile();
    let bottom_marker_width = bottom_marker.width();
    try_concat_relation_lines(
        vec![
            RelationGraphLine::try_blank(geometry.bottom_start, width_profile, resources)?,
            bottom_marker,
            repeated_line(
                horizontal,
                geometry
                    .loop_col
                    .checked_sub(geometry.bottom_start)
                    .and_then(|value| value.checked_sub(bottom_marker_width))
                    .ok_or_else(|| grid_overflow(resources))?,
                AsciiColorRole::EdgeLine,
                width_profile,
                resources,
            )?,
            RelationGraphLine::try_with_role(
                "+",
                AsciiColorRole::EdgeLine,
                width_profile,
                resources,
            )?,
        ],
        width_profile,
        resources,
    )
}

fn repeated_line(
    ch: char,
    count: usize,
    role: AsciiColorRole,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<RelationGraphLine> {
    RelationGraphLine::try_role_repeat(ch, count, role, width_profile, resources)
}

fn grid_overflow(resources: &ResourceContext) -> AsciiError {
    resources.grid_overflow()
}

fn work_overflow(resources: &ResourceContext) -> AsciiError {
    resources.work_overflow()
}

fn layout_allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

fn try_share_relation_box_lines(relation_box: &RelationGraphBox) -> Result<Vec<RelationGraphLine>> {
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(relation_box.height())
        .map_err(|_| layout_allocation_failed())?;
    lines.extend(relation_box.lines.iter().map(RelationGraphLine::shared));
    Ok(lines)
}

pub(crate) fn find_box<'a>(
    boxes: &'a [RelationGraphBox],
    id: &str,
) -> Option<&'a RelationGraphBox> {
    boxes.iter().find(|relation_box| relation_box.id() == id)
}

fn find_box_ref<'a>(boxes: &[&'a RelationGraphBox], id: &str) -> Option<&'a RelationGraphBox> {
    boxes
        .iter()
        .copied()
        .find(|relation_box| relation_box.id() == id)
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

pub(crate) fn vertical_stack_lines(
    top: &RelationGraphBox,
    bottom: &RelationGraphBox,
    center: usize,
    relation_lines: Vec<RelationGraphLine>,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let height = resources.checked_grid_add(
        resources.checked_grid_add(top.height(), relation_lines.len())?,
        bottom.height(),
    )?;
    let width = relation_lines
        .iter()
        .map(RelationGraphLine::width)
        .chain(std::iter::once(
            resources.checked_grid_add(center - top.width() / 2, top.width())?,
        ))
        .chain(std::iter::once(resources.checked_grid_add(
            center - bottom.width() / 2,
            bottom.width(),
        )?))
        .max()
        .unwrap_or(0);
    let extent = resources.grid_extent(width, height)?;
    resources.charge_layout_work(extent.cells())?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(height)
        .map_err(|_| layout_allocation_failed())?;
    lines.extend(try_align_box_lines(top, center, resources)?);
    lines.extend(relation_lines);
    lines.extend(try_align_box_lines(bottom, center, resources)?);
    Ok(lines)
}

#[cfg(test)]
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

fn render_lines_with_options(
    lines: &[RelationGraphLine],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<String> {
    if lines.is_empty() {
        return Ok(String::new());
    }

    debug_assert!(
        lines
            .iter()
            .all(|line| line.width_profile() == options.terminal_width_profile)
    );

    finish_styled_line_iter_with_resources(
        lines.iter().map(RelationGraphLine::styled),
        options,
        true,
        resources,
    )
}

fn line_char_width(line: &RelationGraphLine) -> usize {
    line.width()
}

fn try_align_box_lines(
    relation_box: &RelationGraphBox,
    center: usize,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let left_padding = center
        .checked_sub(relation_box.width() / 2)
        .ok_or_else(|| grid_overflow(resources))?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(relation_box.height())
        .map_err(|_| layout_allocation_failed())?;
    for line in relation_box.lines() {
        lines.push(try_padded_line(line, left_padding, 0, resources)?);
    }
    Ok(lines)
}

fn try_padded_line(
    line: &RelationGraphLine,
    left: usize,
    right: usize,
    resources: &ResourceContext,
) -> Result<RelationGraphLine> {
    let mut padded = StyledLine::try_blank_with_resources(left, line.width_profile(), resources)?;
    padded.try_push_line(&line.line)?;
    padded.try_push_spaces(right)?;
    Ok(RelationGraphLine::from_styled(padded))
}

pub(crate) fn try_concat_relation_lines(
    parts: Vec<RelationGraphLine>,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<RelationGraphLine> {
    let mut line = StyledLine::with_resources(width_profile, resources);
    for part in parts {
        line.try_push_line(&part.line)?;
    }
    Ok(RelationGraphLine::from_styled(line))
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

    trait TestRelationEndpoints {
        fn source_id(&self) -> &'static str;
        fn target_id(&self) -> &'static str;
    }

    impl TestRelationEndpoints for (&'static str, &'static str) {
        fn source_id(&self) -> &'static str {
            self.0
        }

        fn target_id(&self) -> &'static str {
            self.1
        }
    }

    struct NonCloneTestRelation {
        source_id: &'static str,
        target_id: &'static str,
    }

    impl TestRelationEndpoints for NonCloneTestRelation {
        fn source_id(&self) -> &'static str {
            self.source_id
        }

        fn target_id(&self) -> &'static str {
            self.target_id
        }
    }

    impl<R> RelationComponentAdapter<R> for TestRelationAdapter
    where
        R: TestRelationEndpoints,
    {
        fn build_edges(&self, relation: &R) -> LayeredRelationEdge {
            LayeredRelationEdge::new(relation.source_id(), relation.target_id(), 0, 0)
        }

        fn is_same_endpoint_parallel(&self, _relations: &[R]) -> bool {
            false
        }

        fn is_self_relation(&self, relation: &R) -> bool {
            relation.source_id() == relation.target_id()
        }

        fn render_self_relation(
            &self,
            _relation_box: &RelationGraphBox,
            _relation: &R,
            _options: &AsciiRenderOptions,
            _resources: &mut ResourceContext,
        ) -> Result<Vec<RelationGraphLine>> {
            Ok(Vec::new())
        }

        fn render_self_relations(
            &self,
            _relation_box: &RelationGraphBox,
            _relations: &[R],
            _options: &AsciiRenderOptions,
            _resources: &mut ResourceContext,
        ) -> Result<Vec<RelationGraphLine>> {
            Ok(Vec::new())
        }

        fn layered_horizontal_gap(&self) -> usize {
            1
        }

        fn layered_route_style(&self, _relation: &R) -> Result<LayeredRelationRouteStyle> {
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
            _relation: &R,
            _geometry: &LayeredRelationRouteGeometry,
            _resources: &mut ResourceContext,
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
            boxes: &[RelationGraphBox],
            relation: &R,
            _options: &AsciiRenderOptions,
            resources: &mut ResourceContext,
        ) -> Result<Vec<RelationGraphLine>> {
            let top = find_box(boxes, relation.source_id()).ok_or_else(|| {
                <Self as RelationComponentAdapter<R>>::layered_error(
                    self,
                    LayeredRelationError::MissingEndpoint,
                )
            })?;
            let bottom = find_box(boxes, relation.target_id()).ok_or_else(|| {
                <Self as RelationComponentAdapter<R>>::layered_error(
                    self,
                    LayeredRelationError::MissingEndpoint,
                )
            })?;
            RelationStackPlan::from_centered_rows(top, bottom, &[], |_| Ok(Vec::new()))?
                .render_lines(resources)
        }

        fn render_parallel(
            &self,
            _boxes: &[RelationGraphBox],
            _relations: &[R],
            _options: &AsciiRenderOptions,
            _resources: &mut ResourceContext,
        ) -> Result<Vec<RelationGraphLine>> {
            Ok(Vec::new())
        }

        fn build_summary_row(
            &self,
            _relation: &R,
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

    fn test_resources(options: &AsciiRenderOptions) -> ResourceContext {
        ResourceContext::new(options.resources)
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
        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        ];
        let section_lines = vec![
            RelationGraphLine::plain("A --> B".to_string(), TerminalWidthProfile::Unicode),
            RelationGraphLine::plain("B --> A".to_string(), TerminalWidthProfile::Unicode),
        ];

        assert_eq!(
            render_stacked_boxes_with_section(
                &boxes,
                RelationGraphLine::plain("relations:".to_string(), TerminalWidthProfile::Unicode,),
                &section_lines,
                &options,
                &mut resources,
            )
            .expect("summary section should render"),
            "A\n\nB\n\nrelations:\nA --> B\nB --> A\n"
        );
    }

    #[test]
    fn render_stacked_boxes_with_section_colors_title_and_summary_lines() {
        let options = AsciiRenderOptions::ascii()
            .with_color_mode(AsciiColorMode::Html)
            .with_color_theme(
                AsciiColorTheme::default_light()
                    .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x111111))
                    .with_role(AsciiColorRole::MutedText, AsciiRgb::from_hex24(0x222222))
                    .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::from_hex24(0x333333)),
            );
        let mut resources = test_resources(&options);
        let boxes = vec![RelationGraphBox::new_with_lines(
            "a".to_string(),
            vec![RelationGraphLine::with_role(
                "A".to_string(),
                AsciiColorRole::Text,
                TerminalWidthProfile::Unicode,
            )],
            1,
            TerminalWidthProfile::Unicode,
        )];
        let section_lines = vec![RelationGraphLine::with_role(
            "A --> B".to_string(),
            AsciiColorRole::EdgeLabel,
            TerminalWidthProfile::Unicode,
        )];
        let rendered = render_stacked_boxes_with_section(
            &boxes,
            RelationGraphLine::with_role(
                "relations:".to_string(),
                AsciiColorRole::MutedText,
                TerminalWidthProfile::Unicode,
            ),
            &section_lines,
            &options,
            &mut resources,
        )
        .expect("colored summary section should render");

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
        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
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
            TerminalWidthProfile::Unicode,
            &mut resources,
        )
        .expect("sectioned box should render");
        let mut canvas = Canvas::new(relation_box.width(), relation_box.height());

        relation_box
            .draw_at(&mut canvas, 0, 0, &resources)
            .expect("box should fit the canvas");

        assert_eq!(relation_box.width(), 5);
        assert_eq!(relation_box.height(), 5);
        assert_eq!(
            canvas
                .finish_trimmed_with_options(&options)
                .expect("canvas should encode"),
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

        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
        let components =
            relation_components(&boxes, &edges, &mut resources).expect("components should split");
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
    fn disconnected_component_rendering_borrows_non_clone_relations() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
            RelationGraphBox::new("c".to_string(), vec!["C".to_string()], 1),
            RelationGraphBox::new("d".to_string(), vec!["D".to_string()], 1),
            RelationGraphBox::new("isolated".to_string(), vec!["I".to_string()], 1),
        ];
        let relations = vec![
            NonCloneTestRelation {
                source_id: "a",
                target_id: "b",
            },
            NonCloneTestRelation {
                source_id: "c",
                target_id: "d",
            },
        ];
        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
        let adapter = TestRelationAdapter {
            summary_reason: Cell::new(None),
            overlap: TestRelationOverlap::None,
        };
        let projected_box = boxes[0].shared_projection();
        let shared_line = boxes[0].lines[0].clone();

        assert!(Rc::ptr_eq(&boxes[0].id, &projected_box.id));
        assert!(Rc::ptr_eq(&boxes[0].lines, &projected_box.lines));
        assert!(Rc::ptr_eq(&boxes[0].lines[0].line, &shared_line.line));

        let lines =
            render_relation_component_lines(&boxes, &relations, &options, &mut resources, &adapter)
                .expect("disconnected components should render")
                .expect("non-empty components should produce lines");
        let rendered = render_lines_with_options(&lines, &options, &mut resources)
            .expect("component lines should encode");

        for label in ["A", "B", "C", "D", "I"] {
            assert!(rendered.contains(label), "missing {label:?}: {rendered:?}");
        }
    }

    #[test]
    fn render_layered_relation_component_propagates_grid_resource_errors() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        ];
        let relations = vec![("a", "b")];
        let adapter = TestRelationAdapter {
            summary_reason: Cell::new(None),
            overlap: TestRelationOverlap::None,
        };

        let options = AsciiRenderOptions::ascii()
            .with_resource_limit(AsciiResourceLimitId::MaxGridCells, 1)
            .expect("test resource limit should be valid");
        let error = render_layered_relation_component(&boxes, &relations, &options, 1, &adapter)
            .expect_err("grid resource errors must not become summary fallback");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
        ));
        assert_eq!(adapter.summary_reason.get(), None);
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
        let line = RelationGraphLine::with_role(
            "AB".to_string(),
            AsciiColorRole::Text,
            TerminalWidthProfile::Unicode,
        );
        let relation_box = RelationGraphBox::new_with_lines(
            "box".to_string(),
            vec![line],
            2,
            TerminalWidthProfile::Unicode,
        );
        let options = AsciiRenderOptions::ascii()
            .with_color_mode(AsciiColorMode::TrueColor)
            .with_color_theme(theme);
        let resources = test_resources(&options);
        let mut canvas = Canvas::new(4, 1);
        relation_box
            .draw_at(&mut canvas, 0, 0, &resources)
            .expect("box should fit the canvas");

        let output = canvas
            .finish_trimmed_with_options(&options)
            .expect("canvas should encode");

        assert_eq!(output, "\u{1b}[38;2;1;2;3mAB\u{1b}[0m\n");
    }

    #[test]
    fn relation_graph_box_content_line_preserves_border_and_text_roles() {
        let options = AsciiRenderOptions::ascii()
            .with_color_mode(AsciiColorMode::Html)
            .with_color_theme(
                AsciiColorTheme::default_light()
                    .with_role(AsciiColorRole::NodeBorder, AsciiRgb::from_hex24(0x111111))
                    .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x222222)),
            );
        let resources = test_resources(&options);
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
        let line = RelationGraphLine::box_content(
            "A",
            3,
            1,
            style,
            TerminalWidthProfile::Unicode,
            &resources,
        )
        .expect("box content should fit");
        let mut canvas = Canvas::new(5, 1);

        line.draw_at(&mut canvas, 0, 0)
            .expect("line should fit the canvas");

        assert_eq!(line.text(), "| A |");
        assert_eq!(
            canvas
                .finish_trimmed_with_options(&options)
                .expect("canvas should encode"),
            "<span style=\"color:#111111\">|</span> <span style=\"color:#222222\">A</span> <span style=\"color:#111111\">|</span>\n"
        );
    }

    #[test]
    fn relation_line_chars_merge_crossing_relation_lines_to_junction() {
        let chars = RelationLineChars::new(['-', '|', '.', ':'], '+');
        let mut canvas = Canvas::new(1, 1);
        canvas.set_role(0, 0, '-', AsciiColorRole::EdgeLine);

        put_relation_char(&mut canvas, 0, 0, '|', chars)
            .expect("test relation character should fit");

        assert_eq!(canvas.get(0, 0), Some('+'));
        assert_eq!(
            canvas.get_color(0, 0),
            Some(crate::canvas::CanvasColor::Role(AsciiColorRole::Junction))
        );
    }

    #[test]
    fn parallel_relation_lane_offsets_group_by_endpoint_pair() {
        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
        let offsets = parallel_relation_lane_offsets(
            [("A", "B"), ("A", "B"), ("A", "C"), ("A", "B")],
            &mut resources,
        )
        .expect("lane offsets should fit");

        assert_eq!(offsets, vec![-6, 0, 0, 6]);
    }

    #[test]
    fn parallel_relation_lane_offsets_group_reverse_endpoint_pairs() {
        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
        let offsets =
            parallel_relation_lane_offsets([("A", "B"), ("B", "A"), ("A", "B")], &mut resources)
                .expect("lane offsets should fit");

        assert_eq!(offsets, vec![-6, 0, 6]);
    }

    #[test]
    fn relation_graph_label_splits_breaks_and_tracks_line_count() {
        let options = AsciiRenderOptions::ascii();
        let resources = test_resources(&options);
        let label = RelationGraphLabel::try_new(
            "north<br>south",
            TerminalWidthProfile::Unicode,
            &resources,
        )
        .expect("label should fit the selected resource policy")
        .expect("label should be present");

        assert_eq!(label.lines(), ["north", "south"]);
        assert_eq!(label.half_width(), 2);
        assert_eq!(label.line_count(), 2);
    }

    #[test]
    fn write_centered_relation_label_draws_each_line() {
        let options = AsciiRenderOptions::ascii();
        let resources = test_resources(&options);
        let label =
            RelationGraphLabel::try_new("A<br>B", TerminalWidthProfile::Unicode, &resources)
                .expect("label should fit the selected resource policy")
                .expect("label should be present");
        let mut canvas = Canvas::new(3, 3);

        write_centered_relation_label(&mut canvas, 1, 1, &label, AsciiColorRole::EdgeLabel)
            .expect("test relation label should fit");

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

        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
        let no_label_plan = plan_layered_relation_boxes(&boxes, &no_label_edges, 1, &mut resources)
            .expect("unlabeled layered relation should plan");
        let mut resources = test_resources(&options);
        let one_line_plan = plan_layered_relation_boxes(&boxes, &one_line_edges, 1, &mut resources)
            .expect("single-line labeled relation should plan");
        let mut resources = test_resources(&options);
        let two_line_plan = plan_layered_relation_boxes(&boxes, &two_line_edges, 1, &mut resources)
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

        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
        let plan = plan_layered_relation_boxes(&boxes, &edges, 1, &mut resources)
            .expect("cyclic plan should render");

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

        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
        let plan = plan_layered_relation_boxes(&boxes, &edges, 1, &mut resources)
            .expect("bidirectional plan should render");

        assert_eq!(plan.width(), 7);
    }

    #[test]
    fn layered_relation_route_plan_draws_route_and_overlays() {
        let top_box = RelationGraphBox::new("top".to_string(), vec!["AAA".to_string()], 3);
        let bottom_box = RelationGraphBox::new("bottom".to_string(), vec!["BBB".to_string()], 3);
        let placed = vec![
            PlacedRelationGraphBox::for_test("top", &top_box, 0, 0),
            PlacedRelationGraphBox::for_test("bottom", &bottom_box, 0, 4),
        ];
        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
        let geometry = plan_layered_relation_route(
            LayeredRelationRouteRequest::new(
                &placed,
                &placed[0],
                &placed[1],
                0,
                LayeredRelationRouteProfile::new(1, 1, 1, 0, 0),
            ),
            &mut resources,
        )
        .expect("route geometry should fit");
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
                    TerminalWidthProfile::Unicode,
                ),
                RelationOverlay::text(
                    (geometry.source_x() + geometry.target_x()) / 2,
                    geometry.route_y() - 1,
                    "L".to_string(),
                    AsciiColorRole::EdgeLabel,
                    TerminalWidthProfile::Unicode,
                ),
                RelationOverlay::text(
                    geometry.target_x(),
                    geometry.target_marker_y(),
                    "B".to_string(),
                    AsciiColorRole::EdgeArrow,
                    TerminalWidthProfile::Unicode,
                ),
            ],
        );
        let mut canvas = Canvas::new(3, 5);

        route
            .draw_route_at(&mut canvas)
            .expect("test route should fit");
        route
            .draw_overlays_at(&mut canvas)
            .expect("test overlays should fit");

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
            PlacedRelationGraphBox::for_test("top", &top_box, 0, 0),
            PlacedRelationGraphBox::for_test("bottom", &bottom_box, 0, 10),
        ];

        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
        let downward = plan_layered_relation_route(
            LayeredRelationRouteRequest::new(
                &placed,
                &placed[0],
                &placed[1],
                0,
                LayeredRelationRouteProfile::new(1, 1, 1, 0, 0),
            ),
            &mut resources,
        )
        .expect("downward route should fit");
        let mut resources = test_resources(&options);
        let upward = plan_layered_relation_route(
            LayeredRelationRouteRequest::new(
                &placed,
                &placed[1],
                &placed[0],
                0,
                LayeredRelationRouteProfile::new(1, 1, 1, 0, 0),
            ),
            &mut resources,
        )
        .expect("upward route should fit");

        assert_eq!(downward.label_y_after_source(), 2);
        assert_eq!(upward.label_y_after_source(), 8);
    }

    #[test]
    fn layered_relation_route_profile_reserves_rows_for_multiline_endpoint_labels() {
        let top_box = RelationGraphBox::new("top".to_string(), vec!["AAA".to_string()], 3);
        let bottom_box = RelationGraphBox::new("bottom".to_string(), vec!["BBB".to_string()], 3);
        let placed = vec![
            PlacedRelationGraphBox::for_test("top", &top_box, 0, 0),
            PlacedRelationGraphBox::for_test("bottom", &bottom_box, 0, 10),
        ];

        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
        let geometry = plan_layered_relation_route(
            LayeredRelationRouteRequest::new(
                &placed,
                &placed[0],
                &placed[1],
                0,
                LayeredRelationRouteProfile::new(1, 1, 1, 0, 2),
            ),
            &mut resources,
        )
        .expect("labeled route should fit");

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
            PlacedRelationGraphBox::for_test("top", &top_box, 0, 0),
            PlacedRelationGraphBox::for_test("middle", &middle_box, 0, 4),
            PlacedRelationGraphBox::for_test("bottom", &bottom_box, 0, 10),
        ];

        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
        let geometry = plan_layered_relation_route(
            LayeredRelationRouteRequest::new(
                &placed,
                &placed[0],
                &placed[2],
                0,
                LayeredRelationRouteProfile::new(1, 1, 1, 0, 0),
            ),
            &mut resources,
        )
        .expect("spanning route should fit");

        assert_eq!(geometry.source_x(), 7);
        assert_eq!(geometry.target_x(), 7);
        assert_eq!(geometry.route_y(), 9);
    }
}

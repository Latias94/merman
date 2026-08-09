use crate::canvas::{Canvas, finish_styled_line_iter_with_resources};
use crate::color::AsciiColorRole;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
#[cfg(test)]
use crate::resource::AsciiResourceLimitId;
use crate::resource::{AsciiResourceLimitPhase, LogicalExtent, ResourceContext};
#[cfg(test)]
use crate::safe_text::normalize_terminal_text;
use crate::safe_text::try_build_normalized_label_lines;
#[cfg(test)]
use crate::text::split_label_lines;
use crate::text::{StyledLine, display_width_with_profile};
use crate::{AsciiError, Result};
use std::rc::Rc;
mod document;
mod horizontal;
mod layered;
mod self_loop;
mod summary;

#[cfg(test)]
use self::document::relation_lines_extent;
pub(crate) use self::document::{
    LayeredRelationPaintPlan, RelationBoxStripPlan, RelationDocumentPlan, RelationRegionPlan,
    RelationRenderPlan, RelationSummaryPaintPlan,
};
pub(crate) use self::horizontal::*;
pub(crate) use self::layered::*;
pub(crate) use self::self_loop::{
    RelationSelfLoopMetrics, RelationSelfLoopPlan, RelationSelfLoopRows,
};
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

    fn is_self_relation(&self, relation: &R) -> bool;

    /// Describe a self-loop without constructing any styled terminal rows.
    ///
    /// Families own marker/cardinality semantics; the shared renderer only
    /// consumes the resulting geometry metrics for resource admission.
    fn self_loop_metrics(
        &self,
        relation: &R,
        resources: &ResourceContext,
    ) -> Result<RelationSelfLoopMetrics>;

    fn self_loop_rows(
        &self,
        relation: &R,
        resources: &ResourceContext,
    ) -> Result<RelationSelfLoopRows>;

    fn horizontal_relation_style(
        &self,
        relation: &R,
        source_side: RelationPortSide,
        target_side: RelationPortSide,
        resources: &ResourceContext,
    ) -> Result<HorizontalRelationStyle>;

    fn layered_horizontal_gap(&self) -> usize;

    fn layered_route_style(&self, relation: &R) -> Result<LayeredRelationRouteStyle>;

    fn layered_relation_overlays(
        &self,
        relation: &R,
        geometry: &LayeredRelationRouteGeometry,
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationOverlay>>;

    fn plan_vertical_region<'plan>(
        &self,
        boxes: &[&'plan RelationGraphBox],
        relation: &'plan R,
        resources: &mut ResourceContext,
    ) -> Result<RelationRegionPlan<'plan>>;

    fn plan_parallel_region<'plan>(
        &self,
        boxes: Vec<&'plan RelationGraphBox>,
        relations: Vec<&'plan R>,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
    ) -> Result<RelationRegionPlan<'plan>>;

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
    relation_extent: LogicalExtent,
    extent: LogicalExtent,
}

impl<'a> RelationStackPlan<'a> {
    pub(crate) fn try_new(
        top: &'a RelationGraphBox,
        bottom: &'a RelationGraphBox,
        extra_half_widths: &[usize],
        resources: &ResourceContext,
        measure_rows: impl FnOnce(usize, &ResourceContext) -> Result<LogicalExtent>,
    ) -> Result<Self> {
        debug_assert_eq!(
            top.width_profile(),
            bottom.width_profile(),
            "stacked relation boxes must share one terminal width profile"
        );
        let center = vertical_center(top, bottom, extra_half_widths);
        let relation_extent = measure_rows(center, resources)?;
        let extent = vertical_stack_extent(top, bottom, center, relation_extent, resources)?;
        Ok(Self {
            top,
            bottom,
            center,
            relation_extent,
            extent,
        })
    }

    pub(crate) const fn extent(&self) -> LogicalExtent {
        self.extent
    }

    pub(crate) fn render_lines(
        self,
        resources: &mut ResourceContext,
        materialize_rows: impl FnOnce(usize, &ResourceContext) -> Result<Vec<RelationGraphLine>>,
    ) -> Result<Vec<RelationGraphLine>> {
        resources.charge_layout_work(self.extent.cells())?;
        let relation_lines = materialize_rows(self.center, resources)?;
        self.validate_relation_lines(&relation_lines, resources)?;
        assemble_vertical_stack_lines(
            self.top,
            self.bottom,
            self.center,
            relation_lines,
            self.extent,
            resources,
        )
    }

    fn validate_relation_lines(
        &self,
        relation_lines: &[RelationGraphLine],
        resources: &ResourceContext,
    ) -> Result<()> {
        let width = relation_lines
            .iter()
            .map(RelationGraphLine::width)
            .max()
            .unwrap_or(0);
        if relation_lines.len() != self.relation_extent.height()
            || width != self.relation_extent.width()
            || relation_lines
                .iter()
                .any(|line| line.width_profile() != self.top.width_profile())
        {
            return Err(grid_overflow(resources));
        }
        Ok(())
    }
}

pub(crate) fn centered_row_blocks_extent(
    center: usize,
    blocks: impl IntoIterator<Item = (usize, usize)>,
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let mut width = 0usize;
    let mut height = 0usize;
    for (block_width, row_count) in blocks {
        if row_count == 0 {
            continue;
        }
        let left = center
            .checked_sub(block_width / 2)
            .ok_or_else(|| grid_overflow(resources))?;
        width = width.max(resources.checked_grid_add(left, block_width)?);
        height = resources.checked_grid_add(height, row_count)?;
    }
    resources.grid_extent(width, height)
}

/// A geometry-only parallel plan. Family-owned styled rows are materialized only
/// after port validation and aggregate grid admission succeed.
#[derive(Debug)]
pub(crate) struct RelationParallelPlan<'a> {
    top: &'a RelationGraphBox,
    bottom: &'a RelationGraphBox,
    center: usize,
    lane_left: usize,
    lane_gap: usize,
    lane_extents: Vec<LogicalExtent>,
    extent: LogicalExtent,
}

impl<'a> RelationParallelPlan<'a> {
    pub(crate) fn new(
        top: &'a RelationGraphBox,
        bottom: &'a RelationGraphBox,
        lane_extents: Vec<LogicalExtent>,
        lane_gap: usize,
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        debug_assert_eq!(
            top.width_profile(),
            bottom.width_profile(),
            "parallel relation boxes must share one terminal width profile"
        );
        resources.charge_layout_work(lane_extents.len().max(1))?;
        for lane_extent in &lane_extents {
            resources.grid_extent(lane_extent.width().max(1), lane_extent.height())?;
        }
        let lanes_content_width = lane_extents.iter().try_fold(0usize, |total, extent| {
            resources.checked_grid_add(total, extent.width().max(1))
        })?;
        let gap_count = lane_extents.len().saturating_sub(1);
        let gaps_width = resources.checked_grid_mul(lane_gap, gap_count)?;
        let lanes_width = resources.checked_grid_add(lanes_content_width, gaps_width)?;
        let lane_center = lanes_width / 2;
        let center = (top.width / 2).max(bottom.width / 2).max(lane_center);
        let lane_left = center - lane_center;
        let extent = parallel_relation_extent(
            top,
            bottom,
            center,
            lane_left,
            lane_gap,
            &lane_extents,
            resources,
        )?;

        Ok(Self {
            top,
            bottom,
            center,
            lane_left,
            lane_gap,
            lane_extents,
            extent,
        })
    }

    pub(crate) const fn extent(&self) -> LogicalExtent {
        self.extent
    }

    /// Check that every lane's stem column lands on both endpoint box faces.
    ///
    /// Lane cells are centered with the same left-padding rule used by
    /// `render_lines`; keeping this calculation here prevents family renderers
    /// from accepting a visually plausible but disconnected parallel layout.
    pub(crate) fn ports_fit(&self, resources: &ResourceContext) -> Result<bool> {
        let top_left = self
            .center
            .checked_sub(self.top.width / 2)
            .ok_or_else(|| grid_overflow(resources))?;
        let bottom_left = self
            .center
            .checked_sub(self.bottom.width / 2)
            .ok_or_else(|| grid_overflow(resources))?;
        let top_right = resources.checked_grid_add(top_left, self.top.width.saturating_sub(1))?;
        let bottom_right =
            resources.checked_grid_add(bottom_left, self.bottom.width.saturating_sub(1))?;

        let mut lane_left = self.lane_left;
        for (lane_index, lane_extent) in self.lane_extents.iter().copied().enumerate() {
            let lane_width = lane_extent.width().max(1);
            let lane_anchor =
                resources.checked_grid_add(lane_left, lane_width.saturating_sub(1) / 2)?;
            if !(top_left..=top_right).contains(&lane_anchor)
                || !(bottom_left..=bottom_right).contains(&lane_anchor)
            {
                return Ok(false);
            }
            lane_left = resources.checked_grid_add(lane_left, lane_width)?;
            if lane_index + 1 < self.lane_extents.len() {
                lane_left = resources.checked_grid_add(lane_left, self.lane_gap)?;
            }
        }
        Ok(true)
    }

    pub(crate) fn render_lines(
        &self,
        resources: &mut ResourceContext,
        materialize_lanes: impl FnOnce(&mut ResourceContext) -> Result<Vec<Vec<RelationGraphLine>>>,
    ) -> Result<Vec<RelationGraphLine>> {
        resources.charge_layout_work(self.extent.cells())?;
        let lanes = materialize_lanes(resources)?;
        if lanes.len() != self.lane_extents.len() {
            return Err(grid_overflow(resources));
        }
        for (lane, planned) in lanes.iter().zip(&self.lane_extents) {
            let actual_width = lane.iter().map(RelationGraphLine::width).max().unwrap_or(1);
            if lane.len() != planned.height()
                || actual_width != planned.width().max(1)
                || lane
                    .iter()
                    .any(|line| line.width_profile() != self.top.width_profile())
            {
                return Err(grid_overflow(resources));
            }
        }

        let mut relation_lines = Vec::new();
        let row_count = self.row_count();
        relation_lines
            .try_reserve_exact(row_count)
            .map_err(|_| layout_allocation_failed())?;
        for row_index in 0..row_count {
            let mut line = StyledLine::with_resources(self.top.width_profile(), resources);
            line.try_push_spaces(self.lane_left)?;
            for (lane_index, lane) in lanes.iter().enumerate() {
                if lane_index > 0 {
                    line.try_push_spaces(self.lane_gap)?;
                }
                let lane_width = self.lane_extents[lane_index].width().max(1);
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

        assemble_vertical_stack_lines(
            self.top,
            self.bottom,
            self.center,
            relation_lines,
            self.extent,
            resources,
        )
    }

    fn row_count(&self) -> usize {
        self.lane_extents
            .iter()
            .map(|extent| extent.height())
            .max()
            .unwrap_or(0)
    }
}

fn parallel_relation_extent(
    top: &RelationGraphBox,
    bottom: &RelationGraphBox,
    center: usize,
    lane_left: usize,
    lane_gap: usize,
    lane_extents: &[LogicalExtent],
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let row_count = lane_extents
        .iter()
        .map(|extent| extent.height())
        .max()
        .unwrap_or(0);
    let height = resources.checked_grid_add(
        resources.checked_grid_add(top.height(), row_count)?,
        bottom.height(),
    )?;
    let lanes_width = lane_extents.iter().try_fold(0usize, |total, extent| {
        resources.checked_grid_add(total, extent.width().max(1))
    })?;
    let gaps_width = resources.checked_grid_mul(lane_gap, lane_extents.len().saturating_sub(1))?;
    let relation_width = resources.checked_grid_add(
        lane_left,
        resources.checked_grid_add(lanes_width, gaps_width)?,
    )?;
    let top_left = center
        .checked_sub(top.width() / 2)
        .ok_or_else(|| grid_overflow(resources))?;
    let bottom_left = center
        .checked_sub(bottom.width() / 2)
        .ok_or_else(|| grid_overflow(resources))?;
    let top_width = resources.checked_grid_add(top_left, top.width())?;
    let bottom_width = resources.checked_grid_add(bottom_left, bottom.width())?;
    resources.grid_extent(relation_width.max(top_width).max(bottom_width), height)
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

#[cfg(test)]
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
    stacked_box_lines_ordered(boxes, width_profile, false, resources)
}

pub(crate) fn stacked_box_lines_ordered(
    boxes: &[RelationGraphBox],
    width_profile: TerminalWidthProfile,
    reverse: bool,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let extent = stacked_box_extent(boxes, resources)?;
    resources.charge_layout_work(extent.cells())?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(extent.height())
        .map_err(|_| layout_allocation_failed())?;
    let ordered = (0..boxes.len()).map(|index| {
        let ordered_index = if reverse {
            boxes.len() - index - 1
        } else {
            index
        };
        &boxes[ordered_index]
    });
    for (index, relation_box) in ordered.enumerate() {
        if index > 0 {
            lines.push(RelationGraphLine::try_plain("", width_profile, resources)?);
        }
        lines.extend(relation_box.lines.iter().map(RelationGraphLine::shared));
    }
    Ok(lines)
}

pub(crate) fn stacked_box_extent(
    boxes: &[RelationGraphBox],
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let height = stacked_boxes_height(boxes, resources)?;
    let width = boxes.iter().map(RelationGraphBox::width).max().unwrap_or(0);
    resources.grid_extent(width, height)
}

fn stacked_box_ref_lines(
    boxes: &[&RelationGraphBox],
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let extent = stacked_box_ref_extent(boxes, resources)?;
    resources.charge_layout_work(extent.cells())?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(extent.height())
        .map_err(|_| layout_allocation_failed())?;
    for (index, relation_box) in boxes.iter().enumerate() {
        if index > 0 {
            lines.push(RelationGraphLine::try_plain("", width_profile, resources)?);
        }
        lines.extend(relation_box.lines.iter().map(RelationGraphLine::shared));
    }
    Ok(lines)
}

fn stacked_box_ref_extent(
    boxes: &[&RelationGraphBox],
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
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
    resources.grid_extent(width, height)
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

pub(crate) fn render_relation_component_lines<'plan, R, A>(
    boxes: &'plan [RelationGraphBox],
    relations: &'plan [R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &'plan A,
) -> Result<Option<Vec<RelationGraphLine>>>
where
    A: RelationComponentAdapter<R> + 'plan,
{
    let edges = build_layered_edges(relations, adapter, resources)?;
    let layered_error = |error| adapter.layered_error(error);
    let components = relation_components(boxes, &edges, resources)
        .map_err(|error| error.into_ascii_error(layered_error))?;
    resources.charge_layout_work(components.len().max(1))?;
    let mut relation_regions = Vec::new();
    relation_regions
        .try_reserve_exact(components.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut standalone_regions = Vec::new();
    standalone_regions
        .try_reserve_exact(components.len())
        .map_err(|_| layout_allocation_failed())?;
    for component in components {
        let has_relations = !component.edge_indices().is_empty();
        let region =
            plan_relation_component_region(component, relations, options, resources, adapter)?;
        if has_relations {
            relation_regions.push(region);
        } else {
            standalone_regions.push(region);
        }
    }

    let mut regions = Vec::new();
    regions
        .try_reserve_exact(
            relation_regions
                .len()
                .checked_add(standalone_regions.len())
                .ok_or_else(|| work_overflow(resources))?,
        )
        .map_err(|_| layout_allocation_failed())?;
    if relation_regions.len() > 1 && relation_regions.iter().all(|region| !region.is_summary()) {
        regions.push(RelationRegionPlan::horizontal_strip(
            relation_regions,
            adapter.layered_horizontal_gap(),
            resources,
        )?);
    } else {
        regions.extend(relation_regions);
    }
    regions.extend(standalone_regions);
    let plan = RelationRenderPlan::try_new(regions, resources)?;
    Ok(Some(plan.materialize(options, resources)?))
}

fn plan_relation_component_region<'plan, R, A>(
    component: RelationGraphComponent<'plan>,
    relations: &'plan [R],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    adapter: &'plan A,
) -> Result<RelationRegionPlan<'plan>>
where
    A: RelationComponentAdapter<R> + 'plan,
{
    let (component_boxes, edge_indices) = component.into_parts();
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(edge_indices.len())
        .map_err(|_| layout_allocation_failed())?;
    for edge_index in edge_indices {
        selected.push(
            relations
                .get(edge_index)
                .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?,
        );
    }
    if selected.is_empty() {
        return Ok(RelationRegionPlan::BoxStrip(RelationBoxStripPlan::stacked(
            component_boxes,
            resources,
        )?));
    }

    let has_self = selected
        .iter()
        .any(|relation| adapter.is_self_relation(*relation));
    let has_non_self = selected
        .iter()
        .any(|relation| !adapter.is_self_relation(*relation));
    if has_self && has_non_self {
        return plan_relation_summary_region(
            component_boxes,
            selected,
            LayeredRelationSummaryReason::RouteCollision,
            options,
            resources,
            adapter,
        );
    }

    if has_self {
        let first_edge = adapter.build_edges(selected[0]);
        let same_endpoint = selected.iter().all(|relation| {
            let edge = adapter.build_edges(*relation);
            edge.source_id() == first_edge.source_id() && edge.target_id() == first_edge.target_id()
        });
        if same_endpoint {
            let relation_box = find_box_ref(&component_boxes, first_edge.source_id())
                .ok_or_else(|| adapter.layered_error(LayeredRelationError::MissingEndpoint))?;
            return plan_relation_self_loop_region(relation_box, selected, adapter, resources);
        }
    }

    if selected.len() > 1 && same_directed_endpoints(&selected, adapter) {
        return adapter.plan_parallel_region(component_boxes, selected, options, resources);
    }
    if let [relation] = selected.as_slice() {
        return adapter.plan_vertical_region(&component_boxes, relation, resources);
    }

    match plan_layered_relation_component_ref_result(
        &component_boxes,
        &selected,
        options,
        adapter.layered_horizontal_gap(),
        resources,
        adapter,
    )? {
        Ok(plan) => Ok(RelationRegionPlan::Layered(plan)),
        Err(reason) => plan_relation_summary_region(
            component_boxes,
            selected,
            reason,
            options,
            resources,
            adapter,
        ),
    }
}

fn same_directed_endpoints<R, A>(relations: &[&R], adapter: &A) -> bool
where
    A: RelationComponentAdapter<R>,
{
    let Some(first) = relations.first() else {
        return false;
    };
    let first = adapter.build_edges(first);
    relations.iter().skip(1).all(|relation| {
        let edge = adapter.build_edges(*relation);
        edge.source_id() == first.source_id() && edge.target_id() == first.target_id()
    })
}

fn plan_relation_self_loop_region<'plan, R, A>(
    relation_box: &'plan RelationGraphBox,
    relations: Vec<&'plan R>,
    adapter: &'plan A,
    resources: &mut ResourceContext,
) -> Result<RelationRegionPlan<'plan>>
where
    A: RelationComponentAdapter<R> + 'plan,
{
    let mut metrics = Vec::new();
    metrics
        .try_reserve_exact(relations.len())
        .map_err(|_| layout_allocation_failed())?;
    for relation in &relations {
        metrics.push(adapter.self_loop_metrics(relation, resources)?);
    }
    let plan = RelationSelfLoopPlan::try_new(relation_box, metrics, resources)?;
    Ok(RelationRegionPlan::SelfLoops {
        plan,
        rows: Box::new(move |resources| {
            let mut loops = Vec::new();
            loops
                .try_reserve_exact(relations.len())
                .map_err(|_| layout_allocation_failed())?;
            for relation in relations {
                loops.push(adapter.self_loop_rows(relation, resources)?);
            }
            Ok(loops)
        }),
    })
}

fn plan_relation_summary_region<'plan, R, A>(
    boxes: Vec<&'plan RelationGraphBox>,
    relations: Vec<&'plan R>,
    reason: LayeredRelationSummaryReason,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
    adapter: &A,
) -> Result<RelationRegionPlan<'plan>>
where
    A: RelationComponentAdapter<R>,
{
    let mut rows = Vec::new();
    rows.try_reserve_exact(relations.len())
        .map_err(|_| layout_allocation_failed())?;
    for relation in relations {
        rows.push(adapter.build_summary_row(relation, reason)?);
    }
    Ok(RelationRegionPlan::Summary(
        RelationSummaryPaintPlan::stacked(boxes, rows, Some(reason), options, resources)?,
    ))
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

#[cfg(test)]
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
        Err(reason) => render_relation_summary_component_lines(
            boxes,
            relations,
            options,
            reason,
            resources,
            |relation| adapter.build_summary_row(relation, reason),
        ),
    }
}

/// Admit a base relation block and an optional lossless summary as one logical
/// document before either block allocates its terminal rows.
pub(crate) fn render_relation_document_with_summary(
    base_extent: LogicalExtent,
    rows: &[RelationGraphSummaryRow],
    reason: Option<LayeredRelationSummaryReason>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    build_base: impl FnOnce(&mut ResourceContext) -> Result<Vec<RelationGraphLine>>,
) -> Result<Vec<RelationGraphLine>> {
    let summary_extent = if rows.is_empty() {
        None
    } else {
        Some(relation_summary_extent(rows, reason, options, resources)?)
    };
    let plan = RelationDocumentPlan::new(
        base_extent,
        summary_extent,
        display_width_with_profile("relations:", options.terminal_width_profile),
        resources,
    )?;
    if rows.is_empty() {
        plan.materialize(resources, build_base)
    } else {
        plan.materialize_with_section(options, resources, build_base, |resources| {
            relation_summary_lines_for_rows(rows, reason, options, resources)
        })
    }
}

/// Render a lossless relation summary for a component whose spatial plan is
/// not safe to materialize. This is shared by family-owned parallel planners so
/// they can reject invalid endpoint ports without duplicating section assembly.
#[cfg(test)]
pub(crate) fn render_relation_summary_component_lines<R>(
    boxes: &[RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    reason: LayeredRelationSummaryReason,
    resources: &mut ResourceContext,
    mut build_row: impl FnMut(&R) -> Result<RelationGraphSummaryRow>,
) -> Result<Vec<RelationGraphLine>> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(relations.len())
        .map_err(|_| layout_allocation_failed())?;
    for relation in relations {
        rows.push(build_row(relation)?);
    }
    let base_extent = stacked_box_extent(boxes, resources)?;
    render_relation_document_with_summary(
        base_extent,
        &rows,
        Some(reason),
        options,
        resources,
        |resources| stacked_box_lines(boxes, options.terminal_width_profile, resources),
    )
}

#[cfg(test)]
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
    match plan_layered_relation_component_result(
        boxes,
        relations,
        options,
        horizontal_gap,
        resources,
        adapter,
    )? {
        Ok(plan) => Ok(Ok(plan.paint(options, resources)?)),
        Err(reason) => Ok(Err(reason)),
    }
}

#[cfg(test)]
fn plan_layered_relation_component_result<'boxes, R, A>(
    boxes: &'boxes [RelationGraphBox],
    relations: &[R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<std::result::Result<LayeredRelationPaintPlan<'boxes>, LayeredRelationSummaryReason>>
where
    A: RelationComponentAdapter<R>,
{
    let box_refs = boxes.iter().collect::<Vec<_>>();
    let relation_refs = relations.iter().collect::<Vec<_>>();
    plan_layered_relation_component_ref_result(
        &box_refs,
        &relation_refs,
        options,
        horizontal_gap,
        resources,
        adapter,
    )
}

fn plan_layered_relation_component_ref_result<'boxes, R, A>(
    boxes: &[&'boxes RelationGraphBox],
    relations: &[&R],
    options: &AsciiRenderOptions,
    horizontal_gap: usize,
    resources: &mut ResourceContext,
    adapter: &A,
) -> Result<std::result::Result<LayeredRelationPaintPlan<'boxes>, LayeredRelationSummaryReason>>
where
    A: RelationComponentAdapter<R>,
{
    let has_self_relation = relations
        .iter()
        .any(|relation| adapter.is_self_relation(*relation));
    if has_self_relation
        && relations
            .iter()
            .any(|relation| !adapter.is_self_relation(*relation))
    {
        return Ok(Err(LayeredRelationSummaryReason::RouteCollision));
    }
    resources.charge_layout_work(relations.len().max(1))?;
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(relations.len())
        .map_err(|_| layout_allocation_failed())?;
    edges.extend(
        relations
            .iter()
            .map(|relation| adapter.build_edges(*relation)),
    );
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
        let relation = relations[edge_index];
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

        if !scene.edge_ports_fit(edge_index, route_plan.source_x(), route_plan.target_x()) {
            return Ok(Err(LayeredRelationSummaryReason::RouteCollision));
        }

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
    for (index, route_plan) in route_plans.iter().enumerate() {
        if route_plans[index + 1..]
            .iter()
            .any(|other| route_plan.overlays_overlap(other))
        {
            return Ok(Err(LayeredRelationSummaryReason::OverlayCollision));
        }
    }
    if route_plans
        .iter()
        .any(|route_plan| scene.route_overlaps_box(route_plan))
    {
        return Ok(Err(LayeredRelationSummaryReason::RouteCollision));
    }
    if route_plans
        .iter()
        .any(|route_plan| scene.overlays_overlap_box(route_plan))
    {
        return Ok(Err(LayeredRelationSummaryReason::OverlayCollision));
    }
    let extent = resources.grid_extent(scene.width(), scene.height())?;
    Ok(Ok(LayeredRelationPaintPlan {
        scene,
        routes: route_plans,
        extent,
    }))
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

pub(crate) fn find_box_ref<'a>(
    boxes: &[&'a RelationGraphBox],
    id: &str,
) -> Option<&'a RelationGraphBox> {
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

fn vertical_stack_extent(
    top: &RelationGraphBox,
    bottom: &RelationGraphBox,
    center: usize,
    relation_extent: LogicalExtent,
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let height = resources.checked_grid_add(
        resources.checked_grid_add(top.height(), relation_extent.height())?,
        bottom.height(),
    )?;
    let top_left = center
        .checked_sub(top.width() / 2)
        .ok_or_else(|| grid_overflow(resources))?;
    let bottom_left = center
        .checked_sub(bottom.width() / 2)
        .ok_or_else(|| grid_overflow(resources))?;
    let top_width = resources.checked_grid_add(top_left, top.width())?;
    let bottom_width = resources.checked_grid_add(bottom_left, bottom.width())?;
    resources.grid_extent(
        relation_extent.width().max(top_width).max(bottom_width),
        height,
    )
}

fn assemble_vertical_stack_lines(
    top: &RelationGraphBox,
    bottom: &RelationGraphBox,
    center: usize,
    relation_lines: Vec<RelationGraphLine>,
    extent: LogicalExtent,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(extent.height())
        .map_err(|_| layout_allocation_failed())?;
    lines.extend(try_align_box_lines(top, center, resources)?);
    lines.extend(relation_lines);
    lines.extend(try_align_box_lines(bottom, center, resources)?);
    debug_assert_eq!(lines.len(), extent.height());
    debug_assert_eq!(
        lines
            .iter()
            .map(RelationGraphLine::width)
            .max()
            .unwrap_or(0),
        extent.width()
    );
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

pub(crate) fn render_lines_with_options(
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

        fn is_self_relation(&self, relation: &R) -> bool {
            relation.source_id() == relation.target_id()
        }

        fn self_loop_metrics(
            &self,
            _relation: &R,
            _resources: &ResourceContext,
        ) -> Result<RelationSelfLoopMetrics> {
            Ok(RelationSelfLoopMetrics::new(1, 0, 0, 1, None, '-', '|'))
        }

        fn self_loop_rows(
            &self,
            _relation: &R,
            resources: &ResourceContext,
        ) -> Result<RelationSelfLoopRows> {
            let line = RelationGraphLine::try_with_role(
                "-",
                AsciiColorRole::EdgeLine,
                TerminalWidthProfile::Unicode,
                resources,
            )?;
            Ok(RelationSelfLoopRows::new(
                line.clone(),
                Vec::new(),
                line,
                '-',
                '|',
            ))
        }

        fn horizontal_relation_style(
            &self,
            _relation: &R,
            _source_side: RelationPortSide,
            _target_side: RelationPortSide,
            _resources: &ResourceContext,
        ) -> Result<HorizontalRelationStyle> {
            Ok(HorizontalRelationStyle::new(
                HorizontalRelationEndpoint::new(None, None),
                HorizontalRelationEndpoint::new(None, None),
                None,
                '-',
                '|',
                RelationLineChars::new(['-', '|', '.', ':'], '+'),
            ))
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

        fn plan_vertical_region<'plan>(
            &self,
            boxes: &[&'plan RelationGraphBox],
            relation: &'plan R,
            resources: &mut ResourceContext,
        ) -> Result<RelationRegionPlan<'plan>> {
            let top = find_box_ref(boxes, relation.source_id()).ok_or_else(|| {
                <Self as RelationComponentAdapter<R>>::layered_error(
                    self,
                    LayeredRelationError::MissingEndpoint,
                )
            })?;
            let bottom = find_box_ref(boxes, relation.target_id()).ok_or_else(|| {
                <Self as RelationComponentAdapter<R>>::layered_error(
                    self,
                    LayeredRelationError::MissingEndpoint,
                )
            })?;
            let plan =
                RelationStackPlan::try_new(top, bottom, &[], resources, |_center, resources| {
                    resources.grid_extent(0, 0)
                })?;
            Ok(RelationRegionPlan::Vertical {
                plan,
                rows: Box::new(|_center, _resources| Ok(Vec::new())),
            })
        }

        fn plan_parallel_region<'plan>(
            &self,
            boxes: Vec<&'plan RelationGraphBox>,
            _relations: Vec<&'plan R>,
            _options: &AsciiRenderOptions,
            resources: &mut ResourceContext,
        ) -> Result<RelationRegionPlan<'plan>> {
            Ok(RelationRegionPlan::BoxStrip(RelationBoxStripPlan::stacked(
                boxes, resources,
            )?))
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

    fn options_with_grid_limit(max: usize) -> AsciiRenderOptions {
        AsciiRenderOptions::ascii()
            .with_resource_limit(AsciiResourceLimitId::MaxGridCells, max)
            .expect("test grid limit should be valid")
    }

    fn assert_grid_limit(error: AsciiError, actual: usize, max: usize) {
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == actual
                    && details.max == max
        ));
    }

    fn admission_test_boxes() -> Vec<RelationGraphBox> {
        vec![
            RelationGraphBox::new(
                "a".to_string(),
                vec!["aaa".to_string(), "aaa".to_string()],
                3,
            ),
            RelationGraphBox::new(
                "b".to_string(),
                vec!["bbbb".to_string(), "bbbb".to_string(), "bbbb".to_string()],
                4,
            ),
        ]
    }

    fn aggregate_test_regions<'a>(
        wide_top: &'a RelationGraphBox,
        wide_bottom: &'a RelationGraphBox,
        narrow_top: &'a RelationGraphBox,
        narrow_bottom: &'a RelationGraphBox,
        first_called: &'a Cell<bool>,
        second_called: &'a Cell<bool>,
        resources: &ResourceContext,
    ) -> Result<Vec<RelationRegionPlan<'a>>> {
        let first = RelationStackPlan::try_new(
            wide_top,
            wide_bottom,
            &[],
            resources,
            |center, resources| centered_row_blocks_extent(center, [(1, 1)], resources),
        )?;
        let second = RelationStackPlan::try_new(
            narrow_top,
            narrow_bottom,
            &[],
            resources,
            |_center, resources| resources.grid_extent(0, 0),
        )?;
        Ok(vec![
            RelationRegionPlan::Vertical {
                plan: first,
                rows: Box::new(move |center, resources| {
                    first_called.set(true);
                    Ok(vec![centered_text_line_with_role(
                        "|",
                        center,
                        AsciiColorRole::EdgeLine,
                        TerminalWidthProfile::Unicode,
                        resources,
                    )?])
                }),
            },
            RelationRegionPlan::Vertical {
                plan: second,
                rows: Box::new(move |_center, _resources| {
                    second_called.set(true);
                    Ok(Vec::new())
                }),
            },
        ])
    }

    #[test]
    fn render_plan_admits_aggregate_extent_before_materializing_regions() -> Result<()> {
        let options = options_with_grid_limit(30);
        let mut resources = test_resources(&options);
        let wide_top = RelationGraphBox::new("a".to_string(), vec!["aaaaa".to_string()], 5);
        let wide_bottom = RelationGraphBox::new("b".to_string(), vec!["bbbbb".to_string()], 5);
        let narrow_top = RelationGraphBox::new("c".to_string(), vec!["ccc".to_string()], 3);
        let narrow_bottom = RelationGraphBox::new("d".to_string(), vec!["ddd".to_string()], 3);
        let first_called = Cell::new(false);
        let second_called = Cell::new(false);
        let regions = aggregate_test_regions(
            &wide_top,
            &wide_bottom,
            &narrow_top,
            &narrow_bottom,
            &first_called,
            &second_called,
            &resources,
        )?;

        let plan = RelationRenderPlan::try_new(regions, &mut resources)?;
        assert_eq!(plan.extent(), resources.grid_extent(5, 6)?);
        assert!(!first_called.get());
        assert!(!second_called.get());
        let lines = plan.materialize(&options, &mut resources)?;
        assert_eq!(
            relation_lines_extent(&lines, &resources)?,
            resources.grid_extent(5, 6)?
        );
        assert!(first_called.get());
        assert!(second_called.get());
        Ok(())
    }

    #[test]
    fn render_plan_rejects_aggregate_n_minus_one_before_materializing_regions() -> Result<()> {
        let options = options_with_grid_limit(29);
        let mut resources = test_resources(&options);
        let wide_top = RelationGraphBox::new("a".to_string(), vec!["aaaaa".to_string()], 5);
        let wide_bottom = RelationGraphBox::new("b".to_string(), vec!["bbbbb".to_string()], 5);
        let narrow_top = RelationGraphBox::new("c".to_string(), vec!["ccc".to_string()], 3);
        let narrow_bottom = RelationGraphBox::new("d".to_string(), vec!["ddd".to_string()], 3);
        let first_called = Cell::new(false);
        let second_called = Cell::new(false);
        let regions = aggregate_test_regions(
            &wide_top,
            &wide_bottom,
            &narrow_top,
            &narrow_bottom,
            &first_called,
            &second_called,
            &resources,
        )?;

        let error = match RelationRenderPlan::try_new(regions, &mut resources) {
            Ok(_) => panic!("aggregate N-1 must fail before painting any region"),
            Err(error) => error,
        };
        assert_grid_limit(error, 30, 29);
        assert!(!first_called.get());
        assert!(!second_called.get());
        Ok(())
    }

    fn parallel_test_lane_extents(resources: &ResourceContext) -> Vec<LogicalExtent> {
        vec![
            resources
                .grid_extent(1, 2)
                .expect("first lane extent should fit"),
            resources
                .grid_extent(1, 2)
                .expect("second lane extent should fit"),
        ]
    }

    fn materialize_parallel_test_lanes(
        resources: &ResourceContext,
    ) -> Result<Vec<Vec<RelationGraphLine>>> {
        let width_profile = TerminalWidthProfile::Unicode;
        Ok(vec![
            vec![
                RelationGraphLine::try_plain("^", width_profile, resources)?,
                RelationGraphLine::try_plain("|", width_profile, resources)?,
            ],
            vec![
                RelationGraphLine::try_plain("^", width_profile, resources)?,
                RelationGraphLine::try_plain("|", width_profile, resources)?,
            ],
        ])
    }

    fn materialize_stack_test_rows(
        center: usize,
        resources: &ResourceContext,
    ) -> Result<Vec<RelationGraphLine>> {
        Ok(vec![marker_line_with_role(
            '|',
            center,
            AsciiColorRole::EdgeLine,
            TerminalWidthProfile::Unicode,
            resources,
        )?])
    }

    fn self_loop_test_metrics() -> Vec<RelationSelfLoopMetrics> {
        vec![
            RelationSelfLoopMetrics::new(1, 1, 1, 1, None, '-', '|'),
            RelationSelfLoopMetrics::new(1, 2, 1, 1, Some(1), '-', '|'),
        ]
    }

    fn materialize_self_loop_test_rows(
        resources: &ResourceContext,
    ) -> Result<Vec<RelationSelfLoopRows>> {
        let width_profile = TerminalWidthProfile::Unicode;
        let marker = |text, resources: &ResourceContext| {
            RelationGraphLine::try_with_role(
                text,
                AsciiColorRole::EdgeArrow,
                width_profile,
                resources,
            )
        };
        Ok(vec![
            RelationSelfLoopRows::new(
                marker("^", resources)?,
                vec![RelationGraphLine::try_plain("x", width_profile, resources)?],
                marker("v", resources)?,
                '-',
                '|',
            ),
            RelationSelfLoopRows::new(
                marker("^", resources)?,
                vec![RelationGraphLine::try_plain(
                    "yy",
                    width_profile,
                    resources,
                )?],
                marker("v", resources)?,
                '-',
                '|',
            )
            .with_tail_prefix(marker(">", resources)?),
        ])
    }

    #[test]
    fn self_loop_plan_admits_exact_extent_before_materializing() {
        let boxes = admission_test_boxes();
        let options = options_with_grid_limit(42);
        let mut resources = test_resources(&options);
        let plan = RelationSelfLoopPlan::try_new(&boxes[0], self_loop_test_metrics(), &resources)
            .expect("self-loop descriptor should fit the exact aggregate limit");
        assert_eq!(
            plan.extent(),
            resources
                .grid_extent(7, 6)
                .expect("7 by 6 should fit the exact limit")
        );

        let materialized = Cell::new(false);
        let lines = plan
            .render_lines(&mut resources, |resources| {
                materialized.set(true);
                materialize_self_loop_test_rows(resources)
            })
            .expect("7 by 6 self-loop layout should fit 42 cells");

        assert!(materialized.get());
        assert_eq!(lines.len(), 6);
        assert_eq!(lines.iter().map(RelationGraphLine::width).max(), Some(7));
    }

    #[test]
    fn self_loop_plan_rejects_n_minus_one_before_materializing() {
        let boxes = admission_test_boxes();
        let options = options_with_grid_limit(41);
        let mut resources = test_resources(&options);
        let materialized = Cell::new(false);

        let error = RelationSelfLoopPlan::try_new(&boxes[0], self_loop_test_metrics(), &resources)
            .and_then(|plan| {
                plan.render_lines(&mut resources, |resources| {
                    materialized.set(true);
                    materialize_self_loop_test_rows(resources)
                })
            })
            .expect_err("7 by 6 self-loop layout must not fit 41 cells");

        assert_grid_limit(error, 42, 41);
        assert!(!materialized.get());
    }

    #[test]
    fn self_loop_plan_rejects_materialized_descriptor_mismatch() {
        let boxes = admission_test_boxes();
        let options = options_with_grid_limit(42);
        let mut resources = test_resources(&options);
        let plan = RelationSelfLoopPlan::try_new(&boxes[0], self_loop_test_metrics(), &resources)
            .expect("self-loop descriptor should fit before row validation");
        let error = plan
            .render_lines(&mut resources, |resources| {
                let mut rows = materialize_self_loop_test_rows(resources)?;
                rows[0].label_lines[0] =
                    RelationGraphLine::try_plain("xx", TerminalWidthProfile::Unicode, resources)?;
                Ok(rows)
            })
            .expect_err("materialized label width must match its admitted descriptor");

        assert_grid_limit(error, usize::MAX, 42);
    }

    #[test]
    fn stack_plan_admits_exact_extent_before_materializing() {
        let boxes = admission_test_boxes();
        let options = options_with_grid_limit(24);
        let mut resources = test_resources(&options);
        let plan = RelationStackPlan::try_new(
            &boxes[0],
            &boxes[1],
            &[],
            &resources,
            |center, resources| centered_row_blocks_extent(center, [(1, 1)], resources),
        )
        .expect("relation row descriptor should fit before aggregate admission");
        assert_eq!(
            plan.extent(),
            resources
                .grid_extent(4, 6)
                .expect("4 by 6 should fit the exact limit")
        );

        let materialized = Cell::new(false);
        let lines = plan
            .render_lines(&mut resources, |center, resources| {
                materialized.set(true);
                materialize_stack_test_rows(center, resources)
            })
            .expect("4 by 6 relation stack should fit 24 cells");

        assert!(materialized.get());
        assert_eq!(lines.len(), 6);
        assert_eq!(lines.iter().map(RelationGraphLine::width).max(), Some(4));
    }

    #[test]
    fn stack_plan_rejects_n_minus_one_before_materializing() {
        let boxes = admission_test_boxes();
        let options = options_with_grid_limit(23);
        let mut resources = test_resources(&options);
        let materialized = Cell::new(false);

        let error = RelationStackPlan::try_new(
            &boxes[0],
            &boxes[1],
            &[],
            &resources,
            |center, resources| centered_row_blocks_extent(center, [(1, 1)], resources),
        )
        .and_then(|plan| {
            plan.render_lines(&mut resources, |center, resources| {
                materialized.set(true);
                materialize_stack_test_rows(center, resources)
            })
        })
        .expect_err("4 by 6 relation stack must not fit 23 cells");

        assert_grid_limit(error, 24, 23);
        assert!(!materialized.get());
    }

    #[test]
    fn parallel_plan_admits_odd_endpoint_extent_at_exact_limit_before_materializing() {
        let top = RelationGraphBox::new("top".to_string(), vec!["abcde".to_string()], 5);
        let bottom = RelationGraphBox::new("bottom".to_string(), vec!["vwxyz".to_string()], 5);
        let default_options = AsciiRenderOptions::ascii();
        let mut default_resources = test_resources(&default_options);
        let plan = RelationParallelPlan::new(
            &top,
            &bottom,
            parallel_test_lane_extents(&default_resources),
            2,
            &mut default_resources,
        )
        .expect("parallel geometry should plan from lane extents");
        assert!(
            plan.ports_fit(&default_resources)
                .expect("wide endpoints should accept both ports")
        );
        let planned = plan.extent();
        assert_eq!(
            (planned.width(), planned.height(), planned.cells()),
            (5, 4, 20)
        );

        let options = options_with_grid_limit(planned.cells());
        let mut resources = test_resources(&options);
        let plan = RelationParallelPlan::new(
            &top,
            &bottom,
            parallel_test_lane_extents(&resources),
            2,
            &mut resources,
        )
        .expect("exact-limit parallel geometry should plan");
        assert!(
            plan.ports_fit(&resources)
                .expect("wide endpoints should accept both ports")
        );
        let materialized = Cell::new(false);
        let lines = plan
            .render_lines(&mut resources, |resources| {
                materialized.set(true);
                materialize_parallel_test_lanes(resources)
            })
            .expect("5 by 4 parallel document should fit 20 cells");

        assert!(materialized.get());
        assert_eq!(lines.len(), 4);
        assert_eq!(lines.iter().map(RelationGraphLine::width).max(), Some(5));
    }

    #[test]
    fn parallel_plan_rejects_odd_endpoint_extent_at_n_minus_one_before_materializing() {
        let top = RelationGraphBox::new("top".to_string(), vec!["abcde".to_string()], 5);
        let bottom = RelationGraphBox::new("bottom".to_string(), vec!["vwxyz".to_string()], 5);
        let options = options_with_grid_limit(19);
        let mut resources = test_resources(&options);
        let materialized = Cell::new(false);
        let error = RelationParallelPlan::new(
            &top,
            &bottom,
            parallel_test_lane_extents(&resources),
            2,
            &mut resources,
        )
        .and_then(|plan| {
            plan.render_lines(&mut resources, |resources| {
                materialized.set(true);
                materialize_parallel_test_lanes(resources)
            })
        })
        .expect_err("5 by 4 parallel document must not fit 19 cells");

        assert_grid_limit(error, 20, 19);
        assert!(!materialized.get());
    }

    #[test]
    fn stack_and_horizontal_strip_admit_exact_grid_extent() {
        let boxes = admission_test_boxes();

        let stack_options = options_with_grid_limit(24);
        let mut stack_resources = test_resources(&stack_options);
        let stack = stacked_box_lines_ordered(
            &boxes,
            stack_options.terminal_width_profile,
            true,
            &mut stack_resources,
        )
        .expect("4 by 6 reversed stack should fit 24 cells");
        assert_eq!(stack.len(), 6);
        assert_eq!(stack[0].width(), 4);
        assert_eq!(stack[4].width(), 3);

        let horizontal_options = options_with_grid_limit(27);
        let horizontal_resources = test_resources(&horizontal_options);
        let strip = render_horizontal_box_strip_lines(
            &boxes,
            RelationGraphHorizontalDirection::LeftRight,
            2,
            horizontal_options.terminal_width_profile,
            &horizontal_resources,
        )
        .expect("9 by 3 horizontal strip should fit 27 cells");
        assert_eq!(strip.len(), 3);
        assert!(strip.iter().all(|line| line.width() == 9));
    }

    #[test]
    fn stack_and_horizontal_strip_reject_grid_extent_at_n_minus_one() {
        let boxes = admission_test_boxes();

        let stack_options = options_with_grid_limit(23);
        let mut stack_resources = test_resources(&stack_options);
        let error = stacked_box_lines_ordered(
            &boxes,
            stack_options.terminal_width_profile,
            true,
            &mut stack_resources,
        )
        .expect_err("4 by 6 reversed stack must not fit 23 cells");
        assert_grid_limit(error, 24, 23);

        let horizontal_options = options_with_grid_limit(26);
        let horizontal_resources = test_resources(&horizontal_options);
        let error = render_horizontal_box_strip_lines(
            &boxes,
            RelationGraphHorizontalDirection::LeftRight,
            2,
            horizontal_options.terminal_width_profile,
            &horizontal_resources,
        )
        .expect_err("9 by 3 horizontal strip must not fit 26 cells");
        assert_grid_limit(error, 27, 26);
    }

    #[test]
    fn relation_document_admits_exact_extent_before_materializing() {
        let boxes = admission_test_boxes();
        let rows = vec![RelationGraphSummaryRow::new("A", "-->", "B")];
        let default_options = AsciiRenderOptions::ascii();
        let default_resources = test_resources(&default_options);
        let base_extent = stacked_box_extent(&boxes, &default_resources)
            .expect("base stack should have a checked extent");
        let summary_extent =
            relation_summary_extent(&rows, None, &default_options, &default_resources)
                .expect("summary should have a checked extent");
        let planned =
            RelationDocumentPlan::new(base_extent, Some(summary_extent), 10, &default_resources)
                .expect("aggregate document should have a checked extent")
                .extent();
        assert_eq!(
            (planned.width(), planned.height(), planned.cells()),
            (10, 9, 90)
        );

        let exact = planned.cells();
        let options = options_with_grid_limit(exact);
        let mut resources = test_resources(&options);
        let base_extent = stacked_box_extent(&boxes, &resources)
            .expect("base stack should fit the aggregate limit");
        let materialized = Cell::new(false);

        let lines = render_relation_document_with_summary(
            base_extent,
            &rows,
            None,
            &options,
            &mut resources,
            |resources| {
                materialized.set(true);
                stacked_box_lines_ordered(&boxes, options.terminal_width_profile, true, resources)
            },
        )
        .expect("10 by 9 aggregate document should fit 90 cells");

        assert!(materialized.get());
        assert_eq!(lines.len(), 9);
        assert_eq!(lines.iter().map(RelationGraphLine::width).max(), Some(10));
    }

    #[test]
    fn relation_document_rejects_n_minus_one_before_materializing() {
        let boxes = admission_test_boxes();
        let rows = vec![RelationGraphSummaryRow::new("A", "-->", "B")];
        let options = options_with_grid_limit(89);
        let mut resources = test_resources(&options);
        let base_extent = stacked_box_extent(&boxes, &resources)
            .expect("base stack should fit before aggregate admission");
        let materialized = Cell::new(false);

        let error = render_relation_document_with_summary(
            base_extent,
            &rows,
            None,
            &options,
            &mut resources,
            |resources| {
                materialized.set(true);
                stacked_box_lines_ordered(&boxes, options.terminal_width_profile, true, resources)
            },
        )
        .expect_err("10 by 9 aggregate document must not fit 89 cells");

        assert_grid_limit(error, 90, 89);
        assert!(!materialized.get());
    }

    #[test]
    fn render_stacked_boxes_preserves_plain_text() {
        let boxes = [
            RelationGraphBox::new("a".to_string(), vec!["A".to_string(), "|".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string(), "|".to_string()], 1),
        ];

        assert_eq!(render_stacked_boxes(&boxes), "A\n|\n\nB\n|\n");
    }

    #[test]
    fn render_stacked_boxes_with_section_appends_summary() {
        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(&options);
        let boxes = [
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
        let boxes = [
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
        let boxes = [
            RelationGraphBox::new("top".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("bottom".to_string(), vec!["B".to_string()], 1),
        ];
        let no_label_edges = vec![LayeredRelationEdge::new("top", "bottom", 0, 0)];
        let one_line_edges = vec![LayeredRelationEdge::new("top", "bottom", 0, 1)];
        let two_line_edges = vec![LayeredRelationEdge::new("top", "bottom", 0, 2)];

        let options = AsciiRenderOptions::ascii();
        let box_refs = boxes.iter().collect::<Vec<_>>();
        let mut resources = test_resources(&options);
        let no_label_plan =
            plan_layered_relation_boxes(&box_refs, &no_label_edges, 1, &mut resources)
                .expect("unlabeled layered relation should plan");
        let mut resources = test_resources(&options);
        let one_line_plan =
            plan_layered_relation_boxes(&box_refs, &one_line_edges, 1, &mut resources)
                .expect("single-line labeled relation should plan");
        let mut resources = test_resources(&options);
        let two_line_plan =
            plan_layered_relation_boxes(&box_refs, &two_line_edges, 1, &mut resources)
                .expect("multiline labeled relation should plan");

        assert_eq!(no_label_plan.height(), 5);
        assert_eq!(one_line_plan.height(), 6);
        assert_eq!(two_line_plan.height(), 7);
    }

    #[test]
    fn layered_relation_plan_reserves_width_for_reverse_spanning_edges() {
        let boxes = [
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
        let box_refs = boxes.iter().collect::<Vec<_>>();
        let mut resources = test_resources(&options);
        let plan = plan_layered_relation_boxes(&box_refs, &edges, 1, &mut resources)
            .expect("cyclic plan should render");

        assert_eq!(plan.width(), 7);
    }

    #[test]
    fn layered_relation_plan_reserves_width_for_reverse_parallel_lanes() {
        let boxes = [
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        ];
        let edges = vec![
            LayeredRelationEdge::new("a", "b", 0, 0),
            LayeredRelationEdge::new("b", "a", 0, 0),
        ];

        let options = AsciiRenderOptions::ascii();
        let box_refs = boxes.iter().collect::<Vec<_>>();
        let mut resources = test_resources(&options);
        let plan = plan_layered_relation_boxes(&box_refs, &edges, 1, &mut resources)
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

use super::{grid_overflow, layout_allocation_failed};
use crate::Result;
use crate::canvas::Canvas;
use crate::color::AsciiColorRole;
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::safe_text::{
    DeferredTextLine, DeferredTextLineMetrics, DeferredTextRegistry, NormalizedLabelMetrics,
    NormalizedLabelPlan, try_plan_normalized_label_lines,
};
use crate::text::StyledLine;
#[cfg(test)]
use crate::text::display_width_with_profile;
use std::rc::Rc;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RelationGraphLine {
    pub(super) line: Rc<StyledLine>,
}

impl Clone for RelationGraphLine {
    fn clone(&self) -> Self {
        self.shared()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelationGraphBox {
    pub(super) id: Rc<String>,
    pub(super) lines: Rc<Vec<RelationGraphLine>>,
    pub(super) width: usize,
    pub(super) width_profile: TerminalWidthProfile,
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
    lines: Rc<Vec<DeferredTextLine>>,
    width: usize,
    width_profile: TerminalWidthProfile,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RelationGraphLabelPlan<'a> {
    raw: &'a str,
    normalized: Option<NormalizedLabelPlan>,
    disclosure_prefix: &'static str,
    preserve_empty_authored: bool,
    disclose_authored: bool,
    metrics: RelationGraphLabelMetrics,
    width_profile: TerminalWidthProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RelationGraphLabelMetrics {
    grid_cells: usize,
    document_cells: usize,
    plain_bytes: usize,
    html_bytes: usize,
    line_count: usize,
    width: usize,
}

impl RelationGraphLabelMetrics {
    const EMPTY: Self = Self {
        grid_cells: 0,
        document_cells: 0,
        plain_bytes: 0,
        html_bytes: 0,
        line_count: 0,
        width: 0,
    };

    fn try_from_normalized(
        raw: &str,
        plan: NormalizedLabelPlan,
        html_bytes: usize,
        disclosure: Option<DeferredTextLineMetrics>,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let NormalizedLabelMetrics {
            materialized_bytes,
            document_cells,
            line_count,
            max_width,
        } = plan.metrics();
        let mut metrics = Self {
            grid_cells: 0,
            document_cells,
            plain_bytes: materialized_bytes,
            html_bytes,
            line_count,
            width: max_width,
        };
        if let Some(disclosure) = disclosure {
            metrics.try_include_line(disclosure, resources)?;
        }
        metrics.grid_cells =
            resources.checked_grid_mul(metrics.width.max(1), metrics.line_count.max(1))?;
        debug_assert!(!raw.is_empty() || line_count > 0);
        Ok(metrics)
    }

    fn try_include_line(
        &mut self,
        line: DeferredTextLineMetrics,
        resources: &ResourceContext,
    ) -> Result<()> {
        self.document_cells = self
            .document_cells
            .checked_add(line.width)
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
        self.plain_bytes = self
            .plain_bytes
            .checked_add(line.plain_bytes)
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        self.html_bytes = self
            .html_bytes
            .checked_add(line.html_bytes)
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        self.line_count = resources.checked_grid_add(self.line_count, 1)?;
        self.width = self.width.max(line.width);
        Ok(())
    }

    const fn document_cells(self) -> usize {
        self.document_cells
    }

    const fn grid_cells(self) -> usize {
        self.grid_cells
    }

    const fn encoded_bytes(self, html: bool) -> usize {
        if html {
            self.html_bytes
        } else {
            self.plain_bytes
        }
    }

    const fn line_count(self) -> usize {
        self.line_count
    }

    const fn width(self) -> usize {
        self.width
    }
}

impl<'a> RelationGraphLabelPlan<'a> {
    pub(crate) fn try_new(
        raw: &'a str,
        width_profile: TerminalWidthProfile,
        deferred: &DeferredTextRegistry<'a>,
        resources: &ResourceContext,
    ) -> Result<Option<Self>> {
        Self::try_new_with_presence(raw, width_profile, deferred, false, resources)
    }

    pub(crate) fn try_new_present(
        raw: &'a str,
        width_profile: TerminalWidthProfile,
        deferred: &DeferredTextRegistry<'a>,
        resources: &ResourceContext,
    ) -> Result<Self> {
        Self::try_new_with_disclosure(
            raw,
            "authored(bytes=",
            true,
            width_profile,
            deferred,
            resources,
        )?
        .ok_or_else(layout_allocation_failed)
    }

    fn try_new_with_presence(
        raw: &'a str,
        width_profile: TerminalWidthProfile,
        deferred: &DeferredTextRegistry<'a>,
        preserve_empty_authored: bool,
        resources: &ResourceContext,
    ) -> Result<Option<Self>> {
        Self::try_new_with_disclosure(
            raw,
            "authored(bytes=",
            preserve_empty_authored,
            width_profile,
            deferred,
            resources,
        )
    }

    fn try_new_with_disclosure(
        raw: &'a str,
        disclosure_prefix: &'static str,
        preserve_empty_authored: bool,
        width_profile: TerminalWidthProfile,
        deferred: &DeferredTextRegistry<'a>,
        resources: &ResourceContext,
    ) -> Result<Option<Self>> {
        resources.transaction(|resources| {
            let normalized =
                try_plan_normalized_label_lines(raw, width_profile, true, None, resources)?;
            let disclose_authored = normalized
                .map(NormalizedLabelPlan::authored_projection_is_lossy)
                .unwrap_or(preserve_empty_authored || !raw.is_empty());
            if normalized.is_none() && !disclose_authored {
                return Ok(None);
            }
            let disclosure = disclose_authored
                .then(|| {
                    deferred.try_measure_framed_value(
                        disclosure_prefix,
                        raw,
                        width_profile,
                        resources,
                    )
                })
                .transpose()?;
            let metrics = if let Some(plan) = normalized {
                RelationGraphLabelMetrics::try_from_normalized(
                    raw,
                    plan,
                    plan.try_encoded_bytes(raw, true, resources)?,
                    disclosure,
                    resources,
                )?
            } else {
                let mut metrics = RelationGraphLabelMetrics::EMPTY;
                metrics.try_include_line(
                    DeferredTextLineMetrics {
                        width: 0,
                        plain_bytes: 0,
                        html_bytes: 0,
                    },
                    resources,
                )?;
                metrics.try_include_line(
                    disclosure.ok_or_else(layout_allocation_failed)?,
                    resources,
                )?;
                metrics.grid_cells =
                    resources.checked_grid_mul(metrics.width.max(1), metrics.line_count.max(1))?;
                metrics
            };
            resources.grid_extent(metrics.width().max(1), metrics.line_count().max(1))?;
            Ok(Some(Self {
                raw,
                normalized,
                disclosure_prefix,
                preserve_empty_authored,
                disclose_authored,
                metrics,
                width_profile,
            }))
        })
    }

    const fn metrics(self) -> RelationGraphLabelMetrics {
        self.metrics
    }

    pub(crate) fn materialize(
        self,
        deferred: &mut DeferredTextRegistry<'a>,
        resources: &ResourceContext,
    ) -> Result<RelationGraphLabel> {
        let lines = deferred.try_register_preplanned_label_lines(
            self.raw,
            self.normalized,
            self.disclose_authored.then_some(self.disclosure_prefix),
            self.preserve_empty_authored,
            self.width_profile,
            resources,
        )?;
        let label = RelationGraphLabel::try_from_lines(lines, self.width_profile, resources)?;
        if label.width() != self.metrics.width() || label.line_count() != self.metrics.line_count()
        {
            return Err(grid_overflow(resources));
        }
        Ok(label)
    }
}

pub(crate) struct RelationGraphLabelBatchPlan<'a> {
    labels: Vec<Option<RelationGraphLabelPlan<'a>>>,
    grid_cells: usize,
    document_cells: usize,
    plain_bytes: usize,
    html_bytes: usize,
}

impl<'a> RelationGraphLabelBatchPlan<'a> {
    pub(crate) fn try_new(
        labels: Vec<Option<RelationGraphLabelPlan<'a>>>,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let (grid_cells, document_cells, plain_bytes, html_bytes) =
            labels.iter().flatten().try_fold(
                (0usize, 0usize, 0usize, 0usize),
                |(grid_cells, document_cells, plain_bytes, html_bytes), label| {
                    let metrics = label.metrics();
                    Ok::<_, crate::AsciiError>((
                        grid_cells.max(metrics.grid_cells()),
                        document_cells
                            .checked_add(metrics.document_cells())
                            .ok_or_else(|| {
                                resources.overflow(AsciiResourceLimitId::MaxDocumentCells)
                            })?,
                        plain_bytes
                            .checked_add(metrics.encoded_bytes(false))
                            .ok_or_else(|| {
                                resources.overflow(AsciiResourceLimitId::MaxOutputBytes)
                            })?,
                        html_bytes
                            .checked_add(metrics.encoded_bytes(true))
                            .ok_or_else(|| {
                                resources.overflow(AsciiResourceLimitId::MaxOutputBytes)
                            })?,
                    ))
                },
            )?;
        Ok(Self {
            labels,
            grid_cells,
            document_cells,
            plain_bytes,
            html_bytes,
        })
    }

    pub(crate) fn materialize(
        self,
        html: bool,
        deferred: &mut DeferredTextRegistry<'a>,
        resources: &ResourceContext,
    ) -> Result<Vec<Option<RelationGraphLabel>>> {
        resources.transaction(|resources| {
            resources.check(AsciiResourceLimitId::MaxGridCells, self.grid_cells)?;
            resources.check_usage(0, self.document_cells)?;
            resources.check(
                AsciiResourceLimitId::MaxOutputBytes,
                if html {
                    self.html_bytes
                } else {
                    self.plain_bytes
                },
            )?;
            deferred.transaction(|deferred| {
                let mut labels = Vec::new();
                labels
                    .try_reserve_exact(self.labels.len())
                    .map_err(|_| layout_allocation_failed())?;
                for label in self.labels {
                    labels.push(
                        label
                            .map(|label| label.materialize(deferred, resources))
                            .transpose()?,
                    );
                }
                Ok(labels)
            })
        })
    }
}

impl RelationGraphLabel {
    pub(crate) fn try_from_lines(
        lines: Vec<DeferredTextLine>,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let width = lines
            .iter()
            .map(DeferredTextLine::width)
            .max()
            .unwrap_or_default();
        resources.grid_extent(width.max(1), lines.len().max(1))?;
        Ok(Self {
            lines: Rc::new(lines),
            width,
            width_profile,
        })
    }

    pub(crate) fn lines(&self) -> &[DeferredTextLine] {
        &self.lines
    }

    pub(crate) fn shared_lines(&self) -> Rc<Vec<DeferredTextLine>> {
        Rc::clone(&self.lines)
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

    #[cfg(test)]
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

    pub(crate) fn deferred_box_content(
        text: &DeferredTextLine,
        content_width: usize,
        padding: usize,
        style: RelationGraphBoxStyle,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let used_width = resources.checked_grid_add(padding, text.width())?;
        let trailing = content_width
            .checked_sub(used_width)
            .ok_or_else(|| grid_overflow(resources))?;

        let mut line = StyledLine::with_resources(width_profile, resources);
        line.try_push_role_char(style.vertical, style.border_role)?;
        line.try_push_spaces(padding)?;
        line.try_push_deferred_text(text, style.text_role)?;
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

    pub(super) fn styled(&self) -> &StyledLine {
        &self.line
    }

    pub(super) fn shared(&self) -> Self {
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

    #[cfg(test)]
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

    pub(crate) fn from_deferred_sections(
        id: String,
        sections: &[Vec<DeferredTextLine>],
        padding: usize,
        style: RelationGraphBoxStyle,
        width_profile: TerminalWidthProfile,
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        let max_line_width = sections
            .iter()
            .flat_map(|section| section.iter())
            .map(DeferredTextLine::width)
            .max()
            .unwrap_or(0)
            .max(1);
        let content_width =
            resources.checked_grid_add(max_line_width, resources.checked_grid_mul(padding, 2)?)?;
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
                lines.push(RelationGraphLine::deferred_box_content(
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

    pub(super) fn shared_projection(&self) -> Self {
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

#[cfg(test)]
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

fn line_char_width(line: &RelationGraphLine) -> usize {
    line.width()
}

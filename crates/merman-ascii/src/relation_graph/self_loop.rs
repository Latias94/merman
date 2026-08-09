use super::{
    RelationGraphBox, RelationGraphLine, grid_overflow, layout_allocation_failed,
    try_concat_relation_lines, try_share_relation_box_lines, work_overflow,
};
use crate::Result;
use crate::color::AsciiColorRole;
use crate::options::TerminalWidthProfile;
use crate::resource::{LogicalExtent, ResourceContext};

/// Geometry-only self-loop inputs supplied by a diagram family.
///
/// The shared renderer deliberately does not inspect markers or labels to
/// derive these values.  That keeps cardinality/marker meaning in Class and ER
/// while allowing the complete loop extent to be admitted before any styled
/// rows are allocated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RelationSelfLoopMetrics {
    pub(crate) top_marker_width: usize,
    pub(crate) max_label_width: usize,
    pub(crate) label_line_count: usize,
    pub(crate) bottom_marker_width: usize,
    pub(crate) tail_prefix_width: usize,
    pub(crate) has_tail_prefix: bool,
    pub(crate) horizontal: char,
    pub(crate) vertical: char,
}

impl RelationSelfLoopMetrics {
    pub(crate) const fn new(
        top_marker_width: usize,
        max_label_width: usize,
        label_line_count: usize,
        bottom_marker_width: usize,
        tail_prefix_width: Option<usize>,
        horizontal: char,
        vertical: char,
    ) -> Self {
        let (tail_prefix_width, has_tail_prefix) = match tail_prefix_width {
            Some(width) => (width, true),
            None => (0, false),
        };
        Self {
            top_marker_width,
            max_label_width,
            label_line_count,
            bottom_marker_width,
            tail_prefix_width,
            has_tail_prefix,
            horizontal,
            vertical,
        }
    }
}

/// Admission plan for a group of parallel self-loops.
///
/// `metrics` contains no `StyledLine`.  The materializer closure is invoked
/// only after the aggregate extent has passed the grid limit; the resulting
/// rows are then checked against the exact metrics before rendering.
#[derive(Debug)]
pub(crate) struct RelationSelfLoopPlan<'a> {
    relation_box: &'a RelationGraphBox,
    metrics: Vec<RelationSelfLoopMetrics>,
    geometry: SelfLoopGeometry,
    extent: LogicalExtent,
}

impl<'a> RelationSelfLoopPlan<'a> {
    pub(crate) fn try_new(
        relation_box: &'a RelationGraphBox,
        metrics: Vec<RelationSelfLoopMetrics>,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let geometry = if metrics.is_empty() {
            SelfLoopGeometry {
                bottom_start: relation_box.width() / 2,
                loop_col: relation_box.width(),
            }
        } else {
            SelfLoopGeometry::for_metrics(relation_box, &metrics, resources)?
        };
        let extent = if metrics.is_empty() {
            resources.grid_extent(relation_box.width(), relation_box.height())?
        } else {
            let height =
                metrics
                    .iter()
                    .enumerate()
                    .try_fold(0usize, |height, (index, metric)| {
                        let label_row_count = if index == 0 {
                            metric.label_line_count
                        } else {
                            metric
                                .label_line_count
                                .max(usize::from(metric.has_tail_prefix))
                        };
                        let loop_height = if index == 0 {
                            resources
                                .checked_grid_add(
                                    relation_box.height(),
                                    resources.checked_grid_add(label_row_count, 1)?,
                                )?
                                .max(3)
                        } else {
                            resources.checked_grid_add(label_row_count, 1)?
                        };
                        resources.checked_grid_add(height, loop_height)
                    })?;
            // Only the first loop emits its top marker.  Tail top markers are
            // retained in the descriptor for semantic validation but are not
            // present in the final rows.
            let first_top_marker_width = metrics[0].top_marker_width.max(1);
            let width = resources.checked_grid_add(geometry.loop_col, first_top_marker_width)?;
            resources.grid_extent(width, height)?
        };
        Ok(Self {
            relation_box,
            metrics,
            geometry,
            extent,
        })
    }

    pub(crate) const fn extent(&self) -> LogicalExtent {
        self.extent
    }

    pub(crate) fn render_lines(
        self,
        resources: &mut ResourceContext,
        materialize_rows: impl FnOnce(&ResourceContext) -> Result<Vec<RelationSelfLoopRows>>,
    ) -> Result<Vec<RelationGraphLine>> {
        resources.charge_layout_work(self.extent.cells())?;
        if self.metrics.is_empty() {
            return try_share_relation_box_lines(self.relation_box);
        }

        let loops = materialize_rows(resources)?;
        self.validate_rows(&loops, resources)?;
        let lines =
            render_parallel_self_loops(self.relation_box, loops, &self.geometry, resources)?;
        let actual_width = lines
            .iter()
            .map(RelationGraphLine::width)
            .max()
            .unwrap_or(0);
        let actual = resources.grid_extent(actual_width, lines.len())?;
        if actual != self.extent {
            return Err(grid_overflow(resources));
        }
        Ok(lines)
    }

    fn validate_rows(
        &self,
        loops: &[RelationSelfLoopRows],
        resources: &ResourceContext,
    ) -> Result<()> {
        if loops.len() != self.metrics.len() {
            return Err(grid_overflow(resources));
        }
        for (rows, metric) in loops.iter().zip(&self.metrics) {
            let max_label_width = rows
                .label_lines
                .iter()
                .map(RelationGraphLine::width)
                .max()
                .unwrap_or(0);
            let tail_prefix_width = rows
                .tail_prefix
                .as_ref()
                .map(RelationGraphLine::width)
                .unwrap_or(0);
            let exact = rows.top_marker.width() == metric.top_marker_width
                && max_label_width == metric.max_label_width
                && rows.label_lines.len() == metric.label_line_count
                && rows.bottom_marker.width() == metric.bottom_marker_width
                && tail_prefix_width == metric.tail_prefix_width
                && rows.tail_prefix.is_some() == metric.has_tail_prefix
                && rows.horizontal == metric.horizontal
                && rows.vertical == metric.vertical;
            let profile = self.relation_box.width_profile();
            let profiles_match = rows.top_marker.width_profile() == profile
                && rows
                    .label_lines
                    .iter()
                    .all(|line| line.width_profile() == profile)
                && rows.bottom_marker.width_profile() == profile
                && rows
                    .tail_prefix
                    .as_ref()
                    .is_none_or(|line| line.width_profile() == profile);
            if !exact || !profiles_match {
                return Err(grid_overflow(resources));
            }
        }
        Ok(())
    }
}

fn render_parallel_self_loops(
    relation_box: &RelationGraphBox,
    loops: Vec<RelationSelfLoopRows>,
    geometry: &SelfLoopGeometry,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let mut loop_iter = loops.into_iter();
    let Some(first_loop) = loop_iter.next() else {
        return try_share_relation_box_lines(relation_box);
    };
    let mut lines = first_self_loop_lines(relation_box, first_loop, geometry, resources)?;
    for loop_rows in loop_iter {
        lines.extend(tail_self_loop_lines(
            relation_box,
            loop_rows,
            geometry,
            resources,
        )?);
    }
    Ok(lines)
}

pub(crate) struct RelationSelfLoopRows {
    top_marker: RelationGraphLine,
    pub(super) label_lines: Vec<RelationGraphLine>,
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

#[derive(Debug)]
struct SelfLoopGeometry {
    bottom_start: usize,
    loop_col: usize,
}

impl SelfLoopGeometry {
    fn for_metrics(
        relation_box: &RelationGraphBox,
        metrics: &[RelationSelfLoopMetrics],
        resources: &ResourceContext,
    ) -> Result<Self> {
        let bottom_start = relation_box.width() / 2;
        let mut loop_col = resources.checked_grid_add(relation_box.width(), 3)?;
        for (loop_index, metric) in metrics.iter().enumerate() {
            let prefix_width = if loop_index > 0 && metric.has_tail_prefix {
                Some(metric.tail_prefix_width)
            } else {
                None
            };
            let label_start = self_loop_label_start(
                relation_box.width(),
                metric.max_label_width,
                prefix_width,
                resources,
            )?;
            let label_end = resources.checked_grid_add(
                resources.checked_grid_add(label_start, metric.max_label_width)?,
                2,
            )?;
            let marker_end = resources.checked_grid_add(
                resources.checked_grid_add(bottom_start, metric.bottom_marker_width)?,
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

fn self_loop_label_start(
    relation_box_width: usize,
    label_width: usize,
    prefix_width: Option<usize>,
    resources: &ResourceContext,
) -> Result<usize> {
    let centered_start = if label_width >= relation_box_width {
        1
    } else {
        resources.checked_grid_add((relation_box_width - label_width) / 2, 1)?
    };
    let prefix_start = match prefix_width {
        Some(width) => resources.checked_grid_add(width, 1)?,
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
    let label_row_count = label_lines.len().max(usize::from(tail_prefix.is_some()));
    let capacity = label_row_count
        .checked_add(1)
        .ok_or_else(|| work_overflow(resources))?;
    lines
        .try_reserve_exact(capacity)
        .map_err(|_| layout_allocation_failed())?;
    if label_lines.is_empty() {
        if let Some(prefix) = tail_prefix {
            lines.push(self_loop_label_line(
                relation_box,
                Some(prefix),
                RelationGraphLine::try_plain("", relation_box.width_profile(), resources)?,
                vertical,
                geometry,
                resources,
            )?);
        }
    } else {
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

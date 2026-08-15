use crate::color::AsciiColorRole;
use crate::operation::AsciiExecution;
use crate::options::TerminalWidthProfile;
use crate::resource::{
    AsciiResourceLimitId, AsciiResourceLimitPhase, LogicalExtent, ResourceContext,
};
use crate::safe_text::{
    charge_text_layout, terminal_text_requires_normalization, visit_safe_line_graphemes,
};
use crate::text::StyledLine;
#[cfg(test)]
use crate::text::display_width_with_profile;
use crate::{AsciiCharset, AsciiError, AsciiRenderOptions, Result};
use merman_core::OperationPhase;
use merman_core::diagrams::xychart::{
    XyChartAxisRenderModel, XyChartDiagramRenderModel, XyChartPlotRenderModel, XyChartPlotType,
};

const BAND_GAP: usize = 1;
const BAND_GAP_LABEL: &str = " ";
const LINE_NORTH: u8 = 1 << 0;
const LINE_EAST: u8 = 1 << 1;
const LINE_SOUTH: u8 = 1 << 2;
const LINE_WEST: u8 = 1 << 3;
const LINE_POINT: u8 = 1 << 4;
const LINE_HORIZONTAL_MASK: u8 = LINE_EAST | LINE_WEST;
const LINE_VERTICAL_MASK: u8 = LINE_NORTH | LINE_SOUTH;
const LINE_TOP_LEFT_MASK: u8 = LINE_EAST | LINE_SOUTH;
const LINE_TOP_RIGHT_MASK: u8 = LINE_WEST | LINE_SOUTH;
const LINE_BOTTOM_LEFT_MASK: u8 = LINE_EAST | LINE_NORTH;
const LINE_BOTTOM_RIGHT_MASK: u8 = LINE_WEST | LINE_NORTH;
const LINE_TEE_DOWN_MASK: u8 = LINE_EAST | LINE_WEST | LINE_SOUTH;
const LINE_TEE_UP_MASK: u8 = LINE_EAST | LINE_WEST | LINE_NORTH;
const LINE_TEE_RIGHT_MASK: u8 = LINE_NORTH | LINE_SOUTH | LINE_EAST;
const LINE_TEE_LEFT_MASK: u8 = LINE_NORTH | LINE_SOUTH | LINE_WEST;
const LINE_CROSS_MASK: u8 = LINE_NORTH | LINE_EAST | LINE_SOUTH | LINE_WEST;

/// Monotonic cooperative checkpoint state shared by all XYChart paint loops.
///
/// Resource charges still perform their own cancellation checks. This cursor covers the loops
/// that mutate already-admitted terminal cells, where a single up-front charge would otherwise
/// leave a large cancellation window.
#[derive(Debug, Clone, Copy)]
pub(super) struct XyChartCheckpointCursor<'a> {
    execution: AsciiExecution<'a>,
    iteration: usize,
}

impl<'a> XyChartCheckpointCursor<'a> {
    pub(super) const fn new(execution: AsciiExecution<'a>) -> Self {
        Self {
            execution,
            iteration: 0,
        }
    }

    pub(super) fn tick(&mut self, phase: OperationPhase) -> Result<()> {
        let iteration = self.iteration;
        self.iteration = self.iteration.wrapping_add(1);
        self.execution.checkpoint_loop(phase, iteration)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct LineTopologyCell(u8);

impl LineTopologyCell {
    fn add(&mut self, mask: u8) {
        self.0 |= mask;
    }

    fn connections(self) -> u8 {
        self.0 & !LINE_POINT
    }

    fn is_point(self) -> bool {
        self.0 & LINE_POINT != 0
    }

    fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct XyChartPlotArea {
    pub(super) vertical_height: usize,
    pub(super) category_band_width: usize,
    pub(super) horizontal_width: usize,
    pub(super) width_profile: TerminalWidthProfile,
}

impl XyChartPlotArea {
    pub(super) fn from_options(options: &AsciiRenderOptions) -> Self {
        Self {
            vertical_height: options.xychart_vertical_plot_height,
            category_band_width: options.xychart_category_band_width,
            horizontal_width: options.xychart_horizontal_plot_width,
            width_profile: options.terminal_width_profile,
        }
    }

    pub(super) fn vertical_plot_width(
        self,
        category_count: usize,
        resources: &ResourceContext,
    ) -> Result<usize> {
        if category_count == 0 {
            Ok(0)
        } else {
            let band_cells =
                resources.checked_grid_mul(category_count, self.category_band_width)?;
            let gaps = checked_grid_sub(resources, category_count, 1)?;
            let gap_cells = resources.checked_grid_mul(gaps, BAND_GAP)?;
            resources.checked_grid_add(band_cells, gap_cells)
        }
    }

    pub(super) fn vertical_plot_extent(
        self,
        category_count: usize,
        resources: &ResourceContext,
    ) -> Result<LogicalExtent> {
        let width = self.vertical_plot_width(category_count, resources)?;
        resources.grid_extent(width, self.vertical_height)
    }

    pub(super) fn horizontal_plot_extent(
        self,
        category_count: usize,
        resources: &ResourceContext,
    ) -> Result<LogicalExtent> {
        resources.grid_extent(self.horizontal_width, category_count)
    }

    pub(super) fn vertical_band_start(
        self,
        idx: usize,
        resources: &ResourceContext,
    ) -> Result<usize> {
        let stride = resources.checked_grid_add(self.category_band_width, BAND_GAP)?;
        resources.checked_grid_mul(idx, stride)
    }

    pub(super) fn vertical_band_center(
        self,
        idx: usize,
        resources: &ResourceContext,
    ) -> Result<usize> {
        let start = self.vertical_band_start(idx, resources)?;
        resources.checked_grid_add(start, self.category_band_width / 2)
    }

    pub(super) fn band_labels<T: AsRef<str>>(
        self,
        labels: &[T],
        resources: &mut ResourceContext,
        checkpoints: &mut XyChartCheckpointCursor<'_>,
    ) -> Result<String> {
        let width = self.vertical_plot_width(labels.len(), resources)?;
        let mut rendered = String::new();
        rendered
            .try_reserve(width)
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
            })?;
        for (index, label) in labels.iter().enumerate() {
            checkpoints.tick(OperationPhase::Emit)?;
            resources.charge_layout_work(self.category_band_width)?;
            if index > 0 {
                rendered.push_str(BAND_GAP_LABEL);
            }
            rendered.push_str(&fit_centered(
                label.as_ref(),
                self.category_band_width,
                self.width_profile,
                resources,
            )?);
        }
        Ok(rendered)
    }

    pub(super) fn category_axis_labels<T: AsRef<str>>(
        self,
        categories: &[T],
        resources: &mut ResourceContext,
        checkpoints: &mut XyChartCheckpointCursor<'_>,
    ) -> Result<String> {
        self.band_labels(categories, resources, checkpoints)
    }

    pub(super) fn band_labels_retained_width<T: AsRef<str>>(
        self,
        labels: &[T],
        resources: &mut ResourceContext,
    ) -> Result<usize> {
        let stride = resources.checked_grid_add(self.category_band_width, BAND_GAP)?;
        let mut retained_width = 0usize;
        for (index, label) in labels.iter().enumerate() {
            let metrics = visit_fitted_safe_line(
                label.as_ref(),
                self.category_band_width,
                self.width_profile,
                resources,
                |_| Ok(()),
            )?;
            let remaining = checked_grid_sub(
                resources,
                self.category_band_width,
                metrics.materialized_width,
            )?;
            let left_padding = remaining / 2;
            if metrics.retained_width == 0 {
                continue;
            }
            let band_start = resources.checked_grid_mul(index, stride)?;
            let text_start = resources.checked_grid_add(band_start, left_padding)?;
            retained_width =
                retained_width.max(resources.checked_grid_add(text_start, metrics.retained_width)?);
        }
        Ok(retained_width)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ChartChars {
    pub(super) horizontal_axis: char,
    pub(super) vertical_axis: char,
    pub(super) origin: char,
    pub(super) horizontal_tick: char,
    pub(super) vertical_tick: char,
    bar: char,
    line_horizontal: char,
    line_vertical: char,
    line_point: char,
    line_top_left: char,
    line_top_right: char,
    line_bottom_left: char,
    line_bottom_right: char,
    line_tee_down: char,
    line_tee_up: char,
    line_tee_right: char,
    line_tee_left: char,
    line_cross: char,
}

impl ChartChars {
    pub(super) fn from_options(options: &AsciiRenderOptions) -> Self {
        match options.structural_charset() {
            AsciiCharset::Ascii => Self {
                horizontal_axis: '-',
                vertical_axis: '|',
                origin: '+',
                horizontal_tick: '+',
                vertical_tick: '+',
                bar: '#',
                line_horizontal: '-',
                line_vertical: '|',
                line_point: '*',
                line_top_left: '+',
                line_top_right: '+',
                line_bottom_left: '+',
                line_bottom_right: '+',
                line_tee_down: '+',
                line_tee_up: '+',
                line_tee_right: '+',
                line_tee_left: '+',
                line_cross: '+',
            },
            AsciiCharset::Unicode => Self {
                horizontal_axis: '─',
                vertical_axis: '│',
                origin: '┼',
                horizontal_tick: '┬',
                vertical_tick: '┤',
                bar: '█',
                line_horizontal: '─',
                line_vertical: '│',
                line_point: '●',
                line_top_left: '╭',
                line_top_right: '╮',
                line_bottom_left: '╰',
                line_bottom_right: '╯',
                line_tee_down: '┬',
                line_tee_up: '┴',
                line_tee_right: '├',
                line_tee_left: '┤',
                line_cross: '┼',
            },
        }
    }

    pub(super) fn legend_symbol(self, plot_type: XyChartPlotType) -> char {
        match plot_type {
            XyChartPlotType::Bar => self.bar,
            XyChartPlotType::Line => self.line_point,
        }
    }

    fn line_glyph(self, cell: LineTopologyCell) -> char {
        if cell.is_point() {
            return self.line_point;
        }
        match cell.connections() {
            LINE_HORIZONTAL_MASK => self.line_horizontal,
            LINE_VERTICAL_MASK => self.line_vertical,
            LINE_TOP_LEFT_MASK => self.line_top_left,
            LINE_TOP_RIGHT_MASK => self.line_top_right,
            LINE_BOTTOM_LEFT_MASK => self.line_bottom_left,
            LINE_BOTTOM_RIGHT_MASK => self.line_bottom_right,
            LINE_TEE_DOWN_MASK => self.line_tee_down,
            LINE_TEE_UP_MASK => self.line_tee_up,
            LINE_TEE_RIGHT_MASK => self.line_tee_right,
            LINE_TEE_LEFT_MASK => self.line_tee_left,
            LINE_CROSS_MASK => self.line_cross,
            mask if mask & (LINE_EAST | LINE_WEST) != 0 => self.line_horizontal,
            _ => self.line_vertical,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ValueRange {
    pub(super) min: f64,
    pub(super) max: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LinearDomainPlan {
    pub(super) authored_min: Option<f64>,
    pub(super) authored_max: Option<f64>,
    pub(super) resolved: ValueRange,
}

impl LinearDomainPlan {
    fn authored_range_matches_resolved(self) -> bool {
        self.authored_min == Some(self.resolved.min) && self.authored_max == Some(self.resolved.max)
    }
}

impl ValueRange {
    pub(super) fn span(self) -> f64 {
        self.max - self.min
    }

    fn normalized(self, value: f64) -> f64 {
        if self.span() == 0.0 {
            return 0.5;
        }

        ((value - self.min) / self.span()).clamp(0.0, 1.0)
    }

    pub(super) fn contains(self, value: f64) -> bool {
        let lower = self.min.min(self.max);
        let upper = self.min.max(self.max);
        value >= lower && value <= upper
    }
}

#[derive(Debug, Clone)]
pub(super) enum AxisPlan {
    Band { authored_category_count: usize },
    Linear { range: ValueRange },
}

impl AxisPlan {
    fn resolve_sample_position(
        &self,
        band_labels: &[String],
        x: &str,
        fallback_index: usize,
        slot_count: usize,
        resources: &mut ResourceContext,
    ) -> Result<(usize, Option<f64>, bool)> {
        if slot_count <= 1 {
            let (normalized_x, clipped) = match self {
                Self::Band {
                    authored_category_count,
                } => {
                    let categories = band_labels
                        .get(..*authored_category_count)
                        .ok_or_else(|| resources.grid_overflow())?;
                    (
                        None,
                        !categories.is_empty()
                            && categories
                                .get(fallback_index)
                                .is_none_or(|category| category != x),
                    )
                }
                Self::Linear { range } => match x.parse::<f64>() {
                    Ok(value) if value.is_finite() => {
                        (Some(range.normalized(value)), !range.contains(value))
                    }
                    _ => (Some(0.0), !x.trim().is_empty()),
                },
            };
            return Ok((0, normalized_x, clipped));
        }

        match self {
            Self::Band {
                authored_category_count,
            } => {
                let categories = band_labels
                    .get(..*authored_category_count)
                    .ok_or_else(|| resources.grid_overflow())?;
                if categories
                    .get(fallback_index)
                    .is_some_and(|category| category == x)
                {
                    return Ok((fallback_index.min(slot_count - 1), None, false));
                }
                for (index, category) in categories.iter().enumerate() {
                    resources.charge_layout_work(1)?;
                    if category == x {
                        return Ok((index.min(slot_count - 1), None, false));
                    }
                }
                Ok((
                    fallback_index.min(slot_count - 1),
                    None,
                    !categories.is_empty(),
                ))
            }
            Self::Linear { range } => match x.parse::<f64>() {
                Ok(value) if value.is_finite() => {
                    let normalized = range.normalized(value);
                    Ok((
                        (normalized * (slot_count - 1) as f64).round() as usize,
                        Some(normalized),
                        !range.contains(value),
                    ))
                }
                _ => Ok((
                    fallback_index.min(slot_count - 1),
                    Some(fallback_index.min(slot_count - 1) as f64 / (slot_count - 1) as f64),
                    !x.trim().is_empty(),
                )),
            },
        }
    }

    fn sample_column(
        &self,
        datum: &SeriesDatum,
        slot_count: usize,
        plot_width: usize,
        plot_area: XyChartPlotArea,
        resources: &ResourceContext,
    ) -> Result<usize> {
        if plot_width <= 1 {
            return Ok(0);
        }

        match self {
            Self::Band { .. } => plot_area.vertical_band_center(datum.slot, resources),
            Self::Linear { .. } => {
                let first = plot_area
                    .vertical_band_center(0, resources)?
                    .min(plot_width - 1);
                if slot_count <= 1 {
                    return Ok(first);
                }
                let last = plot_area
                    .vertical_band_center(slot_count - 1, resources)?
                    .min(plot_width - 1);
                let span = checked_grid_sub(resources, last, first)?;
                let normalized = datum.normalized_x.unwrap_or(0.0);
                resources.checked_grid_add(first, (normalized * span as f64).round() as usize)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct SeriesDatum {
    pub(super) x: String,
    pub(super) value: Option<f64>,
    pub(super) has_point_label: bool,
    authored_x: bool,
    slot: usize,
    normalized_x: Option<f64>,
    pub(super) x_clipped: bool,
}

#[derive(Debug, Clone)]
pub(super) struct SeriesPlan {
    pub(super) series_index: usize,
    pub(super) plot_type: XyChartPlotType,
    pub(super) title: Option<String>,
    pub(super) data: Vec<SeriesDatum>,
    pub(super) has_orphan_point_labels: bool,
    pub(super) bar_lane: Option<usize>,
}

#[derive(Debug, Clone)]
pub(super) struct TerminalChartPlan {
    pub(super) x_axis: AxisPlan,
    pub(super) x_linear_domain: Option<LinearDomainPlan>,
    pub(super) y_range: ValueRange,
    pub(super) y_linear_domain: LinearDomainPlan,
    pub(super) series: Vec<SeriesPlan>,
    pub(super) category_labels: Vec<String>,
    pub(super) horizontal_axis_labels: Vec<String>,
    pub(super) slot_count: usize,
    pub(super) bar_series_count: usize,
    pub(super) line_series_count: usize,
    has_authored_x: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct TerminalDisclosurePlan {
    pub(super) values: bool,
    pub(super) band_domain: bool,
    pub(super) x_linear_domain: bool,
    pub(super) y_linear_domain: bool,
}

impl TerminalDisclosurePlan {
    pub(super) const fn is_required(self) -> bool {
        self.values || self.band_domain || self.x_linear_domain || self.y_linear_domain
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TerminalChartCardinality {
    max_data_count: usize,
    slot_count: usize,
}

impl TerminalChartCardinality {
    pub(super) const fn is_empty(self) -> bool {
        self.slot_count == 0
    }
}

impl TerminalChartPlan {
    pub(super) fn measure_cardinality(
        model: &XyChartDiagramRenderModel,
        resources: &mut ResourceContext,
    ) -> Result<TerminalChartCardinality> {
        let mut max_data_count = 0;
        for plot in &model.plots {
            resources.charge_layout_work(1)?;
            max_data_count = max_data_count.max(effective_sample_count(plot));
        }
        let category_count = match &model.x_axis {
            XyChartAxisRenderModel::Band { categories, .. } => categories.len(),
            XyChartAxisRenderModel::Linear { .. } => 0,
        };
        Ok(TerminalChartCardinality {
            max_data_count,
            slot_count: category_count.max(max_data_count),
        })
    }

    pub(super) fn build(
        model: &XyChartDiagramRenderModel,
        cardinality: TerminalChartCardinality,
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        let x_axis = build_axis_plan(
            &model.x_axis,
            cardinality.max_data_count,
            &model.plots,
            resources,
        )?;
        let x_linear_domain = match (&model.x_axis, &x_axis) {
            (XyChartAxisRenderModel::Linear { min, max, .. }, AxisPlan::Linear { range }) => {
                Some(LinearDomainPlan {
                    authored_min: *min,
                    authored_max: *max,
                    resolved: *range,
                })
            }
            _ => None,
        };
        let slot_count = cardinality.slot_count;
        let authored_band_categories: &[String] = match &model.x_axis {
            XyChartAxisRenderModel::Band { categories, .. } => categories,
            XyChartAxisRenderModel::Linear { .. } => &[],
        };
        let category_labels =
            axis_labels(&x_axis, authored_band_categories, slot_count, resources)?;

        let mut series = Vec::new();
        series
            .try_reserve_exact(model.plots.len())
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
            })?;
        let mut bar_lane = 0;
        let mut line_series_count = 0;
        let mut has_authored_x = false;
        for (series_index, plot) in model.plots.iter().enumerate() {
            resources.charge_layout_work(1)?;
            has_authored_x |= !plot.data.is_empty();
            let lane = if plot.plot_type == XyChartPlotType::Bar {
                let lane = bar_lane;
                bar_lane = resources.checked_grid_add(bar_lane, 1)?;
                Some(lane)
            } else {
                line_series_count = resources.checked_grid_add(line_series_count, 1)?;
                None
            };
            series.push(build_series_plan(
                plot,
                series_index,
                lane,
                &x_axis,
                &category_labels,
                slot_count,
                resources,
            )?);
        }
        let horizontal_axis_labels = build_horizontal_axis_labels(
            &x_axis,
            &category_labels,
            slot_count,
            &series,
            resources,
        )?;

        let y_range = build_value_range(&model.y_axis, &series, resources)?;
        let XyChartAxisRenderModel::Linear {
            min: y_min,
            max: y_max,
            ..
        } = &model.y_axis
        else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "xychart",
                feature: "band y-axis",
            });
        };
        Ok(Self {
            x_axis,
            x_linear_domain,
            y_range,
            y_linear_domain: LinearDomainPlan {
                authored_min: *y_min,
                authored_max: *y_max,
                resolved: y_range,
            },
            series,
            category_labels,
            horizontal_axis_labels,
            slot_count,
            bar_series_count: bar_lane,
            line_series_count,
            has_authored_x,
        })
    }

    pub(super) fn horizontal_rows_per_slot(&self) -> usize {
        if self.bar_series_count <= 1 {
            1
        } else {
            self.bar_series_count + usize::from(self.line_series_count > 0)
        }
    }

    pub(super) fn horizontal_row_count(&self, resources: &ResourceContext) -> Result<usize> {
        resources.checked_grid_mul(self.slot_count, self.horizontal_rows_per_slot())
    }

    pub(super) fn sample_slot(&self, datum: &SeriesDatum) -> usize {
        datum.slot
    }

    pub(super) fn sample_vertical_column(
        &self,
        datum: &SeriesDatum,
        plot_width: usize,
        plot_area: XyChartPlotArea,
        resources: &ResourceContext,
    ) -> Result<usize> {
        self.x_axis
            .sample_column(datum, self.slot_count, plot_width, plot_area, resources)
    }

    pub(super) fn horizontal_anchor_row(
        &self,
        slot: usize,
        resources: &ResourceContext,
    ) -> Result<usize> {
        let start = resources.checked_grid_mul(slot, self.horizontal_rows_per_slot())?;
        if self.bar_series_count > 1 && self.line_series_count > 0 {
            resources.checked_grid_add(start, self.bar_series_count)
        } else {
            Ok(start)
        }
    }

    pub(super) fn disclosure_plan(
        &self,
        plot_area: XyChartPlotArea,
        horizontal: bool,
        show_x_axis_labels: bool,
        show_y_axis_labels: bool,
        resources: &mut ResourceContext,
    ) -> Result<TerminalDisclosurePlan> {
        resources.charge_layout_work(1)?;
        let grouped_bars_do_not_fit =
            !horizontal && self.bar_series_count > plot_area.category_band_width;
        let values_are_ambiguous = self.series.len() > 1 || grouped_bars_do_not_fit;

        let axis_labels = if horizontal {
            &self.horizontal_axis_labels
        } else {
            &self.category_labels
        };
        let authored_band_categories = match &self.x_axis {
            AxisPlan::Band {
                authored_category_count,
            } => Some(
                self.category_labels
                    .get(..*authored_category_count)
                    .ok_or_else(|| resources.grid_overflow())?,
            ),
            AxisPlan::Linear { .. } => None,
        };
        let has_band_domain = authored_band_categories.is_some();
        let implicit_band_domain =
            authored_band_categories.is_some_and(|categories| categories.is_empty());
        let has_linear_x_domain = matches!(&self.x_axis, AxisPlan::Linear { .. });
        let x_linear_domain = has_linear_x_domain;
        let y_linear_domain = !show_y_axis_labels
            || !self.linear_y_domain_is_visible_exact(plot_area, horizontal, resources)?;
        let disclosure = |values, band_domain| TerminalDisclosurePlan {
            values,
            band_domain,
            x_linear_domain,
            y_linear_domain,
        };
        if has_linear_x_domain && (values_are_ambiguous || self.has_authored_x) {
            return Ok(disclosure(true, false));
        }
        for (index, category) in axis_labels.iter().enumerate() {
            resources.charge_layout_work(1)?;
            let category_loses_geometry = if horizontal {
                false
            } else {
                visit_fitted_safe_line(
                    category,
                    plot_area.category_band_width,
                    plot_area.width_profile,
                    resources,
                    |_| Ok(()),
                )?
                .truncated
            };
            if category_loses_geometry {
                return Ok(disclosure(true, has_band_domain));
            }
            let category_changes_during_normalization =
                has_band_domain && terminal_text_requires_normalization(category, resources)?;
            if category_changes_during_normalization {
                return Ok(disclosure(true, has_band_domain));
            }
            for previous in &axis_labels[..index] {
                resources.charge_layout_work(1)?;
                if category == previous {
                    return Ok(disclosure(true, has_band_domain));
                }
            }
        }

        // Band labels are projected into renderer-owned fields: fixed-width centering for a
        // vertical chart and gutter-aligned padding for a horizontal chart. A changed projection
        // can be authored directly by another model, and distinct categories can project to the
        // same field. Keep the complete framed domain whenever that channel is not injective.
        let band_domain_disclosure = match authored_band_categories {
            Some(_) if !show_x_axis_labels => true,
            Some(categories) => band_label_projection_loses_identity(
                categories,
                axis_labels,
                plot_area,
                horizontal,
                resources,
            )?,
            None => false,
        };

        if values_are_ambiguous {
            return Ok(disclosure(true, band_domain_disclosure));
        }

        for series in &self.series {
            resources.charge_layout_work(1)?;
            if series.data.is_empty() && !series.has_orphan_point_labels {
                return Ok(disclosure(true, band_domain_disclosure));
            }
            if series.title.is_some() {
                return Ok(disclosure(true, band_domain_disclosure));
            }
            if series.has_orphan_point_labels {
                return Ok(disclosure(true, band_domain_disclosure));
            }
            for (index, datum) in series.data.iter().enumerate() {
                resources.charge_layout_work(1)?;
                if ((implicit_band_domain || has_linear_x_domain) && datum.authored_x)
                    || datum.value.is_none()
                    || datum.has_point_label
                    || datum
                        .value
                        .is_some_and(|value| !self.y_range.contains(value))
                    || datum.x_clipped
                    || self.datum_projection_loses_identity(
                        series, datum, plot_area, horizontal, resources,
                    )?
                {
                    return Ok(disclosure(true, band_domain_disclosure));
                }

                let projected_point = if series.plot_type == XyChartPlotType::Line {
                    self.projected_point(datum, plot_area, horizontal, resources)?
                } else {
                    None
                };
                for previous in &series.data[..index] {
                    resources.charge_layout_work(1)?;
                    if horizontal
                        && datum.value.is_some()
                        && previous.value.is_some()
                        && self.sample_slot(previous) == self.sample_slot(datum)
                    {
                        return Ok(disclosure(true, band_domain_disclosure));
                    }
                    if projected_point.is_some()
                        && projected_point
                            == self.projected_point(previous, plot_area, horizontal, resources)?
                    {
                        return Ok(disclosure(true, band_domain_disclosure));
                    }
                    if !horizontal
                        && series.plot_type == XyChartPlotType::Bar
                        && self
                            .vertical_bars_overlap(series, datum, previous, plot_area, resources)?
                    {
                        return Ok(disclosure(true, band_domain_disclosure));
                    }
                }
            }
        }
        Ok(disclosure(false, band_domain_disclosure))
    }

    fn linear_y_domain_is_visible_exact(
        &self,
        plot_area: XyChartPlotArea,
        horizontal: bool,
        resources: &mut ResourceContext,
    ) -> Result<bool> {
        let domain = self.y_linear_domain;
        if !domain.authored_range_matches_resolved() {
            return Ok(false);
        }
        let step = if horizontal {
            domain.resolved.span()
        } else {
            domain.resolved.span() / plot_area.vertical_height as f64
        };
        let displayed_min = format_tick_number(domain.resolved.min, step);
        let displayed_max = format_tick_number(domain.resolved.max, step);
        if displayed_min != format_data_number(domain.resolved.min)
            || displayed_max != format_data_number(domain.resolved.max)
        {
            return Ok(false);
        }
        if !horizontal {
            return Ok(true);
        }
        let min_metrics = visit_fitted_safe_line(
            &displayed_min,
            plot_area.horizontal_width,
            plot_area.width_profile,
            resources,
            |_| Ok(()),
        )?;
        let max_metrics = visit_fitted_safe_line(
            &displayed_max,
            plot_area.horizontal_width,
            plot_area.width_profile,
            resources,
            |_| Ok(()),
        )?;
        let max_start = checked_grid_sub(
            resources,
            plot_area.horizontal_width,
            max_metrics.materialized_width,
        )?;
        Ok(!min_metrics.truncated
            && !max_metrics.truncated
            && min_metrics.materialized_width <= max_start)
    }

    fn datum_projection_loses_identity(
        &self,
        series: &SeriesPlan,
        datum: &SeriesDatum,
        plot_area: XyChartPlotArea,
        horizontal: bool,
        resources: &ResourceContext,
    ) -> Result<bool> {
        let Some(value) = datum.value.filter(|value| value.is_finite()) else {
            return Ok(false);
        };
        let value_steps = if horizontal {
            plot_area.horizontal_width
        } else {
            plot_area.vertical_height
        };
        let projected_value = match series.plot_type {
            XyChartPlotType::Bar => bar_height(value, self.y_range, value_steps),
            XyChartPlotType::Line => line_level(value, self.y_range, value_steps),
        };
        let canonical_value = projected_value as f64 / value_steps as f64;
        if self.y_range.normalized(value) != canonical_value {
            return Ok(true);
        }

        let AxisPlan::Linear { .. } = self.x_axis else {
            return Ok(false);
        };
        if horizontal {
            return Ok(false);
        }
        let Some(normalized_x) = datum.normalized_x else {
            return Ok(true);
        };
        let plot_width = plot_area.vertical_plot_width(self.slot_count, resources)?;
        if plot_width <= 1 {
            return Ok(normalized_x != 0.0);
        }
        let first = plot_area
            .vertical_band_center(0, resources)?
            .min(plot_width - 1);
        let last = plot_area
            .vertical_band_center(self.slot_count.saturating_sub(1), resources)?
            .min(plot_width - 1);
        let span = checked_grid_sub(resources, last, first)?;
        let column = self.sample_vertical_column(datum, plot_width, plot_area, resources)?;
        let offset = checked_grid_sub(resources, column, first)?;
        let canonical_x = if span == 0 {
            0.0
        } else {
            offset as f64 / span as f64
        };
        Ok(normalized_x != canonical_x)
    }

    fn projected_point(
        &self,
        datum: &SeriesDatum,
        plot_area: XyChartPlotArea,
        horizontal: bool,
        resources: &ResourceContext,
    ) -> Result<Option<(usize, usize)>> {
        let Some(value) = datum.value.filter(|value| value.is_finite()) else {
            return Ok(None);
        };

        if horizontal {
            let row = self.horizontal_anchor_row(self.sample_slot(datum), resources)?;
            let col = checked_grid_sub(
                resources,
                line_level(value, self.y_range, plot_area.horizontal_width),
                1,
            )?;
            return Ok(Some((row, col)));
        }

        let plot_width = plot_area.vertical_plot_width(self.slot_count, resources)?;
        let row = checked_grid_sub(
            resources,
            plot_area.vertical_height,
            line_level(value, self.y_range, plot_area.vertical_height),
        )?;
        let col = self.sample_vertical_column(datum, plot_width, plot_area, resources)?;
        Ok(Some((row, col)))
    }

    fn vertical_bars_overlap(
        &self,
        series: &SeriesPlan,
        datum: &SeriesDatum,
        previous: &SeriesDatum,
        plot_area: XyChartPlotArea,
        resources: &ResourceContext,
    ) -> Result<bool> {
        let Some(value) = datum.value.filter(|value| value.is_finite()) else {
            return Ok(false);
        };
        let Some(previous_value) = previous.value.filter(|value| value.is_finite()) else {
            return Ok(false);
        };
        if bar_height(value, self.y_range, plot_area.vertical_height) == 0
            || bar_height(previous_value, self.y_range, plot_area.vertical_height) == 0
        {
            return Ok(false);
        }

        let plot_width = plot_area.vertical_plot_width(self.slot_count, resources)?;
        let (start, width) =
            vertical_bar_span(self, series, datum, plot_width, plot_area, resources)?;
        let (previous_start, previous_width) =
            vertical_bar_span(self, series, previous, plot_width, plot_area, resources)?;
        let end = resources.checked_grid_add(start, width)?;
        let previous_end = resources.checked_grid_add(previous_start, previous_width)?;
        Ok(start < previous_end && previous_start < end)
    }
}

fn band_label_projection_loses_identity(
    authored_labels: &[String],
    displayed_labels: &[String],
    plot_area: XyChartPlotArea,
    horizontal: bool,
    resources: &mut ResourceContext,
) -> Result<bool> {
    if authored_labels.len() != displayed_labels.len() {
        return Ok(true);
    }
    if horizontal {
        let mut projected_width = None;
        for (authored, displayed) in authored_labels.iter().zip(displayed_labels) {
            let metrics = visit_fitted_safe_line(
                displayed,
                usize::MAX,
                plot_area.width_profile,
                resources,
                |_| Ok(()),
            )?;
            if metrics.truncated || displayed != authored || displayed.ends_with(' ') {
                return Ok(true);
            }
            if projected_width.is_some_and(|width| width != metrics.materialized_width) {
                return Ok(true);
            }
            projected_width = Some(metrics.materialized_width);
        }
        return Ok(false);
    }
    for (index, (authored, displayed)) in authored_labels.iter().zip(displayed_labels).enumerate() {
        let metrics = visit_fitted_safe_line(
            displayed,
            plot_area.category_band_width,
            plot_area.width_profile,
            resources,
            |_| Ok(()),
        )?;
        if metrics.truncated {
            return Ok(true);
        }
        let padding = checked_grid_sub(
            resources,
            plot_area.category_band_width,
            metrics.materialized_width,
        )?;
        let loses_trailing_spaces = displayed.ends_with(' ') && index + 1 == displayed_labels.len();
        if padding != 0 || displayed != authored || loses_trailing_spaces {
            return Ok(true);
        }
    }
    Ok(false)
}

fn build_axis_plan(
    axis: &XyChartAxisRenderModel,
    data_count: usize,
    plots: &[XyChartPlotRenderModel],
    resources: &mut ResourceContext,
) -> Result<AxisPlan> {
    match axis {
        XyChartAxisRenderModel::Band { categories, .. } => Ok(AxisPlan::Band {
            authored_category_count: categories.len(),
        }),
        XyChartAxisRenderModel::Linear { min, max, .. } => {
            let mut data_min = f64::INFINITY;
            let mut data_max = f64::NEG_INFINITY;
            for plot in plots {
                resources.charge_layout_work(1)?;
                for (x, _) in &plot.data {
                    resources.charge_layout_work(1)?;
                    if let Ok(value) = x.parse::<f64>()
                        && value.is_finite()
                    {
                        data_min = data_min.min(value);
                        data_max = data_max.max(value);
                    }
                }
            }
            let fallback_min = if data_min.is_finite() { data_min } else { 1.0 };
            let fallback_max = if data_max.is_finite() {
                data_max
            } else {
                data_count.max(1) as f64
            };
            Ok(AxisPlan::Linear {
                range: ValueRange {
                    min: min.unwrap_or(fallback_min),
                    max: max.unwrap_or(fallback_max),
                },
            })
        }
    }
}

fn axis_labels(
    axis: &AxisPlan,
    authored_band_categories: &[String],
    count: usize,
    resources: &mut ResourceContext,
) -> Result<Vec<String>> {
    axis_labels_with_allocation_probe(axis, authored_band_categories, count, resources, || {})
}

fn axis_labels_with_allocation_probe(
    axis: &AxisPlan,
    authored_band_categories: &[String],
    count: usize,
    resources: &mut ResourceContext,
    before_band_allocation: impl FnOnce(),
) -> Result<Vec<String>> {
    match axis {
        AxisPlan::Band {
            authored_category_count,
        } => {
            let categories = authored_band_categories
                .get(..*authored_category_count)
                .ok_or_else(|| resources.grid_overflow())?;
            if count < categories.len() {
                return Err(resources.grid_overflow());
            }

            resources.charge_layout_work(count)?;
            for category in categories {
                charge_text_layout(resources, category)?;
            }

            before_band_allocation();
            let mut labels = Vec::new();
            labels
                .try_reserve_exact(count)
                .map_err(|_| AsciiError::AllocationFailed {
                    phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
                })?;
            for category in categories {
                let mut label = String::new();
                label.try_reserve_exact(category.len()).map_err(|_| {
                    AsciiError::AllocationFailed {
                        phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
                    }
                })?;
                label.push_str(category);
                labels.push(label);
            }
            for index in labels.len()..count {
                labels.push(resources.checked_grid_add(index, 1)?.to_string());
            }
            Ok(labels)
        }
        AxisPlan::Linear { range } => {
            let mut labels = Vec::new();
            labels
                .try_reserve_exact(count)
                .map_err(|_| AsciiError::AllocationFailed {
                    phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
                })?;
            if count == 1 {
                resources.charge_layout_work(1)?;
                labels.push(format_data_number(range.min));
            } else if count > 1 {
                let intervals = checked_grid_sub(resources, count, 1)?;
                let step = range.span() / intervals as f64;
                for index in 0..count {
                    resources.charge_layout_work(1)?;
                    labels.push(format_tick_number(range.min + step * index as f64, step));
                }
            }
            Ok(labels)
        }
    }
}

fn build_horizontal_axis_labels(
    axis: &AxisPlan,
    fallback_labels: &[String],
    slot_count: usize,
    series: &[SeriesPlan],
    resources: &mut ResourceContext,
) -> Result<Vec<String>> {
    let mut labels = Vec::new();
    labels
        .try_reserve_exact(fallback_labels.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    labels.extend(fallback_labels.iter().cloned());
    if !matches!(axis, AxisPlan::Linear { .. }) {
        return Ok(labels);
    }

    let mut authored_x_by_slot = Vec::new();
    authored_x_by_slot
        .try_reserve_exact(slot_count)
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for _ in 0..slot_count {
        authored_x_by_slot.push(Vec::<&str>::new());
    }

    for series in series {
        resources.charge_layout_work(1)?;
        for datum in &series.data {
            resources.charge_layout_work(1)?;
            let x = datum.x.trim();
            if x.is_empty() {
                continue;
            }
            let Some(slot_values) = authored_x_by_slot.get_mut(datum.slot) else {
                return Err(resources.grid_overflow());
            };
            let mut duplicate = false;
            for existing in slot_values.iter().copied() {
                resources.charge_layout_work(1)?;
                if existing == x {
                    duplicate = true;
                    break;
                }
            }
            if duplicate {
                continue;
            }
            slot_values
                .try_reserve_exact(1)
                .map_err(|_| AsciiError::AllocationFailed {
                    phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
                })?;
            slot_values.push(x);
        }
    }

    for (slot, authored_values) in authored_x_by_slot.into_iter().enumerate() {
        if authored_values.is_empty() {
            continue;
        }
        let mut label = String::new();
        for (index, x) in authored_values.into_iter().enumerate() {
            resources.charge_layout_work(1)?;
            let separator = if index == 0 { "" } else { " / " };
            let additional = separator
                .len()
                .checked_add(x.len())
                .ok_or_else(|| resources.work_overflow())?;
            label
                .try_reserve_exact(additional)
                .map_err(|_| AsciiError::AllocationFailed {
                    phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
                })?;
            label.push_str(separator);
            label.push_str(x);
        }
        let Some(target) = labels.get_mut(slot) else {
            return Err(resources.grid_overflow());
        };
        *target = label;
    }
    Ok(labels)
}

fn build_series_plan(
    plot: &XyChartPlotRenderModel,
    series_index: usize,
    bar_lane: Option<usize>,
    x_axis: &AxisPlan,
    category_labels: &[String],
    slot_count: usize,
    resources: &mut ResourceContext,
) -> Result<SeriesPlan> {
    let data_len = effective_sample_count(plot);
    let mut data = Vec::new();
    data.try_reserve_exact(data_len)
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;

    if plot.data.is_empty() {
        for (index, value) in plot.values.iter().copied().enumerate() {
            resources.charge_layout_work(1)?;
            let x = fallback_x_label(x_axis, category_labels, index, plot.values.len());
            let (slot, normalized_x, x_clipped) = x_axis.resolve_sample_position(
                category_labels,
                &x,
                index,
                slot_count,
                resources,
            )?;
            data.push(SeriesDatum {
                x,
                value: Some(value),
                has_point_label: plot.point_labels.get(index).is_some(),
                authored_x: false,
                slot,
                normalized_x,
                x_clipped,
            });
        }
    } else {
        for (index, (x, value)) in plot.data.iter().enumerate() {
            resources.charge_layout_work(1)?;
            let (slot, normalized_x, x_clipped) =
                x_axis.resolve_sample_position(category_labels, x, index, slot_count, resources)?;
            data.push(SeriesDatum {
                x: x.clone(),
                value: *value,
                has_point_label: plot.point_labels.get(index).is_some(),
                authored_x: true,
                slot,
                normalized_x,
                x_clipped,
            });
        }
    }

    let has_orphan_point_labels = plot.point_labels.len() > data_len;

    Ok(SeriesPlan {
        series_index,
        plot_type: plot.plot_type,
        title: plot.title.clone(),
        data,
        has_orphan_point_labels,
        bar_lane,
    })
}

pub(super) const fn effective_sample_count(plot: &XyChartPlotRenderModel) -> usize {
    if plot.data.is_empty() {
        plot.values.len()
    } else {
        plot.data.len()
    }
}

pub(super) const fn plot_type_name(plot_type: XyChartPlotType) -> &'static str {
    match plot_type {
        XyChartPlotType::Line => "line",
        XyChartPlotType::Bar => "bar",
    }
}

fn fallback_x_label(
    axis: &AxisPlan,
    category_labels: &[String],
    index: usize,
    count: usize,
) -> String {
    match axis {
        AxisPlan::Band { .. } => category_labels
            .get(index)
            .cloned()
            .unwrap_or_else(|| (index + 1).to_string()),
        AxisPlan::Linear { range } => {
            let value = if count <= 1 {
                range.min
            } else {
                range.min + range.span() * index as f64 / (count - 1) as f64
            };
            format_data_number(value)
        }
    }
}

fn build_value_range(
    axis: &XyChartAxisRenderModel,
    series: &[SeriesPlan],
    resources: &mut ResourceContext,
) -> Result<ValueRange> {
    let mut data_min = f64::INFINITY;
    let mut data_max = f64::NEG_INFINITY;
    for series in series {
        resources.charge_layout_work(1)?;
        for value in series.data.iter().filter_map(|datum| datum.value) {
            resources.charge_layout_work(1)?;
            if value.is_finite() {
                data_min = data_min.min(value);
                data_max = data_max.max(value);
            }
        }
    }

    let (axis_min, axis_max) = match axis {
        XyChartAxisRenderModel::Linear { min, max, .. } => (*min, *max),
        XyChartAxisRenderModel::Band { .. } => (None, None),
    };
    let fallback_min = if data_min.is_finite() { data_min } else { 0.0 };
    let fallback_max = if data_max.is_finite() { data_max } else { 1.0 };
    Ok(ValueRange {
        min: axis_min.unwrap_or(fallback_min),
        max: axis_max.unwrap_or(fallback_max),
    })
}

#[derive(Debug, Clone)]
pub(super) struct VerticalPlot {
    pub(super) rows: Vec<StyledLine>,
    pub(super) width: usize,
}

#[derive(Debug)]
pub(super) struct VerticalPlotAdmissionPlan {
    row_widths: Vec<usize>,
}

impl VerticalPlotAdmissionPlan {
    pub(super) fn row_widths(&self) -> &[usize] {
        &self.row_widths
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RetainedInterval {
    start: usize,
    end: usize,
}

#[derive(Debug)]
struct RetainedCellMask {
    extent: LogicalExtent,
    cells: Vec<u8>,
}

impl RetainedCellMask {
    fn try_new(
        extent: LogicalExtent,
        resources: &mut ResourceContext,
        checkpoints: &mut XyChartCheckpointCursor<'_>,
    ) -> Result<Self> {
        resources.check(AsciiResourceLimitId::MaxGridCells, extent.cells())?;
        resources.charge_layout_work(extent.cells())?;
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(extent.cells())
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::Layout.as_str(),
            })?;
        for _ in 0..extent.cells() {
            checkpoints.tick(OperationPhase::Layout)?;
            cells.push(0);
        }
        Ok(Self { extent, cells })
    }

    fn fill(
        &mut self,
        row: usize,
        start: usize,
        width: usize,
        resources: &mut ResourceContext,
        checkpoints: &mut XyChartCheckpointCursor<'_>,
    ) -> Result<()> {
        let end = resources.checked_grid_add(start, width)?;
        self.overwrite(
            row,
            RetainedInterval { start, end },
            Some(RetainedInterval { start, end }),
            resources,
            checkpoints,
        )
    }

    fn overwrite(
        &mut self,
        row: usize,
        target: RetainedInterval,
        replacement: Option<RetainedInterval>,
        resources: &mut ResourceContext,
        checkpoints: &mut XyChartCheckpointCursor<'_>,
    ) -> Result<()> {
        if row >= self.extent.height()
            || target.start > target.end
            || target.end > self.extent.width()
            || replacement.is_some_and(|value| {
                value.start > value.end || value.start < target.start || value.end > target.end
            })
        {
            return Err(resources.grid_overflow());
        }
        let target_width = checked_grid_sub(resources, target.end, target.start)?;
        resources.charge_layout_work(target_width)?;
        let row_start = resources.checked_grid_mul(row, self.extent.width())?;
        for col in target.start..target.end {
            checkpoints.tick(OperationPhase::Layout)?;
            let index = resources.checked_grid_add(row_start, col)?;
            let cell = self
                .cells
                .get_mut(index)
                .ok_or_else(|| resources.grid_overflow())?;
            *cell =
                u8::from(replacement.is_some_and(|value| value.start <= col && col < value.end));
        }
        Ok(())
    }

    fn write_row_widths(
        &self,
        widths: &mut [usize],
        resources: &mut ResourceContext,
        checkpoints: &mut XyChartCheckpointCursor<'_>,
    ) -> Result<()> {
        if widths.len() != self.extent.height() {
            return Err(resources.grid_overflow());
        }
        for (row, width) in widths.iter_mut().enumerate() {
            resources.charge_layout_work(self.extent.width())?;
            let row_start = resources.checked_grid_mul(row, self.extent.width())?;
            let mut retained_width = 0;
            for col in 0..self.extent.width() {
                checkpoints.tick(OperationPhase::Layout)?;
                let index = resources.checked_grid_add(row_start, col)?;
                if self
                    .cells
                    .get(index)
                    .copied()
                    .ok_or_else(|| resources.grid_overflow())?
                    != 0
                {
                    retained_width = resources.checked_grid_add(col, 1)?;
                }
            }
            *width = retained_width;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(super) struct HorizontalPlotRow {
    pub(super) line: StyledLine,
    pub(super) category_index: usize,
    pub(super) show_category_label: bool,
    pub(super) bar_value: Option<f64>,
    pub(super) bar_label: Option<String>,
}

#[derive(Debug)]
pub(super) struct HorizontalPlotAdmissionPlan {
    row_widths: Vec<usize>,
    compact_bar_values: Option<CompactBarSlotValues>,
}

impl HorizontalPlotAdmissionPlan {
    pub(super) fn row_widths(&self) -> &[usize] {
        &self.row_widths
    }

    pub(super) fn compact_bar_value(&self, slot: usize) -> Option<f64> {
        self.compact_bar_values
            .as_ref()
            .and_then(|values| values.value(slot))
    }

    pub(super) fn into_compact_bar_values(self) -> Option<CompactBarSlotValues> {
        self.compact_bar_values
    }
}

#[derive(Debug)]
pub(super) struct CompactBarSlotValues(Vec<Option<Option<f64>>>);

impl CompactBarSlotValues {
    fn try_new(
        slot_count: usize,
        resources: &mut ResourceContext,
        checkpoints: &mut XyChartCheckpointCursor<'_>,
    ) -> Result<Self> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(slot_count)
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::Layout.as_str(),
            })?;
        for _ in 0..slot_count {
            checkpoints.tick(OperationPhase::Layout)?;
            resources.charge_layout_work(1)?;
            values.push(None);
        }
        Ok(Self(values))
    }

    fn record_first(
        &mut self,
        slot: usize,
        value: Option<f64>,
        resources: &ResourceContext,
    ) -> Result<()> {
        let planned = self
            .0
            .get_mut(slot)
            .ok_or_else(|| resources.grid_overflow())?;
        if planned.is_none() {
            *planned = Some(value);
        }
        Ok(())
    }

    pub(super) fn value(&self, slot: usize) -> Option<f64> {
        self.0.get(slot).copied().flatten().flatten()
    }
}

#[derive(Debug, Clone, Copy)]
enum OrthogonalSegment {
    Horizontal {
        row: usize,
        from_col: usize,
        to_col: usize,
    },
    Vertical {
        col: usize,
        from_row: usize,
        to_row: usize,
    },
}

#[derive(Debug, Clone, Copy)]
struct OrthogonalConnection {
    segments: [Option<OrthogonalSegment>; 3],
}

impl OrthogonalConnection {
    fn new(from: (usize, usize), to: (usize, usize), resources: &ResourceContext) -> Result<Self> {
        let (from_row, from_col) = from;
        let (to_row, to_col) = to;
        let segments = if from_col == to_col {
            [
                Some(OrthogonalSegment::Vertical {
                    col: from_col,
                    from_row,
                    to_row,
                }),
                None,
                None,
            ]
        } else if from_row == to_row {
            [
                Some(OrthogonalSegment::Horizontal {
                    row: from_row,
                    from_col,
                    to_col,
                }),
                None,
                None,
            ]
        } else {
            let mid_col = resources.checked_grid_add(from_col, to_col)? / 2;
            [
                Some(OrthogonalSegment::Horizontal {
                    row: from_row,
                    from_col,
                    to_col: mid_col,
                }),
                Some(OrthogonalSegment::Vertical {
                    col: mid_col,
                    from_row,
                    to_row,
                }),
                Some(OrthogonalSegment::Horizontal {
                    row: to_row,
                    from_col: mid_col,
                    to_col,
                }),
            ]
        };
        Ok(Self { segments })
    }

    fn segments(self) -> impl Iterator<Item = OrthogonalSegment> {
        self.segments.into_iter().flatten()
    }
}

pub(super) fn plan_horizontal_plot_admission(
    plan: &TerminalChartPlan,
    plot_area: XyChartPlotArea,
    row_count: usize,
    resources: &mut ResourceContext,
    checkpoints: &mut XyChartCheckpointCursor<'_>,
) -> Result<HorizontalPlotAdmissionPlan> {
    let mut widths = Vec::new();
    widths
        .try_reserve_exact(row_count)
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::Layout.as_str(),
        })?;
    for _ in 0..row_count {
        checkpoints.tick(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        widths.push(0);
    }

    let mut compact_bar_values = if plan.bar_series_count == 1 && plan.line_series_count == 0 {
        Some(CompactBarSlotValues::try_new(
            plan.slot_count,
            resources,
            checkpoints,
        )?)
    } else {
        None
    };

    let rows_per_slot = plan.horizontal_rows_per_slot();
    for series in &plan.series {
        resources.charge_layout_work(1)?;
        if series.plot_type != XyChartPlotType::Bar {
            continue;
        }
        let lane = series.bar_lane.unwrap_or(0);
        for datum in &series.data {
            checkpoints.tick(OperationPhase::Layout)?;
            resources.charge_layout_work(1)?;
            let slot = plan.sample_slot(datum);
            if let Some(values) = compact_bar_values.as_mut() {
                values.record_first(slot, datum.value, resources)?;
            }
            let Some(value) = datum.value.filter(|value| value.is_finite()) else {
                continue;
            };
            let row_start = resources.checked_grid_mul(slot, rows_per_slot)?;
            let row = if plan.bar_series_count > 1 {
                resources.checked_grid_add(row_start, lane)?
            } else {
                row_start
            };
            let width = horizontal_bar_width(value, plan.y_range, plot_area);
            include_horizontal_row_width(&mut widths, row, width, resources)?;
        }
    }

    for series in &plan.series {
        resources.charge_layout_work(1)?;
        if series.plot_type != XyChartPlotType::Line {
            continue;
        }
        let mut previous = None;
        for datum in &series.data {
            checkpoints.tick(OperationPhase::Layout)?;
            resources.charge_layout_work(1)?;
            let Some(value) = datum.value.filter(|value| value.is_finite()) else {
                previous = None;
                continue;
            };
            let row = plan.horizontal_anchor_row(plan.sample_slot(datum), resources)?;
            let col = checked_grid_sub(
                resources,
                line_level(value, plan.y_range, plot_area.horizontal_width),
                1,
            )?;
            let point = (row, col);
            if let Some(previous) = previous {
                include_horizontal_connection_widths(
                    &mut widths,
                    previous,
                    point,
                    resources,
                    checkpoints,
                )?;
            }
            include_horizontal_row_width(
                &mut widths,
                row,
                resources.checked_grid_add(col, 1)?,
                resources,
            )?;
            previous = Some(point);
        }
    }

    Ok(HorizontalPlotAdmissionPlan {
        row_widths: widths,
        compact_bar_values,
    })
}

pub(super) fn plan_vertical_plot_admission(
    plan: &TerminalChartPlan,
    plot_area: XyChartPlotArea,
    plot_extent: LogicalExtent,
    compact_inside_labels: bool,
    preserve_color: bool,
    resources: &mut ResourceContext,
    checkpoints: &mut XyChartCheckpointCursor<'_>,
) -> Result<VerticalPlotAdmissionPlan> {
    let mut widths = Vec::new();
    widths
        .try_reserve_exact(plot_extent.height())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::Layout.as_str(),
        })?;
    let track_plain_label_overwrites = compact_inside_labels
        && !preserve_color
        && plan.series.len() == 1
        && plan.series[0].plot_type == XyChartPlotType::Bar;
    let mut retained_mask = None;
    for _ in 0..plot_extent.height() {
        checkpoints.tick(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        widths.push(0);
    }

    for series in &plan.series {
        resources.charge_layout_work(1)?;
        if series.plot_type != XyChartPlotType::Bar {
            continue;
        }
        for datum in &series.data {
            checkpoints.tick(OperationPhase::Layout)?;
            resources.charge_layout_work(1)?;
            let Some(value) = datum.value.filter(|value| value.is_finite()) else {
                continue;
            };
            let height = bar_height(value, plan.y_range, plot_area.vertical_height);
            if height == 0 {
                continue;
            }
            if track_plain_label_overwrites && retained_mask.is_none() {
                retained_mask = Some(RetainedCellMask::try_new(
                    plot_extent,
                    resources,
                    checkpoints,
                )?);
            }
            let (band_start, band_width) = vertical_bar_span(
                plan,
                series,
                datum,
                plot_extent.width(),
                plot_area,
                resources,
            )?;
            let full_retained_width = resources.checked_grid_add(band_start, band_width)?;
            let label_retained_width = if compact_inside_labels && retained_mask.is_none() {
                Some(vertical_inside_label_retained_width(
                    value,
                    band_start,
                    band_width,
                    preserve_color,
                    plot_area.width_profile,
                    resources,
                )?)
            } else {
                None
            };
            for level in 1..=height {
                checkpoints.tick(OperationPhase::Layout)?;
                resources.charge_layout_work(1)?;
                let row = checked_grid_sub(resources, plot_area.vertical_height, level)?;
                if let Some(mask) = retained_mask.as_mut() {
                    mask.fill(row, band_start, band_width, resources, checkpoints)?;
                } else {
                    let retained_width = if level == height {
                        label_retained_width.unwrap_or(full_retained_width)
                    } else {
                        full_retained_width
                    };
                    include_horizontal_row_width(&mut widths, row, retained_width, resources)?;
                }
            }
        }
    }

    for series in &plan.series {
        resources.charge_layout_work(1)?;
        if series.plot_type != XyChartPlotType::Line {
            continue;
        }
        let mut previous = None;
        for datum in &series.data {
            checkpoints.tick(OperationPhase::Layout)?;
            resources.charge_layout_work(1)?;
            let Some(value) = datum.value.filter(|value| value.is_finite()) else {
                previous = None;
                continue;
            };
            let level = line_level(value, plan.y_range, plot_area.vertical_height);
            let row = checked_grid_sub(resources, plot_area.vertical_height, level)?;
            let col =
                plan.sample_vertical_column(datum, plot_extent.width(), plot_area, resources)?;
            let point = (row, col);
            if let Some(previous) = previous {
                include_horizontal_connection_widths(
                    &mut widths,
                    previous,
                    point,
                    resources,
                    checkpoints,
                )?;
            }
            include_horizontal_row_width(
                &mut widths,
                row,
                resources.checked_grid_add(col, 1)?,
                resources,
            )?;
            previous = Some(point);
        }
    }

    if let Some(mask) = retained_mask.as_mut() {
        let bar_series = &plan.series[0];
        for datum in &bar_series.data {
            checkpoints.tick(OperationPhase::Layout)?;
            resources.charge_layout_work(1)?;
            let Some(value) = datum.value.filter(|value| value.is_finite()) else {
                continue;
            };
            let height = bar_height(value, plan.y_range, plot_area.vertical_height);
            if height == 0 {
                continue;
            }
            let row = checked_grid_sub(resources, plot_area.vertical_height, height)?;
            let (band_start, band_width) = vertical_bar_span(
                plan,
                bar_series,
                datum,
                plot_extent.width(),
                plot_area,
                resources,
            )?;
            let target = RetainedInterval {
                start: band_start,
                end: resources.checked_grid_add(band_start, band_width)?,
            };
            let replacement = vertical_inside_label_retained_interval(
                value,
                band_start,
                band_width,
                plot_area.width_profile,
                resources,
            )?;
            mask.overwrite(row, target, replacement, resources, checkpoints)?;
        }
        mask.write_row_widths(&mut widths, resources, checkpoints)?;
    }

    Ok(VerticalPlotAdmissionPlan { row_widths: widths })
}

fn vertical_inside_label_retained_width(
    value: f64,
    band_start: usize,
    band_width: usize,
    preserve_color: bool,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<usize> {
    if preserve_color {
        return resources.checked_grid_add(band_start, band_width);
    }
    Ok(vertical_inside_label_retained_interval(
        value,
        band_start,
        band_width,
        width_profile,
        resources,
    )?
    .map_or(0, |interval| interval.end))
}

fn vertical_inside_label_retained_interval(
    value: f64,
    band_start: usize,
    band_width: usize,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<Option<RetainedInterval>> {
    let label = format_data_number(value);
    let metrics = visit_fitted_safe_line(&label, band_width, width_profile, resources, |_| Ok(()))?;
    if metrics.retained_width == 0 {
        return Ok(None);
    }
    let remaining = checked_grid_sub(resources, band_width, metrics.materialized_width)?;
    let left_padding = remaining / 2;
    let start = resources.checked_grid_add(band_start, left_padding)?;
    let end = resources.checked_grid_add(start, metrics.retained_width)?;
    Ok(Some(RetainedInterval { start, end }))
}

fn include_horizontal_connection_widths(
    widths: &mut [usize],
    from: (usize, usize),
    to: (usize, usize),
    resources: &mut ResourceContext,
    checkpoints: &mut XyChartCheckpointCursor<'_>,
) -> Result<()> {
    for segment in OrthogonalConnection::new(from, to, resources)?.segments() {
        match segment {
            OrthogonalSegment::Horizontal {
                row,
                from_col,
                to_col,
            } => {
                checkpoints.tick(OperationPhase::Layout)?;
                resources.charge_layout_work(1)?;
                include_horizontal_row_width(
                    widths,
                    row,
                    resources.checked_grid_add(from_col.max(to_col), 1)?,
                    resources,
                )?;
            }
            OrthogonalSegment::Vertical {
                col,
                from_row,
                to_row,
            } => include_vertical_connection_widths(
                widths,
                col,
                from_row,
                to_row,
                resources,
                checkpoints,
            )?,
        }
    }
    Ok(())
}

fn include_vertical_connection_widths(
    widths: &mut [usize],
    col: usize,
    from_row: usize,
    to_row: usize,
    resources: &mut ResourceContext,
    checkpoints: &mut XyChartCheckpointCursor<'_>,
) -> Result<()> {
    let start = from_row.min(to_row);
    let end = from_row.max(to_row);
    let width = resources.checked_grid_add(col, 1)?;
    for row in start..=end {
        checkpoints.tick(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        include_horizontal_row_width(widths, row, width, resources)?;
    }
    Ok(())
}

fn include_horizontal_row_width(
    widths: &mut [usize],
    row: usize,
    width: usize,
    resources: &ResourceContext,
) -> Result<()> {
    let planned = widths
        .get_mut(row)
        .ok_or_else(|| resources.grid_overflow())?;
    *planned = (*planned).max(width);
    Ok(())
}

#[derive(Debug)]
struct LineTopology {
    width: usize,
    height: usize,
    cells: Vec<LineTopologyCell>,
}

impl LineTopology {
    fn new(
        width: usize,
        height: usize,
        resources: &mut ResourceContext,
        checkpoints: &mut XyChartCheckpointCursor<'_>,
    ) -> Result<Self> {
        let extent = resources.grid_extent(width, height)?;
        let concurrent_cells = resources.checked_grid_mul(extent.cells(), 2)?;
        resources.check(AsciiResourceLimitId::MaxGridCells, concurrent_cells)?;
        resources.charge_layout_work(extent.cells())?;
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(extent.cells())
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::Layout.as_str(),
            })?;
        for _ in 0..extent.cells() {
            checkpoints.tick(OperationPhase::Layout)?;
            cells.push(LineTopologyCell::default());
        }
        Ok(Self {
            width,
            height,
            cells,
        })
    }

    fn connect(
        &mut self,
        from: (usize, usize),
        to: (usize, usize),
        resources: &mut ResourceContext,
        checkpoints: &mut XyChartCheckpointCursor<'_>,
    ) -> Result<()> {
        for segment in OrthogonalConnection::new(from, to, resources)?.segments() {
            match segment {
                OrthogonalSegment::Horizontal {
                    row,
                    from_col,
                    to_col,
                } => self.connect_horizontal(row, from_col, to_col, resources, checkpoints)?,
                OrthogonalSegment::Vertical {
                    col,
                    from_row,
                    to_row,
                } => self.connect_vertical(col, from_row, to_row, resources, checkpoints)?,
            }
        }
        Ok(())
    }

    fn mark_point(&mut self, row: usize, col: usize, resources: &ResourceContext) -> Result<()> {
        self.cell_mut(row, col, resources)?.add(LINE_POINT);
        Ok(())
    }

    fn paint(
        &self,
        rows: &mut [StyledLine],
        chars: ChartChars,
        role: AsciiColorRole,
        resources: &mut ResourceContext,
        checkpoints: &mut XyChartCheckpointCursor<'_>,
    ) -> Result<()> {
        resources.charge_layout_work(self.cells.len())?;
        for (index, cell) in self.cells.iter().copied().enumerate() {
            checkpoints.tick(OperationPhase::Layout)?;
            if cell.is_empty() {
                continue;
            }
            let row = index / self.width;
            let col = index % self.width;
            set_cell(rows, row, col, chars.line_glyph(cell), role)?;
        }
        Ok(())
    }

    fn connect_horizontal(
        &mut self,
        row: usize,
        from_col: usize,
        to_col: usize,
        resources: &mut ResourceContext,
        checkpoints: &mut XyChartCheckpointCursor<'_>,
    ) -> Result<()> {
        let start = from_col.min(to_col);
        let end = from_col.max(to_col);
        let steps = checked_grid_sub(resources, end, start)?;
        resources.charge_layout_work(steps)?;
        for col in start..end {
            checkpoints.tick(OperationPhase::Layout)?;
            self.cell_mut(row, col, resources)?.add(LINE_EAST);
            self.cell_mut(row, col + 1, resources)?.add(LINE_WEST);
        }
        Ok(())
    }

    fn connect_vertical(
        &mut self,
        col: usize,
        from_row: usize,
        to_row: usize,
        resources: &mut ResourceContext,
        checkpoints: &mut XyChartCheckpointCursor<'_>,
    ) -> Result<()> {
        let start = from_row.min(to_row);
        let end = from_row.max(to_row);
        let steps = checked_grid_sub(resources, end, start)?;
        resources.charge_layout_work(steps)?;
        for row in start..end {
            checkpoints.tick(OperationPhase::Layout)?;
            self.cell_mut(row, col, resources)?.add(LINE_SOUTH);
            self.cell_mut(row + 1, col, resources)?.add(LINE_NORTH);
        }
        Ok(())
    }

    fn cell_mut(
        &mut self,
        row: usize,
        col: usize,
        resources: &ResourceContext,
    ) -> Result<&mut LineTopologyCell> {
        if row >= self.height || col >= self.width {
            return Err(resources.grid_overflow());
        }
        let index =
            resources.checked_grid_add(resources.checked_grid_mul(row, self.width)?, col)?;
        self.cells
            .get_mut(index)
            .ok_or_else(|| resources.grid_overflow())
    }
}

pub(super) fn build_vertical_plot(
    plan: &TerminalChartPlan,
    chars: ChartChars,
    plot_area: XyChartPlotArea,
    plot_extent: LogicalExtent,
    resources: &mut ResourceContext,
    checkpoints: &mut XyChartCheckpointCursor<'_>,
) -> Result<VerticalPlot> {
    debug_assert_eq!(plot_extent.height(), plot_area.vertical_height);
    let width = plot_extent.width();
    let mut rows = Vec::new();
    rows.try_reserve_exact(plot_extent.height())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::Layout.as_str(),
        })?;
    for _ in 0..plot_extent.height() {
        checkpoints.tick(OperationPhase::Layout)?;
        rows.push(try_plot_row(width, plot_area, resources)?);
    }

    for series in &plan.series {
        resources.charge_layout_work(1)?;
        if series.plot_type == XyChartPlotType::Bar {
            draw_vertical_bar_plot(
                &mut rows,
                series,
                plan,
                chars,
                plot_area,
                width,
                resources,
                checkpoints,
            )?;
        }
    }

    for series in &plan.series {
        resources.charge_layout_work(1)?;
        if series.plot_type == XyChartPlotType::Line {
            draw_vertical_line_plot(
                &mut rows,
                series,
                plan,
                chars,
                plot_area,
                width,
                resources,
                checkpoints,
            )?;
        }
    }

    Ok(VerticalPlot { rows, width })
}

pub(super) fn build_horizontal_plot_rows(
    plan: &TerminalChartPlan,
    chars: ChartChars,
    plot_area: XyChartPlotArea,
    plot_extent: LogicalExtent,
    compact_bar_values: Option<&CompactBarSlotValues>,
    resources: &mut ResourceContext,
    checkpoints: &mut XyChartCheckpointCursor<'_>,
) -> Result<Vec<HorizontalPlotRow>> {
    debug_assert_eq!(plot_extent.width(), plot_area.horizontal_width);
    debug_assert_eq!(plot_extent.height(), plan.horizontal_row_count(resources)?);
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(plot_extent.height())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::Layout.as_str(),
        })?;
    for _ in 0..plot_extent.height() {
        checkpoints.tick(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        lines.push(try_plot_row(
            plot_area.horizontal_width,
            plot_area,
            resources,
        )?);
    }

    let rows_per_slot = plan.horizontal_rows_per_slot();
    for series in &plan.series {
        resources.charge_layout_work(1)?;
        if series.plot_type != XyChartPlotType::Bar {
            continue;
        }
        let lane = series.bar_lane.unwrap_or(0);
        for datum in &series.data {
            checkpoints.tick(OperationPhase::Layout)?;
            resources.charge_layout_work(1)?;
            let Some(value) = datum.value.filter(|value| value.is_finite()) else {
                continue;
            };
            let slot = plan.sample_slot(datum);
            let row_start = resources.checked_grid_mul(slot, rows_per_slot)?;
            let row_index = if plan.bar_series_count > 1 {
                resources.checked_grid_add(row_start, lane)?
            } else {
                row_start
            };
            if let Some(line) = lines.get_mut(row_index) {
                draw_horizontal_bar_value(
                    line,
                    value,
                    series.series_index,
                    plan.y_range,
                    chars,
                    plot_area,
                    resources,
                    checkpoints,
                )?;
            }
        }
    }

    for series in &plan.series {
        resources.charge_layout_work(1)?;
        if series.plot_type != XyChartPlotType::Line {
            continue;
        }
        let role = AsciiColorRole::ChartSeries(series.series_index);
        let mut topology = LineTopology::new(
            plot_area.horizontal_width,
            plot_extent.height(),
            resources,
            checkpoints,
        )?;
        let mut previous = None;
        for datum in &series.data {
            checkpoints.tick(OperationPhase::Layout)?;
            resources.charge_layout_work(1)?;
            let Some(value) = datum.value.filter(|value| value.is_finite()) else {
                previous = None;
                continue;
            };
            let slot = plan.sample_slot(datum);
            let row = plan.horizontal_anchor_row(slot, resources)?;
            let col = checked_grid_sub(
                resources,
                line_level(value, plan.y_range, plot_area.horizontal_width),
                1,
            )?;
            let point = (row, col);
            if let Some(previous) = previous {
                topology.connect(previous, point, resources, checkpoints)?;
            }
            resources.charge_layout_work(1)?;
            topology.mark_point(row, col, resources)?;
            previous = Some(point);
        }
        topology.paint(&mut lines, chars, role, resources, checkpoints)?;
    }

    let mut rows = Vec::new();
    rows.try_reserve_exact(lines.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::Layout.as_str(),
        })?;
    for (row_index, line) in lines.into_iter().enumerate() {
        let category_index = row_index / rows_per_slot;
        let show_category_label = row_index % rows_per_slot == 0;
        let bar_value = compact_bar_values.and_then(|values| values.value(category_index));
        let bar_label = bar_value.map(format_data_number);
        rows.push(HorizontalPlotRow {
            line,
            category_index,
            show_category_label,
            bar_value,
            bar_label,
        });
    }
    Ok(rows)
}

pub(super) fn apply_vertical_bar_data_labels(
    plot: &mut VerticalPlot,
    plan: &TerminalChartPlan,
    plot_area: XyChartPlotArea,
    resources: &mut ResourceContext,
    checkpoints: &mut XyChartCheckpointCursor<'_>,
) -> Result<()> {
    let Some(bar_series) = plan
        .series
        .iter()
        .find(|series| series.plot_type == XyChartPlotType::Bar)
    else {
        return Ok(());
    };

    for datum in &bar_series.data {
        checkpoints.tick(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        let Some(value) = datum.value.filter(|value| value.is_finite()) else {
            continue;
        };
        let height = bar_height(value, plan.y_range, plot_area.vertical_height);
        if height == 0 {
            continue;
        }

        let row_idx = checked_grid_sub(resources, plot_area.vertical_height, height)?;
        let (band_start, band_width) =
            vertical_bar_span(plan, bar_series, datum, plot.width, plot_area, resources)?;
        if let Some(row) = plot.rows.get_mut(row_idx) {
            resources.charge_layout_work(band_width)?;
            write_band_text(
                row,
                band_start,
                band_width,
                &format_data_number(value),
                AsciiColorRole::Text,
                resources,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn draw_vertical_bar_plot(
    rows: &mut [StyledLine],
    series: &SeriesPlan,
    plan: &TerminalChartPlan,
    chars: ChartChars,
    plot_area: XyChartPlotArea,
    plot_width: usize,
    resources: &mut ResourceContext,
    checkpoints: &mut XyChartCheckpointCursor<'_>,
) -> Result<()> {
    let role = AsciiColorRole::ChartSeries(series.series_index);
    for datum in &series.data {
        checkpoints.tick(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        let Some(value) = datum.value.filter(|value| value.is_finite()) else {
            continue;
        };
        let height = bar_height(value, plan.y_range, plot_area.vertical_height);
        if height == 0 {
            continue;
        }

        let (band_start, band_width) =
            vertical_bar_span(plan, series, datum, plot_width, plot_area, resources)?;
        for level in 1..=height {
            checkpoints.tick(OperationPhase::Layout)?;
            resources.charge_layout_work(band_width)?;
            let row_idx = checked_grid_sub(resources, plot_area.vertical_height, level)?;
            if let Some(row) = rows.get_mut(row_idx) {
                fill_band(
                    row,
                    band_start,
                    band_width,
                    chars.bar,
                    role,
                    resources,
                    checkpoints,
                )?;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn draw_vertical_line_plot(
    rows: &mut [StyledLine],
    series: &SeriesPlan,
    plan: &TerminalChartPlan,
    chars: ChartChars,
    plot_area: XyChartPlotArea,
    plot_width: usize,
    resources: &mut ResourceContext,
    checkpoints: &mut XyChartCheckpointCursor<'_>,
) -> Result<()> {
    let role = AsciiColorRole::ChartSeries(series.series_index);
    let mut topology = LineTopology::new(plot_width, rows.len(), resources, checkpoints)?;
    let mut previous = None;
    for datum in &series.data {
        checkpoints.tick(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        let Some(value) = datum.value.filter(|value| value.is_finite()) else {
            previous = None;
            continue;
        };
        let level = line_level(value, plan.y_range, plot_area.vertical_height);
        let row = checked_grid_sub(resources, plot_area.vertical_height, level)?;
        let col = plan.sample_vertical_column(datum, plot_width, plot_area, resources)?;
        let point = (row, col);
        if let Some(previous) = previous {
            topology.connect(previous, point, resources, checkpoints)?;
        }
        resources.charge_layout_work(1)?;
        topology.mark_point(row, col, resources)?;
        previous = Some(point);
    }
    topology.paint(rows, chars, role, resources, checkpoints)
}

fn vertical_bar_span(
    plan: &TerminalChartPlan,
    series: &SeriesPlan,
    datum: &SeriesDatum,
    plot_width: usize,
    plot_area: XyChartPlotArea,
    resources: &ResourceContext,
) -> Result<(usize, usize)> {
    if plot_width == 0 {
        return Ok((0, 0));
    }
    let lane_count = plan.bar_series_count.max(1);
    let lane = series.bar_lane.unwrap_or(0).min(lane_count - 1);

    if matches!(&plan.x_axis, AxisPlan::Band { .. }) {
        let slot = plan.sample_slot(datum);
        let band_start = plot_area.vertical_band_start(slot, resources)?;
        if lane_count == 1 {
            return Ok((band_start, plot_area.category_band_width));
        }

        if lane_count <= plot_area.category_band_width {
            let lane_width = plot_area.category_band_width / lane_count;
            let remainder = plot_area.category_band_width % lane_count;
            let extra_before = lane.min(remainder);
            let lane_offset = resources
                .checked_grid_add(resources.checked_grid_mul(lane, lane_width)?, extra_before)?;
            let width = lane_width + usize::from(lane < remainder);
            return Ok((resources.checked_grid_add(band_start, lane_offset)?, width));
        }

        let offset = lane % plot_area.category_band_width;
        return Ok((resources.checked_grid_add(band_start, offset)?, 1));
    }

    let center = plan.sample_vertical_column(datum, plot_width, plot_area, resources)?;
    if lane_count == 1 {
        let width = plot_area.category_band_width.min(plot_width).max(1);
        let start = center
            .saturating_sub(width / 2)
            .min(checked_grid_sub(resources, plot_width, width)?);
        return Ok((start, width));
    }

    let visible_lanes = lane_count.min(plot_width);
    let start = center
        .saturating_sub(visible_lanes / 2)
        .min(checked_grid_sub(resources, plot_width, visible_lanes)?);
    let offset = lane.min(visible_lanes - 1);
    Ok((resources.checked_grid_add(start, offset)?, 1))
}

#[allow(clippy::too_many_arguments)]
fn draw_horizontal_bar_value(
    row: &mut StyledLine,
    value: f64,
    series_index: usize,
    y_range: ValueRange,
    chars: ChartChars,
    plot_area: XyChartPlotArea,
    resources: &mut ResourceContext,
    checkpoints: &mut XyChartCheckpointCursor<'_>,
) -> Result<()> {
    let role = AsciiColorRole::ChartSeries(series_index);
    let width = bar_height(value, y_range, plot_area.horizontal_width);
    resources.charge_layout_work(width)?;
    for col in 0..width {
        checkpoints.tick(OperationPhase::Layout)?;
        row.try_set_role(col, chars.bar, role)?;
    }
    Ok(())
}

pub(super) fn horizontal_bar_width(
    value: f64,
    y_range: ValueRange,
    plot_area: XyChartPlotArea,
) -> usize {
    bar_height(value, y_range, plot_area.horizontal_width)
}

fn set_cell(
    rows: &mut [StyledLine],
    row: usize,
    col: usize,
    value: char,
    role: AsciiColorRole,
) -> Result<()> {
    if let Some(row) = rows.get_mut(row) {
        row.try_set_role(col, value, role)?;
    }
    Ok(())
}

fn try_plot_row(
    width: usize,
    plot_area: XyChartPlotArea,
    resources: &ResourceContext,
) -> Result<StyledLine> {
    StyledLine::try_blank_with_resources(width, plot_area.width_profile, resources).map_err(
        |error| match error {
            AsciiError::AllocationFailed { .. } => AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::Layout.as_str(),
            },
            other => other,
        },
    )
}

fn fill_band(
    row: &mut StyledLine,
    band_start: usize,
    band_width: usize,
    value: char,
    role: AsciiColorRole,
    resources: &ResourceContext,
    checkpoints: &mut XyChartCheckpointCursor<'_>,
) -> Result<()> {
    for offset in 0..band_width {
        checkpoints.tick(OperationPhase::Layout)?;
        let col = resources.checked_grid_add(band_start, offset)?;
        row.try_set_role(col, value, role)?;
    }
    Ok(())
}

fn write_band_text(
    row: &mut StyledLine,
    band_start: usize,
    band_width: usize,
    value: &str,
    role: AsciiColorRole,
    resources: &ResourceContext,
) -> Result<()> {
    let fitted = fit_centered(value, band_width, row.width_profile(), resources)?;
    row.try_write_text_role(band_start, &fitted, role)
}

fn fit_centered(
    source: &str,
    width: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<String> {
    let mut value = String::new();
    let mut scan_resources = resources.clone();
    let metrics = visit_fitted_safe_line(
        source,
        width,
        width_profile,
        &mut scan_resources,
        |grapheme| {
            value
                .try_reserve(grapheme.len())
                .map_err(|_| AsciiError::AllocationFailed {
                    phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
                })?;
            value.push_str(grapheme);
            Ok(())
        },
    )?;
    let remaining = checked_grid_sub(resources, width, metrics.materialized_width)?;
    let left = remaining / 2;
    let right = checked_grid_sub(resources, remaining, left)?;
    let capacity = left
        .checked_add(value.len())
        .and_then(|capacity| capacity.checked_add(right))
        .ok_or_else(|| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    let mut fitted = String::new();
    fitted
        .try_reserve_exact(capacity)
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    fitted.extend(std::iter::repeat_n(' ', left));
    fitted.push_str(&value);
    fitted.extend(std::iter::repeat_n(' ', right));
    Ok(fitted)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FittedSafeLineMetrics {
    materialized_width: usize,
    retained_width: usize,
    truncated: bool,
}

fn visit_fitted_safe_line(
    value: &str,
    width: usize,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
    mut visit: impl FnMut(&str) -> Result<()>,
) -> Result<FittedSafeLineMetrics> {
    let mut materialized_width = 0usize;
    let mut retained_width = 0usize;
    let mut truncated = false;
    let policy = resources.policy();
    visit_safe_line_graphemes(
        resources,
        value,
        width_profile,
        |grapheme, grapheme_width| {
            let next_width = materialized_width
                .checked_add(grapheme_width)
                .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxGridCells))?;
            if next_width > width {
                truncated = true;
                return Ok(false);
            }
            visit(grapheme)?;
            materialized_width = next_width;
            if !grapheme.as_bytes().iter().all(|byte| *byte == b' ') {
                retained_width = materialized_width;
            }
            Ok(true)
        },
    )?;
    Ok(FittedSafeLineMetrics {
        materialized_width,
        retained_width,
        truncated,
    })
}

pub(super) fn checked_grid_sub(
    resources: &ResourceContext,
    left: usize,
    right: usize,
) -> Result<usize> {
    left.checked_sub(right).ok_or_else(|| {
        resources
            .policy()
            .overflow(AsciiResourceLimitId::MaxGridCells)
    })
}

fn bar_height(value: f64, range: ValueRange, height: usize) -> usize {
    (range.normalized(value) * height as f64).round() as usize
}

fn line_level(value: f64, range: ValueRange, height: usize) -> usize {
    bar_height(value, range, height).clamp(1, height)
}

pub(super) fn format_data_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    value.to_string()
}

pub(super) fn format_tick_number(value: f64, step: f64) -> String {
    if !value.is_finite() || !step.is_finite() {
        return format_data_number(value);
    }
    let magnitude = step.abs();
    if magnitude != 0.0 && magnitude < 1e-12 {
        return format_data_number(value);
    }
    let precision = if magnitude == 0.0 {
        0
    } else if magnitude >= 1.0 {
        usize::from((magnitude - magnitude.round()).abs() > 1e-9) * 2
    } else {
        ((-magnitude.log10()).ceil() as usize + 1).min(15)
    };
    let mut out = format!("{value:.precision$}");
    while out.contains('.') && out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.pop();
    }
    if out == "-0" { "0".to_string() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::AsciiResourcePolicy;
    use merman_core::{CancelReason, OperationControl};
    use std::cell::Cell;

    fn default_resources() -> AsciiResourcePolicy {
        AsciiResourcePolicy::default()
    }

    #[test]
    fn tick_formatter_keeps_representable_sub_epsilon_steps_distinct() {
        let step = f64::EPSILON;
        assert_ne!(
            format_tick_number(1.0, step),
            format_tick_number(1.0 + step, step)
        );
    }

    #[test]
    fn linear_samples_align_with_the_first_and_last_axis_band_centers() {
        let options = AsciiRenderOptions::ascii().with_xychart_category_band_width(4);
        let plot_area = XyChartPlotArea::from_options(&options);
        let resources = ResourceContext::new(default_resources());
        let axis = AxisPlan::Linear {
            range: ValueRange {
                min: 0.0,
                max: 10.0,
            },
        };
        let plot_width = plot_area
            .vertical_plot_width(2, &resources)
            .expect("two linear slots should fit");
        let datum = |normalized_x| SeriesDatum {
            x: String::new(),
            value: Some(0.0),
            has_point_label: false,
            authored_x: false,
            slot: 0,
            normalized_x: Some(normalized_x),
            x_clipped: false,
        };

        assert_eq!(
            axis.sample_column(&datum(0.0), 2, plot_width, plot_area, &resources)
                .expect("minimum x should map"),
            plot_area
                .vertical_band_center(0, &resources)
                .expect("first band center should fit")
        );
        assert_eq!(
            axis.sample_column(&datum(1.0), 2, plot_width, plot_area, &resources)
                .expect("maximum x should map"),
            plot_area
                .vertical_band_center(1, &resources)
                .expect("last band center should fit")
        );
    }

    #[test]
    fn write_band_text_preserves_wide_glyph_continuation_cells() {
        let options = AsciiRenderOptions::unicode();
        let policy = default_resources();
        let resources = ResourceContext::new(policy);
        let mut row = StyledLine::try_blank_with_policy(5, options.terminal_width_profile, policy)
            .expect("wide-glyph test row should fit the configured resource policy");

        write_band_text(&mut row, 1, 3, "中", AsciiColorRole::Text, &resources)
            .expect("wide glyph should fit the authored band");

        assert_eq!(row.len(), 5);
        assert_eq!(row.text(), " 中  ");
        assert_eq!(row.get(1), Some('中'));
        assert_eq!(row.get(2), None);
        assert_eq!(row.get(3), Some(' '));
        assert_eq!(row.get(4), Some(' '));
    }

    #[test]
    fn write_band_text_preserves_complex_grapheme_arena_entries() {
        let options = AsciiRenderOptions::unicode();
        let policy = default_resources();
        let resources = ResourceContext::new(policy);
        let mut row = StyledLine::try_blank_with_policy(6, options.terminal_width_profile, policy)
            .expect("ZWJ test row should fit the configured resource policy");

        write_band_text(&mut row, 1, 4, "👩‍💻", AsciiColorRole::Text, &resources)
            .expect("ZWJ grapheme should fit the authored band");

        assert_eq!(row.len(), 6);
        assert_eq!(row.text(), "  👩‍💻  ");
        assert_eq!(
            display_width_with_profile(&row.text(), TerminalWidthProfile::Unicode),
            6
        );
    }

    #[test]
    fn fit_centered_obeys_cjk_ambiguous_width() {
        let resources = ResourceContext::new(default_resources());
        assert_eq!(
            fit_centered("·", 3, TerminalWidthProfile::Unicode, &resources)
                .expect("Unicode-width label should fit"),
            " · "
        );
        assert_eq!(
            fit_centered("·", 3, TerminalWidthProfile::Cjk, &resources)
                .expect("CJK-width label should fit"),
            "· "
        );
    }

    #[test]
    fn fit_centered_escapes_terminal_controls_before_measurement() {
        let resources = ResourceContext::new(default_resources());
        let fitted = fit_centered("A\u{1b}B", 10, TerminalWidthProfile::Unicode, &resources)
            .expect("normalized control text should fit");

        assert!(fitted.contains("A\\u{1B}B"));
        assert!(!fitted.contains('\u{1b}'));
        assert_eq!(
            display_width_with_profile(&fitted, TerminalWidthProfile::Unicode),
            10
        );
    }

    #[test]
    fn band_axis_labels_materialize_only_after_exact_admission() {
        let policy = default_resources()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 4)
            .expect("the exact band-label work limit should be valid");
        let mut resources = ResourceContext::new(policy);
        let axis = AxisPlan::Band {
            authored_category_count: 1,
        };
        let categories = vec!["A".to_string()];
        let allocated = Cell::new(false);

        let labels =
            axis_labels_with_allocation_probe(&axis, &categories, 2, &mut resources, || {
                allocated.set(true)
            })
            .expect("the exact container and category-text admission should fit");

        assert!(allocated.get());
        assert_eq!(labels, ["A", "2"]);
        assert_eq!(resources.layout_work_used(), 4);
    }

    #[test]
    fn band_axis_labels_reject_large_text_before_first_allocation() {
        let policy = default_resources()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 2)
            .expect("the bounded band-label work limit should be valid");
        let mut resources = ResourceContext::new(policy);
        let axis = AxisPlan::Band {
            authored_category_count: 1,
        };
        let categories = vec!["A".repeat(256 * 1_024)];
        let allocated = Cell::new(false);

        let error =
            axis_labels_with_allocation_probe(&axis, &categories, 1, &mut resources, || {
                allocated.set(true)
            })
            .expect_err("the category scan must reject before reserving or copying labels");

        assert!(!allocated.get());
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, 3);
        assert_eq!(details.max, 2);
        assert_eq!(resources.layout_work_used(), 2);
    }

    #[test]
    fn band_axis_labels_check_grapheme_limit_before_first_allocation() {
        let policy = default_resources()
            .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 1)
            .expect("the bounded grapheme limit should be valid");
        let mut resources = ResourceContext::new(policy);
        let axis = AxisPlan::Band {
            authored_category_count: 1,
        };
        let categories = vec!["é".to_string()];
        let allocated = Cell::new(false);

        let error =
            axis_labels_with_allocation_probe(&axis, &categories, 1, &mut resources, || {
                allocated.set(true)
            })
            .expect_err("the grapheme ceiling must reject before reserving or copying labels");

        assert!(!allocated.get());
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a grapheme resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxGraphemeBytes);
        assert_eq!(details.actual, 2);
        assert_eq!(details.max, 1);
    }

    #[test]
    fn band_axis_label_admission_prioritizes_cancellation_before_allocation() {
        let policy = default_resources()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("the bounded band-label work limit should be valid");
        let base_resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel();
        let mut resources = base_resources.controlled(control, OperationPhase::Layout);
        let axis = AxisPlan::Band {
            authored_category_count: 1,
        };
        let categories = vec!["A".repeat(256 * 1_024)];
        let allocated = Cell::new(false);

        let error =
            axis_labels_with_allocation_probe(&axis, &categories, 2, &mut resources, || {
                allocated.set(true)
            })
            .expect_err("cancellation must win over the container work ceiling");

        assert!(!allocated.get());
        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }

    #[test]
    fn retained_band_width_stops_after_the_first_overflowing_grapheme() {
        let policy = default_resources()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 3)
            .expect("the exact bounded scan limit should be valid");
        let mut resources = ResourceContext::new(policy);
        let label = format!("{}\u{1b}", "A".repeat(4_096));
        let plot_area = XyChartPlotArea::from_options(
            &AsciiRenderOptions::ascii().with_xychart_category_band_width(1),
        );

        let retained = plot_area
            .band_labels_retained_width(&[label], &mut resources)
            .expect("the retained-width scan should stop after one fitted cell");

        assert_eq!(retained, 1);
        assert_eq!(resources.layout_work_used(), 3);
    }

    #[test]
    fn retained_cell_mask_large_fixture_has_linear_work_boundary() {
        const WIDTH: usize = 600;
        const HEIGHT: usize = 10;
        const REQUIRED_WORK: usize = 18_600;

        let run = |limit| -> Result<(Vec<usize>, usize)> {
            let policy = default_resources()
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
                .expect("the retained-mask work limit should be valid");
            let mut resources = ResourceContext::new(policy);
            let mut checkpoints = XyChartCheckpointCursor::new(AsciiExecution::for_test(&policy));
            let extent = LogicalExtent::checked(WIDTH, HEIGHT, policy)?;
            let mut mask = RetainedCellMask::try_new(extent, &mut resources, &mut checkpoints)?;

            for col in 0..WIDTH {
                for row in 0..HEIGHT {
                    mask.fill(row, col, 1, &mut resources, &mut checkpoints)?;
                }
            }
            for col in 0..WIDTH {
                mask.overwrite(
                    0,
                    RetainedInterval {
                        start: col,
                        end: col + 1,
                    },
                    None,
                    &mut resources,
                    &mut checkpoints,
                )?;
            }

            let mut widths = vec![0; HEIGHT];
            mask.write_row_widths(&mut widths, &mut resources, &mut checkpoints)?;
            Ok((widths, resources.layout_work_used()))
        };

        let (widths, used) = run(REQUIRED_WORK).expect("the exact linear work limit should fit");
        assert_eq!(widths[0], 0);
        assert!(widths[1..].iter().all(|width| *width == WIDTH));
        assert_eq!(used, REQUIRED_WORK);

        let error = run(REQUIRED_WORK - 1).expect_err("N-1 must reject the final linear scan");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a resource-limit error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, REQUIRED_WORK);
        assert_eq!(details.max, REQUIRED_WORK - 1);
    }

    #[test]
    fn disclosure_stops_after_the_first_overflowing_category_grapheme() {
        let policy = default_resources()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 5)
            .expect("the exact disclosure scan limit should be valid");
        let mut resources = ResourceContext::new(policy);
        let category = format!("{}\u{1b}", "A".repeat(4_096));
        let plan = TerminalChartPlan {
            x_axis: AxisPlan::Band {
                authored_category_count: 1,
            },
            x_linear_domain: None,
            y_range: ValueRange { min: 0.0, max: 1.0 },
            y_linear_domain: LinearDomainPlan {
                authored_min: Some(0.0),
                authored_max: Some(1.0),
                resolved: ValueRange { min: 0.0, max: 1.0 },
            },
            series: Vec::new(),
            category_labels: vec![category],
            horizontal_axis_labels: Vec::new(),
            slot_count: 1,
            bar_series_count: 0,
            line_series_count: 0,
            has_authored_x: false,
        };
        let plot_area = XyChartPlotArea::from_options(
            &AsciiRenderOptions::ascii().with_xychart_category_band_width(1),
        );

        let disclosure = plan
            .disclosure_plan(plot_area, false, true, true, &mut resources)
            .expect("the disclosure scan should stop at the first overflowing grapheme");

        assert!(disclosure.values);
        assert!(disclosure.band_domain);
        assert_eq!(resources.layout_work_used(), 5);
    }

    #[test]
    fn topology_initialization_observes_cancellation_inside_the_cell_loop() {
        let policy = default_resources();
        let mut resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);
        let mut checkpoints = XyChartCheckpointCursor::new(AsciiExecution::new(&control, &policy));

        let error = LineTopology::new(65, 1, &mut resources, &mut checkpoints)
            .expect_err("the second cadence checkpoint must stop topology initialization");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
    }

    #[test]
    fn topology_initialization_admits_source_and_mask_exactly() {
        let exact_policy = default_resources()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 42)
            .expect("the exact topology grid limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        let mut exact_checkpoints =
            XyChartCheckpointCursor::new(AsciiExecution::for_test(&exact_policy));
        LineTopology::new(7, 3, &mut exact_resources, &mut exact_checkpoints)
            .expect("the exact source-plus-mask grid should fit");

        let below_policy = default_resources()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 41)
            .expect("the N-1 topology grid limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        let mut below_checkpoints =
            XyChartCheckpointCursor::new(AsciiExecution::for_test(&below_policy));
        let error = LineTopology::new(7, 3, &mut below_resources, &mut below_checkpoints)
            .expect_err("N-1 must reject before the topology mask is allocated");

        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a resource-limit error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
        assert_eq!(details.actual, 42);
        assert_eq!(details.max, 41);
    }

    #[test]
    fn topology_connect_observes_cancellation_inside_a_long_segment() {
        let policy = default_resources();
        let mut resources = ResourceContext::new(policy);
        let mut setup = XyChartCheckpointCursor::new(AsciiExecution::for_test(&policy));
        let mut topology = LineTopology::new(66, 1, &mut resources, &mut setup)
            .expect("the topology fixture should allocate");
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);
        let mut checkpoints = XyChartCheckpointCursor::new(AsciiExecution::new(&control, &policy));

        let error = topology
            .connect((0, 0), (0, 65), &mut resources, &mut checkpoints)
            .expect_err("the second cadence checkpoint must stop segment routing");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
    }

    #[test]
    fn band_label_projection_observes_cancellation_inside_a_long_domain() {
        let policy = default_resources();
        let mut resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);
        let mut checkpoints = XyChartCheckpointCursor::new(AsciiExecution::new(&control, &policy));
        let labels = vec!["A"; 65];
        let plot_area = XyChartPlotArea::from_options(&AsciiRenderOptions::ascii());

        let error = plot_area
            .band_labels(&labels, &mut resources, &mut checkpoints)
            .expect_err("the second cadence checkpoint must stop Band label projection");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Emit
                    && cancelled.reason == CancelReason::Requested
        ));
    }

    #[test]
    fn cjk_profile_uses_single_cell_ascii_plot_structure() {
        let options =
            AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk);
        let chars = ChartChars::from_options(&options);
        let plot_area = XyChartPlotArea::from_options(&options);
        let policy = default_resources();
        let mut resources = ResourceContext::new(policy);
        let mut checkpoints = XyChartCheckpointCursor::new(AsciiExecution::for_test(&policy));
        let mut row = StyledLine::try_blank_with_policy(
            plot_area.horizontal_width,
            TerminalWidthProfile::Cjk,
            policy,
        )
        .expect("CJK plot row should fit the configured resource policy");

        draw_horizontal_bar_value(
            &mut row,
            5.0,
            0,
            ValueRange {
                min: 0.0,
                max: 10.0,
            },
            chars,
            plot_area,
            &mut resources,
            &mut checkpoints,
        )
        .expect("horizontal bar paint should fit the configured work budget");

        assert_eq!(chars.bar, '#');
        assert_eq!(row.len(), 10);
        assert_eq!(row.text().chars().filter(|ch| *ch == '#').count(), 5);
        assert_eq!(row.get(0), Some('#'));
        assert_eq!(row.get(1), Some('#'));
        assert_eq!(
            display_width_with_profile(&row.text(), TerminalWidthProfile::Cjk),
            10
        );
    }
}

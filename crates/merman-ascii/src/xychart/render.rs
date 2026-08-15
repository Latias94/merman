mod disclosure;
mod empty;

use super::plot::{
    ChartChars, HorizontalPlotAdmissionPlan, SeriesPlan, TerminalChartPlan, ValueRange,
    VerticalPlotAdmissionPlan, XyChartCheckpointCursor, XyChartPlotArea,
    apply_vertical_bar_data_labels, build_horizontal_plot_rows, build_vertical_plot,
    checked_grid_sub, format_data_number, format_tick_number, horizontal_bar_width,
    plan_horizontal_plot_admission, plan_vertical_plot_admission,
};
use crate::canvas::finish_styled_lines_with_resources_with_execution;
use crate::color::{AsciiColorMode, AsciiColorRole};
use crate::error::AsciiError;
use crate::operation::AsciiExecution;
use crate::options::TerminalWidthProfile;
#[cfg(test)]
use crate::resource::AsciiResourcePolicy;
use crate::resource::{AsciiResourceLimitPhase, LogicalExtent, ResourceContext};
use crate::safe_text::{
    LabelBreakPolicy, charge_text_layout, try_plan_normalized_label_lines_with_policy,
};
use crate::text::{StyledLine, display_width_with_profile, truncate_display_width_with_profile};
use crate::{AsciiRenderOptions, Result};
use disclosure::{
    TitleOwner, band_domain_disclosure_line_width, linear_domain_disclosure_line_widths,
    push_title_display_line, push_value_disclosure_lines, title_display_line_width,
    value_disclosure_line_width,
};
use merman_core::diagrams::xychart::{
    XyChartAxisDisplayPolicy, XyChartAxisRenderModel, XyChartDiagramRenderModel, XyChartPlotType,
};
type ChartLine = StyledLine;
type CategoryLabel = String;

#[derive(Debug, Default)]
struct ChartDocument {
    lines: Vec<ChartLine>,
    width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlotDocumentMetrics {
    width: usize,
    materialized_width: usize,
    document_cells: usize,
}

#[derive(Debug, Clone, Copy)]
struct HorizontalPlotMetricsContext<'a> {
    plan: &'a TerminalChartPlan,
    plot_prefix_width: usize,
    plot_area: XyChartPlotArea,
    show_axis_labels: bool,
    gutter: usize,
    axis_mark: Option<char>,
    preserve_color: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ChartDocumentPlan {
    width: usize,
    materialized_width: usize,
    height: usize,
    document_cells: usize,
    transient_grid_cells: usize,
}

impl ChartDocumentPlan {
    fn include_line(&mut self, width: usize, resources: &ResourceContext) -> Result<()> {
        self.include_materialized_line(width, width, resources)
    }

    fn include_materialized_block(
        &mut self,
        width: usize,
        materialized_width: usize,
        height: usize,
        document_cells: usize,
        resources: &ResourceContext,
    ) -> Result<()> {
        if height == 0 {
            return Ok(());
        }
        self.width = self.width.max(width);
        self.materialized_width = self.materialized_width.max(materialized_width);
        self.height = resources.checked_grid_add(self.height, height)?;
        self.document_cells =
            checked_document_cells_add(self.document_cells, document_cells, resources)?;
        Ok(())
    }

    fn include_materialized_line(
        &mut self,
        width: usize,
        materialized_width: usize,
        resources: &ResourceContext,
    ) -> Result<()> {
        self.include_materialized_block(width, materialized_width, 1, width, resources)
    }

    fn include_transient_grid(&mut self, cells: usize, resources: &ResourceContext) -> Result<()> {
        self.transient_grid_cells = resources.checked_grid_add(self.transient_grid_cells, cells)?;
        Ok(())
    }

    fn admit_grid(&self, resources: &ResourceContext) -> Result<LogicalExtent> {
        resources.grid_extent(self.width, self.height)?;
        let materialized_extent = resources.grid_extent(self.materialized_width, self.height)?;
        let concurrent_cells =
            resources.checked_grid_add(self.transient_grid_cells, materialized_extent.cells())?;
        resources.grid_extent(concurrent_cells, 1)?;
        Ok(materialized_extent)
    }

    fn materialize(
        self,
        materialized_extent: LogicalExtent,
        resources: &mut ResourceContext,
        before_materialize: impl FnOnce(),
        materialize: impl FnOnce(&mut ResourceContext) -> Result<ChartDocument>,
    ) -> Result<ChartDocument> {
        let plan = self;
        resources.transaction(|resources| {
            resources.charge_usage(0, plan.document_cells)?;
            before_materialize();
            // Materialization builds complete StyledLine values and therefore performs its own
            // document-cell charges. Keep those charges in a disposable document scope: the
            // plan's exact aggregate was admitted above and is committed only once.
            let mut materialize_resources =
                resources.scoped_after_document_admission(materialized_extent.cells())?;
            let document = materialize(&mut materialize_resources)?;
            if document.width != plan.materialized_width || document.lines.len() != plan.height {
                return Err(invalid_chart_document_extent_plan());
            }
            Ok(document)
        })
    }
}

fn checked_document_cells_add(
    current: usize,
    delta: usize,
    resources: &ResourceContext,
) -> Result<usize> {
    current
        .checked_add(delta)
        .ok_or_else(|| resources.overflow(crate::resource::AsciiResourceLimitId::MaxDocumentCells))
}

impl ChartDocument {
    fn push(&mut self, line: ChartLine, resources: &ResourceContext) -> Result<()> {
        let height = resources.checked_grid_add(self.lines.len(), 1)?;
        let width = self.width.max(line.len());
        resources.grid_extent(width, height)?;
        self.lines
            .try_reserve(1)
            .map_err(|_| allocation_failed(AsciiResourceLimitPhase::LayoutWork))?;
        self.lines.push(line);
        self.width = width;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct ChartRenderContext<'a> {
    y_range: ValueRange,
    chars: ChartChars,
    plot_area: XyChartPlotArea,
    plot_extent: LogicalExtent,
    options: &'a AsciiRenderOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChartOrientation {
    Vertical,
    Horizontal,
}

impl ChartOrientation {
    const fn is_horizontal(self) -> bool {
        matches!(self, Self::Horizontal)
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Vertical => "vertical",
            Self::Horizontal => "horizontal",
        }
    }
}

pub(crate) fn render_xychart_diagram_with_execution(
    model: &XyChartDiagramRenderModel,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    render_xychart_diagram_controlled(model, options, execution, || {})
}

#[cfg(test)]
pub(crate) fn render_xychart_diagram_with_resources(
    model: &XyChartDiagramRenderModel,
    options: &AsciiRenderOptions,
    resources: AsciiResourcePolicy,
) -> Result<String> {
    render_xychart_diagram_controlled(model, options, AsciiExecution::for_test(&resources), || {})
}

#[cfg(test)]
fn render_xychart_diagram_with_materializer(
    model: &XyChartDiagramRenderModel,
    options: &AsciiRenderOptions,
    resources: AsciiResourcePolicy,
    before_document_materialize: impl FnOnce(),
) -> Result<String> {
    render_xychart_diagram_controlled(
        model,
        options,
        AsciiExecution::for_test(&resources),
        before_document_materialize,
    )
}

fn render_xychart_diagram_controlled(
    model: &XyChartDiagramRenderModel,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
    before_document_materialize: impl FnOnce(),
) -> Result<String> {
    options.validate()?;
    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    let orientation = validate_xychart_model(model)?;
    let base_resources = ResourceContext::new(*execution.resources());
    let mut resources =
        execution.resource_context(&base_resources, merman_core::OperationPhase::Layout);
    if model.plots.is_empty() {
        execution.checkpoint(merman_core::OperationPhase::Emit)?;
        let emit_resources =
            execution.resource_context(&resources, merman_core::OperationPhase::Emit);
        let rendered = empty::render(model, orientation, options, emit_resources)?;
        checkpoint_emitted_lines(&rendered, execution)?;
        return Ok(rendered);
    }
    let cardinality = TerminalChartPlan::measure_cardinality(model, &mut resources)?;
    if cardinality.is_empty() {
        execution.checkpoint(merman_core::OperationPhase::Emit)?;
        let emit_resources =
            execution.resource_context(&resources, merman_core::OperationPhase::Emit);
        let rendered = empty::render(model, orientation, options, emit_resources)?;
        checkpoint_emitted_lines(&rendered, execution)?;
        return Ok(rendered);
    }

    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    let plan = TerminalChartPlan::build(model, cardinality, &mut resources)?;

    let chars = ChartChars::from_options(options);
    let plot_area = XyChartPlotArea::from_options(options);
    let horizontal = orientation.is_horizontal();
    let plot_extent = if horizontal {
        plot_area.horizontal_plot_extent(plan.horizontal_row_count(&resources)?, &resources)?
    } else {
        plot_area.vertical_plot_extent(plan.slot_count, &resources)?
    };
    let plot_cells = plot_extent.width().saturating_mul(plot_extent.height());
    execution.admit_grid(plot_cells)?;
    let context = ChartRenderContext {
        y_range: plan.y_range,
        chars,
        plot_area,
        plot_extent,
        options,
    };

    if horizontal {
        let mut checkpoints = XyChartCheckpointCursor::new(execution);
        return render_horizontal(
            model,
            &plan,
            context,
            &mut resources,
            execution,
            &mut checkpoints,
            before_document_materialize,
        );
    }

    let mut checkpoints = XyChartCheckpointCursor::new(execution);
    render_vertical(
        model,
        &plan,
        context,
        &mut resources,
        execution,
        &mut checkpoints,
        before_document_materialize,
    )
}

fn validate_xychart_model(model: &XyChartDiagramRenderModel) -> Result<ChartOrientation> {
    let orientation =
        if model.orientation.is_empty() || model.orientation.eq_ignore_ascii_case("vertical") {
            ChartOrientation::Vertical
        } else if model.orientation.eq_ignore_ascii_case("horizontal") {
            ChartOrientation::Horizontal
        } else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "xychart",
                feature: "chart orientation",
            });
        };

    if matches!(model.y_axis, XyChartAxisRenderModel::Band { .. }) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "xychart",
            feature: "band y-axis",
        });
    }
    Ok(orientation)
}

fn render_vertical(
    model: &XyChartDiagramRenderModel,
    plan: &TerminalChartPlan,
    context: ChartRenderContext<'_>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
    checkpoints: &mut XyChartCheckpointCursor<'_>,
    before_document_materialize: impl FnOnce(),
) -> Result<String> {
    let ChartRenderContext {
        y_range,
        chars,
        plot_area,
        plot_extent,
        options,
    } = context;
    let width_profile = options.terminal_width_profile;
    let categories = &plan.category_labels;
    let mut document_resources = resources.scoped();
    let resources = &mut document_resources;
    let show_y_labels = axis_labels_visible(model.display.y_axis);
    let mut disclosure = plan.disclosure_plan(
        plot_area,
        false,
        axis_labels_visible(model.display.x_axis),
        show_y_labels,
        resources,
    )?;
    disclosure.values |= model.display.show_data_label && !uses_compact_bar_data_labels(model);
    let requires_disclosure = disclosure.is_required();
    let (tick_labels, min_label, gutter) = if show_y_labels {
        let tick_labels = vertical_tick_labels(y_range, plot_area, resources)?;
        let min_label = Some(format_tick_number(
            y_range.min,
            y_range.span() / plot_area.vertical_height as f64,
        ));
        let gutter = label_gutter(
            min_label
                .iter()
                .map(String::as_str)
                .chain(tick_labels.iter().map(String::as_str)),
            width_profile,
            resources,
        )?;
        (tick_labels, min_label, gutter)
    } else {
        (empty_labels(plot_area.vertical_height)?, None, 0)
    };
    let y_axis_mark = vertical_axis_mark(model.display.y_axis, chars);
    let baseline_mark = if model.display.x_axis.show_axis_line || model.display.x_axis.show_tick {
        Some(chars.origin)
    } else {
        y_axis_mark
    };
    let reserve_axis_slot = show_y_labels || y_axis_mark.is_some() || baseline_mark.is_some();
    let plot_prefix_width = plot_prefix_width(show_y_labels, reserve_axis_slot, gutter, resources)?;
    let compact_inside_labels = model.display.show_data_label
        && uses_compact_bar_data_labels(model)
        && !model.display.show_data_label_outside_bar;
    let outside_data_labels = if model.display.show_data_label
        && uses_compact_bar_data_labels(model)
        && model.display.show_data_label_outside_bar
    {
        compact_bar_value_labels(model, plan, resources)?
    } else {
        None
    };
    let vertical_plan = plan_vertical_plot_admission(
        plan,
        plot_area,
        plot_extent,
        compact_inside_labels,
        options.color_mode != AsciiColorMode::Plain,
        resources,
        checkpoints,
    )?;
    let mut document_plan = measure_vertical_document(
        model,
        plan,
        chars,
        &vertical_plan,
        plot_extent,
        plot_prefix_width,
        &tick_labels,
        gutter,
        show_y_labels,
        y_axis_mark,
        baseline_mark,
        outside_data_labels.as_deref(),
        disclosure,
        options,
        resources,
    )?;
    document_plan.include_transient_grid(plot_extent.cells(), resources)?;
    let materialized_extent = document_plan.admit_grid(resources)?;
    execution.checkpoint(merman_core::OperationPhase::Emit)?;
    let mut emit_resources =
        execution.resource_context(resources, merman_core::OperationPhase::Emit);
    let out = document_plan.materialize(
        materialized_extent,
        &mut emit_resources,
        before_document_materialize,
        |resources| {
            let mut plot_resources = resources
                .with_operation_phase(merman_core::OperationPhase::Layout)
                .scoped();
            let mut plot = build_vertical_plot(
                plan,
                chars,
                plot_area,
                plot_extent,
                &mut plot_resources,
                checkpoints,
            )?;
            let mut out = ChartDocument::default();
            push_title_lines(&mut out, model, options, resources)?;
            push_legend_line(&mut out, plan, chars, options, resources)?;
            if requires_disclosure {
                push_value_disclosure_lines(
                    &mut out, model, plan, chars, disclosure, options, resources,
                )?;
            }

            if model.display.show_data_label && uses_compact_bar_data_labels(model) {
                if model.display.show_data_label_outside_bar {
                    if let Some(labels) = outside_data_labels.as_deref()
                        && let Some(line) = vertical_data_label_line(
                            labels,
                            plot_prefix_width,
                            plot_area,
                            resources,
                            checkpoints,
                        )?
                    {
                        out.push(line, resources)?;
                    }
                } else {
                    apply_vertical_bar_data_labels(
                        &mut plot,
                        plan,
                        plot_area,
                        &mut plot_resources,
                        checkpoints,
                    )?;
                }
            }

            for (idx, row) in plot.rows.into_iter().enumerate() {
                checkpoints.tick(merman_core::OperationPhase::Emit)?;
                let label = &tick_labels[idx];
                let mut line = new_chart_line(options, resources);
                push_axis_prefix(
                    &mut line,
                    label,
                    gutter,
                    show_y_labels,
                    y_axis_mark,
                    reserve_axis_slot,
                    resources,
                )?;
                let mut checkpoint = || checkpoints.tick(merman_core::OperationPhase::Emit);
                line.try_push_line_with_checkpoint(&row, &plot_resources, &mut checkpoint)?;
                out.push(line, resources)?;
            }

            if show_y_labels || baseline_mark.is_some() {
                let mut axis_line = new_chart_line(options, resources);
                push_axis_baseline_prefix(
                    &mut axis_line,
                    min_label.as_deref().unwrap_or_default(),
                    gutter,
                    show_y_labels,
                    baseline_mark,
                    reserve_axis_slot,
                    resources,
                )?;
                if model.display.x_axis.show_axis_line {
                    resources.charge_layout_work(plot.width)?;
                    axis_line.try_push_role_repeat(
                        chars.horizontal_axis,
                        plot.width,
                        AsciiColorRole::ChartAxis,
                    )?;
                } else if model.display.x_axis.show_tick {
                    resources.charge_layout_work(plot.width)?;
                    axis_line.try_push_spaces(plot.width)?;
                }
                if model.display.x_axis.show_tick {
                    overlay_vertical_category_ticks(
                        &mut axis_line,
                        plot_prefix_width,
                        categories.len(),
                        plot_area,
                        chars.horizontal_tick,
                        resources,
                        checkpoints,
                    )?;
                }
                out.push(axis_line, resources)?;
            }

            if axis_labels_visible(model.display.x_axis) {
                charge_category_text(categories, resources)?;
                let labels = plot_area.category_axis_labels(categories, resources, checkpoints)?;
                let mut category_line = new_chart_line(options, resources);
                resources.charge_layout_work(plot_prefix_width)?;
                category_line.try_push_spaces(plot_prefix_width)?;
                category_line.try_push_role_text_with_unstyled_trailing_spaces(
                    &labels,
                    AsciiColorRole::Text,
                )?;
                out.push(category_line, resources)?;
            }

            if model.display.x_axis.show_title
                && let Some(title) = nonempty_axis_title(&model.x_axis)
            {
                push_title_display_line(&mut out, TitleOwner::XAxis, title, options, resources)?;
            }
            Ok(out)
        },
    )?;

    finish_chart_lines_controlled(out, options, &mut emit_resources, execution)
}

fn render_horizontal(
    model: &XyChartDiagramRenderModel,
    plan: &TerminalChartPlan,
    context: ChartRenderContext<'_>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
    checkpoints: &mut XyChartCheckpointCursor<'_>,
    before_document_materialize: impl FnOnce(),
) -> Result<String> {
    let ChartRenderContext {
        y_range,
        chars,
        plot_area,
        plot_extent,
        options,
    } = context;
    let width_profile = options.terminal_width_profile;
    let categories = &plan.horizontal_axis_labels;
    let mut document_resources = resources.scoped();
    let resources = &mut document_resources;
    let show_x_labels = axis_labels_visible(model.display.x_axis);
    let show_y_labels = axis_labels_visible(model.display.y_axis);
    let mut disclosure =
        plan.disclosure_plan(plot_area, true, show_x_labels, show_y_labels, resources)?;
    disclosure.values |= model.display.show_data_label && !uses_compact_bar_data_labels(model);
    let requires_disclosure = disclosure.is_required();
    let gutter = if show_x_labels {
        label_gutter(
            categories.iter().map(String::as_str),
            width_profile,
            resources,
        )?
    } else {
        0
    };
    let x_axis_mark = vertical_axis_mark(model.display.x_axis, chars);
    let baseline_mark = if model.display.y_axis.show_axis_line || model.display.y_axis.show_tick {
        Some(chars.origin)
    } else {
        x_axis_mark
    };
    let reserve_axis_slot = show_x_labels || x_axis_mark.is_some() || baseline_mark.is_some();
    let plot_prefix_width = plot_prefix_width(show_x_labels, reserve_axis_slot, gutter, resources)?;
    let horizontal_plan = plan_horizontal_plot_admission(
        plan,
        plot_area,
        plot_extent.height(),
        resources,
        checkpoints,
    )?;
    let metrics_context = HorizontalPlotMetricsContext {
        plan,
        plot_prefix_width,
        plot_area,
        show_axis_labels: show_x_labels,
        gutter,
        axis_mark: x_axis_mark,
        preserve_color: options.color_mode != AsciiColorMode::Plain,
    };
    let mut document_plan = measure_horizontal_document(
        model,
        context,
        metrics_context,
        &horizontal_plan,
        baseline_mark,
        disclosure,
        resources,
    )?;
    document_plan.include_transient_grid(plot_extent.cells(), resources)?;
    let compact_bar_values = horizontal_plan.into_compact_bar_values();
    let materialized_extent = document_plan.admit_grid(resources)?;
    execution.checkpoint(merman_core::OperationPhase::Emit)?;
    let mut emit_resources =
        execution.resource_context(resources, merman_core::OperationPhase::Emit);
    let out = document_plan.materialize(
        materialized_extent,
        &mut emit_resources,
        before_document_materialize,
        |resources| {
            let mut plot_resources = resources
                .with_operation_phase(merman_core::OperationPhase::Layout)
                .scoped();
            let plot_rows = build_horizontal_plot_rows(
                plan,
                chars,
                plot_area,
                plot_extent,
                compact_bar_values.as_ref(),
                &mut plot_resources,
                checkpoints,
            )?;
            let mut out = ChartDocument::default();
            push_title_lines(&mut out, model, options, resources)?;
            push_legend_line(&mut out, plan, chars, options, resources)?;
            if requires_disclosure {
                push_value_disclosure_lines(
                    &mut out, model, plan, chars, disclosure, options, resources,
                )?;
            }

            for plot_row in &plot_rows {
                checkpoints.tick(merman_core::OperationPhase::Emit)?;
                resources.charge_layout_work(1)?;
                let category = plot_row
                    .show_category_label
                    .then(|| categories.get(plot_row.category_index))
                    .flatten()
                    .map(String::as_str)
                    .unwrap_or_default();
                let mut line = new_chart_line(options, resources);
                push_axis_prefix(
                    &mut line,
                    category,
                    gutter,
                    show_x_labels,
                    x_axis_mark,
                    reserve_axis_slot,
                    resources,
                )?;
                let mut checkpoint = || checkpoints.tick(merman_core::OperationPhase::Emit);
                line.try_push_line_with_checkpoint(
                    &plot_row.line,
                    &plot_resources,
                    &mut checkpoint,
                )?;
                if model.display.show_data_label
                    && uses_compact_bar_data_labels(model)
                    && let (Some(value), Some(label)) =
                        (plot_row.bar_value, plot_row.bar_label.as_deref())
                {
                    let written_inside = if model.display.show_data_label_outside_bar {
                        false
                    } else {
                        write_horizontal_inside_data_label(
                            &mut line,
                            plot_prefix_width,
                            label,
                            value,
                            y_range,
                            plot_area,
                            resources,
                        )?
                    };
                    if !written_inside {
                        push_horizontal_outside_data_label(&mut line, label, resources)?;
                    }
                }
                out.push(line, resources)?;
            }

            if axis_labels_visible(model.display.y_axis) || baseline_mark.is_some() {
                let mut axis_line = new_chart_line(options, resources);
                push_axis_baseline_prefix(
                    &mut axis_line,
                    "",
                    gutter,
                    show_x_labels,
                    baseline_mark,
                    reserve_axis_slot,
                    resources,
                )?;
                if model.display.y_axis.show_axis_line {
                    resources.charge_layout_work(plot_area.horizontal_width)?;
                    axis_line.try_push_role_repeat(
                        chars.horizontal_axis,
                        plot_area.horizontal_width,
                        AsciiColorRole::ChartAxis,
                    )?;
                } else if model.display.y_axis.show_tick {
                    resources.charge_layout_work(plot_area.horizontal_width)?;
                    axis_line.try_push_spaces(plot_area.horizontal_width)?;
                }
                if model.display.y_axis.show_tick {
                    overlay_horizontal_value_ticks(
                        &mut axis_line,
                        plot_prefix_width,
                        plot_area,
                        chars.horizontal_tick,
                        resources,
                        checkpoints,
                    )?;
                }
                out.push(axis_line, resources)?;
            }

            if axis_labels_visible(model.display.y_axis) {
                let tick_labels = horizontal_tick_label_line(y_range, plot_area, resources)?;
                let mut tick_line = new_chart_line(options, resources);
                resources.charge_layout_work(plot_prefix_width)?;
                tick_line.try_push_spaces(plot_prefix_width)?;
                let mut checkpoint = || checkpoints.tick(merman_core::OperationPhase::Emit);
                tick_line.try_push_line_with_checkpoint(
                    &tick_labels,
                    &plot_resources,
                    &mut checkpoint,
                )?;
                out.push(tick_line, resources)?;
            }

            if model.display.x_axis.show_title
                && let Some(title) = nonempty_axis_title(&model.x_axis)
            {
                push_title_display_line(&mut out, TitleOwner::XAxis, title, options, resources)?;
            }
            Ok(out)
        },
    )?;

    finish_chart_lines_controlled(out, options, &mut emit_resources, execution)
}

#[allow(clippy::too_many_arguments)]
fn measure_vertical_document(
    model: &XyChartDiagramRenderModel,
    plan: &TerminalChartPlan,
    chars: ChartChars,
    vertical_plan: &VerticalPlotAdmissionPlan,
    plot_extent: LogicalExtent,
    plot_prefix_width: usize,
    tick_labels: &[String],
    gutter: usize,
    show_y_labels: bool,
    y_axis_mark: Option<char>,
    baseline_mark: Option<char>,
    outside_data_labels: Option<&[String]>,
    disclosure: super::plot::TerminalDisclosurePlan,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<ChartDocumentPlan> {
    let mut document = ChartDocumentPlan::default();
    let plot_area = XyChartPlotArea::from_options(options);
    measure_chart_header(&mut document, model, plan, chars, options, resources)?;
    if disclosure.is_required() {
        measure_value_disclosure_lines(
            &mut document,
            model,
            plan,
            chars,
            disclosure,
            options,
            resources,
        )?;
    }

    let plot_row_width = resources.checked_grid_add(plot_prefix_width, plot_extent.width())?;
    if let Some(labels) = outside_data_labels
        && let Some(retained_width) = vertical_data_label_line_retained_width(
            labels,
            plot_prefix_width,
            plot_area,
            resources,
        )?
    {
        document.include_materialized_line(retained_width, plot_row_width, resources)?;
    }
    let plot_metrics = vertical_plot_document_metrics(
        vertical_plan,
        tick_labels,
        plot_prefix_width,
        gutter,
        show_y_labels,
        y_axis_mark,
        options.color_mode != AsciiColorMode::Plain,
        options.terminal_width_profile,
        plot_extent,
        resources,
    )?;
    document.include_materialized_block(
        plot_metrics.width,
        plot_metrics.materialized_width,
        plot_extent.height(),
        plot_metrics.document_cells,
        resources,
    )?;

    if show_y_labels || baseline_mark.is_some() {
        let retained_prefix_width = if baseline_mark.is_some() {
            plot_prefix_width
        } else if show_y_labels {
            gutter
        } else {
            0
        };
        let baseline_width = vertical_axis_baseline_retained_width(
            model.display.x_axis,
            plan.category_labels.len(),
            plot_prefix_width,
            plot_row_width,
            plot_area,
            retained_prefix_width,
            resources,
        )?;
        document.include_line(baseline_width, resources)?;
    }
    if axis_labels_visible(model.display.x_axis) {
        let labels_width =
            plot_area.band_labels_retained_width(&plan.category_labels, resources)?;
        let retained_width = if labels_width == 0 {
            0
        } else {
            resources.checked_grid_add(plot_prefix_width, labels_width)?
        };
        document.include_materialized_line(retained_width, plot_row_width, resources)?;
    }
    if model.display.x_axis.show_title
        && let Some(title) = nonempty_axis_title(&model.x_axis)
    {
        let width = title_display_line_width(TitleOwner::XAxis, title, options, resources)?;
        document.include_line(width, resources)?;
    }
    Ok(document)
}

fn vertical_axis_baseline_retained_width(
    x_axis: XyChartAxisDisplayPolicy,
    category_count: usize,
    plot_prefix_width: usize,
    plot_row_width: usize,
    plot_area: XyChartPlotArea,
    retained_prefix_width: usize,
    resources: &ResourceContext,
) -> Result<usize> {
    if x_axis.show_axis_line {
        return Ok(plot_row_width);
    }
    if !x_axis.show_tick || category_count == 0 {
        return Ok(retained_prefix_width);
    }

    let last_center = plot_area.vertical_band_center(category_count - 1, resources)?;
    let retained_plot_width = resources.checked_grid_add(last_center, 1)?;
    resources.checked_grid_add(plot_prefix_width, retained_plot_width)
}

fn measure_horizontal_document(
    model: &XyChartDiagramRenderModel,
    chart: ChartRenderContext<'_>,
    metrics: HorizontalPlotMetricsContext<'_>,
    horizontal_plan: &HorizontalPlotAdmissionPlan,
    baseline_mark: Option<char>,
    disclosure: super::plot::TerminalDisclosurePlan,
    resources: &mut ResourceContext,
) -> Result<ChartDocumentPlan> {
    let ChartRenderContext {
        chars,
        plot_area,
        plot_extent,
        options,
        ..
    } = chart;
    let HorizontalPlotMetricsContext {
        plan,
        plot_prefix_width,
        ..
    } = metrics;
    let mut document = ChartDocumentPlan::default();
    measure_chart_header(&mut document, model, plan, chars, options, resources)?;
    if disclosure.is_required() {
        measure_value_disclosure_lines(
            &mut document,
            model,
            plan,
            chars,
            disclosure,
            options,
            resources,
        )?;
    }

    let plot_metrics =
        horizontal_plot_document_metrics(model, metrics, horizontal_plan, resources)?;
    document.include_materialized_block(
        plot_metrics.width,
        plot_metrics.materialized_width,
        plot_extent.height(),
        plot_metrics.document_cells,
        resources,
    )?;

    if axis_labels_visible(model.display.y_axis) || baseline_mark.is_some() {
        let (retained_width, materialized_width) = horizontal_axis_baseline_widths(
            model.display.y_axis,
            plot_prefix_width,
            plot_area.horizontal_width,
            baseline_mark,
            resources,
        )?;
        document.include_materialized_line(retained_width, materialized_width, resources)?;
    }
    if axis_labels_visible(model.display.y_axis) {
        document.include_line(
            resources.checked_grid_add(plot_prefix_width, plot_area.horizontal_width)?,
            resources,
        )?;
    }
    if model.display.x_axis.show_title
        && let Some(title) = nonempty_axis_title(&model.x_axis)
    {
        let width = title_display_line_width(TitleOwner::XAxis, title, options, resources)?;
        document.include_line(width, resources)?;
    }
    Ok(document)
}

fn horizontal_axis_baseline_widths(
    y_axis: XyChartAxisDisplayPolicy,
    plot_prefix_width: usize,
    plot_width: usize,
    baseline_mark: Option<char>,
    resources: &ResourceContext,
) -> Result<(usize, usize)> {
    if y_axis.show_axis_line || y_axis.show_tick {
        let width = resources.checked_grid_add(plot_prefix_width, plot_width)?;
        return Ok((width, width));
    }
    let retained_width = if baseline_mark.is_some() {
        plot_prefix_width
    } else {
        0
    };
    Ok((retained_width, plot_prefix_width))
}

fn measure_chart_header(
    document: &mut ChartDocumentPlan,
    model: &XyChartDiagramRenderModel,
    plan: &TerminalChartPlan,
    chars: ChartChars,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<()> {
    if model.display.show_title
        && let Some(title) = model.title.as_deref()
    {
        let width = title_display_line_width(TitleOwner::Chart, title, options, resources)?;
        document.include_line(width, resources)?;
    }
    if model.display.y_axis.show_title
        && let Some(title) = nonempty_axis_title(&model.y_axis)
    {
        let width = title_display_line_width(TitleOwner::YAxis, title, options, resources)?;
        document.include_line(width, resources)?;
    }
    if let Some(width) = legend_line_width(plan, chars, options, resources)? {
        document.include_line(width, resources)?;
    }
    Ok(())
}

fn legend_line_width(
    plan: &TerminalChartPlan,
    _chars: ChartChars,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<Option<usize>> {
    if plan.series.len() <= 1 {
        return Ok(None);
    }

    let mut width = 0usize;
    let mut state = SeriesLabelState::default();
    for series in &plan.series {
        resources.charge_layout_work(1)?;
        if series.series_index > 0 {
            width = resources.checked_grid_add(width, 2)?;
        }
        width = resources.checked_grid_add(width, 2)?;
        let label_width = match normalized_optional_text_width(
            series.title.as_deref(),
            options.terminal_width_profile,
            resources,
        )? {
            Some(width) => {
                match series.plot_type {
                    XyChartPlotType::Bar => state.bar_index += 1,
                    XyChartPlotType::Line => state.line_index += 1,
                }
                width
            }
            None => {
                let label = match series.plot_type {
                    XyChartPlotType::Bar => {
                        state.bar_index += 1;
                        format!("Bar {}", state.bar_index)
                    }
                    XyChartPlotType::Line => {
                        state.line_index += 1;
                        format!("Line {}", state.line_index)
                    }
                };
                charge_text_layout(resources, &label)?;
                display_width_with_profile(&label, options.terminal_width_profile)
            }
        };
        width = resources.checked_grid_add(width, label_width)?;
    }
    Ok(Some(width))
}

fn measure_value_disclosure_lines(
    document: &mut ChartDocumentPlan,
    model: &XyChartDiagramRenderModel,
    plan: &TerminalChartPlan,
    chars: ChartChars,
    disclosure: super::plot::TerminalDisclosurePlan,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<()> {
    if let Some(width) =
        band_domain_disclosure_line_width(model, plan, disclosure, options, resources)?
    {
        document.include_line(width, resources)?;
    }
    let (x_domain_width, y_domain_width) =
        linear_domain_disclosure_line_widths(plan, disclosure, options, resources)?;
    if let Some(width) = x_domain_width {
        document.include_line(width, resources)?;
    }
    if let Some(width) = y_domain_width {
        document.include_line(width, resources)?;
    }
    if !disclosure.values {
        return Ok(());
    }
    for series in &plan.series {
        resources.charge_layout_work(1)?;
        if let Some(width) =
            value_disclosure_line_width(model, series, plan, chars, options, resources)?
        {
            document.include_line(width, resources)?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn vertical_plot_document_metrics(
    vertical_plan: &VerticalPlotAdmissionPlan,
    tick_labels: &[String],
    plot_prefix_width: usize,
    gutter: usize,
    show_axis_labels: bool,
    axis_mark: Option<char>,
    preserve_color: bool,
    width_profile: TerminalWidthProfile,
    plot_extent: LogicalExtent,
    resources: &mut ResourceContext,
) -> Result<PlotDocumentMetrics> {
    let row_widths = vertical_plan.row_widths();
    if row_widths.len() != plot_extent.height() || tick_labels.len() != plot_extent.height() {
        return Err(invalid_chart_document_extent_plan());
    }

    let materialized_width = resources.checked_grid_add(plot_prefix_width, plot_extent.width())?;
    let mut width = 0usize;
    let mut document_cells = 0usize;
    for (label, plot_width) in tick_labels.iter().zip(row_widths.iter().copied()) {
        resources.charge_layout_work(1)?;
        let row_width = if plot_width > 0 {
            resources.checked_grid_add(plot_prefix_width, plot_width)?
        } else {
            axis_prefix_retained_width(
                label,
                gutter,
                plot_prefix_width,
                show_axis_labels,
                axis_mark,
                preserve_color,
                width_profile,
                resources,
            )?
        };
        width = width.max(row_width);
        document_cells = checked_document_cells_add(document_cells, row_width, resources)?;
    }
    Ok(PlotDocumentMetrics {
        width,
        materialized_width,
        document_cells,
    })
}

fn horizontal_plot_document_metrics(
    model: &XyChartDiagramRenderModel,
    context: HorizontalPlotMetricsContext<'_>,
    horizontal_plan: &HorizontalPlotAdmissionPlan,
    resources: &mut ResourceContext,
) -> Result<PlotDocumentMetrics> {
    let HorizontalPlotMetricsContext {
        plan,
        plot_prefix_width,
        plot_area,
        ..
    } = context;
    let plot_row_widths = horizontal_plan.row_widths();
    if plot_row_widths.len() != plan.horizontal_row_count(resources)? {
        return Err(invalid_chart_document_extent_plan());
    }

    let mut width = 0usize;
    let mut document_cells = 0usize;
    for (row, plot_width) in plot_row_widths.iter().copied().enumerate() {
        resources.charge_layout_work(1)?;
        let row_width = horizontal_plot_row_document_width(context, row, plot_width, resources)?;
        width = width.max(row_width);
        document_cells = checked_document_cells_add(document_cells, row_width, resources)?;
    }
    let mut materialized_width = if plot_row_widths.is_empty() {
        0
    } else {
        resources.checked_grid_add(plot_prefix_width, plot_area.horizontal_width)?
    };

    if !model.display.show_data_label || !uses_compact_bar_data_labels(model) {
        return Ok(PlotDocumentMetrics {
            width,
            materialized_width,
            document_cells,
        });
    }
    if plan.bar_series_count == 0 {
        return Ok(PlotDocumentMetrics {
            width,
            materialized_width,
            document_cells,
        });
    }

    let materialized_plot_width =
        resources.checked_grid_add(plot_prefix_width, plot_area.horizontal_width)?;
    let rows_per_slot = plan.horizontal_rows_per_slot();
    for slot in 0..plan.slot_count {
        resources.charge_layout_work(1)?;
        let Some(value) = horizontal_plan.compact_bar_value(slot) else {
            continue;
        };
        let label = format_data_number(value);
        let label_width = display_width_with_profile(&label, plot_area.width_profile);
        let bar_width = horizontal_bar_width(value, plan.y_range, plot_area);
        let fits_inside = !model.display.show_data_label_outside_bar
            && bar_width > 0
            && label_width > 0
            && label_width <= bar_width;
        if !fits_inside && !label.is_empty() {
            let row = resources.checked_grid_mul(slot, rows_per_slot)?;
            let plot_width = plot_row_widths
                .get(row)
                .copied()
                .ok_or_else(|| resources.grid_overflow())?;
            let planned = horizontal_plot_row_document_width(context, row, plot_width, resources)?;
            let outside = resources.checked_grid_add(
                materialized_plot_width,
                resources.checked_grid_add(1, label_width)?,
            )?;
            width = width.max(outside);
            materialized_width = materialized_width.max(outside);
            document_cells = checked_document_cells_add(
                document_cells,
                checked_grid_sub(resources, outside, planned)?,
                resources,
            )?;
        }
    }
    Ok(PlotDocumentMetrics {
        width,
        materialized_width,
        document_cells,
    })
}

fn horizontal_plot_row_document_width(
    context: HorizontalPlotMetricsContext<'_>,
    row: usize,
    plot_width: usize,
    resources: &ResourceContext,
) -> Result<usize> {
    if plot_width > 0 {
        return resources.checked_grid_add(context.plot_prefix_width, plot_width);
    }
    horizontal_prefix_retained_width(context, row, resources)
}

fn horizontal_prefix_retained_width(
    context: HorizontalPlotMetricsContext<'_>,
    row: usize,
    resources: &ResourceContext,
) -> Result<usize> {
    let show_label =
        context.show_axis_labels && row.is_multiple_of(context.plan.horizontal_rows_per_slot());
    let label = if show_label {
        let category_index = row / context.plan.horizontal_rows_per_slot();
        context
            .plan
            .horizontal_axis_labels
            .get(category_index)
            .map(String::as_str)
            .unwrap_or_default()
    } else {
        ""
    };
    axis_prefix_retained_width(
        label,
        context.gutter,
        context.plot_prefix_width,
        show_label,
        context.axis_mark,
        context.preserve_color,
        context.plot_area.width_profile,
        resources,
    )
}

#[allow(clippy::too_many_arguments)]
fn axis_prefix_retained_width(
    label: &str,
    gutter: usize,
    plot_prefix_width: usize,
    show_axis_labels: bool,
    axis_mark: Option<char>,
    preserve_color: bool,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<usize> {
    if axis_mark.is_some() {
        return Ok(plot_prefix_width);
    }
    if !show_axis_labels {
        return Ok(0);
    }

    let Some((label_width, retained_width)) =
        normalized_visible_line_metrics(label, width_profile, resources)?
    else {
        return Ok(0);
    };
    let retained_width = if preserve_color {
        label_width
    } else {
        retained_width
    };
    if retained_width == 0 {
        return Ok(0);
    }
    let padding = checked_grid_sub(resources, gutter, label_width)?;
    resources.checked_grid_add(padding, retained_width)
}

fn normalized_optional_text_width(
    value: Option<&str>,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Option<usize>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Some(plan) = try_plan_normalized_label_lines_with_policy(
        value,
        width_profile,
        true,
        None,
        LabelBreakPolicy::VisibleLine,
        resources,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(plan.metrics().max_width))
}

fn normalized_visible_line_metrics(
    value: &str,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Option<(usize, usize)>> {
    let Some(plan) = try_plan_normalized_label_lines_with_policy(
        value,
        width_profile,
        false,
        None,
        LabelBreakPolicy::VisibleLine,
        resources,
    )?
    else {
        return Ok(None);
    };
    let width = plan.metrics().max_width;
    let mut retained_width = 0usize;
    plan.try_visit_row_metrics_with_checkpoint(
        value,
        resources,
        || resources.check_usage(0, 0),
        |row| {
            retained_width = retained_width.max(row.retained_width);
            Ok(())
        },
    )?;
    Ok(Some((width, retained_width)))
}

fn push_title_lines(
    out: &mut ChartDocument,
    model: &XyChartDiagramRenderModel,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<()> {
    if model.display.show_title
        && let Some(title) = model.title.as_deref()
    {
        push_title_display_line(out, TitleOwner::Chart, title, options, resources)?;
    }

    if model.display.y_axis.show_title
        && let Some(title) = nonempty_axis_title(&model.y_axis)
    {
        push_title_display_line(out, TitleOwner::YAxis, title, options, resources)?;
    }
    Ok(())
}

fn push_legend_line(
    out: &mut ChartDocument,
    plan: &TerminalChartPlan,
    chars: ChartChars,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<()> {
    if plan.series.len() <= 1 {
        return Ok(());
    }

    let line = legend_line(&plan.series, chars, options, resources)?;
    out.push(line, resources)
}

fn legend_line(
    series: &[SeriesPlan],
    chars: ChartChars,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<ChartLine> {
    let mut line = new_chart_line(options, resources);
    let mut label_state = SeriesLabelState::default();

    for series in series {
        resources.charge_layout_work(1)?;
        if series.series_index > 0 {
            resources.charge_layout_work(2)?;
            line.try_push_spaces(2)?;
        }

        resources.charge_layout_work(2)?;
        line.try_push_role_char(
            chars.legend_symbol(series.plot_type),
            AsciiColorRole::ChartSeries(series.series_index),
        )?;
        line.try_push_plain_char(' ')?;
        let label = series_label(
            series,
            &mut label_state,
            options.terminal_width_profile,
            resources,
        )?;
        line.try_push_role_text(&label, AsciiColorRole::Text)?;
    }

    Ok(line)
}

#[derive(Debug, Default)]
struct SeriesLabelState {
    bar_index: usize,
    line_index: usize,
}

fn series_label(
    series: &SeriesPlan,
    state: &mut SeriesLabelState,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<String> {
    let default_label = match series.plot_type {
        XyChartPlotType::Bar => {
            state.bar_index += 1;
            format!("Bar {}", state.bar_index)
        }
        XyChartPlotType::Line => {
            state.line_index += 1;
            format!("Line {}", state.line_index)
        }
    };

    if let Some(title) = series.title.as_deref()
        && let Some(title) = normalized_optional_text(Some(title), width_profile, resources)?
    {
        return Ok(title);
    }

    charge_text_layout(resources, &default_label)?;
    Ok(default_label)
}

fn axis_labels_visible(axis: XyChartAxisDisplayPolicy) -> bool {
    axis.show_label
}

fn plot_prefix_width(
    show_axis_labels: bool,
    reserve_axis_slot: bool,
    gutter: usize,
    resources: &ResourceContext,
) -> Result<usize> {
    if show_axis_labels {
        resources.checked_grid_add(gutter, 2)
    } else if reserve_axis_slot {
        Ok(1)
    } else {
        Ok(0)
    }
}

fn vertical_axis_mark(axis: XyChartAxisDisplayPolicy, chars: ChartChars) -> Option<char> {
    if axis.show_tick {
        Some(chars.vertical_tick)
    } else if axis.show_axis_line {
        Some(chars.vertical_axis)
    } else {
        None
    }
}

fn push_axis_prefix(
    line: &mut ChartLine,
    label: &str,
    gutter: usize,
    show_axis_labels: bool,
    axis_mark: Option<char>,
    reserve_axis_slot: bool,
    resources: &mut ResourceContext,
) -> Result<()> {
    if show_axis_labels {
        charge_text_layout(resources, label)?;
        let label_width = display_width_with_profile(label, line.width_profile());
        let padding = checked_grid_sub(resources, gutter, label_width)?;
        resources.charge_layout_work(gutter)?;
        line.try_push_spaces(padding)?;
        line.try_push_role_text(label, AsciiColorRole::Text)?;
        resources.charge_layout_work(1)?;
        line.try_push_plain_char(' ')?;
    }

    match axis_mark {
        Some(axis_mark) => {
            resources.charge_layout_work(1)?;
            line.try_push_role_char(axis_mark, AsciiColorRole::ChartAxis)?;
        }
        None if reserve_axis_slot => {
            resources.charge_layout_work(1)?;
            line.try_push_plain_char(' ')?;
        }
        None => {}
    }
    Ok(())
}

fn push_axis_baseline_prefix(
    line: &mut ChartLine,
    label: &str,
    gutter: usize,
    show_axis_labels: bool,
    origin: Option<char>,
    reserve_axis_slot: bool,
    resources: &mut ResourceContext,
) -> Result<()> {
    if show_axis_labels {
        charge_text_layout(resources, label)?;
        let label_width = display_width_with_profile(label, line.width_profile());
        let padding = checked_grid_sub(resources, gutter, label_width)?;
        resources.charge_layout_work(gutter)?;
        line.try_push_spaces(padding)?;
        line.try_push_role_text(label, AsciiColorRole::Text)?;
        resources.charge_layout_work(1)?;
        line.try_push_plain_char(' ')?;
    }

    match origin {
        Some(origin) => {
            resources.charge_layout_work(1)?;
            line.try_push_role_char(origin, AsciiColorRole::ChartAxis)?;
        }
        None if reserve_axis_slot => {
            resources.charge_layout_work(1)?;
            line.try_push_plain_char(' ')?;
        }
        None => {}
    }
    Ok(())
}

fn overlay_vertical_category_ticks(
    line: &mut ChartLine,
    plot_start: usize,
    category_count: usize,
    plot_area: XyChartPlotArea,
    tick: char,
    resources: &mut ResourceContext,
    checkpoints: &mut XyChartCheckpointCursor<'_>,
) -> Result<()> {
    for index in 0..category_count {
        checkpoints.tick(merman_core::OperationPhase::Emit)?;
        resources.charge_layout_work(1)?;
        let position = plot_area.vertical_band_center(index, resources)?;
        let target = resources.checked_grid_add(plot_start, position)?;
        line.try_set_role(target, tick, AsciiColorRole::ChartAxis)?;
    }
    Ok(())
}

fn overlay_horizontal_value_ticks(
    line: &mut ChartLine,
    plot_start: usize,
    plot_area: XyChartPlotArea,
    tick: char,
    resources: &mut ResourceContext,
    checkpoints: &mut XyChartCheckpointCursor<'_>,
) -> Result<()> {
    let last = plot_area
        .horizontal_width
        .checked_sub(1)
        .ok_or(AsciiError::InvalidOption {
            field: "xychart_horizontal_plot_width",
            message: "must be at least 2",
        })?;
    for position in [0, last] {
        checkpoints.tick(merman_core::OperationPhase::Emit)?;
        resources.charge_layout_work(1)?;
        let target = resources.checked_grid_add(plot_start, position)?;
        line.try_set_role(target, tick, AsciiColorRole::ChartAxis)?;
    }
    Ok(())
}

fn vertical_data_label_line(
    labels: &[String],
    plot_prefix_width: usize,
    plot_area: XyChartPlotArea,
    resources: &mut ResourceContext,
    checkpoints: &mut XyChartCheckpointCursor<'_>,
) -> Result<Option<ChartLine>> {
    if labels.is_empty() {
        return Ok(None);
    }

    let band_labels = plot_area.band_labels(labels, resources, checkpoints)?;
    let mut line = ChartLine::with_resources(plot_area.width_profile, resources);
    resources.charge_layout_work(plot_prefix_width)?;
    line.try_push_spaces(plot_prefix_width)?;
    line.try_push_role_text_with_unstyled_trailing_spaces(&band_labels, AsciiColorRole::Text)?;
    Ok(Some(line))
}

fn vertical_data_label_line_retained_width(
    labels: &[String],
    plot_prefix_width: usize,
    plot_area: XyChartPlotArea,
    resources: &mut ResourceContext,
) -> Result<Option<usize>> {
    if labels.is_empty() {
        return Ok(None);
    }
    let labels_width = plot_area.band_labels_retained_width(labels, resources)?;
    if labels_width == 0 {
        Ok(Some(0))
    } else {
        Ok(Some(
            resources.checked_grid_add(plot_prefix_width, labels_width)?,
        ))
    }
}

fn write_horizontal_inside_data_label(
    line: &mut ChartLine,
    plot_prefix_width: usize,
    label: &str,
    value: f64,
    y_range: ValueRange,
    plot_area: XyChartPlotArea,
    resources: &mut ResourceContext,
) -> Result<bool> {
    charge_text_layout(resources, label)?;
    let bar_width = horizontal_bar_width(value, y_range, plot_area);
    let label_width = display_width_with_profile(label, plot_area.width_profile);
    if bar_width == 0 || label_width == 0 || label_width > bar_width {
        return Ok(false);
    }

    let bar_end = resources.checked_grid_add(plot_prefix_width, bar_width)?;
    let start = checked_grid_sub(resources, bar_end, label_width)?;
    resources.charge_layout_work(label_width)?;
    line.try_write_text_role(start, label, AsciiColorRole::Text)?;
    Ok(true)
}

fn push_horizontal_outside_data_label(
    line: &mut ChartLine,
    label: &str,
    resources: &mut ResourceContext,
) -> Result<()> {
    if label.is_empty() {
        return Ok(());
    }

    charge_text_layout(resources, label)?;
    resources.charge_layout_work(1)?;
    line.try_push_plain_char(' ')?;
    line.try_push_role_text(label, AsciiColorRole::Text)?;
    Ok(())
}

fn uses_compact_bar_data_labels(model: &XyChartDiagramRenderModel) -> bool {
    model.plots.len() == 1 && model.plots[0].plot_type == XyChartPlotType::Bar
}

fn compact_bar_value_labels(
    model: &XyChartDiagramRenderModel,
    plan: &TerminalChartPlan,
    resources: &mut ResourceContext,
) -> Result<Option<Vec<String>>> {
    if !uses_compact_bar_data_labels(model) {
        return Ok(None);
    }

    let Some(series) = plan
        .series
        .iter()
        .find(|series| series.plot_type == XyChartPlotType::Bar)
    else {
        return Ok(None);
    };
    let mut labels = Vec::new();
    labels
        .try_reserve_exact(plan.slot_count)
        .map_err(|_| allocation_failed(AsciiResourceLimitPhase::LayoutWork))?;
    for _ in 0..plan.slot_count {
        resources.charge_layout_work(1)?;
        labels.push(String::new());
    }
    for datum in &series.data {
        resources.charge_layout_work(1)?;
        if let Some(value) = datum.value
            && let Some(label) = labels.get_mut(plan.sample_slot(datum))
        {
            *label = format_data_number(value);
        }
    }
    Ok(Some(labels))
}

pub(super) fn axis_title(axis: &XyChartAxisRenderModel) -> &str {
    match axis {
        XyChartAxisRenderModel::Linear { title, .. }
        | XyChartAxisRenderModel::Band { title, .. } => title,
    }
}

fn nonempty_axis_title(axis: &XyChartAxisRenderModel) -> Option<&str> {
    let title = axis_title(axis);
    (!title.is_empty()).then_some(title)
}

fn normalized_optional_text(
    value: Option<&str>,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Some(plan) = try_plan_normalized_label_lines_with_policy(
        value,
        width_profile,
        true,
        None,
        LabelBreakPolicy::VisibleLine,
        resources,
    )?
    else {
        return Ok(None);
    };
    let (mut lines, _) = plan.materialize(value, resources)?.into_parts();
    if lines.len() != 1 {
        return Err(invalid_chart_document_extent_plan());
    }
    Ok(lines.pop())
}

fn vertical_tick_labels(
    y_range: ValueRange,
    plot_area: XyChartPlotArea,
    resources: &mut ResourceContext,
) -> Result<Vec<String>> {
    let mut labels = Vec::new();
    labels
        .try_reserve_exact(plot_area.vertical_height)
        .map_err(|_| allocation_failed(AsciiResourceLimitPhase::LayoutWork))?;
    let step = y_range.span() / plot_area.vertical_height as f64;
    for level in (1..=plot_area.vertical_height).rev() {
        resources.charge_layout_work(1)?;
        let value = y_range.min + step * level as f64;
        labels.push(format_tick_number(value, step));
    }
    Ok(labels)
}

fn horizontal_tick_label_line(
    y_range: ValueRange,
    plot_area: XyChartPlotArea,
    resources: &mut ResourceContext,
) -> Result<ChartLine> {
    let step = y_range.span();
    let min = format_tick_number(y_range.min, step);
    let max = format_tick_number(y_range.max, step);
    horizontal_tick_label_line_for_labels(
        &min,
        &max,
        plot_area.horizontal_width,
        plot_area.width_profile,
        resources,
    )
}

fn horizontal_tick_label_line_for_labels(
    min: &str,
    max: &str,
    width: usize,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<ChartLine> {
    let mut line = ChartLine::try_blank_with_resources(width, width_profile, resources)?;
    write_horizontal_tick_label(&mut line, 0, min, width, resources)?;

    let fitted_max = truncate_display_width_with_profile(max, width, width_profile);
    let fitted_width = display_width_with_profile(&fitted_max, width_profile);
    let max_start = checked_grid_sub(resources, width, fitted_width)?;
    let remaining = checked_grid_sub(resources, width, max_start)?;
    write_horizontal_tick_label(&mut line, max_start, &fitted_max, remaining, resources)?;

    Ok(line)
}

fn write_horizontal_tick_label(
    line: &mut ChartLine,
    start: usize,
    label: &str,
    width: usize,
    resources: &mut ResourceContext,
) -> Result<()> {
    charge_text_layout(resources, label)?;
    let fitted = truncate_display_width_with_profile(label, width, line.width_profile());
    resources.charge_layout_work(display_width_with_profile(&fitted, line.width_profile()))?;
    line.try_write_text_role(start, &fitted, AsciiColorRole::Text)?;
    Ok(())
}

fn new_chart_line(options: &AsciiRenderOptions, resources: &ResourceContext) -> ChartLine {
    ChartLine::with_resources(options.terminal_width_profile, resources)
}

fn finish_chart_lines_controlled(
    document: ChartDocument,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    if document.lines.is_empty() {
        return Ok(String::new());
    }

    finish_styled_lines_with_resources_with_execution(
        &document.lines,
        options,
        true,
        resources,
        execution,
    )
}

fn checkpoint_emitted_lines(rendered: &str, execution: AsciiExecution<'_>) -> Result<()> {
    for _ in rendered.lines() {
        execution.checkpoint(merman_core::OperationPhase::Emit)?;
    }
    Ok(())
}

fn allocation_failed(phase: AsciiResourceLimitPhase) -> AsciiError {
    AsciiError::allocation_failed(phase.as_str())
}

fn invalid_chart_document_extent_plan() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "xychart",
        feature: "chart document extent planning",
    }
}

fn empty_labels(count: usize) -> Result<Vec<String>> {
    let mut labels = Vec::new();
    labels
        .try_reserve_exact(count)
        .map_err(|_| allocation_failed(AsciiResourceLimitPhase::LayoutWork))?;
    labels.resize_with(count, String::new);
    Ok(labels)
}

fn label_gutter<'a>(
    labels: impl IntoIterator<Item = &'a str>,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<usize> {
    let mut gutter = 0;
    for label in labels {
        if let Some((width, _)) = normalized_visible_line_metrics(label, width_profile, resources)?
        {
            gutter = gutter.max(width);
        }
    }
    Ok(gutter)
}

fn charge_category_text(
    categories: &[CategoryLabel],
    resources: &mut ResourceContext,
) -> Result<()> {
    for category in categories {
        charge_text_layout(resources, category.as_str())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ChartDocument, finish_chart_lines_controlled, horizontal_tick_label_line_for_labels,
        render_xychart_diagram_with_materializer, render_xychart_diagram_with_resources,
    };
    use crate::operation::AsciiExecution;
    use crate::resource::ResourceContext;
    use crate::text::{StyledLine, display_width_with_profile};
    use crate::xychart::plot::XyChartCheckpointCursor;
    use crate::{
        AsciiColorMode, AsciiColorRole, AsciiError, AsciiRenderOptions, AsciiResourceLimitId,
        AsciiResourcePolicy, Result, TerminalWidthProfile,
    };
    use merman_core::diagrams::xychart::{
        XyChartAxisDisplayPolicy, XyChartAxisRenderModel, XyChartDiagramRenderModel,
        XyChartDisplayPolicy, XyChartPlotRenderModel, XyChartPlotType,
    };
    use merman_core::{CancelReason, OperationControl, OperationPhase};
    use std::cell::Cell;

    fn render_xychart_diagram(
        model: &XyChartDiagramRenderModel,
        options: &AsciiRenderOptions,
    ) -> Result<String> {
        render_xychart_diagram_with_resources(model, options, AsciiResourcePolicy::default())
    }

    fn resources_with_limit(limit: AsciiResourceLimitId, max: usize) -> AsciiResourcePolicy {
        AsciiResourcePolicy::default()
            .with_limit(limit, max)
            .expect("test resource limit should be valid")
    }

    fn authored_text_model() -> XyChartDiagramRenderModel {
        XyChartDiagramRenderModel {
            orientation: "horizontal".to_string(),
            title: Some("<Ops 👩‍💻>\u{1b}[31m".to_string()),
            acc_title: None,
            acc_descr: None,
            x_axis: XyChartAxisRenderModel::Band {
                title: String::new(),
                categories: vec!["A".to_string(), "·".to_string()],
            },
            y_axis: XyChartAxisRenderModel::Linear {
                title: String::new(),
                min: Some(0.0),
                max: Some(10.0),
            },
            plots: vec![XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Bar,
                title: None,
                values: vec![5.0, 8.0],
                data: Vec::new(),
                point_labels: Vec::new(),
            }],
            display: XyChartDisplayPolicy::default(),
        }
    }

    fn tick_label_line(
        min: &str,
        max: &str,
        width: usize,
        width_profile: TerminalWidthProfile,
    ) -> super::ChartLine {
        let mut resources = ResourceContext::new(AsciiResourcePolicy::default());
        horizontal_tick_label_line_for_labels(min, max, width, width_profile, &mut resources)
            .expect("tick labels should fit the default resource policy")
    }

    fn compact_vertical_model(title: Option<&str>) -> XyChartDiagramRenderModel {
        let hidden_axis = XyChartAxisDisplayPolicy {
            show_label: false,
            show_title: false,
            show_tick: false,
            show_axis_line: false,
        };
        XyChartDiagramRenderModel {
            orientation: "vertical".to_string(),
            title: title.map(str::to_string),
            acc_title: None,
            acc_descr: None,
            x_axis: XyChartAxisRenderModel::Band {
                title: String::new(),
                categories: vec!["A".to_string()],
            },
            y_axis: XyChartAxisRenderModel::Linear {
                title: String::new(),
                min: Some(0.0),
                max: Some(1.0),
            },
            plots: vec![XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Bar,
                title: None,
                values: vec![1.0],
                data: Vec::new(),
                point_labels: Vec::new(),
            }],
            display: XyChartDisplayPolicy {
                show_title: title.is_some(),
                show_data_label: false,
                show_data_label_outside_bar: false,
                x_axis: hidden_axis,
                y_axis: hidden_axis,
            },
        }
    }

    fn compact_options(color_mode: AsciiColorMode) -> AsciiRenderOptions {
        AsciiRenderOptions::ascii()
            .with_color_mode(color_mode)
            .with_xychart_vertical_plot_height(2)
            .with_xychart_category_band_width(1)
    }

    fn compact_horizontal_model() -> XyChartDiagramRenderModel {
        let mut model = compact_vertical_model(None);
        model.orientation = "horizontal".to_string();
        model.x_axis = XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string(), "B".to_string()],
        };
        model.plots[0].values = vec![1.0, 0.5];
        model
    }

    fn disclosure_budget_model() -> XyChartDiagramRenderModel {
        let mut model = compact_vertical_model(None);
        model.plots[0] = XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Line,
            title: Some("a disclosure title that dominates the compact plot width".to_string()),
            values: vec![1.0],
            data: vec![(
                "an authored x coordinate with delimiters = [, ] and a long suffix".to_string(),
                Some(1.0),
            )],
            point_labels: vec!["a quoted point label with \\ and \" delimiters".to_string()],
        };
        model
    }

    fn assert_resource_error(
        error: AsciiError,
        limit: AsciiResourceLimitId,
        actual: usize,
        max: usize,
    ) {
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a resource-limit error, got {error:?}");
        };
        assert_eq!(details.limit, limit);
        assert_eq!(details.actual, actual);
        assert_eq!(details.max, max);
    }

    #[test]
    fn horizontal_tick_label_line_uses_display_cells_for_wide_labels() {
        let line = tick_label_line("中", "界", 5, TerminalWidthProfile::Unicode);

        assert_eq!(line.len(), 5);
        assert_eq!(line.text(), "中 界");
        assert_eq!(line.get(0), Some('中'));
        assert_eq!(line.get(1), None);
        assert_eq!(line.get(2), Some(' '));
        assert_eq!(line.get(3), Some('界'));
        assert_eq!(line.get(4), None);
    }

    #[test]
    fn horizontal_tick_label_line_truncates_at_display_cell_boundaries() {
        let line = tick_label_line("中国A", "", 3, TerminalWidthProfile::Unicode);

        assert_eq!(line.len(), 3);
        assert_eq!(line.text(), "中 ");
        assert_eq!(line.get(0), Some('中'));
        assert_eq!(line.get(1), None);
        assert_eq!(line.get(2), Some(' '));
    }

    #[test]
    fn horizontal_tick_label_line_obeys_cjk_ambiguous_width() {
        let unicode = tick_label_line("·", "", 2, TerminalWidthProfile::Unicode);
        let cjk = tick_label_line("·", "", 2, TerminalWidthProfile::Cjk);

        assert_eq!(unicode.text(), "· ");
        assert_eq!(unicode.get(1), Some(' '));
        assert_eq!(cjk.text(), "·");
        assert_eq!(cjk.get(1), None);
    }

    #[test]
    fn xychart_authored_text_is_grapheme_safe_across_output_modes() {
        for color_mode in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Html,
        ] {
            let options = AsciiRenderOptions::ascii().with_color_mode(color_mode);
            let rendered = render_xychart_diagram(&authored_text_model(), &options)
                .expect("authored XYChart text should render");

            assert!(rendered.contains("👩‍💻"));
            assert!(rendered.contains("\\u{1B}"));
            if color_mode != AsciiColorMode::Ansi16 {
                assert!(!rendered.contains('\u{1b}'));
            }
            if color_mode == AsciiColorMode::Html {
                assert!(rendered.contains("&lt;Ops 👩‍💻&gt;"));
                assert!(!rendered.contains("<Ops"));
            }
        }
    }

    #[test]
    fn xychart_horizontal_gutter_obeys_cjk_ambiguous_width() {
        let unicode = render_xychart_diagram(
            &authored_text_model(),
            &AsciiRenderOptions::ascii().with_terminal_width_profile(TerminalWidthProfile::Unicode),
        )
        .expect("Unicode-width XYChart should render");
        let cjk = render_xychart_diagram(
            &authored_text_model(),
            &AsciiRenderOptions::ascii().with_terminal_width_profile(TerminalWidthProfile::Cjk),
        )
        .expect("CJK-width XYChart should render");

        let unicode_a = unicode
            .lines()
            .find(|line| line.contains("A +"))
            .expect("Unicode-width category row should render");
        let cjk_a = cjk
            .lines()
            .find(|line| line.contains("A +"))
            .expect("CJK-width category row should render");

        assert!(unicode_a.starts_with("A +"));
        assert!(cjk_a.starts_with(" A +"));
    }

    #[test]
    fn xychart_cjk_profile_falls_back_to_single_cell_ascii_structure() {
        let rendered = render_xychart_diagram(
            &authored_text_model(),
            &AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk),
        )
        .expect("CJK-width XYChart should render");

        assert!(rendered.lines().any(|line| line.starts_with(" A +#####")));
        assert!(
            !rendered
                .chars()
                .any(|ch| matches!(ch, '┤' | '█' | '┼' | '┬' | '─'))
        );
    }

    #[test]
    fn xychart_titles_always_use_owned_utf8_byte_frames() {
        let mut model = authored_text_model();
        model.title = Some("\ttitle\r".to_string());
        model.x_axis = XyChartAxisRenderModel::Band {
            title: "\tx-axis\r".to_string(),
            categories: vec!["A".to_string(), "B".to_string()],
        };
        model.y_axis = XyChartAxisRenderModel::Linear {
            title: "\ty-axis\r".to_string(),
            min: Some(0.0),
            max: Some(10.0),
        };

        let rendered = render_xychart_diagram(&model, &AsciiRenderOptions::ascii())
            .expect("XYChart titles should render");

        for (key, text) in [("chart", "title"), ("xAxis", "x-axis"), ("yAxis", "y-axis")] {
            let authored = format!("\t{text}\r");
            let expected = format!(
                r#"titleDisplay: {key}(bytes={})="\t{text}\r""#,
                authored.len()
            );
            assert!(
                rendered.contains(&expected),
                "missing framed {key} title:\n{rendered}"
            );
        }
    }

    #[test]
    fn xychart_complete_grid_budget_covers_title_and_plot_in_every_mode() {
        let model = compact_vertical_model(Some("12345678"));
        for color_mode in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Ansi256,
            AsciiColorMode::TrueColor,
            AsciiColorMode::Html,
        ] {
            let options = compact_options(color_mode);
            let accepted = resources_with_limit(AsciiResourceLimitId::MaxGridCells, 227);
            render_xychart_diagram_with_resources(&model, &options, accepted)
                .expect("the exact complete XYChart grid limit should succeed");

            let rejected = resources_with_limit(AsciiResourceLimitId::MaxGridCells, 226);
            let error = render_xychart_diagram_with_resources(&model, &options, rejected)
                .expect_err("the complete document plus retained plot must exceed 226 cells");
            assert_resource_error(error, AsciiResourceLimitId::MaxGridCells, 227, 226);
        }
    }

    #[test]
    fn xychart_complete_document_extent_is_admitted_before_row_materialization() {
        let model = disclosure_budget_model();
        let options = compact_options(AsciiColorMode::Plain);
        let rendered = render_xychart_diagram(&model, &options)
            .expect("the disclosure-dominated fixture should render without a tight limit");
        let line_count = rendered.lines().count();
        let max_width = rendered
            .lines()
            .map(|line| display_width_with_profile(line, options.terminal_width_profile))
            .max()
            .unwrap_or(0);
        let retained_plot_cells = 2usize;
        let exact_cells = max_width * line_count + retained_plot_cells;
        assert!(exact_cells > 100);

        let exact_probe = Cell::new(false);
        let exact = resources_with_limit(AsciiResourceLimitId::MaxGridCells, exact_cells);
        render_xychart_diagram_with_materializer(&model, &options, exact, || exact_probe.set(true))
            .expect("the exact complete document extent should render");
        assert!(
            exact_probe.get(),
            "exact admission did not materialize rows"
        );

        let below_probe = Cell::new(false);
        let below = resources_with_limit(AsciiResourceLimitId::MaxGridCells, exact_cells - 1);
        let error = render_xychart_diagram_with_materializer(&model, &options, below, || {
            below_probe.set(true)
        })
        .expect_err("N-1 must fail before materializing any chart row");
        assert!(!below_probe.get(), "N-1 overflow materialized chart rows");
        assert_resource_error(
            error,
            AsciiResourceLimitId::MaxGridCells,
            exact_cells,
            exact_cells - 1,
        );
    }

    #[test]
    fn xychart_emit_surface_copy_observes_mid_row_cancellation() {
        let policy = AsciiResourcePolicy::default();
        let setup_resources = ResourceContext::new(policy);
        let mut source =
            StyledLine::with_resources(TerminalWidthProfile::Unicode, &setup_resources);
        source
            .try_push_role_repeat('#', 65, AsciiColorRole::ChartSeries(0))
            .expect("the source plot row should fit the default policy");

        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);
        let execution = AsciiExecution::new(&control, &policy);
        let base_resources = ResourceContext::new(policy);
        let emit_resources = execution.resource_context(&base_resources, OperationPhase::Emit);
        let mut target = StyledLine::with_resources(TerminalWidthProfile::Unicode, &emit_resources);
        let mut checkpoints = XyChartCheckpointCursor::new(execution);

        let error = target
            .try_push_line_with_checkpoint(&source, &emit_resources, || {
                checkpoints.tick(OperationPhase::Emit)
            })
            .expect_err("the second cadence checkpoint should stop plot-row emission");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Emit
                    && cancelled.reason == CancelReason::Requested
        ));
    }

    #[test]
    fn xychart_document_cells_are_admitted_before_materialization() {
        let model = compact_vertical_model(Some("12345678"));
        let options = compact_options(AsciiColorMode::Plain);
        let probe = Cell::new(false);
        let rejected = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, 123);
        let error = render_xychart_diagram_with_materializer(&model, &options, rejected, || {
            probe.set(true)
        })
        .expect_err("N-1 document cells must fail before row materialization");
        assert!(!probe.get(), "document admission materialized chart rows");
        assert_resource_error(error, AsciiResourceLimitId::MaxDocumentCells, 124, 123);

        let exact = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, 124);
        render_xychart_diagram_with_materializer(&model, &options, exact, || probe.set(true))
            .expect("the exact planned document-cell budget should render");
        assert!(
            probe.get(),
            "exact document admission did not materialize rows"
        );
    }

    #[test]
    fn chart_document_materialization_error_rolls_back_shared_ledgers() {
        let policy = AsciiResourcePolicy::default();
        let mut resources = ResourceContext::new(policy);
        resources
            .charge_layout_work(7)
            .expect("the fixture work debit should fit");
        let mut plan = super::ChartDocumentPlan::default();
        plan.include_line(3, &resources)
            .expect("the fixture document plan should fit");
        let materialized_extent = plan
            .admit_grid(&resources)
            .expect("the fixture grid should fit");

        let error = plan
            .materialize(
                materialized_extent,
                &mut resources,
                || {},
                |resources| {
                    resources.charge_layout_work(5)?;
                    resources.charge_document_cells(3)?;
                    Err(AsciiError::UnsupportedFeature {
                        diagram_type: "xychart",
                        feature: "test materialization failure",
                    })
                },
            )
            .expect_err("the injected materialization failure should escape");

        assert!(matches!(error, AsciiError::UnsupportedFeature { .. }));
        assert_eq!(resources.layout_work_used(), 7);
        assert_eq!(resources.document_cells_used(), 0);
    }

    #[test]
    fn chart_document_concurrent_grid_admission_prefers_layout_cancellation() {
        let policy = resources_with_limit(AsciiResourceLimitId::MaxGridCells, 5);
        let setup_resources = ResourceContext::new(policy);
        let mut plan = super::ChartDocumentPlan::default();
        plan.include_line(3, &setup_resources)
            .expect("the visible fixture row should fit");
        plan.include_transient_grid(3, &setup_resources)
            .expect("the transient fixture grid should fit arithmetically");

        let control = OperationControl::new();
        control.cancel();
        let execution = AsciiExecution::new(&control, &policy);
        let resources = execution.resource_context(&setup_resources, OperationPhase::Layout);
        let error = plan
            .admit_grid(&resources)
            .expect_err("cancellation must win over the simultaneous grid overflow");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), 0);
        assert_eq!(resources.document_cells_used(), 0);

        let error = plan
            .admit_grid(&setup_resources)
            .expect_err("without cancellation the concurrent grid must exceed the limit");
        assert_resource_error(error, AsciiResourceLimitId::MaxGridCells, 6, 5);
    }

    #[test]
    fn xychart_horizontal_plot_budget_is_checked_before_row_allocation() {
        let model = compact_horizontal_model();
        let options = compact_options(AsciiColorMode::Plain);
        let accepted = resources_with_limit(AsciiResourceLimitId::MaxGridCells, 224);
        render_xychart_diagram_with_resources(&model, &options, accepted)
            .expect("the exact source-plus-document grid limit should succeed");

        let rejected = resources_with_limit(AsciiResourceLimitId::MaxGridCells, 223);
        let error = render_xychart_diagram_with_resources(&model, &options, rejected)
            .expect_err("N-1 must reject before source rows and the document coexist");
        assert_resource_error(error, AsciiResourceLimitId::MaxGridCells, 224, 223);
    }

    #[test]
    fn xychart_horizontal_document_budget_counts_each_planned_row_exactly() {
        let mut model = compact_horizontal_model();
        model.display.show_data_label = true;
        model.display.show_data_label_outside_bar = true;
        model.plots[0].values.clear();
        model.plots[0].data = vec![("A".to_string(), Some(123_456.0)), ("B".to_string(), None)];
        let options = compact_options(AsciiColorMode::Plain);
        let reference = render_xychart_diagram(&model, &options)
            .expect("the unrestricted fixture should render for independent measurement");
        let exact_cells = reference
            .lines()
            .map(|line| super::display_width_with_profile(line, options.terminal_width_profile))
            .sum::<usize>();
        assert_eq!(exact_cells, 316);

        let exact_probe = Cell::new(false);
        let exact = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells);
        let rendered = render_xychart_diagram_with_materializer(&model, &options, exact, || {
            exact_probe.set(true)
        })
        .expect("the exact sum of the short and extended rows should render");
        assert!(rendered.contains("123456"));
        assert!(
            exact_probe.get(),
            "exact horizontal document admission did not materialize rows"
        );

        let below_probe = Cell::new(false);
        let below = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells - 1);
        let error = render_xychart_diagram_with_materializer(&model, &options, below, || {
            below_probe.set(true)
        })
        .expect_err("N-1 horizontal document cells must fail before materialization");
        assert!(
            !below_probe.get(),
            "N-1 horizontal document admission materialized rows"
        );
        assert_resource_error(
            error,
            AsciiResourceLimitId::MaxDocumentCells,
            exact_cells,
            exact_cells - 1,
        );
    }

    #[test]
    fn xychart_document_budget_counts_trimmed_rows_in_every_mode() {
        let model = compact_vertical_model(Some("12345678"));
        for color_mode in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Ansi256,
            AsciiColorMode::TrueColor,
            AsciiColorMode::Html,
        ] {
            let options = compact_options(color_mode);
            let accepted = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, 124);
            render_xychart_diagram_with_resources(&model, &options, accepted)
                .expect("the exact trimmed document-cell limit should succeed");

            let rejected = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, 123);
            let error = render_xychart_diagram_with_resources(&model, &options, rejected)
                .expect_err("the owned title, domains, and plot rows should exceed 123 cells");
            assert_resource_error(error, AsciiResourceLimitId::MaxDocumentCells, 124, 123);
        }
    }

    #[test]
    fn xychart_vertical_document_budget_uses_retained_plot_widths() {
        let mut model = compact_vertical_model(None);
        model.x_axis = XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string(), "B".to_string()],
        };
        model.plots[0].values = vec![1.0, 0.0];
        let options = AsciiRenderOptions::ascii()
            .with_xychart_vertical_plot_height(2)
            .with_xychart_category_band_width(3);
        let reference = render_xychart_diagram(&model, &options)
            .expect("the unrestricted sparse vertical plot should render");
        let exact_cells = reference
            .lines()
            .map(|line| display_width_with_profile(line, options.terminal_width_profile))
            .sum::<usize>();
        assert_eq!(
            reference,
            concat!(
                "xDomain: band categories=[bytes=1=\"A\", bytes=1=\"B\"]\n",
                "yDomain: linear authored=[0,1] resolved=[0,1]\n",
                "###\n",
                "###\n",
            )
        );
        assert_eq!(exact_cells, 102);

        let exact_probe = Cell::new(false);
        let exact = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells);
        render_xychart_diagram_with_materializer(&model, &options, exact, || exact_probe.set(true))
            .expect("the exact retained vertical document should render");
        assert!(exact_probe.get());

        let below_probe = Cell::new(false);
        let below = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells - 1);
        let error = render_xychart_diagram_with_materializer(&model, &options, below, || {
            below_probe.set(true)
        })
        .expect_err("N-1 retained vertical cells must fail before materialization");
        assert!(!below_probe.get());
        assert_resource_error(
            error,
            AsciiResourceLimitId::MaxDocumentCells,
            exact_cells,
            exact_cells - 1,
        );
    }

    #[test]
    fn xychart_vertical_category_row_admits_its_retained_width_exactly() {
        let mut model = compact_vertical_model(None);
        model.display.x_axis.show_label = true;
        let options = compact_options(AsciiColorMode::Plain).with_xychart_category_band_width(3);
        let reference = render_xychart_diagram(&model, &options)
            .expect("the unrestricted category-row fixture should render");
        let exact_cells = reference
            .lines()
            .map(|line| display_width_with_profile(line, options.terminal_width_profile))
            .sum::<usize>();

        let exact_probe = Cell::new(false);
        let exact = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells);
        render_xychart_diagram_with_materializer(&model, &options, exact, || exact_probe.set(true))
            .expect("the exact retained category-row budget should render");
        assert!(exact_probe.get());

        let below_probe = Cell::new(false);
        let below = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells - 1);
        let error = render_xychart_diagram_with_materializer(&model, &options, below, || {
            below_probe.set(true)
        })
        .expect_err("N-1 retained category cells must fail before materialization");
        assert!(!below_probe.get());
        assert_resource_error(
            error,
            AsciiResourceLimitId::MaxDocumentCells,
            exact_cells,
            exact_cells - 1,
        );
    }

    #[test]
    fn xychart_vertical_tick_only_baseline_admits_its_retained_width_exactly() {
        let mut model = compact_vertical_model(None);
        model.x_axis = XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string(), "B".to_string()],
        };
        model.plots[0].values = vec![1.0, 0.0];
        model.display.x_axis = XyChartAxisDisplayPolicy {
            show_label: false,
            show_title: false,
            show_tick: true,
            show_axis_line: false,
        };

        let plain_options =
            compact_options(AsciiColorMode::Plain).with_xychart_category_band_width(3);
        let reference = render_xychart_diagram(&model, &plain_options)
            .expect("the unrestricted tick-only fixture should render");
        let exact_cells = reference
            .lines()
            .map(|line| display_width_with_profile(line, plain_options.terminal_width_profile))
            .sum::<usize>();

        for color_mode in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Html,
        ] {
            let options = compact_options(color_mode).with_xychart_category_band_width(3);
            let exact_probe = Cell::new(false);
            let exact = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells);
            render_xychart_diagram_with_materializer(&model, &options, exact, || {
                exact_probe.set(true)
            })
            .expect("the exact retained tick-only baseline budget should render");
            assert!(exact_probe.get());

            let below_probe = Cell::new(false);
            let below =
                resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells - 1);
            let error = render_xychart_diagram_with_materializer(&model, &options, below, || {
                below_probe.set(true)
            })
            .expect_err("N-1 tick-only baseline cells must fail before materialization");
            assert!(!below_probe.get());
            assert_resource_error(
                error,
                AsciiResourceLimitId::MaxDocumentCells,
                exact_cells,
                exact_cells - 1,
            );
        }
    }

    #[test]
    fn xychart_vertical_label_only_baseline_admits_its_retained_width_exactly() {
        let mut model = compact_vertical_model(None);
        model.display.y_axis = XyChartAxisDisplayPolicy {
            show_label: true,
            show_title: false,
            show_tick: false,
            show_axis_line: false,
        };

        let plain_options =
            compact_options(AsciiColorMode::Plain).with_xychart_category_band_width(3);
        let reference = render_xychart_diagram(&model, &plain_options)
            .expect("the unrestricted label-only fixture should render");
        let exact_cells = reference
            .lines()
            .map(|line| display_width_with_profile(line, plain_options.terminal_width_profile))
            .sum::<usize>();

        for color_mode in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Html,
        ] {
            let options = compact_options(color_mode).with_xychart_category_band_width(3);
            let exact_probe = Cell::new(false);
            let exact = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells);
            render_xychart_diagram_with_materializer(&model, &options, exact, || {
                exact_probe.set(true)
            })
            .expect("the exact retained label-only baseline budget should render");
            assert!(exact_probe.get());

            let below_probe = Cell::new(false);
            let below =
                resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells - 1);
            let error = render_xychart_diagram_with_materializer(&model, &options, below, || {
                below_probe.set(true)
            })
            .expect_err("N-1 label-only baseline cells must fail before materialization");
            assert!(!below_probe.get());
            assert_resource_error(
                error,
                AsciiResourceLimitId::MaxDocumentCells,
                exact_cells,
                exact_cells - 1,
            );
        }
    }

    #[test]
    fn xychart_vertical_outside_label_row_admits_its_retained_width_exactly() {
        let mut model = compact_vertical_model(None);
        model.display.show_data_label = true;
        model.display.show_data_label_outside_bar = true;
        let options = compact_options(AsciiColorMode::Plain).with_xychart_category_band_width(3);
        let reference = render_xychart_diagram(&model, &options)
            .expect("the unrestricted outside-label fixture should render");
        let exact_cells = reference
            .lines()
            .map(|line| display_width_with_profile(line, options.terminal_width_profile))
            .sum::<usize>();

        let exact_probe = Cell::new(false);
        let exact = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells);
        render_xychart_diagram_with_materializer(&model, &options, exact, || exact_probe.set(true))
            .expect("the exact retained outside-label budget should render");
        assert!(exact_probe.get());

        let below_probe = Cell::new(false);
        let below = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells - 1);
        let error = render_xychart_diagram_with_materializer(&model, &options, below, || {
            below_probe.set(true)
        })
        .expect_err("N-1 retained outside-label cells must fail before materialization");
        assert!(!below_probe.get());
        assert_resource_error(
            error,
            AsciiResourceLimitId::MaxDocumentCells,
            exact_cells,
            exact_cells - 1,
        );
    }

    #[test]
    fn xychart_vertical_inside_label_budget_tracks_final_trim_by_color_mode() {
        let mut model = compact_vertical_model(None);
        model.display.show_data_label = true;

        for (color_mode, exact_cells) in [
            (AsciiColorMode::Plain, 88usize),
            (AsciiColorMode::TrueColor, 89usize),
        ] {
            let options = compact_options(color_mode).with_xychart_category_band_width(3);
            let exact_probe = Cell::new(false);
            let exact = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells);
            render_xychart_diagram_with_materializer(&model, &options, exact, || {
                exact_probe.set(true)
            })
            .expect("the exact inside-label document budget should render");
            assert!(exact_probe.get());

            let below_probe = Cell::new(false);
            let below =
                resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells - 1);
            let error = render_xychart_diagram_with_materializer(&model, &options, below, || {
                below_probe.set(true)
            })
            .expect_err("N-1 inside-label cells must fail before materialization");
            assert!(!below_probe.get());
            assert_resource_error(
                error,
                AsciiResourceLimitId::MaxDocumentCells,
                exact_cells,
                exact_cells - 1,
            );
        }
    }

    #[test]
    fn xychart_overlapping_inside_labels_admit_last_writer_width_exactly() {
        let mut model = compact_vertical_model(None);
        model.x_axis = XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string()],
        };
        model.y_axis = XyChartAxisRenderModel::Linear {
            title: String::new(),
            min: Some(-100.0),
            max: Some(100.0),
        };
        model.plots[0].values.clear();
        model.plots[0].data = vec![("A".to_string(), Some(10.0)), ("A".to_string(), Some(9.0))];
        model.display.show_data_label = true;
        let options = compact_options(AsciiColorMode::Plain)
            .with_xychart_vertical_plot_height(2)
            .with_xychart_category_band_width(4);
        let reference = render_xychart_diagram(&model, &options)
            .expect("the unrestricted overlapping-label fixture should render");
        assert!(reference.ends_with("\n 9\n"), "{reference}");
        let exact_cells = reference
            .lines()
            .map(|line| display_width_with_profile(line, options.terminal_width_profile))
            .sum::<usize>();

        let exact_probe = Cell::new(false);
        let exact = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells);
        render_xychart_diagram_with_materializer(&model, &options, exact, || exact_probe.set(true))
            .expect("the exact last-writer document budget should render");
        assert!(exact_probe.get());

        let below_probe = Cell::new(false);
        let below = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells - 1);
        let error = render_xychart_diagram_with_materializer(&model, &options, below, || {
            below_probe.set(true)
        })
        .expect_err("N-1 overlapping-label cells must fail before materialization");
        assert!(!below_probe.get());
        assert_resource_error(
            error,
            AsciiResourceLimitId::MaxDocumentCells,
            exact_cells,
            exact_cells - 1,
        );
    }

    #[test]
    fn xychart_plain_horizontal_prefix_uses_plain_finalizer_trim_semantics() {
        let mut model = compact_horizontal_model();
        model.x_axis = XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A ".to_string()],
        };
        model.plots[0].values = vec![0.0];
        model.display.x_axis.show_label = true;
        let options = compact_options(AsciiColorMode::Plain);
        let reference = render_xychart_diagram(&model, &options)
            .expect("the unrestricted horizontal-prefix fixture should render");
        let exact_cells = reference
            .lines()
            .map(|line| display_width_with_profile(line, options.terminal_width_profile))
            .sum::<usize>();

        let exact_probe = Cell::new(false);
        let exact = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells);
        render_xychart_diagram_with_materializer(&model, &options, exact, || exact_probe.set(true))
            .expect("the exact Plain prefix budget should render");
        assert!(exact_probe.get());

        let below_probe = Cell::new(false);
        let below = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells - 1);
        let error = render_xychart_diagram_with_materializer(&model, &options, below, || {
            below_probe.set(true)
        })
        .expect_err("N-1 Plain prefix cells must fail before materialization");
        assert!(!below_probe.get());
        assert_resource_error(
            error,
            AsciiResourceLimitId::MaxDocumentCells,
            exact_cells,
            exact_cells - 1,
        );
    }

    #[test]
    fn xychart_horizontal_empty_baseline_admits_zero_retained_cells_exactly() {
        let mut model = compact_horizontal_model();
        model.display.x_axis = XyChartAxisDisplayPolicy {
            show_label: true,
            show_title: false,
            show_tick: false,
            show_axis_line: false,
        };
        model.display.y_axis = XyChartAxisDisplayPolicy {
            show_label: true,
            show_title: false,
            show_tick: false,
            show_axis_line: false,
        };

        let plain_options = compact_options(AsciiColorMode::Plain);
        let reference = render_xychart_diagram(&model, &plain_options)
            .expect("the unrestricted empty-baseline fixture should render");
        let exact_cells = reference
            .lines()
            .map(|line| display_width_with_profile(line, plain_options.terminal_width_profile))
            .sum::<usize>();

        for color_mode in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Html,
        ] {
            let options = compact_options(color_mode);
            let exact_probe = Cell::new(false);
            let exact = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells);
            render_xychart_diagram_with_materializer(&model, &options, exact, || {
                exact_probe.set(true)
            })
            .expect("the exact empty-baseline document budget should render");
            assert!(exact_probe.get());

            let below_probe = Cell::new(false);
            let below =
                resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells - 1);
            let error = render_xychart_diagram_with_materializer(&model, &options, below, || {
                below_probe.set(true)
            })
            .expect_err("N-1 empty-baseline cells must fail before materialization");
            assert!(!below_probe.get());
            assert_resource_error(
                error,
                AsciiResourceLimitId::MaxDocumentCells,
                exact_cells,
                exact_cells - 1,
            );
        }
    }

    #[test]
    fn horizontal_prefix_metrics_preserve_styled_trailing_spaces() {
        let resources = ResourceContext::new(AsciiResourcePolicy::default());

        assert_eq!(
            super::axis_prefix_retained_width(
                "A ",
                2,
                4,
                true,
                None,
                false,
                TerminalWidthProfile::Unicode,
                &resources,
            )
            .expect("plain prefix metrics should fit"),
            1
        );
        assert_eq!(
            super::axis_prefix_retained_width(
                "A ",
                2,
                4,
                true,
                None,
                true,
                TerminalWidthProfile::Unicode,
                &resources,
            )
            .expect("styled prefix metrics should fit"),
            2
        );
    }

    #[test]
    fn xychart_plain_output_budget_is_enforced_during_final_encoding() {
        let model = compact_vertical_model(None);
        let options = compact_options(AsciiColorMode::Plain);
        let accepted = resources_with_limit(AsciiResourceLimitId::MaxOutputBytes, 89);
        let rendered = render_xychart_diagram_with_resources(&model, &options, accepted)
            .expect("the exact plain-output byte limit should succeed");
        assert_eq!(
            rendered,
            concat!(
                "xDomain: band categories=[bytes=1=\"A\"]\n",
                "yDomain: linear authored=[0,1] resolved=[0,1]\n",
                "#\n",
                "#\n",
            )
        );

        let rejected = resources_with_limit(AsciiResourceLimitId::MaxOutputBytes, 88);
        let error = render_xychart_diagram_with_resources(&model, &options, rejected)
            .expect_err("the final newline should cross the output-byte boundary");
        assert_resource_error(error, AsciiResourceLimitId::MaxOutputBytes, 89, 88);
    }

    #[test]
    fn xychart_final_styled_line_count_prefers_cancellation_to_output_ceiling() {
        let options = compact_options(AsciiColorMode::Plain);
        let policy = resources_with_limit(AsciiResourceLimitId::MaxOutputBytes, 1);
        let line_resources = ResourceContext::new(AsciiResourcePolicy::default());
        let mut line = StyledLine::with_resources(options.terminal_width_profile, &line_resources);
        line.try_push_role_text("AB", AsciiColorRole::Text)
            .expect("the fixture chart line should fit");
        let document = ChartDocument {
            width: line.len(),
            lines: vec![line],
        };
        let mut resources = ResourceContext::new(policy);
        resources
            .charge_layout_work(7)
            .expect("the existing ledger debit should fit");
        let work_before = resources.layout_work_used();
        let document_before = resources.document_cells_used();
        let control = OperationControl::new();
        control.cancel_after_checkpoints(0);
        let execution = AsciiExecution::new(&control, &policy);

        let error = finish_chart_lines_controlled(document, &options, &mut resources, execution)
            .expect_err("the XYChart finalizer must keep execution through line counting");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Emit
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), work_before);
        assert_eq!(resources.document_cells_used(), document_before);
    }

    #[test]
    fn xychart_grapheme_budget_rejects_zwj_before_arena_insertion() {
        let model = compact_vertical_model(Some("👩‍💻"));
        let options = compact_options(AsciiColorMode::Plain);
        let accepted = resources_with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 11);
        render_xychart_diagram_with_resources(&model, &options, accepted)
            .expect("the exact UTF-8 grapheme-byte limit should succeed");

        let rejected = resources_with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 10);
        let error = render_xychart_diagram_with_resources(&model, &options, rejected)
            .expect_err("the authored ZWJ grapheme should be rejected before insertion");
        assert_resource_error(error, AsciiResourceLimitId::MaxGraphemeBytes, 11, 10);
    }

    #[test]
    fn xychart_layout_work_budget_covers_series_values_categories_paint_and_encoding() {
        let model = compact_vertical_model(None);
        let options = compact_options(AsciiColorMode::Plain);
        // This is a fixed semantic boundary for the compact fixture. It covers the declared
        // cardinality, series/sample planning, retained-width admission, paint, and encoding
        // work; it is intentionally not derived from a renderer-reported usage value.
        const REQUIRED_LAYOUT_WORK: usize = 319;
        let accepted = resources_with_limit(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            REQUIRED_LAYOUT_WORK,
        );
        render_xychart_diagram_with_resources(&model, &options, accepted)
            .expect("the exact XYChart layout-work limit should succeed");

        let rejected = resources_with_limit(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            REQUIRED_LAYOUT_WORK - 1,
        );
        let error = render_xychart_diagram_with_resources(&model, &options, rejected)
            .expect_err("the final encoder count/write passes should cross the work boundary");
        assert_resource_error(
            error,
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            REQUIRED_LAYOUT_WORK,
            REQUIRED_LAYOUT_WORK - 1,
        );
    }

    #[test]
    fn xychart_plot_geometry_overflow_fails_before_row_allocation() {
        let model = compact_vertical_model(None);
        let options =
            compact_options(AsciiColorMode::Plain).with_xychart_category_band_width(usize::MAX);
        let error = render_xychart_diagram(&model, &options)
            .expect_err("overflow-shaped plot geometry should be rejected");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a resource-limit error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
        assert_eq!(details.actual, usize::MAX);
    }
}

mod disclosure;
mod empty;

use super::plot::{
    ChartChars, SeriesPlan, TerminalChartPlan, ValueRange, XyChartPlotArea,
    apply_vertical_bar_data_labels, build_horizontal_plot_rows, build_vertical_plot,
    checked_grid_sub, format_data_number, format_tick_number, horizontal_bar_width,
};
use crate::canvas::finish_styled_lines_with_resources;
use crate::color::AsciiColorRole;
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
    TitleOwner, band_domain_disclosure_line_width, push_title_display_line,
    push_value_disclosure_lines, title_display_line_width, value_disclosure_line_width,
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ChartDocumentPlan {
    width: usize,
    height: usize,
}

impl ChartDocumentPlan {
    fn include_line(&mut self, width: usize, resources: &ResourceContext) -> Result<()> {
        self.width = self.width.max(width);
        self.height = resources.checked_grid_add(self.height, 1)?;
        Ok(())
    }

    fn include_repeated_line(
        &mut self,
        width: usize,
        count: usize,
        resources: &ResourceContext,
    ) -> Result<()> {
        if count == 0 {
            return Ok(());
        }
        self.width = self.width.max(width);
        self.height = resources.checked_grid_add(self.height, count)?;
        Ok(())
    }

    fn materialize(
        self,
        resources: &mut ResourceContext,
        before_materialize: impl FnOnce(),
        materialize: impl FnOnce(&mut ResourceContext) -> Result<ChartDocument>,
    ) -> Result<ChartDocument> {
        resources.grid_extent(self.width, self.height)?;
        before_materialize();
        let document = materialize(resources)?;
        if document.width != self.width || document.lines.len() != self.height {
            return Err(invalid_chart_document_extent_plan());
        }
        Ok(document)
    }
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
    render_xychart_diagram_controlled(
        model,
        options,
        AsciiExecution::standalone(&resources),
        || {},
    )
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
        AsciiExecution::standalone(&resources),
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
        return render_horizontal(
            model,
            &plan,
            context,
            &mut resources,
            execution,
            before_document_materialize,
        );
    }

    render_vertical(
        model,
        &plan,
        context,
        &mut resources,
        execution,
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
    let disclosure = plan.disclosure_plan(plot_area, false, resources)?;
    let requires_disclosure = disclosure.values;
    let show_y_labels = axis_labels_visible(model.display.y_axis);
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
    let document_plan = measure_vertical_document(
        model,
        plan,
        chars,
        plot_extent,
        plot_prefix_width,
        show_y_labels,
        baseline_mark,
        requires_disclosure,
        disclosure,
        options,
        resources,
    )?;
    execution.admit_grid(document_plan.width.saturating_mul(document_plan.height))?;
    execution.checkpoint(merman_core::OperationPhase::Emit)?;
    let mut emit_resources =
        execution.resource_context(resources, merman_core::OperationPhase::Emit);
    let out = document_plan.materialize(
        &mut emit_resources,
        before_document_materialize,
        |resources| {
            let mut plot_resources = resources.scoped();
            let mut plot =
                build_vertical_plot(plan, chars, plot_area, plot_extent, &mut plot_resources)?;
            let mut out = ChartDocument::default();
            push_title_lines(&mut out, model, options, resources)?;
            push_legend_line(&mut out, plan, chars, options, resources)?;
            if (model.display.show_data_label && !uses_compact_bar_data_labels(model))
                || requires_disclosure
            {
                push_value_disclosure_lines(
                    &mut out, model, plan, chars, disclosure, options, resources,
                )?;
            }

            if model.display.show_data_label && uses_compact_bar_data_labels(model) {
                if model.display.show_data_label_outside_bar {
                    if let Some(line) = vertical_data_label_line(
                        model,
                        plan,
                        plot_prefix_width,
                        plot_area,
                        resources,
                    )? {
                        out.push(line, resources)?;
                    }
                } else {
                    apply_vertical_bar_data_labels(
                        &mut plot,
                        plan,
                        plot_area,
                        &mut plot_resources,
                    )?;
                }
            }

            for (idx, row) in plot.rows.into_iter().enumerate() {
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
                resources.charge_layout_work(row.len())?;
                line.try_push_line(&row)?;
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
                    )?;
                }
                out.push(axis_line, resources)?;
            }

            if axis_labels_visible(model.display.x_axis) {
                charge_category_text(categories, resources)?;
                let labels = plot_area.category_axis_labels(categories, resources)?;
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
    let disclosure = plan.disclosure_plan(plot_area, true, resources)?;
    let requires_disclosure = disclosure.values;
    let show_x_labels = axis_labels_visible(model.display.x_axis);
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
    let document_plan = measure_horizontal_document(
        model,
        plan,
        chars,
        plot_area,
        plot_extent,
        plot_prefix_width,
        baseline_mark,
        requires_disclosure,
        disclosure,
        options,
        resources,
    )?;
    execution.admit_grid(document_plan.width.saturating_mul(document_plan.height))?;
    execution.checkpoint(merman_core::OperationPhase::Emit)?;
    let mut emit_resources =
        execution.resource_context(resources, merman_core::OperationPhase::Emit);
    let out = document_plan.materialize(
        &mut emit_resources,
        before_document_materialize,
        |resources| {
            let mut plot_resources = resources.scoped();
            let plot_rows = build_horizontal_plot_rows(
                plan,
                chars,
                plot_area,
                plot_extent,
                &mut plot_resources,
            )?;
            let mut out = ChartDocument::default();
            push_title_lines(&mut out, model, options, resources)?;
            push_legend_line(&mut out, plan, chars, options, resources)?;
            if (model.display.show_data_label && !uses_compact_bar_data_labels(model))
                || requires_disclosure
            {
                push_value_disclosure_lines(
                    &mut out, model, plan, chars, disclosure, options, resources,
                )?;
            }

            for plot_row in &plot_rows {
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
                resources.charge_layout_work(plot_row.line.len())?;
                line.try_push_line(&plot_row.line)?;
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
                    )?;
                }
                out.push(axis_line, resources)?;
            }

            if axis_labels_visible(model.display.y_axis) {
                let tick_labels = horizontal_tick_label_line(y_range, plot_area, resources)?;
                let mut tick_line = new_chart_line(options, resources);
                resources.charge_layout_work(plot_prefix_width)?;
                tick_line.try_push_spaces(plot_prefix_width)?;
                resources.charge_layout_work(tick_labels.len())?;
                tick_line.try_push_line(&tick_labels)?;
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
    plot_extent: LogicalExtent,
    plot_prefix_width: usize,
    show_y_labels: bool,
    baseline_mark: Option<char>,
    requires_disclosure: bool,
    disclosure: super::plot::TerminalDisclosurePlan,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<ChartDocumentPlan> {
    let mut document = ChartDocumentPlan::default();
    measure_chart_header(&mut document, model, plan, chars, options, resources)?;
    if (model.display.show_data_label && !uses_compact_bar_data_labels(model))
        || requires_disclosure
    {
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
    if model.display.show_data_label
        && uses_compact_bar_data_labels(model)
        && model.display.show_data_label_outside_bar
    {
        document.include_line(plot_row_width, resources)?;
    }
    document.include_repeated_line(plot_row_width, plot_extent.height(), resources)?;

    if show_y_labels || baseline_mark.is_some() {
        let baseline_width =
            if model.display.x_axis.show_axis_line || model.display.x_axis.show_tick {
                plot_row_width
            } else {
                plot_prefix_width
            };
        document.include_line(baseline_width, resources)?;
    }
    if axis_labels_visible(model.display.x_axis) {
        document.include_line(plot_row_width, resources)?;
    }
    if model.display.x_axis.show_title
        && let Some(title) = nonempty_axis_title(&model.x_axis)
    {
        let width = title_display_line_width(TitleOwner::XAxis, title, options, resources)?;
        document.include_line(width, resources)?;
    }
    Ok(document)
}

#[allow(clippy::too_many_arguments)]
fn measure_horizontal_document(
    model: &XyChartDiagramRenderModel,
    plan: &TerminalChartPlan,
    chars: ChartChars,
    plot_area: XyChartPlotArea,
    plot_extent: LogicalExtent,
    plot_prefix_width: usize,
    baseline_mark: Option<char>,
    requires_disclosure: bool,
    disclosure: super::plot::TerminalDisclosurePlan,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<ChartDocumentPlan> {
    let mut document = ChartDocumentPlan::default();
    measure_chart_header(&mut document, model, plan, chars, options, resources)?;
    if (model.display.show_data_label && !uses_compact_bar_data_labels(model))
        || requires_disclosure
    {
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

    let plot_row_width =
        horizontal_plot_row_width(model, plan, plot_prefix_width, plot_area, resources)?;
    document.include_repeated_line(plot_row_width, plot_extent.height(), resources)?;

    if axis_labels_visible(model.display.y_axis) || baseline_mark.is_some() {
        let baseline_width =
            if model.display.y_axis.show_axis_line || model.display.y_axis.show_tick {
                resources.checked_grid_add(plot_prefix_width, plot_area.horizontal_width)?
            } else {
                plot_prefix_width
            };
        document.include_line(baseline_width, resources)?;
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

fn horizontal_plot_row_width(
    model: &XyChartDiagramRenderModel,
    plan: &TerminalChartPlan,
    plot_prefix_width: usize,
    plot_area: XyChartPlotArea,
    resources: &mut ResourceContext,
) -> Result<usize> {
    let base_width = resources.checked_grid_add(plot_prefix_width, plot_area.horizontal_width)?;
    if !model.display.show_data_label || !uses_compact_bar_data_labels(model) {
        return Ok(base_width);
    }
    let Some(series) = plan
        .series
        .iter()
        .find(|series| series.plot_type == XyChartPlotType::Bar)
    else {
        return Ok(base_width);
    };

    let mut width = base_width;
    for slot in 0..plan.slot_count {
        let mut value = None;
        for datum in &series.data {
            resources.charge_layout_work(1)?;
            if plan.sample_slot(datum) == slot {
                value = datum.value;
                break;
            }
        }
        let Some(value) = value else {
            continue;
        };
        let label = format_data_number(value);
        let label_width = display_width_with_profile(&label, plot_area.width_profile);
        let fits_inside = !model.display.show_data_label_outside_bar
            && horizontal_bar_width(value, plan.y_range, plot_area) > 0
            && label_width > 0
            && label_width <= horizontal_bar_width(value, plan.y_range, plot_area);
        if !fits_inside && !label.is_empty() {
            let outside = resources
                .checked_grid_add(base_width, resources.checked_grid_add(1, label_width)?)?;
            width = width.max(outside);
        }
    }
    Ok(width)
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
) -> Result<()> {
    for index in 0..category_count {
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
) -> Result<()> {
    let last = plot_area
        .horizontal_width
        .checked_sub(1)
        .ok_or(AsciiError::InvalidOption {
            field: "xychart_horizontal_plot_width",
            message: "must be at least 2",
        })?;
    for position in [0, last] {
        resources.charge_layout_work(1)?;
        let target = resources.checked_grid_add(plot_start, position)?;
        line.try_set_role(target, tick, AsciiColorRole::ChartAxis)?;
    }
    Ok(())
}

fn vertical_data_label_line(
    model: &XyChartDiagramRenderModel,
    plan: &TerminalChartPlan,
    plot_prefix_width: usize,
    plot_area: XyChartPlotArea,
    resources: &mut ResourceContext,
) -> Result<Option<ChartLine>> {
    let Some(labels) = compact_bar_value_labels(model, plan, resources)? else {
        return Ok(None);
    };
    if labels.is_empty() {
        return Ok(None);
    }

    let band_labels = plot_area.band_labels(&labels, resources)?;
    let mut line = ChartLine::with_resources(plot_area.width_profile, resources);
    resources.charge_layout_work(plot_prefix_width)?;
    line.try_push_spaces(plot_prefix_width)?;
    line.try_push_role_text_with_unstyled_trailing_spaces(&band_labels, AsciiColorRole::Text)?;
    Ok(Some(line))
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
    labels.resize(plan.slot_count, String::new());
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

fn finish_chart_lines(
    document: ChartDocument,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<String> {
    if document.lines.is_empty() {
        return Ok(String::new());
    }

    resources.grid_extent(document.width, document.lines.len())?;
    finish_styled_lines_with_resources(&document.lines, options, true, resources)
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
    let cells = document.width.saturating_mul(document.lines.len());
    execution.admit_grid(cells)?;
    for _ in &document.lines {
        execution.checkpoint(merman_core::OperationPhase::Emit)?;
    }
    let mut emit_resources =
        execution.resource_context(resources, merman_core::OperationPhase::Emit);
    finish_chart_lines(document, options, &mut emit_resources)
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
        charge_text_layout(resources, label)?;
        gutter = gutter.max(display_width_with_profile(label, width_profile));
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
        horizontal_tick_label_line_for_labels, render_xychart_diagram_with_materializer,
        render_xychart_diagram_with_resources,
    };
    use crate::resource::ResourceContext;
    use crate::{
        AsciiColorMode, AsciiError, AsciiRenderOptions, AsciiResourceLimitId, AsciiResourcePolicy,
        Result, TerminalWidthProfile,
    };
    use merman_core::diagrams::xychart::{
        XyChartAxisDisplayPolicy, XyChartAxisRenderModel, XyChartDiagramRenderModel,
        XyChartDisplayPolicy, XyChartPlotRenderModel, XyChartPlotType,
    };
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
            let accepted = resources_with_limit(AsciiResourceLimitId::MaxGridCells, 117);
            render_xychart_diagram_with_resources(&model, &options, accepted)
                .expect("the exact complete XYChart grid limit should succeed");

            let rejected = resources_with_limit(AsciiResourceLimitId::MaxGridCells, 116);
            let error = render_xychart_diagram_with_resources(&model, &options, rejected)
                .expect_err("the complete owned title plus plot grid should exceed 116 cells");
            assert_resource_error(error, AsciiResourceLimitId::MaxGridCells, 117, 116);
        }
    }

    #[test]
    fn xychart_complete_document_extent_is_admitted_before_row_materialization() {
        let model = disclosure_budget_model();
        let probe = Cell::new(false);
        let options = compact_options(AsciiColorMode::Plain);
        let trial = resources_with_limit(AsciiResourceLimitId::MaxGridCells, 100);
        let error =
            render_xychart_diagram_with_materializer(&model, &options, trial, || probe.set(true))
                .expect_err("the disclosure-dominated document should exceed the trial limit");
        assert!(!probe.get(), "trial overflow materialized chart rows");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a resource-limit error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
        let exact_cells = details.actual;
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
    fn xychart_horizontal_plot_budget_is_checked_before_row_allocation() {
        let model = compact_horizontal_model();
        let options = compact_options(AsciiColorMode::Plain);
        let accepted = resources_with_limit(AsciiResourceLimitId::MaxGridCells, 20);
        render_xychart_diagram_with_resources(&model, &options, accepted)
            .expect("the exact horizontal plot grid limit should succeed");

        let rejected = resources_with_limit(AsciiResourceLimitId::MaxGridCells, 19);
        let error = render_xychart_diagram_with_resources(&model, &options, rejected)
            .expect_err("the horizontal plot extent should be rejected before row allocation");
        assert_resource_error(error, AsciiResourceLimitId::MaxGridCells, 20, 19);
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
            let accepted = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, 41);
            render_xychart_diagram_with_resources(&model, &options, accepted)
                .expect("the exact trimmed document-cell limit should succeed");

            let rejected = resources_with_limit(AsciiResourceLimitId::MaxDocumentCells, 40);
            let error = render_xychart_diagram_with_resources(&model, &options, rejected)
                .expect_err("the owned title plus two plot rows should exceed 40 document cells");
            assert_resource_error(error, AsciiResourceLimitId::MaxDocumentCells, 41, 40);
        }
    }

    #[test]
    fn xychart_plain_output_budget_is_enforced_during_final_encoding() {
        let model = compact_vertical_model(None);
        let options = compact_options(AsciiColorMode::Plain);
        let accepted = resources_with_limit(AsciiResourceLimitId::MaxOutputBytes, 4);
        let rendered = render_xychart_diagram_with_resources(&model, &options, accepted)
            .expect("the exact plain-output byte limit should succeed");
        assert_eq!(rendered, "#\n#\n");

        let rejected = resources_with_limit(AsciiResourceLimitId::MaxOutputBytes, 3);
        let error = render_xychart_diagram_with_resources(&model, &options, rejected)
            .expect_err("the final newline should cross the output-byte boundary");
        assert_resource_error(error, AsciiResourceLimitId::MaxOutputBytes, 4, 3);
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
        let accepted = resources_with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 22);
        render_xychart_diagram_with_resources(&model, &options, accepted)
            .expect("the exact XYChart layout-work limit should succeed");

        let rejected = resources_with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 21);
        let error = render_xychart_diagram_with_resources(&model, &options, rejected)
            .expect_err("the final encoder count/write passes should cross the work boundary");
        assert_resource_error(error, AsciiResourceLimitId::MaxLayoutWorkUnits, 22, 21);
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

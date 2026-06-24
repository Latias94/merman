use super::plot::{
    ChartChars, ValueRange, XyChartPlotArea, build_horizontal_plot_rows, build_vertical_plot,
    format_number,
};
use crate::canvas::Canvas;
use crate::color::{AsciiColorMode, AsciiColorRole};
use crate::error::AsciiError;
use crate::text::{StyledLine, display_width};
use crate::{AsciiRenderOptions, Result};
use merman_core::diagrams::xychart::{
    XyChartAxisRenderModel, XyChartDiagramRenderModel, XyChartPlotRenderModel, XyChartPlotType,
};

type ChartLine = StyledLine;

pub(crate) fn render_xychart_diagram(
    model: &XyChartDiagramRenderModel,
    options: &AsciiRenderOptions,
) -> Result<String> {
    if model.plots.is_empty() {
        return Ok(String::new());
    }

    let chars = ChartChars::from_options(options);
    let categories = category_labels(model);
    if categories.is_empty() {
        return Ok(String::new());
    }

    let y_range = y_value_range(model);
    let plot_area = XyChartPlotArea::from_options(options);
    if model.orientation.eq_ignore_ascii_case("horizontal") {
        enforce_plot_cell_limit(plot_area.horizontal_cell_count(categories.len()), options)?;
        return Ok(render_horizontal(
            model,
            &categories,
            y_range,
            chars,
            plot_area,
            options,
        ));
    }

    enforce_plot_cell_limit(plot_area.vertical_cell_count(categories.len()), options)?;
    Ok(render_vertical(
        model,
        &categories,
        y_range,
        chars,
        plot_area,
        options,
    ))
}

fn enforce_plot_cell_limit(actual: usize, options: &AsciiRenderOptions) -> Result<()> {
    if actual > options.max_grid_cells {
        return Err(AsciiError::RenderLimitExceeded {
            actual,
            limit: options.max_grid_cells,
        });
    }

    Ok(())
}

fn render_vertical(
    model: &XyChartDiagramRenderModel,
    categories: &[String],
    y_range: ValueRange,
    chars: ChartChars,
    plot_area: XyChartPlotArea,
    options: &AsciiRenderOptions,
) -> String {
    let plot = build_vertical_plot(model, categories.len(), y_range, chars, plot_area);

    let mut out = Vec::new();
    push_title_lines(&mut out, model);
    push_legend_line(&mut out, model, chars);

    let tick_labels = vertical_tick_labels(y_range, plot_area);
    let min_label = format_number(y_range.min);
    let gutter = tick_labels
        .iter()
        .chain(std::iter::once(&min_label))
        .map(|s| display_width(s))
        .max()
        .unwrap_or(1);

    for (idx, row) in plot.rows.into_iter().enumerate() {
        let label = &tick_labels[idx];
        let mut line = ChartLine::new();
        line.push_right_aligned_role_text(label, gutter, AsciiColorRole::Text);
        line.push_plain_char(' ');
        line.push_role_char(chars.vertical_axis, AsciiColorRole::ChartAxis);
        line.push_cells(&row);
        out.push(line);
    }

    let mut axis_line = ChartLine::new();
    axis_line.push_right_aligned_role_text(&min_label, gutter, AsciiColorRole::Text);
    axis_line.push_plain_char(' ');
    axis_line.push_role_char(chars.origin, AsciiColorRole::ChartAxis);
    axis_line.push_role_repeat(chars.horizontal_axis, plot.width, AsciiColorRole::ChartAxis);
    out.push(axis_line);

    let mut category_line = ChartLine::new();
    category_line.push_spaces(gutter + 2);
    category_line.push_role_text_with_unstyled_trailing_spaces(
        &plot_area.category_axis_labels(categories),
        AsciiColorRole::Text,
    );
    out.push(category_line);

    if let Some(title) = x_axis_title(model) {
        let mut line = ChartLine::new();
        line.push_role_text("x: ", AsciiColorRole::Text);
        line.push_role_text(title, AsciiColorRole::Text);
        out.push(line);
    }

    finish_chart_lines(out, options)
}

fn render_horizontal(
    model: &XyChartDiagramRenderModel,
    categories: &[String],
    y_range: ValueRange,
    chars: ChartChars,
    plot_area: XyChartPlotArea,
    options: &AsciiRenderOptions,
) -> String {
    let mut out = Vec::new();
    push_title_lines(&mut out, model);
    push_legend_line(&mut out, model, chars);
    let plot_rows = build_horizontal_plot_rows(model, categories.len(), y_range, chars, plot_area);

    let gutter = categories
        .iter()
        .map(|c| display_width(c))
        .max()
        .unwrap_or(1);

    for (idx, category) in categories.iter().enumerate() {
        let plot_row = &plot_rows[idx];

        let mut line = ChartLine::new();
        line.push_right_aligned_role_text(category, gutter, AsciiColorRole::Text);
        line.push_plain_char(' ');
        line.push_role_char(chars.vertical_axis, AsciiColorRole::ChartAxis);
        line.push_cells(&plot_row.cells);
        if !plot_row.values.is_empty() {
            line.push_plain_char(' ');
            line.push_role_text(&plot_row.values.join("/"), AsciiColorRole::Text);
        }
        out.push(line);
    }

    let mut axis_line = ChartLine::new();
    axis_line.push_spaces(gutter + 1);
    axis_line.push_role_char(chars.origin, AsciiColorRole::ChartAxis);
    axis_line.push_role_repeat(
        chars.horizontal_axis,
        plot_area.horizontal_width,
        AsciiColorRole::ChartAxis,
    );
    out.push(axis_line);

    let mut tick_line = ChartLine::new();
    tick_line.push_spaces(gutter + 2);
    tick_line.push_role_text(
        &horizontal_tick_labels(y_range, plot_area),
        AsciiColorRole::Text,
    );
    out.push(tick_line);

    finish_chart_lines(out, options)
}

fn push_title_lines(out: &mut Vec<ChartLine>, model: &XyChartDiagramRenderModel) {
    if let Some(title) = model.title.as_deref().filter(|t| !t.trim().is_empty()) {
        out.push(ChartLine::role_text(title, AsciiColorRole::Text));
    }

    if let Some(title) = y_axis_title(model) {
        let mut line = ChartLine::new();
        line.push_role_text("y: ", AsciiColorRole::Text);
        line.push_role_text(title, AsciiColorRole::Text);
        out.push(line);
    }
}

fn push_legend_line(
    out: &mut Vec<ChartLine>,
    model: &XyChartDiagramRenderModel,
    chars: ChartChars,
) {
    if model.plots.len() <= 1 {
        return;
    }

    out.push(legend_line(&model.plots, chars));
}

fn legend_line(plots: &[XyChartPlotRenderModel], chars: ChartChars) -> ChartLine {
    let mut line = ChartLine::new();
    let mut bar_index = 0;
    let mut line_index = 0;

    for (series_index, plot) in plots.iter().enumerate() {
        if series_index > 0 {
            line.push_spaces(2);
        }

        line.push_role_char(
            chars.legend_symbol(plot.plot_type),
            AsciiColorRole::ChartSeries(series_index),
        );
        line.push_plain_char(' ');
        let label = match plot.plot_type {
            XyChartPlotType::Bar => {
                bar_index += 1;
                format!("Bar {bar_index}")
            }
            XyChartPlotType::Line => {
                line_index += 1;
                format!("Line {line_index}")
            }
        };
        line.push_role_text(&label, AsciiColorRole::Text);
    }

    line
}

fn category_labels(model: &XyChartDiagramRenderModel) -> Vec<String> {
    let data_count = model
        .plots
        .iter()
        .map(|plot| plot.values.len())
        .max()
        .unwrap_or(0);

    match &model.x_axis {
        XyChartAxisRenderModel::Band { categories, .. } => {
            let mut labels = categories.clone();
            labels.extend((labels.len()..data_count).map(|idx| (idx + 1).to_string()));
            labels
        }
        XyChartAxisRenderModel::Linear { min, max, .. } => linear_axis_labels(
            min.unwrap_or(1.0),
            max.unwrap_or(data_count as f64),
            data_count,
        ),
    }
}

fn linear_axis_labels(min: f64, max: f64, count: usize) -> Vec<String> {
    if count == 0 {
        return Vec::new();
    }
    if count == 1 {
        return vec![format_number(min)];
    }

    let step = (max - min) / ((count - 1) as f64);
    (0..count)
        .map(|idx| format_number(min + step * (idx as f64)))
        .collect()
}

fn y_value_range(model: &XyChartDiagramRenderModel) -> ValueRange {
    let mut data_min = f64::INFINITY;
    let mut data_max = f64::NEG_INFINITY;
    for value in model
        .plots
        .iter()
        .flat_map(|plot| plot.values.iter())
        .copied()
    {
        data_min = data_min.min(value);
        data_max = data_max.max(value);
    }

    let (axis_min, axis_max) = match &model.y_axis {
        XyChartAxisRenderModel::Linear { min, max, .. } => (*min, *max),
        XyChartAxisRenderModel::Band { .. } => (None, None),
    };

    let mut min = axis_min.unwrap_or_else(|| data_min.min(0.0));
    let mut max = axis_max.unwrap_or(data_max);

    if !min.is_finite() {
        min = 0.0;
    }
    if !max.is_finite() {
        max = min + 1.0;
    }
    if (max - min).abs() <= f64::EPSILON {
        max = min + 1.0;
    }

    ValueRange { min, max }
}

fn y_axis_title(model: &XyChartDiagramRenderModel) -> Option<&str> {
    match &model.y_axis {
        XyChartAxisRenderModel::Linear { title, .. }
        | XyChartAxisRenderModel::Band { title, .. } => non_empty(title),
    }
}

fn x_axis_title(model: &XyChartDiagramRenderModel) -> Option<&str> {
    match &model.x_axis {
        XyChartAxisRenderModel::Linear { title, .. }
        | XyChartAxisRenderModel::Band { title, .. } => non_empty(title),
    }
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn vertical_tick_labels(y_range: ValueRange, plot_area: XyChartPlotArea) -> Vec<String> {
    (1..=plot_area.vertical_height)
        .rev()
        .map(|level| {
            let value =
                y_range.min + (y_range.span() * (level as f64) / plot_area.vertical_height as f64);
            format_number(value)
        })
        .collect()
}

fn horizontal_tick_labels(y_range: ValueRange, plot_area: XyChartPlotArea) -> String {
    let min = format_number(y_range.min);
    let max = format_number(y_range.max);
    let mut cells = vec![' '; plot_area.horizontal_width];

    for (idx, ch) in min.chars().take(plot_area.horizontal_width).enumerate() {
        cells[idx] = ch;
    }

    let max_len = display_width(&max);
    let max_start = plot_area.horizontal_width.saturating_sub(max_len);
    for (idx, ch) in max.chars().enumerate() {
        if let Some(cell) = cells.get_mut(max_start + idx) {
            *cell = ch;
        }
    }

    chars_to_string(&cells)
}

fn chars_to_string(chars: &[char]) -> String {
    chars.iter().collect::<String>()
}

fn finish_chart_lines(lines: Vec<ChartLine>, options: &AsciiRenderOptions) -> String {
    if options.color_mode == AsciiColorMode::Plain {
        return finish_lines(lines.into_iter().map(|line| line.text()).collect());
    }

    if lines.is_empty() {
        return String::new();
    }

    let width = lines.iter().map(ChartLine::len).max().unwrap_or(0);
    if width == 0 {
        return "\n".repeat(lines.len());
    }

    let mut canvas = Canvas::new(width, lines.len());
    for (y, line) in lines.iter().enumerate() {
        line.write_to(&mut canvas, y);
    }

    canvas.finish_trimmed_with_options(options)
}

fn finish_lines(lines: Vec<String>) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    for line in lines {
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

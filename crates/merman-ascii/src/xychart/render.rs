use super::plot::{
    ChartChars, ValueRange, XyChartPlotArea, apply_vertical_bar_data_labels,
    build_horizontal_plot_rows, build_vertical_plot, format_number, horizontal_bar_width,
};
use crate::canvas::Canvas;
use crate::color::{AsciiColorMode, AsciiColorRole};
use crate::error::AsciiError;
use crate::text::{StyledLine, display_width, truncate_display_width};
use crate::{AsciiRenderOptions, Result};
use merman_core::diagrams::xychart::{
    XyChartAxisDisplayPolicy, XyChartAxisRenderModel, XyChartDiagramRenderModel,
    XyChartPlotRenderModel, XyChartPlotType,
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
    let mut plot = build_vertical_plot(model, categories.len(), y_range, chars, plot_area);

    let mut out = Vec::new();
    push_title_lines(&mut out, model);
    push_legend_line(&mut out, model, chars);
    if model.display.show_data_label && !uses_compact_bar_data_labels(model) {
        push_value_disclosure_lines(&mut out, model, categories, chars);
    }

    let show_y_labels = axis_labels_visible(model.display.y_axis);
    let tick_labels = if show_y_labels {
        vertical_tick_labels(y_range, plot_area)
    } else {
        vec![String::new(); plot_area.vertical_height]
    };
    let min_label = show_y_labels.then(|| format_number(y_range.min));
    let gutter = min_label
        .iter()
        .chain(tick_labels.iter())
        .map(|s| display_width(s))
        .max()
        .unwrap_or(0);
    let y_axis_mark = vertical_axis_mark(model.display.y_axis, chars);
    let plot_prefix_width = plot_prefix_width(show_y_labels, y_axis_mark.is_some(), gutter);

    if model.display.show_data_label && uses_compact_bar_data_labels(model) {
        if model.display.show_data_label_outside_bar {
            if let Some(line) = vertical_data_label_line(model, plot_prefix_width, plot_area) {
                out.push(line);
            }
        } else {
            apply_vertical_bar_data_labels(&mut plot, model, y_range, plot_area);
        }
    }

    for (idx, row) in plot.rows.into_iter().enumerate() {
        let label = &tick_labels[idx];
        let mut line = ChartLine::new();
        push_axis_prefix(&mut line, label, gutter, show_y_labels, y_axis_mark);
        line.push_cells(&row);
        out.push(line);
    }

    let baseline_mark = if model.display.x_axis.show_axis_line || model.display.x_axis.show_tick {
        Some(chars.origin)
    } else {
        y_axis_mark
    };
    if show_y_labels || baseline_mark.is_some() {
        let mut axis_line = ChartLine::new();
        push_axis_baseline_prefix(
            &mut axis_line,
            min_label.as_deref().unwrap_or_default(),
            gutter,
            show_y_labels,
            baseline_mark,
        );
        if model.display.x_axis.show_axis_line {
            axis_line.push_role_repeat(
                chars.horizontal_axis,
                plot.width,
                AsciiColorRole::ChartAxis,
            );
        } else if model.display.x_axis.show_tick {
            axis_line.push_spaces(plot.width);
        }
        if model.display.x_axis.show_tick {
            overlay_axis_ticks(
                &mut axis_line,
                plot_prefix_width,
                vertical_category_tick_positions(categories.len(), plot_area),
                chars.horizontal_tick,
            );
        }
        out.push(axis_line);
    }

    if axis_labels_visible(model.display.x_axis) {
        let mut category_line = ChartLine::new();
        category_line.push_spaces(plot_prefix_width);
        category_line.push_role_text_with_unstyled_trailing_spaces(
            &plot_area.category_axis_labels(categories),
            AsciiColorRole::Text,
        );
        out.push(category_line);
    }

    if model.display.x_axis.show_title
        && let Some(title) = x_axis_title(model)
    {
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
    if model.display.show_data_label && !uses_compact_bar_data_labels(model) {
        push_value_disclosure_lines(&mut out, model, categories, chars);
    }
    let plot_rows = build_horizontal_plot_rows(model, categories.len(), y_range, chars, plot_area);

    let show_x_labels = axis_labels_visible(model.display.x_axis);
    let gutter = if show_x_labels {
        categories
            .iter()
            .map(|c| display_width(c))
            .max()
            .unwrap_or(0)
    } else {
        0
    };
    let x_axis_mark = vertical_axis_mark(model.display.x_axis, chars);
    let plot_prefix_width = plot_prefix_width(show_x_labels, x_axis_mark.is_some(), gutter);

    for (idx, category) in categories.iter().enumerate() {
        let plot_row = &plot_rows[idx];

        let mut line = ChartLine::new();
        push_axis_prefix(&mut line, category, gutter, show_x_labels, x_axis_mark);
        line.push_cells(&plot_row.cells);
        if model.display.show_data_label
            && uses_compact_bar_data_labels(model)
            && let (Some(value), Some(label)) = (plot_row.bar_value, plot_row.bar_label.as_deref())
            && (model.display.show_data_label_outside_bar
                || !write_horizontal_inside_data_label(
                    &mut line,
                    plot_prefix_width,
                    label,
                    value,
                    y_range,
                    plot_area,
                ))
        {
            push_horizontal_outside_data_label(&mut line, label);
        }
        out.push(line);
    }

    let baseline_mark = if model.display.y_axis.show_axis_line || model.display.y_axis.show_tick {
        Some(chars.origin)
    } else {
        x_axis_mark
    };
    if axis_labels_visible(model.display.y_axis) || baseline_mark.is_some() {
        let mut axis_line = ChartLine::new();
        push_axis_baseline_prefix(&mut axis_line, "", gutter, show_x_labels, baseline_mark);
        if model.display.y_axis.show_axis_line {
            axis_line.push_role_repeat(
                chars.horizontal_axis,
                plot_area.horizontal_width,
                AsciiColorRole::ChartAxis,
            );
        } else if model.display.y_axis.show_tick {
            axis_line.push_spaces(plot_area.horizontal_width);
        }
        if model.display.y_axis.show_tick {
            overlay_axis_ticks(
                &mut axis_line,
                plot_prefix_width,
                horizontal_value_tick_positions(plot_area),
                chars.horizontal_tick,
            );
        }
        out.push(axis_line);
    }

    if axis_labels_visible(model.display.y_axis) {
        let mut tick_line = ChartLine::new();
        tick_line.push_spaces(plot_prefix_width);
        tick_line.push_line(&horizontal_tick_label_line(y_range, plot_area));
        out.push(tick_line);
    }

    if model.display.x_axis.show_title
        && let Some(title) = x_axis_title(model)
    {
        let mut line = ChartLine::new();
        line.push_role_text("x: ", AsciiColorRole::Text);
        line.push_role_text(title, AsciiColorRole::Text);
        out.push(line);
    }

    finish_chart_lines(out, options)
}

fn push_title_lines(out: &mut Vec<ChartLine>, model: &XyChartDiagramRenderModel) {
    if model.display.show_title
        && let Some(title) = model.title.as_deref().filter(|t| !t.trim().is_empty())
    {
        out.push(ChartLine::role_text(title, AsciiColorRole::Text));
    }

    if model.display.y_axis.show_title
        && let Some(title) = y_axis_title(model)
    {
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
    let mut label_state = SeriesLabelState::default();

    for (series_index, plot) in plots.iter().enumerate() {
        if series_index > 0 {
            line.push_spaces(2);
        }

        line.push_role_char(
            chars.legend_symbol(plot.plot_type),
            AsciiColorRole::ChartSeries(series_index),
        );
        line.push_plain_char(' ');
        line.push_role_text(&series_label(plot, &mut label_state), AsciiColorRole::Text);
    }

    line
}

#[derive(Debug, Default)]
struct SeriesLabelState {
    bar_index: usize,
    line_index: usize,
}

fn series_label(plot: &XyChartPlotRenderModel, state: &mut SeriesLabelState) -> String {
    let default_label = match plot.plot_type {
        XyChartPlotType::Bar => {
            state.bar_index += 1;
            format!("Bar {}", state.bar_index)
        }
        XyChartPlotType::Line => {
            state.line_index += 1;
            format!("Line {}", state.line_index)
        }
    };

    plot.title
        .as_deref()
        .and_then(non_empty)
        .unwrap_or(&default_label)
        .to_string()
}

fn axis_labels_visible(axis: XyChartAxisDisplayPolicy) -> bool {
    axis.show_label
}

fn plot_prefix_width(show_axis_labels: bool, show_axis_mark: bool, gutter: usize) -> usize {
    if show_axis_labels {
        gutter + 1 + usize::from(show_axis_mark)
    } else if show_axis_mark {
        1
    } else {
        0
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
) {
    if show_axis_labels {
        line.push_right_aligned_role_text(label, gutter, AsciiColorRole::Text);
        line.push_plain_char(' ');
    }

    match axis_mark {
        Some(axis_mark) => line.push_role_char(axis_mark, AsciiColorRole::ChartAxis),
        None if show_axis_labels => line.push_plain_char(' '),
        None => {}
    }
}

fn push_axis_baseline_prefix(
    line: &mut ChartLine,
    label: &str,
    gutter: usize,
    show_axis_labels: bool,
    origin: Option<char>,
) {
    if show_axis_labels {
        line.push_right_aligned_role_text(label, gutter, AsciiColorRole::Text);
        line.push_plain_char(' ');
    }

    if let Some(origin) = origin {
        line.push_role_char(origin, AsciiColorRole::ChartAxis);
    }
}

fn overlay_axis_ticks(
    line: &mut ChartLine,
    plot_start: usize,
    tick_positions: impl IntoIterator<Item = usize>,
    tick: char,
) {
    for position in tick_positions {
        line.set_role(plot_start + position, tick, AsciiColorRole::ChartAxis);
    }
}

fn vertical_category_tick_positions(
    category_count: usize,
    plot_area: XyChartPlotArea,
) -> impl Iterator<Item = usize> {
    (0..category_count)
        .map(move |idx| plot_area.vertical_band_start(idx) + (plot_area.category_band_width / 2))
}

fn horizontal_value_tick_positions(plot_area: XyChartPlotArea) -> impl Iterator<Item = usize> {
    [0, plot_area.horizontal_width.saturating_sub(1)].into_iter()
}

fn vertical_data_label_line(
    model: &XyChartDiagramRenderModel,
    plot_prefix_width: usize,
    plot_area: XyChartPlotArea,
) -> Option<ChartLine> {
    let labels = compact_bar_value_labels(model)?;
    if labels.is_empty() {
        return None;
    }

    let mut line = ChartLine::new();
    line.push_spaces(plot_prefix_width);
    line.push_role_text_with_unstyled_trailing_spaces(
        &plot_area.band_labels(&labels),
        AsciiColorRole::Text,
    );
    Some(line)
}

fn push_value_disclosure_lines(
    out: &mut Vec<ChartLine>,
    model: &XyChartDiagramRenderModel,
    categories: &[String],
    chars: ChartChars,
) {
    let mut label_state = SeriesLabelState::default();

    for (series_index, plot) in model.plots.iter().enumerate() {
        let label = series_label(plot, &mut label_state);
        let Some(line) = value_disclosure_line(series_index, plot, &label, categories, chars)
        else {
            continue;
        };
        out.push(line);
    }
}

fn value_disclosure_line(
    series_index: usize,
    plot: &XyChartPlotRenderModel,
    label: &str,
    categories: &[String],
    chars: ChartChars,
) -> Option<ChartLine> {
    if plot.values.is_empty() {
        return None;
    }

    let mut line = ChartLine::new();
    line.push_role_text("values: ", AsciiColorRole::Text);
    line.push_role_char(
        chars.legend_symbol(plot.plot_type),
        AsciiColorRole::ChartSeries(series_index),
    );
    line.push_plain_char(' ');
    line.push_role_text(label, AsciiColorRole::Text);
    line.push_role_text(": ", AsciiColorRole::Text);

    for (idx, value) in plot.values.iter().copied().enumerate() {
        if idx > 0 {
            line.push_role_text(", ", AsciiColorRole::Text);
        }
        if let Some(category) = categories.get(idx).map(String::as_str).and_then(non_empty) {
            line.push_role_text(category, AsciiColorRole::Text);
            line.push_plain_char('=');
        }
        line.push_role_text(&format_number(value), AsciiColorRole::Text);
    }

    Some(line)
}

fn write_horizontal_inside_data_label(
    line: &mut ChartLine,
    plot_prefix_width: usize,
    label: &str,
    value: f64,
    y_range: ValueRange,
    plot_area: XyChartPlotArea,
) -> bool {
    let bar_width = horizontal_bar_width(value, y_range, plot_area);
    let label_width = display_width(label);
    if bar_width == 0 || label_width == 0 || label_width > bar_width {
        return false;
    }

    let start = plot_prefix_width + bar_width - label_width;
    line.write_text_role(start, label, AsciiColorRole::Text);
    true
}

fn push_horizontal_outside_data_label(line: &mut ChartLine, label: &str) {
    if label.is_empty() {
        return;
    }

    line.push_plain_char(' ');
    line.push_role_text(label, AsciiColorRole::Text);
}

fn uses_compact_bar_data_labels(model: &XyChartDiagramRenderModel) -> bool {
    model.plots.len() == 1 && model.plots[0].plot_type == XyChartPlotType::Bar
}

fn compact_bar_value_labels(model: &XyChartDiagramRenderModel) -> Option<Vec<String>> {
    uses_compact_bar_data_labels(model).then(|| {
        model.plots[0]
            .values
            .iter()
            .copied()
            .map(format_number)
            .collect()
    })
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

fn horizontal_tick_label_line(y_range: ValueRange, plot_area: XyChartPlotArea) -> ChartLine {
    let min = format_number(y_range.min);
    let max = format_number(y_range.max);
    horizontal_tick_label_line_for_labels(&min, &max, plot_area.horizontal_width)
}

fn horizontal_tick_label_line_for_labels(min: &str, max: &str, width: usize) -> ChartLine {
    let mut line = ChartLine::blank(width);
    write_horizontal_tick_label(&mut line, 0, min, width);

    let fitted_max = truncate_display_width(max, width);
    let max_start = width.saturating_sub(display_width(&fitted_max));
    write_horizontal_tick_label(
        &mut line,
        max_start,
        &fitted_max,
        width.saturating_sub(max_start),
    );

    line
}

fn write_horizontal_tick_label(line: &mut ChartLine, start: usize, label: &str, width: usize) {
    let fitted = truncate_display_width(label, width);
    line.write_text_role(start, &fitted, AsciiColorRole::Text);
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

#[cfg(test)]
mod tests {
    use super::horizontal_tick_label_line_for_labels;

    #[test]
    fn horizontal_tick_label_line_uses_display_cells_for_wide_labels() {
        let line = horizontal_tick_label_line_for_labels("中", "界", 5);

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
        let line = horizontal_tick_label_line_for_labels("中国A", "", 3);

        assert_eq!(line.len(), 3);
        assert_eq!(line.text(), "中 ");
        assert_eq!(line.get(0), Some('中'));
        assert_eq!(line.get(1), None);
        assert_eq!(line.get(2), Some(' '));
    }
}

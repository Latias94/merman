use crate::canvas::Canvas;
use crate::color::{AsciiColorMode, AsciiColorRole};
use crate::terminal::char_display_width;
use crate::text::{StyledCell, StyledLine, display_width};
use crate::{AsciiCharset, AsciiRenderOptions, Result};
use merman_core::diagrams::xychart::{
    XyChartAxisRenderModel, XyChartDiagramRenderModel, XyChartPlotRenderModel, XyChartPlotType,
};

const VERTICAL_PLOT_HEIGHT: usize = 5;
const BAND_WIDTH: usize = 3;
const BAND_GAP: usize = 1;
const BAND_GAP_LABEL: &str = " ";
const HORIZONTAL_PLOT_WIDTH: usize = 10;

#[derive(Debug, Clone, Copy)]
struct ChartChars {
    horizontal_axis: char,
    vertical_axis: char,
    origin: char,
    bar: char,
    line_horizontal: char,
    line_vertical: char,
    line_point: char,
}

impl ChartChars {
    fn from_options(options: &AsciiRenderOptions) -> Self {
        match options.charset {
            AsciiCharset::Ascii => Self {
                horizontal_axis: '-',
                vertical_axis: '|',
                origin: '+',
                bar: '#',
                line_horizontal: '*',
                line_vertical: '*',
                line_point: '*',
            },
            AsciiCharset::Unicode => Self {
                horizontal_axis: '─',
                vertical_axis: '│',
                origin: '┼',
                bar: '█',
                line_horizontal: '─',
                line_vertical: '│',
                line_point: '●',
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ValueRange {
    min: f64,
    max: f64,
}

impl ValueRange {
    fn span(self) -> f64 {
        self.max - self.min
    }

    fn normalized(self, value: f64) -> f64 {
        if self.span().abs() <= f64::EPSILON {
            return 0.0;
        }

        ((value - self.min) / self.span()).clamp(0.0, 1.0)
    }
}

type ChartCell = StyledCell;
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
    if model.orientation.eq_ignore_ascii_case("horizontal") {
        return Ok(render_horizontal(
            model,
            &categories,
            y_range,
            chars,
            options,
        ));
    }

    Ok(render_vertical(model, &categories, y_range, chars, options))
}

fn render_vertical(
    model: &XyChartDiagramRenderModel,
    categories: &[String],
    y_range: ValueRange,
    chars: ChartChars,
    options: &AsciiRenderOptions,
) -> String {
    let plot_width = vertical_plot_width(categories.len());
    let mut rows = vec![vec![ChartCell::blank(); plot_width]; VERTICAL_PLOT_HEIGHT];

    for (series_index, plot) in model.plots.iter().enumerate() {
        if plot.plot_type == XyChartPlotType::Bar {
            draw_vertical_bar_plot(&mut rows, plot, series_index, y_range, chars);
        }
    }

    for (series_index, plot) in model.plots.iter().enumerate() {
        if plot.plot_type == XyChartPlotType::Line {
            draw_vertical_line_plot(&mut rows, plot, series_index, y_range, chars);
        }
    }

    let mut out = Vec::new();
    push_title_lines(&mut out, model);

    let tick_labels = vertical_tick_labels(y_range);
    let min_label = format_number(y_range.min);
    let gutter = tick_labels
        .iter()
        .chain(std::iter::once(&min_label))
        .map(|s| display_width(s))
        .max()
        .unwrap_or(1);

    for (idx, row) in rows.into_iter().enumerate() {
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
    axis_line.push_role_repeat(chars.horizontal_axis, plot_width, AsciiColorRole::ChartAxis);
    out.push(axis_line);

    let mut category_line = ChartLine::new();
    category_line.push_spaces(gutter + 2);
    category_line.push_role_text_with_unstyled_trailing_spaces(
        &category_axis_labels(categories),
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
    options: &AsciiRenderOptions,
) -> String {
    let mut out = Vec::new();
    push_title_lines(&mut out, model);

    let gutter = categories
        .iter()
        .map(|c| display_width(c))
        .max()
        .unwrap_or(1);

    for (idx, category) in categories.iter().enumerate() {
        let mut row = vec![ChartCell::blank(); HORIZONTAL_PLOT_WIDTH];
        let mut values = Vec::new();

        for (series_index, plot) in model.plots.iter().enumerate() {
            let Some(value) = plot.values.get(idx).copied() else {
                continue;
            };
            values.push(format_number(value));

            match plot.plot_type {
                XyChartPlotType::Bar => {
                    draw_horizontal_bar_value(&mut row, value, series_index, y_range, chars)
                }
                XyChartPlotType::Line => {
                    draw_horizontal_line_value(&mut row, value, series_index, y_range, chars)
                }
            }
        }

        let mut line = ChartLine::new();
        line.push_right_aligned_role_text(category, gutter, AsciiColorRole::Text);
        line.push_plain_char(' ');
        line.push_role_char(chars.vertical_axis, AsciiColorRole::ChartAxis);
        line.push_cells(&row);
        if !values.is_empty() {
            line.push_plain_char(' ');
            line.push_role_text(&values.join("/"), AsciiColorRole::Text);
        }
        out.push(line);
    }

    let mut axis_line = ChartLine::new();
    axis_line.push_spaces(gutter + 1);
    axis_line.push_role_char(chars.origin, AsciiColorRole::ChartAxis);
    axis_line.push_role_repeat(
        chars.horizontal_axis,
        HORIZONTAL_PLOT_WIDTH,
        AsciiColorRole::ChartAxis,
    );
    out.push(axis_line);

    let mut tick_line = ChartLine::new();
    tick_line.push_spaces(gutter + 2);
    tick_line.push_role_text(&horizontal_tick_labels(y_range), AsciiColorRole::Text);
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

fn draw_vertical_bar_plot(
    rows: &mut [Vec<ChartCell>],
    plot: &XyChartPlotRenderModel,
    series_index: usize,
    y_range: ValueRange,
    chars: ChartChars,
) {
    let role = AsciiColorRole::ChartSeries(series_index);
    for (idx, value) in plot.values.iter().copied().enumerate() {
        let height = bar_height(value, y_range, VERTICAL_PLOT_HEIGHT);
        if height == 0 {
            continue;
        }

        let band_start = vertical_band_start(idx);
        for level in 1..=height {
            let row_idx = VERTICAL_PLOT_HEIGHT - level;
            if let Some(row) = rows.get_mut(row_idx) {
                fill_band(row, band_start, chars.bar, role);
            }
        }
    }
}

fn draw_vertical_line_plot(
    rows: &mut [Vec<ChartCell>],
    plot: &XyChartPlotRenderModel,
    series_index: usize,
    y_range: ValueRange,
    chars: ChartChars,
) {
    let role = AsciiColorRole::ChartSeries(series_index);
    let points = plot
        .values
        .iter()
        .copied()
        .enumerate()
        .map(|(idx, value)| {
            let level = line_level(value, y_range, VERTICAL_PLOT_HEIGHT);
            let row = VERTICAL_PLOT_HEIGHT - level;
            let col = vertical_band_start(idx) + (BAND_WIDTH / 2);
            (row, col)
        })
        .collect::<Vec<_>>();

    for pair in points.windows(2) {
        let (from_row, from_col) = pair[0];
        let (to_row, to_col) = pair[1];
        draw_vertical_line_segment(rows, from_row, from_col, to_row, to_col, chars, role);
    }

    for (row, col) in points {
        set_cell(rows, row, col, chars.line_point, role);
    }
}

fn draw_vertical_line_segment(
    rows: &mut [Vec<ChartCell>],
    from_row: usize,
    from_col: usize,
    to_row: usize,
    to_col: usize,
    chars: ChartChars,
    role: AsciiColorRole,
) {
    if from_col == to_col {
        draw_column(rows, from_col, from_row, to_row, chars.line_vertical, role);
        return;
    }

    if from_row == to_row {
        draw_row(
            rows,
            from_row,
            from_col,
            to_col,
            chars.line_horizontal,
            role,
        );
        return;
    }

    let mid_col = (from_col + to_col) / 2;
    draw_row(
        rows,
        from_row,
        from_col,
        mid_col,
        chars.line_horizontal,
        role,
    );
    draw_column(rows, mid_col, from_row, to_row, chars.line_vertical, role);
    draw_row(rows, to_row, mid_col, to_col, chars.line_horizontal, role);
}

fn draw_horizontal_bar_value(
    row: &mut [ChartCell],
    value: f64,
    series_index: usize,
    y_range: ValueRange,
    chars: ChartChars,
) {
    let role = AsciiColorRole::ChartSeries(series_index);
    let width = bar_height(value, y_range, HORIZONTAL_PLOT_WIDTH);
    for cell in row.iter_mut().take(width) {
        *cell = ChartCell::with_role(chars.bar, role);
    }
}

fn draw_horizontal_line_value(
    row: &mut [ChartCell],
    value: f64,
    series_index: usize,
    y_range: ValueRange,
    chars: ChartChars,
) {
    let role = AsciiColorRole::ChartSeries(series_index);
    let col = line_level(value, y_range, HORIZONTAL_PLOT_WIDTH).saturating_sub(1);
    if let Some(cell) = row.get_mut(col) {
        *cell = ChartCell::with_role(chars.line_point, role);
    }
}

fn draw_row(
    rows: &mut [Vec<ChartCell>],
    row_idx: usize,
    from_col: usize,
    to_col: usize,
    value: char,
    role: AsciiColorRole,
) {
    let start = from_col.min(to_col);
    let end = from_col.max(to_col);
    if let Some(row) = rows.get_mut(row_idx) {
        for col in start..=end {
            if let Some(cell) = row.get_mut(col) {
                *cell = ChartCell::with_role(value, role);
            }
        }
    }
}

fn draw_column(
    rows: &mut [Vec<ChartCell>],
    col: usize,
    from_row: usize,
    to_row: usize,
    value: char,
    role: AsciiColorRole,
) {
    let start = from_row.min(to_row);
    let end = from_row.max(to_row);
    for row_idx in start..=end {
        set_cell(rows, row_idx, col, value, role);
    }
}

fn set_cell(
    rows: &mut [Vec<ChartCell>],
    row: usize,
    col: usize,
    value: char,
    role: AsciiColorRole,
) {
    if let Some(cell) = rows.get_mut(row).and_then(|r| r.get_mut(col)) {
        *cell = ChartCell::with_role(value, role);
    }
}

fn fill_band(row: &mut [ChartCell], band_start: usize, value: char, role: AsciiColorRole) {
    for offset in 0..BAND_WIDTH {
        if let Some(cell) = row.get_mut(band_start + offset) {
            *cell = ChartCell::with_role(value, role);
        }
    }
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

fn vertical_tick_labels(y_range: ValueRange) -> Vec<String> {
    (1..=VERTICAL_PLOT_HEIGHT)
        .rev()
        .map(|level| {
            let value =
                y_range.min + (y_range.span() * (level as f64) / VERTICAL_PLOT_HEIGHT as f64);
            format_number(value)
        })
        .collect()
}

fn horizontal_tick_labels(y_range: ValueRange) -> String {
    let min = format_number(y_range.min);
    let max = format_number(y_range.max);
    let mut cells = vec![' '; HORIZONTAL_PLOT_WIDTH];

    for (idx, ch) in min.chars().take(HORIZONTAL_PLOT_WIDTH).enumerate() {
        cells[idx] = ch;
    }

    let max_len = display_width(&max);
    let max_start = HORIZONTAL_PLOT_WIDTH.saturating_sub(max_len);
    for (idx, ch) in max.chars().enumerate() {
        if let Some(cell) = cells.get_mut(max_start + idx) {
            *cell = ch;
        }
    }

    chars_to_string(&cells)
}

fn category_axis_labels(categories: &[String]) -> String {
    categories
        .iter()
        .map(|category| fit_centered(category, BAND_WIDTH))
        .collect::<Vec<_>>()
        .join(BAND_GAP_LABEL)
}

fn fit_centered(value: &str, width: usize) -> String {
    let value = truncate_display_width(value, width);
    let value_width = display_width(&value);
    let left = (width - value_width) / 2;
    let right = width - value_width - left;
    format!("{}{}{}", " ".repeat(left), value, " ".repeat(right))
}

fn truncate_display_width(value: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;

    for ch in value.chars() {
        let ch_width = char_display_width(ch);
        if used + ch_width > width {
            break;
        }
        out.push(ch);
        used += ch_width;
    }

    out
}

fn bar_height(value: f64, range: ValueRange, height: usize) -> usize {
    (range.normalized(value) * height as f64).round() as usize
}

fn line_level(value: f64, range: ValueRange, height: usize) -> usize {
    bar_height(value, range, height).clamp(1, height)
}

fn vertical_plot_width(category_count: usize) -> usize {
    if category_count == 0 {
        0
    } else {
        (category_count * BAND_WIDTH) + ((category_count - 1) * BAND_GAP)
    }
}

fn vertical_band_start(idx: usize) -> usize {
    idx * (BAND_WIDTH + BAND_GAP)
}

fn chars_to_string(chars: &[char]) -> String {
    chars.iter().collect::<String>()
}

fn format_number(value: f64) -> String {
    let rounded = value.round();
    if (value - rounded).abs() <= 1e-9 {
        return format!("{rounded:.0}");
    }

    let mut out = format!("{value:.1}");
    while out.contains('.') && out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.pop();
    }
    out
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

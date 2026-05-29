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
        return Ok(render_horizontal(model, &categories, y_range, chars));
    }

    Ok(render_vertical(model, &categories, y_range, chars))
}

fn render_vertical(
    model: &XyChartDiagramRenderModel,
    categories: &[String],
    y_range: ValueRange,
    chars: ChartChars,
) -> String {
    let plot_width = vertical_plot_width(categories.len());
    let mut rows = vec![vec![' '; plot_width]; VERTICAL_PLOT_HEIGHT];

    for plot in &model.plots {
        if plot.plot_type == XyChartPlotType::Bar {
            draw_vertical_bar_plot(&mut rows, plot, y_range, chars);
        }
    }

    for plot in &model.plots {
        if plot.plot_type == XyChartPlotType::Line {
            draw_vertical_line_plot(&mut rows, plot, y_range, chars);
        }
    }

    let mut out = Vec::new();
    push_title_lines(&mut out, model);

    let tick_labels = vertical_tick_labels(y_range);
    let min_label = format_number(y_range.min);
    let gutter = tick_labels
        .iter()
        .chain(std::iter::once(&min_label))
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(1);

    for (idx, row) in rows.into_iter().enumerate() {
        let label = &tick_labels[idx];
        out.push(format!(
            "{label:>gutter$} {}{}",
            chars.vertical_axis,
            chars_to_string(&row)
        ));
    }

    out.push(format!(
        "{min_label:>gutter$} {}{}",
        chars.origin,
        chars.horizontal_axis.to_string().repeat(plot_width)
    ));
    out.push(format!(
        "{}{}",
        " ".repeat(gutter + 2),
        category_axis_labels(categories)
    ));

    if let Some(title) = x_axis_title(model) {
        out.push(format!("x: {title}"));
    }

    finish_lines(out)
}

fn render_horizontal(
    model: &XyChartDiagramRenderModel,
    categories: &[String],
    y_range: ValueRange,
    chars: ChartChars,
) -> String {
    let mut out = Vec::new();
    push_title_lines(&mut out, model);

    let gutter = categories
        .iter()
        .map(|c| c.chars().count())
        .max()
        .unwrap_or(1);

    for (idx, category) in categories.iter().enumerate() {
        let mut row = vec![' '; HORIZONTAL_PLOT_WIDTH];
        let mut values = Vec::new();

        for plot in &model.plots {
            let Some(value) = plot.values.get(idx).copied() else {
                continue;
            };
            values.push(format_number(value));

            match plot.plot_type {
                XyChartPlotType::Bar => draw_horizontal_bar_value(&mut row, value, y_range, chars),
                XyChartPlotType::Line => {
                    draw_horizontal_line_value(&mut row, value, y_range, chars)
                }
            }
        }

        let value_suffix = if values.is_empty() {
            String::new()
        } else {
            format!(" {}", values.join("/"))
        };
        out.push(format!(
            "{category:>gutter$} {}{}{}",
            chars.vertical_axis,
            chars_to_string(&row),
            value_suffix
        ));
    }

    out.push(format!(
        "{}{}{}",
        " ".repeat(gutter + 1),
        chars.origin,
        chars
            .horizontal_axis
            .to_string()
            .repeat(HORIZONTAL_PLOT_WIDTH)
    ));
    out.push(format!(
        "{}{}",
        " ".repeat(gutter + 2),
        horizontal_tick_labels(y_range)
    ));

    finish_lines(out)
}

fn push_title_lines(out: &mut Vec<String>, model: &XyChartDiagramRenderModel) {
    if let Some(title) = model.title.as_deref().filter(|t| !t.trim().is_empty()) {
        out.push(title.to_string());
    }

    if let Some(title) = y_axis_title(model) {
        out.push(format!("y: {title}"));
    }
}

fn draw_vertical_bar_plot(
    rows: &mut [Vec<char>],
    plot: &XyChartPlotRenderModel,
    y_range: ValueRange,
    chars: ChartChars,
) {
    for (idx, value) in plot.values.iter().copied().enumerate() {
        let height = bar_height(value, y_range, VERTICAL_PLOT_HEIGHT);
        if height == 0 {
            continue;
        }

        let band_start = vertical_band_start(idx);
        for level in 1..=height {
            let row_idx = VERTICAL_PLOT_HEIGHT - level;
            if let Some(row) = rows.get_mut(row_idx) {
                fill_band(row, band_start, chars.bar);
            }
        }
    }
}

fn draw_vertical_line_plot(
    rows: &mut [Vec<char>],
    plot: &XyChartPlotRenderModel,
    y_range: ValueRange,
    chars: ChartChars,
) {
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
        draw_vertical_line_segment(rows, from_row, from_col, to_row, to_col, chars);
    }

    for (row, col) in points {
        set_cell(rows, row, col, chars.line_point);
    }
}

fn draw_vertical_line_segment(
    rows: &mut [Vec<char>],
    from_row: usize,
    from_col: usize,
    to_row: usize,
    to_col: usize,
    chars: ChartChars,
) {
    if from_col == to_col {
        draw_column(rows, from_col, from_row, to_row, chars.line_vertical);
        return;
    }

    if from_row == to_row {
        draw_row(rows, from_row, from_col, to_col, chars.line_horizontal);
        return;
    }

    let mid_col = (from_col + to_col) / 2;
    draw_row(rows, from_row, from_col, mid_col, chars.line_horizontal);
    draw_column(rows, mid_col, from_row, to_row, chars.line_vertical);
    draw_row(rows, to_row, mid_col, to_col, chars.line_horizontal);
}

fn draw_horizontal_bar_value(row: &mut [char], value: f64, y_range: ValueRange, chars: ChartChars) {
    let width = bar_height(value, y_range, HORIZONTAL_PLOT_WIDTH);
    for cell in row.iter_mut().take(width) {
        *cell = chars.bar;
    }
}

fn draw_horizontal_line_value(
    row: &mut [char],
    value: f64,
    y_range: ValueRange,
    chars: ChartChars,
) {
    let col = line_level(value, y_range, HORIZONTAL_PLOT_WIDTH).saturating_sub(1);
    if let Some(cell) = row.get_mut(col) {
        *cell = chars.line_point;
    }
}

fn draw_row(rows: &mut [Vec<char>], row_idx: usize, from_col: usize, to_col: usize, value: char) {
    let start = from_col.min(to_col);
    let end = from_col.max(to_col);
    if let Some(row) = rows.get_mut(row_idx) {
        for col in start..=end {
            if let Some(cell) = row.get_mut(col) {
                *cell = value;
            }
        }
    }
}

fn draw_column(rows: &mut [Vec<char>], col: usize, from_row: usize, to_row: usize, value: char) {
    let start = from_row.min(to_row);
    let end = from_row.max(to_row);
    for row_idx in start..=end {
        set_cell(rows, row_idx, col, value);
    }
}

fn set_cell(rows: &mut [Vec<char>], row: usize, col: usize, value: char) {
    if let Some(cell) = rows.get_mut(row).and_then(|r| r.get_mut(col)) {
        *cell = value;
    }
}

fn fill_band(row: &mut [char], band_start: usize, value: char) {
    for offset in 0..BAND_WIDTH {
        if let Some(cell) = row.get_mut(band_start + offset) {
            *cell = value;
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

    let max_len = max.chars().count();
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
    let mut chars = value.chars().collect::<Vec<_>>();
    if chars.len() > width {
        chars.truncate(width);
        return chars.into_iter().collect();
    }

    let left = (width - chars.len()) / 2;
    let right = width - chars.len() - left;
    format!(
        "{}{}{}",
        " ".repeat(left),
        chars.into_iter().collect::<String>(),
        " ".repeat(right)
    )
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

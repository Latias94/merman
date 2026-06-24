use crate::color::AsciiColorRole;
use crate::terminal::char_display_width;
use crate::text::{StyledCell, display_width};
use crate::{AsciiCharset, AsciiRenderOptions};
use merman_core::diagrams::xychart::{
    XyChartDiagramRenderModel, XyChartPlotRenderModel, XyChartPlotType,
};

pub(super) const VERTICAL_PLOT_HEIGHT: usize = 5;
const BAND_WIDTH: usize = 3;
const BAND_GAP: usize = 1;
const BAND_GAP_LABEL: &str = " ";
pub(super) const HORIZONTAL_PLOT_WIDTH: usize = 10;

pub(super) type ChartCell = StyledCell;

#[derive(Debug, Clone, Copy)]
pub(super) struct ChartChars {
    pub(super) horizontal_axis: char,
    pub(super) vertical_axis: char,
    pub(super) origin: char,
    bar: char,
    line_horizontal: char,
    line_vertical: char,
    line_point: char,
}

impl ChartChars {
    pub(super) fn from_options(options: &AsciiRenderOptions) -> Self {
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

    pub(super) fn legend_symbol(self, plot_type: XyChartPlotType) -> char {
        match plot_type {
            XyChartPlotType::Bar => self.bar,
            XyChartPlotType::Line => self.line_point,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ValueRange {
    pub(super) min: f64,
    pub(super) max: f64,
}

impl ValueRange {
    pub(super) fn span(self) -> f64 {
        self.max - self.min
    }

    fn normalized(self, value: f64) -> f64 {
        if self.span().abs() <= f64::EPSILON {
            return 0.0;
        }

        ((value - self.min) / self.span()).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone)]
pub(super) struct VerticalPlot {
    pub(super) rows: Vec<Vec<ChartCell>>,
    pub(super) width: usize,
}

#[derive(Debug, Clone)]
pub(super) struct HorizontalPlotRow {
    pub(super) cells: Vec<ChartCell>,
    pub(super) values: Vec<String>,
}

pub(super) fn build_vertical_plot(
    model: &XyChartDiagramRenderModel,
    category_count: usize,
    y_range: ValueRange,
    chars: ChartChars,
) -> VerticalPlot {
    let width = vertical_plot_width(category_count);
    let mut rows = vec![vec![ChartCell::blank(); width]; VERTICAL_PLOT_HEIGHT];

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

    VerticalPlot { rows, width }
}

pub(super) fn build_horizontal_plot_rows(
    model: &XyChartDiagramRenderModel,
    category_count: usize,
    y_range: ValueRange,
    chars: ChartChars,
) -> Vec<HorizontalPlotRow> {
    (0..category_count)
        .map(|idx| {
            let mut cells = vec![ChartCell::blank(); HORIZONTAL_PLOT_WIDTH];
            let mut values = Vec::new();

            for (series_index, plot) in model.plots.iter().enumerate() {
                let Some(value) = plot.values.get(idx).copied() else {
                    continue;
                };
                values.push(format_number(value));

                match plot.plot_type {
                    XyChartPlotType::Bar => {
                        draw_horizontal_bar_value(&mut cells, value, series_index, y_range, chars)
                    }
                    XyChartPlotType::Line => {
                        draw_horizontal_line_value(&mut cells, value, series_index, y_range, chars)
                    }
                }
            }

            HorizontalPlotRow { cells, values }
        })
        .collect()
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

pub(super) fn category_axis_labels(categories: &[String]) -> String {
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

pub(super) fn format_number(value: f64) -> String {
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

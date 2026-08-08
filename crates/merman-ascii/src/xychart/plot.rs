use crate::color::AsciiColorRole;
use crate::options::TerminalWidthProfile;
use crate::resource::{
    AsciiResourceLimitId, AsciiResourceLimitPhase, AsciiResourcePolicy, LogicalExtent,
    ResourceContext,
};
use crate::text::{StyledLine, display_width_with_profile, truncate_display_width_with_profile};
use crate::{AsciiCharset, AsciiError, AsciiRenderOptions, Result};
use merman_core::diagrams::xychart::{
    XyChartDiagramRenderModel, XyChartPlotRenderModel, XyChartPlotType,
};

const BAND_GAP: usize = 1;
const BAND_GAP_LABEL: &str = " ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct XyChartPlotArea {
    pub(super) vertical_height: usize,
    pub(super) category_band_width: usize,
    pub(super) horizontal_width: usize,
    pub(super) width_profile: TerminalWidthProfile,
    pub(super) resources: AsciiResourcePolicy,
}

impl XyChartPlotArea {
    pub(super) fn from_options(options: &AsciiRenderOptions) -> Self {
        Self {
            vertical_height: options.xychart_vertical_plot_height,
            category_band_width: options.xychart_category_band_width,
            horizontal_width: options.xychart_horizontal_plot_width,
            width_profile: options.terminal_width_profile,
            resources: options.resources,
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
    ) -> Result<String> {
        let width = self.vertical_plot_width(labels.len(), resources)?;
        let mut rendered = String::new();
        rendered
            .try_reserve(width)
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
            })?;
        for (index, label) in labels.iter().enumerate() {
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
    ) -> Result<String> {
        self.band_labels(categories, resources)
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
                line_horizontal: '*',
                line_vertical: '*',
                line_point: '*',
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
    pub(super) rows: Vec<StyledLine>,
    pub(super) width: usize,
}

#[derive(Debug, Clone)]
pub(super) struct HorizontalPlotRow {
    pub(super) line: StyledLine,
    pub(super) bar_value: Option<f64>,
    pub(super) bar_label: Option<String>,
}

pub(super) fn build_vertical_plot(
    model: &XyChartDiagramRenderModel,
    y_range: ValueRange,
    chars: ChartChars,
    plot_area: XyChartPlotArea,
    plot_extent: LogicalExtent,
    resources: &mut ResourceContext,
) -> Result<VerticalPlot> {
    debug_assert_eq!(plot_extent.height(), plot_area.vertical_height);
    let width = plot_extent.width();
    let mut rows = Vec::new();
    rows.try_reserve_exact(plot_extent.height())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::Layout.as_str(),
        })?;
    for _ in 0..plot_extent.height() {
        rows.push(try_plot_row(width, plot_area, resources)?);
    }

    for (series_index, plot) in model.plots.iter().enumerate() {
        resources.charge_layout_work(1)?;
        if plot.plot_type == XyChartPlotType::Bar {
            draw_vertical_bar_plot(
                &mut rows,
                plot,
                series_index,
                y_range,
                chars,
                plot_area,
                resources,
            )?;
        }
    }

    for (series_index, plot) in model.plots.iter().enumerate() {
        resources.charge_layout_work(1)?;
        if plot.plot_type == XyChartPlotType::Line {
            draw_vertical_line_plot(
                &mut rows,
                plot,
                series_index,
                y_range,
                chars,
                plot_area,
                resources,
            )?;
        }
    }

    Ok(VerticalPlot { rows, width })
}

pub(super) fn build_horizontal_plot_rows(
    model: &XyChartDiagramRenderModel,
    y_range: ValueRange,
    chars: ChartChars,
    plot_area: XyChartPlotArea,
    plot_extent: LogicalExtent,
    resources: &mut ResourceContext,
) -> Result<Vec<HorizontalPlotRow>> {
    let category_count = plot_extent.height();
    debug_assert_eq!(plot_extent.width(), plot_area.horizontal_width);
    let mut first_bar_values = None;
    for plot in &model.plots {
        resources.charge_layout_work(1)?;
        if plot.plot_type == XyChartPlotType::Bar {
            first_bar_values = Some(plot.values.as_slice());
            break;
        }
    }

    let mut rows = Vec::new();
    rows.try_reserve_exact(category_count)
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::Layout.as_str(),
        })?;
    for idx in 0..category_count {
        resources.charge_layout_work(1)?;
        let mut line = try_plot_row(plot_area.horizontal_width, plot_area, resources)?;

        for (series_index, plot) in model.plots.iter().enumerate() {
            resources.charge_layout_work(1)?;
            let Some(value) = plot.values.get(idx).copied() else {
                continue;
            };

            match plot.plot_type {
                XyChartPlotType::Bar => draw_horizontal_bar_value(
                    &mut line,
                    value,
                    series_index,
                    y_range,
                    chars,
                    plot_area,
                    resources,
                )?,
                XyChartPlotType::Line => draw_horizontal_line_value(
                    &mut line,
                    value,
                    series_index,
                    y_range,
                    chars,
                    plot_area,
                    resources,
                )?,
            }
        }

        let bar_value = first_bar_values.and_then(|values| values.get(idx).copied());
        let bar_label = bar_value.map(format_number);
        rows.push(HorizontalPlotRow {
            line,
            bar_value,
            bar_label,
        });
    }
    Ok(rows)
}

pub(super) fn apply_vertical_bar_data_labels(
    plot: &mut VerticalPlot,
    model: &XyChartDiagramRenderModel,
    y_range: ValueRange,
    plot_area: XyChartPlotArea,
    resources: &mut ResourceContext,
) -> Result<()> {
    let mut bar_plot = None;
    for plot in &model.plots {
        resources.charge_layout_work(1)?;
        if plot.plot_type == XyChartPlotType::Bar {
            bar_plot = Some(plot);
            break;
        }
    }
    let Some(bar_plot) = bar_plot else {
        return Ok(());
    };

    for (idx, value) in bar_plot.values.iter().copied().enumerate() {
        resources.charge_layout_work(1)?;
        let height = bar_height(value, y_range, plot_area.vertical_height);
        if height == 0 {
            continue;
        }

        let row_idx = checked_grid_sub(resources, plot_area.vertical_height, height)?;
        let band_start = plot_area.vertical_band_start(idx, resources)?;
        if let Some(row) = plot.rows.get_mut(row_idx) {
            resources.charge_layout_work(plot_area.category_band_width)?;
            write_band_text(
                row,
                band_start,
                plot_area.category_band_width,
                &format_number(value),
                AsciiColorRole::Text,
                resources,
            )?;
        }
    }
    Ok(())
}

fn draw_vertical_bar_plot(
    rows: &mut [StyledLine],
    plot: &XyChartPlotRenderModel,
    series_index: usize,
    y_range: ValueRange,
    chars: ChartChars,
    plot_area: XyChartPlotArea,
    resources: &mut ResourceContext,
) -> Result<()> {
    let role = AsciiColorRole::ChartSeries(series_index);
    for (idx, value) in plot.values.iter().copied().enumerate() {
        resources.charge_layout_work(1)?;
        let height = bar_height(value, y_range, plot_area.vertical_height);
        if height == 0 {
            continue;
        }

        let band_start = plot_area.vertical_band_start(idx, resources)?;
        for level in 1..=height {
            resources.charge_layout_work(plot_area.category_band_width)?;
            let row_idx = checked_grid_sub(resources, plot_area.vertical_height, level)?;
            if let Some(row) = rows.get_mut(row_idx) {
                fill_band(
                    row,
                    band_start,
                    plot_area.category_band_width,
                    chars.bar,
                    role,
                    resources,
                )?;
            }
        }
    }
    Ok(())
}

fn draw_vertical_line_plot(
    rows: &mut [StyledLine],
    plot: &XyChartPlotRenderModel,
    series_index: usize,
    y_range: ValueRange,
    chars: ChartChars,
    plot_area: XyChartPlotArea,
    resources: &mut ResourceContext,
) -> Result<()> {
    let role = AsciiColorRole::ChartSeries(series_index);
    let mut points = Vec::new();
    points
        .try_reserve_exact(plot.values.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for (idx, value) in plot.values.iter().copied().enumerate() {
        resources.charge_layout_work(1)?;
        let level = line_level(value, y_range, plot_area.vertical_height);
        let row = checked_grid_sub(resources, plot_area.vertical_height, level)?;
        let col = plot_area.vertical_band_center(idx, resources)?;
        points.push((row, col));
    }

    for pair in points.windows(2) {
        draw_vertical_line_segment(rows, pair[0], pair[1], chars, role, resources)?;
    }

    for (row, col) in points {
        resources.charge_layout_work(1)?;
        set_cell(rows, row, col, chars.line_point, role)?;
    }
    Ok(())
}

fn draw_vertical_line_segment(
    rows: &mut [StyledLine],
    from: (usize, usize),
    to: (usize, usize),
    chars: ChartChars,
    role: AsciiColorRole,
    resources: &mut ResourceContext,
) -> Result<()> {
    let (from_row, from_col) = from;
    let (to_row, to_col) = to;
    if from_col == to_col {
        return draw_column(
            rows,
            from_col,
            from_row,
            to_row,
            chars.line_vertical,
            role,
            resources,
        );
    }

    if from_row == to_row {
        return draw_row(
            rows,
            from_row,
            from_col,
            to_col,
            chars.line_horizontal,
            role,
            resources,
        );
    }

    let mid_col = resources.checked_grid_add(from_col, to_col)? / 2;
    draw_row(
        rows,
        from_row,
        from_col,
        mid_col,
        chars.line_horizontal,
        role,
        resources,
    )?;
    draw_column(
        rows,
        mid_col,
        from_row,
        to_row,
        chars.line_vertical,
        role,
        resources,
    )?;
    draw_row(
        rows,
        to_row,
        mid_col,
        to_col,
        chars.line_horizontal,
        role,
        resources,
    )
}

fn draw_horizontal_bar_value(
    row: &mut StyledLine,
    value: f64,
    series_index: usize,
    y_range: ValueRange,
    chars: ChartChars,
    plot_area: XyChartPlotArea,
    resources: &mut ResourceContext,
) -> Result<()> {
    let role = AsciiColorRole::ChartSeries(series_index);
    let width = bar_height(value, y_range, plot_area.horizontal_width);
    resources.charge_layout_work(width)?;
    for col in 0..width {
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

fn draw_horizontal_line_value(
    row: &mut StyledLine,
    value: f64,
    series_index: usize,
    y_range: ValueRange,
    chars: ChartChars,
    plot_area: XyChartPlotArea,
    resources: &mut ResourceContext,
) -> Result<()> {
    let role = AsciiColorRole::ChartSeries(series_index);
    resources.charge_layout_work(1)?;
    let col = checked_grid_sub(
        resources,
        line_level(value, y_range, plot_area.horizontal_width),
        1,
    )?;
    row.try_set_role(col, chars.line_point, role)
}

fn draw_row(
    rows: &mut [StyledLine],
    row_idx: usize,
    from_col: usize,
    to_col: usize,
    value: char,
    role: AsciiColorRole,
    resources: &mut ResourceContext,
) -> Result<()> {
    let start = from_col.min(to_col);
    let end = from_col.max(to_col);
    let length = resources.checked_grid_add(checked_grid_sub(resources, end, start)?, 1)?;
    resources.charge_layout_work(length)?;
    if let Some(row) = rows.get_mut(row_idx) {
        for col in start..=end {
            row.try_set_role(col, value, role)?;
        }
    }
    Ok(())
}

fn draw_column(
    rows: &mut [StyledLine],
    col: usize,
    from_row: usize,
    to_row: usize,
    value: char,
    role: AsciiColorRole,
    resources: &mut ResourceContext,
) -> Result<()> {
    let start = from_row.min(to_row);
    let end = from_row.max(to_row);
    let length = resources.checked_grid_add(checked_grid_sub(resources, end, start)?, 1)?;
    resources.charge_layout_work(length)?;
    for row_idx in start..=end {
        set_cell(rows, row_idx, col, value, role)?;
    }
    Ok(())
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
) -> Result<()> {
    for offset in 0..band_width {
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
    value: &str,
    width: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<String> {
    let value = truncate_display_width_with_profile(value, width, width_profile);
    let value_width = display_width_with_profile(&value, width_profile);
    let remaining = checked_grid_sub(resources, width, value_width)?;
    let left = remaining / 2;
    let right = checked_grid_sub(resources, remaining, left)?;
    Ok(format!(
        "{}{}{}",
        " ".repeat(left),
        value,
        " ".repeat(right)
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_band_text_preserves_wide_glyph_continuation_cells() {
        let options = AsciiRenderOptions::unicode();
        let resources = ResourceContext::new(options.resources);
        let mut row =
            StyledLine::try_blank_with_policy(5, options.terminal_width_profile, options.resources)
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
        let resources = ResourceContext::new(options.resources);
        let mut row =
            StyledLine::try_blank_with_policy(6, options.terminal_width_profile, options.resources)
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
        let options = AsciiRenderOptions::unicode();
        let resources = ResourceContext::new(options.resources);
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
        let options = AsciiRenderOptions::unicode();
        let resources = ResourceContext::new(options.resources);
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
    fn cjk_profile_uses_single_cell_ascii_plot_structure() {
        let options =
            AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk);
        let chars = ChartChars::from_options(&options);
        let plot_area = XyChartPlotArea::from_options(&options);
        let mut resources = ResourceContext::new(options.resources);
        let mut row = StyledLine::try_blank_with_policy(
            plot_area.horizontal_width,
            TerminalWidthProfile::Cjk,
            options.resources,
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

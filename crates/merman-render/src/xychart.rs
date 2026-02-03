use crate::model::{
    XyChartDiagramLayout, XyChartDrawableElem, XyChartPathData, XyChartRectData, XyChartTextData,
};
use crate::text::{TextMeasurer, TextStyle};
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
struct XyChartModel {
    #[serde(default)]
    pub orientation: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub plots: Vec<XyChartPlotModel>,
    #[serde(rename = "xAxis")]
    pub x_axis: XyChartAxisModel,
    #[serde(rename = "yAxis")]
    pub y_axis: XyChartAxisModel,
}

#[derive(Debug, Clone, Deserialize)]
struct XyChartPlotModel {
    #[serde(rename = "type")]
    pub plot_type: String,
    #[serde(default)]
    pub data: Vec<(String, Option<f64>)>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum XyChartAxisModel {
    #[serde(rename = "band")]
    Band {
        #[serde(default)]
        title: String,
        #[serde(default)]
        categories: Vec<String>,
    },
    #[serde(rename = "linear")]
    Linear {
        #[serde(default)]
        title: String,
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
    },
}

#[derive(Debug, Clone)]
struct ChartThemeConfig {
    background_color: String,
    title_color: String,
    x_axis_title_color: String,
    x_axis_label_color: String,
    x_axis_tick_color: String,
    x_axis_line_color: String,
    y_axis_title_color: String,
    y_axis_label_color: String,
    y_axis_tick_color: String,
    y_axis_line_color: String,
    plot_color_palette: Vec<String>,
}

#[derive(Debug, Clone)]
struct AxisThemeConfig {
    title_color: String,
    label_color: String,
    tick_color: String,
    axis_line_color: String,
}

#[derive(Debug, Clone)]
struct AxisConfig {
    show_label: bool,
    label_font_size: f64,
    label_padding: f64,
    show_title: bool,
    title_font_size: f64,
    title_padding: f64,
    show_tick: bool,
    tick_length: f64,
    tick_width: f64,
    show_axis_line: bool,
    axis_line_width: f64,
}

#[derive(Debug, Clone)]
struct ChartConfig {
    width: f64,
    height: f64,
    plot_reserved_space_percent: f64,
    show_data_label: bool,
    show_title: bool,
    title_font_size: f64,
    title_padding: f64,
    chart_orientation: String,
    x_axis: AxisConfig,
    y_axis: AxisConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AxisPosition {
    Left,
    Bottom,
    Top,
}

#[derive(Debug, Clone, Copy)]
struct Dimension {
    width: f64,
    height: f64,
}

type Point = merman_core::geom::Point;

fn pt(x: f64, y: f64) -> Point {
    merman_core::geom::point(x, y)
}

#[derive(Debug, Clone, Copy)]
struct BoundingRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn json_f64(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_i64().map(|n| n as f64))
        .or_else(|| v.as_u64().map(|n| n as f64))
}

fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    json_f64(cur)
}

fn config_bool(cfg: &Value, path: &[&str]) -> Option<bool> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_bool()
}

fn config_string(cfg: &Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn has_ref_object(v: &Value) -> bool {
    v.as_object().is_some_and(|m| m.contains_key("$ref"))
}

fn default_axis_config() -> AxisConfig {
    AxisConfig {
        show_label: true,
        label_font_size: 14.0,
        label_padding: 5.0,
        show_title: true,
        title_font_size: 16.0,
        title_padding: 5.0,
        show_tick: true,
        tick_length: 5.0,
        tick_width: 2.0,
        show_axis_line: true,
        axis_line_width: 2.0,
    }
}

fn parse_axis_config(effective_config: &Value, axis_key: &str) -> AxisConfig {
    let base = default_axis_config();
    let Some(v) = effective_config
        .get("xyChart")
        .and_then(|c| c.get(axis_key))
    else {
        return base;
    };
    if !v.is_object() || has_ref_object(v) {
        return base;
    }

    AxisConfig {
        show_label: config_bool(effective_config, &["xyChart", axis_key, "showLabel"])
            .unwrap_or(base.show_label),
        label_font_size: config_f64(effective_config, &["xyChart", axis_key, "labelFontSize"])
            .unwrap_or(base.label_font_size),
        label_padding: config_f64(effective_config, &["xyChart", axis_key, "labelPadding"])
            .unwrap_or(base.label_padding),
        show_title: config_bool(effective_config, &["xyChart", axis_key, "showTitle"])
            .unwrap_or(base.show_title),
        title_font_size: config_f64(effective_config, &["xyChart", axis_key, "titleFontSize"])
            .unwrap_or(base.title_font_size),
        title_padding: config_f64(effective_config, &["xyChart", axis_key, "titlePadding"])
            .unwrap_or(base.title_padding),
        show_tick: config_bool(effective_config, &["xyChart", axis_key, "showTick"])
            .unwrap_or(base.show_tick),
        tick_length: config_f64(effective_config, &["xyChart", axis_key, "tickLength"])
            .unwrap_or(base.tick_length),
        tick_width: config_f64(effective_config, &["xyChart", axis_key, "tickWidth"])
            .unwrap_or(base.tick_width),
        show_axis_line: config_bool(effective_config, &["xyChart", axis_key, "showAxisLine"])
            .unwrap_or(base.show_axis_line),
        axis_line_width: config_f64(effective_config, &["xyChart", axis_key, "axisLineWidth"])
            .unwrap_or(base.axis_line_width),
    }
}

fn default_plot_color_palette() -> Vec<String> {
    "#ECECFF,#8493A6,#FFC3A0,#DCDDE1,#B8E994,#D1A36F,#C3CDE6,#FFB6C1,#496078,#F8F3E3"
        .split(',')
        .map(|s| s.trim().to_string())
        .collect()
}

fn theme_xychart_color(effective_config: &Value, key: &str) -> Option<String> {
    config_string(effective_config, &["themeVariables", "xyChart", key])
}

fn theme_color(effective_config: &Value, key: &str) -> Option<String> {
    config_string(effective_config, &["themeVariables", key])
}

fn invert_hex_color(s: &str) -> Option<String> {
    let s = s.trim();
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(format!("#{:02x}{:02x}{:02x}", 255 - r, 255 - g, 255 - b))
}

fn parse_theme_config(effective_config: &Value) -> ChartThemeConfig {
    let background = theme_xychart_color(effective_config, "backgroundColor")
        .or_else(|| theme_color(effective_config, "background"))
        .unwrap_or_else(|| "white".to_string());
    let primary_color =
        theme_color(effective_config, "primaryColor").unwrap_or_else(|| "#ECECFF".to_string());
    let primary_text = theme_color(effective_config, "primaryTextColor")
        .or_else(|| invert_hex_color(&primary_color))
        .unwrap_or_else(|| "#333".to_string());

    let palette_raw = theme_xychart_color(effective_config, "plotColorPalette");
    let plot_color_palette = palette_raw
        .map(|s| {
            s.split(',')
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty())
                .collect()
        })
        .unwrap_or_else(default_plot_color_palette);

    ChartThemeConfig {
        background_color: background,
        title_color: theme_xychart_color(effective_config, "titleColor")
            .unwrap_or_else(|| primary_text.clone()),
        x_axis_title_color: theme_xychart_color(effective_config, "xAxisTitleColor")
            .unwrap_or_else(|| primary_text.clone()),
        x_axis_label_color: theme_xychart_color(effective_config, "xAxisLabelColor")
            .unwrap_or_else(|| primary_text.clone()),
        x_axis_tick_color: theme_xychart_color(effective_config, "xAxisTickColor")
            .unwrap_or_else(|| primary_text.clone()),
        x_axis_line_color: theme_xychart_color(effective_config, "xAxisLineColor")
            .unwrap_or_else(|| primary_text.clone()),
        y_axis_title_color: theme_xychart_color(effective_config, "yAxisTitleColor")
            .unwrap_or_else(|| primary_text.clone()),
        y_axis_label_color: theme_xychart_color(effective_config, "yAxisLabelColor")
            .unwrap_or_else(|| primary_text.clone()),
        y_axis_tick_color: theme_xychart_color(effective_config, "yAxisTickColor")
            .unwrap_or_else(|| primary_text.clone()),
        y_axis_line_color: theme_xychart_color(effective_config, "yAxisLineColor")
            .unwrap_or_else(|| primary_text.clone()),
        plot_color_palette,
    }
}

fn parse_chart_config(effective_config: &Value, model: &XyChartModel) -> ChartConfig {
    ChartConfig {
        width: config_f64(effective_config, &["xyChart", "width"]).unwrap_or(700.0),
        height: config_f64(effective_config, &["xyChart", "height"]).unwrap_or(500.0),
        plot_reserved_space_percent: config_f64(
            effective_config,
            &["xyChart", "plotReservedSpacePercent"],
        )
        .unwrap_or(50.0),
        show_data_label: config_bool(effective_config, &["xyChart", "showDataLabel"])
            .unwrap_or(false),
        show_title: config_bool(effective_config, &["xyChart", "showTitle"]).unwrap_or(true),
        title_font_size: config_f64(effective_config, &["xyChart", "titleFontSize"])
            .unwrap_or(20.0),
        title_padding: config_f64(effective_config, &["xyChart", "titlePadding"]).unwrap_or(10.0),
        chart_orientation: match model.orientation.as_str() {
            "horizontal" => "horizontal".to_string(),
            _ => "vertical".to_string(),
        },
        x_axis: parse_axis_config(effective_config, "xAxis"),
        y_axis: parse_axis_config(effective_config, "yAxis"),
    }
}

fn max_text_dimension(texts: &[String], font_size: f64, measurer: &dyn TextMeasurer) -> Dimension {
    let style = TextStyle {
        font_size,
        ..Default::default()
    };
    let mut max_w: f64 = 0.0;
    let mut max_h: f64 = 0.0;
    if texts.is_empty() {
        return Dimension {
            width: 0.0,
            height: 0.0,
        };
    }
    for t in texts {
        let m = measurer.measure(t, &style);
        max_w = max_w.max(m.width);
        max_h = max_h.max(m.height);
    }
    Dimension {
        width: max_w,
        height: max_h,
    }
}

fn d3_ticks(start: f64, stop: f64, count: usize) -> Vec<f64> {
    fn tick_spec(start: f64, stop: f64, count: f64) -> Option<(i64, i64, f64)> {
        if !(count > 0.0) {
            return None;
        }

        let step = (stop - start) / count.max(0.0);
        if !step.is_finite() || step == 0.0 {
            return None;
        }
        let power = step.log10().floor();
        let error = step / 10f64.powf(power);
        let e10 = 50f64.sqrt();
        let e5 = 10f64.sqrt();
        let e2 = 2f64.sqrt();
        let factor = if error >= e10 {
            10.0
        } else if error >= e5 {
            5.0
        } else if error >= e2 {
            2.0
        } else {
            1.0
        };

        let (i1, i2, inc) = if power < 0.0 {
            let inc = 10f64.powf(-power) / factor;
            let mut i1 = (start * inc).round() as i64;
            let mut i2 = (stop * inc).round() as i64;
            if (i1 as f64) / inc < start {
                i1 += 1;
            }
            if (i2 as f64) / inc > stop {
                i2 -= 1;
            }
            (i1, i2, -inc)
        } else {
            let inc = 10f64.powf(power) * factor;
            let mut i1 = (start / inc).round() as i64;
            let mut i2 = (stop / inc).round() as i64;
            if (i1 as f64) * inc < start {
                i1 += 1;
            }
            if (i2 as f64) * inc > stop {
                i2 -= 1;
            }
            (i1, i2, inc)
        };

        if i2 < i1 && (0.5..2.0).contains(&count) {
            return tick_spec(start, stop, count * 2.0);
        }

        if !inc.is_finite() {
            return None;
        }
        if inc == 0.0 {
            return None;
        }

        Some((i1, i2, inc))
    }

    if !start.is_finite() || !stop.is_finite() {
        return Vec::new();
    }
    let start = start;
    let stop = stop;
    let count = count as f64;
    if !(count > 0.0) {
        return Vec::new();
    }
    if start == stop {
        return vec![start];
    }

    let reverse = stop < start;
    let (a, b) = if reverse {
        (stop, start)
    } else {
        (start, stop)
    };
    let Some((i1, i2, inc)) = tick_spec(a, b, count) else {
        return Vec::new();
    };
    if i2 < i1 {
        return Vec::new();
    }

    let n = (i2 - i1 + 1).max(0) as usize;
    let mut out = Vec::with_capacity(n);

    if reverse {
        if inc < 0.0 {
            for i in 0..n {
                out.push((i2 - i as i64) as f64 / -inc);
            }
        } else {
            for i in 0..n {
                out.push((i2 - i as i64) as f64 * inc);
            }
        }
    } else if inc < 0.0 {
        for i in 0..n {
            out.push((i1 + i as i64) as f64 / -inc);
        }
    } else {
        for i in 0..n {
            out.push((i1 + i as i64) as f64 * inc);
        }
    }

    out
}

#[derive(Debug, Clone)]
enum AxisKind {
    Band { categories: Vec<String> },
    Linear { domain: (f64, f64) },
}

#[derive(Debug, Clone)]
struct Axis {
    kind: AxisKind,
    axis_config: AxisConfig,
    axis_theme: AxisThemeConfig,
    axis_position: AxisPosition,
    bounding_rect: BoundingRect,
    range: (f64, f64),
    show_title: bool,
    show_label: bool,
    show_tick: bool,
    show_axis_line: bool,
    outer_padding: f64,
    title: String,
    title_text_height: f64,
}

impl Axis {
    fn new(
        kind: AxisKind,
        axis_config: AxisConfig,
        axis_theme: AxisThemeConfig,
        title: String,
    ) -> Self {
        Self {
            kind,
            axis_config,
            axis_theme,
            axis_position: AxisPosition::Left,
            bounding_rect: BoundingRect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            range: (0.0, 10.0),
            show_title: false,
            show_label: false,
            show_tick: false,
            show_axis_line: false,
            outer_padding: 0.0,
            title,
            title_text_height: 0.0,
        }
    }

    fn set_axis_position(&mut self, pos: AxisPosition) {
        self.axis_position = pos;
        let range = self.range;
        self.set_range(range);
    }

    fn set_range(&mut self, range: (f64, f64)) {
        self.range = range;
        if matches!(self.axis_position, AxisPosition::Left) {
            self.bounding_rect.height = range.1 - range.0;
        } else {
            self.bounding_rect.width = range.1 - range.0;
        }
    }

    fn set_bounding_box_xy(&mut self, pt: Point) {
        self.bounding_rect.x = pt.x;
        self.bounding_rect.y = pt.y;
    }

    fn get_range(&self) -> (f64, f64) {
        (
            self.range.0 + self.outer_padding,
            self.range.1 - self.outer_padding,
        )
    }

    fn tick_values(&self) -> Vec<String> {
        match &self.kind {
            AxisKind::Band { categories } => categories.clone(),
            AxisKind::Linear { domain } => {
                let (mut a, mut b) = *domain;
                if matches!(self.axis_position, AxisPosition::Left) {
                    std::mem::swap(&mut a, &mut b);
                }
                d3_ticks(a, b, 10)
                    .into_iter()
                    .map(|v| format!("{v}"))
                    .collect()
            }
        }
    }

    fn tick_distance(&self) -> f64 {
        let ticks = self.tick_values();
        let (a, b) = self.get_range();
        let span = (a - b).abs();
        if ticks.is_empty() {
            return 0.0;
        }
        span / (ticks.len() as f64)
    }

    fn get_scale_value(&self, value: &str) -> f64 {
        match &self.kind {
            AxisKind::Band { categories } => {
                let (a, b) = self.get_range();
                let n = categories.len();
                if n == 0 {
                    return a;
                }
                if n == 1 {
                    return a + (b - a) * 0.5;
                }
                let step = (b - a) / ((n - 1) as f64);
                let idx = categories.iter().position(|c| c == value).unwrap_or(0);
                a + step * (idx as f64)
            }
            AxisKind::Linear { domain } => {
                let Ok(v) = value.parse::<f64>() else {
                    return self.get_range().0;
                };
                if v.is_nan() {
                    return f64::NAN;
                }
                let (mut d0, mut d1) = *domain;
                if matches!(self.axis_position, AxisPosition::Left) {
                    std::mem::swap(&mut d0, &mut d1);
                }
                let (r0, r1) = self.get_range();
                if d0 == d1 {
                    return r0 + (r1 - r0) * 0.5;
                }
                let t = (v - d0) / (d1 - d0);
                r0 + t * (r1 - r0)
            }
        }
    }

    fn recalculate_outer_padding_to_draw_bar(&mut self) {
        const BAR_WIDTH_TO_TICK_WIDTH_RATIO: f64 = 0.7;
        let target = BAR_WIDTH_TO_TICK_WIDTH_RATIO * self.tick_distance();
        if target > self.outer_padding * 2.0 {
            self.outer_padding = (target / 2.0).floor();
        }
    }

    fn calculate_space(&mut self, available: Dimension, measurer: &dyn TextMeasurer) -> Dimension {
        self.show_title = false;
        self.show_label = false;
        self.show_tick = false;
        self.show_axis_line = false;
        self.outer_padding = 0.0;
        self.title_text_height = 0.0;

        if matches!(self.axis_position, AxisPosition::Left) {
            let mut available_width = available.width;

            if self.axis_config.show_axis_line && available_width > self.axis_config.axis_line_width
            {
                available_width -= self.axis_config.axis_line_width;
                self.show_axis_line = true;
            }

            if self.axis_config.show_label {
                let ticks = self.tick_values();
                let dim = max_text_dimension(&ticks, self.axis_config.label_font_size, measurer);
                let max_padding = 0.2 * available.height;
                self.outer_padding = (dim.height / 2.0).min(max_padding);
                let width_required = dim.width + self.axis_config.label_padding * 2.0;
                if width_required <= available_width {
                    available_width -= width_required;
                    self.show_label = true;
                }
            }

            if self.axis_config.show_tick && available_width >= self.axis_config.tick_length {
                self.show_tick = true;
                available_width -= self.axis_config.tick_length;
            }

            if self.axis_config.show_title && !self.title.is_empty() {
                let dim = max_text_dimension(
                    &[self.title.clone()],
                    self.axis_config.title_font_size,
                    measurer,
                );
                let width_required = dim.height + self.axis_config.title_padding * 2.0;
                self.title_text_height = dim.height;
                if width_required <= available_width {
                    available_width -= width_required;
                    self.show_title = true;
                }
            }

            self.bounding_rect.width = available.width - available_width;
            self.bounding_rect.height = available.height;
            Dimension {
                width: self.bounding_rect.width,
                height: self.bounding_rect.height,
            }
        } else {
            let mut available_height = available.height;

            if self.axis_config.show_axis_line
                && available_height > self.axis_config.axis_line_width
            {
                available_height -= self.axis_config.axis_line_width;
                self.show_axis_line = true;
            }

            if self.axis_config.show_label {
                let ticks = self.tick_values();
                let dim = max_text_dimension(&ticks, self.axis_config.label_font_size, measurer);
                let max_padding = 0.2 * available.width;
                self.outer_padding = (dim.width / 2.0).min(max_padding);
                let height_required = dim.height + self.axis_config.label_padding * 2.0;
                if height_required <= available_height {
                    available_height -= height_required;
                    self.show_label = true;
                }
            }

            if self.axis_config.show_tick && available_height >= self.axis_config.tick_length {
                self.show_tick = true;
                available_height -= self.axis_config.tick_length;
            }

            if self.axis_config.show_title && !self.title.is_empty() {
                let dim = max_text_dimension(
                    &[self.title.clone()],
                    self.axis_config.title_font_size,
                    measurer,
                );
                let height_required = dim.height + self.axis_config.title_padding * 2.0;
                self.title_text_height = dim.height;
                if height_required <= available_height {
                    available_height -= height_required;
                    self.show_title = true;
                }
            }

            self.bounding_rect.width = available.width;
            self.bounding_rect.height = available.height - available_height;
            Dimension {
                width: self.bounding_rect.width,
                height: self.bounding_rect.height,
            }
        }
    }

    fn drawable_elements(&self) -> Vec<XyChartDrawableElem> {
        match self.axis_position {
            AxisPosition::Left => self.drawable_elements_for_left_axis(),
            AxisPosition::Bottom => self.drawable_elements_for_bottom_axis(),
            AxisPosition::Top => self.drawable_elements_for_top_axis(),
        }
    }

    fn drawable_elements_for_left_axis(&self) -> Vec<XyChartDrawableElem> {
        let mut out: Vec<XyChartDrawableElem> = Vec::new();
        if self.show_axis_line {
            let x = self.bounding_rect.x + self.bounding_rect.width
                - self.axis_config.axis_line_width / 2.0;
            out.push(XyChartDrawableElem::Path {
                group_texts: vec!["left-axis".to_string(), "axisl-line".to_string()],
                data: vec![XyChartPathData {
                    path: format!(
                        "M {x},{} L {x},{} ",
                        self.bounding_rect.y,
                        self.bounding_rect.y + self.bounding_rect.height
                    ),
                    fill: None,
                    stroke_fill: self.axis_theme.axis_line_color.clone(),
                    stroke_width: self.axis_config.axis_line_width,
                }],
            });
        }
        if self.show_label {
            let x = self.bounding_rect.x + self.bounding_rect.width
                - (if self.show_label {
                    self.axis_config.label_padding
                } else {
                    0.0
                })
                - (if self.show_tick {
                    self.axis_config.tick_length
                } else {
                    0.0
                })
                - (if self.show_axis_line {
                    self.axis_config.axis_line_width
                } else {
                    0.0
                });
            let ticks = self.tick_values();
            out.push(XyChartDrawableElem::Text {
                group_texts: vec!["left-axis".to_string(), "label".to_string()],
                data: ticks
                    .iter()
                    .map(|t| XyChartTextData {
                        text: t.clone(),
                        x,
                        y: self.get_scale_value(t),
                        fill: self.axis_theme.label_color.clone(),
                        font_size: self.axis_config.label_font_size,
                        rotation: 0.0,
                        vertical_pos: "middle".to_string(),
                        horizontal_pos: "right".to_string(),
                    })
                    .collect(),
            });
        }
        if self.show_tick {
            let x = self.bounding_rect.x + self.bounding_rect.width
                - (if self.show_axis_line {
                    self.axis_config.axis_line_width
                } else {
                    0.0
                });
            let ticks = self.tick_values();
            out.push(XyChartDrawableElem::Path {
                group_texts: vec!["left-axis".to_string(), "ticks".to_string()],
                data: ticks
                    .iter()
                    .map(|t| {
                        let y = self.get_scale_value(t);
                        XyChartPathData {
                            path: format!("M {x},{y} L {},{y}", x - self.axis_config.tick_length),
                            fill: None,
                            stroke_fill: self.axis_theme.tick_color.clone(),
                            stroke_width: self.axis_config.tick_width,
                        }
                    })
                    .collect(),
            });
        }
        if self.show_title {
            out.push(XyChartDrawableElem::Text {
                group_texts: vec!["left-axis".to_string(), "title".to_string()],
                data: vec![XyChartTextData {
                    text: self.title.clone(),
                    x: self.bounding_rect.x + self.axis_config.title_padding,
                    y: self.bounding_rect.y + self.bounding_rect.height / 2.0,
                    fill: self.axis_theme.title_color.clone(),
                    font_size: self.axis_config.title_font_size,
                    rotation: 270.0,
                    vertical_pos: "top".to_string(),
                    horizontal_pos: "center".to_string(),
                }],
            });
        }
        out
    }

    fn drawable_elements_for_bottom_axis(&self) -> Vec<XyChartDrawableElem> {
        let mut out: Vec<XyChartDrawableElem> = Vec::new();
        if self.show_axis_line {
            let y = self.bounding_rect.y + self.axis_config.axis_line_width / 2.0;
            out.push(XyChartDrawableElem::Path {
                group_texts: vec!["bottom-axis".to_string(), "axis-line".to_string()],
                data: vec![XyChartPathData {
                    path: format!(
                        "M {},{y} L {},{y}",
                        self.bounding_rect.x,
                        self.bounding_rect.x + self.bounding_rect.width
                    ),
                    fill: None,
                    stroke_fill: self.axis_theme.axis_line_color.clone(),
                    stroke_width: self.axis_config.axis_line_width,
                }],
            });
        }
        if self.show_label {
            let ticks = self.tick_values();
            out.push(XyChartDrawableElem::Text {
                group_texts: vec!["bottom-axis".to_string(), "label".to_string()],
                data: ticks
                    .iter()
                    .map(|t| XyChartTextData {
                        text: t.clone(),
                        x: self.get_scale_value(t),
                        y: self.bounding_rect.y
                            + self.axis_config.label_padding
                            + (if self.show_tick {
                                self.axis_config.tick_length
                            } else {
                                0.0
                            })
                            + (if self.show_axis_line {
                                self.axis_config.axis_line_width
                            } else {
                                0.0
                            }),
                        fill: self.axis_theme.label_color.clone(),
                        font_size: self.axis_config.label_font_size,
                        rotation: 0.0,
                        vertical_pos: "top".to_string(),
                        horizontal_pos: "center".to_string(),
                    })
                    .collect(),
            });
        }
        if self.show_tick {
            let y = self.bounding_rect.y
                + (if self.show_axis_line {
                    self.axis_config.axis_line_width
                } else {
                    0.0
                });
            let ticks = self.tick_values();
            out.push(XyChartDrawableElem::Path {
                group_texts: vec!["bottom-axis".to_string(), "ticks".to_string()],
                data: ticks
                    .iter()
                    .map(|t| {
                        let x = self.get_scale_value(t);
                        XyChartPathData {
                            path: format!("M {x},{y} L {x},{}", y + self.axis_config.tick_length),
                            fill: None,
                            stroke_fill: self.axis_theme.tick_color.clone(),
                            stroke_width: self.axis_config.tick_width,
                        }
                    })
                    .collect(),
            });
        }
        if self.show_title {
            out.push(XyChartDrawableElem::Text {
                group_texts: vec!["bottom-axis".to_string(), "title".to_string()],
                data: vec![XyChartTextData {
                    text: self.title.clone(),
                    x: self.range.0 + (self.range.1 - self.range.0) / 2.0,
                    y: self.bounding_rect.y + self.bounding_rect.height
                        - self.axis_config.title_padding
                        - self.title_text_height,
                    fill: self.axis_theme.title_color.clone(),
                    font_size: self.axis_config.title_font_size,
                    rotation: 0.0,
                    vertical_pos: "top".to_string(),
                    horizontal_pos: "center".to_string(),
                }],
            });
        }
        out
    }

    fn drawable_elements_for_top_axis(&self) -> Vec<XyChartDrawableElem> {
        let mut out: Vec<XyChartDrawableElem> = Vec::new();
        if self.show_axis_line {
            let y = self.bounding_rect.y + self.bounding_rect.height
                - self.axis_config.axis_line_width / 2.0;
            out.push(XyChartDrawableElem::Path {
                group_texts: vec!["top-axis".to_string(), "axis-line".to_string()],
                data: vec![XyChartPathData {
                    path: format!(
                        "M {},{y} L {},{y}",
                        self.bounding_rect.x,
                        self.bounding_rect.x + self.bounding_rect.width
                    ),
                    fill: None,
                    stroke_fill: self.axis_theme.axis_line_color.clone(),
                    stroke_width: self.axis_config.axis_line_width,
                }],
            });
        }
        if self.show_label {
            let ticks = self.tick_values();
            out.push(XyChartDrawableElem::Text {
                group_texts: vec!["top-axis".to_string(), "label".to_string()],
                data: ticks
                    .iter()
                    .map(|t| XyChartTextData {
                        text: t.clone(),
                        x: self.get_scale_value(t),
                        y: self.bounding_rect.y
                            + (if self.show_title {
                                self.title_text_height + self.axis_config.title_padding * 2.0
                            } else {
                                0.0
                            })
                            + self.axis_config.label_padding,
                        fill: self.axis_theme.label_color.clone(),
                        font_size: self.axis_config.label_font_size,
                        rotation: 0.0,
                        vertical_pos: "top".to_string(),
                        horizontal_pos: "center".to_string(),
                    })
                    .collect(),
            });
        }
        if self.show_tick {
            let y = self.bounding_rect.y;
            let ticks = self.tick_values();
            out.push(XyChartDrawableElem::Path {
                group_texts: vec!["top-axis".to_string(), "ticks".to_string()],
                data: ticks
                    .iter()
                    .map(|t| {
                        let x = self.get_scale_value(t);
                        let y0 = y + self.bounding_rect.height
                            - (if self.show_axis_line {
                                self.axis_config.axis_line_width
                            } else {
                                0.0
                            });
                        let y1 = y + self.bounding_rect.height
                            - self.axis_config.tick_length
                            - (if self.show_axis_line {
                                self.axis_config.axis_line_width
                            } else {
                                0.0
                            });
                        XyChartPathData {
                            path: format!("M {x},{y0} L {x},{y1}"),
                            fill: None,
                            stroke_fill: self.axis_theme.tick_color.clone(),
                            stroke_width: self.axis_config.tick_width,
                        }
                    })
                    .collect(),
            });
        }
        if self.show_title {
            out.push(XyChartDrawableElem::Text {
                group_texts: vec!["top-axis".to_string(), "title".to_string()],
                data: vec![XyChartTextData {
                    text: self.title.clone(),
                    x: self.bounding_rect.x + self.bounding_rect.width / 2.0,
                    y: self.bounding_rect.y + self.axis_config.title_padding,
                    fill: self.axis_theme.title_color.clone(),
                    font_size: self.axis_config.title_font_size,
                    rotation: 0.0,
                    vertical_pos: "top".to_string(),
                    horizontal_pos: "center".to_string(),
                }],
            });
        }
        out
    }
}

fn plot_color_from_palette(palette: &[String], plot_index: usize) -> String {
    if palette.is_empty() {
        return String::new();
    }
    let idx = if plot_index == 0 {
        0
    } else {
        plot_index % palette.len()
    };
    palette[idx].clone()
}

fn line_path(points: &[(f64, f64)]) -> Option<String> {
    let (first, rest) = points.split_first()?;
    if rest.is_empty() {
        return Some(format!("M{},{}Z", first.0, first.1));
    }
    let mut out = format!("M{},{}", first.0, first.1);
    for p in rest {
        out.push_str(&format!("L{},{}", p.0, p.1));
    }
    Some(out)
}

pub(crate) fn layout_xychart_diagram(
    semantic: &Value,
    effective_config: &Value,
    text_measurer: &dyn TextMeasurer,
) -> Result<XyChartDiagramLayout> {
    let model: XyChartModel = serde_json::from_value(semantic.clone()).map_err(Error::Json)?;

    if model
        .orientation
        .as_str()
        .split_whitespace()
        .next()
        .is_some_and(|t| t != "vertical" && t != "horizontal" && !t.is_empty())
    {
        return Err(Error::InvalidModel {
            message: format!("unexpected xychart orientation: {}", model.orientation),
        });
    }

    let chart_cfg = parse_chart_config(effective_config, &model);
    let theme_cfg = parse_theme_config(effective_config);

    let title = model.title.clone().unwrap_or_default();
    let title_dim = max_text_dimension(&[title.clone()], chart_cfg.title_font_size, text_measurer);
    let title_height = title_dim.height + 2.0 * chart_cfg.title_padding;
    let show_chart_title =
        chart_cfg.show_title && !title.is_empty() && title_height <= chart_cfg.height;

    let mut drawables: Vec<XyChartDrawableElem> = Vec::new();
    if show_chart_title {
        drawables.push(XyChartDrawableElem::Text {
            group_texts: vec!["chart-title".to_string()],
            data: vec![XyChartTextData {
                text: title.clone(),
                x: chart_cfg.width / 2.0,
                y: title_height / 2.0,
                fill: theme_cfg.title_color.clone(),
                font_size: chart_cfg.title_font_size,
                rotation: 0.0,
                vertical_pos: "middle".to_string(),
                horizontal_pos: "center".to_string(),
            }],
        });
    }

    let (x_axis_kind, x_axis_title) = match &model.x_axis {
        XyChartAxisModel::Band { title, categories } => (
            AxisKind::Band {
                categories: categories.clone(),
            },
            title.clone(),
        ),
        XyChartAxisModel::Linear { title, min, max } => (
            AxisKind::Linear {
                domain: (min.unwrap_or(0.0), max.unwrap_or(1.0)),
            },
            title.clone(),
        ),
    };
    let (y_axis_kind, y_axis_title) = match &model.y_axis {
        XyChartAxisModel::Band { title, categories } => (
            AxisKind::Band {
                categories: categories.clone(),
            },
            title.clone(),
        ),
        XyChartAxisModel::Linear { title, min, max } => (
            AxisKind::Linear {
                domain: (min.unwrap_or(0.0), max.unwrap_or(1.0)),
            },
            title.clone(),
        ),
    };

    let x_axis_theme = AxisThemeConfig {
        title_color: theme_cfg.x_axis_title_color.clone(),
        label_color: theme_cfg.x_axis_label_color.clone(),
        tick_color: theme_cfg.x_axis_tick_color.clone(),
        axis_line_color: theme_cfg.x_axis_line_color.clone(),
    };
    let y_axis_theme = AxisThemeConfig {
        title_color: theme_cfg.y_axis_title_color.clone(),
        label_color: theme_cfg.y_axis_label_color.clone(),
        tick_color: theme_cfg.y_axis_tick_color.clone(),
        axis_line_color: theme_cfg.y_axis_line_color.clone(),
    };

    let mut x_axis = Axis::new(
        x_axis_kind,
        chart_cfg.x_axis.clone(),
        x_axis_theme,
        x_axis_title,
    );
    let mut y_axis = Axis::new(
        y_axis_kind,
        chart_cfg.y_axis.clone(),
        y_axis_theme,
        y_axis_title,
    );

    let mut chart_width = (chart_cfg.width * chart_cfg.plot_reserved_space_percent / 100.0).floor();
    let mut chart_height =
        (chart_cfg.height * chart_cfg.plot_reserved_space_percent / 100.0).floor();

    let mut available_width = chart_cfg.width - chart_width;
    let mut available_height = chart_cfg.height - chart_height;

    let plot_rect = if chart_cfg.chart_orientation == "horizontal" {
        let title_y_end = if show_chart_title { title_height } else { 0.0 };
        available_height = (available_height - title_y_end).max(0.0);

        x_axis.set_axis_position(AxisPosition::Left);
        let space_used_x = x_axis.calculate_space(
            Dimension {
                width: available_width,
                height: available_height,
            },
            text_measurer,
        );
        available_width = (available_width - space_used_x.width).max(0.0);
        let plot_x = space_used_x.width;

        y_axis.set_axis_position(AxisPosition::Top);
        let space_used_y = y_axis.calculate_space(
            Dimension {
                width: available_width,
                height: available_height,
            },
            text_measurer,
        );
        available_height = (available_height - space_used_y.height).max(0.0);
        let plot_y = title_y_end + space_used_y.height;

        if available_width > 0.0 {
            chart_width += available_width;
        }
        if available_height > 0.0 {
            chart_height += available_height;
        }

        let plot_rect = BoundingRect {
            x: plot_x,
            y: plot_y,
            width: chart_width,
            height: chart_height,
        };

        y_axis.set_range((plot_x, plot_x + chart_width));
        y_axis.set_bounding_box_xy(pt(plot_x, title_y_end));
        x_axis.set_range((plot_y, plot_y + chart_height));
        x_axis.set_bounding_box_xy(pt(0.0, plot_y));
        plot_rect
    } else {
        let plot_y = if show_chart_title { title_height } else { 0.0 };
        available_height = (available_height - plot_y).max(0.0);

        x_axis.set_axis_position(AxisPosition::Bottom);
        let space_used_x = x_axis.calculate_space(
            Dimension {
                width: available_width,
                height: available_height,
            },
            text_measurer,
        );
        available_height = (available_height - space_used_x.height).max(0.0);

        y_axis.set_axis_position(AxisPosition::Left);
        let space_used_y = y_axis.calculate_space(
            Dimension {
                width: available_width,
                height: available_height,
            },
            text_measurer,
        );
        let plot_x = space_used_y.width;
        available_width = (available_width - space_used_y.width).max(0.0);

        if available_width > 0.0 {
            chart_width += available_width;
        }
        if available_height > 0.0 {
            chart_height += available_height;
        }

        let plot_rect = BoundingRect {
            x: plot_x,
            y: plot_y,
            width: chart_width,
            height: chart_height,
        };

        x_axis.set_range((plot_x, plot_x + chart_width));
        x_axis.set_bounding_box_xy(pt(plot_x, plot_y + chart_height));
        y_axis.set_range((plot_y, plot_y + chart_height));
        y_axis.set_bounding_box_xy(pt(0.0, plot_y));
        plot_rect
    };

    if model.plots.iter().any(|p| p.plot_type == "bar") {
        x_axis.recalculate_outer_padding_to_draw_bar();
    }

    for (plot_index, plot) in model.plots.iter().enumerate() {
        let color = plot_color_from_palette(&theme_cfg.plot_color_palette, plot_index);

        match plot.plot_type.as_str() {
            "bar" => {
                let bar_padding_percent = 0.05;
                let bar_width = (x_axis.outer_padding * 2.0).min(x_axis.tick_distance())
                    * (1.0 - bar_padding_percent);
                let bar_width_half = bar_width / 2.0;

                let mut rects: Vec<XyChartRectData> = Vec::new();
                for (cat, value) in &plot.data {
                    let x = x_axis.get_scale_value(cat);
                    let y = match value {
                        Some(v) => y_axis.get_scale_value(&format!("{v}")),
                        None => y_axis.get_scale_value("NaN"),
                    };
                    if chart_cfg.chart_orientation == "horizontal" {
                        rects.push(XyChartRectData {
                            x: plot_rect.x,
                            y: x - bar_width_half,
                            width: y - plot_rect.x,
                            height: bar_width,
                            fill: color.clone(),
                            stroke_fill: color.clone(),
                            stroke_width: 0.0,
                        });
                    } else {
                        rects.push(XyChartRectData {
                            x: x - bar_width_half,
                            y,
                            width: bar_width,
                            height: plot_rect.y + plot_rect.height - y,
                            fill: color.clone(),
                            stroke_fill: color.clone(),
                            stroke_width: 0.0,
                        });
                    }
                }

                drawables.push(XyChartDrawableElem::Rect {
                    group_texts: vec!["plot".to_string(), format!("bar-plot-{plot_index}")],
                    data: rects,
                });
            }
            "line" => {
                let mut points: Vec<(f64, f64)> = Vec::new();
                for (cat, value) in &plot.data {
                    let x = x_axis.get_scale_value(cat);
                    let y = match value {
                        Some(v) => y_axis.get_scale_value(&format!("{v}")),
                        None => y_axis.get_scale_value("NaN"),
                    };
                    points.push(if chart_cfg.chart_orientation == "horizontal" {
                        (y, x)
                    } else {
                        (x, y)
                    });
                }
                if let Some(path) = line_path(&points) {
                    drawables.push(XyChartDrawableElem::Path {
                        group_texts: vec!["plot".to_string(), format!("line-plot-{plot_index}")],
                        data: vec![XyChartPathData {
                            path,
                            fill: None,
                            stroke_fill: color,
                            stroke_width: 2.0,
                        }],
                    });
                }
            }
            _ => {}
        }
    }

    drawables.extend(x_axis.drawable_elements());
    drawables.extend(y_axis.drawable_elements());

    let label_data = model
        .plots
        .first()
        .map(|p| {
            p.data
                .iter()
                .map(|(_, y)| {
                    y.map(|v| format!("{v}"))
                        .unwrap_or_else(|| "null".to_string())
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(XyChartDiagramLayout {
        width: chart_cfg.width,
        height: chart_cfg.height,
        chart_orientation: chart_cfg.chart_orientation,
        show_data_label: chart_cfg.show_data_label,
        background_color: theme_cfg.background_color,
        label_data,
        drawables,
    })
}

use crate::Result;
use crate::model::{
    QuadrantChartAxisLabelData, QuadrantChartBorderLineData, QuadrantChartDiagramLayout,
    QuadrantChartPointData, QuadrantChartQuadrantData, QuadrantChartTextData,
};
use crate::text::TextMeasurer;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuadrantChartStyles {
    radius: Option<f64>,
    color: Option<String>,
    stroke_color: Option<String>,
    stroke_width: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuadrantChartPointModel {
    text: String,
    x: f64,
    y: f64,
    #[serde(default)]
    class_name: Option<String>,
    #[serde(default)]
    styles: QuadrantChartStyles,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuadrantChartQuadrantsModel {
    quadrant1_text: String,
    quadrant2_text: String,
    quadrant3_text: String,
    quadrant4_text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuadrantChartAxesModel {
    x_axis_left_text: String,
    x_axis_right_text: String,
    y_axis_bottom_text: String,
    y_axis_top_text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuadrantChartModel {
    #[serde(default)]
    title: Option<String>,
    quadrants: QuadrantChartQuadrantsModel,
    axes: QuadrantChartAxesModel,
    #[serde(default)]
    points: Vec<QuadrantChartPointModel>,
    #[serde(default)]
    classes: BTreeMap<String, QuadrantChartStyles>,
}

#[derive(Debug, Clone)]
struct QuadrantChartConfig {
    chart_width: f64,
    chart_height: f64,
    title_padding: f64,
    title_font_size: f64,
    quadrant_padding: f64,
    x_axis_label_padding: f64,
    y_axis_label_padding: f64,
    x_axis_label_font_size: f64,
    y_axis_label_font_size: f64,
    quadrant_label_font_size: f64,
    quadrant_text_top_padding: f64,
    point_text_padding: f64,
    point_label_font_size: f64,
    point_radius: f64,
    x_axis_position: String,
    y_axis_position: String,
    quadrant_internal_border_stroke_width: f64,
    quadrant_external_border_stroke_width: f64,
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

fn config_string(cfg: &Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn default_quadrant_config(effective_config: &Value) -> QuadrantChartConfig {
    QuadrantChartConfig {
        chart_width: config_f64(effective_config, &["quadrantChart", "chartWidth"])
            .unwrap_or(500.0),
        chart_height: config_f64(effective_config, &["quadrantChart", "chartHeight"])
            .unwrap_or(500.0),
        title_padding: config_f64(effective_config, &["quadrantChart", "titlePadding"])
            .unwrap_or(10.0),
        title_font_size: config_f64(effective_config, &["quadrantChart", "titleFontSize"])
            .unwrap_or(20.0),
        quadrant_padding: config_f64(effective_config, &["quadrantChart", "quadrantPadding"])
            .unwrap_or(5.0),
        x_axis_label_padding: config_f64(effective_config, &["quadrantChart", "xAxisLabelPadding"])
            .unwrap_or(5.0),
        y_axis_label_padding: config_f64(effective_config, &["quadrantChart", "yAxisLabelPadding"])
            .unwrap_or(5.0),
        x_axis_label_font_size: config_f64(
            effective_config,
            &["quadrantChart", "xAxisLabelFontSize"],
        )
        .unwrap_or(16.0),
        y_axis_label_font_size: config_f64(
            effective_config,
            &["quadrantChart", "yAxisLabelFontSize"],
        )
        .unwrap_or(16.0),
        quadrant_label_font_size: config_f64(
            effective_config,
            &["quadrantChart", "quadrantLabelFontSize"],
        )
        .unwrap_or(16.0),
        quadrant_text_top_padding: config_f64(
            effective_config,
            &["quadrantChart", "quadrantTextTopPadding"],
        )
        .unwrap_or(5.0),
        point_text_padding: config_f64(effective_config, &["quadrantChart", "pointTextPadding"])
            .unwrap_or(5.0),
        point_label_font_size: config_f64(
            effective_config,
            &["quadrantChart", "pointLabelFontSize"],
        )
        .unwrap_or(12.0),
        point_radius: config_f64(effective_config, &["quadrantChart", "pointRadius"])
            .unwrap_or(5.0),
        x_axis_position: config_string(effective_config, &["quadrantChart", "xAxisPosition"])
            .unwrap_or_else(|| "top".to_string()),
        y_axis_position: config_string(effective_config, &["quadrantChart", "yAxisPosition"])
            .unwrap_or_else(|| "left".to_string()),
        quadrant_internal_border_stroke_width: config_f64(
            effective_config,
            &["quadrantChart", "quadrantInternalBorderStrokeWidth"],
        )
        .unwrap_or(1.0),
        quadrant_external_border_stroke_width: config_f64(
            effective_config,
            &["quadrantChart", "quadrantExternalBorderStrokeWidth"],
        )
        .unwrap_or(2.0),
    }
}

#[derive(Debug, Clone)]
struct QuadrantThemeConfig {
    quadrant1_fill: String,
    quadrant2_fill: String,
    quadrant3_fill: String,
    quadrant4_fill: String,
    quadrant1_text_fill: String,
    quadrant2_text_fill: String,
    quadrant3_text_fill: String,
    quadrant4_text_fill: String,
    quadrant_point_fill: String,
    quadrant_point_text_fill: String,
    quadrant_x_axis_text_fill: String,
    quadrant_y_axis_text_fill: String,
    quadrant_title_fill: String,
    quadrant_internal_border_stroke_fill: String,
    quadrant_external_border_stroke_fill: String,
}

fn parse_hex_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let t = s.trim().strip_prefix('#').unwrap_or(s.trim());
    if t.len() != 6 || !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&t[0..2], 16).ok()?;
    let g = u8::from_str_radix(&t[2..4], 16).ok()?;
    let b = u8::from_str_radix(&t[4..6], 16).ok()?;
    Some((r, g, b))
}

fn invert_hex_rgb(hex: &str) -> Option<String> {
    let (r, g, b) = parse_hex_rgb(hex)?;
    Some(format!("#{:02x}{:02x}{:02x}", 255 - r, 255 - g, 255 - b))
}

fn adjust_hex_rgb(hex: &str, delta: i16) -> Option<String> {
    let (r, g, b) = parse_hex_rgb(hex)?;
    let adj = |c: u8| -> u8 {
        let v = c as i16 + delta;
        v.clamp(0, 255) as u8
    };
    Some(format!("#{:02x}{:02x}{:02x}", adj(r), adj(g), adj(b)))
}

fn default_quadrant_theme() -> QuadrantThemeConfig {
    // Mermaid 11.12.2 default theme values (derived from `theme-default.js`).
    //
    // Note: quadrant point fill currently resolves to an `hsl(...NaN%)` string in upstream.
    // Keep that behavior for DOM parity at the pinned baseline.
    let quadrant1_fill = "#ECECFF".to_string();
    let primary_text = invert_hex_rgb(&quadrant1_fill).unwrap_or_else(|| "#131300".to_string());
    QuadrantThemeConfig {
        quadrant2_fill: adjust_hex_rgb(&quadrant1_fill, 5).unwrap_or_else(|| "#f1f1ff".to_string()),
        quadrant3_fill: adjust_hex_rgb(&quadrant1_fill, 10)
            .unwrap_or_else(|| "#f6f6ff".to_string()),
        quadrant4_fill: adjust_hex_rgb(&quadrant1_fill, 15)
            .unwrap_or_else(|| "#fbfbff".to_string()),
        quadrant1_text_fill: primary_text.clone(),
        quadrant2_text_fill: adjust_hex_rgb(&primary_text, -5)
            .unwrap_or_else(|| "#0e0e00".to_string()),
        quadrant3_text_fill: adjust_hex_rgb(&primary_text, -10)
            .unwrap_or_else(|| "#090900".to_string()),
        quadrant4_text_fill: adjust_hex_rgb(&primary_text, -15)
            .unwrap_or_else(|| "#040400".to_string()),
        quadrant_point_fill: "hsl(240, 100%, NaN%)".to_string(),
        quadrant_point_text_fill: primary_text.clone(),
        quadrant_x_axis_text_fill: primary_text.clone(),
        quadrant_y_axis_text_fill: primary_text.clone(),
        quadrant_title_fill: primary_text,
        quadrant_internal_border_stroke_fill: "rgb(199, 199, 241)".to_string(),
        quadrant_external_border_stroke_fill: "rgb(199, 199, 241)".to_string(),
        quadrant1_fill,
    }
}

fn quadrant_theme_with_overrides(effective_config: &Value) -> QuadrantThemeConfig {
    let mut theme = default_quadrant_theme();

    // Mermaid applies theme variables as raw CSS tokens (some upstream examples omit the leading
    // `#` in hex colors). Preserve the string verbatim for DOM parity.
    let set = |field: &mut String, key: &str| {
        if let Some(v) = config_string(effective_config, &["themeVariables", key]) {
            *field = v;
        }
    };

    set(&mut theme.quadrant1_fill, "quadrant1Fill");
    set(&mut theme.quadrant2_fill, "quadrant2Fill");
    set(&mut theme.quadrant3_fill, "quadrant3Fill");
    set(&mut theme.quadrant4_fill, "quadrant4Fill");

    set(&mut theme.quadrant1_text_fill, "quadrant1TextFill");
    set(&mut theme.quadrant2_text_fill, "quadrant2TextFill");
    set(&mut theme.quadrant3_text_fill, "quadrant3TextFill");
    set(&mut theme.quadrant4_text_fill, "quadrant4TextFill");

    set(&mut theme.quadrant_point_fill, "quadrantPointFill");
    set(&mut theme.quadrant_point_text_fill, "quadrantPointTextFill");
    set(
        &mut theme.quadrant_x_axis_text_fill,
        "quadrantXAxisTextFill",
    );
    set(
        &mut theme.quadrant_y_axis_text_fill,
        "quadrantYAxisTextFill",
    );
    set(&mut theme.quadrant_title_fill, "quadrantTitleFill");

    set(
        &mut theme.quadrant_internal_border_stroke_fill,
        "quadrantInternalBorderStrokeFill",
    );
    set(
        &mut theme.quadrant_external_border_stroke_fill,
        "quadrantExternalBorderStrokeFill",
    );

    theme
}

fn scale_linear(domain: (f64, f64), range: (f64, f64), v: f64) -> f64 {
    let (d0, d1) = domain;
    let (r0, r1) = range;
    if d1 == d0 {
        return r0;
    }
    let t = (v - d0) / (d1 - d0);
    r0 + t * (r1 - r0)
}

pub fn layout_quadrantchart_diagram(
    model: &Value,
    effective_config: &Value,
    _text_measurer: &dyn TextMeasurer,
) -> Result<QuadrantChartDiagramLayout> {
    let model: QuadrantChartModel = serde_json::from_value(model.clone())?;

    let cfg = default_quadrant_config(effective_config);
    let theme = quadrant_theme_with_overrides(effective_config);

    let title_text = model.title.as_deref().unwrap_or("").trim();
    let show_title = !title_text.is_empty();

    let show_x_axis = !model.axes.x_axis_left_text.trim().is_empty()
        || !model.axes.x_axis_right_text.trim().is_empty();
    let show_y_axis = !model.axes.y_axis_top_text.trim().is_empty()
        || !model.axes.y_axis_bottom_text.trim().is_empty();

    let x_axis_position = if model.points.is_empty() {
        cfg.x_axis_position.as_str()
    } else {
        "bottom"
    };

    let x_axis_space_calc = cfg.x_axis_label_padding * 2.0 + cfg.x_axis_label_font_size;
    let x_axis_space_top = if x_axis_position == "top" && show_x_axis {
        x_axis_space_calc
    } else {
        0.0
    };
    let x_axis_space_bottom = if x_axis_position == "bottom" && show_x_axis {
        x_axis_space_calc
    } else {
        0.0
    };

    let y_axis_space_calc = cfg.y_axis_label_padding * 2.0 + cfg.y_axis_label_font_size;
    let y_axis_space_left = if cfg.y_axis_position == "left" && show_y_axis {
        y_axis_space_calc
    } else {
        0.0
    };
    let y_axis_space_right = if cfg.y_axis_position == "right" && show_y_axis {
        y_axis_space_calc
    } else {
        0.0
    };

    let title_space_top = if show_title {
        cfg.title_font_size + cfg.title_padding * 2.0
    } else {
        0.0
    };

    let quadrant_left = cfg.quadrant_padding + y_axis_space_left;
    let quadrant_top = cfg.quadrant_padding + x_axis_space_top + title_space_top;
    let quadrant_width =
        cfg.chart_width - cfg.quadrant_padding * 2.0 - y_axis_space_left - y_axis_space_right;
    let quadrant_height = cfg.chart_height
        - cfg.quadrant_padding * 2.0
        - x_axis_space_top
        - x_axis_space_bottom
        - title_space_top;
    let quadrant_half_width = quadrant_width / 2.0;
    let quadrant_half_height = quadrant_height / 2.0;

    let mut quadrants: Vec<QuadrantChartQuadrantData> = vec![
        QuadrantChartQuadrantData {
            x: quadrant_left + quadrant_half_width,
            y: quadrant_top,
            width: quadrant_half_width,
            height: quadrant_half_height,
            fill: theme.quadrant1_fill.clone(),
            text: QuadrantChartTextData {
                text: model.quadrants.quadrant1_text,
                fill: theme.quadrant1_text_fill.clone(),
                x: 0.0,
                y: 0.0,
                font_size: cfg.quadrant_label_font_size,
                vertical_pos: "center".to_string(),
                horizontal_pos: "middle".to_string(),
                rotation: 0.0,
            },
        },
        QuadrantChartQuadrantData {
            x: quadrant_left,
            y: quadrant_top,
            width: quadrant_half_width,
            height: quadrant_half_height,
            fill: theme.quadrant2_fill.clone(),
            text: QuadrantChartTextData {
                text: model.quadrants.quadrant2_text,
                fill: theme.quadrant2_text_fill.clone(),
                x: 0.0,
                y: 0.0,
                font_size: cfg.quadrant_label_font_size,
                vertical_pos: "center".to_string(),
                horizontal_pos: "middle".to_string(),
                rotation: 0.0,
            },
        },
        QuadrantChartQuadrantData {
            x: quadrant_left,
            y: quadrant_top + quadrant_half_height,
            width: quadrant_half_width,
            height: quadrant_half_height,
            fill: theme.quadrant3_fill.clone(),
            text: QuadrantChartTextData {
                text: model.quadrants.quadrant3_text,
                fill: theme.quadrant3_text_fill.clone(),
                x: 0.0,
                y: 0.0,
                font_size: cfg.quadrant_label_font_size,
                vertical_pos: "center".to_string(),
                horizontal_pos: "middle".to_string(),
                rotation: 0.0,
            },
        },
        QuadrantChartQuadrantData {
            x: quadrant_left + quadrant_half_width,
            y: quadrant_top + quadrant_half_height,
            width: quadrant_half_width,
            height: quadrant_half_height,
            fill: theme.quadrant4_fill.clone(),
            text: QuadrantChartTextData {
                text: model.quadrants.quadrant4_text,
                fill: theme.quadrant4_text_fill.clone(),
                x: 0.0,
                y: 0.0,
                font_size: cfg.quadrant_label_font_size,
                vertical_pos: "center".to_string(),
                horizontal_pos: "middle".to_string(),
                rotation: 0.0,
            },
        },
    ];
    for q in &mut quadrants {
        q.text.x = q.x + q.width / 2.0;
        if model.points.is_empty() {
            q.text.y = q.y + q.height / 2.0;
            q.text.horizontal_pos = "middle".to_string();
        } else {
            q.text.y = q.y + cfg.quadrant_text_top_padding;
            q.text.horizontal_pos = "top".to_string();
        }
    }

    let draw_x_axis_labels_in_middle = !model.axes.x_axis_right_text.trim().is_empty();
    let draw_y_axis_labels_in_middle = !model.axes.y_axis_top_text.trim().is_empty();

    let mut axis_labels: Vec<QuadrantChartAxisLabelData> = Vec::new();
    if !model.axes.x_axis_left_text.trim().is_empty() && show_x_axis {
        axis_labels.push(QuadrantChartAxisLabelData {
            text: model.axes.x_axis_left_text,
            fill: theme.quadrant_x_axis_text_fill.clone(),
            x: quadrant_left
                + if draw_x_axis_labels_in_middle {
                    quadrant_half_width / 2.0
                } else {
                    0.0
                },
            y: if x_axis_position == "top" {
                cfg.x_axis_label_padding + title_space_top
            } else {
                cfg.x_axis_label_padding + quadrant_top + quadrant_height + cfg.quadrant_padding
            },
            font_size: cfg.x_axis_label_font_size,
            vertical_pos: if draw_x_axis_labels_in_middle {
                "center".to_string()
            } else {
                "left".to_string()
            },
            horizontal_pos: "top".to_string(),
            rotation: 0.0,
        });
    }
    if !model.axes.x_axis_right_text.trim().is_empty() && show_x_axis {
        axis_labels.push(QuadrantChartAxisLabelData {
            text: model.axes.x_axis_right_text,
            fill: theme.quadrant_x_axis_text_fill.clone(),
            x: quadrant_left
                + quadrant_half_width
                + if draw_x_axis_labels_in_middle {
                    quadrant_half_width / 2.0
                } else {
                    0.0
                },
            y: if x_axis_position == "top" {
                cfg.x_axis_label_padding + title_space_top
            } else {
                cfg.x_axis_label_padding + quadrant_top + quadrant_height + cfg.quadrant_padding
            },
            font_size: cfg.x_axis_label_font_size,
            vertical_pos: if draw_x_axis_labels_in_middle {
                "center".to_string()
            } else {
                "left".to_string()
            },
            horizontal_pos: "top".to_string(),
            rotation: 0.0,
        });
    }
    if !model.axes.y_axis_bottom_text.trim().is_empty() && show_y_axis {
        axis_labels.push(QuadrantChartAxisLabelData {
            text: model.axes.y_axis_bottom_text,
            fill: theme.quadrant_y_axis_text_fill.clone(),
            x: if cfg.y_axis_position == "left" {
                cfg.y_axis_label_padding
            } else {
                cfg.y_axis_label_padding + quadrant_left + quadrant_width + cfg.quadrant_padding
            },
            y: quadrant_top + quadrant_height
                - if draw_y_axis_labels_in_middle {
                    quadrant_half_height / 2.0
                } else {
                    0.0
                },
            font_size: cfg.y_axis_label_font_size,
            vertical_pos: if draw_y_axis_labels_in_middle {
                "center".to_string()
            } else {
                "left".to_string()
            },
            horizontal_pos: "top".to_string(),
            rotation: -90.0,
        });
    }
    if !model.axes.y_axis_top_text.trim().is_empty() && show_y_axis {
        axis_labels.push(QuadrantChartAxisLabelData {
            text: model.axes.y_axis_top_text,
            fill: theme.quadrant_y_axis_text_fill.clone(),
            x: if cfg.y_axis_position == "left" {
                cfg.y_axis_label_padding
            } else {
                cfg.y_axis_label_padding + quadrant_left + quadrant_width + cfg.quadrant_padding
            },
            y: quadrant_top + quadrant_half_height
                - if draw_y_axis_labels_in_middle {
                    quadrant_half_height / 2.0
                } else {
                    0.0
                },
            font_size: cfg.y_axis_label_font_size,
            vertical_pos: if draw_y_axis_labels_in_middle {
                "center".to_string()
            } else {
                "left".to_string()
            },
            horizontal_pos: "top".to_string(),
            rotation: -90.0,
        });
    }

    let half_external_border_width = cfg.quadrant_external_border_stroke_width / 2.0;
    let border_lines = vec![
        QuadrantChartBorderLineData {
            stroke_fill: theme.quadrant_external_border_stroke_fill.clone(),
            stroke_width: cfg.quadrant_external_border_stroke_width,
            x1: quadrant_left - half_external_border_width,
            y1: quadrant_top,
            x2: quadrant_left + quadrant_width + half_external_border_width,
            y2: quadrant_top,
        },
        QuadrantChartBorderLineData {
            stroke_fill: theme.quadrant_external_border_stroke_fill.clone(),
            stroke_width: cfg.quadrant_external_border_stroke_width,
            x1: quadrant_left + quadrant_width,
            y1: quadrant_top + half_external_border_width,
            x2: quadrant_left + quadrant_width,
            y2: quadrant_top + quadrant_height - half_external_border_width,
        },
        QuadrantChartBorderLineData {
            stroke_fill: theme.quadrant_external_border_stroke_fill.clone(),
            stroke_width: cfg.quadrant_external_border_stroke_width,
            x1: quadrant_left - half_external_border_width,
            y1: quadrant_top + quadrant_height,
            x2: quadrant_left + quadrant_width + half_external_border_width,
            y2: quadrant_top + quadrant_height,
        },
        QuadrantChartBorderLineData {
            stroke_fill: theme.quadrant_external_border_stroke_fill.clone(),
            stroke_width: cfg.quadrant_external_border_stroke_width,
            x1: quadrant_left,
            y1: quadrant_top + half_external_border_width,
            x2: quadrant_left,
            y2: quadrant_top + quadrant_height - half_external_border_width,
        },
        QuadrantChartBorderLineData {
            stroke_fill: theme.quadrant_internal_border_stroke_fill.clone(),
            stroke_width: cfg.quadrant_internal_border_stroke_width,
            x1: quadrant_left + quadrant_half_width,
            y1: quadrant_top + half_external_border_width,
            x2: quadrant_left + quadrant_half_width,
            y2: quadrant_top + quadrant_height - half_external_border_width,
        },
        QuadrantChartBorderLineData {
            stroke_fill: theme.quadrant_internal_border_stroke_fill.clone(),
            stroke_width: cfg.quadrant_internal_border_stroke_width,
            x1: quadrant_left + half_external_border_width,
            y1: quadrant_top + quadrant_half_height,
            x2: quadrant_left + quadrant_width - half_external_border_width,
            y2: quadrant_top + quadrant_half_height,
        },
    ];

    let mut points: Vec<QuadrantChartPointData> = Vec::new();
    for p in model.points {
        let class_styles = p
            .class_name
            .as_deref()
            .and_then(|name| model.classes.get(name));

        let radius = p
            .styles
            .radius
            .or_else(|| class_styles.and_then(|c| c.radius))
            .unwrap_or(cfg.point_radius);
        let fill = p
            .styles
            .color
            .clone()
            .or_else(|| class_styles.and_then(|c| c.color.clone()))
            .unwrap_or_else(|| theme.quadrant_point_fill.clone());
        let stroke_color = p
            .styles
            .stroke_color
            .clone()
            .or_else(|| class_styles.and_then(|c| c.stroke_color.clone()))
            .unwrap_or_else(|| theme.quadrant_point_fill.clone());
        let stroke_width = p
            .styles
            .stroke_width
            .clone()
            .or_else(|| class_styles.and_then(|c| c.stroke_width.clone()))
            .unwrap_or_else(|| "0px".to_string());

        let x = scale_linear(
            (0.0, 1.0),
            (quadrant_left, quadrant_width + quadrant_left),
            p.x,
        );
        let y = scale_linear(
            (0.0, 1.0),
            (quadrant_height + quadrant_top, quadrant_top),
            p.y,
        );
        points.push(QuadrantChartPointData {
            x,
            y,
            fill: fill.clone(),
            radius,
            stroke_color,
            stroke_width,
            text: QuadrantChartTextData {
                text: p.text,
                fill: theme.quadrant_point_text_fill.clone(),
                x,
                y: y + cfg.point_text_padding,
                font_size: cfg.point_label_font_size,
                vertical_pos: "center".to_string(),
                horizontal_pos: "top".to_string(),
                rotation: 0.0,
            },
        });
    }

    let title = if show_title {
        Some(QuadrantChartTextData {
            text: title_text.to_string(),
            fill: theme.quadrant_title_fill,
            font_size: cfg.title_font_size,
            horizontal_pos: "top".to_string(),
            vertical_pos: "center".to_string(),
            rotation: 0.0,
            y: cfg.title_padding,
            x: cfg.chart_width / 2.0,
        })
    } else {
        None
    };

    Ok(QuadrantChartDiagramLayout {
        width: cfg.chart_width,
        height: cfg.chart_height,
        title,
        quadrants,
        border_lines,
        points,
        axis_labels,
    })
}

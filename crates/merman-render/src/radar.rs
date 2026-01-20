use crate::Result;
use crate::model::{
    Bounds, LayoutPoint, RadarAxisLayout, RadarCurveLayout, RadarDiagramLayout,
    RadarGraticuleShapeLayout, RadarLegendItemLayout,
};
use crate::text::TextMeasurer;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct RadarAxis {
    name: String,
    label: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RadarCurve {
    name: String,
    label: String,
    entries: Vec<f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct RadarOptions {
    #[serde(rename = "showLegend")]
    show_legend: bool,
    ticks: i64,
    min: f64,
    max: Option<f64>,
    graticule: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RadarModel {
    #[serde(rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    acc_descr: Option<String>,
    title: Option<String>,
    axes: Vec<RadarAxis>,
    curves: Vec<RadarCurve>,
    options: RadarOptions,
}

fn config_f64(cfg: &serde_json::Value, path: &[&str], default: f64) -> f64 {
    let mut cur = cfg;
    for key in path {
        cur = match cur.get(*key) {
            Some(v) => v,
            None => return default,
        };
    }
    cur.as_f64()
        .or_else(|| cur.as_i64().map(|n| n as f64))
        .or_else(|| cur.as_u64().map(|n| n as f64))
        .unwrap_or(default)
}

fn fmt_number(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    let mut r = (v * 1000.0).round() / 1000.0;
    if r.abs() < 0.0005 {
        r = 0.0;
    }
    let mut s = format!("{r:.3}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" { "0".to_string() } else { s }
}

fn polar_xy(radius: f64, angle: f64) -> LayoutPoint {
    LayoutPoint {
        x: radius * angle.cos(),
        y: radius * angle.sin(),
    }
}

fn closed_cardinal_path(points: &[LayoutPoint], tension: f64) -> String {
    if points.is_empty() {
        return String::new();
    }
    if points.len() == 1 {
        let p = points[0].clone();
        return format!("M{},{}Z", fmt_number(p.x), fmt_number(p.y));
    }

    let k = ((1.0 - tension).clamp(0.0, 1.0)) / 6.0;
    let mut out = String::new();
    let p0 = points[0].clone();
    out.push_str(&format!("M{},{}", fmt_number(p0.x), fmt_number(p0.y)));

    let n = points.len();
    for i in 0..n {
        let p_prev = points[(i + n - 1) % n].clone();
        let p = points[i].clone();
        let p_next = points[(i + 1) % n].clone();
        let p_next2 = points[(i + 2) % n].clone();

        let cp1 = LayoutPoint {
            x: p.x + k * (p_next.x - p_prev.x),
            y: p.y + k * (p_next.y - p_prev.y),
        };
        let cp2 = LayoutPoint {
            x: p_next.x - k * (p_next2.x - p.x),
            y: p_next.y - k * (p_next2.y - p.y),
        };

        out.push_str(&format!(
            " C{},{} {},{} {},{}",
            fmt_number(cp1.x),
            fmt_number(cp1.y),
            fmt_number(cp2.x),
            fmt_number(cp2.y),
            fmt_number(p_next.x),
            fmt_number(p_next.y)
        ));
    }
    out.push_str(" Z");
    out
}

pub fn layout_radar_diagram(
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    _measurer: &dyn TextMeasurer,
) -> Result<RadarDiagramLayout> {
    let model: RadarModel = serde_json::from_value(semantic.clone())?;
    let _ = (
        model.acc_title.as_deref(),
        model.acc_descr.as_deref(),
        model.title.as_deref(),
    );

    let cfg = effective_config;
    let width = config_f64(cfg, &["radar", "width"], 600.0);
    let height = config_f64(cfg, &["radar", "height"], 600.0);
    let margin_left = config_f64(cfg, &["radar", "marginLeft"], 50.0);
    let margin_right = config_f64(cfg, &["radar", "marginRight"], 50.0);
    let margin_top = config_f64(cfg, &["radar", "marginTop"], 50.0);
    let axis_scale_factor = config_f64(cfg, &["radar", "axisScaleFactor"], 1.0);
    let axis_label_factor = config_f64(cfg, &["radar", "axisLabelFactor"], 1.05);
    let curve_tension = config_f64(cfg, &["radar", "curveTension"], 0.17);

    // Mermaid radar uses a square SVG sized from the configured width and horizontal margins.
    let svg_width = width + margin_left + margin_right;
    let svg_height = svg_width;

    let center_x = margin_left + width / 2.0;
    let center_y = margin_top + height / 2.0;
    let radius = (width / 2.0) * axis_scale_factor;

    let title_y = -center_y;

    let axis_count = model.axes.len();
    let mut axes: Vec<RadarAxisLayout> = Vec::new();
    if axis_count > 0 {
        for (i, axis) in model.axes.iter().enumerate() {
            let angle = -std::f64::consts::FRAC_PI_2
                + (i as f64) * (std::f64::consts::TAU / (axis_count as f64));
            let line = polar_xy(radius, angle);
            let label = polar_xy(radius * axis_label_factor, angle);
            axes.push(RadarAxisLayout {
                label: axis.label.clone(),
                angle,
                line_x2: line.x,
                line_y2: line.y,
                label_x: label.x,
                label_y: label.y,
            });
        }
    }

    let ticks = model.options.ticks.max(0);
    let mut graticules: Vec<RadarGraticuleShapeLayout> = Vec::new();
    if ticks > 0 {
        for t in 1..=ticks {
            let r = radius * (t as f64) / (ticks as f64);
            if model.options.graticule.trim() == "polygon" {
                let points = if axis_count == 0 {
                    Vec::new()
                } else {
                    (0..axis_count)
                        .map(|i| {
                            let angle = -std::f64::consts::FRAC_PI_2
                                + (i as f64) * (std::f64::consts::TAU / (axis_count as f64));
                            polar_xy(r, angle)
                        })
                        .collect()
                };
                graticules.push(RadarGraticuleShapeLayout {
                    kind: "polygon".to_string(),
                    r: None,
                    points,
                });
            } else {
                graticules.push(RadarGraticuleShapeLayout {
                    kind: "circle".to_string(),
                    r: Some(r),
                    points: Vec::new(),
                });
            }
        }
    }

    let mut inferred_max: f64 = 0.0;
    for c in &model.curves {
        for v in &c.entries {
            if v.is_finite() {
                inferred_max = inferred_max.max(*v);
            }
        }
    }
    let max_value = model.options.max.unwrap_or(inferred_max);
    let min_value = model.options.min;
    let denom = (max_value - min_value).abs().max(1e-9);

    let mut curves: Vec<RadarCurveLayout> = Vec::new();
    for (curve_idx, curve) in model.curves.iter().enumerate() {
        let mut points: Vec<LayoutPoint> = Vec::new();
        if axis_count > 0 {
            for i in 0..axis_count {
                let angle = -std::f64::consts::FRAC_PI_2
                    + (i as f64) * (std::f64::consts::TAU / (axis_count as f64));
                let v = curve.entries.get(i).copied().unwrap_or(min_value);
                let frac = ((v - min_value) / denom).clamp(0.0, 1.0);
                points.push(polar_xy(radius * frac, angle));
            }
        }
        let path_d = closed_cardinal_path(&points, curve_tension);
        curves.push(RadarCurveLayout {
            label: curve.label.clone(),
            class_index: curve_idx as i64,
            points,
            path_d,
        });
    }

    let mut legend_items: Vec<RadarLegendItemLayout> = Vec::new();
    if model.options.show_legend && !curves.is_empty() {
        let base_x = radius * 0.875;
        let base_y = -radius * 0.875;
        let step_y = 22.5;
        for (i, c) in model.curves.iter().enumerate() {
            legend_items.push(RadarLegendItemLayout {
                label: c.label.clone(),
                class_index: i as i64,
                x: base_x,
                y: base_y + (i as f64) * step_y,
            });
        }
    }

    Ok(RadarDiagramLayout {
        bounds: Some(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: svg_width,
            max_y: svg_height,
        }),
        svg_width,
        svg_height,
        center_x,
        center_y,
        radius,
        axis_label_factor,
        title_y,
        axes,
        graticules,
        curves,
        legend_items,
    })
}

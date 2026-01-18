use crate::Result;
use crate::model::{Bounds, PieDiagramLayout, PieLegendItemLayout, PieSliceLayout};
use crate::text::{TextMeasurer, TextStyle};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct PieSection {
    label: String,
    value: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct PieModel {
    #[serde(rename = "showData")]
    show_data: bool,
    title: Option<String>,
    #[serde(rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    acc_descr: Option<String>,
    sections: Vec<PieSection>,
}

#[derive(Debug, Clone)]
struct ColorScale {
    palette: Vec<String>,
    mapping: std::collections::HashMap<String, usize>,
    next: usize,
}

impl ColorScale {
    fn new_default() -> Self {
        // Default theme colors as emitted by Mermaid 11.12.2 in SVG.
        Self {
            palette: vec![
                "#ECECFF".to_string(),
                "#ffffde".to_string(),
                "hsl(80, 100%, 56.2745098039%)".to_string(),
            ],
            mapping: std::collections::HashMap::new(),
            next: 0,
        }
    }

    fn color_for(&mut self, label: &str) -> String {
        if let Some(idx) = self.mapping.get(label).copied() {
            return self.palette[idx % self.palette.len()].clone();
        }
        let idx = self.next;
        self.next += 1;
        self.mapping.insert(label.to_string(), idx);
        self.palette[idx % self.palette.len()].clone()
    }
}

fn polar_xy(radius: f64, angle: f64) -> (f64, f64) {
    // Mermaid pie charts use a "12 o'clock is zero" convention with y increasing downwards.
    let x = radius * angle.sin();
    let y = -radius * angle.cos();
    (x, y)
}

fn fmt_number(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    if v.abs() < 0.0005 {
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

pub fn layout_pie_diagram(
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    measurer: &dyn TextMeasurer,
) -> Result<PieDiagramLayout> {
    let model: PieModel = serde_json::from_value(semantic.clone())?;
    let _ = (model.acc_title.as_deref(), model.acc_descr.as_deref());

    let center_x: f64 = 225.0;
    let center_y: f64 = 225.0;
    let radius: f64 = 185.0;
    let outer_radius: f64 = 186.0;
    let label_radius: f64 = radius * 0.75;
    let legend_x: f64 = 216.0;
    let legend_step_y: f64 = 22.0;
    let legend_start_y: f64 = -11.0 * (model.sections.len().max(1) as f64);

    let positive_sections: Vec<&PieSection> = model
        .sections
        .iter()
        .filter(|s| s.value.is_finite() && s.value > 0.0)
        .collect();
    let total: f64 = positive_sections.iter().map(|s| s.value).sum();

    let mut color_scale = ColorScale::new_default();

    let mut slices: Vec<PieSliceLayout> = Vec::new();
    if !positive_sections.is_empty() && total.is_finite() && total > 0.0 {
        if positive_sections.len() == 1 {
            let s = positive_sections[0];
            let fill = color_scale.color_for(&s.label);
            let (tx, ty) = polar_xy(label_radius, std::f64::consts::PI);
            slices.push(PieSliceLayout {
                label: s.label.clone(),
                value: s.value,
                start_angle: 0.0,
                end_angle: std::f64::consts::TAU,
                is_full_circle: true,
                percent: 100,
                text_x: tx,
                text_y: ty,
                fill,
            });
        } else {
            let mut start = 0.0;
            for s in positive_sections {
                let frac = (s.value / total).max(0.0);
                let delta = frac * std::f64::consts::TAU;
                let end = start + delta;
                let mid = (start + end) / 2.0;
                let (tx, ty) = polar_xy(label_radius, mid);
                let fill = color_scale.color_for(&s.label);
                let percent = (100.0 * frac).round() as i64;
                slices.push(PieSliceLayout {
                    label: s.label.clone(),
                    value: s.value,
                    start_angle: start,
                    end_angle: end,
                    is_full_circle: false,
                    percent,
                    text_x: tx,
                    text_y: ty,
                    fill,
                });
                start = end;
            }
        }
    }

    // Lock the color scale domain based on the drawn slices first, then compute legend colors in
    // the original section order (this matches Mermaid's zero-slice behavior).
    let mut legend_items: Vec<PieLegendItemLayout> = Vec::new();
    for (i, sec) in model.sections.iter().enumerate() {
        let y = legend_start_y + (i as f64) * legend_step_y;
        let fill = color_scale.color_for(&sec.label);
        legend_items.push(PieLegendItemLayout {
            label: sec.label.clone(),
            value: sec.value,
            fill,
            y,
        });
    }

    let legend_style = TextStyle {
        font_family: None,
        font_size: 17.0,
        font_weight: None,
    };
    let mut max_legend_width: f64 = 0.0;
    for sec in &model.sections {
        let label = if model.show_data {
            format!("{} [{}]", sec.label, fmt_number(sec.value))
        } else {
            sec.label.clone()
        };
        let metrics = measurer.measure(&label, &legend_style);
        max_legend_width = max_legend_width.max(metrics.width);
    }

    let base_w: f64 = center_x * 2.0;
    let legend_right = center_x + legend_x + 22.0 + max_legend_width + 50.0;
    let width: f64 = base_w.max(legend_right).max(1.0);
    let height: f64 = f64::max(center_y * 2.0, 1.0);

    Ok(PieDiagramLayout {
        bounds: Some(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: width,
            max_y: height,
        }),
        center_x,
        center_y,
        radius,
        outer_radius,
        legend_x,
        legend_start_y,
        legend_step_y,
        slices,
        legend_items,
    })
}

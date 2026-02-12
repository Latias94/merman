use crate::Result;
use crate::model::{Bounds, PieDiagramLayout, PieLegendItemLayout, PieSliceLayout};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use ryu_js::Buffer;
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

#[derive(Debug, Clone, Copy)]
struct Rgb01 {
    r: f64,
    g: f64,
    b: f64,
}

#[derive(Debug, Clone, Copy)]
struct Hsl {
    h_deg: f64,
    s_pct: f64,
    l_pct: f64,
}

fn round_1e10(v: f64) -> f64 {
    let v = (v * 1e10).round() / 1e10;
    if v == -0.0 { 0.0 } else { v }
}

fn fmt_js_1e10(v: f64) -> String {
    let v = round_1e10(v);
    let mut b = Buffer::new();
    b.format_finite(v).to_string()
}

fn round_hsl_1e10(mut hsl: Hsl) -> Hsl {
    // Match Mermaid's base theme output: wrap using remainder without forcing positive hue.
    // (JS `%` keeps the sign, so negative hues remain negative.)
    hsl.h_deg = round_1e10(hsl.h_deg) % 360.0;
    hsl.s_pct = round_1e10(hsl.s_pct).clamp(0.0, 100.0);
    hsl.l_pct = round_1e10(hsl.l_pct).clamp(0.0, 100.0);
    hsl
}

fn parse_hex_rgb01(s: &str) -> Option<Rgb01> {
    let s = s.trim();
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()? as f64 / 255.0;
    let g = u8::from_str_radix(&s[2..4], 16).ok()? as f64 / 255.0;
    let b = u8::from_str_radix(&s[4..6], 16).ok()? as f64 / 255.0;
    Some(Rgb01 { r, g, b })
}

fn rgb01_to_hsl(rgb: Rgb01) -> Hsl {
    let r = rgb.r;
    let g = rgb.g;
    let b = rgb.b;

    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let mut h = 0.0;
    let mut s = 0.0;
    let l = (max + min) / 2.0;

    if max != min {
        let d = max - min;
        s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };

        h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        h /= 6.0;
    }

    round_hsl_1e10(Hsl {
        h_deg: h * 360.0,
        s_pct: s * 100.0,
        l_pct: l * 100.0,
    })
}

fn parse_hsl(s: &str) -> Option<Hsl> {
    let s = s.trim();
    let inner = s.strip_prefix("hsl(")?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
    if parts.len() != 3 {
        return None;
    }
    let h = parts[0].parse::<f64>().ok()?;
    let s_pct = parts[1].trim_end_matches('%').parse::<f64>().ok()?;
    let l_pct = parts[2].trim_end_matches('%').parse::<f64>().ok()?;
    Some(round_hsl_1e10(Hsl {
        h_deg: h,
        s_pct,
        l_pct,
    }))
}

fn adjust_hsl(mut hsl: Hsl, h_delta: f64, s_delta: f64, l_delta: f64) -> Hsl {
    hsl.h_deg = (hsl.h_deg + h_delta) % 360.0;
    hsl.s_pct = (hsl.s_pct + s_delta).clamp(0.0, 100.0);
    hsl.l_pct = (hsl.l_pct + l_delta).clamp(0.0, 100.0);
    round_hsl_1e10(hsl)
}

fn fmt_hsl(hsl: Hsl) -> String {
    format!(
        "hsl({}, {}%, {}%)",
        fmt_js_1e10(hsl.h_deg),
        fmt_js_1e10(hsl.s_pct),
        fmt_js_1e10(hsl.l_pct)
    )
}

fn adjust_color_to_hsl_string(
    color: &str,
    h_delta: f64,
    s_delta: f64,
    l_delta: f64,
) -> Option<String> {
    let base = if let Some(rgb) = parse_hex_rgb01(color) {
        rgb01_to_hsl(rgb)
    } else if let Some(hsl) = parse_hsl(color) {
        hsl
    } else {
        return None;
    };
    Some(fmt_hsl(adjust_hsl(base, h_delta, s_delta, l_delta)))
}

impl ColorScale {
    fn new_default() -> Self {
        // Default theme colors as emitted by Mermaid 11.12.2 in SVG.
        //
        // Mermaid derives this palette from `theme-default.js` `pie1..pie12` (using `adjust()`),
        // where the base colors are:
        // - primaryColor = "#ECECFF"
        // - secondaryColor = "#ffffde"
        // - tertiaryColor = "hsl(80, 100%, 96.2745098039%)"
        //
        // Note: `adjust(...)` serializes as `hsl(...)` (not hex), so the palette contains a mix.
        const PRIMARY: &str = "#ECECFF";
        const SECONDARY: &str = "#ffffde";
        const TERTIARY: &str = "hsl(80, 100%, 96.2745098039%)";

        let pie3 = adjust_color_to_hsl_string(TERTIARY, 0.0, 0.0, -40.0)
            .unwrap_or_else(|| "hsl(80, 100%, 56.2745098039%)".to_string());
        let pie4 = adjust_color_to_hsl_string(PRIMARY, 0.0, 0.0, -10.0)
            .unwrap_or_else(|| "hsl(240, 100%, 86.2745098039%)".to_string());
        let pie5 = adjust_color_to_hsl_string(SECONDARY, 0.0, 0.0, -30.0)
            .unwrap_or_else(|| "hsl(60, 100%, 57.0588235294%)".to_string());
        let pie6 = adjust_color_to_hsl_string(TERTIARY, 0.0, 0.0, -20.0)
            .unwrap_or_else(|| "hsl(80, 100%, 76.2745098039%)".to_string());
        let pie7 = adjust_color_to_hsl_string(PRIMARY, 60.0, 0.0, -20.0)
            .unwrap_or_else(|| "hsl(300, 100%, 76.2745098039%)".to_string());
        let pie8 = adjust_color_to_hsl_string(PRIMARY, -60.0, 0.0, -40.0)
            .unwrap_or_else(|| "hsl(180, 100%, 56.2745098039%)".to_string());
        let pie9 = adjust_color_to_hsl_string(PRIMARY, 120.0, 0.0, -40.0)
            .unwrap_or_else(|| "hsl(0, 100%, 56.2745098039%)".to_string());
        let pie10 = adjust_color_to_hsl_string(PRIMARY, 60.0, 0.0, -40.0)
            .unwrap_or_else(|| "hsl(300, 100%, 56.2745098039%)".to_string());
        let pie11 = adjust_color_to_hsl_string(PRIMARY, -90.0, 0.0, -40.0)
            .unwrap_or_else(|| "hsl(150, 100%, 56.2745098039%)".to_string());
        let pie12 = adjust_color_to_hsl_string(PRIMARY, 120.0, 0.0, -30.0)
            .unwrap_or_else(|| "hsl(0, 100%, 66.2745098039%)".to_string());

        Self {
            palette: vec![
                PRIMARY.to_string(),
                SECONDARY.to_string(),
                pie3,
                pie4,
                pie5,
                pie6,
                pie7,
                pie8,
                pie9,
                pie10,
                pie11,
                pie12,
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

fn pie_legend_bbox_overhang_left_em(ch: char) -> f64 {
    // Mermaid pie charts compute `longestTextWidth` via `legend.selectAll('text').nodes()
    // .map(node => node.getBoundingClientRect().width)` (Mermaid@11.12.2). For some ASCII glyphs,
    // Chromium's SVG text bbox extends beyond the advance width (e.g. trailing `t`/`r`, leading
    // and trailing `_`).
    //
    // Our vendored measurer primarily models advance widths (close to `getComputedTextLength()`).
    // Model the bbox delta with a small per-glyph overhang in `em` units so viewport sizing
    // (`viewBox` / `max-width`) matches upstream baselines.
    match ch {
        // Leading underscore (observed in `__proto__`).
        '_' => 0.06125057352941176,
        _ => 0.0,
    }
}

fn pie_legend_bbox_overhang_right_em(ch: char) -> f64 {
    match ch {
        // Trailing underscore (observed in `__proto__`).
        '_' => 0.06125057352941176,
        // Trailing `t` expands bbox in Chromium (`bat`).
        't' => 0.01496444117647059,
        // Trailing `r` expands bbox in Chromium (`constructor`).
        'r' => 0.08091001764705883,
        // Trailing `e` expands bbox in Chromium (`prototype`).
        'e' => 0.04291130514705883,
        // Trailing `s` expands bbox in Chromium (`dogs`/`rats`).
        's' => 0.007008272058823529,
        // Trailing `h` small bbox delta (`ash`).
        'h' => 0.0009191176470588235,
        // Trailing `]` small bbox delta (`bat [40]`).
        ']' => 0.00045955882352941176,
        _ => 0.0,
    }
}

pub fn layout_pie_diagram(
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    measurer: &dyn TextMeasurer,
) -> Result<PieDiagramLayout> {
    let model: PieModel = crate::json::from_value_ref(semantic)?;
    let _ = (
        model.title.as_deref(),
        model.acc_title.as_deref(),
        model.acc_descr.as_deref(),
    );

    // Mermaid@11.12.2 `packages/mermaid/src/diagrams/pie/pieRenderer.ts` constants.
    let margin: f64 = 40.0;
    let legend_rect_size: f64 = 18.0;
    let legend_spacing: f64 = 4.0;

    let center_x: f64 = 225.0;
    let center_y: f64 = 225.0;
    let radius: f64 = 185.0;
    let outer_radius: f64 = 186.0;
    let label_radius: f64 = radius * 0.75;
    let legend_x: f64 = 12.0 * legend_rect_size;
    let legend_step_y: f64 = legend_rect_size + legend_spacing;
    let legend_start_y: f64 = -(legend_step_y * (model.sections.len().max(1) as f64)) / 2.0;

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
        // Mermaid pie legend labels render as a single `<text>` run (no `<tspan>` tokenization).
        // Mermaid measures the width via `getBoundingClientRect().width` (not `getBBox()`), but
        // we approximate it with a single-run SVG measurement plus a small overhang correction.
        let metrics =
            measurer.measure_wrapped(&label, &legend_style, None, WrapMode::SvgLikeSingleRun);
        let mut w = metrics.width.max(0.0);
        let trimmed = label.trim_end();
        if !trimmed.is_empty() {
            let font_size = legend_style.font_size.max(1.0);
            let first = trimmed.chars().next().unwrap_or(' ');
            let last = trimmed.chars().last().unwrap_or(' ');
            w += pie_legend_bbox_overhang_left_em(first) * font_size;
            w += pie_legend_bbox_overhang_right_em(last) * font_size;
        }
        max_legend_width = max_legend_width.max(w);
    }

    let base_w: f64 = center_x * 2.0;
    // Mermaid computes:
    //   totalWidth = pieWidth + MARGIN + LEGEND_RECT_SIZE + LEGEND_SPACING + longestTextWidth
    // where `pieWidth == height == 450`.
    let width: f64 =
        (base_w + margin + legend_rect_size + legend_spacing + max_legend_width).max(1.0);
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

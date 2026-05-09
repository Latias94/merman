//! Shared RoughJS formatting helpers used by multiple parity renderers.

use std::fmt::Write as _;

pub(in crate::svg::parity) fn parse_hex_color_to_srgba(s: &str) -> Option<roughr::Srgba> {
    let s = s.trim();
    let hex = s.strip_prefix('#')?;
    let (r, g, b) = match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b)
        }
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            (r, g, b)
        }
        _ => return None,
    };
    Some(roughr::Srgba::new(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        1.0,
    ))
}

pub(in crate::svg::parity) fn ops_to_svg_path_d(opset: &roughr::core::OpSet<f64>) -> String {
    let mut out = String::new();
    for op in &opset.ops {
        match op.op {
            roughr::core::OpType::Move => {
                let _ = write!(&mut out, "M{} {} ", op.data[0], op.data[1]);
            }
            roughr::core::OpType::BCurveTo => {
                let _ = write!(
                    &mut out,
                    "C{} {}, {} {}, {} {} ",
                    op.data[0], op.data[1], op.data[2], op.data[3], op.data[4], op.data[5]
                );
            }
            roughr::core::OpType::LineTo => {
                let _ = write!(&mut out, "L{} {} ", op.data[0], op.data[1]);
            }
        }
    }
    out.trim_end().to_string()
}

pub(in crate::svg::parity) struct RoughRectSpec<'a> {
    pub(in crate::svg::parity) x: f64,
    pub(in crate::svg::parity) y: f64,
    pub(in crate::svg::parity) w: f64,
    pub(in crate::svg::parity) h: f64,
    pub(in crate::svg::parity) fill: &'a str,
    pub(in crate::svg::parity) stroke: &'a str,
    pub(in crate::svg::parity) stroke_width: f32,
    pub(in crate::svg::parity) seed: u64,
}

pub(in crate::svg::parity) fn roughjs_paths_for_rect(
    spec: RoughRectSpec<'_>,
) -> Option<(String, String)> {
    let RoughRectSpec {
        x,
        y,
        w,
        h,
        fill,
        stroke,
        stroke_width,
        seed,
    } = spec;

    let fill = parse_hex_color_to_srgba(fill)?;
    let stroke = parse_hex_color_to_srgba(stroke)?;
    let mut opts = roughr::core::OptionsBuilder::default()
        .seed(seed)
        .roughness(0.0)
        .fill_style(roughr::core::FillStyle::Solid)
        .fill(fill)
        .stroke(stroke)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![0.0, 0.0])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    let fill_poly = vec![vec![
        roughr::Point2D::new(x, y),
        roughr::Point2D::new(x + w, y),
        roughr::Point2D::new(x + w, y + h),
        roughr::Point2D::new(x, y + h),
    ]];
    // Rough.js computes the rectangle outline first (advancing the PRNG state), then the fill, and
    // finally emits the fill path before the stroke path. Keep the same generation order to match
    // Mermaid's seeded output.
    let stroke_opset = roughr::renderer::rectangle::<f64>(x, y, w, h, &mut opts);
    let fill_opset = roughr::renderer::solid_fill_polygon(&fill_poly, &mut opts);

    Some((
        ops_to_svg_path_d(&fill_opset),
        ops_to_svg_path_d(&stroke_opset),
    ))
}

pub(in crate::svg::parity) fn roughjs_circle_path_d(diameter: f64, seed: u64) -> Option<String> {
    let mut opts = roughr::core::OptionsBuilder::default()
        .seed(seed)
        .roughness(0.0)
        .fill_style(roughr::core::FillStyle::Solid)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;
    let opset = roughr::renderer::ellipse::<f64>(0.0, 0.0, diameter, diameter, &mut opts);
    Some(ops_to_svg_path_d(&opset))
}

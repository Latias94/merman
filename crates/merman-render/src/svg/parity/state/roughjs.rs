//! Rough.js helpers used by multiple parity renderers.

use std::fmt::Write as _;
use svgtypes::{PathParser, PathSegment};

pub(in crate::svg::parity) fn roughjs_parse_hex_color_to_srgba(s: &str) -> Option<roughr::Srgba> {
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

pub(in crate::svg::parity) fn roughjs_ops_to_svg_path_d(
    opset: &roughr::core::OpSet<f64>,
) -> String {
    let mut out = String::with_capacity(opset.ops.len() * 24);
    for (idx, op) in opset.ops.iter().enumerate() {
        if idx != 0 {
            out.push(' ');
        }
        match op.op {
            roughr::core::OpType::Move => {
                let _ = write!(&mut out, "M{} {}", op.data[0], op.data[1]);
            }
            roughr::core::OpType::BCurveTo => {
                let _ = write!(
                    &mut out,
                    "C{} {}, {} {}, {} {}",
                    op.data[0], op.data[1], op.data[2], op.data[3], op.data[4], op.data[5]
                );
            }
            roughr::core::OpType::LineTo => {
                let _ = write!(&mut out, "L{} {}", op.data[0], op.data[1]);
            }
        }
    }
    out
}

fn mermaid_create_path_from_points(points: &[(f64, f64)]) -> String {
    let mut out = String::new();
    for (i, (x, y)) in points.iter().copied().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        let _ = write!(&mut out, "{cmd}{x},{y} ");
    }
    out.push('Z');
    out.trim_end().to_string()
}

fn mermaid_generate_arc_points(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    rx: f64,
    ry: f64,
    clockwise: bool,
) -> Vec<(f64, f64)> {
    let num_points: usize = 20;

    let mid_x = (x1 + x2) / 2.0;
    let mid_y = (y1 + y2) / 2.0;
    let angle = (y2 - y1).atan2(x2 - x1);

    let dx = (x2 - x1) / 2.0;
    let dy = (y2 - y1) / 2.0;
    let transformed_x = dx / rx;
    let transformed_y = dy / ry;
    let distance = (transformed_x * transformed_x + transformed_y * transformed_y).sqrt();
    if distance > 1.0 {
        return vec![(x1, y1), (x2, y2)];
    }

    let scaled_center_distance = (1.0 - distance * distance).sqrt();
    let sign = if clockwise { -1.0 } else { 1.0 };
    let center_x = mid_x + scaled_center_distance * ry * angle.sin() * sign;
    let center_y = mid_y - scaled_center_distance * rx * angle.cos() * sign;

    let start_angle = ((y1 - center_y) / ry).atan2((x1 - center_x) / rx);
    let end_angle = ((y2 - center_y) / ry).atan2((x2 - center_x) / rx);

    let mut angle_range = end_angle - start_angle;
    if clockwise && angle_range < 0.0 {
        angle_range += 2.0 * std::f64::consts::PI;
    }
    if !clockwise && angle_range > 0.0 {
        angle_range -= 2.0 * std::f64::consts::PI;
    }

    let mut points: Vec<(f64, f64)> = Vec::with_capacity(num_points);
    for i in 0..num_points {
        let t = i as f64 / (num_points - 1) as f64;
        let a = start_angle + t * angle_range;
        let x = center_x + rx * a.cos();
        let y = center_y + ry * a.sin();
        points.push((x, y));
    }
    points
}

pub(super) fn mermaid_rounded_rect_path_data(w: f64, h: f64) -> String {
    let radius = 5.0;
    let taper = 5.0;

    let mut points: Vec<(f64, f64)> = Vec::new();

    points.push((-w / 2.0 + taper, -h / 2.0));
    points.push((w / 2.0 - taper, -h / 2.0));
    points.extend(mermaid_generate_arc_points(
        w / 2.0 - taper,
        -h / 2.0,
        w / 2.0,
        -h / 2.0 + taper,
        radius,
        radius,
        true,
    ));

    points.push((w / 2.0, -h / 2.0 + taper));
    points.push((w / 2.0, h / 2.0 - taper));
    points.extend(mermaid_generate_arc_points(
        w / 2.0,
        h / 2.0 - taper,
        w / 2.0 - taper,
        h / 2.0,
        radius,
        radius,
        true,
    ));

    points.push((w / 2.0 - taper, h / 2.0));
    points.push((-w / 2.0 + taper, h / 2.0));
    points.extend(mermaid_generate_arc_points(
        -w / 2.0 + taper,
        h / 2.0,
        -w / 2.0,
        h / 2.0 - taper,
        radius,
        radius,
        true,
    ));

    points.push((-w / 2.0, h / 2.0 - taper));
    points.push((-w / 2.0, -h / 2.0 + taper));
    points.extend(mermaid_generate_arc_points(
        -w / 2.0,
        -h / 2.0 + taper,
        -w / 2.0 + taper,
        -h / 2.0,
        radius,
        radius,
        true,
    ));

    mermaid_create_path_from_points(&points)
}

pub(super) fn mermaid_choice_diamond_path_data(w: f64, h: f64) -> String {
    let points: Vec<(f64, f64)> = vec![
        (0.0, h / 2.0),
        (w / 2.0, 0.0),
        (0.0, -h / 2.0),
        (-w / 2.0, 0.0),
    ];
    mermaid_create_path_from_points(&points)
}

pub(super) fn roughjs_paths_for_svg_path(
    svg_path_data: &str,
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    seed: u64,
) -> Option<(String, String)> {
    let fill = roughjs_parse_hex_color_to_srgba(fill)?;
    let stroke = roughjs_parse_hex_color_to_srgba(stroke)?;

    let mut dash0: Option<f32> = None;
    let mut dash1: Option<f32> = None;
    for t in stroke_dasharray
        .trim()
        .split(|ch: char| ch == ',' || ch.is_whitespace())
    {
        if t.is_empty() {
            continue;
        }
        let Ok(v) = t.parse::<f32>() else {
            continue;
        };
        if dash0.is_none() {
            dash0 = Some(v);
        } else {
            dash1 = Some(v);
            break;
        }
    }
    let (dash0, dash1) = match (dash0, dash1) {
        (Some(a), Some(b)) => (a, b),
        (Some(a), None) => (a, a),
        _ => (0.0, 0.0),
    };

    let path_segments: Vec<PathSegment> = PathParser::from(svg_path_data).flatten().collect();
    let normalized_segments = roughr::points_on_path::normalized_segments(&path_segments);

    // Use a single mutable `Options` to match Rough.js behavior: the PRNG state (`randomizer`)
    // lives on the options object and advances across drawing phases.
    let mut options = roughr::core::OptionsBuilder::default()
        .seed(seed)
        .roughness(0.0)
        .fill_style(roughr::core::FillStyle::Solid)
        .fill(fill)
        .stroke(stroke)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0 as f64, dash1 as f64])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    let base_roughness = options.roughness.unwrap_or(1.0);
    let move_to_count = normalized_segments
        .iter()
        .filter(|seg| matches!(seg, PathSegment::MoveTo { abs: true, .. }))
        .count();
    let single_set = move_to_count <= 1;

    let fill_opset = if single_set {
        // Rough.js uses a different setting profile for solid fill on paths.
        options.disable_multi_stroke = Some(true);
        options.disable_multi_stroke_fill = Some(true);
        options.roughness = Some(if base_roughness != 0.0 {
            base_roughness + 0.8
        } else {
            0.0
        });

        let mut opset =
            roughr::renderer::svg_normalized_segments::<f64>(&normalized_segments, &mut options);
        let mut idx = 0usize;
        opset.ops.retain(|op| {
            let keep = idx == 0 || op.op != roughr::core::OpType::Move;
            idx += 1;
            keep
        });
        opset
    } else {
        let distance = (1.0 + base_roughness as f64) / 2.0;
        let sets = roughr::points_on_path::points_on_normalized_segments::<f64>(
            &normalized_segments,
            Some(1.0),
            Some(distance),
        );
        options.disable_multi_stroke = Some(true);
        options.disable_multi_stroke_fill = Some(true);
        roughr::renderer::solid_fill_polygon(&sets, &mut options)
    };

    // Restore stroke settings and render the outline *after* fill so the PRNG stream matches.
    options.disable_multi_stroke = Some(false);
    options.disable_multi_stroke_fill = Some(false);
    options.roughness = Some(base_roughness);
    let stroke_opset =
        roughr::renderer::svg_normalized_segments::<f64>(&normalized_segments, &mut options);

    Some((
        roughjs_ops_to_svg_path_d(&fill_opset),
        roughjs_ops_to_svg_path_d(&stroke_opset),
    ))
}

pub(in crate::svg::parity) fn roughjs_paths_for_rect(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    seed: u64,
) -> Option<(String, String)> {
    let fill = roughjs_parse_hex_color_to_srgba(fill)?;
    let stroke = roughjs_parse_hex_color_to_srgba(stroke)?;

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
        roughjs_ops_to_svg_path_d(&fill_opset),
        roughjs_ops_to_svg_path_d(&stroke_opset),
    ))
}

pub(super) fn roughjs_circle_path_d(diameter: f64, seed: u64) -> Option<String> {
    let mut opts = roughr::core::OptionsBuilder::default()
        .seed(seed)
        .roughness(0.0)
        .fill_style(roughr::core::FillStyle::Solid)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;
    let opset = roughr::renderer::ellipse::<f64>(0.0, 0.0, diameter, diameter, &mut opts);
    Some(roughjs_ops_to_svg_path_d(&opset))
}

use std::fmt::Write as _;

use super::{controlled, ops_to_svg_path_d};

pub fn closed_path_d_from_points(points: &[(f64, f64)]) -> String {
    let mut out = String::new();
    for (i, (x, y)) in points.iter().copied().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        let _ = write!(&mut out, "{cmd}{x},{y} ");
    }
    out.push('Z');
    out
}

pub fn mermaid_arc_points(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    rx: f64,
    ry: f64,
    clockwise: bool,
) -> Vec<(f64, f64)> {
    // Port of Mermaid `generateArcPoints(...)` in
    // `packages/mermaid/src/rendering-util/rendering-elements/shapes/roundedRect.ts`.
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

pub struct RoughRectSpec<'a> {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub stroke_width: f32,
    pub work_meter: &'a crate::resources::OperationWorkMeter,
    pub phase: merman_model::OperationPhase,
    pub randomness: &'a roughr::core::RoughRandomness,
}

pub fn roughjs_paths_for_rect(spec: RoughRectSpec<'_>) -> Option<(String, String)> {
    roughjs_rect_operations(spec)
        .map(|(fill, stroke)| (ops_to_svg_path_d(&fill), ops_to_svg_path_d(&stroke)))
}

pub fn roughjs_rect_operations(
    spec: RoughRectSpec<'_>,
) -> Option<(roughr::core::OpSet<f64>, roughr::core::OpSet<f64>)> {
    let RoughRectSpec {
        x,
        y,
        w,
        h,
        stroke_width,
        work_meter,
        phase,
        randomness,
    } = spec;

    // Geometry generation does not inspect paint colors. Callers retain the original
    // CSS values when emitting SVG, including HSL/RGB, named colors, and CSS variables.
    let mut opts = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(0.0)
        .fill_style(roughr::core::FillStyle::Solid)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![0.0, 0.0])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    // Rough.js computes the rectangle outline first (advancing the PRNG state), then the fill, and
    // finally emits the fill path before the stroke path. Keep the same generation order to match
    // Mermaid's seeded output.
    controlled::generate(work_meter, phase, |control| {
        let stroke =
            roughr::renderer::rectangle_with_control::<f64>(x, y, w, h, &mut opts, control)?;
        roughr::work::WorkControl::charge(control, 4)?;
        let polygons = vec![vec![
            roughr::Point2D::new(x, y),
            roughr::Point2D::new(x + w, y),
            roughr::Point2D::new(x + w, y + h),
            roughr::Point2D::new(x, y + h),
        ]];
        let fill =
            roughr::renderer::solid_fill_polygon_with_control(&polygons, &mut opts, control)?;
        Ok((fill, stroke))
    })
    .ok()
}

pub fn roughjs_circle_path_d(
    diameter: f64,
    randomness: &roughr::core::RoughRandomness,
) -> Option<String> {
    roughjs_circle_operations(diameter, randomness).map(|opset| ops_to_svg_path_d(&opset))
}

pub fn roughjs_circle_operations(
    diameter: f64,
    randomness: &roughr::core::RoughRandomness,
) -> Option<roughr::core::OpSet<f64>> {
    let mut opts = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(0.0)
        .fill_style(roughr::core::FillStyle::Solid)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;
    let opset = roughr::renderer::ellipse::<f64>(0.0, 0.0, diameter, diameter, &mut opts);
    Some(opset)
}

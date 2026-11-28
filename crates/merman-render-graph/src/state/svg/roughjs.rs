//! State shape geometry for Mermaid parity and Shoal.

use merman_model::OperationPhase;
use roughr::core::{OpSet, OpType, RoughRandomness};
#[cfg(feature = "layout-shoal")]
use roughr::core::{Options, OptionsBuilder};
use roughr::{PathParser, PathSegment};

use merman_render_core::rough_svg::{
    RoughRectSpec, closed_path_d_from_points, mermaid_arc_points, ops_to_svg_path_d,
    roughjs_circle_path_d, roughjs_paths_for_rect,
};
#[cfg(feature = "layout-shoal")]
use merman_render_core::rough_svg::{
    append_minor_circular_arc, controlled, ops_to_compact_svg_path_d,
};

use super::StateRenderCtx;

// Match the roughness Mermaid uses for its hand-drawn node shapes.
#[cfg(feature = "layout-shoal")]
const SHOAL_HAND_DRAWN_ROUGHNESS: f32 = 0.7;

impl StateRenderCtx<'_> {
    #[cfg(feature = "layout-shoal")]
    fn uses_shoal_hand_drawn(&self) -> bool {
        self.uses_canonical_routes() && self.style_plan.compatibility().is_hand_drawn()
    }

    pub(super) fn rough_circle_path(&self, diameter: f64) -> Option<String> {
        #[cfg(feature = "layout-shoal")]
        if self.uses_shoal_hand_drawn() {
            let mut options = shoal_hand_drawn_options(&self.hand_drawn_seed)?;
            let outline = controlled::generate(self.work_meter, OperationPhase::Emit, |control| {
                let ellipse = roughr::renderer::generate_ellipse_params_with_control(
                    diameter,
                    diameter,
                    &mut options,
                    control,
                )?;
                roughr::renderer::ellipse_with_params_with_control(
                    0.0,
                    0.0,
                    &mut options,
                    &ellipse,
                    control,
                )
            })
            .ok()?;
            return Some(ops_to_compact_svg_path_d(&outline.opset));
        }
        roughjs_circle_path_d(diameter, &self.hand_drawn_seed)
    }

    pub(super) fn rough_leaf_rect_paths(&self, w: f64, h: f64) -> Option<(String, String)> {
        #[cfg(feature = "layout-shoal")]
        if self.uses_shoal_hand_drawn() {
            return self.shoal_rough_shape_paths(&mermaid_rounded_rect_path_data(w, h, 0.0));
        }
        roughjs_paths_for_rect(RoughRectSpec {
            x: -w / 2.0,
            y: -h / 2.0,
            w,
            h,
            stroke_width: 1.3,
            work_meter: self.work_meter,
            phase: OperationPhase::Emit,
            randomness: &self.hand_drawn_seed,
        })
    }

    pub(super) fn rough_rounded_rect_paths(
        &self,
        w: f64,
        h: f64,
        radius: f64,
    ) -> Option<(String, String)> {
        #[cfg(feature = "layout-shoal")]
        if self.uses_shoal_hand_drawn() {
            return self.shoal_rough_shape_paths(&shoal_rounded_rect_path_data(w, h, radius));
        }
        mermaid_rough_paths_for_svg_path(
            &mermaid_rounded_rect_path_data(w, h, radius),
            1.3,
            "0 0",
            &self.hand_drawn_seed,
        )
    }

    pub(super) fn rough_closed_path(
        &self,
        path: &str,
        stroke_width: f32,
        stroke_dasharray: &str,
    ) -> Option<(String, String)> {
        #[cfg(feature = "layout-shoal")]
        if self.uses_shoal_hand_drawn() {
            return self.shoal_rough_shape_paths(path);
        }
        mermaid_rough_paths_for_svg_path(
            path,
            stroke_width,
            stroke_dasharray,
            &self.hand_drawn_seed,
        )
    }

    #[cfg(feature = "layout-shoal")]
    fn shoal_rough_shape_paths(&self, path: &str) -> Option<(String, String)> {
        let mut options = shoal_hand_drawn_options(&self.hand_drawn_seed)?;
        // These shapes have one closed contour. Preserve its joins and perturb the curves.
        options.preserve_vertices = Some(true);
        let (fill, stroke) =
            controlled::generate(self.work_meter, OperationPhase::Emit, |control| {
                let segments = roughr::points_on_path::normalized_path_with_control(path, control)?;
                options.disable_multi_stroke = Some(true);
                let mut fill = roughr::renderer::svg_normalized_segments_with_control::<f64>(
                    &segments,
                    &mut options,
                    control,
                )?;
                join_solid_fill(&mut fill);
                options.disable_multi_stroke = Some(false);
                let stroke = roughr::renderer::svg_normalized_segments_with_control::<f64>(
                    &segments,
                    &mut options,
                    control,
                )?;
                Ok((fill, stroke))
            })
            .ok()?;
        Some((
            ops_to_compact_svg_path_d(&fill),
            ops_to_compact_svg_path_d(&stroke),
        ))
    }
}

#[cfg(feature = "layout-shoal")]
fn shoal_hand_drawn_options(randomness: &RoughRandomness) -> Option<Options> {
    OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(SHOAL_HAND_DRAWN_ROUGHNESS)
        .bowing(1.0)
        .build()
        .ok()
}

fn join_solid_fill(opset: &mut OpSet<f64>) {
    let mut first = true;
    opset
        .ops
        .retain(|op| std::mem::take(&mut first) || op.op != OpType::Move);
}

pub(super) fn normalized_rounded_rect_radius(w: f64, h: f64, radius: f64) -> f64 {
    let max_radius = (w.abs() / 2.0).min(h.abs() / 2.0);
    if !radius.is_finite() || !max_radius.is_finite() || max_radius <= 0.0 {
        0.0
    } else {
        radius.clamp(0.0, max_radius)
    }
}

#[cfg(feature = "layout-shoal")]
fn shoal_rounded_rect_path_data(w: f64, h: f64, radius: f64) -> String {
    use std::fmt::Write as _;

    let radius = normalized_rounded_rect_radius(w, h, radius);
    if radius == 0.0 {
        return mermaid_rounded_rect_path_data(w, h, 0.0);
    }

    let (x, y) = (w / 2.0, h / 2.0);
    let (tx, ty) = (x - radius, y - radius);
    let mut point = (-tx, -y);
    let mut path = format!("M{} {}", point.0, point.1);
    for (start, end) in [
        ((tx, -y), (x, -ty)),
        ((x, ty), (tx, y)),
        ((-tx, y), (-x, ty)),
        ((-x, -ty), (-tx, -y)),
    ] {
        if point != start {
            let _ = write!(path, "L{} {}", start.0, start.1);
        }
        // Each corner becomes a single cubic. The final arc returns to the starting point.
        append_minor_circular_arc(&mut path, radius, end, true);
        point = end;
    }
    path
}

fn mermaid_rounded_rect_path_data(w: f64, h: f64, radius: f64) -> String {
    let radius = normalized_rounded_rect_radius(w, h, radius);
    if radius == 0.0 {
        return closed_path_d_from_points(&[
            (-w / 2.0, -h / 2.0),
            (w / 2.0, -h / 2.0),
            (w / 2.0, h / 2.0),
            (-w / 2.0, h / 2.0),
        ]);
    }
    let taper = radius;

    let mut points: Vec<(f64, f64)> = Vec::new();

    points.push((-w / 2.0 + taper, -h / 2.0));
    points.push((w / 2.0 - taper, -h / 2.0));
    points.extend(mermaid_arc_points(
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
    points.extend(mermaid_arc_points(
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
    points.extend(mermaid_arc_points(
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
    points.extend(mermaid_arc_points(
        -w / 2.0,
        -h / 2.0 + taper,
        -w / 2.0 + taper,
        -h / 2.0,
        radius,
        radius,
        true,
    ));

    closed_path_d_from_points(&points)
}

pub(super) fn mermaid_choice_diamond_path_data(w: f64, h: f64) -> String {
    let points: Vec<(f64, f64)> = vec![
        (0.0, h / 2.0),
        (w / 2.0, 0.0),
        (0.0, -h / 2.0),
        (-w / 2.0, 0.0),
    ];
    closed_path_d_from_points(&points)
}

fn mermaid_rough_paths_for_svg_path(
    svg_path_data: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
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
        .randomness(randomness.clone())
        .roughness(0.0)
        .fill_style(roughr::core::FillStyle::Solid)
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
        join_solid_fill(&mut opset);
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
        ops_to_svg_path_d(&fill_opset),
        ops_to_svg_path_d(&stroke_opset),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_rect_radius_changes_path_geometry() {
        let square = mermaid_rounded_rect_path_data(100.0, 40.0, 0.0);
        let rounded = mermaid_rounded_rect_path_data(100.0, 40.0, 12.0);

        assert_ne!(square, rounded);
        assert_eq!(square, "M-50,-20 L50,-20 L50,20 L-50,20 Z");
    }

    #[test]
    fn rounded_rect_radius_is_clamped_to_half_the_smallest_side() {
        let clamped = mermaid_rounded_rect_path_data(100.0, 40.0, 100.0);
        let max = mermaid_rounded_rect_path_data(100.0, 40.0, 20.0);

        assert_eq!(clamped, max);
        assert_eq!(normalized_rounded_rect_radius(100.0, 40.0, -1.0), 0.0);
    }
}

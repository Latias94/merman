//! Flowchart edge curve selection and bounds.
//!
//! This logic is shared between flowchart SVG emission and viewBox computation, and mirrors the
//! behavior of upstream Mermaid when selecting the D3 curve implementation and when deciding
//! whether curve bounds can be skipped for `basis`.

use super::*;

pub(in crate::svg::parity::flowchart) fn curve_path_d_and_bounds(
    line_data: &[crate::model::LayoutPoint],
    interpolate: &str,
    origin_x: f64,
    abs_top_transform: f64,
    viewbox_current_bounds: Option<(f64, f64, f64, f64)>,
) -> (String, Option<path_bounds::SvgPathBounds>, bool) {
    let curve_is_basis = !matches!(
        interpolate,
        "linear"
            | "natural"
            | "bumpY"
            | "catmullRom"
            | "step"
            | "stepAfter"
            | "stepBefore"
            | "cardinal"
            | "monotoneX"
            | "monotoneY"
    );

    if curve_is_basis {
        // For `basis`, D3's curve stays inside the convex hull of the input points, so if the
        // polyline bbox is already inside the current viewBox bbox we can skip the expensive cubic
        // extrema solving used for tight bounds.
        let should_try_skip = viewbox_current_bounds.is_some();
        if should_try_skip && !line_data.is_empty() {
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for p in line_data {
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
                max_x = max_x.max(p.x);
                max_y = max_y.max(p.y);
            }
            let (cur_min_x, cur_min_y, cur_max_x, cur_max_y) =
                viewbox_current_bounds.expect("checked");
            let eps = 1e-9;
            let gx0 = min_x + origin_x;
            let gy0 = min_y + abs_top_transform;
            let gx1 = max_x + origin_x;
            let gy1 = max_y + abs_top_transform;
            if gx0 >= cur_min_x - eps
                && gy0 >= cur_min_y - eps
                && gx1 <= cur_max_x + eps
                && gy1 <= cur_max_y + eps
            {
                return (
                    crate::svg::parity::curve::curve_basis_path_d(line_data),
                    None,
                    true,
                );
            }
        }

        let (d, pb) = crate::svg::parity::curve::curve_basis_path_d_and_bounds(line_data);
        (d, pb, false)
    } else {
        let (d, pb) = match interpolate {
            "linear" => crate::svg::parity::curve::curve_linear_path_d_and_bounds(line_data),
            "natural" => crate::svg::parity::curve::curve_natural_path_d_and_bounds(line_data),
            "bumpY" => crate::svg::parity::curve::curve_bump_y_path_d_and_bounds(line_data),
            "catmullRom" => {
                crate::svg::parity::curve::curve_catmull_rom_path_d_and_bounds(line_data)
            }
            "step" => crate::svg::parity::curve::curve_step_path_d_and_bounds(line_data),
            "stepAfter" => crate::svg::parity::curve::curve_step_after_path_d_and_bounds(line_data),
            "stepBefore" => {
                crate::svg::parity::curve::curve_step_before_path_d_and_bounds(line_data)
            }
            "cardinal" => {
                crate::svg::parity::curve::curve_cardinal_path_d_and_bounds(line_data, 0.0)
            }
            "monotoneX" => {
                crate::svg::parity::curve::curve_monotone_path_d_and_bounds(line_data, false)
            }
            "monotoneY" => {
                crate::svg::parity::curve::curve_monotone_path_d_and_bounds(line_data, true)
            }
            // Mermaid defaults to `basis` for flowchart edges.
            _ => crate::svg::parity::curve::curve_basis_path_d_and_bounds(line_data),
        };

        (d, pb, false)
    }
}

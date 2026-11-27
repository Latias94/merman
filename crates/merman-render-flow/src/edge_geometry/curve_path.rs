//! Flowchart edge curve selection and bounds used by SVG emission and viewBox computation.

use merman_render_core::{
    LayoutPoint,
    svg_bounds::SvgPathBounds,
    svg_edge_parity::curve::{CurveGeometry, curve_geometry},
};

pub(super) fn curve_path_geometry(
    points: &[LayoutPoint],
    interpolate: &str,
    rounded_radius: f64,
    measure_length: bool,
) -> CurveGeometry {
    let mut geometry = curve_geometry(points, interpolate, rounded_radius, measure_length);
    geometry.d = maybe_close_single_point_path(geometry.d, points);
    if geometry.d == "Z" {
        // The rounded generator has no segments for a singleton and emits a bare close.
        // The bounds reader includes the origin for a bare close.
        geometry.bounds = Some(SvgPathBounds::at(0.0, 0.0));
        geometry.centerline_length = None;
    }
    geometry
}

fn maybe_close_single_point_path(d: String, line_data: &[LayoutPoint]) -> String {
    if line_data.len() == 1 && !d.ends_with('Z') {
        let mut d = d;
        d.push('Z');
        d
    } else {
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_render_core::{
        svg_bounds::svg_path_bounds_from_d, svg_edge_parity::svg_path_length_from_d,
    };

    #[test]
    fn bounds_match_the_serialized_curve_parser() {
        let points = [
            LayoutPoint {
                x: -5.0005,
                y: 0.0004,
            },
            LayoutPoint {
                x: -5.0005,
                y: 12.2405,
            },
            LayoutPoint {
                x: 14.9945,
                y: -0.12349,
            },
            LayoutPoint {
                x: 14.9945,
                y: -0.12349,
            },
            LayoutPoint {
                x: 2.0075,
                y: 32.5214,
            },
        ];
        let large_coordinates = [
            LayoutPoint {
                x: 123_456_789_123_456.75,
                y: -9_007_199_254_740.992,
            },
            LayoutPoint {
                x: -1.0e17,
                y: 1.0e17,
            },
        ];
        for kind in [
            "basis",
            "linear",
            "natural",
            "bumpX",
            "bumpY",
            "catmullRom",
            "step",
            "stepAfter",
            "stepBefore",
            "cardinal",
            "monotoneX",
            "monotoneY",
            "rounded",
            "unrecognized",
        ] {
            for route in [
                &points[..0],
                &points[..1],
                &points[..2],
                &points[..],
                &large_coordinates,
            ] {
                let geometry = curve_path_geometry(route, kind, 5.0, true);
                let bounds_bits = |bounds: SvgPathBounds| {
                    [bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y].map(f64::to_bits)
                };
                assert_eq!(
                    geometry.bounds.map(bounds_bits),
                    svg_path_bounds_from_d(&geometry.d).map(bounds_bits),
                    "{kind}: {}",
                    geometry.d,
                );
                assert_eq!(
                    geometry.centerline_length.map(f64::to_bits),
                    svg_path_length_from_d(&geometry.d).map(f64::to_bits),
                    "{kind}: {}",
                    geometry.d,
                );
            }
        }
    }

    #[test]
    fn basis_two_points_stays_linear_like_d3() {
        let points = vec![
            LayoutPoint { x: 0.0, y: 0.0 },
            LayoutPoint { x: 10.0, y: 4.0 },
        ];
        let geometry = curve_path_geometry(&points, "basis", 5.0, false);
        assert_eq!(geometry.d, "M0,0L10,4");
    }

    #[test]
    fn bump_axes_match_d3_reference_paths() {
        let points = vec![
            LayoutPoint { x: 0.0, y: 0.0 },
            LayoutPoint { x: 20.0, y: 10.0 },
            LayoutPoint { x: 40.0, y: 0.0 },
        ];
        // d3-shape from the Mermaid 12 reference graph, line().curve(curveBumpX/Y).
        for (curve, expected) in [
            ("bumpX", "M0,0C10,0,10,10,20,10C30,10,30,0,40,0"),
            ("bumpY", "M0,0C0,5,20,5,20,10C20,5,40,5,40,0"),
        ] {
            let geometry = curve_path_geometry(&points, curve, 5.0, false);
            assert_eq!(geometry.d, expected);
            assert!(geometry.bounds.is_some());
        }
    }

    #[test]
    fn maybe_close_single_point_path_appends_z_once() {
        let line_data = vec![LayoutPoint { x: 1.0, y: 2.0 }];

        assert_eq!(
            maybe_close_single_point_path("M1,2".to_string(), &line_data),
            "M1,2Z"
        );
        assert_eq!(
            maybe_close_single_point_path("M1,2Z".to_string(), &line_data),
            "M1,2Z"
        );
    }

    #[test]
    fn maybe_close_single_point_path_preserves_multi_point_paths() {
        let line_data = vec![
            LayoutPoint { x: 1.0, y: 2.0 },
            LayoutPoint { x: 3.0, y: 4.0 },
        ];

        assert_eq!(
            maybe_close_single_point_path("M1,2L3,4".to_string(), &line_data),
            "M1,2L3,4"
        );
    }

    #[test]
    fn rounded_corners_keep_mermaid_half_segment_cut_on_shallow_turns() {
        let line_data = vec![
            LayoutPoint {
                x: 130.484,
                y: 214.303,
            },
            LayoutPoint {
                x: 135.784,
                y: 230.203,
            },
            LayoutPoint {
                x: 135.784,
                y: 250.203,
            },
            LayoutPoint {
                x: 260.023,
                y: 250.203,
            },
        ];

        let parity = curve_path_geometry(&line_data, "rounded", 12.0, false).d;
        assert!(
            parity.starts_with("M130.484,214.303L133.134,222.253Q135.784,230.203 135.784,238.583"),
            "expected the Mermaid-compatible half-segment cut: {parity}"
        );
    }
}

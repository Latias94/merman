//! Flowchart edge curve selection and bounds used by SVG emission and viewBox computation.

use super::*;
use crate::svg::parity::curve::{CurveGeometry, curve_geometry};

pub(super) fn curve_path_geometry(
    points: &[crate::model::LayoutPoint],
    interpolate: &str,
    rounded_radius: f64,
    compact_edge_corners: bool,
    rounded_corner_mask: Option<&[bool]>,
) -> CurveGeometry {
    let mut geometry = curve_geometry(
        points,
        interpolate,
        rounded_radius,
        compact_edge_corners,
        rounded_corner_mask,
    );
    geometry.d = maybe_close_single_point_path(geometry.d, points);
    if geometry.d == "Z" {
        // The rounded generator has no segments for a singleton and emits a bare close.
        // The bounds reader includes the origin for a bare close.
        geometry.bounds = Some(path_bounds::SvgPathBounds::at(0.0, 0.0));
    }
    geometry
}

fn maybe_close_single_point_path(d: String, line_data: &[crate::model::LayoutPoint]) -> String {
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
    use crate::model::LayoutPoint;

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
        let corner_mask = [true, false, true, true, true];
        for (kind, compact, mask) in [
            ("basis", false, None),
            ("linear", false, None),
            ("natural", false, None),
            ("bumpX", false, None),
            ("bumpY", false, None),
            ("catmullRom", false, None),
            ("step", false, None),
            ("stepAfter", false, None),
            ("stepBefore", false, None),
            ("cardinal", false, None),
            ("monotoneX", false, None),
            ("monotoneY", false, None),
            ("rounded", false, None),
            ("rounded", true, Some(corner_mask.as_slice())),
            ("unrecognized", false, None),
        ] {
            for route in [
                &points[..0],
                &points[..1],
                &points[..2],
                &points[..],
                &large_coordinates,
            ] {
                let geometry = curve_path_geometry(route, kind, 5.0, compact, mask);
                let bounds_bits = |bounds: path_bounds::SvgPathBounds| {
                    [bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y].map(f64::to_bits)
                };
                assert_eq!(
                    geometry.bounds.map(bounds_bits),
                    svg_path_bounds_from_d(&geometry.d).map(bounds_bits),
                    "{kind}: {}",
                    geometry.d,
                );
            }
        }
    }

    #[test]
    fn basis_two_points_stays_linear_like_d3() {
        let points = vec![
            crate::model::LayoutPoint { x: 0.0, y: 0.0 },
            crate::model::LayoutPoint { x: 10.0, y: 4.0 },
        ];
        let geometry = curve_path_geometry(&points, "basis", 5.0, false, None);
        assert_eq!(geometry.d, "M0,0L10,4");
    }

    #[test]
    fn bump_axes_match_d3_reference_paths() {
        let points = vec![
            crate::model::LayoutPoint { x: 0.0, y: 0.0 },
            crate::model::LayoutPoint { x: 20.0, y: 10.0 },
            crate::model::LayoutPoint { x: 40.0, y: 0.0 },
        ];
        // d3-shape from the Mermaid 12 reference graph, line().curve(curveBumpX/Y).
        for (curve, expected) in [
            ("bumpX", "M0,0C10,0,10,10,20,10C30,10,30,0,40,0"),
            ("bumpY", "M0,0C0,5,20,5,20,10C20,5,40,5,40,0"),
        ] {
            let geometry = curve_path_geometry(&points, curve, 5.0, false, None);
            assert_eq!(geometry.d, expected);
            assert!(geometry.bounds.is_some());
        }
    }

    #[test]
    fn maybe_close_single_point_path_appends_z_once() {
        let line_data = vec![crate::model::LayoutPoint { x: 1.0, y: 2.0 }];

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
            crate::model::LayoutPoint { x: 1.0, y: 2.0 },
            crate::model::LayoutPoint { x: 3.0, y: 4.0 },
        ];

        assert_eq!(
            maybe_close_single_point_path("M1,2L3,4".to_string(), &line_data),
            "M1,2L3,4"
        );
    }

    #[test]
    fn compact_rounded_corners_do_not_overrun_shallow_elk_endpoint_turns() {
        let line_data = vec![
            crate::model::LayoutPoint {
                x: 130.484,
                y: 214.303,
            },
            crate::model::LayoutPoint {
                x: 135.784,
                y: 230.203,
            },
            crate::model::LayoutPoint {
                x: 135.784,
                y: 250.203,
            },
            crate::model::LayoutPoint {
                x: 260.023,
                y: 250.203,
            },
        ];

        let parity = curve_path_geometry(&line_data, "rounded", 12.0, false, None).d;
        let compact = curve_path_geometry(&line_data, "rounded", 12.0, true, None).d;

        assert!(
            parity.starts_with("M130.484,214.303L133.134,222.253Q135.784,230.203 135.784,238.583"),
            "expected the Mermaid-compatible half-segment cut: {parity}"
        );
        assert!(
            compact.starts_with("M130.484,214.303L135.168,228.356Q135.784,230.203 135.784,232.15"),
            "expected the compact tangent-length cut: {compact}"
        );

        let corner_mask = [true, false, true, true];
        let adapter_linear =
            curve_path_geometry(&line_data, "rounded", 12.0, true, Some(&corner_mask)).d;
        assert!(
            adapter_linear
                .starts_with("M130.484,214.303L135.784,230.203L135.784,240.203Q135.784,250.203"),
            "expected the endpoint adapter to stay linear while the ELK bend remains rounded: {adapter_linear}"
        );
    }
}

//! The pinned Cynefin marker's local geometry and placement, shared by drawing and SVG output.

use merman_display_list::{PathSegment, Point, Transform};

pub(crate) const VIEW_BOX_SIZE: f64 = 10.0;
pub(crate) const MARKER_SIZE: f64 = 6.0;
pub(crate) const REF_X: f64 = 9.0;
pub(crate) const REF_Y: f64 = 5.0;

pub(crate) fn marker_path() -> Vec<PathSegment> {
    vec![
        PathSegment::MoveTo {
            to: Point::new(0.0, 0.0),
        },
        PathSegment::LineTo {
            to: Point::new(10.0, 5.0),
        },
        PathSegment::LineTo {
            to: Point::new(0.0, 10.0),
        },
        PathSegment::Close,
    ]
}

pub(crate) fn marker_transform(
    start: Point,
    control: Point,
    end: Point,
    stroke_width: f64,
) -> Option<Transform> {
    let mut dx = end.x - control.x;
    let mut dy = end.y - control.y;
    if dx == 0.0 && dy == 0.0 {
        dx = end.x - start.x;
        dy = end.y - start.y;
    }
    let length = dx.hypot(dy);
    if !length.is_finite() || length == 0.0 || !stroke_width.is_finite() || stroke_width < 0.0 {
        return None;
    }
    // SVG's default markerUnits is strokeWidth. refX/refY are in viewBox coordinates.
    let scale = MARKER_SIZE / VIEW_BOX_SIZE * stroke_width;
    let a = dx / length * scale;
    let b = dy / length * scale;
    let c = -b;
    let d = a;
    Some(Transform {
        a,
        b,
        c,
        d,
        e: end.x - a * REF_X - c * REF_Y,
        f: end.y - b * REF_X - d * REF_Y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_reference_and_tip_preserve_source_offsets() {
        for (start, control, end, tip) in [
            (
                Point::new(0.0, 0.0),
                Point::new(5.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(11.2, 0.0),
            ),
            (
                Point::new(0.0, 0.0),
                Point::new(0.0, 5.0),
                Point::new(0.0, 10.0),
                Point::new(0.0, 11.2),
            ),
            (
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(11.2, 0.0),
            ),
        ] {
            let t = marker_transform(start, control, end, 2.0).unwrap();
            let apply = |x, y| Point::new(t.a * x + t.c * y + t.e, t.b * x + t.d * y + t.f);
            assert_eq!(apply(9.0, 5.0), end);
            let actual_tip = apply(10.0, 5.0);
            assert!((actual_tip.x - tip.x).abs() < 1e-12);
            assert!((actual_tip.y - tip.y).abs() < 1e-12);
            let zero = marker_transform(start, control, end, 0.0).unwrap();
            assert_eq!([zero.a, zero.b, zero.c, zero.d], [0.0; 4]);
        }
        let same = Point::new(1.0, 1.0);
        assert!(marker_transform(same, same, same, 2.0).is_none());
    }
}

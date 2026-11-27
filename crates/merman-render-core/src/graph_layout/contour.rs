//! Port spacing and normal projections in node-local coordinates. A node can retain its
//! profile from measurement onwards; placing or translating the node only changes `center`.

use crate::LayoutPoint;
use crate::route_types::{RoutePoint, Side};
use kurbo::{Arc, Point};
use std::f64::consts::TAU;

pub const ROUNDED_PORT_FRACTION: f64 = 0.55;

/// The span is a full width (top/bottom) or height (left/right), centered on the port anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapePortProfile {
    outline: Outline,
    span: [f64; 2],
    anchor_y: Option<f64>,
}

impl ShapePortProfile {
    pub fn new(outline: Outline, span_x: f64, span_y: f64) -> Self {
        Self {
            outline,
            span: [span_x.max(0.0), span_y.max(0.0)],
            anchor_y: None,
        }
    }

    pub fn rectangle(width: f64, height: f64) -> Self {
        Self::new(
            Outline::rect(width, height, 0.0, 0.0),
            width - 16.0,
            height - 16.0,
        )
    }

    pub fn rounded_rectangle(width: f64, height: f64, rx: f64, ry: f64) -> Self {
        let rx = rx.clamp(0.0, width.max(0.0) / 2.0);
        let ry = ry.clamp(0.0, height.max(0.0) / 2.0);
        Self::new(
            Outline::rect(width, height, rx, ry),
            width - 2.0 * rx,
            height - 2.0 * ry,
        )
    }

    pub fn ellipse(width: f64, height: f64) -> Self {
        Self::new(
            Outline::ellipse(width, height),
            width * ROUNDED_PORT_FRACTION,
            height * ROUNDED_PORT_FRACTION,
        )
    }

    pub fn diamond(width: f64, height: f64) -> Self {
        Self::new(Outline::diamond(width, height, 0.0), 0.0, 0.0)
    }

    pub fn port_span(&self, side: Side) -> f64 {
        self.span[usize::from(matches!(side, Side::Right | Side::Left))]
    }

    /// Offset from the top of the measured box for an icon's left/right port anchor.
    pub fn anchor_y(&self) -> Option<f64> {
        self.anchor_y
    }

    pub fn outline(&self) -> &Outline {
        &self.outline
    }

    pub fn with_anchor_y(mut self, anchor_y: f64) -> Self {
        self.anchor_y = Some(anchor_y);
        self
    }

    /// Set the span shared by the opposing ports on this axis.
    pub fn with_port_span(mut self, side: Side, span: f64) -> Self {
        self.span[usize::from(matches!(side, Side::Right | Side::Left))] = span;
        self
    }

    /// A fixed port outside the transverse extent of the entire outline has no normal hit.
    pub fn try_contact(
        &self,
        center: &LayoutPoint,
        side: Side,
        port: RoutePoint,
    ) -> Option<RoutePoint> {
        let vertical = matches!(side, Side::Top | Side::Bottom);
        let (cross, anchor) = if vertical {
            (port.x, center.x)
        } else {
            (port.y, center.y)
        };
        let mut across = cross - anchor;
        if matches!(
            self.outline,
            Outline::Box {
                hw: 0.0,
                hh: 0.0,
                ..
            }
        ) && across.abs() <= cross.abs().max(anchor.abs()).max(1.0) * (4.0 * f64::EPSILON)
        {
            // A translated point port can differ from its box center by a few ulps.
            across = 0.0;
        }
        let along = self.outline.crossing(side, across)?;
        Some(if vertical {
            RoutePoint {
                y: center.y + along,
                ..port
            }
        } else {
            RoutePoint {
                x: center.x + along,
                ..port
            }
        })
    }

    /// Keep an out-of-range explicit port at its supplied box contact.
    pub fn contact(&self, center: &LayoutPoint, side: Side, port: RoutePoint) -> RoutePoint {
        self.try_contact(center, side, port).unwrap_or(port)
    }
}

pub fn inset_endpoints(
    points: &mut [RoutePoint],
    source: (&ShapePortProfile, &LayoutPoint, Side),
    target: (&ShapePortProfile, &LayoutPoint, Side),
) {
    if let [start, .., end] = points {
        *start = source.0.contact(source.1, source.2, *start);
        *end = target.0.contact(target.1, target.2, *end);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outline {
    /// Includes a sharp rectangle, ellipse and elliptical cylinder caps as limiting cases.
    Box {
        cx: f64,
        cy: f64,
        hw: f64,
        hh: f64,
        rx: f64,
        ry: f64,
    },
    Polygon(Vec<(f64, f64)>),
    Path {
        start: Point,
        edges: Vec<Edge>,
    },
    Parts(Vec<Outline>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Edge {
    Line(Point),
    Arc(Arc, Point),
}

fn transverse(side: Side, p: Point) -> (f64, f64) {
    if matches!(side, Side::Top | Side::Bottom) {
        (p.x, p.y)
    } else {
        (p.y, p.x)
    }
}

fn outer(side: Side, a: f64, b: f64) -> f64 {
    if matches!(side, Side::Top | Side::Left) {
        a.min(b)
    } else {
        a.max(b)
    }
}

fn line_hit(side: Side, across: f64, a: Point, b: Point) -> Option<f64> {
    let ((a, u), (b, v)) = (transverse(side, a), transverse(side, b));
    if across < a.min(b) || across > a.max(b) {
        return None;
    }
    Some(if a == b {
        outer(side, u, v)
    } else if across == b {
        v
    } else {
        u + (across - a) / (b - a) * (v - u)
    })
}

fn arc_hit(side: Side, across: f64, arc: &Arc) -> Option<f64> {
    // The transverse component is A cos(t) + B sin(t). Both roots matter on a tilted arc.
    let vertical = matches!(side, Side::Top | Side::Bottom);
    let (sin, cos) = arc.x_rotation.sin_cos();
    let (a, b, offset) = if vertical {
        (arc.radii.x * cos, -arc.radii.y * sin, across - arc.center.x)
    } else {
        (arc.radii.x * sin, arc.radii.y * cos, across - arc.center.y)
    };
    let radius = a.hypot(b);
    if radius == 0.0 || offset.abs() > radius * (1.0 + 8.0 * f64::EPSILON) {
        return None;
    }
    let phase = b.atan2(a);
    let spread = (offset / radius).clamp(-1.0, 1.0).acos();
    [phase - spread, phase + spread]
        .into_iter()
        .filter_map(|t| {
            let traveled = ((t - arc.start_angle) * arc.sweep_angle.signum()).rem_euclid(TAU);
            if traveled > arc.sweep_angle.abs() + 1e-12 && TAU - traveled > 1e-12 {
                return None;
            }
            let (s, c) = t.sin_cos();
            Some(if vertical {
                arc.center.y + arc.radii.x * c * sin + arc.radii.y * s * cos
            } else {
                arc.center.x + arc.radii.x * c * cos - arc.radii.y * s * sin
            })
        })
        .reduce(|a, b| outer(side, a, b))
}

impl Outline {
    pub fn rect(w: f64, h: f64, rx: f64, ry: f64) -> Self {
        Self::rect_at((0.0, 0.0), w, h, rx, ry)
    }

    pub fn rect_at((cx, cy): (f64, f64), w: f64, h: f64, rx: f64, ry: f64) -> Self {
        let (hw, hh) = (w.max(0.0) * 0.5, h.max(0.0) * 0.5);
        Self::Box {
            cx,
            cy,
            hw,
            hh,
            rx: rx.clamp(0.0, hw),
            ry: ry.clamp(0.0, hh),
        }
    }

    fn ellipse(w: f64, h: f64) -> Self {
        Self::rect(w, h, w * 0.5, h * 0.5)
    }

    pub fn diamond(w: f64, h: f64, cx: f64) -> Self {
        Self::Polygon(vec![
            (cx, -h / 2.0),
            (cx + w / 2.0, 0.0),
            (cx, h / 2.0),
            (cx - w / 2.0, 0.0),
        ])
    }

    pub fn polygon_at(points: impl IntoIterator<Item = (f64, f64)>, dx: f64, dy: f64) -> Self {
        Self::Polygon(points.into_iter().map(|(x, y)| (x + dx, y + dy)).collect())
    }

    fn crossing(&self, side: Side, across: f64) -> Option<f64> {
        let vertical = matches!(side, Side::Top | Side::Bottom);
        let negative = matches!(side, Side::Top | Side::Left);
        let outer = |a, b| outer(side, a, b);
        match self {
            &Self::Box {
                cx,
                cy,
                hw,
                hh,
                rx,
                ry,
            } => {
                let (offset, half, radius, depth, round, center) = if vertical {
                    ((across - cx).abs(), hw, rx, hh, ry, cy)
                } else {
                    ((across - cy).abs(), hh, ry, hw, rx, cx)
                };
                if offset > half {
                    return None;
                }
                let corner = offset - (half - radius);
                let mut extent = depth;
                if corner > 0.0 && round > 0.0 && radius > 0.0 {
                    let ratio = (corner / radius).min(1.0);
                    extent -= round * (1.0 - ((1.0 - ratio) * (1.0 + ratio)).sqrt());
                }
                Some(if negative {
                    center - extent
                } else {
                    center + extent
                })
            }
            Self::Polygon(points) => {
                let mut prev = Point::from(*points.last()?);
                points
                    .iter()
                    .filter_map(|&p| {
                        let p = Point::from(p);
                        line_hit(side, across, std::mem::replace(&mut prev, p), p)
                    })
                    .reduce(outer)
            }
            Self::Path { start, edges } => {
                let mut prev = *start;
                let hits = edges
                    .iter()
                    .filter_map(|edge| match edge {
                        Edge::Line(p) => {
                            line_hit(side, across, std::mem::replace(&mut prev, *p), *p)
                        }
                        Edge::Arc(arc, p) => {
                            prev = *p;
                            arc_hit(side, across, arc)
                        }
                    })
                    .reduce(outer);
                hits.into_iter()
                    .chain(line_hit(side, across, prev, *start))
                    .reduce(outer)
            }
            Self::Parts(parts) => parts
                .iter()
                .filter_map(|part| part.crossing(side, across))
                .reduce(outer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route_types::Axis;

    fn p(x: f64, y: f64) -> LayoutPoint {
        LayoutPoint { x, y }
    }
    fn v(x: f64, y: f64) -> RoutePoint {
        RoutePoint::new(x, y, Axis::Vertical)
    }
    fn h(x: f64, y: f64) -> RoutePoint {
        RoutePoint::new(x, y, Axis::Horizontal)
    }
    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} vs {b}");
    }

    #[test]
    fn projected_ports_keep_their_cross_coordinate_on_all_four_sides() {
        let center = p(123.0, -38.0);
        let ellipse = ShapePortProfile::ellipse(100.0, 60.0);
        // At x = 30, y = ±24; at y = 18, x = ±40. A ray to the center misses all four ports.
        for (side, a, b) in [
            (Side::Top, v(153.0, -68.0), v(153.0, -62.0)),
            (Side::Bottom, v(153.0, -8.0), v(153.0, -14.0)),
            (Side::Left, h(73.0, -20.0), h(83.0, -20.0)),
            (Side::Right, h(173.0, -20.0), h(163.0, -20.0)),
        ] {
            let out = ellipse.contact(&center, side, a);
            assert_eq!(out.axis, b.axis);
            close(out.x, b.x);
            close(out.y, b.y);
        }
    }

    #[test]
    fn inset_both_ends_once_preserves_every_interior_knot() {
        let rect = ShapePortProfile::rectangle(60.0, 40.0);
        let rounded = ShapePortProfile::rounded_rectangle(80.0, 60.0, 10.0, 5.0);
        let mut route = [h(20.0, 12.0), h(48.0, 12.0), v(116.0, 40.0), v(116.0, 60.0)];
        let inside = route[1..3].to_vec();
        let src = (&rect, &p(-10.0, 10.0), Side::Right);
        let dst = (&rounded, &p(80.0, 90.0), Side::Top);
        inset_endpoints(&mut route, src, dst);
        assert_eq!(route[0], h(20.0, 12.0));
        assert_eq!(route[1..3], inside);
        assert_eq!(route[3], v(116.0, 61.0));
        let clipped = route;
        inset_endpoints(&mut route, src, dst);
        assert_eq!(route, clipped);
    }

    #[test]
    fn rotated_arc_uses_only_the_requested_angular_interval() {
        use kurbo::ParamCurve;
        let arc = Arc::new((20.0, -40.0), (41.0, 12.0), 0.3, 0.7, 0.35);
        for t in [0.0, 0.2, 0.8, 1.0] {
            let p = arc.eval(t);
            close(arc_hit(Side::Top, p.x, &arc).unwrap(), p.y);
        }
        assert!(arc_hit(Side::Bottom, -30.0, &arc).is_none());
    }
}

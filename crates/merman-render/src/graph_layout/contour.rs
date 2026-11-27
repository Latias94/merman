//! Port spacing and normal projections in node-local coordinates. A node can retain its
//! profile from measurement onwards; placing or translating the node only changes `center`.

use super::ir::{RoutePoint, Side};
use crate::model::LayoutPoint;
use kurbo::{Arc, Point};
use std::f64::consts::TAU;

#[cfg(all(feature = "diagram-c4", feature = "layout-shoal"))]
#[path = "../c4/contour.rs"]
mod c4;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
mod flowchart;
#[cfg(all(feature = "diagram-usecase", feature = "layout-shoal"))]
mod usecase;

#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-state",
    feature = "diagram-usecase"
))]
const ROUNDED_PORT_FRACTION: f64 = 0.55;

/// The span is a full width (top/bottom) or height (left/right), centered on the port anchor.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ShapePortProfile {
    outline: Outline,
    span: [f64; 2],
    anchor_y: Option<f64>,
}

impl ShapePortProfile {
    fn new(outline: Outline, span_x: f64, span_y: f64) -> Self {
        Self {
            outline,
            span: [span_x.max(0.0), span_y.max(0.0)],
            anchor_y: None,
        }
    }

    pub(crate) fn rectangle(width: f64, height: f64) -> Self {
        Self::new(
            Outline::rect(width, height, 0.0, 0.0),
            width - 16.0,
            height - 16.0,
        )
    }

    pub(crate) fn rounded_rectangle(width: f64, height: f64, rx: f64, ry: f64) -> Self {
        let rx = rx.clamp(0.0, width.max(0.0) / 2.0);
        let ry = ry.clamp(0.0, height.max(0.0) / 2.0);
        Self::new(
            Outline::rect(width, height, rx, ry),
            width - 2.0 * rx,
            height - 2.0 * ry,
        )
    }

    #[cfg(any(
        feature = "diagram-agentflow",
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-state",
        feature = "diagram-usecase"
    ))]
    pub(crate) fn ellipse(width: f64, height: f64) -> Self {
        Self::new(
            Outline::ellipse(width, height),
            width * ROUNDED_PORT_FRACTION,
            height * ROUNDED_PORT_FRACTION,
        )
    }

    #[cfg(any(
        feature = "diagram-agentflow",
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-state"
    ))]
    pub(crate) fn diamond(width: f64, height: f64) -> Self {
        Self::new(Outline::diamond(width, height, 0.0), 0.0, 0.0)
    }

    #[cfg(all(
        feature = "layout-shoal",
        any(
            feature = "diagram-flowchart",
            feature = "diagram-agentflow",
            feature = "diagram-swimlane"
        )
    ))]
    pub(crate) fn decision(width: f64, height: f64) -> Self {
        Self::new(
            Outline::diamond(width, height, 0.0),
            width * 0.54,
            height * 0.54,
        )
    }

    pub(crate) fn port_span(&self, side: Side) -> f64 {
        self.span[usize::from(matches!(side, Side::Right | Side::Left))]
    }

    /// Offset from the top of the measured box for an icon's left/right port anchor.
    #[cfg(any(
        feature = "diagram-agentflow",
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-c4",
        feature = "diagram-state"
    ))]
    pub(crate) fn anchor_y(&self) -> Option<f64> {
        self.anchor_y
    }

    #[cfg(feature = "layout-shoal")]
    pub(crate) fn shoal_contour(
        &self,
        size: super::ir::Size,
    ) -> merman_layout_shoal::ShapeContourHint {
        use merman_layout_shoal::ShapeContourHint;

        let (width, height) = (size.width / 2.0, size.height / 2.0);
        match &self.outline {
            &Outline::Box {
                cx: 0.0,
                cy: 0.0,
                hw,
                hh,
                rx,
                ry,
            } if hw == width && hh == height => {
                if rx == hw && ry == hh {
                    ShapeContourHint::Ellipse
                } else if rx == 0.0 || ry == 0.0 {
                    ShapeContourHint::Rectangle
                } else if rx == ry && rx == hw.min(hh) {
                    ShapeContourHint::Stadium
                } else {
                    ShapeContourHint::Rounded { rx, ry }
                }
            }
            Outline::Polygon(points) => {
                if let &[(x, top), (right, y), (bottom_x, bottom), (left, left_y)] =
                    points.as_slice()
                    && [top, right, bottom, left, bottom_x, left_y]
                        == [y - height, x + width, y + height, x - width, x, y]
                {
                    if x == 0.0 && y == 0.0 {
                        ShapeContourHint::Diamond
                    } else {
                        ShapeContourHint::OffsetDiamond { x, y }
                    }
                } else {
                    ShapeContourHint::Rectangle
                }
            }
            _ => ShapeContourHint::Rectangle,
        }
    }

    /// A fixed port outside the transverse extent of the entire outline has no normal hit.
    pub(crate) fn try_contact(
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
    pub(crate) fn contact(&self, center: &LayoutPoint, side: Side, port: RoutePoint) -> RoutePoint {
        self.try_contact(center, side, port).unwrap_or(port)
    }
}

#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
pub(crate) fn inset_endpoints(
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
#[allow(
    dead_code,
    reason = "Each family constructs only the outlines used by its shapes."
)]
enum Outline {
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
#[allow(
    dead_code,
    reason = "Flowchart-derived shapes construct line and arc outlines."
)]
enum Edge {
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
    fn rect(w: f64, h: f64, rx: f64, ry: f64) -> Self {
        Self::rect_at((0.0, 0.0), w, h, rx, ry)
    }

    fn rect_at((cx, cy): (f64, f64), w: f64, h: f64, rx: f64, ry: f64) -> Self {
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

    #[cfg(any(
        feature = "diagram-agentflow",
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-state",
        feature = "diagram-usecase"
    ))]
    fn ellipse(w: f64, h: f64) -> Self {
        Self::rect(w, h, w * 0.5, h * 0.5)
    }

    #[cfg(any(
        feature = "diagram-agentflow",
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-state"
    ))]
    fn diamond(w: f64, h: f64, cx: f64) -> Self {
        Self::Polygon(vec![
            (cx, -h / 2.0),
            (cx + w / 2.0, 0.0),
            (cx, h / 2.0),
            (cx - w / 2.0, 0.0),
        ])
    }

    #[cfg(any(
        feature = "diagram-agentflow",
        feature = "diagram-flowchart",
        feature = "diagram-swimlane"
    ))]
    fn polygon_at(points: impl IntoIterator<Item = (f64, f64)>, dx: f64, dy: f64) -> Self {
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

#[cfg(all(
    test,
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    )
))]
mod tests {
    use super::*;
    use crate::flowchart::FlowNode;
    use crate::graph_layout::ir::Axis;
    use crate::model::LayoutNode;

    fn p(x: f64, y: f64) -> LayoutPoint {
        LayoutPoint { x, y }
    }
    fn v(x: f64, y: f64) -> RoutePoint {
        RoutePoint::new(x, y, Axis::Vertical)
    }
    fn h(x: f64, y: f64) -> RoutePoint {
        RoutePoint::new(x, y, Axis::Horizontal)
    }
    fn profile(name: &str, w: f64, h: f64, lw: f64, lh: f64, neo: bool) -> ShapePortProfile {
        let node: FlowNode =
            serde_json::from_value(serde_json::json!({"id":"n","layoutShape":name})).unwrap();
        let measured = LayoutNode {
            id: "n".into(),
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
            is_cluster: false,
            label_width: Some(lw),
            label_height: Some(lh),
        };
        ShapePortProfile::flowchart(&node, &measured, 15.0, neo).unwrap()
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
    fn shape_table_sets_the_spans_before_placement() {
        for (name, w, h, x, y) in [
            ("process", 100.0, 40.0, 84.0, 24.0),
            ("subroutine", 100.0, 40.0, 84.0, 24.0),
            ("rounded", 100.0, 40.0, 90.0, 30.0),
            ("stadium", 100.0, 40.0, 60.0, 22.0),
            ("circle", 60.0, 60.0, 33.0, 33.0),
            ("double-circle", 60.0, 60.0, 33.0, 33.0),
            ("small-circle", 14.0, 14.0, 7.7, 7.7),
            ("diamond", 100.0, 80.0, 60.0, 48.0),
            ("choice", 28.0, 28.0, 0.0, 0.0),
            ("triangle", 80.0, 80.0, 0.0, 0.0),
        ] {
            let shape = profile(name, w, h, 40.0, 16.0, false);
            for side in [Side::Top, Side::Bottom] {
                close(shape.port_span(side), x);
            }
            for side in [Side::Left, Side::Right] {
                close(shape.port_span(side), y);
            }
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
    fn concave_notches_folded_layers_and_hex_shoulders_use_the_actual_face() {
        let c = p(0.0, 0.0);
        let notch = profile("card", 100.0, 60.0, 50.0, 15.0, false);
        close(notch.contact(&c, Side::Top, v(-47.0, -30.0)).y, -21.0);
        let odd = profile("odd", 100.0, 40.0, 50.0, 15.0, false);
        close(odd.contact(&c, Side::Left, h(-50.0, 0.0)).x, -40.0);
        close(odd.contact(&c, Side::Left, h(-50.0, -10.0)).x, -45.0);
        let stack = profile("stacked-rectangle", 120.0, 80.0, 50.0, 20.0, true);
        close(stack.contact(&c, Side::Top, v(-55.0, -40.0)).y, -20.0);
        close(stack.contact(&c, Side::Bottom, v(55.0, 40.0)).y, 20.0);
        let hex = profile("hexagon", 100.0, 40.0, 50.0, 15.0, false);
        close(hex.contact(&c, Side::Top, v(45.0, -20.0)).y, -10.0);
        let diamond = profile("decision", 80.0, 80.0, 0.0, 0.0, false);
        close(diamond.contact(&c, Side::Bottom, v(10.5, 40.0)).y, 30.0);
        let triangle = profile("extract", 80.0, 80.0, 40.0, 25.0, false);
        close(triangle.contact(&c, Side::Right, h(40.0, -10.0)).x, 15.0);
    }

    #[test]
    fn cylinder_caps_waves_and_bucket_extend_to_the_painted_coordinates() {
        let c = p(0.0, 0.0);
        let cylinder = profile("database", 100.0, 80.0, 0.0, 0.0, false);
        let ry = 50.0 / 4.5;
        close(
            cylinder.contact(&c, Side::Bottom, v(30.0, 40.0)).y,
            40.0 - 0.2 * ry,
        );
        close(
            cylinder
                .contact(&c, Side::Right, h(50.0, -40.0 + ry / 5.0))
                .x,
            30.0,
        );
        // Neo paper tape is the pair of 50-chord waves used in Flowchart's SVG. Sample halfway
        // between vertices 12 and 13: those vertices have equal y, below the analytical peak.
        let tape = profile("flag", 132.0, 59.98, 100.0, 20.0, true);
        let peak = 5.0 * (0.24 * TAU).sin();
        close(
            tape.contact(&c, Side::Bottom, v(-33.0, 30.0)).y,
            25.0 + peak,
        );
        close(tape.contact(&c, Side::Top, v(-33.0, -30.0)).y, -25.0 + peak);
        let bucket = profile("bucket", 100.0, 60.0, 62.0, 18.0, false);
        close(bucket.contact(&c, Side::Top, v(0.0, -30.0)).y, -30.0);
        close(bucket.contact(&c, Side::Bottom, v(0.0, 30.0)).y, 38.0);
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
        let cloud = profile("cloud", 85.67, 60.7, 40.53125, 19.0, false);
        let c = p(0.0, 0.0);
        let top = cloud.try_contact(&c, Side::Top, v(8.0, -30.35)).unwrap();
        let bottom = cloud.try_contact(&c, Side::Bottom, v(8.0, 30.35)).unwrap();
        assert!(top.y < 0.0 && bottom.y > 0.0 && top.y.abs() != bottom.y);
    }

    #[test]
    fn captions_do_not_stretch_bpmn_glyphs_or_shift_icon_side_anchors() {
        let c = p(0.0, 0.0);
        let event = profile("bpmn-end", 140.0, 100.0, 140.0, 26.0, false);
        close(event.contact(&c, Side::Right, h(70.0, 0.0)).x, 18.0);
        close(event.contact(&c, Side::Bottom, v(0.0, 50.0)).y, 18.0);
        let gateway = profile("bpmn-gateway", 140.0, 110.0, 140.0, 24.0, false);
        close(gateway.contact(&c, Side::Left, h(-70.0, 0.0)).x, -25.0);
        let note = profile("bpmn-annotation", 102.0, 40.0, 80.0, 24.0, false);
        close(note.contact(&c, Side::Right, h(51.0, 0.0)).x, 43.0);
        close(note.contact(&c, Side::Right, h(51.0, 19.0)).x, -51.0);
        close(note.contact(&c, Side::Top, v(0.0, -20.0)).y, -12.0);
        let icon = profile("iconRounded", 104.0, 95.0, 100.0, 20.0, false);
        close(icon.anchor_y().unwrap(), 31.5);
        close(icon.contact(&c, Side::Right, h(52.0, -16.0)).x, 31.5);
        close(icon.contact(&c, Side::Bottom, v(0.0, 47.5)).y, 47.5);
    }

    #[test]
    fn paint_metrics_preserve_rect_radius_overrides_and_the_image_behind_a_wider_caption() {
        let mut node: FlowNode = serde_json::from_value(serde_json::json!({
            "id":"n", "layoutShape":"imageSquare", "assetWidth":40.0, "assetHeight":60.0,
            "pos":"t", "img":"image.png", "label":"caption"
        }))
        .unwrap();
        let measured = LayoutNode {
            id: "n".into(),
            x: 0.0,
            y: 0.0,
            width: 244.0,
            height: 92.0,
            is_cluster: false,
            label_width: Some(240.0),
            label_height: Some(20.0),
        };
        let c = p(0.0, 0.0);
        let image_geometry = crate::flowchart::ImageSquareGeometry::resolve(
            crate::flowchart::ImageSquareSizing {
                href: node.img.as_deref().unwrap(),
                width: node.asset_width,
                height: node.asset_height,
                constraint_on: false,
            },
            crate::text::TextMetrics {
                width: 240.0,
                height: 20.0,
                line_count: 1,
            },
            true,
            160.0,
            true,
            |_| Ok(()),
        )
        .unwrap();
        let image = ShapePortProfile::flowchart_with_paint_metrics(
            &node,
            &measured,
            15.0,
            false,
            Some(crate::flowchart::PreparedFlowchartNodeGeometry::ImageSquare(image_geometry)),
            None,
        )
        .unwrap();
        close(image.contact(&c, Side::Right, h(122.0, 16.0)).x, 80.0);
        close(image.contact(&c, Side::Right, h(122.0, -36.0)).x, 122.0);
        for name in ["process", "rounded"] {
            node.layout_shape = Some(name.into());
            let shape = ShapePortProfile::flowchart_with_paint_metrics(
                &node,
                &measured,
                15.0,
                false,
                None,
                Some((30.0, 10.0)),
            )
            .unwrap();
            close(shape.contact(&c, Side::Top, v(110.0, -46.0)).y, -44.0);
            close(shape.port_span(Side::Top), 184.0);
        }
    }
}

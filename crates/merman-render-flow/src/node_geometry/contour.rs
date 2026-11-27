//! Flowchart's measured paint geometry, including local transforms and caption bands.

use kurbo::{Arc, Point, SvgArc, Vec2};
use merman_model::families::flowchart::FlowNode;
use merman_render_core::graph_layout::contour::{
    Edge, Outline, ROUNDED_PORT_FRACTION, ShapePortProfile,
};
use merman_render_core::{LayoutNode, Result};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

use super::{
    DelayGeometry, DisplayGeometry, DoubleCircleGeometry, HexagonGeometry, LeanGeometry, LeanKind,
    OddGeometry, PreparedFlowchartNodeGeometry, StadiumGeometry,
    flowchart_brace_content_dimensions, flowchart_stacked_document_geometry,
    resolve_flowchart_shape,
};
use crate::bpmn::BpmnShape;
use crate::shapes::{FlowchartShape as S, OrganicShapeGeometry, bang_geometry, cloud_geometry};

pub fn fork_join(width: f64, height: f64) -> ShapePortProfile {
    let (width, height) = if width >= height {
        (70.0, 10.0)
    } else {
        (10.0, 70.0)
    };
    ShapePortProfile::new(
        Outline::rect(width, height, 0.0, 0.0),
        width - 8.0,
        height - 8.0,
    )
}

pub fn decision(width: f64, height: f64) -> ShapePortProfile {
    ShapePortProfile::new(
        Outline::diamond(width, height, 0.0),
        width * 0.54,
        height * 0.54,
    )
}

/// Construct ports from final paint dimensions and the retained label metrics.
pub fn from_paint_metrics(
    node: &FlowNode,
    measured: &LayoutNode,
    padding: f64,
    look_is_neo: bool,
    geometry: Option<PreparedFlowchartNodeGeometry>,
    rectangle_radii: Option<(f64, f64)>,
) -> Result<ShapePortProfile> {
    let name = node.layout_shape.as_deref().unwrap_or("squareRect");
    let shape = resolve_flowchart_shape(name)?;
    if let Some(geometry) = geometry {
        return Ok(match geometry {
            PreparedFlowchartNodeGeometry::Rectangle(rectangle) => {
                let (rx, ry) = rectangle_radii.unwrap_or(match shape {
                    S::RoundedRectangle => (5.0, 5.0),
                    _ => (0.0, 0.0),
                });
                let bounds = rectangle.shape;
                let center = bounds.center();
                ShapePortProfile::new(
                    Outline::rect_at(
                        (center.x, center.y),
                        bounds.width(),
                        bounds.height(),
                        rx,
                        ry,
                    ),
                    bounds.width() - 2.0 * (rx.max(8.0) + center.x.abs()),
                    bounds.height() - 2.0 * (ry.max(8.0) + center.y.abs()),
                )
            }
            PreparedFlowchartNodeGeometry::ImageSquare(image) => {
                let image_center_y = image.image_top() + image.image.height / 2.0;
                let body = Outline::rect_at(
                    (0.0, image_center_y),
                    image.image.width,
                    image.image.height,
                    0.0,
                    0.0,
                );
                let outline = if image.label_padding > 0.0 {
                    let label_y = (image.outer.height - image.label.height) / 2.0;
                    let caption = Outline::rect_at(
                        (0.0, if image.top_label { -label_y } else { label_y }),
                        image.label.width,
                        image.label.height,
                        0.0,
                        0.0,
                    );
                    Outline::Parts(vec![body, caption])
                } else {
                    body
                };
                ShapePortProfile::new(outline, image.image.width - 16.0, image.image.height - 16.0)
                    .with_anchor_y(image_center_y + image.outer.height / 2.0)
            }
        });
    }
    let (w, h) = (measured.width.max(0.0), measured.height.max(0.0));
    let (lw, lh) = (
        measured.label_width.unwrap_or(0.0),
        measured.label_height.unwrap_or(0.0),
    );
    let (x, y) = (w / 2.0, h / 2.0);
    let p = padding.max(0.0);
    let neo = look_is_neo;
    let flat = |outline| ShapePortProfile::new(outline, w - 16.0, h - 16.0);
    let fixed = |outline| ShapePortProfile::new(outline, 0.0, 0.0);
    let circle = |radius| ShapePortProfile::ellipse(2.0 * radius, 2.0 * radius);
    let profile = match shape {
        S::Process => match rectangle_radii {
            Some((rx, ry)) => ShapePortProfile::new(
                Outline::rect(w, h, rx, ry),
                w - 2.0 * rx.max(8.0),
                h - 2.0 * ry.max(8.0),
            ),
            None => ShapePortProfile::rectangle(w, h),
        },
        S::Subroutine
        | S::DividedRectangle
        | S::WindowPane
        | S::ShadedProcess
        | S::TaggedRectangle
        | S::Note
        | S::Datastore
        | S::Text => ShapePortProfile::rectangle(w, h),
        S::RoundedRectangle => {
            let (rx, ry) = rectangle_radii.unwrap_or((5.0, 5.0));
            ShapePortProfile::rounded_rectangle(w, h, rx, ry)
        }
        S::Browser | S::Console => ShapePortProfile::rounded_rectangle(w, h, 12.0, 12.0),
        S::CollapsedGroup => ShapePortProfile::rounded_rectangle(w, h, 8.0, 8.0),
        S::Stadium => {
            let shape = StadiumGeometry {
                width: w,
                height: h,
            };
            ShapePortProfile::new(
                Outline::Polygon(shape.outline()),
                w - h,
                h * ROUNDED_PORT_FRACTION,
            )
        }
        S::Anchor => circle(1.0),
        S::FilledCircle | S::SmallCircle | S::FramedCircle => circle(7.0),
        S::CrossedCircle => circle(super::CROSSED_CIRCLE_RADIUS),
        S::Circle => circle(x.min(y).max(0.5)),
        S::DoubleCircle => {
            circle(DoubleCircleGeometry::from_outer_diameter(w.min(h), neo).outer_radius)
        }
        S::Ellipse => ShapePortProfile::ellipse(w, h),
        // basic_geoms::render_diamond translates the polygon half a pixel to the right.
        S::Diamond => ShapePortProfile::new(Outline::diamond(w, h, 0.5), w * 0.6, h * 0.6),
        S::Choice => ShapePortProfile::diamond(28.0, 28.0),
        S::ForkJoin => fork_join(w, h),
        S::Hexagon => {
            let hex = HexagonGeometry::from_bounds(w, h, neo);
            let shoulder = hex.points[0].0;
            ShapePortProfile::new(
                Outline::polygon_at(hex.points, -x, y),
                w - 2.0 * shoulder - 8.0,
                h * 0.5,
            )
        }
        S::LeanLeft => lean(LeanKind::Left, w, h),
        S::LeanRight => lean(LeanKind::Right, w, h),
        S::Trapezoid => lean(LeanKind::Trapezoid, w, h),
        S::InvertedTrapezoid => lean(LeanKind::InvertedTrapezoid, w, h),
        S::Odd => {
            let g = OddGeometry::from_bounds(w, h);
            ShapePortProfile::new(
                Outline::polygon_at(g.points, g.shift_x, 0.0),
                w - h / 4.0 - 16.0,
                0.0,
            )
        }
        S::Triangle => fixed(Outline::Polygon(vec![(-x, y), (x, y), (0.0, -y)])),
        S::ManualFile => fixed(Outline::Polygon(vec![(-x, -y), (x, -y), (0.0, y)])),
        S::Hourglass => fixed(Outline::Polygon(vec![(-x, -y), (x, -y), (-x, y), (x, y)])),
        S::LightningBolt => {
            let (x, y) = (x.max(17.5), y.max(35.0));
            fixed(Outline::Polygon(vec![
                (x, -y),
                (-x, 3.5),
                (x - 14.0, 3.5),
                (-x, y),
                (x, -3.5),
                (-x + 14.0, -3.5),
            ]))
        }
        S::ManualInput => fixed(Outline::Polygon(vec![
            (-x, -y / 3.0),
            (-x, y),
            (x, y),
            (x, -y),
        ])),
        S::NotchedRectangle => flat(Outline::Polygon(vec![
            (-x + 12.0, -y),
            (x, -y),
            (x, y),
            (-x, y),
            (-x, -y + 12.0),
        ])),
        S::NotchedPentagon => flat(Outline::Polygon(vec![
            (-x * 0.8, -y),
            (x * 0.8, -y),
            (x, -y * 0.6),
            (x, y),
            (-x, y),
            (-x, -y * 0.6),
        ])),
        S::Cylinder | S::LinedCylinder => {
            let ry = x / (2.5 + w / 50.0);
            ShapePortProfile::new(Outline::rect(w, h, x, ry), w * 0.6, h - 2.0 * ry - 16.0)
        }
        S::HorizontalCylinder => {
            let h = (lh + if neo { 12.0 } else { p / 2.0 }).max(1.0);
            let ry = h / 2.0;
            let rx = ry / (2.5 + h / 50.0);
            let mid = (lw + rx + if neo { 12.0 } else { p / 2.0 }).max(1.0);
            ShapePortProfile::new(
                Outline::rect(mid + 2.0 * rx, h, rx, ry),
                mid - 16.0,
                h * 0.6,
            )
        }
        S::Delay => fixed(Outline::Polygon(
            DelayGeometry::from_label(lw, lh, p, neo).points,
        )),
        S::CurvedTrapezoid => {
            let g = DisplayGeometry::from_label(lw, lh, p, neo);
            fixed(Outline::polygon_at(g.points, g.tx, g.ty))
        }
        S::BowTieRectangle => {
            let w = lw + 2.0 * if neo { 16.0 } else { p };
            let h = lh + if neo { 12.0 } else { p };
            let (x, y) = (w / 2.0, h / 2.0);
            let rx = y / (2.5 + h / 50.0);
            // Both sampled half-ellipses bow left; the right face is concave.
            let mut pts = vec![(x, -y)];
            sampled_arc(&mut pts, (-x, 0.0), (rx, y), -FRAC_PI_2, -PI, 20);
            sampled_arc(&mut pts, (x, 0.0), (rx, y), FRAC_PI_2, PI, 20);
            fixed(Outline::polygon_at(pts, rx / 2.0, 0.0))
        }
        S::StackedRectangle => {
            let d = if neo { 10.0 } else { 5.0 };
            // The rear layers are displaced top-right. Their union leaves two staircase corners.
            let points = vec![
                (-x, -y + 2.0 * d),
                (-x, y),
                (x - 2.0 * d, y),
                (x - 2.0 * d, y - d),
                (x - d, y - d),
                (x - d, y - 2.0 * d),
                (x, y - 2.0 * d),
                (x, -y),
                (-x + 2.0 * d, -y),
                (-x + 2.0 * d, -y + d),
                (-x + d, -y + d),
                (-x + d, -y + 2.0 * d),
            ];
            ShapePortProfile::new(
                Outline::Polygon(points),
                w - 2.0 * d - 16.0,
                h - 2.0 * d - 16.0,
            )
        }
        S::StackedDocument => {
            let g = flowchart_stacked_document_geometry(lw, lh, p, neo);
            fixed(Outline::polygon_at(g.outer_points, 0.0, g.group_dy))
        }
        S::Document | S::LinedDocument | S::TaggedDocument => {
            fixed(document(shape, lw, lh, p, neo))
        }
        S::PaperTape => {
            let x = lw / 2.0 + if neo { 16.0 } else { p };
            let h = lh + if neo { 20.0 } else { p };
            let a = h / 8.0;
            let y = h / 2.0 + a;
            let mut points = wave(-x, x, y, a, 1.0);
            points.extend(wave(x, -x, -y, a, -1.0));
            fixed(Outline::Polygon(points))
        }
        S::Folder => {
            let tab_h = ((lh + 2.0 * p) * 0.16).clamp(8.0, 14.0).min(h);
            let notch = -x + (w * 0.38).max(28.0);
            fixed(Outline::Polygon(vec![
                (-x, -y),
                (notch, -y),
                (notch, -y + tab_h),
                (x, -y + tab_h),
                (x, y),
                (-x, y),
            ]))
        }
        S::Bucket => {
            let rim = (w * 0.08).clamp(5.0, 12.0);
            let top = -y + rim;
            let base = x * 0.72;
            fixed(Outline::Path {
                start: Point::new(-x, top),
                edges: vec![
                    Edge::Line(Point::new(-base, y)),
                    Edge::Arc(
                        Arc::new((0.0, y), (base, rim), PI, -PI, 0.0),
                        Point::new(base, y),
                    ),
                    Edge::Line(Point::new(x, top)),
                    Edge::Arc(
                        Arc::new((0.0, top), (x, rim), 0.0, -PI, 0.0),
                        Point::new(-x, top),
                    ),
                ],
            })
        }
        S::Person => {
            let r = (w * 0.23).clamp(16.0, 56.0);
            let body_h = (h - 2.0 * r + 0.27 * r).max(lh + 2.0 * p);
            let body_r = (w * 0.177).min(body_h * 0.45);
            fixed(Outline::Parts(vec![
                Outline::rect_at((0.0, -y + r), 2.0 * r, 2.0 * r, r, r),
                Outline::rect_at((0.0, y - body_h / 2.0), w, body_h, body_r, body_r),
            ]))
        }
        S::Bang => fixed(organic(bang_geometry(lw, lh, p))),
        S::Cloud => fixed(organic(cloud_geometry(lw, lh, p))),
        S::BraceLeft | S::BraceRight | S::Braces => fixed(brace(shape, name, lw, lh, p, neo)),
        S::Bpmn(bpmn) => bpmn_profile(bpmn, w, h, lw, lh),
        S::Icon | S::IconCircle | S::IconRounded | S::IconSquare => asset(shape, node, measured, p),
        S::ImageSquare => ShapePortProfile::rectangle(w, h),
    };
    Ok(profile)
}

fn lean(kind: LeanKind, w: f64, h: f64) -> ShapePortProfile {
    let g = LeanGeometry::from_bounds(kind, w, h);
    ShapePortProfile::new(
        Outline::polygon_at(g.points, g.translate_x, g.translate_y),
        w - h / 2.0 - 16.0,
        0.0,
    )
}

fn organic(g: OrganicShapeGeometry) -> Outline {
    let start = Point::new(g.translate_x, g.translate_y);
    let mut at = start;
    let mut edges = Vec::with_capacity(g.arcs.len());
    for a in g.arcs {
        let next = at + Vec2::new(a.dx, a.dy);
        let arc = Arc::from_svg_arc(&SvgArc {
            from: at,
            to: next,
            radii: Vec2::new(a.rx, a.ry),
            x_rotation: a.x_axis_rotation_deg.to_radians(),
            large_arc: a.large_arc,
            sweep: a.sweep,
        });
        edges.push(match arc {
            Some(arc) => Edge::Arc(arc, next),
            None => Edge::Line(next),
        });
        at = next;
    }
    Outline::Path { start, edges }
}

// The wave is painted as 50 straight segments. Normal projection must intersect those actual
// chords, including between samples, rather than the continuous sine used to generate them.
fn wave(from: f64, to: f64, y: f64, amplitude: f64, cycles: f64) -> Vec<(f64, f64)> {
    (0..=50)
        .map(|i| {
            let t = f64::from(i) / 50.0;
            (
                from + (to - from) * t,
                y + amplitude * (TAU * cycles * t).sin(),
            )
        })
        .collect()
}

fn document(shape: S, lw: f64, lh: f64, p: f64, neo: bool) -> Outline {
    let (px, py) = if neo && shape != S::TaggedDocument {
        (16.0, 12.0)
    } else {
        (p, p)
    };
    let w = lw + 2.0 * px;
    let h = lh + 2.0 * py;
    let a = h / if neo && shape != S::TaggedDocument {
        4.0
    } else {
        8.0
    };
    let x = if shape == S::Document {
        w.max(14.0) / 2.0
    } else {
        w * 0.55
    };
    let mut points = wave(-x, x, h / 2.0, a, 0.8);
    let top = -h / 2.0 - a;
    points.extend([(x, top), (-x, top)]);
    let main = Outline::Polygon(points);
    if shape == S::TaggedDocument {
        // The small second filled path can extend below the main wave near the tag.
        let tag_h = 0.2 * h;
        let baseline = h - (h + a) / 2.0 - tag_h * 0.4;
        let tag_x = x - w * 0.2;
        let mut tag = vec![
            (tag_x, baseline * 1.3),
            (x, baseline - tag_h),
            (x, baseline * 0.9),
        ];
        tag.extend(wave(
            x,
            tag_x,
            baseline * (1.25 + 1.3) / 2.0,
            -h * 0.02,
            0.5,
        ));
        Outline::Parts(vec![main, Outline::polygon_at(tag, 0.0, -a / 2.0)])
    } else if shape == S::LinedDocument {
        let rule_bottom = (h + a) * 0.55 - a / 2.0;
        Outline::Parts(vec![
            main,
            Outline::Polygon(vec![(-w / 2.0, top), (-w / 2.0, rule_bottom)]),
        ])
    } else {
        main
    }
}

fn sampled_arc(
    out: &mut Vec<(f64, f64)>,
    center: (f64, f64),
    radii: (f64, f64),
    start: f64,
    sweep: f64,
    count: u32,
) {
    out.extend((0..count).map(|i| {
        let angle = start + sweep * f64::from(i) / f64::from(count - 1);
        (
            center.0 + radii.0 * angle.cos(),
            center.1 + radii.1 * angle.sin(),
        )
    }));
}

fn brace(shape: S, name: &str, lw: f64, lh: f64, p: f64, neo: bool) -> Outline {
    let (w, h) = flowchart_brace_content_dimensions(name, lw, lh, p, neo);
    let r = (h * 0.1).max(5.0);
    let y = h / 2.0;
    // The bracket sides and their label enclosure form the attachment silhouette. The two
    // facing sampled bends meet at a cusp, so a center ray cannot preserve a spread port.
    let side = |x: f64, sign: f64| {
        let mut pts = Vec::with_capacity(92);
        sampled_arc(&mut pts, (0.0, -y), (r, r), -FRAC_PI_2, FRAC_PI_2, 20);
        sampled_arc(&mut pts, (2.0 * r, -r), (r, r), PI, -FRAC_PI_2, 20);
        sampled_arc(&mut pts, (2.0 * r, r), (r, r), -FRAC_PI_2, -FRAC_PI_2, 20);
        sampled_arc(
            &mut pts,
            (0.0, y),
            (r, r),
            0.0,
            FRAC_PI_2,
            if sign < 0.0 { 30 } else { 20 },
        );
        pts.into_iter().map(move |(px, py)| (x + sign * px, py))
    };
    let mut points = Vec::new();
    if shape != S::BraceLeft {
        points.extend(side(
            w / 2.0 - if shape == S::Braces { 1.5 * r } else { 0.0 },
            1.0,
        ));
    } else {
        points.extend([(w / 2.0, -y - r), (w / 2.0, y + r)]);
    }
    if shape != S::BraceRight {
        points.extend(side(-w / 2.0, -1.0).rev());
    } else {
        points.extend([(-w / 2.0, y + r), (-w / 2.0, -y - r)]);
    }
    let dx = if shape == S::Braces {
        0.75 * r
    } else if shape == S::BraceRight {
        -r
    } else {
        r
    };
    Outline::polygon_at(points, dx, 0.0)
}

fn bpmn_profile(shape: BpmnShape, w: f64, h: f64, lw: f64, lh: f64) -> ShapePortProfile {
    use crate::bpmn::{EVENT_DIAMETER, GATEWAY_SIZE};
    match shape {
        BpmnShape::Start | BpmnShape::End | BpmnShape::Boundary | BpmnShape::Intermediate => {
            ShapePortProfile::ellipse(EVENT_DIAMETER, EVENT_DIAMETER)
        }
        BpmnShape::Gateway => ShapePortProfile::diamond(GATEWAY_SIZE, GATEWAY_SIZE),
        BpmnShape::Activity => ShapePortProfile::rounded_rectangle(w, h, 10.0, 10.0),
        BpmnShape::Data => ShapePortProfile::new(
            Outline::Polygon(vec![
                (-18.0, -25.0),
                (8.0, -25.0),
                (18.0, -15.0),
                (18.0, 25.0),
                (-18.0, 25.0),
            ]),
            20.0,
            34.0,
        ),
        BpmnShape::DataStore => {
            ShapePortProfile::new(Outline::rect(50.0, 50.0, 25.0, 6.0), 30.0, 22.0)
        }
        // The open bracket and the label have independent extents. The transparent bpmn-bounds
        // rectangle can be much wider than the first painted object on a right-hand port ray.
        BpmnShape::Annotation => {
            let (left, y) = (-w / 2.0, h / 2.0);
            let mut ink = vec![
                Outline::Polygon(vec![(left, -y), (left, y)]),
                Outline::Polygon(vec![(left, -y), (left + 6.0, -y)]),
                Outline::Polygon(vec![(left, y), (left + 6.0, y)]),
            ];
            if lw > 0.0 && lh > 0.0 {
                ink.push(Outline::rect_at((3.0, 0.0), lw, lh, 0.0, 0.0));
            }
            ShapePortProfile::new(Outline::Parts(ink), 0.0, 0.0)
        }
    }
}

fn asset(shape: S, node: &FlowNode, measured: &LayoutNode, p: f64) -> ShapePortProfile {
    let size = node
        .asset_width
        .unwrap_or(48.0)
        .max(node.asset_height.unwrap_or(48.0));
    let lw = measured.label_width.unwrap_or(0.0);
    let lh = measured.label_height.unwrap_or(0.0);
    let has_label = lw > 0.0 || lh > 0.0;
    let (w, h, rx, ry) = match shape {
        S::IconCircle => {
            let ink = if node.icon.as_deref().is_some_and(|s| !s.trim().is_empty()) {
                size
            } else {
                0.0
            };
            let d = ink * std::f64::consts::SQRT_2 + 40.0;
            (d, d, d / 2.0, d / 2.0)
        }
        S::Icon => (size, size, 0.0, 0.0),
        S::IconSquare => (size + p, size + p, 0.1, 0.1),
        S::IconRounded => (size + p, size + p, 5.0, 5.0),
        _ => unreachable!("asset shape selected by the Flowchart shape table"),
    };
    let shift = if node.pos.as_deref() == Some("t") {
        1.0
    } else {
        -1.0
    };
    let cy = shift * if has_label { (lh + 12.0) / 2.0 } else { 0.0 };
    let body = Outline::rect_at((0.0, cy), w, h, rx, ry);
    let outline = if has_label {
        let caption = Outline::rect_at(
            (0.0, -shift * (h + 8.0) / 2.0),
            lw + 4.0,
            lh + 4.0,
            0.0,
            0.0,
        );
        Outline::Parts(vec![body, caption])
    } else {
        body
    };
    let profile = if shape == S::IconCircle {
        ShapePortProfile::new(outline, w * 0.55, h * 0.55)
    } else {
        ShapePortProfile::new(outline, w - 16.0, h - 16.0)
    };
    profile.with_anchor_y(measured.height / 2.0 + cy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_geometry::{ImageSquareGeometry, ImageSquareSizing};
    use merman_render_core::LayoutPoint;
    use merman_render_core::route_types::{Axis, RoutePoint, Side};

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
        from_paint_metrics(&node, &measured, 15.0, neo, None, None).unwrap()
    }
    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} vs {b}");
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
    fn cloud_keeps_asymmetric_normal_contacts() {
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
        let image_geometry = ImageSquareGeometry::resolve(
            ImageSquareSizing {
                href: node.img.as_deref().unwrap(),
                width: node.asset_width,
                height: node.asset_height,
                constraint_on: false,
            },
            merman_render_core::text::TextMetrics {
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
        let image = from_paint_metrics(
            &node,
            &measured,
            15.0,
            false,
            Some(PreparedFlowchartNodeGeometry::ImageSquare(image_geometry)),
            None,
        )
        .unwrap();
        close(image.contact(&c, Side::Right, h(122.0, 16.0)).x, 80.0);
        close(image.contact(&c, Side::Right, h(122.0, -36.0)).x, 122.0);
        for name in ["process", "rounded"] {
            node.layout_shape = Some(name.into());
            let shape = from_paint_metrics(&node, &measured, 15.0, false, None, Some((30.0, 10.0)))
                .unwrap();
            close(shape.contact(&c, Side::Top, v(110.0, -46.0)).y, -44.0);
            close(shape.port_span(Side::Top), 184.0);
        }
    }
}

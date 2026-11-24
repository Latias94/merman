use super::{Outline, ShapePortProfile};
use crate::graph_layout::ir::{Axis, RoutePoint, Side};
use crate::model::LayoutPoint;
use crate::usecase::actor::{self, ActorPlacement};
use kurbo::{BezPath, PathEl, flatten};
use merman_model::families::usecase::UsecaseActorType;

fn glyph(parts: &mut Vec<Outline>, path: &str, dy: f64) {
    let path = BezPath::from_svg(path).expect("static Usecase actor path");
    let mut points = Vec::new();
    flatten(path, 0.02, |element| match element {
        PathEl::MoveTo(point) => {
            if !points.is_empty() {
                parts.push(Outline::Polygon(std::mem::take(&mut points)));
            }
            points.push((point.x, point.y + dy));
        }
        PathEl::LineTo(point) => points.push((point.x, point.y + dy)),
        PathEl::ClosePath => parts.push(Outline::Polygon(std::mem::take(&mut points))),
        _ => unreachable!("flatten emits only line segments"),
    });
    if !points.is_empty() {
        parts.push(Outline::Polygon(points));
    }
}

fn rounded_box(x: f64, y: f64, width: f64, height: f64, radius: f64) -> Outline {
    Outline::Box {
        cx: x,
        cy: y,
        hw: width / 2.0,
        hh: height / 2.0,
        rx: radius,
        ry: radius,
    }
}

impl ShapePortProfile {
    pub(crate) fn usecase_actor(
        variant: UsecaseActorType,
        placement: ActorPlacement,
        label: (f64, f64),
        stereotype: Option<(f64, f64)>,
    ) -> Self {
        let mut parts = Vec::new();
        let (span_x, span_y) = match variant {
            UsecaseActorType::Normal => {
                glyph(&mut parts, actor::NORMAL, placement.glyph_y);
                (20.0, 26.0)
            }
            UsecaseActorType::Hollow => {
                let diameter = 2.0 * actor::HOLLOW_HEAD_RADIUS;
                parts.push(rounded_box(
                    0.0,
                    placement.glyph_y + actor::HOLLOW_HEAD_Y,
                    diameter,
                    diameter,
                    actor::HOLLOW_HEAD_RADIUS,
                ));
                glyph(&mut parts, actor::HOLLOW_BODY, placement.glyph_y);
                (15.0, 26.0)
            }
            UsecaseActorType::Awesome => {
                glyph(&mut parts, actor::AWESOME, placement.glyph_y);
                (22.0, 30.0)
            }
            UsecaseActorType::Icon => {
                parts.push(rounded_box(
                    0.0,
                    placement.glyph_y + actor::ICON_Y + actor::ICON_HEIGHT / 2.0,
                    actor::ICON_WIDTH,
                    actor::ICON_HEIGHT,
                    actor::ICON_CORNER,
                ));
                (
                    actor::ICON_WIDTH - 2.0 * actor::ICON_CORNER,
                    actor::ICON_HEIGHT - 2.0 * actor::ICON_CORNER,
                )
            }
        };
        let mut top = placement.caption_top;
        if let Some((width, height)) = stereotype {
            if width > 0.0 && height > 0.0 {
                parts.push(rounded_box(0.0, top + height / 2.0, width, height, 0.0));
            }
            top += height + 2.0;
        }
        if label.0 > 0.0 && label.1 > 0.0 {
            parts.push(rounded_box(0.0, top + label.1 / 2.0, label.0, label.1, 0.0));
        }
        Self::new(Outline::Parts(parts), span_x, span_y)
    }

    pub(crate) fn fit_usecase_actor_loop(
        &self,
        points: &mut [RoutePoint],
        center: LayoutPoint,
        sides: (Side, Side),
    ) {
        if let [start, departure, .., arrival, end] = points {
            self.fit_actor_terminal(&center, sides.0, start, departure);
            self.fit_actor_terminal(&center, sides.1, end, arrival);
        }
    }

    fn fit_actor_terminal(
        &self,
        center: &LayoutPoint,
        side: Side,
        port: &mut RoutePoint,
        neighbor: &mut RoutePoint,
    ) {
        if self.try_contact(center, side, *port).is_some() {
            return;
        }
        let Outline::Parts(parts) = &self.outline else {
            return;
        };
        let vertical = side.axis() == Axis::Vertical;
        let (origin, across) = if vertical {
            (center.x, port.x - center.x)
        } else {
            (center.y, port.y - center.y)
        };
        let nearest = parts
            .iter()
            .filter_map(|part| match part {
                Outline::Box { cx, cy, hw, hh, .. } => {
                    let (center, radius) = if vertical { (cx, hw) } else { (cy, hh) };
                    Some((center - radius, center + radius))
                }
                Outline::Polygon(points) => {
                    let mut coords = points.iter().map(|&(x, y)| if vertical { x } else { y });
                    let first = coords.next()?;
                    Some(coords.fold((first, first), |(min, max), coord| {
                        (min.min(coord), max.max(coord))
                    }))
                }
                _ => None,
            })
            .map(|(min, max)| {
                let inset = ((max - min) / 2.0).min(0.125);
                across.clamp(min + inset, max - inset)
            })
            .min_by(|left, right| (left - across).abs().total_cmp(&(right - across).abs()));
        if let Some(nearest) = nearest {
            if vertical {
                port.x = origin + nearest;
                neighbor.x = port.x;
            } else {
                port.y = origin + nearest;
                neighbor.y = port.y;
            }
        }
    }
}

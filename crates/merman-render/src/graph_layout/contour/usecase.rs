use crate::usecase::actor::{self, ActorPlacement};
use kurbo::{BezPath, PathEl, flatten};
use merman_model::families::usecase::UsecaseActorType;
use merman_render_core::graph_layout::contour::{Outline, ShapePortProfile};

fn glyph(parts: &mut Vec<Outline>, path: &BezPath, dy: f64) {
    let mut points = Vec::new();
    flatten(
        path.elements().iter().copied(),
        0.02,
        |element| match element {
            PathEl::MoveTo(point) => {
                if !points.is_empty() {
                    parts.push(Outline::Polygon(std::mem::take(&mut points)));
                }
                points.push((point.x, point.y + dy));
            }
            PathEl::LineTo(point) => points.push((point.x, point.y + dy)),
            PathEl::ClosePath => parts.push(Outline::Polygon(std::mem::take(&mut points))),
            _ => unreachable!("flatten emits only line segments"),
        },
    );
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

pub(crate) fn usecase_actor(
    variant: UsecaseActorType,
    placement: ActorPlacement,
    label: (f64, f64),
    stereotype: Option<(f64, f64)>,
) -> ShapePortProfile {
    let mut parts = Vec::new();
    if let Some(path) = actor::body_path(variant) {
        glyph(&mut parts, path, placement.glyph_y);
    }
    let (span_x, span_y) = match variant {
        UsecaseActorType::Normal => (20.0, 26.0),
        UsecaseActorType::Hollow => {
            let diameter = 2.0 * actor::HOLLOW_HEAD_RADIUS;
            parts.push(rounded_box(
                0.0,
                placement.glyph_y + actor::HOLLOW_HEAD_Y,
                diameter,
                diameter,
                actor::HOLLOW_HEAD_RADIUS,
            ));
            (15.0, 26.0)
        }
        UsecaseActorType::Awesome => (22.0, 30.0),
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
    ShapePortProfile::new(Outline::Parts(parts), span_x, span_y)
}

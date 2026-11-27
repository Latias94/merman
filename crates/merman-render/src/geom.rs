#![forbid(unsafe_code)]

pub type Unit = euclid::UnknownUnit;

pub type Point = euclid::Point2D<f64, Unit>;
pub type Vector = euclid::Vector2D<f64, Unit>;
pub type Size = euclid::Size2D<f64, Unit>;
pub type Rect = euclid::Rect<f64, Unit>;
pub type Transform = euclid::Transform2D<f64, Unit, Unit>;

pub use merman_render_core::euclid_box::Box2;

pub fn point(x: f64, y: f64) -> Point {
    euclid::point2(x, y)
}

pub fn vector(x: f64, y: f64) -> Vector {
    euclid::vec2(x, y)
}

#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-er",
    feature = "diagram-flowchart",
    feature = "diagram-mindmap",
    feature = "diagram-swimlane"
))]
pub(crate) fn intersect_centered_rect_ray(center: Point, size: Size, target: Point) -> Point {
    let delta = target - center;
    let half_width = size.width / 2.0;
    let half_height = size.height / 2.0;
    if delta.x == 0.0 && delta.y == 0.0 {
        return point(center.x + half_width, center.y);
    }

    let (offset_x, offset_y) = if delta.y.abs() * half_width > delta.x.abs() * half_height {
        let height = if delta.y < 0.0 {
            -half_height
        } else {
            half_height
        };
        (height * delta.x / delta.y, height)
    } else {
        let width = if delta.x < 0.0 {
            -half_width
        } else {
            half_width
        };
        let offset_y = if delta.x == 0.0 {
            0.0
        } else {
            width * delta.y / delta.x
        };
        (width, offset_y)
    };
    point(center.x + offset_x, center.y + offset_y)
}

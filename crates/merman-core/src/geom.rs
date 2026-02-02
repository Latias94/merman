#![forbid(unsafe_code)]

pub type Unit = euclid::UnknownUnit;

pub type Point = euclid::Point2D<f64, Unit>;
pub type Vector = euclid::Vector2D<f64, Unit>;
pub type Size = euclid::Size2D<f64, Unit>;
pub type Rect = euclid::Rect<f64, Unit>;
pub type Transform = euclid::Transform2D<f64, Unit, Unit>;

pub fn point(x: f64, y: f64) -> Point {
    euclid::point2(x, y)
}

pub fn vector(x: f64, y: f64) -> Vector {
    euclid::vec2(x, y)
}

use super::{Outline, ShapePortProfile};
use crate::c4::{
    C4_FRAMED_FRAME_WIDTH, C4_ROUNDED_RADIUS, C4Look, C4NodeShape, C4PersonOutline, c4_cap_radius,
};
use crate::graph_layout::ir::Size;

impl ShapePortProfile {
    /// The solved box surrounds the actual outline. The final value is the horizontal offset
    /// of its center relative to the C4 shape's paint origin.
    pub(crate) fn c4_shape(kind: C4NodeShape, look: C4Look, size: Size) -> (Self, Size, f64) {
        let width = size.width.max(1.0);
        let height = size.height.max(1.0);
        let (profile, width, height, dx) = match kind {
            C4NodeShape::Rounded => (
                Self::rounded_rectangle(width, height, C4_ROUNDED_RADIUS, C4_ROUNDED_RADIUS),
                width,
                height,
                0.0,
            ),
            C4NodeShape::Framed if look.is_hand_drawn() => (
                Self::rounded_rectangle(width, height, C4_ROUNDED_RADIUS, C4_ROUNDED_RADIUS),
                width,
                height,
                0.0,
            ),
            C4NodeShape::Framed => (
                Self::rectangle(width, height),
                width,
                height,
                -C4_FRAMED_FRAME_WIDTH,
            ),
            C4NodeShape::Cylinder => {
                let cap = c4_cap_radius(width);
                let height = (height - 2.0 * cap).max(1.0) + 2.0 * cap;
                let mut profile = Self::rounded_rectangle(width, height, width / 2.0, cap);
                // Several ports can use the same elliptical cap at distinct cross positions.
                profile.span[0] = width * 0.8;
                (profile, width, height, 0.0)
            }
            C4NodeShape::HorizontalCylinder => {
                let cap = c4_cap_radius(height);
                let width = width + 2.0 * cap;
                let mut profile = Self::rounded_rectangle(width, height, cap, height / 2.0);
                profile.span[1] = height * 0.8;
                (profile, width, height, 0.0)
            }
            C4NodeShape::Person => {
                let person = C4PersonOutline::new(width, height);
                let body_y = person.body_top + person.body_height / 2.0;
                let height =
                    person.body_top + person.body_height - (person.head_y - person.head_radius);
                let mut profile = Self::new(
                    Outline::Parts(vec![
                        Outline::rect_at(
                            (0.0, person.head_y),
                            2.0 * person.head_radius,
                            2.0 * person.head_radius,
                            person.head_radius,
                            person.head_radius,
                        ),
                        Outline::rect_at(
                            (0.0, body_y),
                            width,
                            person.body_height,
                            person.body_radius,
                            person.body_radius,
                        ),
                    ]),
                    1.5 * person.head_radius,
                    person.body_height - 2.0 * person.body_radius,
                );
                profile.anchor_y = Some(body_y + height / 2.0);
                (profile, width, height, 0.0)
            }
        };
        (profile, Size { width, height }, dx)
    }
}

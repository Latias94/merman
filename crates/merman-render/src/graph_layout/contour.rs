//! Bind renderer contours to the layout adapter.

#[cfg(any(
    feature = "layout-shoal",
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
pub(crate) use merman_render_core::graph_layout::contour::ShapePortProfile;
#[cfg(all(feature = "diagram-c4", feature = "layout-shoal"))]
#[path = "../c4/contour.rs"]
mod c4;
#[cfg(all(feature = "diagram-c4", feature = "layout-shoal"))]
pub(crate) use c4::c4_shape;
#[cfg(all(feature = "diagram-usecase", feature = "layout-shoal"))]
mod usecase;
#[cfg(all(feature = "diagram-usecase", feature = "layout-shoal"))]
pub(crate) use usecase::usecase_actor;

#[cfg(feature = "layout-shoal")]
pub(crate) fn shoal_contour(
    profile: &ShapePortProfile,
    size: super::ir::Size,
) -> merman_layout_shoal::ShapeContourHint {
    use merman_layout_shoal::ShapeContourHint;
    use merman_render_core::graph_layout::contour::Outline;

    let (width, height) = (size.width / 2.0, size.height / 2.0);
    match profile.outline() {
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
            if let &[(x, top), (right, y), (bottom_x, bottom), (left, left_y)] = points.as_slice()
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

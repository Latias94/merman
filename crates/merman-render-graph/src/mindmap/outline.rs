use super::MindmapNodeModel;
use kurbo::Point;
use merman_render_core::LayoutNode;

pub(super) fn needs_clip(model: &MindmapNodeModel) -> bool {
    // The other Mindmap shapes return the bounding rectangle from `intersect`.
    matches!(model.shape.as_str(), "mindmapCircle" | "hexagon")
}

pub(super) fn intersect_segment(
    node: &LayoutNode,
    model: &MindmapNodeModel,
    origin: Point,
    toward: Point,
) -> Option<Point> {
    let center = Point::new(node.x, node.y);
    let offset = origin - center;
    let delta = toward - origin;
    if model.shape == "mindmapCircle" {
        let radius = (node.width.max(node.height) / 2.0).max(1.0);
        let squared = radius * radius;
        let a = delta.x * delta.x / squared + delta.y * delta.y / squared;
        let b = 2.0 * offset.x * delta.x / squared + 2.0 * offset.y * delta.y / squared;
        let c = offset.x * offset.x / squared + offset.y * offset.y / squared - 1.0;
        let discriminant = b * b - 4.0 * a * c;
        if a <= 0.0 || discriminant < 0.0 {
            return None;
        }
        let root = discriminant.sqrt();
        return [(-b + root) / (2.0 * a), (-b - root) / (2.0 * a)]
            .into_iter()
            .find(|parameter| (0.0..=1.0).contains(parameter))
            .map(|parameter| origin + delta * parameter);
    }

    // The Mindmap hexagon writer uses a taper of one quarter of the painted height.
    let half_width = node.width / 2.0;
    let half_height = node.height / 2.0;
    let taper = half_height / 2.0;
    if delta.x == 0.0 && offset.x.abs() <= half_width {
        let extent = half_height * ((half_width - offset.x.abs()) / taper).min(1.0);
        Some(Point::new(toward.x, center.y + extent.copysign(delta.y)))
    } else if delta.y == 0.0 && offset.y.abs() <= half_height {
        let extent = half_width - taper * offset.y.abs() / half_height;
        Some(Point::new(center.x + extent.copysign(delta.x), toward.y))
    } else {
        None
    }
}

#[cfg(feature = "layout-layered")]
pub(super) fn intersect_ray(node: &LayoutNode, model: &MindmapNodeModel, toward: Point) -> Point {
    let center = Point::new(node.x, node.y);
    let delta = toward - center;
    if model.shape == "mindmapCircle" {
        let radius = (node.width.max(node.height) / 2.0).max(1.0);
        return center + delta * (radius / delta.hypot());
    }

    // A ray exits the hexagon through its top/bottom or one of its inclined sides.
    let height = node.height / 2.0;
    let width = node.width / 2.0;
    let taper = height / 2.0;
    let vertical = delta.y.abs() / height;
    let oblique = (delta.x.abs() + taper * vertical) / width;
    let scale = vertical.max(oblique);
    if scale < 1.0 {
        // Mermaid's polygon segment intersection returns the center for an interior target.
        center
    } else {
        center + delta / scale
    }
}

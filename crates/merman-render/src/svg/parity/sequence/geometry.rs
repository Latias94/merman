use super::super::*;

pub(super) fn node_left_top(n: &LayoutNode) -> (f64, f64) {
    (n.x - n.width / 2.0, n.y - n.height / 2.0)
}

pub(super) fn lifeline_endpoints(
    top: &LayoutNode,
    bottom: &LayoutNode,
    edge: Option<&crate::model::LayoutEdge>,
) -> (f64, f64) {
    let (_, top_y) = node_left_top(top);
    let (_, bottom_y) = node_left_top(bottom);
    edge.and_then(|edge| Some((edge.points.first()?.y, edge.points.get(1)?.y)))
        .unwrap_or((top_y + top.height, bottom_y))
}

pub(super) fn actor_label_dy(index: usize, line_count: usize, font_size: f64) -> f64 {
    let count = line_count.max(1) as f64;
    if count <= 1.0 {
        0.0
    } else {
        (index as f64 - (count - 1.0) / 2.0) * font_size
    }
}

pub(super) fn note_label_position(
    left: f64,
    top: f64,
    width: f64,
    margin: f64,
    alignment: Option<&str>,
) -> ([f64; 2], &'static str) {
    let (x, anchor) = match alignment {
        Some("left" | "start") => ((left + margin).round(), "start"),
        Some("right" | "end") => ((left + width - margin).round(), "end"),
        _ => (left + width / 2.0, "middle"),
    };
    (
        [x, crate::sequence::sequence_drawn_text_first_y(top, margin)],
        anchor,
    )
}

pub(super) fn message_label_x(
    prepared_center: f64,
    from_x: f64,
    to_x: f64,
    bounded_width: f64,
    padding: f64,
    alignment: &str,
) -> (f64, &'static str) {
    let left = from_x.min(to_x);
    match alignment {
        "right" => (left + bounded_width - padding, "end"),
        "left" => (left + padding, "start"),
        _ => (prepared_center, "middle"),
    }
}

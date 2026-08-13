//! Flowchart edge marker and class helpers.

pub(super) fn flowchart_edge_marker_end_base(
    edge: &crate::flowchart::FlowEdge,
) -> Option<&'static str> {
    match edge.edge_type.as_deref() {
        Some("double_arrow_point") => Some("pointEnd"),
        Some("double_arrow_circle") => Some("circleEnd"),
        Some("double_arrow_cross") => Some("crossEnd"),
        Some("arrow_point") => Some("pointEnd"),
        Some("arrow_cross") => Some("crossEnd"),
        Some("arrow_circle") => Some("circleEnd"),
        Some("arrow_open") => None,
        _ => Some("pointEnd"),
    }
}

pub(super) fn flowchart_edge_marker_start_base(
    edge: &crate::flowchart::FlowEdge,
) -> Option<&'static str> {
    match edge.edge_type.as_deref() {
        Some("double_arrow_point") => Some("pointStart"),
        Some("double_arrow_circle") => Some("circleStart"),
        Some("double_arrow_cross") => Some("crossStart"),
        _ => None,
    }
}

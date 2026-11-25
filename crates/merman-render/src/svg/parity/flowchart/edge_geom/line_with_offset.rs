//! Mermaid `lineWithOffset` port.
//!
//! Mermaid shortens edge paths so markers don't render on top of the line (see
//! `packages/mermaid/src/utils/lineWithOffset.ts`).

use crate::svg::parity::edge_path::{
    line_with_offset_points, rounded_line_with_marker_offsets_points,
};

pub(in crate::svg::parity::flowchart) fn arrow_types_for_edge(
    edge_type: Option<&str>,
) -> (Option<&'static str>, Option<&'static str>) {
    let arrow_type_start = match edge_type {
        Some("double_arrow_point") => Some("arrow_point"),
        Some("double_arrow_circle") => Some("arrow_circle"),
        Some("double_arrow_cross") => Some("arrow_cross"),
        _ => None,
    };
    let arrow_type_end = match edge_type {
        Some("arrow_open") => None,
        Some("arrow_cross") => Some("arrow_cross"),
        Some("arrow_circle") => Some("arrow_circle"),
        Some("arrow_open_head") => Some("arrow_open_head"),
        Some("arrow_hollow_circle") => Some("arrow_hollow_circle"),
        Some("double_arrow_point" | "arrow_point") => Some("arrow_point"),
        Some("double_arrow_circle") => Some("arrow_circle"),
        Some("double_arrow_cross") => Some("arrow_cross"),
        _ => Some("arrow_point"),
    };

    (arrow_type_start, arrow_type_end)
}

pub(in crate::svg::parity::flowchart) fn line_with_offset_for_edge_type(
    input: &[crate::model::LayoutPoint],
    edge_type: Option<&str>,
) -> Vec<crate::model::LayoutPoint> {
    let (arrow_type_start, arrow_type_end) = arrow_types_for_edge(edge_type);
    line_with_offset_points(input, arrow_type_start, arrow_type_end)
}

pub(in crate::svg::parity::flowchart) fn rounded_line_with_marker_offsets_for_edge_type(
    input: &[crate::model::LayoutPoint],
    edge_type: Option<&str>,
) -> Vec<crate::model::LayoutPoint> {
    let (arrow_type_start, arrow_type_end) = arrow_types_for_edge(edge_type);
    rounded_line_with_marker_offsets_points(input, arrow_type_start, arrow_type_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_measurements_cover_the_painted_viewport_and_setback() {
        let points = [
            crate::model::LayoutPoint { x: 0.0, y: 0.0 },
            crate::model::LayoutPoint { x: 100.0, y: 0.0 },
        ];
        for edge_type in [
            None,
            Some("arrow_open"),
            Some("double_arrow_point"),
            Some("double_arrow_circle"),
            Some("double_arrow_cross"),
            Some("arrow_open_head"),
            Some("arrow_hollow_circle"),
        ] {
            let edge = serde_json::from_value(serde_json::json!({
                "id": "ab", "from": "a", "to": "b", "length": 1, "type": edge_type
            }))
            .unwrap();
            let (start, end) = super::super::super::flowchart_marker_metrics(&edge);
            let (start, end) = (start.trim + start.depth, end.trim + end.depth);
            let (start_marker, end_marker) = arrow_types_for_edge(edge_type);
            for path in [
                line_with_offset_for_edge_type(&points, edge_type),
                rounded_line_with_marker_offsets_for_edge_type(&points, edge_type),
            ] {
                assert!(start >= path[0].x && end >= 100.0 - path[1].x);
                assert_eq!(start > path[0].x, start_marker.is_some());
                assert_eq!(end > 100.0 - path[1].x, end_marker.is_some());
            }
        }
    }
}

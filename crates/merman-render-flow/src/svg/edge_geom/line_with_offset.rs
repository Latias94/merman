//! Cross-check Flow marker viewport measurements against both offset algorithms.

use super::arrow_types_for_edge;
use merman_render_core::svg_edge_parity::edge_path::{
    line_with_offset_points, rounded_line_with_marker_offsets_points,
};

#[test]
fn marker_measurements_cover_the_painted_viewport_and_setback() {
    let points = [
        merman_render_core::LayoutPoint { x: 0.0, y: 0.0 },
        merman_render_core::LayoutPoint { x: 100.0, y: 0.0 },
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
        let (start, end) = super::super::flowchart_marker_metrics(&edge);
        let (start, end) = (start.trim + start.depth, end.trim + end.depth);
        let (start_marker, end_marker) = arrow_types_for_edge(edge_type);
        for path in [
            line_with_offset_points(&points, start_marker, end_marker),
            rounded_line_with_marker_offsets_points(&points, start_marker, end_marker),
        ] {
            assert!(start >= path[0].x && end >= 100.0 - path[1].x);
            assert_eq!(start > path[0].x, start_marker.is_some());
            assert_eq!(end > 100.0 - path[1].x, end_marker.is_some());
        }
    }
}

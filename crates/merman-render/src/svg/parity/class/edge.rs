use super::ClassSvgRelation;
use crate::model::{LayoutEdge, LayoutLabel, LayoutPoint};
use rustc_hash::FxHashMap;

fn class_arrow_type_for_relation_end(ty: i32) -> Option<&'static str> {
    match ty {
        0 => Some("aggregation"),
        1 => Some("extension"),
        2 => Some("composition"),
        3 => Some("dependency"),
        4 => Some("lollipop"),
        _ => None,
    }
}

pub(super) fn class_line_with_marker_offset_points(
    input: &[LayoutPoint],
    relation: Option<&ClassSvgRelation>,
) -> Vec<LayoutPoint> {
    fn marker_offset_for(arrow_type: Option<&str>) -> Option<f64> {
        match arrow_type {
            Some("dependency") => Some(6.0),
            Some("lollipop") => Some(13.5),
            Some("aggregation" | "extension" | "composition") => Some(17.25),
            _ => None,
        }
    }

    fn calculate_delta_and_angle(a: &LayoutPoint, b: &LayoutPoint) -> (f64, f64, f64) {
        let delta_x = b.x - a.x;
        let delta_y = b.y - a.y;
        let angle = (delta_y / delta_x).atan();
        (angle, delta_x, delta_y)
    }

    if input.len() < 2 {
        return input.to_vec();
    }

    let arrow_type_start =
        relation.and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type1));
    let arrow_type_end =
        relation.and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type2));
    let start = &input[0];
    let end = &input[input.len() - 1];
    let x_direction_is_left = start.x < end.x;
    let y_direction_is_down = start.y < end.y;
    let extra_room = 1.0;
    let start_marker_height = marker_offset_for(arrow_type_start);
    let end_marker_height = marker_offset_for(arrow_type_end);

    let mut out = Vec::with_capacity(input.len());
    for (idx, point) in input.iter().enumerate() {
        let mut offset_x = 0.0;
        let mut offset_y = 0.0;

        if idx == 0 {
            if let Some(height) = start_marker_height {
                let (angle, delta_x, delta_y) = calculate_delta_and_angle(&input[0], &input[1]);
                offset_x = height * angle.cos() * if delta_x >= 0.0 { 1.0 } else { -1.0 };
                offset_y = height * angle.sin().abs() * if delta_y >= 0.0 { 1.0 } else { -1.0 };
            }
        } else if idx == input.len() - 1 {
            if let Some(height) = end_marker_height {
                let (angle, delta_x, delta_y) =
                    calculate_delta_and_angle(&input[input.len() - 1], &input[input.len() - 2]);
                offset_x = height * angle.cos() * if delta_x >= 0.0 { 1.0 } else { -1.0 };
                offset_y = height * angle.sin().abs() * if delta_y >= 0.0 { 1.0 } else { -1.0 };
            }
        }

        if let Some(height) = end_marker_height {
            let diff_x = (point.x - end.x).abs();
            let diff_y = (point.y - end.y).abs();
            if diff_x < height && diff_x > 0.0 && diff_y < height {
                let mut adjustment = height + extra_room - diff_x;
                adjustment *= if !x_direction_is_left { -1.0 } else { 1.0 };
                offset_x -= adjustment;
            }
        }
        if let Some(height) = start_marker_height {
            let diff_x = (point.x - start.x).abs();
            let diff_y = (point.y - start.y).abs();
            if diff_x < height && diff_x > 0.0 && diff_y < height {
                let mut adjustment = height + extra_room - diff_x;
                adjustment *= if !x_direction_is_left { -1.0 } else { 1.0 };
                offset_x += adjustment;
            }
        }

        if let Some(height) = end_marker_height {
            let diff_y = (point.y - end.y).abs();
            let diff_x = (point.x - end.x).abs();
            if diff_y < height && diff_y > 0.0 && diff_x < height {
                let mut adjustment = height + extra_room - diff_y;
                adjustment *= if !y_direction_is_down { -1.0 } else { 1.0 };
                offset_y -= adjustment;
            }
        }
        if let Some(height) = start_marker_height {
            let diff_y = (point.y - start.y).abs();
            let diff_x = (point.x - start.x).abs();
            if diff_y < height && diff_y > 0.0 && diff_x < height {
                let mut adjustment = height + extra_room - diff_y;
                adjustment *= if !y_direction_is_down { -1.0 } else { 1.0 };
                offset_y += adjustment;
            }
        }

        out.push(LayoutPoint {
            x: point.x + offset_x,
            y: point.y + offset_y,
        });
    }

    out
}

fn class_js_round(v: f64, decimals: i32) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    let factor = 10f64.powi(decimals);
    let rounded = (v * factor).round() / factor;
    if rounded == -0.0 { 0.0 } else { rounded }
}

fn class_calc_label_position(points: &[LayoutPoint]) -> Option<LayoutPoint> {
    if points.is_empty() {
        return None;
    }
    if points.len() == 1 {
        return Some(points[0].clone());
    }

    let mut total = 0.0;
    for window in points.windows(2) {
        total += (window[1].x - window[0].x).hypot(window[1].y - window[0].y);
    }
    if !total.is_finite() || total <= 0.0 {
        return Some(points[0].clone());
    }

    let mut remaining = total / 2.0;
    for window in points.windows(2) {
        let a = &window[0];
        let b = &window[1];
        let seg = (b.x - a.x).hypot(b.y - a.y);
        if !seg.is_finite() || seg <= 0.0 {
            return Some(a.clone());
        }
        if seg < remaining {
            remaining -= seg;
            continue;
        }
        let ratio = remaining / seg;
        if ratio <= 0.0 {
            return Some(a.clone());
        }
        if ratio >= 1.0 {
            return Some(LayoutPoint {
                x: class_js_round(b.x, 5),
                y: class_js_round(b.y, 5),
            });
        }
        return Some(LayoutPoint {
            x: class_js_round((1.0 - ratio) * a.x + ratio * b.x, 5),
            y: class_js_round((1.0 - ratio) * a.y + ratio * b.y, 5),
        });
    }

    Some(points[0].clone())
}

fn class_is_label_coordinate_in_path(point: &LayoutPoint, d_attr: &str) -> bool {
    let rounded_x = point.x.round() as i64;
    let rounded_y = point.y.round() as i64;
    let bytes = d_attr.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() {
        let b = bytes[idx];
        let is_start = b.is_ascii_digit() || b == b'-' || b == b'.';
        if !is_start {
            idx += 1;
            continue;
        }

        let start = idx;
        idx += 1;
        while idx < bytes.len() {
            let b = bytes[idx];
            if b.is_ascii_digit() || b == b'.' {
                idx += 1;
                continue;
            }
            break;
        }

        if let Ok(v) = d_attr[start..idx].parse::<f64>() {
            let rounded = v.round() as i64;
            if rounded == rounded_x || rounded == rounded_y {
                return true;
            }
        }
    }

    false
}

pub(super) fn class_edge_label_center(
    raw_points: &[LayoutPoint],
    d_attr: &str,
    label: &LayoutLabel,
    content_tx: f64,
    content_ty: f64,
) -> LayoutPoint {
    let mut center = LayoutPoint {
        x: label.x + content_tx,
        y: label.y + content_ty,
    };
    if let Some(mid) = raw_points.get(raw_points.len() / 2) {
        if !class_is_label_coordinate_in_path(mid, d_attr) {
            if let Some(pos) = class_calc_label_position(raw_points) {
                center = pos;
            }
        }
    }
    center
}

pub(super) fn class_edge_path_style(edge_id: &str) -> &'static str {
    if edge_id.starts_with("edgeNote") {
        "fill: none;;;fill: none"
    } else {
        ";;;"
    }
}

pub(super) fn class_edge_render_order<'a>(
    edges: &'a [LayoutEdge],
    relation_index_by_id: &FxHashMap<&str, usize>,
) -> Vec<&'a LayoutEdge> {
    let mut ordered = edges.iter().collect::<Vec<_>>();
    ordered.sort_by(|a, b| {
        let a_key = if a.id.starts_with("edgeNote") {
            (
                0_u8,
                a.id.trim_start_matches("edgeNote")
                    .parse::<usize>()
                    .unwrap_or(usize::MAX),
                a.id.as_str(),
            )
        } else {
            (
                1_u8,
                relation_index_by_id
                    .get(a.id.as_str())
                    .copied()
                    .unwrap_or(usize::MAX),
                a.id.as_str(),
            )
        };
        let b_key = if b.id.starts_with("edgeNote") {
            (
                0_u8,
                b.id.trim_start_matches("edgeNote")
                    .parse::<usize>()
                    .unwrap_or(usize::MAX),
                b.id.as_str(),
            )
        } else {
            (
                1_u8,
                relation_index_by_id
                    .get(b.id.as_str())
                    .copied()
                    .unwrap_or(usize::MAX),
                b.id.as_str(),
            )
        };
        a_key.cmp(&b_key)
    });
    ordered
}

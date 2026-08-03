use crate::model::LayoutPoint;

fn js_round(value: f64, precision: i32) -> f64 {
    if !value.is_finite() {
        return value;
    }
    let factor = 10_f64.powi(precision);
    let rounded = (value * factor + 0.5).floor() / factor;
    if rounded == 0.0 { 0.0 } else { rounded }
}

pub(super) fn calc_label_position(points: &[LayoutPoint]) -> Option<LayoutPoint> {
    match points {
        [] => return None,
        [point] => return Some(point.clone()),
        _ => {}
    }

    let total_distance = points
        .windows(2)
        .map(|segment| (segment[1].x - segment[0].x).hypot(segment[1].y - segment[0].y))
        .sum::<f64>();
    if !total_distance.is_finite() {
        return None;
    }

    let mut remaining_distance = total_distance / 2.0;
    for segment in points.windows(2) {
        let previous = &segment[0];
        let point = &segment[1];
        let vector_distance = (point.x - previous.x).hypot(point.y - previous.y);
        if vector_distance == 0.0 {
            return Some(previous.clone());
        }
        if vector_distance < remaining_distance {
            remaining_distance -= vector_distance;
            continue;
        }

        let ratio = remaining_distance / vector_distance;
        if ratio <= 0.0 {
            return Some(previous.clone());
        }
        if ratio >= 1.0 {
            return Some(point.clone());
        }
        return Some(LayoutPoint {
            x: js_round((1.0 - ratio) * previous.x + ratio * point.x, 5),
            y: js_round((1.0 - ratio) * previous.y + ratio * point.y, 5),
        });
    }

    None
}

pub(super) fn is_label_coordinate_in_path(point: &LayoutPoint, d_attr: &str) -> bool {
    let rounded_x = js_round(point.x, 0) as i64;
    let rounded_y = js_round(point.y, 0) as i64;
    let sanitized = round_decimal_numbers_in_path(d_attr);
    sanitized.contains(&rounded_x.to_string()) || sanitized.contains(&rounded_y.to_string())
}

pub(super) fn position_edge_label(
    dagre_anchor: LayoutPoint,
    rendered_points: &[LayoutPoint],
    rendered_d: &str,
    points_were_explicitly_updated: bool,
) -> LayoutPoint {
    let midpoint_missing_from_path = rendered_points
        .get(rendered_points.len() / 2)
        .is_some_and(|midpoint| !is_label_coordinate_in_path(midpoint, rendered_d));
    if points_were_explicitly_updated || midpoint_missing_from_path {
        calc_label_position(rendered_points).unwrap_or(dagre_anchor)
    } else {
        dagre_anchor
    }
}

fn round_decimal_numbers_in_path(d_attr: &str) -> String {
    let mut out = String::new();
    let mut copied_until = 0usize;
    let mut cursor = 0usize;
    let mut changed = false;

    while cursor < d_attr.len() {
        if let Some(end) = decimal_number_match_end_at(d_attr, cursor) {
            if !changed {
                out = String::with_capacity(d_attr.len());
                changed = true;
            }
            out.push_str(&d_attr[copied_until..cursor]);
            let value = d_attr[cursor..end].parse::<f64>().unwrap_or(0.0);
            out.push_str(&(js_round(value, 0) as i64).to_string());
            copied_until = end;
            cursor = end;
            continue;
        }

        let Some(character) = d_attr[cursor..].chars().next() else {
            break;
        };
        cursor += character.len_utf8();
    }

    if changed {
        out.push_str(&d_attr[copied_until..]);
        out
    } else {
        d_attr.to_string()
    }
}

fn decimal_number_match_end_at(value: &str, start: usize) -> Option<usize> {
    let digit_start = start;
    let mut cursor = consume_ascii_digits(value, start);
    if cursor == digit_start || !value.get(cursor..)?.starts_with('.') {
        return None;
    }

    let fraction_start = cursor + 1;
    cursor = consume_ascii_digits(value, fraction_start);
    (cursor != fraction_start).then_some(cursor)
}

fn consume_ascii_digits(value: &str, mut cursor: usize) -> usize {
    while value.as_bytes().get(cursor).is_some_and(u8::is_ascii_digit) {
        cursor += 1;
    }
    cursor
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_point(actual: Option<LayoutPoint>, expected_x: f64, expected_y: f64) {
        let actual = actual.expect("expected a label point");
        assert_eq!(actual.x, expected_x);
        assert_eq!(actual.y, expected_y);
    }

    #[test]
    fn requirement_curve_midpoint_matches_mermaid_11_16() {
        let points = [
            LayoutPoint {
                x: 290.96875,
                y: 381.8802782,
            },
            LayoutPoint {
                x: 228.296875,
                y: 439.0,
            },
            LayoutPoint {
                x: 199.9138505,
                y: 476.0,
            },
        ];

        assert_point(calc_label_position(&points), 242.40006, 426.14623);
    }

    #[test]
    fn position_keeps_dagre_anchor_until_insert_edge_marks_the_path_updated() {
        let points = [
            LayoutPoint { x: 0.0, y: 0.0 },
            LayoutPoint { x: 10.0, y: 0.0 },
            LayoutPoint { x: 20.0, y: 0.0 },
        ];
        let anchor = LayoutPoint { x: 4.0, y: 5.0 };

        let unchanged = position_edge_label(anchor.clone(), &points, "M0,0 L10,0 L20,0", false);
        assert_eq!((unchanged.x, unchanged.y), (anchor.x, anchor.y));
        let updated = position_edge_label(anchor, &points, "M0,0 L10,0 L20,0", true);
        assert_eq!((updated.x, updated.y), (10.0, 0.0));
    }

    #[test]
    fn calculation_handles_empty_single_and_zero_length_paths() {
        assert!(calc_label_position(&[]).is_none());
        assert_point(
            calc_label_position(&[LayoutPoint { x: 2.0, y: 3.0 }]),
            2.0,
            3.0,
        );
        assert_point(
            calc_label_position(&[
                LayoutPoint { x: 2.0, y: 3.0 },
                LayoutPoint { x: 2.0, y: 3.0 },
            ]),
            2.0,
            3.0,
        );
    }

    #[test]
    fn interpolation_uses_js_round_but_exact_endpoints_are_not_rounded() {
        assert_point(
            calc_label_position(&[
                LayoutPoint {
                    x: -0.00001,
                    y: 0.0,
                },
                LayoutPoint { x: 0.0, y: 0.0 },
            ]),
            0.0,
            0.0,
        );
        assert_point(
            calc_label_position(&[
                LayoutPoint { x: 0.0, y: 0.0 },
                LayoutPoint {
                    x: 1.234567,
                    y: 0.0,
                },
                LayoutPoint {
                    x: 2.469134,
                    y: 0.0,
                },
            ]),
            1.234567,
            0.0,
        );
    }

    #[test]
    fn path_coordinate_detection_preserves_mermaids_string_heuristic() {
        let point = LayoutPoint { x: 22.0, y: 99.0 };
        assert!(is_label_coordinate_in_path(&point, "M122 0"));
        assert!(is_label_coordinate_in_path(
            &LayoutPoint { x: 12.0, y: 99.0 },
            "M-12.4 0"
        ));
        assert_eq!(round_decimal_numbers_in_path("M.5 10."), "M.5 10.");
        assert!(!is_label_coordinate_in_path(
            &LayoutPoint { x: 7.0, y: 8.0 },
            "M.5 10."
        ));
    }
}

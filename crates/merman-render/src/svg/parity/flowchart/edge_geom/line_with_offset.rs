//! Mermaid `lineWithOffset` port.
//!
//! Mermaid shortens edge paths so markers don't render on top of the line (see
//! `packages/mermaid/src/utils/lineWithOffset.ts`).

pub(in crate::svg::parity::flowchart) fn line_with_offset_points(
    input: &[crate::model::LayoutPoint],
    arrow_type_start: Option<&str>,
    arrow_type_end: Option<&str>,
) -> Vec<crate::model::LayoutPoint> {
    fn marker_offset_for(arrow_type: Option<&str>) -> Option<f64> {
        match arrow_type {
            Some("arrow_point") => Some(4.0),
            Some("dependency") => Some(6.0),
            Some("lollipop") => Some(13.5),
            Some("aggregation" | "extension" | "composition") => Some(17.25),
            _ => None,
        }
    }

    fn calculate_delta_and_angle(
        a: &crate::model::LayoutPoint,
        b: &crate::model::LayoutPoint,
    ) -> (f64, f64, f64) {
        let delta_x = b.x - a.x;
        let delta_y = b.y - a.y;
        let angle = (delta_y / delta_x).atan();
        (angle, delta_x, delta_y)
    }

    if input.len() < 2 {
        return input.to_vec();
    }

    let start = &input[0];
    let end = &input[input.len() - 1];

    let x_direction_is_left = start.x < end.x;
    let y_direction_is_down = start.y < end.y;
    let extra_room = 1.0;

    let start_marker_height = marker_offset_for(arrow_type_start);
    let end_marker_height = marker_offset_for(arrow_type_end);

    let mut out = Vec::with_capacity(input.len());
    for (i, p) in input.iter().enumerate() {
        let mut ox = 0.0;
        let mut oy = 0.0;

        if i == 0 {
            if let Some(h) = start_marker_height {
                let (angle, delta_x, delta_y) = calculate_delta_and_angle(&input[0], &input[1]);
                ox = h * angle.cos() * if delta_x >= 0.0 { 1.0 } else { -1.0 };
                oy = h * angle.sin().abs() * if delta_y >= 0.0 { 1.0 } else { -1.0 };
            }
        } else if i == input.len() - 1 {
            if let Some(h) = end_marker_height {
                let (angle, delta_x, delta_y) =
                    calculate_delta_and_angle(&input[input.len() - 1], &input[input.len() - 2]);
                ox = h * angle.cos() * if delta_x >= 0.0 { 1.0 } else { -1.0 };
                oy = h * angle.sin().abs() * if delta_y >= 0.0 { 1.0 } else { -1.0 };
            }
        }

        if let Some(h) = end_marker_height {
            let diff_x = (p.x - end.x).abs();
            let diff_y = (p.y - end.y).abs();
            if diff_x < h && diff_x > 0.0 && diff_y < h {
                let mut adjustment = h + extra_room - diff_x;
                adjustment *= if !x_direction_is_left { -1.0 } else { 1.0 };
                ox -= adjustment;
            }
        }
        if let Some(h) = start_marker_height {
            let diff_x = (p.x - start.x).abs();
            let diff_y = (p.y - start.y).abs();
            if diff_x < h && diff_x > 0.0 && diff_y < h {
                let mut adjustment = h + extra_room - diff_x;
                adjustment *= if !x_direction_is_left { -1.0 } else { 1.0 };
                ox += adjustment;
            }
        }

        if let Some(h) = end_marker_height {
            let diff_y = (p.y - end.y).abs();
            let diff_x = (p.x - end.x).abs();
            if diff_y < h && diff_y > 0.0 && diff_x < h {
                let mut adjustment = h + extra_room - diff_y;
                adjustment *= if !y_direction_is_down { -1.0 } else { 1.0 };
                oy -= adjustment;
            }
        }
        if let Some(h) = start_marker_height {
            let diff_y = (p.y - start.y).abs();
            let diff_x = (p.x - start.x).abs();
            if diff_y < h && diff_y > 0.0 && diff_x < h {
                let mut adjustment = h + extra_room - diff_y;
                adjustment *= if !y_direction_is_down { -1.0 } else { 1.0 };
                oy += adjustment;
            }
        }

        out.push(crate::model::LayoutPoint {
            x: p.x + ox,
            y: p.y + oy,
        });
    }

    out
}

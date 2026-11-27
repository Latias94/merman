use merman_render_core::LayoutPoint;

/// Center and size of the rectangle used by Mermaid cluster clipping.
#[derive(Debug, Clone, Copy)]
pub struct EdgeClipRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

fn outside_node(node: &EdgeClipRect, point: &LayoutPoint) -> bool {
    let dx = (point.x - node.x).abs();
    let dy = (point.y - node.y).abs();
    let w = node.width / 2.0;
    let h = node.height / 2.0;
    dx >= w || dy >= h
}

fn rect_intersection(
    node: &EdgeClipRect,
    outside_point: &LayoutPoint,
    inside_point: &LayoutPoint,
) -> LayoutPoint {
    let x = node.x;
    let y = node.y;

    let w = node.width / 2.0;
    let h = node.height / 2.0;

    let q_abs = (outside_point.y - inside_point.y).abs();
    let r_abs = (outside_point.x - inside_point.x).abs();

    if (y - outside_point.y).abs() * w > (x - outside_point.x).abs() * h {
        let q = if inside_point.y < outside_point.y {
            outside_point.y - h - y
        } else {
            y - h - outside_point.y
        };
        let r = if q_abs == 0.0 {
            0.0
        } else {
            (r_abs * q) / q_abs
        };
        let mut res = LayoutPoint {
            x: if inside_point.x < outside_point.x {
                inside_point.x + r
            } else {
                inside_point.x - r_abs + r
            },
            y: if inside_point.y < outside_point.y {
                inside_point.y + q_abs - q
            } else {
                inside_point.y - q_abs + q
            },
        };

        if r.abs() <= 1e-9 {
            res.x = outside_point.x;
            res.y = outside_point.y;
        }
        if r_abs == 0.0 {
            res.x = outside_point.x;
        }
        if q_abs == 0.0 {
            res.y = outside_point.y;
        }
        return res;
    }

    let r = if inside_point.x < outside_point.x {
        outside_point.x - w - x
    } else {
        x - w - outside_point.x
    };
    let q = if r_abs == 0.0 {
        0.0
    } else {
        (q_abs * r) / r_abs
    };
    let mut ix = if inside_point.x < outside_point.x {
        inside_point.x + r_abs - r
    } else {
        inside_point.x - r_abs + r
    };
    let mut iy = if inside_point.y < outside_point.y {
        inside_point.y + q
    } else {
        inside_point.y - q
    };

    if r.abs() <= 1e-9 {
        ix = outside_point.x;
        iy = outside_point.y;
    }
    if r_abs == 0.0 {
        ix = outside_point.x;
    }
    if q_abs == 0.0 {
        iy = outside_point.y;
    }

    LayoutPoint { x: ix, y: iy }
}

impl EdgeClipRect {
    /// Clips a terminal run into the existing forward point buffer.
    pub fn clip_end_into(&self, input: &[LayoutPoint], out: &mut Vec<LayoutPoint>) {
        out.clear();
        if input.is_empty() {
            return;
        }
        out.reserve(input.len() + 1);

        let mut last_point_outside = input[0].clone();
        let mut is_inside = false;
        const EPS: f64 = 1e-9;

        for point in input {
            if !outside_node(self, point) && !is_inside {
                let inter = rect_intersection(self, &last_point_outside, point);
                if !out
                    .iter()
                    .any(|p| (p.x - inter.x).abs() <= EPS && (p.y - inter.y).abs() <= EPS)
                {
                    out.push(inter);
                }
                is_inside = true;
            } else {
                last_point_outside = point.clone();
                if !is_inside {
                    out.push(LayoutPoint {
                        x: point.x,
                        y: point.y,
                    });
                }
            }
        }
    }

    /// Clips the start with the same terminal rules, restoring the forward point order.
    pub fn clip_start_in_place(
        &self,
        points: &mut Vec<LayoutPoint>,
        reversed: &mut Vec<LayoutPoint>,
    ) {
        reversed.clear();
        reversed.extend_from_slice(points);
        reversed.reverse();
        self.clip_end_into(reversed, points);
        points.reverse();
    }
}

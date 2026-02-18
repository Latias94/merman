use super::super::*;
use super::BoundaryNode;

pub(in crate::svg::parity::flowchart) fn is_rounded_intersect_shift_shape(
    layout_shape: Option<&str>,
) -> bool {
    matches!(layout_shape, Some("roundedRect" | "rounded"))
}

pub(in crate::svg::parity::flowchart) fn is_polygon_layout_shape(
    layout_shape: Option<&str>,
) -> bool {
    matches!(
        layout_shape,
        Some(
            "hexagon"
                | "hex"
                | "odd"
                | "rect_left_inv_arrow"
                | "stadium"
                | "subroutine"
                | "subproc"
                | "subprocess"
                | "lean_right"
                | "lean-r"
                | "lean-right"
                | "lean_left"
                | "lean-l"
                | "lean-left"
                | "trapezoid"
                | "inv_trapezoid"
                | "inv-trapezoid"
        )
    )
}

pub(in crate::svg::parity::flowchart) fn force_intersect_for_layout_shape(
    layout_shape: Option<&str>,
) -> bool {
    matches!(
        layout_shape,
        Some(
            "circle"
                | "diamond"
                | "diam"
                | "roundedRect"
                | "rounded"
                | "cylinder"
                | "cyl"
                | "h-cyl"
                | "das"
                | "horizontal-cylinder"
                | "stadium",
        )
    ) || is_polygon_layout_shape(layout_shape)
}

fn intersect_rect(
    node: &BoundaryNode,
    point: &crate::model::LayoutPoint,
) -> crate::model::LayoutPoint {
    let x = node.x;
    let y = node.y;
    let dx = point.x - x;
    let dy = point.y - y;
    let mut w = node.width / 2.0;
    let mut h = node.height / 2.0;

    let (sx, sy) = if dy.abs() * w > dx.abs() * h {
        if dy < 0.0 {
            h = -h;
        }
        let sx = if dy == 0.0 { 0.0 } else { (h * dx) / dy };
        (sx, h)
    } else {
        if dx < 0.0 {
            w = -w;
        }
        let sy = if dx == 0.0 { 0.0 } else { (w * dy) / dx };
        (w, sy)
    };

    crate::model::LayoutPoint {
        x: x + sx,
        y: y + sy,
    }
}

fn intersect_circle(
    node: &BoundaryNode,
    point: &crate::model::LayoutPoint,
) -> crate::model::LayoutPoint {
    let dx = point.x - node.x;
    let dy = point.y - node.y;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist <= 1e-12 {
        return crate::model::LayoutPoint {
            x: node.x,
            y: node.y,
        };
    }
    let r = (node.width.min(node.height) / 2.0).max(0.0);
    crate::model::LayoutPoint {
        x: node.x + dx / dist * r,
        y: node.y + dy / dist * r,
    }
}

fn intersect_diamond(
    node: &BoundaryNode,
    point: &crate::model::LayoutPoint,
) -> crate::model::LayoutPoint {
    let vx = point.x - node.x;
    let vy = point.y - node.y;
    if !(vx.is_finite() && vy.is_finite()) {
        return crate::model::LayoutPoint {
            x: node.x,
            y: node.y,
        };
    }
    if vx.abs() <= 1e-12 && vy.abs() <= 1e-12 {
        return crate::model::LayoutPoint {
            x: node.x,
            y: node.y,
        };
    }
    let hw = (node.width / 2.0).max(1e-9);
    let hh = (node.height / 2.0).max(1e-9);
    let denom = vx.abs() / hw + vy.abs() / hh;
    if !(denom.is_finite() && denom > 0.0) {
        return crate::model::LayoutPoint {
            x: node.x,
            y: node.y,
        };
    }
    let t = 1.0 / denom;
    crate::model::LayoutPoint {
        x: node.x + vx * t,
        y: node.y + vy * t,
    }
}

fn intersect_cylinder(
    node: &BoundaryNode,
    point: &crate::model::LayoutPoint,
) -> crate::model::LayoutPoint {
    // Port of Mermaid `cylinder.ts` intersection logic (11.12.2):
    // - start from `intersect.rect(node, point)`,
    // - then adjust y when the intersection hits the curved top/bottom ellipses.
    let mut pos = intersect_rect(node, point);
    let x = pos.x - node.x;

    let w = node.width.max(1.0);
    let rx = w / 2.0;
    let ry = rx / (2.5 + w / 50.0);

    if rx != 0.0
        && (x.abs() < w / 2.0
            || ((x.abs() - w / 2.0).abs() < 1e-12
                && (pos.y - node.y).abs() > node.height / 2.0 - ry))
    {
        let mut y = ry * ry * (1.0 - (x * x) / (rx * rx));
        if y > 0.0 {
            y = y.sqrt();
        } else {
            y = 0.0;
        }
        y = ry - y;
        if point.y - node.y > 0.0 {
            y = -y;
        }
        pos.y += y;
    }

    pos
}

fn intersect_tilted_cylinder(
    node: &BoundaryNode,
    point: &crate::model::LayoutPoint,
) -> crate::model::LayoutPoint {
    // Port of Mermaid `tiltedCylinder.ts` intersection logic (11.12.2):
    // - start from `intersect.rect(node, point)`,
    // - then adjust x when the intersection hits the curved ends.
    let mut pos = intersect_rect(node, point);
    let y = pos.y - node.y;

    let ry = node.height / 2.0;
    let rx = if ry == 0.0 {
        0.0
    } else {
        ry / (2.5 + node.height / 50.0)
    };

    if ry != 0.0
        && (y.abs() < node.height / 2.0
            || (y.abs() == node.height / 2.0 && (pos.x - node.x).abs() > node.width / 2.0 - rx))
    {
        let mut x = rx * rx * (1.0 - (y * y) / (ry * ry));
        if x != 0.0 {
            x = x.abs().sqrt();
        }
        x = rx - x;
        if point.x - node.x > 0.0 {
            x = -x;
        }
        pos.x += x;
    }

    pos
}

fn intersect_line(
    p1: crate::model::LayoutPoint,
    p2: crate::model::LayoutPoint,
    q1: crate::model::LayoutPoint,
    q2: crate::model::LayoutPoint,
) -> Option<crate::model::LayoutPoint> {
    // Port of Mermaid `intersect-line.js` (11.12.2).
    //
    // This does segment intersection with a "denom/2" offset rounding that materially affects
    // flowchart endpoints and thus SVG `viewBox`/`max-width` parity.
    let a1 = p2.y - p1.y;
    let b1 = p1.x - p2.x;
    let c1 = p2.x * p1.y - p1.x * p2.y;

    let r3 = a1 * q1.x + b1 * q1.y + c1;
    let r4 = a1 * q2.x + b1 * q2.y + c1;

    fn same_sign(r1: f64, r2: f64) -> bool {
        r1 * r2 > 0.0
    }

    if r3 != 0.0 && r4 != 0.0 && same_sign(r3, r4) {
        return None;
    }

    let a2 = q2.y - q1.y;
    let b2 = q1.x - q2.x;
    let c2 = q2.x * q1.y - q1.x * q2.y;

    let r1 = a2 * p1.x + b2 * p1.y + c2;
    let r2 = a2 * p2.x + b2 * p2.y + c2;

    // Match Mermaid@11.12.2 `intersect-line.js`: the side test is an exact `!== 0` guard.
    // Keep this exact check so our segment intersection matches upstream for collinear and
    // endpoint cases (flowing into strict SVG `data-points` parity).
    if r1 != 0.0 && r2 != 0.0 && same_sign(r1, r2) {
        return None;
    }

    let denom = a1 * b2 - a2 * b1;
    if denom == 0.0 {
        return None;
    }

    let offset = (denom / 2.0).abs();

    let mut num = b1 * c2 - b2 * c1;
    let x = if num < 0.0 {
        (num - offset) / denom
    } else {
        (num + offset) / denom
    };

    num = a2 * c1 - a1 * c2;
    let y = if num < 0.0 {
        (num - offset) / denom
    } else {
        (num + offset) / denom
    };

    Some(crate::model::LayoutPoint { x, y })
}

fn intersect_polygon(
    node: &BoundaryNode,
    poly_points: &[crate::model::LayoutPoint],
    point: &crate::model::LayoutPoint,
) -> crate::model::LayoutPoint {
    // Port of Mermaid `intersect-polygon.js` (11.12.2).
    let x1 = node.x;
    let y1 = node.y;

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    for p in poly_points {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
    }

    let left = x1 - node.width / 2.0 - min_x;
    let top = y1 - node.height / 2.0 - min_y;

    let mut intersections: Vec<crate::model::LayoutPoint> = Vec::new();
    for i in 0..poly_points.len() {
        let p1 = &poly_points[i];
        let p2 = &poly_points[if i + 1 < poly_points.len() { i + 1 } else { 0 }];
        let q1 = crate::model::LayoutPoint {
            x: left + p1.x,
            y: top + p1.y,
        };
        let q2 = crate::model::LayoutPoint {
            x: left + p2.x,
            y: top + p2.y,
        };
        if let Some(inter) = intersect_line(
            crate::model::LayoutPoint { x: x1, y: y1 },
            point.clone(),
            q1,
            q2,
        ) {
            intersections.push(inter);
        }
    }

    if intersections.is_empty() {
        return crate::model::LayoutPoint { x: x1, y: y1 };
    }

    if intersections.len() > 1 {
        intersections.sort_by(|p, q| {
            let pdx = p.x - point.x;
            let pdy = p.y - point.y;
            let qdx = q.x - point.x;
            let qdy = q.y - point.y;
            let dist_p = (pdx * pdx + pdy * pdy).sqrt();
            let dist_q = (qdx * qdx + qdy * qdy).sqrt();
            dist_p
                .partial_cmp(&dist_q)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    intersections[0].clone()
}

fn polygon_points_for_layout_shape(
    layout_shape: &str,
    node: &BoundaryNode,
) -> Option<Vec<crate::model::LayoutPoint>> {
    let w = node.width.max(1.0);
    let h = node.height.max(1.0);

    match layout_shape {
        // Mermaid "odd" nodes (`>... ]`) are rendered using `rect_left_inv_arrow`.
        //
        // Reference: Mermaid@11.12.2 `rectLeftInvArrow.ts`.
        //
        // Note: Flowchart layout dimensions model this as `node.width = w + h/4`, where `w`
        // corresponds to Mermaid's `w = max(bbox.width + padding, node.width)` prior to the
        // `updateNodeBounds(...)` bbox expansion.
        "odd" | "rect_left_inv_arrow" => {
            let base_w = (w - h / 4.0).max(1.0);
            let x = -base_w / 2.0;
            let y = -h / 2.0;
            let notch = y / 2.0; // negative
            Some(vec![
                crate::model::LayoutPoint { x: x + notch, y },
                crate::model::LayoutPoint { x, y: 0.0 },
                crate::model::LayoutPoint {
                    x: x + notch,
                    y: -y,
                },
                crate::model::LayoutPoint { x: -x, y: -y },
                crate::model::LayoutPoint { x: -x, y },
            ])
        }
        "subroutine" | "subproc" | "subprocess" => {
            // Port of Mermaid@11.12.2 `subroutine.ts` points used for polygon intersection.
            //
            // Mermaid's insertPolygonShape(...) uses `w = bbox.width + padding` but the
            // resulting bbox expands by `offset*2` (=16px) due to the outer frame.
            let inner_w = (w - 16.0).max(1.0);
            Some(vec![
                crate::model::LayoutPoint { x: 0.0, y: 0.0 },
                crate::model::LayoutPoint { x: inner_w, y: 0.0 },
                crate::model::LayoutPoint { x: inner_w, y: -h },
                crate::model::LayoutPoint { x: 0.0, y: -h },
                crate::model::LayoutPoint { x: 0.0, y: 0.0 },
                crate::model::LayoutPoint { x: -8.0, y: 0.0 },
                crate::model::LayoutPoint {
                    x: inner_w + 8.0,
                    y: 0.0,
                },
                crate::model::LayoutPoint {
                    x: inner_w + 8.0,
                    y: -h,
                },
                crate::model::LayoutPoint { x: -8.0, y: -h },
                crate::model::LayoutPoint { x: -8.0, y: 0.0 },
            ])
        }
        "hexagon" | "hex" => {
            let half_width = w / 2.0;
            let half_height = h / 2.0;
            let fixed_length = half_height / 2.0;
            let deduced_width = half_width - fixed_length;
            Some(vec![
                crate::model::LayoutPoint {
                    x: -deduced_width,
                    y: -half_height,
                },
                crate::model::LayoutPoint {
                    x: 0.0,
                    y: -half_height,
                },
                crate::model::LayoutPoint {
                    x: deduced_width,
                    y: -half_height,
                },
                crate::model::LayoutPoint {
                    x: half_width,
                    y: 0.0,
                },
                crate::model::LayoutPoint {
                    x: deduced_width,
                    y: half_height,
                },
                crate::model::LayoutPoint {
                    x: 0.0,
                    y: half_height,
                },
                crate::model::LayoutPoint {
                    x: -deduced_width,
                    y: half_height,
                },
                crate::model::LayoutPoint {
                    x: -half_width,
                    y: 0.0,
                },
            ])
        }
        "lean_right" | "lean-r" | "lean-right" => {
            let total_w = w;
            let w = (total_w - h).max(1.0);
            let dx = (3.0 * h) / 6.0;
            Some(vec![
                crate::model::LayoutPoint { x: -dx, y: 0.0 },
                crate::model::LayoutPoint { x: w, y: 0.0 },
                crate::model::LayoutPoint { x: w + dx, y: -h },
                crate::model::LayoutPoint { x: 0.0, y: -h },
            ])
        }
        "lean_left" | "lean-l" | "lean-left" => {
            let total_w = w;
            let w = (total_w - h).max(1.0);
            let dx = (3.0 * h) / 6.0;
            Some(vec![
                crate::model::LayoutPoint { x: 0.0, y: 0.0 },
                crate::model::LayoutPoint { x: w + dx, y: 0.0 },
                crate::model::LayoutPoint { x: w, y: -h },
                crate::model::LayoutPoint { x: -dx, y: -h },
            ])
        }
        "trapezoid" => {
            let total_w = w;
            let w = (total_w - h).max(1.0);
            let dx = (3.0 * h) / 6.0;
            Some(vec![
                crate::model::LayoutPoint { x: -dx, y: 0.0 },
                crate::model::LayoutPoint { x: w + dx, y: 0.0 },
                crate::model::LayoutPoint { x: w, y: -h },
                crate::model::LayoutPoint { x: 0.0, y: -h },
            ])
        }
        "inv_trapezoid" | "inv-trapezoid" => {
            let total_w = w;
            let w = (total_w - h).max(1.0);
            let dx = (3.0 * h) / 6.0;
            Some(vec![
                crate::model::LayoutPoint { x: 0.0, y: 0.0 },
                crate::model::LayoutPoint { x: w, y: 0.0 },
                crate::model::LayoutPoint { x: w + dx, y: -h },
                crate::model::LayoutPoint { x: -dx, y: -h },
            ])
        }
        _ => None,
    }
}

pub(in crate::svg::parity::flowchart) fn intersect_for_layout_shape(
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    node: &BoundaryNode,
    layout_shape: Option<&str>,
    point: &crate::model::LayoutPoint,
) -> crate::model::LayoutPoint {
    fn intersect_stadium(
        ctx: &FlowchartRenderCtx<'_>,
        node_id: &str,
        node: &BoundaryNode,
        point: &crate::model::LayoutPoint,
    ) -> crate::model::LayoutPoint {
        // Port of Mermaid@11.12.2 `stadium.ts` intersection behavior:
        // - `points` are generated from the theoretical render dimensions,
        // - `node.width/height` used by `intersect.polygon(...)` come from `updateNodeBounds(...)`.
        fn generate_circle_points(
            center_x: f64,
            center_y: f64,
            radius: f64,
            table: &[(f64, f64)],
        ) -> Vec<crate::model::LayoutPoint> {
            let mut pts = Vec::with_capacity(table.len());
            for &(cos, sin) in table {
                let x = center_x + radius * cos;
                let y = center_y + radius * sin;
                pts.push(crate::model::LayoutPoint { x: -x, y: -y });
            }
            pts
        }

        let Some(flow_node) = ctx.nodes_by_id.get(node_id) else {
            return intersect_rect(node, point);
        };

        let label_text = flow_node.label.clone().unwrap_or_default();
        let label_type = flow_node
            .label_type
            .clone()
            .unwrap_or_else(|| "text".to_string());

        let mut metrics = crate::flowchart::flowchart_label_metrics_for_layout(
            ctx.measurer,
            &label_text,
            &label_type,
            &ctx.text_style,
            Some(ctx.wrapping_width),
            ctx.node_wrap_mode,
        );

        let span_css_height_parity = flow_node.classes.iter().any(|c| {
            ctx.class_defs.get(c.as_str()).is_some_and(|styles| {
                styles.iter().any(|s| {
                    matches!(
                        s.split_once(':').map(|p| p.0.trim()),
                        Some("background" | "border")
                    )
                })
            })
        });
        if span_css_height_parity {
            crate::text::flowchart_apply_mermaid_styled_node_height_parity(
                &mut metrics,
                &ctx.text_style,
            );
        }

        let (render_w, render_h) = crate::flowchart::flowchart_node_render_dimensions(
            Some("stadium"),
            metrics,
            ctx.node_padding,
        );
        let mut w = render_w.max(1.0);
        let mut h = render_h.max(1.0);

        // The input bbox values that Mermaid uses to derive these dimensions come from DOM
        // APIs and behave like f32-rounded values in Chromium. Keep the sampled polygon points
        // on the same lattice so the downstream intersection rounding matches strict baselines.
        let w_f32 = w as f32;
        let h_f32 = h as f32;
        if w_f32.is_finite()
            && h_f32.is_finite()
            && w_f32.is_sign_positive()
            && h_f32.is_sign_positive()
        {
            w = w_f32 as f64;
            h = h_f32 as f64;
        }

        let radius = h / 2.0;

        let mut pts: Vec<crate::model::LayoutPoint> = Vec::with_capacity(2 + 50 + 1 + 50);
        pts.push(crate::model::LayoutPoint {
            x: -w / 2.0 + radius,
            y: -h / 2.0,
        });
        pts.push(crate::model::LayoutPoint {
            x: w / 2.0 - radius,
            y: -h / 2.0,
        });
        pts.extend(generate_circle_points(
            -w / 2.0 + radius,
            0.0,
            radius,
            &crate::trig_tables::STADIUM_ARC_90_270_COS_SIN,
        ));
        pts.push(crate::model::LayoutPoint {
            x: w / 2.0 - radius,
            y: h / 2.0,
        });
        pts.extend(generate_circle_points(
            w / 2.0 - radius,
            0.0,
            radius,
            &crate::trig_tables::STADIUM_ARC_270_450_COS_SIN,
        ));
        intersect_polygon(node, &pts, point)
    }

    fn intersect_hexagon(
        ctx: &FlowchartRenderCtx<'_>,
        node_id: &str,
        node: &BoundaryNode,
        point: &crate::model::LayoutPoint,
    ) -> crate::model::LayoutPoint {
        // Port of Mermaid@11.12.2 `hexagon.ts` intersection behavior:
        // - `points` are generated from the theoretical render dimensions,
        // - `node.width/height` used by `intersect.polygon(...)` come from `updateNodeBounds(...)`.
        let Some(flow_node) = ctx.nodes_by_id.get(node_id) else {
            return intersect_rect(node, point);
        };

        let label_text = flow_node.label.clone().unwrap_or_default();
        let label_type = flow_node
            .label_type
            .clone()
            .unwrap_or_else(|| "text".to_string());

        let mut metrics = crate::flowchart::flowchart_label_metrics_for_layout(
            ctx.measurer,
            &label_text,
            &label_type,
            &ctx.text_style,
            Some(ctx.wrapping_width),
            ctx.node_wrap_mode,
        );

        let span_css_height_parity = flow_node.classes.iter().any(|c| {
            ctx.class_defs.get(c.as_str()).is_some_and(|styles| {
                styles.iter().any(|s| {
                    matches!(
                        s.split_once(':').map(|p| p.0.trim()),
                        Some("background" | "border")
                    )
                })
            })
        });
        if span_css_height_parity {
            crate::text::flowchart_apply_mermaid_styled_node_height_parity(
                &mut metrics,
                &ctx.text_style,
            );
        }

        let (render_w, render_h) = crate::flowchart::flowchart_node_render_dimensions(
            Some("hexagon"),
            metrics,
            ctx.node_padding,
        );
        let w = render_w.max(1.0);
        let h = render_h.max(1.0);
        let half_width = w / 2.0;
        let half_height = h / 2.0;
        let fixed_length = half_height / 2.0;
        let deduced_width = half_width - fixed_length;

        let pts: Vec<crate::model::LayoutPoint> = vec![
            crate::model::LayoutPoint {
                x: -deduced_width,
                y: -half_height,
            },
            crate::model::LayoutPoint {
                x: 0.0,
                y: -half_height,
            },
            crate::model::LayoutPoint {
                x: deduced_width,
                y: -half_height,
            },
            crate::model::LayoutPoint {
                x: half_width,
                y: 0.0,
            },
            crate::model::LayoutPoint {
                x: deduced_width,
                y: half_height,
            },
            crate::model::LayoutPoint {
                x: 0.0,
                y: half_height,
            },
            crate::model::LayoutPoint {
                x: -deduced_width,
                y: half_height,
            },
            crate::model::LayoutPoint {
                x: -half_width,
                y: 0.0,
            },
        ];

        intersect_polygon(node, &pts, point)
    }

    match layout_shape {
        Some("circle") => intersect_circle(node, point),
        Some("cylinder" | "cyl") => intersect_cylinder(node, point),
        Some("h-cyl" | "das" | "horizontal-cylinder") => intersect_tilted_cylinder(node, point),
        Some("diamond" | "diam") => intersect_diamond(node, point),
        Some("stadium") => intersect_stadium(ctx, node_id, node, point),
        Some("hexagon" | "hex") => intersect_hexagon(ctx, node_id, node, point),
        Some(s) if is_polygon_layout_shape(Some(s)) => polygon_points_for_layout_shape(s, node)
            .map(|pts| intersect_polygon(node, &pts, point))
            .unwrap_or_else(|| intersect_rect(node, point)),
        _ => intersect_rect(node, point),
    }
}

use super::*;
use crate::block::{BlockArrowPoint as ArrowPoint, block_arrow_points};
use crate::model::LayoutPoint;

// Block diagram SVG renderer implementation (split from parity.rs).

pub(super) fn render_block_diagram_svg(
    layout: &BlockDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    fn decode_block_label_html(raw: &str) -> String {
        // Mermaid's block diagram labels are rendered via an HTML foreignObject label helper,
        // which decodes HTML entities (notably `&nbsp;`).
        raw.replace("&nbsp;", "\u{00A0}")
    }

    #[derive(Clone)]
    struct RenderNode {
        label: String,
        block_type: String,
        classes: Vec<String>,
        directions: Vec<String>,
    }

    fn collect_nodes(
        n: &crate::block::BlockNode,
        out: &mut std::collections::HashMap<String, RenderNode>,
    ) {
        out.entry(n.id.clone()).or_insert_with(|| RenderNode {
            label: n.label.clone(),
            block_type: n.block_type.clone(),
            classes: n.classes.clone(),
            directions: n.directions.clone(),
        });
        for c in &n.children {
            collect_nodes(c, out);
        }
    }

    let model: crate::block::BlockDiagramModel = crate::json::from_value_ref(semantic)?;
    let node_padding = config_f64(effective_config, &["block", "padding"]).unwrap_or(8.0);
    let mut nodes_by_id: std::collections::HashMap<String, RenderNode> =
        std::collections::HashMap::new();
    for n in &model.blocks_flat {
        collect_nodes(n, &mut nodes_by_id);
    }
    let layout_nodes_by_id: std::collections::HashMap<String, LayoutNode> = layout
        .nodes
        .iter()
        .cloned()
        .map(|n| (n.id.clone(), n))
        .collect();

    fn marker_id(diagram_id: &str, marker: &str) -> String {
        format!("{diagram_id}_block-{marker}")
    }

    fn marker_url(diagram_id: &str, marker: &str) -> String {
        format!("url(#{})", marker_id(diagram_id, marker))
    }

    fn edge_marker_end(arrow: Option<&str>) -> Option<&'static str> {
        match arrow.unwrap_or("").trim() {
            "arrow_point" => Some("pointEnd"),
            "arrow_circle" => Some("circleEnd"),
            "arrow_cross" => Some("crossEnd"),
            "arrow_open" | "" => None,
            _ => Some("pointEnd"),
        }
    }

    fn edge_marker_start(arrow: Option<&str>) -> Option<&'static str> {
        match arrow.unwrap_or("").trim() {
            "arrow_point" => Some("pointStart"),
            "arrow_circle" => Some("circleStart"),
            "arrow_cross" => Some("crossStart"),
            "arrow_open" | "" => None,
            _ => None,
        }
    }

    fn block_edge_start_marker_inset(arrow: Option<&str>) -> f64 {
        match arrow.unwrap_or("").trim() {
            "arrow_point" => 4.5,
            _ => 0.0,
        }
    }

    fn block_edge_end_marker_inset(arrow: Option<&str>) -> f64 {
        match arrow.unwrap_or("").trim() {
            "arrow_point" => 4.0,
            _ => 0.0,
        }
    }

    fn move_point_towards(point: &LayoutPoint, target: &LayoutPoint, distance: f64) -> LayoutPoint {
        if distance.abs() <= 1e-12 {
            return point.clone();
        }
        let dx = target.x - point.x;
        let dy = target.y - point.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len <= 1e-12 {
            return point.clone();
        }
        LayoutPoint {
            x: point.x + dx / len * distance,
            y: point.y + dy / len * distance,
        }
    }

    fn intersect_line(
        p1: &LayoutPoint,
        p2: &LayoutPoint,
        q1: &LayoutPoint,
        q2: &LayoutPoint,
    ) -> Option<LayoutPoint> {
        let a1 = p2.y - p1.y;
        let b1 = p1.x - p2.x;
        let c1 = p2.x * p1.y - p1.x * p2.y;

        let r3 = a1 * q1.x + b1 * q1.y + c1;
        let r4 = a1 * q2.x + b1 * q2.y + c1;
        if r3 != 0.0 && r4 != 0.0 && r3 * r4 > 0.0 {
            return None;
        }

        let a2 = q2.y - q1.y;
        let b2 = q1.x - q2.x;
        let c2 = q2.x * q1.y - q1.x * q2.y;

        let r1 = a2 * p1.x + b2 * p1.y + c2;
        let r2 = a2 * p2.x + b2 * p2.y + c2;
        if r1 != 0.0 && r2 != 0.0 && r1 * r2 > 0.0 {
            return None;
        }

        let denom = a1 * b2 - a2 * b1;
        if denom.abs() <= 1e-12 {
            return None;
        }

        let offset = (denom / 2.0).abs();

        let num_x = b1 * c2 - b2 * c1;
        let x = if num_x < 0.0 {
            (num_x - offset) / denom
        } else {
            (num_x + offset) / denom
        };

        let num_y = a2 * c1 - a1 * c2;
        let y = if num_y < 0.0 {
            (num_y - offset) / denom
        } else {
            (num_y + offset) / denom
        };

        Some(LayoutPoint { x, y })
    }

    fn intersect_rect(node: &LayoutNode, point: &LayoutPoint) -> LayoutPoint {
        let dx = point.x - node.x;
        let dy = point.y - node.y;
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

        LayoutPoint {
            x: node.x + sx,
            y: node.y + sy,
        }
    }

    fn intersect_circle(node: &LayoutNode, point: &LayoutPoint) -> LayoutPoint {
        let dx = point.x - node.x;
        let dy = point.y - node.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist <= 1e-12 {
            return LayoutPoint {
                x: node.x,
                y: node.y,
            };
        }
        let radius = (node.width.min(node.height) / 2.0).max(0.0);
        LayoutPoint {
            x: node.x + dx / dist * radius,
            y: node.y + dy / dist * radius,
        }
    }

    fn intersect_cylinder(node: &LayoutNode, point: &LayoutPoint) -> LayoutPoint {
        let mut pos = intersect_rect(node, point);
        let x = pos.x - node.x;

        let width = node.width.max(1.0);
        let rx = width / 2.0;
        let ry = rx / (2.5 + width / 50.0);

        if rx != 0.0
            && (x.abs() < width / 2.0
                || ((x.abs() - width / 2.0).abs() < 1e-12
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

    fn intersect_polygon(
        node: &LayoutNode,
        poly_points: &[ArrowPoint],
        point: &LayoutPoint,
    ) -> LayoutPoint {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        for entry in poly_points {
            min_x = min_x.min(entry.x);
            min_y = min_y.min(entry.y);
        }

        let left = node.x - node.width / 2.0 - min_x;
        let top = node.y - node.height / 2.0 - min_y;

        let mut intersections = Vec::new();
        for idx in 0..poly_points.len() {
            let p1 = &poly_points[idx];
            let p2 = &poly_points[(idx + 1) % poly_points.len()];
            let q1 = LayoutPoint {
                x: left + p1.x,
                y: top + p1.y,
            };
            let q2 = LayoutPoint {
                x: left + p2.x,
                y: top + p2.y,
            };
            if let Some(intersection) = intersect_line(
                &LayoutPoint {
                    x: node.x,
                    y: node.y,
                },
                point,
                &q1,
                &q2,
            ) {
                intersections.push(intersection);
            }
        }

        intersections
            .into_iter()
            .min_by(|p, q| {
                let p_dist = (p.x - point.x).powi(2) + (p.y - point.y).powi(2);
                let q_dist = (q.x - point.x).powi(2) + (q.y - point.y).powi(2);
                p_dist.total_cmp(&q_dist)
            })
            .unwrap_or(LayoutPoint {
                x: node.x,
                y: node.y,
            })
    }

    fn block_polygon_points(
        node: &LayoutNode,
        render_node: &RenderNode,
        node_padding: f64,
    ) -> Option<Vec<ArrowPoint>> {
        let bbox_w = node.label_width.unwrap_or(0.0).max(0.0);
        let bbox_h = node.label_height.unwrap_or(0.0).max(0.0);
        let rect_w = (bbox_w + node_padding).max(1.0);
        let rect_h = (bbox_h + node_padding).max(1.0);

        match render_node.block_type.as_str() {
            "diamond" => {
                let side = (rect_w + rect_h).max(1.0);
                Some(vec![
                    ArrowPoint {
                        x: side / 2.0,
                        y: 0.0,
                    },
                    ArrowPoint {
                        x: side,
                        y: -side / 2.0,
                    },
                    ArrowPoint {
                        x: side / 2.0,
                        y: -side,
                    },
                    ArrowPoint {
                        x: 0.0,
                        y: -side / 2.0,
                    },
                ])
            }
            "hexagon" => {
                let shoulder = rect_h / 4.0;
                let hex_w = (bbox_w + 2.0 * shoulder + node_padding).max(1.0);
                Some(vec![
                    ArrowPoint {
                        x: shoulder,
                        y: 0.0,
                    },
                    ArrowPoint {
                        x: hex_w - shoulder,
                        y: 0.0,
                    },
                    ArrowPoint {
                        x: hex_w,
                        y: -rect_h / 2.0,
                    },
                    ArrowPoint {
                        x: hex_w - shoulder,
                        y: -rect_h,
                    },
                    ArrowPoint {
                        x: shoulder,
                        y: -rect_h,
                    },
                    ArrowPoint {
                        x: 0.0,
                        y: -rect_h / 2.0,
                    },
                ])
            }
            "rect_left_inv_arrow" => Some(vec![
                ArrowPoint {
                    x: -rect_h / 2.0,
                    y: 0.0,
                },
                ArrowPoint { x: rect_w, y: 0.0 },
                ArrowPoint {
                    x: rect_w,
                    y: -rect_h,
                },
                ArrowPoint {
                    x: -rect_h / 2.0,
                    y: -rect_h,
                },
                ArrowPoint {
                    x: 0.0,
                    y: -rect_h / 2.0,
                },
            ]),
            "subroutine" => Some(vec![
                ArrowPoint { x: 0.0, y: 0.0 },
                ArrowPoint { x: rect_w, y: 0.0 },
                ArrowPoint {
                    x: rect_w,
                    y: -rect_h,
                },
                ArrowPoint { x: 0.0, y: -rect_h },
                ArrowPoint { x: 0.0, y: 0.0 },
                ArrowPoint { x: -8.0, y: 0.0 },
                ArrowPoint {
                    x: rect_w + 8.0,
                    y: 0.0,
                },
                ArrowPoint {
                    x: rect_w + 8.0,
                    y: -rect_h,
                },
                ArrowPoint {
                    x: -8.0,
                    y: -rect_h,
                },
                ArrowPoint { x: -8.0, y: 0.0 },
            ]),
            "lean_right" => Some(vec![
                ArrowPoint {
                    x: (-2.0 * rect_h) / 6.0,
                    y: 0.0,
                },
                ArrowPoint {
                    x: rect_w - rect_h / 6.0,
                    y: 0.0,
                },
                ArrowPoint {
                    x: rect_w + (2.0 * rect_h) / 6.0,
                    y: -rect_h,
                },
                ArrowPoint {
                    x: rect_h / 6.0,
                    y: -rect_h,
                },
            ]),
            "lean_left" => Some(vec![
                ArrowPoint {
                    x: (2.0 * rect_h) / 6.0,
                    y: 0.0,
                },
                ArrowPoint {
                    x: rect_w + rect_h / 6.0,
                    y: 0.0,
                },
                ArrowPoint {
                    x: rect_w - (2.0 * rect_h) / 6.0,
                    y: -rect_h,
                },
                ArrowPoint {
                    x: -rect_h / 6.0,
                    y: -rect_h,
                },
            ]),
            "trapezoid" => Some(vec![
                ArrowPoint {
                    x: (-2.0 * rect_h) / 6.0,
                    y: 0.0,
                },
                ArrowPoint {
                    x: rect_w + (2.0 * rect_h) / 6.0,
                    y: 0.0,
                },
                ArrowPoint {
                    x: rect_w - rect_h / 6.0,
                    y: -rect_h,
                },
                ArrowPoint {
                    x: rect_h / 6.0,
                    y: -rect_h,
                },
            ]),
            "inv_trapezoid" => Some(vec![
                ArrowPoint {
                    x: rect_h / 6.0,
                    y: 0.0,
                },
                ArrowPoint {
                    x: rect_w - rect_h / 6.0,
                    y: 0.0,
                },
                ArrowPoint {
                    x: rect_w + (2.0 * rect_h) / 6.0,
                    y: -rect_h,
                },
                ArrowPoint {
                    x: (-2.0 * rect_h) / 6.0,
                    y: -rect_h,
                },
            ]),
            "block_arrow" => Some(block_arrow_points(
                &render_node.directions,
                bbox_w,
                bbox_h,
                node_padding,
            )),
            _ => None,
        }
    }

    fn block_intersect_node(
        node: &LayoutNode,
        render_node: &RenderNode,
        point: &LayoutPoint,
        node_padding: f64,
    ) -> LayoutPoint {
        match render_node.block_type.as_str() {
            "circle" | "doublecircle" => intersect_circle(node, point),
            "cylinder" => intersect_cylinder(node, point),
            "diamond"
            | "hexagon"
            | "rect_left_inv_arrow"
            | "subroutine"
            | "lean_right"
            | "lean_left"
            | "trapezoid"
            | "inv_trapezoid"
            | "block_arrow" => block_polygon_points(node, render_node, node_padding)
                .map(|points| intersect_polygon(node, &points, point))
                .unwrap_or_else(|| intersect_rect(node, point)),
            _ => intersect_rect(node, point),
        }
    }

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    let diagram_padding = config_f64(effective_config, &["block", "diagramPadding"])
        .unwrap_or(5.0)
        .max(0.0);

    let vb_min_x = bounds.min_x - diagram_padding;
    let vb_min_y = bounds.min_y - diagram_padding;
    let vb_w = (bounds.max_x - bounds.min_x + diagram_padding * 2.0).max(1.0);
    let vb_h = (bounds.max_y - bounds.min_y + diagram_padding * 2.0).max(1.0);

    let mut out = String::new();
    let mut viewbox_attr = format!(
        "{} {} {} {}",
        fmt(vb_min_x),
        fmt(vb_min_y),
        fmt(vb_w.max(1.0)),
        fmt(vb_h.max(1.0))
    );
    let mut max_w_style = fmt_max_width_px(vb_w.max(1.0));
    let mut w_attr = fmt(vb_w.max(1.0)).to_string();
    let mut h_attr = fmt(vb_h.max(1.0)).to_string();
    apply_root_viewport_override(
        diagram_id,
        &mut viewbox_attr,
        &mut w_attr,
        &mut h_attr,
        &mut max_w_style,
        crate::generated::block_root_overrides_11_12_2::lookup_block_root_viewport_override,
    );

    let style_attr = format!("max-width: {max_w_style}px; background-color: white;");
    root_svg::push_svg_root_open_ex(
        &mut out,
        diagram_id,
        None,
        root_svg::SvgRootWidth::Percent100,
        None,
        Some(style_attr.as_str()),
        Some(&viewbox_attr),
        root_svg::SvgRootStyleViewBoxOrder::StyleThenViewBox,
        &[],
        "block",
        None,
        None,
        false,
    );
    out.push_str(r#"<style></style><g/>"#);

    let _ = write!(
        &mut out,
        r#"<marker id="{}" class="marker block" viewBox="0 0 10 10" refX="6" refY="5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 0 0 L 10 5 L 0 10 z" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
        escape_xml(&marker_id(diagram_id, "pointEnd"))
    );
    let _ = write!(
        &mut out,
        r#"<marker id="{}" class="marker block" viewBox="0 0 10 10" refX="4.5" refY="5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 0 5 L 10 10 L 10 0 z" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
        escape_xml(&marker_id(diagram_id, "pointStart"))
    );
    let _ = write!(
        &mut out,
        r#"<marker id="{}" class="marker block" viewBox="0 0 10 10" refX="11" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
        escape_xml(&marker_id(diagram_id, "circleEnd"))
    );
    let _ = write!(
        &mut out,
        r#"<marker id="{}" class="marker block" viewBox="0 0 10 10" refX="-1" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
        escape_xml(&marker_id(diagram_id, "circleStart"))
    );
    let _ = write!(
        &mut out,
        r#"<marker id="{}" class="marker cross block" viewBox="0 0 11 11" refX="12" refY="5.2" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><path d="M 1,1 l 9,9 M 10,1 l -9,9" class="arrowMarkerPath" style="stroke-width: 2; stroke-dasharray: 1, 0;"/></marker>"#,
        escape_xml(&marker_id(diagram_id, "crossEnd"))
    );
    let _ = write!(
        &mut out,
        r#"<marker id="{}" class="marker cross block" viewBox="0 0 11 11" refX="-1" refY="5.2" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><path d="M 1,1 l 9,9 M 10,1 l -9,9" class="arrowMarkerPath" style="stroke-width: 2; stroke-dasharray: 1, 0;"/></marker>"#,
        escape_xml(&marker_id(diagram_id, "crossStart"))
    );

    out.push_str(r#"<g class="block">"#);

    for n in &layout.nodes {
        let Some(node) = nodes_by_id.get(&n.id) else {
            continue;
        };

        let class_str = if node.classes.is_empty() {
            "default".to_string()
        } else {
            node.classes.join(" ")
        };
        let class_str = format!("{class_str} flowchart-label");

        let width = n.width.max(1.0);
        let height = n.height.max(1.0);
        let x = -width / 2.0;
        let y = -height / 2.0;

        let id_attr = match n.id.as_str() {
            // Mermaid block diagrams omit `id` for these special-case ids in SVG output.
            "id" | "__proto__" | "constructor" => String::new(),
            _ => format!(r#" id="{}""#, escape_attr(&n.id)),
        };
        let _ = write!(
            &mut out,
            r#"<g class="node default {}"{} transform="translate({}, {})">"#,
            escape_attr(&class_str),
            id_attr,
            fmt(n.x),
            fmt(n.y)
        );

        fn emit_polygon(out: &mut String, points: &[ArrowPoint], base_w: f64, base_h: f64) {
            out.push_str(r#"<polygon points=""#);
            for (idx, point) in points.iter().enumerate() {
                if idx > 0 {
                    out.push(' ');
                }
                let _ = write!(out, "{},{}", fmt_display(point.x), fmt_display(point.y));
            }
            let _ = write!(
                out,
                r#"" class="label-container" style="" transform="translate({},{})"/>"#,
                fmt_display(-base_w / 2.0),
                fmt_display(base_h / 2.0)
            );
        }

        let bbox_w = n.label_width.unwrap_or(0.0).max(0.0);
        let bbox_h = n.label_height.unwrap_or(0.0).max(0.0);
        let rect_w = (bbox_w + node_padding).max(1.0);
        let rect_h = (bbox_h + node_padding).max(1.0);

        match node.block_type.as_str() {
            "circle" => {
                let _ = write!(
                    &mut out,
                    r#"<circle style="" rx="0" ry="0" r="{}" width="{}" height="{}"/>"#,
                    fmt(rect_w / 2.0),
                    fmt(rect_w),
                    fmt(rect_h)
                );
            }
            "doublecircle" => {
                let outer_w = rect_w + 10.0;
                let outer_h = rect_h + 10.0;
                let _ = write!(
                    &mut out,
                    r#"<g class="default flowchart-label"><circle style="" rx="0" ry="0" r="{}" width="{}" height="{}"/><circle style="" rx="0" ry="0" r="{}" width="{}" height="{}"/></g>"#,
                    fmt(outer_w / 2.0),
                    fmt(outer_w),
                    fmt(outer_h),
                    fmt(rect_w / 2.0),
                    fmt(rect_w),
                    fmt(rect_h)
                );
            }
            "stadium" => {
                let stadium_w = (bbox_w + rect_h / 4.0 + node_padding).max(1.0);
                let _ = write!(
                    &mut out,
                    r#"<rect rx="{}" ry="{}" style="" x="{}" y="{}" width="{}" height="{}"/>"#,
                    fmt(rect_h / 2.0),
                    fmt(rect_h / 2.0),
                    fmt(-stadium_w / 2.0),
                    fmt(-rect_h / 2.0),
                    fmt(stadium_w),
                    fmt(rect_h)
                );
            }
            "cylinder" => {
                let rx = rect_w / 2.0;
                let ry = rx / (2.5 + rect_w / 50.0);
                let body_h = (bbox_h + ry + node_padding).max(1.0);
                let _ = write!(
                    &mut out,
                    r#"<path d="M {},{} a {},{} 0,0,0 {} 0 a {},{} 0,0,0 {} 0 l 0,{} a {},{} 0,0,0 {} 0 l 0,{}" style="" transform="translate({},{})"/>"#,
                    fmt_display(0.0),
                    fmt_display(ry),
                    fmt_display(rx),
                    fmt_display(ry),
                    fmt_display(rect_w),
                    fmt_display(rx),
                    fmt_display(ry),
                    fmt_display(-rect_w),
                    fmt_display(body_h),
                    fmt_display(rx),
                    fmt_display(ry),
                    fmt_display(rect_w),
                    fmt_display(-body_h),
                    fmt_display(-rect_w / 2.0),
                    fmt_display(-(body_h / 2.0 + ry))
                );
            }
            "diamond" => {
                let side = (rect_w + rect_h).max(1.0);
                emit_polygon(
                    &mut out,
                    &[
                        ArrowPoint {
                            x: side / 2.0,
                            y: 0.0,
                        },
                        ArrowPoint {
                            x: side,
                            y: -side / 2.0,
                        },
                        ArrowPoint {
                            x: side / 2.0,
                            y: -side,
                        },
                        ArrowPoint {
                            x: 0.0,
                            y: -side / 2.0,
                        },
                    ],
                    side,
                    side,
                );
            }
            "hexagon" => {
                let shoulder = rect_h / 4.0;
                let hex_w = (bbox_w + 2.0 * shoulder + node_padding).max(1.0);
                emit_polygon(
                    &mut out,
                    &[
                        ArrowPoint {
                            x: shoulder,
                            y: 0.0,
                        },
                        ArrowPoint {
                            x: hex_w - shoulder,
                            y: 0.0,
                        },
                        ArrowPoint {
                            x: hex_w,
                            y: -rect_h / 2.0,
                        },
                        ArrowPoint {
                            x: hex_w - shoulder,
                            y: -rect_h,
                        },
                        ArrowPoint {
                            x: shoulder,
                            y: -rect_h,
                        },
                        ArrowPoint {
                            x: 0.0,
                            y: -rect_h / 2.0,
                        },
                    ],
                    hex_w,
                    rect_h,
                );
            }
            "rect_left_inv_arrow" => {
                emit_polygon(
                    &mut out,
                    &[
                        ArrowPoint {
                            x: -rect_h / 2.0,
                            y: 0.0,
                        },
                        ArrowPoint { x: rect_w, y: 0.0 },
                        ArrowPoint {
                            x: rect_w,
                            y: -rect_h,
                        },
                        ArrowPoint {
                            x: -rect_h / 2.0,
                            y: -rect_h,
                        },
                        ArrowPoint {
                            x: 0.0,
                            y: -rect_h / 2.0,
                        },
                    ],
                    rect_w,
                    rect_h,
                );
            }
            "subroutine" => {
                emit_polygon(
                    &mut out,
                    &[
                        ArrowPoint { x: 0.0, y: 0.0 },
                        ArrowPoint { x: rect_w, y: 0.0 },
                        ArrowPoint {
                            x: rect_w,
                            y: -rect_h,
                        },
                        ArrowPoint { x: 0.0, y: -rect_h },
                        ArrowPoint { x: 0.0, y: 0.0 },
                        ArrowPoint { x: -8.0, y: 0.0 },
                        ArrowPoint {
                            x: rect_w + 8.0,
                            y: 0.0,
                        },
                        ArrowPoint {
                            x: rect_w + 8.0,
                            y: -rect_h,
                        },
                        ArrowPoint {
                            x: -8.0,
                            y: -rect_h,
                        },
                        ArrowPoint { x: -8.0, y: 0.0 },
                    ],
                    rect_w,
                    rect_h,
                );
            }
            "lean_right" => {
                emit_polygon(
                    &mut out,
                    &[
                        ArrowPoint {
                            x: (-2.0 * rect_h) / 6.0,
                            y: 0.0,
                        },
                        ArrowPoint {
                            x: rect_w - rect_h / 6.0,
                            y: 0.0,
                        },
                        ArrowPoint {
                            x: rect_w + (2.0 * rect_h) / 6.0,
                            y: -rect_h,
                        },
                        ArrowPoint {
                            x: rect_h / 6.0,
                            y: -rect_h,
                        },
                    ],
                    rect_w,
                    rect_h,
                );
            }
            "lean_left" => {
                emit_polygon(
                    &mut out,
                    &[
                        ArrowPoint {
                            x: (2.0 * rect_h) / 6.0,
                            y: 0.0,
                        },
                        ArrowPoint {
                            x: rect_w + rect_h / 6.0,
                            y: 0.0,
                        },
                        ArrowPoint {
                            x: rect_w - (2.0 * rect_h) / 6.0,
                            y: -rect_h,
                        },
                        ArrowPoint {
                            x: -rect_h / 6.0,
                            y: -rect_h,
                        },
                    ],
                    rect_w,
                    rect_h,
                );
            }
            "trapezoid" => {
                emit_polygon(
                    &mut out,
                    &[
                        ArrowPoint {
                            x: (-2.0 * rect_h) / 6.0,
                            y: 0.0,
                        },
                        ArrowPoint {
                            x: rect_w + (2.0 * rect_h) / 6.0,
                            y: 0.0,
                        },
                        ArrowPoint {
                            x: rect_w - rect_h / 6.0,
                            y: -rect_h,
                        },
                        ArrowPoint {
                            x: rect_h / 6.0,
                            y: -rect_h,
                        },
                    ],
                    rect_w,
                    rect_h,
                );
            }
            "inv_trapezoid" => {
                emit_polygon(
                    &mut out,
                    &[
                        ArrowPoint {
                            x: rect_h / 6.0,
                            y: 0.0,
                        },
                        ArrowPoint {
                            x: rect_w - rect_h / 6.0,
                            y: 0.0,
                        },
                        ArrowPoint {
                            x: rect_w + (2.0 * rect_h) / 6.0,
                            y: -rect_h,
                        },
                        ArrowPoint {
                            x: (-2.0 * rect_h) / 6.0,
                            y: -rect_h,
                        },
                    ],
                    rect_w,
                    rect_h,
                );
            }
            "composite" => {
                let _ = write!(
                    &mut out,
                    r#"<rect class="basic cluster composite label-container" rx="0" ry="0" style="" x="{}" y="{}" width="{}" height="{}"/>"#,
                    fmt(x),
                    fmt(y),
                    fmt(width),
                    fmt(height)
                );
            }
            "block_arrow" => {
                let h = (bbox_h + 2.0 * node_padding).max(1.0);
                let m = h / 2.0;
                let w = (bbox_w + 2.0 * m + node_padding).max(1.0);
                let pts = block_arrow_points(&node.directions, bbox_w, bbox_h, node_padding);

                out.push_str(r#"<polygon points=""#);
                for (idx, p) in pts.iter().enumerate() {
                    if idx > 0 {
                        out.push(' ');
                    }
                    let _ = write!(&mut out, "{},{}", fmt_display(p.x), fmt_display(p.y));
                }
                let _ = write!(
                    &mut out,
                    r#"" class="label-container" style="" transform="translate({},{})"/>"#,
                    fmt_display(-w / 2.0),
                    fmt_display(h / 2.0)
                );
            }
            "round" => {
                let _ = write!(
                    &mut out,
                    r#"<rect class="basic label-container" rx="5" ry="5" style="" x="{}" y="{}" width="{}" height="{}"/>"#,
                    fmt(x),
                    fmt(y),
                    fmt(width),
                    fmt(height)
                );
            }
            _ => {
                let _ = write!(
                    &mut out,
                    r#"<rect class="basic label-container" rx="0" ry="0" style="" x="{}" y="{}" width="{}" height="{}"/>"#,
                    fmt(x),
                    fmt(y),
                    fmt(width),
                    fmt(height)
                );
            }
        }

        let label = decode_block_label_html(&node.label);
        let (label_tx, label_ty, label_w, label_h) = if node.label.is_empty() {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            let label_w = n.label_width.unwrap_or(0.0).max(0.0);
            let label_h = n.label_height.unwrap_or(0.0).max(0.0);
            (-label_w / 2.0, -label_h / 2.0, label_w, label_h)
        };
        let _ = write!(
            &mut out,
            r#"<g class="label" style="" transform="translate({}, {})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: inline-block; white-space: nowrap;"><span class="nodeLabel">{}</span></div></foreignObject></g>"#,
            fmt(label_tx),
            fmt(label_ty),
            fmt(label_w),
            fmt(label_h),
            escape_xml(&label)
        );

        out.push_str("</g>");
    }

    for e in &model.edges {
        let Some(le) = layout.edges.iter().find(|x| x.id == e.id) else {
            continue;
        };
        let mut edge_points = match (
            layout_nodes_by_id.get(&e.start),
            layout_nodes_by_id.get(&e.end),
            nodes_by_id.get(&e.start),
            nodes_by_id.get(&e.end),
        ) {
            (Some(from), Some(to), Some(from_render), Some(to_render)) => {
                let mid = le.points.get(1).cloned().unwrap_or(LayoutPoint {
                    x: from.x + (to.x - from.x) / 2.0,
                    y: from.y + (to.y - from.y) / 2.0,
                });
                vec![
                    block_intersect_node(from, from_render, &mid, node_padding),
                    mid.clone(),
                    block_intersect_node(to, to_render, &mid, node_padding),
                ]
            }
            _ => le.points.clone(),
        };
        if edge_points.len() >= 2 {
            let start_inset = block_edge_start_marker_inset(e.arrow_type_start.as_deref());
            if start_inset > 0.0 {
                edge_points[0] = move_point_towards(&edge_points[0], &edge_points[1], start_inset);
            }
            let end_inset = block_edge_end_marker_inset(e.arrow_type_end.as_deref());
            if end_inset > 0.0 {
                let last = edge_points.len() - 1;
                edge_points[last] =
                    move_point_towards(&edge_points[last], &edge_points[last - 1], end_inset);
            }
        }
        let d = curve_basis_path_d(&edge_points);
        let class_attr = "edge-thickness-normal edge-pattern-solid flowchart-link LS-a1 LE-b1";
        let _ = write!(
            &mut out,
            r#"<path d="{}" id="{}" class="{}""#,
            escape_attr(&d),
            escape_attr(&e.id),
            escape_attr(class_attr)
        );

        if let Some(m) = edge_marker_start(e.arrow_type_start.as_deref()) {
            let _ = write!(
                &mut out,
                r#" marker-start="{}""#,
                escape_attr(&marker_url(diagram_id, m))
            );
        }
        if let Some(m) = edge_marker_end(e.arrow_type_end.as_deref()) {
            let _ = write!(
                &mut out,
                r#" marker-end="{}""#,
                escape_attr(&marker_url(diagram_id, m))
            );
        }
        out.push_str("/>");
    }

    for e in &model.edges {
        let Some(le) = layout.edges.iter().find(|x| x.id == e.id) else {
            continue;
        };
        let Some(lbl) = le.label.as_ref().filter(|_| !e.label.trim().is_empty()) else {
            continue;
        };

        let _ = write!(
            &mut out,
            r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" transform="translate({}, {})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="stroke: rgb(51, 51, 51); stroke-width: 1.5px; display: inline-block; white-space: nowrap;"><span class="edgeLabel" style="stroke: #333; stroke-width: 1.5px;color:none;">{}</span></div></foreignObject></g></g>"#,
            fmt(lbl.x),
            fmt(lbl.y),
            fmt(-lbl.width / 2.0),
            fmt(-lbl.height / 2.0),
            fmt(lbl.width),
            fmt(lbl.height),
            escape_xml(&decode_block_label_html(&e.label))
        );
    }

    out.push_str("</g></svg>\n");
    Ok(out)
}

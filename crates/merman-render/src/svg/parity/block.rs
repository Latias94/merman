use super::*;
use crate::block::{BlockArrowPoint as ArrowPoint, block_arrow_points};

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
        let d = curve_basis_path_d(&le.points);
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
            r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" transform="translate({}, {})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: inline-block; white-space: nowrap;"><span class="edgeLabel">{}</span></div></foreignObject></g></g>"#,
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

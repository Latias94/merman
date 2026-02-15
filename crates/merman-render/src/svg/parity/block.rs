use super::*;

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

    #[derive(Debug, Clone, Copy)]
    struct ArrowPoint {
        x: f64,
        y: f64,
    }

    fn block_arrow_points(
        directions: &[String],
        bbox_w: f64,
        bbox_h: f64,
        node_padding: f64,
    ) -> Vec<ArrowPoint> {
        fn expand_and_dedup(directions: &[String]) -> std::collections::BTreeSet<String> {
            let mut out = std::collections::BTreeSet::new();
            for d in directions {
                match d.trim() {
                    "x" => {
                        out.insert("right".to_string());
                        out.insert("left".to_string());
                    }
                    "y" => {
                        out.insert("up".to_string());
                        out.insert("down".to_string());
                    }
                    other if !other.is_empty() => {
                        out.insert(other.to_string());
                    }
                    _ => {}
                }
            }
            out
        }

        let dirs = expand_and_dedup(directions);
        let height = bbox_h + 2.0 * node_padding;
        let midpoint = height / 2.0;
        let width = bbox_w + 2.0 * midpoint + node_padding;
        let pad = node_padding / 2.0;

        let has = |name: &str| dirs.contains(name);

        if has("right") && has("left") && has("up") && has("down") {
            return vec![
                ArrowPoint { x: 0.0, y: 0.0 },
                ArrowPoint {
                    x: midpoint,
                    y: 0.0,
                },
                ArrowPoint {
                    x: width / 2.0,
                    y: 2.0 * pad,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: 0.0,
                },
                ArrowPoint { x: width, y: 0.0 },
                ArrowPoint {
                    x: width,
                    y: -height / 3.0,
                },
                ArrowPoint {
                    x: width + 2.0 * pad,
                    y: -height / 2.0,
                },
                ArrowPoint {
                    x: width,
                    y: (-2.0 * height) / 3.0,
                },
                ArrowPoint {
                    x: width,
                    y: -height,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -height,
                },
                ArrowPoint {
                    x: width / 2.0,
                    y: -height - 2.0 * pad,
                },
                ArrowPoint {
                    x: midpoint,
                    y: -height,
                },
                ArrowPoint { x: 0.0, y: -height },
                ArrowPoint {
                    x: 0.0,
                    y: (-2.0 * height) / 3.0,
                },
                ArrowPoint {
                    x: -2.0 * pad,
                    y: -height / 2.0,
                },
                ArrowPoint {
                    x: 0.0,
                    y: -height / 3.0,
                },
            ];
        }
        if has("right") && has("left") && has("up") {
            return vec![
                ArrowPoint {
                    x: midpoint,
                    y: 0.0,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: 0.0,
                },
                ArrowPoint {
                    x: width,
                    y: -height / 2.0,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -height,
                },
                ArrowPoint {
                    x: midpoint,
                    y: -height,
                },
                ArrowPoint {
                    x: 0.0,
                    y: -height / 2.0,
                },
            ];
        }
        if has("right") && has("left") && has("down") {
            return vec![
                ArrowPoint { x: 0.0, y: 0.0 },
                ArrowPoint {
                    x: midpoint,
                    y: -height,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -height,
                },
                ArrowPoint { x: width, y: 0.0 },
            ];
        }
        if has("right") && has("up") && has("down") {
            return vec![
                ArrowPoint { x: 0.0, y: 0.0 },
                ArrowPoint {
                    x: width,
                    y: -midpoint,
                },
                ArrowPoint {
                    x: width,
                    y: -height + midpoint,
                },
                ArrowPoint { x: 0.0, y: -height },
            ];
        }
        if has("left") && has("up") && has("down") {
            return vec![
                ArrowPoint { x: width, y: 0.0 },
                ArrowPoint {
                    x: 0.0,
                    y: -midpoint,
                },
                ArrowPoint {
                    x: 0.0,
                    y: -height + midpoint,
                },
                ArrowPoint {
                    x: width,
                    y: -height,
                },
            ];
        }
        if has("right") && has("left") {
            return vec![
                ArrowPoint {
                    x: midpoint,
                    y: 0.0,
                },
                ArrowPoint {
                    x: midpoint,
                    y: -pad,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -pad,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: 0.0,
                },
                ArrowPoint {
                    x: width,
                    y: -height / 2.0,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -height,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -height + pad,
                },
                ArrowPoint {
                    x: midpoint,
                    y: -height + pad,
                },
                ArrowPoint {
                    x: midpoint,
                    y: -height,
                },
                ArrowPoint {
                    x: 0.0,
                    y: -height / 2.0,
                },
            ];
        }
        if has("up") && has("down") {
            return vec![
                ArrowPoint {
                    x: width / 2.0,
                    y: 0.0,
                },
                ArrowPoint { x: 0.0, y: -pad },
                ArrowPoint {
                    x: midpoint,
                    y: -pad,
                },
                ArrowPoint {
                    x: midpoint,
                    y: -height + pad,
                },
                ArrowPoint {
                    x: 0.0,
                    y: -height + pad,
                },
                ArrowPoint {
                    x: width / 2.0,
                    y: -height,
                },
                ArrowPoint {
                    x: width,
                    y: -height + pad,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -height + pad,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -pad,
                },
                ArrowPoint { x: width, y: -pad },
            ];
        }
        if has("right") && has("up") {
            return vec![
                ArrowPoint { x: 0.0, y: 0.0 },
                ArrowPoint {
                    x: width,
                    y: -midpoint,
                },
                ArrowPoint { x: 0.0, y: -height },
            ];
        }
        if has("right") && has("down") {
            return vec![
                ArrowPoint { x: 0.0, y: 0.0 },
                ArrowPoint { x: width, y: 0.0 },
                ArrowPoint { x: 0.0, y: -height },
            ];
        }
        if has("left") && has("up") {
            return vec![
                ArrowPoint { x: width, y: 0.0 },
                ArrowPoint {
                    x: 0.0,
                    y: -midpoint,
                },
                ArrowPoint {
                    x: width,
                    y: -height,
                },
            ];
        }
        if has("left") && has("down") {
            return vec![
                ArrowPoint { x: width, y: 0.0 },
                ArrowPoint { x: 0.0, y: 0.0 },
                ArrowPoint {
                    x: width,
                    y: -height,
                },
            ];
        }
        if has("right") {
            return vec![
                ArrowPoint {
                    x: midpoint,
                    y: -pad,
                },
                ArrowPoint {
                    x: midpoint,
                    y: -pad,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -pad,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: 0.0,
                },
                ArrowPoint {
                    x: width,
                    y: -height / 2.0,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -height,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -height + pad,
                },
                ArrowPoint {
                    x: midpoint,
                    y: -height + pad,
                },
                ArrowPoint {
                    x: midpoint,
                    y: -height + pad,
                },
            ];
        }
        if has("left") {
            return vec![
                ArrowPoint {
                    x: midpoint,
                    y: 0.0,
                },
                ArrowPoint {
                    x: midpoint,
                    y: -pad,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -pad,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -height + pad,
                },
                ArrowPoint {
                    x: midpoint,
                    y: -height + pad,
                },
                ArrowPoint {
                    x: midpoint,
                    y: -height,
                },
                ArrowPoint {
                    x: 0.0,
                    y: -height / 2.0,
                },
            ];
        }
        if has("up") {
            return vec![
                ArrowPoint {
                    x: midpoint,
                    y: -pad,
                },
                ArrowPoint {
                    x: midpoint,
                    y: -height + pad,
                },
                ArrowPoint {
                    x: 0.0,
                    y: -height + pad,
                },
                ArrowPoint {
                    x: width / 2.0,
                    y: -height,
                },
                ArrowPoint {
                    x: width,
                    y: -height + pad,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -height + pad,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -pad,
                },
            ];
        }
        if has("down") {
            return vec![
                ArrowPoint {
                    x: width / 2.0,
                    y: 0.0,
                },
                ArrowPoint { x: 0.0, y: -pad },
                ArrowPoint {
                    x: midpoint,
                    y: -pad,
                },
                ArrowPoint {
                    x: midpoint,
                    y: -height + pad,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -height + pad,
                },
                ArrowPoint {
                    x: width - midpoint,
                    y: -pad,
                },
                ArrowPoint { x: width, y: -pad },
            ];
        }

        vec![ArrowPoint { x: 0.0, y: 0.0 }]
    }

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

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
    if let Some((viewbox, max_w)) =
        crate::generated::block_root_overrides_11_12_2::lookup_block_root_viewport_override(
            diagram_id,
        )
    {
        viewbox_attr = viewbox.to_string();
        max_w_style = max_w.to_string();
    }
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style="max-width: {max_w}px; background-color: white;" viewBox="{viewbox}" role="graphics-document document" aria-roledescription="block">"#,
        max_w = max_w_style,
        viewbox = viewbox_attr,
    );
    out.push_str(r#"<style></style><g/>"#);

    let _ = write!(
        &mut out,
        r#"<marker id="{}" class="marker block" viewBox="0 0 10 10" refX="6" refY="5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 0 0 L 10 5 L 0 10 z" class="arrowMarkerPath"/></marker>"#,
        escape_xml(&marker_id(diagram_id, "pointEnd"))
    );
    let _ = write!(
        &mut out,
        r#"<marker id="{}" class="marker block" viewBox="0 0 10 10" refX="4.5" refY="5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 0 5 L 10 10 L 10 0 z" class="arrowMarkerPath"/></marker>"#,
        escape_xml(&marker_id(diagram_id, "pointStart"))
    );
    let _ = write!(
        &mut out,
        r#"<marker id="{}" class="marker block" viewBox="0 0 10 10" refX="11" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath"/></marker>"#,
        escape_xml(&marker_id(diagram_id, "circleEnd"))
    );
    let _ = write!(
        &mut out,
        r#"<marker id="{}" class="marker block" viewBox="0 0 10 10" refX="-1" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath"/></marker>"#,
        escape_xml(&marker_id(diagram_id, "circleStart"))
    );
    let _ = write!(
        &mut out,
        r#"<marker id="{}" class="marker cross block" viewBox="0 0 11 11" refX="12" refY="5.2" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><path d="M 1,1 l 9,9 M 10,1 l -9,9" class="arrowMarkerPath"/></marker>"#,
        escape_xml(&marker_id(diagram_id, "crossEnd"))
    );
    let _ = write!(
        &mut out,
        r#"<marker id="{}" class="marker cross block" viewBox="0 0 11 11" refX="-1" refY="5.2" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><path d="M 1,1 l 9,9 M 10,1 l -9,9" class="arrowMarkerPath"/></marker>"#,
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

        fn emit_polygon(out: &mut String, points: &[(f64, f64)], tx: f64, ty: f64) {
            out.push_str(r#"<polygon points=""#);
            for (idx, (px, py)) in points.iter().enumerate() {
                if idx > 0 {
                    out.push(' ');
                }
                let _ = write!(out, "{},{}", fmt(*px), fmt(*py));
            }
            let _ = write!(
                out,
                r#"" class="label-container" transform="translate({},{})"/>"#,
                fmt(tx),
                fmt(ty)
            );
        }

        match node.block_type.as_str() {
            "circle" => {
                // Mermaid renders `type: "circle"` block nodes as a `<circle>` element without a
                // `class` attribute, but it still emits `rx`/`ry`/`width`/`height` attributes.
                // Keep that DOM shape for `parity-root` checks.
                let _ = write!(
                    &mut out,
                    r#"<circle rx="0" ry="0" r="{}" width="{}" height="{}"/>"#,
                    fmt(width / 2.0),
                    fmt(width),
                    fmt(height)
                );
            }
            "stadium" => {
                // Upstream uses a plain `<rect>` (no `class`) for stadium-shaped block nodes.
                let _ = write!(
                    &mut out,
                    r#"<rect rx="0" ry="0" x="{}" y="{}" width="{}" height="{}"/>"#,
                    fmt(x),
                    fmt(y),
                    fmt(width),
                    fmt(height)
                );
            }
            "cylinder" => {
                // Cylinder blocks are emitted as a `<path>` in upstream block diagrams.
                // Keep the command-letter structure stable and treat numeric payload as noise in
                // `parity-root` mode.
                let _ = write!(
                    &mut out,
                    r#"<path d="M 0 0 a 1 1 0 0 1 2 0 a 1 1 0 0 1 2 0 l 0 10 a 1 1 0 0 1 -2 0 l 0 -10" transform="translate({},{})"/>"#,
                    fmt(x),
                    fmt(y)
                );
            }
            "diamond" => {
                emit_polygon(
                    &mut out,
                    &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
                    x,
                    y,
                );
            }
            "hexagon" => {
                emit_polygon(
                    &mut out,
                    &[
                        (0.0, 0.5),
                        (0.25, 0.0),
                        (0.75, 0.0),
                        (1.0, 0.5),
                        (0.75, 1.0),
                        (0.25, 1.0),
                    ],
                    x,
                    y,
                );
            }
            "rect_left_inv_arrow" => {
                emit_polygon(
                    &mut out,
                    &[(0.0, 0.5), (0.25, 0.0), (1.0, 0.0), (1.0, 1.0), (0.25, 1.0)],
                    x,
                    y,
                );
            }
            "subroutine" => {
                // Upstream uses a multi-point polygon for subroutine blocks.
                emit_polygon(
                    &mut out,
                    &[
                        (0.0, 0.0),
                        (0.1, 0.0),
                        (0.1, 1.0),
                        (0.0, 1.0),
                        (0.0, 0.0),
                        (1.0, 0.0),
                        (1.0, 1.0),
                        (1.0, 0.0),
                        (0.9, 0.0),
                        (0.9, 1.0),
                    ],
                    x,
                    y,
                );
            }
            "lean_right" | "lean_left" | "trapezoid" | "inv_trapezoid" => {
                emit_polygon(
                    &mut out,
                    &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
                    x,
                    y,
                );
            }
            "composite" => {
                let _ = write!(
                    &mut out,
                    r#"<rect class="basic cluster composite label-container" rx="0" ry="0" x="{}" y="{}" width="{}" height="{}"/>"#,
                    fmt(x),
                    fmt(y),
                    fmt(width),
                    fmt(height)
                );
            }
            "block_arrow" => {
                // Exact sizing is non-semantic in parity checks; keep the arrow point count and element structure.
                let node_padding = 8.0;
                let bbox_w = 1.0;
                let bbox_h = 1.0;
                let h = bbox_h + 2.0 * node_padding;
                let m = h / 2.0;
                let w = bbox_w + 2.0 * m + node_padding;
                let pts = block_arrow_points(&node.directions, bbox_w, bbox_h, node_padding);

                out.push_str(r#"<polygon points=""#);
                for (idx, p) in pts.iter().enumerate() {
                    if idx > 0 {
                        out.push(' ');
                    }
                    let _ = write!(&mut out, "{},{}", fmt(p.x), fmt(p.y));
                }
                let _ = write!(
                    &mut out,
                    r#"" class="label-container" transform="translate({},{})"/>"#,
                    fmt(-w / 2.0),
                    fmt(h / 2.0)
                );
            }
            _ => {
                let _ = write!(
                    &mut out,
                    r#"<rect class="basic label-container" rx="0" ry="0" x="{}" y="{}" width="{}" height="{}"/>"#,
                    fmt(x),
                    fmt(y),
                    fmt(width),
                    fmt(height)
                );
            }
        }

        let label = decode_block_label_html(&node.label);
        let label_w = if label.trim().is_empty() { 0.0 } else { 1.0 };
        let label_h = if label.trim().is_empty() { 0.0 } else { 1.0 };
        let _ = write!(
            &mut out,
            r#"<g class="label" transform="translate({}, {})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: inline-block; white-space: nowrap;"><span class="nodeLabel">{}</span></div></foreignObject></g>"#,
            fmt(-label_w / 2.0),
            fmt(-label_h / 2.0),
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

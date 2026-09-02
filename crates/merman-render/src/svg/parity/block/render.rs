use super::super::*;
use crate::block::{BlockRectangleKind, BlockShapeBoundary, block_label_is_effectively_empty};
use crate::model::LayoutPoint;

// Block diagram SVG renderer implementation (split from parity.rs).

pub(crate) fn render_block_diagram_svg_model(
    layout: &BlockDiagramLayout,
    model: &merman_core::diagrams::block::BlockDiagramRenderModel,
    effective_config: &serde_json::Value,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
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
        styles: Vec<String>,
        directions: Vec<String>,
    }

    fn collect_nodes(
        root: &crate::block::BlockNode,
        out: &mut std::collections::HashMap<String, RenderNode>,
    ) {
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            if let Some(existing) = out.get_mut(&n.id) {
                if !n.label.is_empty() {
                    existing.label = n.label.clone();
                }
                if !n.block_type.is_empty() && n.block_type != "na" {
                    existing.block_type = n.block_type.clone();
                }
                if !n.classes.is_empty() {
                    existing.classes = n.classes.clone();
                }
                if !n.styles.is_empty() {
                    existing.styles = n.styles.clone();
                }
                if !n.directions.is_empty() {
                    existing.directions = n.directions.clone();
                }
            } else {
                out.insert(
                    n.id.clone(),
                    RenderNode {
                        label: n.label.clone(),
                        block_type: n.block_type.clone(),
                        classes: n.classes.clone(),
                        styles: n.styles.clone(),
                        directions: n.directions.clone(),
                    },
                );
            }
            for child in n.children.iter().rev() {
                stack.push(child);
            }
        }
    }

    let mut nodes_by_id: std::collections::HashMap<String, RenderNode> =
        std::collections::HashMap::new();
    for n in &model.blocks_flat {
        collect_nodes(n, &mut nodes_by_id);
    }
    let shape_geometries_by_id: std::collections::HashMap<_, _> = layout
        .shape_geometries
        .iter()
        .map(|geometry| (geometry.id.as_str(), geometry))
        .collect();

    fn marker_id(diagram_id: SvgDiagramId<'_>, marker: &str) -> String {
        format!("{diagram_id}_block-{marker}")
    }

    fn marker_url(diagram_id: SvgDiagramId<'_>, marker: &str) -> String {
        format!("url(#{})", marker_id(diagram_id, marker))
    }

    fn dom_id(diagram_id: SvgDiagramId<'_>, raw_id: &str) -> String {
        if diagram_id.semantic_str().is_empty() {
            raw_id.to_string()
        } else {
            format!("{diagram_id}-{raw_id}")
        }
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

    fn push_ordered_decl(out: &mut Vec<(String, String)>, key: &str, raw: &str) {
        if let Some((_, value)) = out.iter_mut().find(|(existing, _)| existing == key) {
            *value = raw.to_string();
            return;
        }
        out.push((key.to_string(), raw.to_string()));
    }

    fn compile_block_inline_styles(styles: &[String]) -> (String, String, String) {
        let mut box_decls: Vec<(String, String)> = Vec::new();
        let mut text_decls: Vec<(String, String)> = Vec::new();

        for raw in styles {
            let trimmed = raw.trim().trim_end_matches(';').trim();
            if trimmed.is_empty() {
                continue;
            }
            let Some((key, value)) = parse_style_decl(trimmed) else {
                let decoded = decode_mermaid_entities_for_render_text(trimmed);
                let decoded = decoded.as_ref().trim();
                if !decoded.is_empty() {
                    push_ordered_decl(&mut box_decls, decoded, decoded);
                }
                continue;
            };
            if is_rect_style_key(key) {
                push_ordered_decl(&mut box_decls, key, trimmed);
            }
            if is_text_style_key(key) {
                let _ = value;
                push_ordered_decl(&mut text_decls, key, trimmed);
            }
        }

        let style_attr = |decls: &[(String, String)]| -> String {
            let mut out = String::new();
            for (_, raw) in decls {
                out.push_str(raw);
                out.push(';');
            }
            out
        };

        let mut div_prefix = String::new();
        for (key, raw) in &text_decls {
            if key == "color" {
                let value = raw.split_once(':').map(|(_, v)| v.trim()).unwrap_or("");
                if !value.is_empty() {
                    let _ = write!(
                        &mut div_prefix,
                        "color: {}; ",
                        super::super::util::cssom_color_value(value)
                    );
                }
            } else {
                div_prefix.push_str(raw);
                div_prefix.push_str("; ");
            }
        }

        (style_attr(&box_decls), style_attr(&text_decls), div_prefix)
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

    fn block_class_css(
        diagram_id: SvgDiagramId<'_>,
        class_defs: &indexmap::IndexMap<
            String,
            merman_core::diagrams::block::BlockClassDefRenderModel,
        >,
        options: &SvgExecution<'_>,
    ) -> Result<String> {
        fn important_declarations(styles: &[String]) -> String {
            let mut out = String::new();
            for style in styles {
                let Some((key, value)) = parse_style_decl(style) else {
                    continue;
                };
                let _ = write!(&mut out, "{key}:{}!important;", escape_xml(value));
            }
            out
        }

        let mut out = String::new();
        for class_def in class_defs.values() {
            options.checkpoint_emit()?;
            let class = escape_xml(&class_def.id);
            let shape_style = important_declarations(&class_def.styles);
            if !shape_style.is_empty() {
                let _ = write!(
                    &mut out,
                    r#"#{} .{}&gt;*{{{}}}#{} .{} span{{{}}}"#,
                    diagram_id,
                    class.as_str(),
                    shape_style,
                    diagram_id,
                    class.as_str(),
                    shape_style
                );
            }

            let text_style = important_declarations(&class_def.text_styles);
            if !text_style.is_empty() {
                let _ = write!(
                    &mut out,
                    r#"#{} .{} tspan{{{}}}"#,
                    diagram_id,
                    class.as_str(),
                    text_style
                );
            }
            options.checkpoint_emit()?;
        }
        Ok(out)
    }

    fn block_css(
        diagram_id: SvgDiagramId<'_>,
        effective_config: &serde_json::Value,
        class_defs: &indexmap::IndexMap<
            String,
            merman_core::diagrams::block::BlockClassDefRenderModel,
        >,
        options: &SvgExecution<'_>,
    ) -> Result<String> {
        let theme = PresentationTheme::new(effective_config).node_diagram();
        let font_family = theme.common.font_family_css.as_str();
        let font_size = theme.common.font_size_px;
        let text_color = theme.common.text_color.as_str();
        let node_text_color = theme.node_text_color.as_str();
        let title_color = theme.title_color.as_str();
        let main_bkg = theme.main_bkg.as_str();
        let node_border = theme.node_border.as_str();
        let line_color = theme.common.line_color.as_str();
        let arrowhead_color = theme.arrowhead_color.as_str();
        let stroke_width = theme.stroke_width.as_str();
        let edge_label_background = theme.edge_label_background.as_str();
        let cluster_bkg = theme.cluster_bkg.as_str();
        let cluster_border = theme.cluster_border.as_str();
        let cluster_bkg = css_rgba_fade(cluster_bkg, 0.5)?;
        let cluster_border = css_rgba_fade(cluster_border, 0.2)?;

        let mut out = String::new();
        let _ = write!(
            &mut out,
            r#"#{}{{font-family:{};font-size:{}px;fill:{};}}"#,
            diagram_id,
            font_family,
            fmt(font_size),
            node_text_color
        );
        let _ = write!(
            &mut out,
            r#"#{} .edge-thickness-normal{{stroke-width:{}px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
            diagram_id, stroke_width, diagram_id, diagram_id, diagram_id, diagram_id, diagram_id
        );
        let _ = write!(
            &mut out,
            r#"#{} .label{{font-family:{};color:{};}}#{} p{{margin:0;}}#{} .label text,#{} span,#{} p{{fill:{};color:{};}}"#,
            diagram_id,
            font_family,
            node_text_color,
            diagram_id,
            diagram_id,
            diagram_id,
            diagram_id,
            node_text_color,
            node_text_color
        );
        let _ = write!(
            &mut out,
            r#"#{} .cluster-label text{{fill:{};}}#{} .cluster-label span,#{} .cluster-label p{{color:{};}}"#,
            diagram_id, title_color, diagram_id, diagram_id, title_color
        );
        let _ = write!(
            &mut out,
            r#"#{} .node rect,#{} .node circle,#{} .node ellipse,#{} .node polygon,#{} .node path{{fill:{};stroke:{};stroke-width:1px;}}#{} .flowchart-label text{{text-anchor:middle;}}#{} .node .label{{text-align:center;}}#{} .node.clickable{{cursor:pointer;}}"#,
            diagram_id,
            diagram_id,
            diagram_id,
            diagram_id,
            diagram_id,
            main_bkg,
            node_border,
            diagram_id,
            diagram_id,
            diagram_id
        );
        let _ = write!(
            &mut out,
            r#"#{} .arrowheadPath,#{} .arrowMarkerPath{{fill:{};stroke:{};}}#{} .edgePath .path{{stroke:{};stroke-width:2.0px;}}#{} .flowchart-link{{stroke:{};fill:none;}}"#,
            diagram_id,
            diagram_id,
            arrowhead_color,
            line_color,
            diagram_id,
            line_color,
            diagram_id,
            line_color
        );
        let _ = write!(
            &mut out,
            r#"#{} .edgeLabel{{background-color:{};text-align:center;}}#{} .edgeLabel p{{margin:0;padding:0;display:inline;}}#{} .edgeLabel rect{{opacity:0.5;background-color:{};fill:{};}}#{} .labelBkg{{background-color:{}}}"#,
            diagram_id,
            edge_label_background,
            diagram_id,
            diagram_id,
            edge_label_background,
            edge_label_background,
            diagram_id,
            edge_label_background
        );
        let _ = write!(
            &mut out,
            r#"#{} .node .cluster{{fill:{};stroke:{};stroke-width:1px;}}#{} .cluster text{{fill:{};}}#{} .cluster span,#{} .cluster p{{color:{};}}#{} .flowchartTitleText{{text-anchor:middle;font-size:18px;fill:{};}}#{} :root{{--mermaid-font-family:{};}}"#,
            diagram_id,
            cluster_bkg,
            cluster_border,
            diagram_id,
            title_color,
            diagram_id,
            diagram_id,
            title_color,
            diagram_id,
            text_color,
            diagram_id,
            font_family
        );
        out.push_str(&block_class_css(diagram_id, class_defs, options)?);
        Ok(out)
    }

    let diagram_id = options.diagram_id_or("merman");

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    let diagram_padding = config_f64(effective_config, &["block", "diagramPadding"])
        .unwrap_or(5.0)
        .max(0.0);

    let mut out = String::new();
    let root_bounds = root_svg::DiagramBounds::from_extents(
        bounds.min_x,
        bounds.min_y,
        bounds.max_x,
        bounds.max_y,
        diagram_padding,
    );
    let root_spec = root_svg::RootViewportSpec::responsive(root_bounds)
        .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(root_bounds.width));
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "block");
    root_chrome.dom.trailing_newline = false;
    let root_document =
        root_svg::RootViewportContext::new(crate::family::RenderFamilyKind::Block, diagram_id)
            .write_open(&mut out, root_spec, root_chrome)?;
    options.checkpoint_emit()?;
    out.push_str("<style>");
    out.push_str(&block_css(
        diagram_id,
        effective_config,
        &model.class_defs,
        options,
    )?);
    out.push_str("</style><g/>");
    options.checkpoint_emit()?;

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
    options.checkpoint_emit()?;

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
        let (node_box_style, node_text_style, node_div_style_prefix) =
            compile_block_inline_styles(&node.styles);

        let geometry =
            shape_geometries_by_id
                .get(n.id.as_str())
                .ok_or_else(|| Error::InvalidModel {
                    message: format!("missing Block shape geometry for node `{}`", n.id),
                })?;
        let id_attr = format!(r#" id="{}""#, escape_attr(&dom_id(diagram_id, &n.id)));
        options.checkpoint_emit()?;
        let _ = write!(
            &mut out,
            r#"<g class="node default {}"{} transform="translate({}, {})">"#,
            escape_attr(&class_str),
            id_attr,
            fmt(geometry.allocated.x),
            fmt(geometry.allocated.y)
        );

        match &geometry.boundary {
            BlockShapeBoundary::Rectangle {
                width,
                height,
                radius,
                kind,
            } => {
                let class = match kind {
                    BlockRectangleKind::Basic => "basic label-container",
                    BlockRectangleKind::Composite => "basic cluster composite label-container",
                };
                let _ = write!(
                    &mut out,
                    r#"<rect class="{}" rx="{}" ry="{}" style="{}" x="{}" y="{}" width="{}" height="{}"/>"#,
                    class,
                    fmt(*radius),
                    fmt(*radius),
                    escape_attr(&node_box_style),
                    fmt(-width / 2.0),
                    fmt(-height / 2.0),
                    fmt(*width),
                    fmt(*height)
                );
            }
            BlockShapeBoundary::Circle {
                radius,
                width_attribute,
                height_attribute,
            } => {
                let _ = write!(
                    &mut out,
                    r#"<circle style="{}" rx="0" ry="0" r="{}" width="{}" height="{}"/>"#,
                    escape_attr(&node_box_style),
                    fmt(*radius),
                    fmt(*width_attribute),
                    fmt(*height_attribute)
                );
            }
            BlockShapeBoundary::DoubleCircle {
                outer_radius,
                inner_radius,
                inner_width_attribute,
                inner_height_attribute,
            } => {
                let _ = write!(
                    &mut out,
                    r#"<g class="default flowchart-label"><circle style="{}" rx="0" ry="0" r="{}" width="{}" height="{}"/><circle style="{}" rx="0" ry="0" r="{}" width="{}" height="{}"/></g>"#,
                    escape_attr(&node_box_style),
                    fmt(*outer_radius),
                    fmt(inner_width_attribute + 10.0),
                    fmt(inner_height_attribute + 10.0),
                    escape_attr(&node_box_style),
                    fmt(*inner_radius),
                    fmt(*inner_width_attribute),
                    fmt(*inner_height_attribute)
                );
            }
            BlockShapeBoundary::Stadium { width, height } => {
                let radius = height / 2.0;
                let _ = write!(
                    &mut out,
                    r#"<rect rx="{}" ry="{}" style="{}" x="{}" y="{}" width="{}" height="{}"/>"#,
                    fmt(radius),
                    fmt(radius),
                    escape_attr(&node_box_style),
                    fmt(-width / 2.0),
                    fmt(-height / 2.0),
                    fmt(*width),
                    fmt(*height)
                );
            }
            BlockShapeBoundary::Cylinder {
                width,
                body_height,
                radius_x,
                radius_y,
            } => {
                let _ = write!(
                    &mut out,
                    r#"<path d="M {},{} a {},{} 0,0,0 {} 0 a {},{} 0,0,0 {} 0 l 0,{} a {},{} 0,0,0 {} 0 l 0,{}" style="{}" transform="translate({},{})"/>"#,
                    fmt_display(0.0),
                    fmt_display(*radius_y),
                    fmt_display(*radius_x),
                    fmt_display(*radius_y),
                    fmt_display(*width),
                    fmt_display(*radius_x),
                    fmt_display(*radius_y),
                    fmt_display(-width),
                    fmt_display(*body_height),
                    fmt_display(*radius_x),
                    fmt_display(*radius_y),
                    fmt_display(*width),
                    fmt_display(-body_height),
                    escape_attr(&node_box_style),
                    fmt_display(-width / 2.0),
                    fmt_display(-(body_height / 2.0 + radius_y))
                );
            }
            BlockShapeBoundary::Polygon {
                points,
                translation,
            } => {
                out.push_str(r#"<polygon points=""#);
                for (index, point) in points.iter().enumerate() {
                    if index > 0 {
                        out.push(' ');
                    }
                    let _ = write!(
                        &mut out,
                        "{},{}",
                        fmt_display(point.x),
                        fmt_display(point.y)
                    );
                }
                let _ = write!(
                    &mut out,
                    r#"" class="label-container" style="{}" transform="translate({},{})"/>"#,
                    escape_attr(&node_box_style),
                    fmt_display(translation.x),
                    fmt_display(translation.y)
                );
            }
        }

        let label = decode_block_label_html(&node.label);
        let label_effectively_empty =
            node.label.is_empty() || block_label_is_effectively_empty(&label);
        let (label_tx, label_ty, label_w, label_h) = if label_effectively_empty {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            let label_w = n.label_width.unwrap_or(0.0).max(0.0);
            let label_h = n.label_height.unwrap_or(0.0).max(0.0);
            (-label_w / 2.0, -label_h / 2.0, label_w, label_h)
        };
        let span_style_attr = if node_text_style.is_empty() {
            String::new()
        } else {
            format!(r#" style="{}""#, escape_attr(&node_text_style))
        };
        let label_markup = if node.label.is_empty() {
            String::new()
        } else {
            format!("<p>{}</p>", escape_xml(&label))
        };
        let _ = write!(
            &mut out,
            r#"<g class="label" style="{}" transform="translate({}, {})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}display: table-cell; white-space: nowrap; line-height: 1.5;"><span class="nodeLabel"{}>{}</span></div></foreignObject></g>"#,
            escape_attr(&node_text_style),
            fmt(label_tx),
            fmt(label_ty),
            fmt(label_w),
            fmt(label_h),
            escape_attr(&node_div_style_prefix),
            span_style_attr,
            label_markup
        );

        out.push_str("</g>");
    }

    for e in &model.edges {
        let Some(le) = layout.edges.iter().find(|x| x.id == e.id) else {
            continue;
        };
        let mut edge_points = match (
            shape_geometries_by_id.get(e.start.as_str()),
            shape_geometries_by_id.get(e.end.as_str()),
        ) {
            (Some(from), Some(to)) => {
                let mid = le.points.get(1).cloned().unwrap_or(LayoutPoint {
                    x: from.allocated.x + (to.allocated.x - from.allocated.x) / 2.0,
                    y: from.allocated.y + (to.allocated.y - from.allocated.y) / 2.0,
                });
                vec![from.intersect(&mid), mid.clone(), to.intersect(&mid)]
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
        let class_attr = "edge-thickness-normal edge-pattern-solid edge-thickness-normal edge-pattern-solid flowchart-link LS-a1 LE-b1";
        let _ = write!(
            &mut out,
            r#"<path d="{}" id="{}" class="{}""#,
            escape_attr(&d),
            escape_attr(&dom_id(diagram_id, &e.id)),
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
        options.checkpoint_emit()?;
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
            r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" transform="translate({}, {})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="stroke: rgb(51, 51, 51); stroke-width: 1.5px; display: table-cell; white-space: nowrap; line-height: 1.5;"><span class="edgeLabel" style="stroke: #333; stroke-width: 1.5px;color:none;"><p>{}</p></span></div></foreignObject></g></g>"#,
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
    options.checkpoint_emit()?;
    root_document.complete(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::RenderEnvironment;
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};

    #[test]
    fn diagram_id_terminal_precedes_later_block_model_validation() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, 1)
            .expect("valid SVG byte limit");
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .expect("begin render session");
        let request = SvgRenderOptions {
            diagram_id: Some("terminal".to_string()),
            ..SvgRenderOptions::default()
        };
        let debug = SvgDebugOptions::default();
        let options = SvgExecution::new(&request, &debug, &session).expect("SVG execution");
        let layout = BlockDiagramLayout {
            nodes: vec![LayoutNode {
                id: "node".to_string(),
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
                is_cluster: false,
                label_width: None,
                label_height: None,
            }],
            edges: Vec::new(),
            shape_geometries: Vec::new(),
            bounds: None,
        };
        let model: merman_core::diagrams::block::BlockDiagramRenderModel =
            serde_json::from_value(serde_json::json!({
                "blocksFlat": [{ "id": "node" }]
            }))
            .expect("valid Block render model");

        let error =
            render_block_diagram_svg_model(&layout, &model, &serde_json::json!({}), &options)
                .expect_err("diagram-id projection must stop before missing geometry validation");
        let Error::ResourceLimitExceeded(details) = error else {
            panic!("expected SVG byte resource rejection");
        };
        assert_eq!(details.limit, ResourceLimitId::MaxSvgBytes.as_str());
    }
}

//! Flowchart CSS generation.

use super::*;

pub(in crate::svg::parity) fn flowchart_css(
    diagram_id: &str,
    effective_config: &serde_json::Value,
    font_family: &str,
    font_size: f64,
    class_defs: &IndexMap<String, Vec<String>>,
) -> Result<String> {
    let id = escape_xml(diagram_id);
    let theme = PresentationTheme::new(effective_config).node_diagram();
    let stroke = theme.common.line_color.as_str();
    let arrowhead_color = theme.arrowhead_color.as_str();
    let node_border = theme.node_border.as_str();
    let main_bkg = theme.main_bkg.as_str();
    let text_color = theme.common.text_color.as_str();
    let node_text_color = theme.node_text_color.as_str();
    let title_color = theme.title_color.as_str();
    let stroke_width = theme.stroke_width.as_str();
    let error_bkg = theme.common.error_bkg.as_str();
    let error_text = theme.common.error_text.as_str();
    let edge_label_background = theme.edge_label_background.as_str();
    let tertiary = theme.tertiary.as_str();
    let cluster_bkg = theme.cluster_bkg.as_str();
    let cluster_border = theme.cluster_border.as_str();

    let label_bkg = css_rgba_fade(edge_label_background, 0.5)?;

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:{}px;fill:{};}}"#,
        id.as_str(),
        font_family,
        fmt(font_size),
        text_color
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id.as_str(),
        id.as_str()
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:{};}}#{} .error-text{{fill:{};stroke:{};}}"#,
        id.as_str(),
        error_bkg,
        id.as_str(),
        error_text,
        error_text
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:{}px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id.as_str(),
        stroke_width,
        id.as_str(),
        id.as_str(),
        id.as_str(),
        id.as_str(),
        id.as_str()
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:{};stroke:{};}}#{} .marker.cross{{stroke:{};}}"#,
        id.as_str(),
        stroke,
        stroke,
        id.as_str(),
        stroke
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:{}px;}}#{} p{{margin:0;}}#{} .label{{font-family:{};color:{};}}"#,
        id.as_str(),
        font_family,
        fmt(font_size),
        id.as_str(),
        id.as_str(),
        font_family,
        node_text_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .cluster-label text{{fill:{};}}#{} .cluster-label span{{color:{};}}#{} .cluster-label span p{{background-color:transparent;}}#{} .label text,#{} span{{fill:{};color:{};}}"#,
        id.as_str(),
        title_color,
        id.as_str(),
        title_color,
        id.as_str(),
        id.as_str(),
        id.as_str(),
        node_text_color,
        node_text_color
    );
    let _ = write!(
        &mut out,
        r#"#{id} .node rect,#{id} .node circle,#{id} .node ellipse,#{id} .node polygon,#{id} .node path{{fill:{main_bkg};stroke:{node_border};stroke-width:{stroke_width}px;}}#{id} .rough-node .label text,#{id} .node .label text,#{id} .image-shape .label,#{id} .icon-shape .label{{text-anchor:middle;}}#{id} .node .katex path{{fill:#000;stroke:#000;stroke-width:1px;}}#{id} .rough-node .label,#{id} .node .label,#{id} .image-shape .label,#{id} .icon-shape .label{{text-align:center;}}#{id} .node.clickable{{cursor:pointer;}}"#
    );
    let _ = write!(
        &mut out,
        r#"#{} .root .anchor path{{fill:{}!important;stroke-width:0;stroke:{};}}#{} .arrowheadPath{{fill:{};}}#{} .edgePath .path{{stroke:{};stroke-width:{}px;}}#{} .flowchart-link{{stroke:{};fill:none;}}"#,
        id.as_str(),
        stroke,
        stroke,
        id.as_str(),
        arrowhead_color,
        id.as_str(),
        stroke,
        stroke_width,
        id.as_str(),
        stroke
    );
    let _ = write!(
        &mut out,
        r#"#{} .edgeLabel{{background-color:{};text-align:center;}}#{} .edgeLabel p{{background-color:{};}}#{} .edgeLabel rect{{opacity:0.5;background-color:{};fill:{};}}#{} .labelBkg{{background-color:{};}}"#,
        id.as_str(),
        edge_label_background,
        id.as_str(),
        edge_label_background,
        id.as_str(),
        edge_label_background,
        edge_label_background,
        id.as_str(),
        label_bkg
    );
    let _ = write!(
        &mut out,
        r#"#{} .cluster rect{{fill:{};stroke:{};stroke-width:1px;}}#{} .cluster text{{fill:{};}}#{} .cluster span{{color:{};}}#{} div.mermaidTooltip{{position:absolute;text-align:center;max-width:200px;padding:2px;font-family:{};font-size:12px;background:{};border:1px solid {};border-radius:2px;pointer-events:none;z-index:100;}}#{} .flowchartTitleText{{text-anchor:middle;font-size:18px;fill:{};}}#{} rect.text{{fill:none;stroke-width:0;}}"#,
        escape_xml(diagram_id),
        cluster_bkg,
        cluster_border,
        escape_xml(diagram_id),
        title_color,
        escape_xml(diagram_id),
        title_color,
        escape_xml(diagram_id),
        font_family,
        tertiary,
        cluster_border,
        escape_xml(diagram_id),
        text_color,
        escape_xml(diagram_id)
    );
    let _ = write!(
        &mut out,
        r#"#{} .icon-shape,#{} .image-shape{{background-color:{};text-align:center;}}#{} .icon-shape p,#{} .image-shape p{{background-color:{};padding:2px;}}#{} .icon-shape .label rect,#{} .image-shape .label rect{{opacity:0.5;background-color:{};fill:{};}}#{} .label-icon{{display:inline-block;height:1em;overflow:visible;vertical-align:-0.125em;}}#{} .node .label-icon path{{fill:currentColor;stroke:revert;stroke-width:revert;}}#{} :root{{--mermaid-font-family:{};}}"#,
        id.as_str(),
        id.as_str(),
        edge_label_background,
        id.as_str(),
        id.as_str(),
        edge_label_background,
        id.as_str(),
        id.as_str(),
        edge_label_background,
        edge_label_background,
        id.as_str(),
        id.as_str(),
        id.as_str(),
        font_family
    );

    // Mermaid `createCssStyles(...)` chooses different selectors based on `htmlLabels`.
    // - HTML labels: `.classDef > *` + `.classDef span`
    // - SVG labels: `.classDef rect|polygon|ellipse|circle|path`
    let html_labels =
        crate::flowchart::FlowchartConfigView::new(effective_config).effective_html_labels();
    let shape_elements: &[&str] = &["rect", "polygon", "ellipse", "circle", "path"];

    for (class, decls) in class_defs {
        if decls.is_empty() {
            continue;
        }
        let mut style = String::new();
        let mut text_color: Option<String> = None;
        for d in decls {
            let Some((k, v)) = parse_style_decl(d) else {
                continue;
            };
            let _ = write!(&mut style, "{}:{}!important;", k, v);
            if k == "color" {
                text_color = Some(v.to_string());
            }
        }
        if style.is_empty() {
            continue;
        }
        if html_labels {
            // Mermaid (via Stylis) ends up serializing the `>` combinator inside `<style>` as
            // `&gt;` in the final SVG string (see upstream baselines).
            let _ = write!(
                &mut out,
                r#"#{} .{}&gt;*{{{}}}#{} .{} span{{{}}}"#,
                id.as_str(),
                escape_xml(class),
                style,
                id.as_str(),
                escape_xml(class),
                style
            );
        } else {
            for css_element in shape_elements {
                let _ = write!(
                    &mut out,
                    r#"#{} .{} {}{{{}}}"#,
                    id.as_str(),
                    escape_xml(class),
                    css_element,
                    style
                );
            }
        }
        if let Some(c) = text_color.as_deref() {
            let _ = write!(
                &mut out,
                r#"#{} .{} tspan{{fill:{}!important;}}"#,
                id.as_str(),
                escape_xml(class),
                escape_xml(c)
            );
        }
    }

    Ok(out)
}

#[inline]
pub(super) fn write_flowchart_edge_class_attr(out: &mut String, edge: &crate::flowchart::FlowEdge) {
    // Mermaid includes a 2-part class tuple (thickness/pattern) for flowchart edge paths. The
    // second tuple is `edge-thickness-normal edge-pattern-solid` in Mermaid@11.12.2 baselines,
    // even for dotted/thick strokes.
    let (thickness_1, pattern_1) = match edge.stroke.as_deref() {
        Some("thick") => ("edge-thickness-thick", "edge-pattern-solid"),
        Some("invisible") => ("edge-thickness-invisible", "edge-pattern-solid"),
        Some("dotted") => ("edge-thickness-normal", "edge-pattern-dotted"),
        _ => ("edge-thickness-normal", "edge-pattern-solid"),
    };

    if thickness_1 == "edge-thickness-invisible" {
        // Mermaid@11.12.2 does *not* include the second tuple nor `flowchart-link` for invisible
        // edges.
        out.push_str(thickness_1);
        out.push(' ');
        out.push_str(pattern_1);
        return;
    }

    out.push_str(thickness_1);
    out.push(' ');
    out.push_str(pattern_1);
    out.push_str(" edge-thickness-normal edge-pattern-solid flowchart-link");

    // Mermaid attaches animation classes directly on the edge path element when enabled via
    // edge-id `@{ ... }` blocks (e.g. `e1@{ animate: true }` or `e1@{ animation: fast }`).
    if edge.animate == Some(false) {
        return;
    }
    let animation_class = match edge.animation.as_deref() {
        Some("slow") => Some("edge-animation-slow"),
        Some(_) => Some("edge-animation-fast"),
        None => match edge.animate {
            Some(true) => Some("edge-animation-fast"),
            _ => None,
        },
    };
    if let Some(cls) = animation_class {
        out.push(' ');
        out.push_str(cls);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn khroma_named_edge_label_background_preserves_channels() {
        let css = flowchart_css(
            "theme_named_color",
            &json!({
                "themeVariables": {
                    "edgeLabelBackground": "rebeccapurple"
                }
            }),
            "\"trebuchet ms\",verdana,arial,sans-serif",
            16.0,
            &IndexMap::new(),
        )
        .expect("valid khroma color");

        assert!(
            css.contains("#theme_named_color .labelBkg{background-color:rgba(102, 51, 153, 0.5);}")
        );
    }

    #[test]
    fn unsupported_edge_label_background_returns_color_error() {
        let error = flowchart_css(
            "theme_unknown_color",
            &json!({
                "themeVariables": {
                    "edgeLabelBackground": "not-a-css-color"
                }
            }),
            "\"trebuchet ms\",verdana,arial,sans-serif",
            16.0,
            &IndexMap::new(),
        )
        .expect_err("unsupported khroma color must fail");

        assert!(error.to_string().contains("not-a-css-color"));
    }
}

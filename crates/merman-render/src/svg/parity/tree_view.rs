use super::*;
use merman_core::diagrams::tree_view::TreeViewDiagramRenderModel;

pub(super) fn render_tree_view_diagram_svg(
    layout: &TreeViewDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: TreeViewDiagramRenderModel = crate::json::from_value_ref(semantic)?;
    render_tree_view_diagram_svg_model(layout, &model, effective_config, options)
}

pub(super) fn render_tree_view_diagram_svg_model(
    layout: &TreeViewDiagramLayout,
    model: &TreeViewDiagramRenderModel,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("treeView");
    let diagram_id_esc = escape_xml(diagram_id);
    let acc_title = model
        .acc_title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let acc_descr = model
        .acc_descr
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty());
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id_esc}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id_esc}"));
    let min_x = -layout.line_thickness / 2.0;
    let viewbox_width = layout.total_width.max(1.0);
    let viewbox_height = layout.total_height.max(1.0);
    let viewbox_attr = format!(
        "{} 0 {} {}",
        fmt(min_x),
        fmt(viewbox_width),
        fmt(viewbox_height)
    );
    let max_width = fmt_string(viewbox_width);
    let style_attr = format!("max-width: {max_width}px; background-color: white;");
    let fixed_width = fmt_string(viewbox_width);
    let fixed_height = fmt_string(viewbox_height);

    let mut out = String::new();
    if layout.use_max_width {
        root_svg::push_svg_root_open(
            &mut out,
            root_svg::SvgRootAttrs {
                width: root_svg::SvgRootWidth::Percent100,
                style_attr: Some(style_attr.as_str()),
                viewbox_attr: Some(viewbox_attr.as_str()),
                aria_labelledby: aria_labelledby.as_deref(),
                aria_describedby: aria_describedby.as_deref(),
                trailing_newline: false,
                ..root_svg::SvgRootAttrs::new(diagram_id, "treeView")
            },
        );
    } else {
        root_svg::push_svg_root_open(
            &mut out,
            root_svg::SvgRootAttrs {
                width: root_svg::SvgRootWidth::Fixed(fixed_width.as_str()),
                height_attr: Some(fixed_height.as_str()),
                style_attr: Some("background-color: white;"),
                viewbox_attr: Some(viewbox_attr.as_str()),
                aria_labelledby: aria_labelledby.as_deref(),
                aria_describedby: aria_describedby.as_deref(),
                trailing_newline: false,
                ..root_svg::SvgRootAttrs::new(diagram_id, "treeView")
            },
        );
    }

    let css = tree_view_css(effective_config);
    if let Some(title) = acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{}">{}</title>"#,
            diagram_id_esc,
            escape_xml_display(title)
        );
    }
    if let Some(descr) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{}">{}</desc>"#,
            diagram_id_esc,
            escape_xml_display(descr)
        );
    }
    let _ = write!(&mut out, "<style>{css}</style>");
    out.push_str("<g/>");
    out.push_str(r#"<g class="tree-view">"#);
    for node in &layout.nodes {
        let _ = write!(
            &mut out,
            r#"<text dominant-baseline="middle" class="treeView-node-label" x="{}" y="{}">{}</text>"#,
            fmt(node.label_x),
            fmt(node.label_y),
            escape_xml(&node.name)
        );
    }
    for line in &layout.lines {
        let _ = write!(
            &mut out,
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke-width="{}" class="treeView-node-line"></line>"#,
            fmt(line.x1),
            fmt(line.y1),
            fmt(line.x2),
            fmt(line.y2),
            fmt(line.stroke_width)
        );
    }
    out.push_str("</g></svg>\n");
    Ok(out)
}

fn tree_view_css(effective_config: &serde_json::Value) -> String {
    let label_font_size = crate::config::config_css_number_or_string(
        effective_config,
        &["themeVariables", "treeView", "labelFontSize"],
    )
    .unwrap_or_else(|| "16px".to_string());
    let label_color = config_string(
        effective_config,
        &["themeVariables", "treeView", "labelColor"],
    )
    .unwrap_or_else(|| "black".to_string());
    let line_color = config_string(
        effective_config,
        &["themeVariables", "treeView", "lineColor"],
    )
    .unwrap_or_else(|| "black".to_string());

    format!(
        ".treeView-node-label {{ font-size: {label_font_size}; fill: {label_color}; }} .treeView-node-line {{ stroke: {line_color}; }}"
    )
}

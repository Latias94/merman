use super::super::*;
use crate::model::TreeViewNodeLayout;
use merman_core::diagrams::tree_view::TreeViewDiagramRenderModel;
use std::collections::BTreeSet;

const TREE_VIEW_ICON_PREFIX: &str = "mermaid-treeview";
const TREE_VIEW_DIRECTORY_NODE_TYPE: &str = "directory";

pub(crate) fn render_tree_view_diagram_svg(
    layout: &TreeViewDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: TreeViewDiagramRenderModel = crate::json::from_value_ref(semantic)?;
    render_tree_view_diagram_svg_model(layout, &model, effective_config, options)
}

pub(crate) fn render_tree_view_diagram_svg_model(
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
    let root_bounds = root_svg::DiagramBounds::from_view_box(
        -layout.line_thickness / 2.0,
        0.0,
        layout.total_width,
        layout.total_height,
    );
    let root_overrides = if options.apply_root_overrides {
        root_svg::resolve_root_overrides(None, None)
    } else {
        None
    };
    let viewport_plan = root_svg::build_root_viewport_plan(
        root_bounds,
        root_overrides.as_ref(),
        layout.use_max_width,
    );

    let mut out = String::new();
    root_svg::push_svg_root_open_with_viewport_plan(
        &mut out,
        root_svg::SvgRootAttrs {
            aria_labelledby: aria_labelledby.as_deref(),
            aria_describedby: aria_describedby.as_deref(),
            trailing_newline: false,
            ..root_svg::SvgRootAttrs::new(diagram_id, "treeView")
        },
        &viewport_plan,
    );

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
    push_tree_view_icon_defs(&mut out, layout, diagram_id);
    out.push_str("<g/>");
    out.push_str(r#"<g class="tree-view">"#);
    let mut next_node = 0usize;
    for line in &layout.lines {
        if line.kind == "horizontal"
            && let Some(node) = layout.nodes.get(next_node)
        {
            push_tree_view_node(&mut out, node, layout, diagram_id);
            next_node += 1;
        }
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
    for node in layout.nodes.iter().skip(next_node) {
        push_tree_view_node(&mut out, node, layout, diagram_id);
    }
    out.push_str("</g></svg>\n");
    Ok(out)
}

fn push_tree_view_node(
    out: &mut String,
    node: &TreeViewNodeLayout,
    layout: &TreeViewDiagramLayout,
    diagram_id: &str,
) {
    out.push_str("<g>");
    let label_classes = tree_view_label_classes(node);
    if node
        .css_class
        .as_deref()
        .is_some_and(|class| class.split_whitespace().any(|part| part == "highlight"))
    {
        let rect_width = (layout.total_width - node.x + 8.0).max(0.0);
        let _ = write!(
            out,
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3" class="treeView-highlight-bg"></rect>"#,
            fmt(node.x),
            fmt(node.y + 1.0),
            fmt(rect_width),
            fmt((node.height - 2.0).max(0.0))
        );
    }
    if let Some(icon) = &node.resolved_icon {
        let _ = write!(
            out,
            r##"<use xlink:href="#{}" x="{}" y="{}" class="treeView-node-icon"></use>"##,
            tree_view_icon_symbol_id(diagram_id, icon),
            fmt(node.x + layout.padding_x),
            fmt(node.y + layout.padding_y)
        );
    }
    let _ = write!(
        out,
        r#"<text dominant-baseline="middle" class="{}" x="{}" y="{}">{}</text>"#,
        escape_xml(&label_classes),
        fmt(node.label_x),
        fmt(node.label_y),
        escape_xml(&node.name)
    );
    if let (Some(description), Some(description_x)) =
        (node.description.as_deref(), node.description_x)
    {
        let _ = write!(
            out,
            r#"<text dominant-baseline="middle" class="treeView-node-description" x="{}" y="{}">{}</text>"#,
            fmt(description_x),
            fmt(node.label_y),
            escape_xml(description)
        );
    }
    out.push_str("</g>");
}

fn tree_view_css(effective_config: &serde_json::Value) -> String {
    let theme = PresentationTheme::new(effective_config).tree_view();

    format!(
        ".treeView-node-label {{ font-size: {}; fill: {}; white-space: pre; }} .treeView-node-dir {{ font-weight: bold; }} .treeView-node-line {{ stroke: {}; }} .treeView-node-icon {{ color: {}; }} .treeView-node-description {{ font-size: {}; fill: {}; font-style: italic; white-space: pre; }} .treeView-highlight-bg {{ fill: {}; stroke: {}; stroke-width: 1; }}",
        theme.label_font_size_css,
        theme.label_color,
        theme.line_color,
        theme.icon_color,
        theme.label_font_size_css,
        theme.description_color,
        theme.highlight_bg,
        theme.highlight_stroke
    )
}

fn tree_view_label_classes(node: &TreeViewNodeLayout) -> String {
    let mut classes = vec!["treeView-node-label".to_string()];
    if node.node_type == TREE_VIEW_DIRECTORY_NODE_TYPE {
        classes.push("treeView-node-dir".to_string());
    }
    if let Some(css_class) = node.css_class.as_deref() {
        classes.extend(
            css_class
                .split_whitespace()
                .filter(|class| !class.is_empty())
                .map(str::to_string),
        );
    }
    classes.join(" ")
}

fn push_tree_view_icon_defs(out: &mut String, layout: &TreeViewDiagramLayout, diagram_id: &str) {
    let used_icons = layout
        .nodes
        .iter()
        .filter_map(|node| node.resolved_icon.as_deref())
        .collect::<BTreeSet<_>>();
    if used_icons.is_empty() {
        return;
    }
    out.push_str("<defs>");
    for icon in used_icons {
        let _ = write!(
            out,
            r#"<g id="{}">{}</g>"#,
            tree_view_icon_symbol_id(diagram_id, icon),
            tree_view_icon_body(icon).unwrap_or("")
        );
    }
    out.push_str("</defs>");
}

fn tree_view_icon_symbol_id(diagram_id: &str, icon: &str) -> String {
    let mut id = format!("tv-icon-{diagram_id}-");
    for ch in icon.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            id.push(ch);
        } else {
            id.push('-');
        }
    }
    id
}

fn tree_view_icon_body(icon: &str) -> Option<&'static str> {
    match icon
        .strip_prefix(TREE_VIEW_ICON_PREFIX)?
        .strip_prefix(':')?
    {
        "folder" => Some(
            r#"<path fill="currentColor" d="M10.59 4.59A2 2 0 0 0 9.17 4H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.17z"/>"#,
        ),
        "file" => Some(
            r#"<path fill="currentColor" fill-rule="evenodd" d="M6 2a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8.83a2 2 0 0 0-.59-1.42l-4.82-4.82A2 2 0 0 0 13.17 2H6Zm7.5 1.9l4.6 4.6h-3.6a1 1 0 0 1-1-1V3.9Z" clip-rule="evenodd"/>"#,
        ),
        _ => None,
    }
}

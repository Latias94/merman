use super::super::*;
use crate::model::TreeViewNodeLayout;
use crate::svg::icon_registry::mermaid_unknown_icon_svg;
use crate::tree_view::{
    TREE_VIEW_DESCRIPTION_FONT_STYLE, TREE_VIEW_DIRECTORY_FONT_WEIGHT,
    TREE_VIEW_HIGHLIGHT_RECT_EXTENSION, TREE_VIEW_HIGHLIGHT_WIDTH_GROWTH, TREE_VIEW_ICON_SIZE,
    TreeViewThemePlan, TreeViewThemeReceipt, is_tree_view_highlight_class,
};
use merman_core::diagrams::tree_view::TreeViewDiagramRenderModel;

const TREE_VIEW_ICON_PREFIX: &str = "mermaid-treeview";
const TREE_VIEW_DIRECTORY_NODE_TYPE: &str = "directory";

struct TreeViewNodeRenderContext<'a, 'id> {
    layout: &'a TreeViewDiagramLayout,
    diagram_id: SvgDiagramId<'id>,
    icon_registry: Option<&'a crate::svg::IconRegistry>,
    effective_config: &'a merman_core::MermaidConfig,
    work_meter: &'a crate::resources::OperationWorkMeter,
}

pub(crate) fn render_tree_view_diagram_svg_model(
    layout: &TreeViewDiagramLayout,
    model: &TreeViewDiagramRenderModel,
    theme: &TreeViewThemePlan,
    effective_config: &merman_core::MermaidConfig,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let effective_config_value = effective_config.as_value();
    let diagram_id = options.diagram_id_or("treeView");
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
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));
    let root_bounds = if theme.additional_paint_outset_px() > 0.0 {
        let bounds = layout.bounds.as_ref().ok_or_else(|| Error::InvalidModel {
            message: "Tree View themed paint bounds are missing".to_string(),
        })?;
        root_svg::DiagramBounds::from_extents(
            bounds.min_x,
            bounds.min_y,
            bounds.max_x,
            bounds.max_y,
            0.0,
        )
    } else {
        root_svg::DiagramBounds::from_view_box(
            -layout.line_thickness / 2.0,
            0.0,
            layout.total_width,
            layout.total_height,
        )
    };
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, layout.use_max_width);

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "treeView");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.trailing_newline = false;
    let root_document =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::TREE_VIEW, diagram_id)
            .write_open(&mut out, root_spec, root_chrome)?;
    options.checkpoint_emit()?;

    if let Some(title) = acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{diagram_id}">{}</title>"#,
            escape_xml_display(title)
        );
        out.checkpoint()?;
    }
    if let Some(descr) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{diagram_id}">{}</desc>"#,
            escape_xml_display(descr)
        );
        out.checkpoint()?;
    }
    out.push_str("<style>");
    let icon_count = layout
        .nodes
        .iter()
        .filter(|node| node.resolved_icon.is_some())
        .count();
    // Icons are emitted inline for each resolved node, matching Mermaid's
    // node-local DOM and keeping custom icon rendering scoped to that node.
    let emitted_icon_count = icon_count;
    let mut tree_view_receipt =
        theme.begin_terminal_receipt(layout.nodes.len(), emitted_icon_count);
    push_tree_view_css(
        &mut out,
        effective_config_value,
        theme,
        &mut tree_view_receipt,
    )?;
    out.push_str("</style>");
    out.checkpoint()?;
    options.checkpoint_emit()?;
    out.push_str("<g/>");
    out.push_str(r#"<g class="tree-view">"#);
    out.checkpoint()?;
    let mut next_node = 0usize;
    let highlighted_node_count = layout
        .nodes
        .iter()
        .filter(|node| is_tree_view_highlight_class(node.css_class.as_deref()))
        .count();
    let mut width_before_highlight =
        layout.total_width - highlighted_node_count as f64 * TREE_VIEW_HIGHLIGHT_WIDTH_GROWTH;
    let node_context = TreeViewNodeRenderContext {
        layout,
        diagram_id,
        icon_registry: options.icon_registry(),
        effective_config,
        work_meter: options.work_meter(),
    };
    for (line_index, line) in layout.lines.iter().enumerate() {
        if line.kind == "horizontal"
            && let Some(node) = layout.nodes.get(next_node)
        {
            push_tree_view_node(
                &mut out,
                node,
                &node_context,
                &mut width_before_highlight,
                theme,
                &mut tree_view_receipt,
            )?;
            next_node += 1;
        }
        let emitted_stroke_width_token = theme.terminal_stroke_width_token(line.stroke_width);
        // Tree View maps both semantic edge paint facets to the line's CSS stroke channel.  This
        // renderer-owned marker lets the native raster proof bind that final line without
        // inferring semantics from the presentation class.
        let line_id = format!(
            "treeView-edge-{line_index}{}",
            crate::svg::RENDERER_SEMANTIC_FILL_AND_STROKE_PATH_SUFFIX
        );
        if let Some(stroke_width_token) = emitted_stroke_width_token {
            let _ = write!(
                &mut out,
                r#"<line id="{}" x1="{}" y1="{}" x2="{}" y2="{}" stroke-width="{}" class="treeView-node-line"></line>"#,
                line_id,
                fmt(line.x1),
                fmt(line.y1),
                fmt(line.x2),
                fmt(line.y2),
                stroke_width_token,
            );
        } else {
            let _ = write!(
                &mut out,
                r#"<line id="{}" x1="{}" y1="{}" x2="{}" y2="{}" stroke-width="{}" class="treeView-node-line"></line>"#,
                line_id,
                fmt(line.x1),
                fmt(line.y1),
                fmt(line.x2),
                fmt(line.y2),
                fmt(line.stroke_width),
            );
        }
        out.checkpoint()?;
        tree_view_receipt.record_line(theme.line_color_css(""), emitted_stroke_width_token);
    }
    for node in layout.nodes.iter().skip(next_node) {
        push_tree_view_node(
            &mut out,
            node,
            &node_context,
            &mut width_before_highlight,
            theme,
            &mut tree_view_receipt,
        )?;
    }
    out.push_str("</g></svg>\n");
    out.checkpoint()?;
    let rooted = root_document.complete(out.finish()?)?;
    let _ = theme.record_terminal(tree_view_receipt);
    Ok(rooted)
}

fn push_tree_view_node(
    out: &mut impl SvgOutput,
    node: &TreeViewNodeLayout,
    context: &TreeViewNodeRenderContext<'_, '_>,
    width_before_highlight: &mut f64,
    theme: &TreeViewThemePlan,
    tree_view_receipt: &mut TreeViewThemeReceipt,
) -> Result<()> {
    out.push_str("<g>");
    let label_classes = tree_view_label_classes(node);
    if is_tree_view_highlight_class(node.css_class.as_deref()) {
        let rect_width =
            (*width_before_highlight - node.x + TREE_VIEW_HIGHLIGHT_RECT_EXTENSION).max(0.0);
        let _ = write!(
            out,
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3" class="treeView-highlight-bg"></rect>"#,
            fmt(node.x),
            fmt(node.y + 1.0),
            fmt(rect_width),
            fmt((node.height - 2.0).max(0.0))
        );
        *width_before_highlight += TREE_VIEW_HIGHLIGHT_WIDTH_GROWTH;
    }
    if let Some(icon) = node.resolved_icon.as_deref() {
        let icon = tree_view_icon_svg(
            icon,
            context.diagram_id,
            node.id,
            context.icon_registry,
            context.effective_config,
            context.work_meter,
        )?;
        let native_paint_suffix =
            if icon.current_color_use == crate::svg::IconCurrentColorUse::Consumed {
                crate::svg::RENDERER_SEMANTIC_NATIVE_PAINT_SUFFIX
            } else {
                ""
            };
        let current_color_use = icon.current_color_use;
        let icon_svg = icon.svg;
        let _ = write!(
            out,
            r#"<g id="treeView-icon-{}{}" class="treeView-node-icon" transform="translate({}, {})">{}</g>"#,
            node.id,
            native_paint_suffix,
            fmt(node.x + context.layout.padding_x),
            fmt(node.y + context.layout.padding_y),
            icon_svg
        );
        tree_view_receipt.record_icon(theme.icon_color_css(""), current_color_use);
    }
    let _ = write!(
        out,
        r#"<text dominant-baseline="middle" class="{}" x="{}" y="{}">{}</text>"#,
        escape_xml_display(&label_classes),
        fmt(node.label_x),
        fmt(node.label_y),
        escape_xml_display(&node.name)
    );
    tree_view_receipt.record_label(theme.label_color_css(""));
    if let (Some(description), Some(description_x)) =
        (node.description.as_deref(), node.description_x)
    {
        let _ = write!(
            out,
            r#"<text dominant-baseline="middle" class="treeView-node-description" x="{}" y="{}">{}</text>"#,
            fmt(description_x),
            fmt(node.label_y),
            escape_xml_display(description)
        );
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn push_tree_view_css(
    out: &mut impl SvgOutput,
    effective_config: &serde_json::Value,
    theme_plan: &TreeViewThemePlan,
    receipt: &mut TreeViewThemeReceipt,
) -> Result<()> {
    let theme = MermaidThemeAdapter::new(effective_config).tree_view();
    let baseline_font_family = crate::config::config_font_family_css(effective_config);
    let font_family = theme_plan.font_family_css(&baseline_font_family);
    let label_color = theme_plan.label_color_css(&theme.label_color);
    let line_color = theme_plan.line_color_css(&theme.line_color);
    let icon_color = theme_plan.icon_color_css(&theme.icon_color);

    let _ = write!(
        out,
        ".treeView-node-label {{ font-family: {}; font-size: {}; fill: {}; white-space: pre; }} .treeView-node-dir {{ font-weight: {}; }} .treeView-node-line {{ stroke: {}; }} .treeView-node-icon {{ color: {}; }} .treeView-node-description {{ font-family: {}; font-size: {}; fill: {}; font-style: {}; white-space: pre; }} .treeView-highlight-bg {{ fill: {}; stroke: {}; stroke-width: 1; }}",
        font_family,
        theme.label_font_size_css,
        label_color,
        TREE_VIEW_DIRECTORY_FONT_WEIGHT,
        line_color,
        icon_color,
        font_family,
        theme.label_font_size_css,
        theme.description_color,
        TREE_VIEW_DESCRIPTION_FONT_STYLE,
        theme.highlight_bg,
        theme.highlight_stroke
    );
    receipt.record_css(font_family, label_color, line_color, icon_color);
    out.checkpoint()
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

struct TreeViewRenderedIcon {
    svg: String,
    current_color_use: crate::svg::IconCurrentColorUse,
}

fn tree_view_icon_svg(
    icon: &str,
    diagram_id: SvgDiagramId<'_>,
    node_id: i64,
    icon_registry: Option<&crate::svg::IconRegistry>,
    effective_config: &merman_core::MermaidConfig,
    work_meter: &crate::resources::OperationWorkMeter,
) -> Result<TreeViewRenderedIcon> {
    if let Some(body) = tree_view_icon_body(icon) {
        return Ok(TreeViewRenderedIcon {
            svg: format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 24 24">{body}</svg>"#,
                fmt(TREE_VIEW_ICON_SIZE),
                fmt(TREE_VIEW_ICON_SIZE)
            ),
            current_color_use: crate::svg::IconCurrentColorUse::Consumed,
        });
    }

    let node_id = node_id.to_string();
    let icon_svg = match icon_registry {
        Some(registry) => {
            let prefix = crate::svg::icon_registry::IconIdScopePrefix::from_parts(
                &["tree-view-", diagram_id.semantic_str(), "-"],
                work_meter,
            )?;
            let id_scope = prefix.scope_parts(&[&node_id], work_meter)?;
            registry.render_icon_with_paint_fact(crate::svg::icon_registry::IconRenderRequest {
                icon_name: icon,
                width_px: TREE_VIEW_ICON_SIZE,
                height_px: TREE_VIEW_ICON_SIZE,
                fallback_prefix: None,
                extra_class: None,
                id_scope,
                effective_config,
                work_meter,
            })?
        }
        None => None,
    }
    .map(|rendered| {
        let current_color_use = rendered.current_color_use();
        TreeViewRenderedIcon {
            current_color_use,
            svg: rendered.into_svg(),
        }
    })
    .unwrap_or_else(|| TreeViewRenderedIcon {
        svg: mermaid_unknown_icon_svg(fmt(TREE_VIEW_ICON_SIZE), fmt(TREE_VIEW_ICON_SIZE)),
        current_color_use: crate::svg::IconCurrentColorUse::NotConsumed,
    });
    Ok(icon_svg)
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

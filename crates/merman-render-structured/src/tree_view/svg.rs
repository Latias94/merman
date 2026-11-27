use std::fmt::Write as _;

use merman_model::{
    OperationPhase, families::tree_view::TreeViewDiagramModel, family::DiagramFamilyId,
    presentation::PresentationConfig,
};
use merman_render_core::{
    Error, Result,
    icon_registry::{
        IconIdScopePrefix, IconRegistry, IconRenderRequest, mermaid_unknown_icon_svg, render_icon,
    },
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    root_svg::{self, RootViewportContext, RootedSvg},
    svg_css::{CssSelectorDiagramId, SvgId},
    svg_output::{BoundedSvgOutput, SvgOutput, XmlTextOutput},
    xml::{escape_attr_display, escape_xml_display},
};

use super::{
    TREE_VIEW_DESCRIPTION_FONT_STYLE, TREE_VIEW_DIRECTORY_FONT_WEIGHT,
    TREE_VIEW_DIRECTORY_NODE_TYPE, TREE_VIEW_HIGHLIGHT_RECT_EXTENSION,
    TREE_VIEW_HIGHLIGHT_WIDTH_GROWTH, TREE_VIEW_ICON_PREFIX, TREE_VIEW_ICON_SIZE,
    TreeViewCssBinding, TreeViewDiagramLayout, TreeViewNodeLayout, is_tree_view_highlight_class,
};

/// Host-owned SVG identity, viewport, icon registry, and portability policy.
pub struct TreeViewSvgContext<'a> {
    pub diagram_id: SvgId<'a>,
    pub viewport: RootViewportContext<'a>,
    pub icon_registry: Option<&'a IconRegistry>,
    pub work_meter: &'a OperationWorkMeter,
    pub require_portable: bool,
}

struct TreeViewNodeRenderContext<'a, 'id> {
    layout: &'a TreeViewDiagramLayout,
    diagram_id: SvgId<'id>,
    icon_registry: Option<&'a IconRegistry>,
    effective_config: &'a PresentationConfig,
    work_meter: &'a OperationWorkMeter,
}

/// Emits a bounded Tree View document for the selected theme and root policy.
pub fn render_tree_view_diagram_svg_model(
    layout: &TreeViewDiagramLayout,
    model: &TreeViewDiagramModel,
    theme: &TreeViewCssBinding,
    effective_config: &PresentationConfig,
    context: &TreeViewSvgContext<'_>,
) -> Result<RootedSvg> {
    if !theme.label_font_size_verified && context.require_portable {
        return Err(Error::UnsupportedNativeThemeFeature {
            family_id: DiagramFamilyId::TREE_VIEW,
            message: "themeVariables.treeView.labelFontSize must resolve to a measurable size.",
        });
    }
    let diagram_id = context.diagram_id;
    let acc_title = model.acc_title.as_deref().filter(|t| !t.is_empty());
    let acc_descr = model.acc_descr.as_deref().filter(|d| !d.is_empty());
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));
    let root_bounds = if theme.includes_guide_outset() {
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

    let mut out = BoundedSvgOutput::new(context.work_meter);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "treeView");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.trailing_newline = false;
    let root_document = context
        .viewport
        .write_open(&mut out, root_spec, root_chrome)?;
    context.work_meter.checkpoint(OperationPhase::Emit)?;

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
    push_tree_view_css(
        &mut XmlTextOutput::new(&mut out),
        CssSelectorDiagramId(diagram_id),
        theme,
    )?;
    out.push_str("</style>");
    out.checkpoint()?;
    context.work_meter.checkpoint(OperationPhase::Emit)?;
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
        icon_registry: context.icon_registry,
        effective_config,
        work_meter: context.work_meter,
    };
    for line in &layout.lines {
        if line.kind == "horizontal"
            && let Some(node) = layout.nodes.get(next_node)
        {
            push_tree_view_node(
                &mut out,
                node,
                &node_context,
                &mut width_before_highlight,
                theme,
            )?;
            next_node += 1;
        }
        let _ = write!(
            &mut out,
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke-width="{}" class="treeView-node-line"></line>"#,
            fmt(line.x1),
            fmt(line.y1),
            fmt(line.x2),
            fmt(line.y2),
            fmt(line.stroke_width),
        );
        out.checkpoint()?;
    }
    for node in layout.nodes.iter().skip(next_node) {
        push_tree_view_node(
            &mut out,
            node,
            &node_context,
            &mut width_before_highlight,
            theme,
        )?;
    }
    out.push_str("</g></svg>\n");
    out.checkpoint()?;
    root_document.complete(out.finish()?)
}

fn push_tree_view_node(
    out: &mut impl SvgOutput,
    node: &TreeViewNodeLayout,
    context: &TreeViewNodeRenderContext<'_, '_>,
    width_before_highlight: &mut f64,
    theme: &TreeViewCssBinding,
) -> Result<()> {
    let highlighted = is_tree_view_highlight_class(node.css_class.as_deref());
    if theme.native.is_some() && highlighted {
        out.push_str(r#"<g class="treeView-highlighted">"#);
    } else {
        out.push_str("<g>");
    }
    let label_classes = tree_view_label_classes(node);
    if highlighted {
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
    } else if let Some(style) = &theme.native {
        let _ = write!(
            out,
            r#"<rect class="treeView-node-surface" x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
            fmt(node.x),
            fmt(node.y + 1.0),
            fmt(node.width),
            fmt((node.height - 2.0).max(0.0)),
            style.node_fill
        );
    }
    if let Some(icon) = node.resolved_icon.as_deref() {
        let icon_svg = tree_view_icon_svg(
            icon,
            context.diagram_id,
            node.id,
            context.icon_registry,
            context.effective_config,
            context.work_meter,
        )?;
        let _ = write!(
            out,
            r#"<g class="treeView-node-icon" transform="translate({}, {})">{}</g>"#,
            fmt(node.x + context.layout.padding_x),
            fmt(node.y + context.layout.padding_y),
            icon_svg
        );
    }
    let _ = write!(
        out,
        r#"<text dominant-baseline="middle" class="{}" x="{}" y="{}">{}</text>"#,
        escape_attr_display(&label_classes),
        fmt(node.label_x),
        fmt(node.label_y),
        escape_xml_display(&node.name)
    );
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
    diagram_id: impl std::fmt::Display + Copy,
    theme: &TreeViewCssBinding,
) -> Result<()> {
    let font_family = &theme.font_family;
    let label_color = &theme.label_color;
    let line_color = &theme.line_color;
    let icon_color = &theme.icon_color;
    let directory_weight = theme
        .native
        .as_ref()
        .map_or(TREE_VIEW_DIRECTORY_FONT_WEIGHT, |style| {
            style.directory_weight.as_str()
        });

    if let Some(root_font_size) = &theme.root_font_size_css {
        let _ = write!(out, "#{diagram_id} {{ font-size: {root_font_size}; }} ");
    }
    let _ = write!(
        out,
        "#{diagram_id} .treeView-node-label {{ font-family: {}; font-size: {}; fill: {}; white-space: pre; }} #{diagram_id} .treeView-node-dir {{ font-weight: {}; }} #{diagram_id} .treeView-node-line {{ stroke: {}; }} #{diagram_id} .treeView-node-icon {{ color: {}; }} #{diagram_id} .treeView-node-description {{ font-family: {}; font-size: {}; fill: {}; font-style: {}; white-space: pre; }} #{diagram_id} .treeView-highlight-bg {{ fill: {}; stroke: {}; stroke-width: 1; }}",
        font_family,
        theme.label_font_size_css,
        label_color,
        directory_weight,
        line_color,
        icon_color,
        font_family,
        theme.label_font_size_css,
        theme.description_color,
        TREE_VIEW_DESCRIPTION_FONT_STYLE,
        theme.highlight_bg,
        theme.highlight_stroke
    );
    if let Some(native) = &theme.native {
        let _ = write!(
            out,
            " #{diagram_id} .treeView-node-label:not(.treeView-node-dir),#{diagram_id} .treeView-node-description {{ font-weight: {}; }} #{diagram_id} .treeView-highlighted .treeView-node-label,#{diagram_id} .treeView-highlighted .treeView-node-description {{ fill: {}; }} #{diagram_id} .treeView-highlighted .treeView-node-icon {{ color: {}; }}",
            native.label_weight, native.on_highlight, native.on_highlight,
        );
    }
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

fn tree_view_icon_svg(
    icon: &str,
    diagram_id: SvgId<'_>,
    node_id: i64,
    icon_registry: Option<&IconRegistry>,
    effective_config: &PresentationConfig,
    work_meter: &OperationWorkMeter,
) -> Result<String> {
    if let Some(body) = tree_view_icon_body(icon) {
        return Ok(format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 24 24">{body}</svg>"#,
            fmt(TREE_VIEW_ICON_SIZE),
            fmt(TREE_VIEW_ICON_SIZE)
        ));
    }

    let node_id = node_id.to_string();
    let icon_svg = match icon_registry {
        Some(registry) => {
            let prefix = IconIdScopePrefix::from_parts(
                &["tree-view-", diagram_id.semantic_str(), "-"],
                work_meter,
            )?;
            let id_scope = prefix.scope_parts(&[&node_id], work_meter)?;
            render_icon(
                registry,
                IconRenderRequest {
                    icon_name: icon,
                    width_px: TREE_VIEW_ICON_SIZE,
                    height_px: TREE_VIEW_ICON_SIZE,
                    fallback_prefix: None,
                    id_scope,
                    effective_config,
                    work_meter,
                },
            )?
        }
        None => None,
    }
    .unwrap_or_else(|| mermaid_unknown_icon_svg(TREE_VIEW_ICON_SIZE, TREE_VIEW_ICON_SIZE));
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

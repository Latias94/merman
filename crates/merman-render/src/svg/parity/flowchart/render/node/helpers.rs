//! Node-level helpers (link sanitization, class building, placeholders).

use crate::svg::parity::flowchart::types::FlowchartRenderCtx;
use crate::svg::parity::util::escape_attr_display;
use crate::svg::parity::{escape_xml_display, escape_xml_into, fmt_display};
use std::fmt::Write as _;

pub(super) fn is_self_loop_label_node_id(id: &str) -> bool {
    let mut parts = id.split("---");
    let Some(a) = parts.next() else {
        return false;
    };
    let Some(b) = parts.next() else {
        return false;
    };
    let Some(n) = parts.next() else {
        return false;
    };
    parts.next().is_none() && a == b && (n == "1" || n == "2")
}

pub(super) fn try_render_self_loop_label_placeholder(
    out: &mut String,
    node_id: &str,
    x: f64,
    y: f64,
) -> bool {
    if !is_self_loop_label_node_id(node_id) {
        return false;
    }

    let _ = write!(
        out,
        r#"<g class="label edgeLabel" id="{}" transform="translate({}, {})"><rect width="0.1" height="0.1"/><g class="label" style="" transform="translate(0, 0)"><rect/><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 10px; text-align: center;"><span class="nodeLabel"></span></div></foreignObject></g></g>"#,
        escape_xml_display(node_id),
        fmt_display(x),
        fmt_display(y)
    );
    true
}

pub(super) fn href_is_safe_in_strict_mode(href: &str, config: &merman_core::MermaidConfig) -> bool {
    if config.get_str("securityLevel") == Some("loose") {
        return true;
    }

    let href = href.trim();
    if href.is_empty() {
        return false;
    }

    let lower = href.to_ascii_lowercase();
    if lower.starts_with('#')
        || lower.starts_with("mailto:")
        || lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("//")
        || lower.starts_with('/')
        || lower.starts_with("./")
        || lower.starts_with("../")
    {
        return true;
    }

    // In Mermaid's browser pipeline, the rendered SVG is further sanitized in strict mode.
    // This strips unknown deep-link schemes (e.g. `notes://...`) from `xlink:href`.
    !lower.contains("://")
}

pub(super) fn write_class_attr(out: &mut String, base: &str, classes: &[String]) {
    escape_xml_into(out, base);
    for c in classes {
        let t = c.trim();
        if t.is_empty() {
            continue;
        }
        out.push(' ');
        escape_xml_into(out, t);
    }
}

pub(super) fn open_node_wrapper(
    out: &mut String,
    node_id: &str,
    dom_idx: Option<usize>,
    class_attr_base: &str,
    node_classes: &[String],
    wrapped_in_a: bool,
    href: Option<&str>,
    x: f64,
    y: f64,
    tooltip_enabled: bool,
    tooltip: &str,
) {
    if wrapped_in_a {
        if let Some(href) = href {
            out.push_str(r#"<a xlink:href=""#);
            escape_xml_into(out, href);
            out.push_str(r#"" transform="translate("#);
            crate::svg::parity::util::fmt_into(out, x);
            out.push_str(", ");
            crate::svg::parity::util::fmt_into(out, y);
            out.push_str(r#")">"#);
        } else {
            out.push_str(r#"<a transform="translate("#);
            crate::svg::parity::util::fmt_into(out, x);
            out.push_str(", ");
            crate::svg::parity::util::fmt_into(out, y);
            out.push_str(r#")">"#);
        }
        out.push_str(r#"<g class=""#);
        write_class_attr(out, class_attr_base, node_classes);
        if let Some(dom_idx) = dom_idx {
            out.push_str(r#"" id="flowchart-"#);
            escape_xml_into(out, node_id);
            let _ = write!(out, "-{dom_idx}\"");
        } else {
            out.push_str(r#"" id=""#);
            escape_xml_into(out, node_id);
            out.push('"');
        }
    } else {
        out.push_str(r#"<g class=""#);
        write_class_attr(out, class_attr_base, node_classes);
        if let Some(dom_idx) = dom_idx {
            out.push_str(r#"" id="flowchart-"#);
            escape_xml_into(out, node_id);
            let _ = write!(out, r#"-{dom_idx}" transform="translate("#);
            crate::svg::parity::util::fmt_into(out, x);
            out.push_str(", ");
            crate::svg::parity::util::fmt_into(out, y);
            out.push_str(r#")""#);
        } else {
            out.push_str(r#"" id=""#);
            escape_xml_into(out, node_id);
            out.push_str(r#"" transform="translate("#);
            crate::svg::parity::util::fmt_into(out, x);
            out.push_str(", ");
            crate::svg::parity::util::fmt_into(out, y);
            out.push_str(r#")""#);
        }
    }
    if tooltip_enabled {
        let _ = write!(out, r#" title="{}""#, escape_attr_display(tooltip));
    }
    out.push('>');
}

pub(super) struct ResolvedNodeRenderInfo<'a> {
    pub(super) dom_idx: Option<usize>,
    pub(super) class_attr_base: &'static str,
    pub(super) wrapped_in_a: bool,
    pub(super) href: Option<&'a str>,
    pub(super) label_text: &'a str,
    pub(super) label_text_is_node_id: bool,
    pub(super) label_type: &'a str,
    pub(super) shape: &'a str,
    pub(super) node_icon: Option<&'a str>,
    pub(super) node_img: Option<&'a str>,
    pub(super) node_pos: Option<&'a str>,
    pub(super) node_constraint: Option<&'a str>,
    pub(super) node_asset_width: Option<f64>,
    pub(super) node_asset_height: Option<f64>,
    pub(super) node_styles: &'a [String],
    pub(super) node_classes: &'a [String],
}

pub(super) fn resolve_node_render_info<'a>(
    ctx: &'a FlowchartRenderCtx<'a>,
    node_id: &str,
) -> Option<ResolvedNodeRenderInfo<'a>> {
    if let Some(node) = ctx.nodes_by_id.get(node_id) {
        let dom_idx = Some(ctx.node_dom_index.get(node_id).copied().unwrap_or(0));
        let shape = node.layout_shape.as_deref().unwrap_or("squareRect");

        // Mermaid flowchart-v2 uses a distinct wrapper class for icon/image nodes.
        let class_attr_base = if shape == "imageSquare" {
            "image-shape default"
        } else if shape == "icon" || shape.starts_with("icon") {
            "icon-shape default"
        } else {
            "node default"
        };

        let link = node
            .link
            .as_deref()
            .map(|u| u.trim())
            .filter(|u| !u.is_empty());
        let link_present = link.is_some();
        // Mermaid sanitizes unsafe URLs (e.g. `javascript:` in strict mode) into
        // `about:blank`, but the resulting SVG `<a>` carries no `xlink:href` attribute.
        let href = link
            .filter(|u| *u != "about:blank")
            .filter(|u| href_is_safe_in_strict_mode(u, ctx.config));
        // Mermaid wraps nodes in `<a>` only when a link is present. Callback-based
        // interactions (`click A someFn`) still mark the node as clickable, but do not
        // emit an anchor element in the SVG.
        let wrapped_in_a = link_present;

        let (label_text, label_text_is_node_id) = if let Some(v) = node.label.as_deref() {
            (v, false)
        } else {
            ("", true)
        };

        Some(ResolvedNodeRenderInfo {
            dom_idx,
            class_attr_base,
            wrapped_in_a,
            href,
            label_text,
            label_text_is_node_id,
            label_type: node.label_type.as_deref().unwrap_or("text"),
            shape,
            node_icon: node.icon.as_deref(),
            node_img: node.img.as_deref(),
            node_pos: node.pos.as_deref(),
            node_constraint: node.constraint.as_deref(),
            node_asset_width: node.asset_width,
            node_asset_height: node.asset_height,
            node_styles: &node.styles,
            node_classes: &node.classes,
        })
    } else if let Some(sg) = ctx.subgraphs_by_id.get(node_id) {
        if !sg.nodes.is_empty() {
            return None;
        }
        let empty_styles: &'a [String] = &[];
        Some(ResolvedNodeRenderInfo {
            dom_idx: None,
            class_attr_base: "node",
            wrapped_in_a: false,
            href: None,
            label_text: sg.title.as_str(),
            label_text_is_node_id: false,
            label_type: sg.label_type.as_deref().unwrap_or("text"),
            shape: "squareRect",
            node_icon: None,
            node_img: None,
            node_pos: None,
            node_constraint: None,
            node_asset_width: None,
            node_asset_height: None,
            node_styles: empty_styles,
            node_classes: &sg.classes,
        })
    } else {
        None
    }
}

pub(in crate::svg::parity::flowchart::render::node) fn compute_node_label_metrics(
    ctx: &FlowchartRenderCtx<'_>,
    label_text: &str,
    label_type: &str,
    node_classes: &[String],
    node_styles: &[String],
) -> crate::text::TextMetrics {
    // Shared across many Flowchart v2 shape renderers.
    //
    // Keep behavior identical to the inlined implementations to preserve Mermaid SVG parity.
    let label_text_plain = crate::svg::parity::flowchart::flowchart_label_plain_text(
        label_text,
        label_type,
        ctx.node_html_labels,
    );
    let node_text_style = crate::flowchart::flowchart_effective_text_style_for_classes(
        &ctx.text_style,
        ctx.class_defs,
        node_classes,
        node_styles,
    );
    let mut metrics = crate::flowchart::flowchart_label_metrics_for_layout(
        ctx.measurer,
        label_text,
        label_type,
        &node_text_style,
        Some(ctx.wrapping_width),
        ctx.node_wrap_mode,
    );

    let span_css_height_parity = node_classes.iter().any(|c| {
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
            &node_text_style,
        );
    }

    let label_has_visual_content =
        super::super::super::util::flowchart_html_contains_img_tag(label_text)
            || (label_type == "markdown" && label_text.contains("!["));
    if label_text_plain.trim().is_empty() && !label_has_visual_content {
        metrics.width = 0.0;
        metrics.height = 0.0;
    }

    metrics
}

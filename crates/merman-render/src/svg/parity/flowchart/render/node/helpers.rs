//! Node-level helpers (link sanitization, class building, placeholders).

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

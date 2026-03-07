use super::super::*;

pub(super) fn render_class_html_label(
    out: &mut String,
    span_class: &str,
    text: &str,
    include_p: bool,
    extra_span_class: Option<&str>,
) {
    fn is_simple_plain_label(text: &str) -> bool {
        // Fast-path for the common case: no Markdown tokens and no hard/soft line breaks.
        // This avoids pulldown-cmark overhead while producing the same XHTML fragment Mermaid
        // would emit for plain text labels.
        if text.to_ascii_lowercase().contains("<br") {
            return false;
        }
        let bytes = text.as_bytes();
        !bytes.iter().any(|&b| {
            matches!(
                b,
                b'\n' | b'\r' | b'*' | b'_' | b'`' | b'~' | b'|' | b'[' | b']'
            )
        })
    }

    out.push_str(r#"<span class=""#);
    escape_xml_into(out, span_class);
    if let Some(extra) = extra_span_class.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        out.push(' ');
        escape_xml_into(out, extra);
    }
    out.push_str(r#"" style="">"#);

    if is_simple_plain_label(text) {
        if include_p {
            out.push_str("<p>");
            escape_xml_into(out, text);
            out.push_str("</p>");
        } else {
            escape_xml_into(out, text);
        }
        out.push_str("</span>");
        return;
    }

    let html = crate::text::mermaid_markdown_to_xhtml_label_fragment(text, true);
    if include_p {
        out.push_str(&html);
    } else {
        let inner = html
            .strip_prefix("<p>")
            .and_then(|s| s.strip_suffix("</p>"))
            .unwrap_or(html.as_str());
        out.push_str(inner);
    }
    out.push_str("</span>");
}

pub(super) fn class_apply_inline_styles(
    node: &super::ClassSvgNode,
) -> (Option<&str>, Option<&str>, Option<&str>, Option<&str>) {
    let mut fill: Option<&str> = None;
    let mut stroke: Option<&str> = None;
    let mut stroke_width: Option<&str> = None;
    let mut stroke_dasharray: Option<&str> = None;
    for raw in &node.styles {
        let Some((k, v)) = raw.split_once(':') else {
            continue;
        };
        let key = k.trim();
        let val = v.trim().trim_end_matches(';').trim();
        if key.eq_ignore_ascii_case("fill") && !val.is_empty() {
            fill = Some(val);
        }
        if key.eq_ignore_ascii_case("stroke") && !val.is_empty() {
            stroke = Some(val);
        }
        if key.eq_ignore_ascii_case("stroke-width") && !val.is_empty() {
            stroke_width = Some(val);
        }
        if key.eq_ignore_ascii_case("stroke-dasharray") && !val.is_empty() {
            stroke_dasharray = Some(val);
        }
    }
    (fill, stroke, stroke_width, stroke_dasharray)
}

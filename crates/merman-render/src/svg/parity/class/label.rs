use super::super::*;

pub(super) struct ClassInlineStyles<'a> {
    pub style_attr: String,
    pub fill: Option<&'a str>,
    pub stroke: Option<&'a str>,
    pub stroke_width: Option<&'a str>,
    pub stroke_dasharray: Option<&'a str>,
}

pub(super) fn render_class_html_label(
    out: &mut String,
    span_class: &str,
    text: &str,
    include_p: bool,
    extra_span_class: Option<&str>,
    span_style: Option<&str>,
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
    let span_style = span_style.map(str::trim).unwrap_or("");
    if span_class == "nodeLabel" || !span_style.is_empty() {
        out.push_str(r#"" style=""#);
        super::super::util::escape_attr_into(out, span_style);
        out.push_str(r#"">"#);
    } else {
        out.push_str(r#"">"#);
    }

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

pub(super) fn class_apply_inline_styles<'a>(
    node: &'a super::ClassSvgNode,
) -> ClassInlineStyles<'a> {
    let mut style_attr = String::new();
    let mut fill: Option<&str> = None;
    let mut stroke: Option<&str> = None;
    let mut stroke_width: Option<&str> = None;
    let mut stroke_dasharray: Option<&str> = None;

    for raw in &node.styles {
        let trimmed = raw.trim().trim_end_matches(';').trim();
        if trimmed.is_empty() {
            continue;
        }
        if !style_attr.is_empty() {
            style_attr.push(';');
        }
        style_attr.push_str(trimmed);

        let Some((k, v)) = trimmed.split_once(':') else {
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

    ClassInlineStyles {
        style_attr,
        fill,
        stroke,
        stroke_width,
        stroke_dasharray,
    }
}

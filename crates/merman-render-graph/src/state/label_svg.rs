//! Balanced State label fragments for the SVG document writer.

use std::fmt::Write as _;

use merman_model::families::state::StateDiagramRenderNode;
use merman_render_core::{
    entities::{decode_mermaid_entities_for_render_text, decode_svg_text_content_entities},
    mermaid_style::visit_parsed_style_declarations,
    number_format::canonical_number,
    text::inline_html::MermaidMarkdownWordType,
    xml::{escape_attr, escape_attr_display, escape_xml_display, normalize_html_entities_for_xml},
};

use super::label::{state_node_label_xhtml, state_svg_plain_lines, state_value_to_label_text};
use super::label_geometry::StateNativeLabelGeometry;

pub fn state_node_label_text(n: &StateDiagramRenderNode) -> String {
    n.label
        .as_ref()
        .map(state_value_to_label_text)
        .unwrap_or_else(|| n.id.clone())
}

/// Serializes a node label from the State parser's sanitized label source.
pub fn state_node_label_html_with_style(
    raw: &str,
    span_style: Option<&str>,
    font_size: f64,
) -> String {
    let style_attr = span_style
        .filter(|s| !s.is_empty())
        .map(|s| format!(r#" style="{}""#, escape_attr_display(s)))
        .unwrap_or_default();
    format!(
        r#"<span{} class="nodeLabel markdown-node-label">{}</span>"#,
        style_attr,
        state_node_label_xhtml(raw, font_size)
    )
}

fn state_escape_amp_preserving_entities(raw: &str) -> String {
    normalize_html_entities_for_xml(raw).into_owned()
}

fn state_normalize_br_tags(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut cur = 0usize;
    let mut i = 0usize;
    while i + 2 < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let b1 = bytes[i + 1];
        let b2 = bytes[i + 2];
        if !matches!(b1, b'b' | b'B') || !matches!(b2, b'r' | b'R') {
            i += 1;
            continue;
        }
        let next = bytes.get(i + 3).copied();
        if let Some(n) = next
            && !matches!(n, b'>' | b'/' | b' ' | b'\t' | b'\r' | b'\n')
        {
            i += 1;
            continue;
        }
        if i > cur {
            out.push_str(&raw[cur..i]);
        }
        let Some(end_rel) = bytes[i..].iter().position(|&c| c == b'>') else {
            cur = i;
            break;
        };
        out.push('\n');
        i = i + end_rel + 1;
        cur = i;
    }
    if cur < raw.len() {
        out.push_str(&raw[cur..]);
    }
    out
}

fn write_state_html_lines_with_br(out: &mut String, normalized: &str) {
    for (idx, line) in normalized.split('\n').enumerate() {
        if idx > 0 {
            out.push_str("<br />");
        }
        // State diagram labels are sanitized upstream (entities + limited tags). Preserve entities
        // like `&lt;` without double-escaping, while still making stray `&` XML-safe.
        out.push_str(&state_escape_amp_preserving_entities(line));
    }
}

fn state_html_with_br(raw: &str, wrap_paragraph: bool) -> String {
    let decoded = decode_mermaid_entities_for_render_text(raw);
    let normalized = state_normalize_br_tags(decoded.as_ref());
    let mut out = String::new();
    if wrap_paragraph {
        out.push_str("<p>");
    }
    write_state_html_lines_with_br(&mut out, &normalized);
    if wrap_paragraph {
        out.push_str("</p>");
    }
    out
}

fn html_paragraph_with_br(raw: &str) -> String {
    state_html_with_br(raw, true)
}

/// Serializes a plain label from the State parser's sanitized label source.
pub fn state_node_label_plain_html(raw: &str) -> String {
    format!(
        r#"<span class="nodeLabel">{}</span>"#,
        html_paragraph_with_br(raw)
    )
}

fn state_svg_text_style_attr(style_attr: &str) -> String {
    let mut out = String::with_capacity(style_attr.len());
    visit_parsed_style_declarations(style_attr, |declaration| {
        if !out.is_empty() {
            out.push(';');
        }
        let property = if declaration.property() == "color" {
            "fill"
        } else {
            declaration.property_css()
        };
        let _ = write!(&mut out, "{property}:{}", declaration.source_value());
    });
    out
}

pub fn state_svg_text_label(raw: &str, center_text: bool, style_attr: Option<&str>) -> String {
    let lines = state_svg_plain_lines(raw);
    state_svg_text_label_lines(lines.iter().map(String::as_str), center_text, style_attr)
}

pub fn state_measured_svg_text_label(
    geometry: &StateNativeLabelGeometry,
    center_text: bool,
    style_attr: Option<&str>,
) -> String {
    state_native_svg_text_label_lines(geometry, center_text, style_attr)
}

pub fn state_native_svg_text_label(
    raw: &str,
    measured_geometry: Option<&StateNativeLabelGeometry>,
    center_text: bool,
    style_attr: Option<&str>,
) -> String {
    if let Some(geometry) = measured_geometry {
        state_measured_svg_text_label(geometry, center_text, style_attr)
    } else {
        state_svg_text_label(raw, center_text, style_attr)
    }
}

fn state_native_svg_text_label_lines(
    geometry: &StateNativeLabelGeometry,
    center_text: bool,
    style_attr: Option<&str>,
) -> String {
    let text_anchor = if center_text {
        r#" text-anchor="middle""#
    } else {
        ""
    };
    let baseline_x_px = if center_text {
        0.0
    } else {
        geometry.baseline_x_px()
    };
    let svg_style_attr = style_attr
        .map(state_svg_text_style_attr)
        .filter(|style| !style.is_empty())
        .map(|style| format!(r#" style="{}""#, escape_attr(&style)))
        .unwrap_or_default();
    let mut out = format!(r#"<text{text_anchor}{svg_style_attr}>"#);
    for line_index in 0..geometry.line_count() {
        let baseline_y_px = geometry
            .baseline_y_px(line_index)
            .expect("State native geometry covers every emitted line");
        let _ = write!(
            &mut out,
            r#"<tspan class="text-outer-tspan row" x="{}" y="{}">"#,
            canonical_number(baseline_x_px),
            canonical_number(baseline_y_px),
        );
        if let Some(words) = geometry.measured_word_line(line_index) {
            for (index, (word, kind)) in words.iter().enumerate() {
                let font_style = if *kind == MermaidMarkdownWordType::Em {
                    "italic"
                } else {
                    "normal"
                };
                let font_weight = if *kind == MermaidMarkdownWordType::Strong {
                    "bold"
                } else {
                    "normal"
                };
                let word = decode_svg_text_content_entities(word);
                let _ = write!(
                    &mut out,
                    r#"<tspan font-style="{font_style}" class="text-inner-tspan" font-weight="{font_weight}"{svg_style_attr}>"#,
                );
                if index > 0 {
                    out.push(' ');
                }
                let _ = write!(&mut out, "{}</tspan>", escape_xml_display(&word));
            }
        } else {
            let line = geometry
                .measured_line(line_index)
                .expect("measured State label geometry retains emitted rows");
            let _ = write!(
                &mut out,
                r#"<tspan font-style="normal" class="text-inner-tspan" font-weight="normal"{}>{}</tspan>"#,
                svg_style_attr,
                escape_xml_display(line)
            );
        }
        out.push_str("</tspan>");
    }
    out.push_str("</text>");
    out
}

fn state_svg_text_label_lines<'a>(
    lines: impl IntoIterator<Item = &'a str>,
    center_text: bool,
    style_attr: Option<&str>,
) -> String {
    let text_anchor = if center_text {
        r#" text-anchor="middle""#
    } else {
        ""
    };
    let svg_style_attr = style_attr
        .map(state_svg_text_style_attr)
        .filter(|s| !s.is_empty())
        .map(|s| format!(r#" style="{}""#, escape_attr(&s)))
        .unwrap_or_default();

    let mut out = format!(r#"<text y="-10.1"{text_anchor}{svg_style_attr}>"#);
    for (idx, line) in lines.into_iter().enumerate() {
        let y = idx as f64 * 1.1 - 0.1;
        let _ = write!(
            &mut out,
            r#"<tspan class="text-outer-tspan row" x="0" y="{}em" dy="1.1em"><tspan font-style="normal" class="text-inner-tspan" font-weight="normal"{}>{}</tspan></tspan>"#,
            canonical_number(y),
            svg_style_attr,
            escape_xml_display(line)
        );
    }
    out.push_str("</text>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_svg_text_projects_color_and_emphasis_to_inner_tspan() {
        let svg = state_svg_text_label(
            "Styled",
            true,
            Some(
                "color:#123456 !important;font-weight:700 !important;font-style:italic !important",
            ),
        );

        assert!(svg.contains(
            r#"<text y="-10.1" text-anchor="middle" style="fill:#123456 !important;font-weight:700 !important;font-style:italic !important">"#
        ));
        assert!(svg.contains(
            r#"<tspan font-style="normal" class="text-inner-tspan" font-weight="normal" style="fill:#123456 !important;font-weight:700 !important;font-style:italic !important">Styled</tspan>"#
        ));
        assert!(!svg.contains("color:#123456"));
    }

    #[test]
    fn state_svg_text_applies_the_resolved_style_to_each_line() {
        let svg = state_svg_text_label(
            "first\nsecond",
            false,
            Some("color:#abcdef;font-weight:bold"),
        );
        assert_eq!(
            svg.matches("style=\"fill:#abcdef;font-weight:bold\"")
                .count(),
            3
        );
    }
}

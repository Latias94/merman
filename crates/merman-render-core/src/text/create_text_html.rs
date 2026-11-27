//! Mermaid createText HTML source projection shared by Flow and Class.

use crate::math::MathRenderer;
use crate::text::literal_label::flowchart_non_markdown_label_for_html;
use crate::text::{TextMeasurer, TextStyle, WrapMode, trim_html_collapsible_ascii_whitespace};
use merman_model::presentation::PresentationConfig;
use quick_xml::events::BytesStart;
use std::{borrow::Cow, fmt::Write as _};

pub struct FlowchartLabelMetricsRequest<'a> {
    pub measurer: &'a dyn TextMeasurer,
    pub raw_label: &'a str,
    pub label_type: &'a str,
    pub style: &'a TextStyle,
    pub max_width_px: Option<f64>,
    pub wrap_mode: WrapMode,
    pub config: &'a PresentationConfig,
    pub math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
}

#[inline]
pub fn starts_with_ascii_case_insensitive(input: &str, prefix: &[u8]) -> bool {
    input
        .as_bytes()
        .get(..prefix.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
}

#[inline]
fn find_ascii_case_insensitive(input: &str, needle: &[u8]) -> Option<usize> {
    debug_assert!(!needle.is_empty());
    input
        .as_bytes()
        .windows(needle.len())
        .position(|candidate| candidate.eq_ignore_ascii_case(needle))
}

fn starts_with_img_tag(input: &str) -> bool {
    starts_with_ascii_case_insensitive(input, b"<img")
        && input
            .as_bytes()
            .get(4)
            .is_some_and(|byte| byte.is_ascii_whitespace() || matches!(*byte, b'/' | b'>'))
}

pub fn normalize_flowchart_img_tags(input: &str, fixed_width: bool) -> Cow<'_, str> {
    // Mermaid flowchart-v2 adds inline styles to `<img>` tags inside HTML labels to constrain
    // their layout. The SVG baseline uses XHTML, so we also self-close the tags later.
    if find_ascii_case_insensitive(input, b"<img").is_none() {
        return Cow::Borrowed(input);
    }

    let style = if fixed_width {
        "display: flex; flex-direction: column; min-width: 80px; max-width: 80px;"
    } else {
        "display: flex; flex-direction: column; width: 100%;"
    };

    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(offset) = input[cursor..].find('<') {
        let start = cursor + offset;
        out.push_str(&input[cursor..start]);
        let rest = &input[start..];
        if !rest.as_bytes().get(1).is_some_and(u8::is_ascii_alphabetic) {
            out.push('<');
            cursor = start + 1;
            continue;
        }
        let Some(end) = crate::svg_scan::find_tag_end(input, start) else {
            cursor = start;
            break;
        };
        cursor = end + 1;
        if !starts_with_img_tag(rest) {
            out.push_str(&input[start..cursor]);
            continue;
        }

        let tag = BytesStart::from_content(&input[start + 1..end], 3);
        let mut attrs = [None, None, None];
        let names = ["src", "width", "height"];
        for attr in tag.html_attributes().with_checks(false).flatten() {
            if let Some(index) = names
                .iter()
                .position(|name| attr.key.as_ref().eq_ignore_ascii_case(name))
                && attrs[index].is_none()
            {
                attrs[index] = Some(
                    merman_model::text::decode_html_entities_to_unicode(&attr.value).into_owned(),
                );
            }
        }
        out.push_str("<img");
        for (name, value) in names.into_iter().zip(attrs) {
            if let Some(value) = value.filter(|value| name != "src" || !value.trim().is_empty()) {
                let _ = write!(out, r#" {name}="{}""#, crate::xml::escape_attr(&value));
            }
        }
        let _ = write!(out, r#" style="{style}">"#);
    }
    out.push_str(&input[cursor..]);
    Cow::Owned(out)
}

pub fn is_single_img_label(label: &str) -> bool {
    let label = trim_html_collapsible_ascii_whitespace(label);
    if !starts_with_img_tag(label) {
        return false;
    }
    let Some(end) = crate::svg_scan::find_tag_end(label, 0) else {
        return false;
    };
    trim_html_collapsible_ascii_whitespace(&label[end + 1..]).is_empty()
}

pub fn flowchart_label_html_source(
    label: &str,
    label_type: &str,
    config: &merman_model::presentation::PresentationConfig,
) -> String {
    if trim_html_collapsible_ascii_whitespace(label).is_empty() {
        return String::new();
    }

    match label_type {
        "markdown" => {
            let decoded = crate::entities::decode_mermaid_entities_for_render_text(label);
            let decoded = if decoded.contains("\\\\") {
                Cow::Owned(decoded.replace("\\\\", "\\"))
            } else {
                decoded
            };
            let markdown_auto_wrap = config
                .as_value()
                .get("markdownAutoWrap")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            let html_out = crate::text::mermaid_markdown_to_html_label_fragment(
                decoded.trim_end_matches('\n'),
                markdown_auto_wrap,
            );
            crate::text::replace_fontawesome_icons(trim_html_collapsible_ascii_whitespace(
                &html_out,
            ))
        }
        _ => {
            let label = flowchart_non_markdown_label_for_html(label, label_type);

            // Mermaid's nonMarkdownToHTML() wraps every non-empty label in one paragraph.
            // Markdown block classification must not leak into this branch.
            let fixed_img_width = is_single_img_label(&label);
            let label = normalize_flowchart_img_tags(&label, fixed_img_width);
            let wrapped = format!("<p>{}</p>", label);
            if wrapped.contains(":fa-") {
                crate::text::replace_fontawesome_icons(&wrapped)
            } else {
                wrapped
            }
        }
    }
}

pub fn flowchart_label_sanitize_config(
    config: &PresentationConfig,
) -> std::borrow::Cow<'_, merman_model::MermaidConfig> {
    if config.get_str("securityLevel").is_some() {
        return std::borrow::Cow::Borrowed(config.as_config());
    }
    let mut config = config.as_config().clone();
    config.set_value("securityLevel", serde_json::Value::String("strict".into()));
    std::borrow::Cow::Owned(config)
}

//! HTML and SVG writers for prepared Flow label occurrences.

use std::{borrow::Cow, fmt::Write as _};

use merman_model::presentation::PresentationConfig;
use merman_render_core::{
    math::{MathRenderer, render_math_html_label},
    mermaid_style::{
        is_static_css_font_family_list, parse_style_declaration, visit_style_declaration_boundaries,
    },
    svg_output::SvgOutput,
    text::{TextMeasurer, TextStyle, create_text_html::flowchart_label_html_source, svg_label},
    xml::escape_attr_display,
};

use super::{FlowchartPreparedLabelResolution, FlowchartSvgLabelRenderPlan};

pub struct OptionalStyleAttr<'a>(pub &'a str);

impl std::fmt::Display for OptionalStyleAttr<'_> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.trim().is_empty() {
            return Ok(());
        }
        write!(f, r#" style="{}""#, escape_attr_display(self.0))
    }
}

pub(crate) fn with_svg_font_fallback<'a>(style: &'a str, fallback: Option<&str>) -> Cow<'a, str> {
    let Some(fallback) = fallback.filter(|_| {
        style
            .as_bytes()
            .windows(b"font-family".len())
            .any(|name| name.eq_ignore_ascii_case(b"font-family"))
    }) else {
        return Cow::Borrowed(style);
    };
    let fallback = if is_static_css_font_family_list(fallback) {
        fallback
    } else {
        "sans-serif"
    };
    let mut rewritten = String::new();
    let mut changed = false;
    visit_style_declaration_boundaries(style, |boundary| {
        let raw = boundary.raw();
        if !rewritten.is_empty() {
            rewritten.push(';');
        }
        if let Some(decl) = parse_style_declaration(raw)
            && decl.property() == "font-family"
            && is_static_css_font_family_list(decl.value())
        {
            let _ = write!(
                rewritten,
                "{}:{}, {}",
                decl.property_css(),
                decl.value(),
                fallback,
            );
            if !fallback.ends_with("sans-serif") {
                rewritten.push_str(", sans-serif");
            }
            if decl.important() {
                rewritten.push_str(" !important");
            }
            changed = true;
        } else {
            rewritten.push_str(raw);
        }
    });
    if changed {
        Cow::Owned(rewritten)
    } else {
        Cow::Borrowed(style)
    }
}

pub fn append_native_label_color(style: &mut String, fill: &str) {
    if !style.is_empty() {
        style.push(';');
    }
    let _ = write!(
        style,
        "fill:{fill} !important;color:{} !important",
        if fill == "none" { "transparent" } else { fill }
    );
}

pub fn flowchart_label_html_with_prepared_label<'a>(
    label: &'a str,
    label_type: &str,
    config: &PresentationConfig,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    prepared_label: FlowchartPreparedLabelResolution<'a>,
) -> Cow<'a, str> {
    match prepared_label {
        FlowchartPreparedLabelResolution::PreparedMath(prepared) => {
            Cow::Borrowed(prepared.browser_xhtml())
        }
        FlowchartPreparedLabelResolution::PreparedHtml(prepared) => Cow::Borrowed(prepared.xhtml()),
        FlowchartPreparedLabelResolution::Unavailable(_reason) => {
            Cow::Owned(flowchart_label_html_impl(label, label_type, config, None))
        }
        FlowchartPreparedLabelResolution::NotPrepared => Cow::Owned(flowchart_label_html_impl(
            label,
            label_type,
            config,
            math_renderer,
        )),
    }
}

fn flowchart_label_html_impl(
    label: &str,
    label_type: &str,
    config: &PresentationConfig,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> String {
    if let Some(html) = render_math_html_label(label, config, math_renderer) {
        return html;
    }
    let html = flowchart_label_html_source(label, label_type, config);
    merman_markup::sanitize::sanitize_xhtml(&html, config.as_config())
}

pub fn write_flowchart_svg_source_word_lines(
    out: &mut impl SvgOutput,
    lines: &[Vec<String>],
    include_style: bool,
) {
    svg_label::write_svg_text_source_word_lines(out, lines, include_style, false);
}

pub fn write_flowchart_svg_label_plan(
    out: &mut impl SvgOutput,
    plan: &FlowchartSvgLabelRenderPlan<'_>,
    include_style: bool,
) {
    write_flowchart_svg_label_plan_with_style(out, plan, include_style, None);
}

pub fn write_flowchart_svg_label_plan_with_style(
    out: &mut impl SvgOutput,
    plan: &FlowchartSvgLabelRenderPlan<'_>,
    include_style: bool,
    extra_style: Option<&str>,
) {
    let lines = plan.wrapped_lines();
    if let Some(extra_style) = extra_style.filter(|style| !style.is_empty()) {
        svg_label::write_svg_text_source_word_lines_with_style(
            out,
            &lines,
            extra_style,
            false,
            None,
        );
    } else {
        write_flowchart_svg_source_word_lines(out, &lines, include_style);
    }
}

pub fn write_flowchart_svg_text_markdown(
    out: &mut impl SvgOutput,
    markdown: &str,
    include_style: bool,
) {
    svg_label::write_svg_text_markdown_from_create_text_source(out, markdown, include_style);
}

pub fn write_flowchart_svg_text_markdown_wrapped(
    out: &mut impl SvgOutput,
    markdown: &str,
    include_style: bool,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_width_px: Option<f64>,
) {
    svg_label::write_svg_text_markdown_wrapped_from_create_text_source(
        out,
        markdown,
        include_style,
        measurer,
        style,
        max_width_px,
    );
}

pub fn write_flowchart_svg_text_markdown_wrapped_with_style(
    out: &mut impl SvgOutput,
    markdown: &str,
    style: &str,
    measurer: &dyn TextMeasurer,
    text_style: &TextStyle,
    max_width_px: Option<f64>,
) {
    svg_label::write_svg_text_markdown_wrapped_from_create_text_source_with_style(
        out,
        markdown,
        style,
        measurer,
        text_style,
        max_width_px,
        false,
    );
}

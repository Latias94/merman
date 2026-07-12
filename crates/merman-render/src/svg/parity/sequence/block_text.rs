use super::super::*;
use crate::sequence::{bracketize_sequence_block_label, sequence_text_line_step_px};

pub(super) struct LoopTextRenderContext<'a> {
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) style: &'a TextStyle,
}

impl<'a> LoopTextRenderContext<'a> {
    pub(super) fn new(measurer: &'a dyn TextMeasurer, style: &'a TextStyle) -> Self {
        Self { measurer, style }
    }
}

pub(super) fn display_block_label(raw_label: &str, always_show: bool) -> Option<String> {
    let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(raw_label);
    let t = decoded.as_ref().trim();
    if t.is_empty() {
        if always_show {
            // Mermaid renders empty block labels as a zero-width space inside `<tspan>`.
            Some("\u{200B}".to_string())
        } else {
            None
        }
    } else {
        Some(bracketize_sequence_block_label(t))
    }
}

pub(super) fn wrap_svg_text_lines(
    text: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_width: Option<f64>,
) -> Vec<String> {
    let lines = max_width.map_or_else(
        || {
            crate::text::split_html_br_lines(text)
                .into_iter()
                .map(str::to_string)
                .collect()
        },
        |width| crate::text::wrap_label_like_mermaid_lines(text, measurer, style, width),
    );
    if lines.is_empty() {
        vec!["".to_string()]
    } else {
        lines
    }
}

pub(super) fn write_loop_text_lines(
    out: &mut String,
    ctx: &LoopTextRenderContext<'_>,
    x: f64,
    y0: f64,
    max_width: Option<f64>,
    text: &str,
    use_tspan: bool,
) {
    let line_step = sequence_text_line_step_px(ctx.style.font_size);
    let lines = wrap_svg_text_lines(text, ctx.measurer, ctx.style, max_width);
    for (i, line) in lines.into_iter().enumerate() {
        let y = y0 + (i as f64) * line_step;
        if use_tspan {
            let _ = write!(
                out,
                r#"<text x="{x}" y="{y}" text-anchor="middle" class="loopText" style="font-size: {fs}px; font-weight: 400;"><tspan x="{x}">{text}</tspan></text>"#,
                x = fmt(x),
                y = fmt(y),
                fs = fmt(ctx.style.font_size),
                text = escape_xml(&line)
            );
        } else {
            let _ = write!(
                out,
                r#"<text x="{x}" y="{y}" text-anchor="middle" class="loopText" style="font-size: {fs}px; font-weight: 400;">{text}</text>"#,
                x = fmt(x),
                y = fmt(y),
                fs = fmt(ctx.style.font_size),
                text = escape_xml(&line)
            );
        }
    }
}

pub(super) fn write_section_title_lines(
    out: &mut String,
    ctx: &LoopTextRenderContext<'_>,
    x: f64,
    y0: f64,
    max_width: Option<f64>,
    text: &str,
) {
    let line_step = sequence_text_line_step_px(ctx.style.font_size);
    let lines = wrap_svg_text_lines(text, ctx.measurer, ctx.style, max_width);
    for (i, line) in lines.into_iter().enumerate() {
        let y = y0 + (i as f64) * line_step;
        let _ = write!(
            out,
            r#"<text x="{x}" y="{y}" text-anchor="middle" class="sectionTitle" style="font-size: {fs}px; font-weight: 400;">{text}</text>"#,
            x = fmt(x),
            y = fmt(y),
            fs = fmt(ctx.style.font_size),
            text = escape_xml(&line)
        );
    }
}

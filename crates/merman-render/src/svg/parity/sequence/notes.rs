use super::super::*;
use super::SequenceEmitCheckpoints;
use super::geometry::node_left_top;
use super::math_label::{sequence_katex_label, write_sequence_katex_foreign_object};
use crate::sequence::{
    SequenceMathHeightMode, sequence_note_final_wrapped_lines, sequence_text_line_step_px,
};
use merman_core::diagrams::sequence::{SequenceMessage, SequenceMessageKind};
use rustc_hash::FxHashMap;

pub(super) struct SequenceNoteRenderContext<'a> {
    pub(super) nodes_by_id: &'a FxHashMap<&'a str, &'a LayoutNode>,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) actor_label_font_size: f64,
    pub(super) wrap_padding: f64,
    pub(super) note_text_style: &'a TextStyle,
    pub(super) sanitize_config: &'a merman_core::MermaidConfig,
    pub(super) math_renderer: Option<&'a (dyn crate::math::MathRenderer + Send + Sync)>,
    pub(super) checkpoints: SequenceEmitCheckpoints<'a>,
}

pub(super) fn render_sequence_note(
    out: &mut String,
    msg: &SequenceMessage,
    ctx: &SequenceNoteRenderContext<'_>,
) -> Result<()> {
    if msg.semantic_kind() != SequenceMessageKind::Note {
        return Ok(());
    }
    ctx.checkpoints.checkpoint()?;

    let id = &msg.id;
    let raw = msg.message_text();
    let node_id = format!("note-{id}");
    let Some(n) = ctx.nodes_by_id.get(node_id.as_str()).copied() else {
        return Ok(());
    };
    let (x, y) = node_left_top(n);
    let cx = x + (n.width / 2.0);
    let text_y = y + 5.0;
    let line_step = sequence_text_line_step_px(ctx.actor_label_font_size);
    let _ = write!(out, r#"<g data-et="note" data-id="i{}">"#, escape_attr(id));
    let _ = write!(
        &mut *out,
        r##"<rect x="{x}" y="{y}" fill="#EDF2AE" stroke="#666" width="{w}" height="{h}" class="note"/>"##,
        x = fmt(x),
        y = fmt(y),
        w = fmt(n.width),
        h = fmt(n.height)
    );
    if let Some(katex) = sequence_katex_label(
        raw,
        ctx.measurer,
        ctx.note_text_style,
        ctx.sanitize_config,
        ctx.math_renderer,
        SequenceMathHeightMode::Draw,
    ) {
        write_sequence_katex_foreign_object(
            out,
            &katex,
            (x + n.width / 2.0 - katex.width / 2.0).round(),
            (y + n.height / 2.0 - katex.height / 2.0).round(),
        );
    } else if msg.wrap {
        // Mermaid@11.12.2 (Sequence) wraps notes *after* placement width is known:
        //   noteModel.message = wrapLabel(msg.message, noteModel.width - 2*wrapPadding, noteFont)
        //
        // Layout already computed the note box width (`n.width`) to match Mermaid's
        // `noteModel.width`, so wrap to `n.width - 2*wrapPadding` here.
        let lines = sequence_note_final_wrapped_lines(
            raw,
            n.width,
            2.0 * ctx.wrap_padding,
            ctx.measurer,
            ctx.note_text_style,
        );
        render_sequence_note_lines(
            out,
            lines.iter().map(String::as_str),
            cx,
            text_y,
            line_step,
            ctx.actor_label_font_size,
            ctx.checkpoints,
        )?;
    } else {
        render_sequence_note_lines(
            out,
            crate::text::split_html_br_lines(raw),
            cx,
            text_y,
            line_step,
            ctx.actor_label_font_size,
            ctx.checkpoints,
        )?;
    }
    out.push_str("</g>");
    ctx.checkpoints.checkpoint()
}

fn render_sequence_note_lines<'a>(
    out: &mut String,
    lines: impl IntoIterator<Item = &'a str>,
    cx: f64,
    text_y: f64,
    line_step: f64,
    actor_label_font_size: f64,
    checkpoints: SequenceEmitCheckpoints<'_>,
) -> Result<()> {
    for (i, line) in lines.into_iter().enumerate() {
        checkpoints.checkpoint_loop(i)?;
        let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(line);
        let text = if decoded.as_ref().is_empty() {
            "\u{200B}"
        } else {
            decoded.as_ref()
        };
        let y = text_y + (i as f64) * line_step;
        let _ = write!(
            &mut *out,
            r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="noteText" dy="1em" style="font-size: {fs}px; font-weight: 400;"><tspan x="{x}">{text}</tspan></text>"#,
            x = fmt(cx),
            y = fmt(y),
            fs = fmt(actor_label_font_size),
            text = escape_xml(text)
        );
    }
    checkpoints.checkpoint()
}

#[cfg(test)]
mod tests {
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};

    #[test]
    fn empty_note_rows_render_the_upstream_zero_width_space() {
        let mut out = String::new();
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        super::render_sequence_note_lines(
            &mut out,
            ["first", "", "last"],
            50.0,
            10.0,
            19.0,
            16.0,
            super::SequenceEmitCheckpoints::new(&meter),
        )
        .unwrap();

        assert!(out.contains("<tspan x=\"50\">\u{200b}</tspan>"), "{out}");
    }
}

use super::super::*;
use super::geometry::node_left_top;
use crate::sequence::{SEQUENCE_NOTE_WRAP_SLACK_PX, sequence_text_line_step_px};
use merman_core::diagrams::sequence::SequenceMessage;
use rustc_hash::FxHashMap;

pub(super) fn render_sequence_note(
    out: &mut String,
    msg: &SequenceMessage,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    measurer: &dyn TextMeasurer,
    actor_label_font_size: f64,
    wrap_padding: f64,
    note_text_style: &TextStyle,
) {
    if msg.message_type != 2 {
        return;
    }

    let id = &msg.id;
    let raw = msg.message_text();
    let node_id = format!("note-{id}");
    let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
        return;
    };
    let (x, y) = node_left_top(n);
    let cx = x + (n.width / 2.0);
    let text_y = y + 5.0;
    let line_step = sequence_text_line_step_px(actor_label_font_size);
    out.push_str(r#"<g>"#);
    let _ = write!(
        &mut *out,
        r##"<rect x="{x}" y="{y}" fill="#EDF2AE" stroke="#666" width="{w}" height="{h}" class="note"/>"##,
        x = fmt(x),
        y = fmt(y),
        w = fmt(n.width),
        h = fmt(n.height)
    );
    if msg.wrap {
        // Mermaid@11.12.2 (Sequence) wraps notes *after* placement width is known:
        //   noteModel.message = wrapLabel(msg.message, noteModel.width - 2*wrapPadding, noteFont)
        //
        // Layout already computed the note box width (`n.width`) to match Mermaid's
        // `noteModel.width`, so wrap to `n.width - 2*wrapPadding` here.
        let wrap_w = (n.width - 2.0 * wrap_padding + SEQUENCE_NOTE_WRAP_SLACK_PX).max(1.0);
        let lines = crate::text::wrap_label_like_mermaid_lines_floored_bbox(
            raw,
            measurer,
            note_text_style,
            wrap_w,
        );
        render_sequence_note_lines(
            out,
            lines.iter().map(String::as_str),
            cx,
            text_y,
            line_step,
            actor_label_font_size,
        );
    } else {
        render_sequence_note_lines(
            out,
            crate::text::split_html_br_lines(raw),
            cx,
            text_y,
            line_step,
            actor_label_font_size,
        );
    }
    out.push_str("</g>");
}

fn render_sequence_note_lines<'a>(
    out: &mut String,
    lines: impl IntoIterator<Item = &'a str>,
    cx: f64,
    text_y: f64,
    line_step: f64,
    actor_label_font_size: f64,
) {
    for (i, line) in lines.into_iter().enumerate() {
        let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(line);
        let y = text_y + (i as f64) * line_step;
        let _ = write!(
            &mut *out,
            r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="noteText" dy="1em" style="font-size: {fs}px; font-weight: 400;"><tspan x="{x}">{text}</tspan></text>"#,
            x = fmt(cx),
            y = fmt(y),
            fs = fmt(actor_label_font_size),
            text = escape_xml(decoded.as_ref())
        );
    }
}

use super::super::*;
use super::geometry::node_left_top;
use super::model::SequenceSvgModel;
use crate::generated::sequence_text_overrides_11_12_2 as sequence_text_overrides;
use rustc_hash::FxHashMap;

pub(super) fn render_sequence_notes(
    out: &mut String,
    model: &SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    measurer: &dyn TextMeasurer,
    actor_label_font_size: f64,
    wrap_padding: f64,
    note_text_style: &TextStyle,
) {
    for msg in &model.messages {
        if msg.message_type != 2 {
            continue;
        }

        let id = &msg.id;
        let raw = msg.message_text();
        let node_id = format!("note-{id}");
        let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
            continue;
        };
        let (x, y) = node_left_top(n);
        let cx = x + (n.width / 2.0);
        let text_y = y + 5.0;
        let line_step = sequence_text_overrides::sequence_text_line_step_px(actor_label_font_size);
        out.push_str(r#"<g>"#);
        let _ = write!(
            &mut *out,
            r##"<rect x="{x}" y="{y}" fill="#EDF2AE" stroke="#666" width="{w}" height="{h}" class="note"/>"##,
            x = fmt(x),
            y = fmt(y),
            w = fmt(n.width),
            h = fmt(n.height)
        );
        let lines: Vec<String> = if msg.wrap {
            // Mermaid@11.12.2 (Sequence) wraps notes *after* placement width is known:
            //   noteModel.message = wrapLabel(msg.message, noteModel.width - 2*wrapPadding, noteFont)
            //
            // Layout already computed the note box width (`n.width`) to match Mermaid's
            // `noteModel.width`, so wrap to `n.width - 2*wrapPadding` here.
            let wrap_w = (n.width - 2.0 * wrap_padding
                + sequence_text_overrides::sequence_note_wrap_slack_px())
            .max(1.0);
            crate::text::wrap_label_like_mermaid_lines_floored_bbox(
                raw,
                measurer,
                note_text_style,
                wrap_w,
            )
        } else {
            crate::text::split_html_br_lines(raw)
                .into_iter()
                .map(|s| s.to_string())
                .collect()
        };
        for (i, line) in lines.iter().enumerate() {
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
        out.push_str("</g>");
    }
}

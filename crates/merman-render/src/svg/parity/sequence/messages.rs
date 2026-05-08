use super::super::*;
use super::model::{SequenceSvgMessagePayload, SequenceSvgModel};
use crate::generated::sequence_text_overrides_11_12_2 as sequence_text_overrides;
use rustc_hash::FxHashMap;

pub(super) struct SequenceMessageRenderContext<'a> {
    pub(super) model: &'a SequenceSvgModel,
    pub(super) nodes_by_id: &'a FxHashMap<&'a str, &'a LayoutNode>,
    pub(super) edges_by_id: &'a FxHashMap<&'a str, &'a crate::model::LayoutEdge>,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) message_align: &'a str,
    pub(super) actor_height: f64,
    pub(super) actor_label_font_size: f64,
    pub(super) sequence_width: f64,
    pub(super) wrap_padding: f64,
    pub(super) right_angles: bool,
    pub(super) loop_text_style: &'a TextStyle,
}

pub(super) fn render_sequence_messages(out: &mut String, ctx: &SequenceMessageRenderContext<'_>) {
    let mut sequence_number_visible = false;
    let mut sequence_number: i64 = 1;
    let mut sequence_number_step: i64 = 1;

    for msg in &ctx.model.messages {
        match msg.message_type {
            // AUTONUMBER
            26 => {
                if let SequenceSvgMessagePayload::Autonumber(autonumber) = &msg.message {
                    sequence_number_visible = autonumber.visible;
                    if let Some(start) = autonumber.start {
                        sequence_number = start;
                    }
                    if let Some(step) = autonumber.step {
                        sequence_number_step = step;
                    }
                }
                continue;
            }
            // NOTE
            2 => continue,
            _ => {}
        }

        let (Some(from), Some(to)) = (msg.from.as_deref(), msg.to.as_deref()) else {
            continue;
        };
        let edge_id = format!("msg-{}", msg.id);
        let Some(edge) = ctx.edges_by_id.get(edge_id.as_str()).copied() else {
            continue;
        };
        if edge.points.len() < 2 {
            continue;
        }

        let p0 = &edge.points[0];
        let p1 = &edge.points[1];

        let text = msg.message_text();
        if let Some(lbl) = &edge.label {
            let line_step =
                sequence_text_overrides::sequence_text_line_step_px(ctx.actor_label_font_size);
            let bounded_width = (p0.x - p1.x).abs().max(0.0);
            // Mermaid aligns message label text based on `sequence.messageAlign`.
            let (label_x, label_anchor) = match ctx.message_align {
                "right" => (p1.x - 10.0, "end"),
                "left" => (p0.x + 10.0, "start"),
                _ => (lbl.x, "middle"),
            };
            if msg.wrap && !text.is_empty() {
                // Mermaid's `wrapLabel(...)` uses DOM-backed SVG text bbox widths. Our headless
                // vendored metrics are close but can be slightly more conservative in some edge
                // cases; give message wrapping a bit of extra horizontal slack so line breaks match
                // upstream Cypress baselines.
                let wrap_w = (bounded_width + 4.5 * ctx.wrap_padding)
                    .max(ctx.sequence_width)
                    .max(1.0);
                let raw_lines = crate::text::wrap_label_like_mermaid_lines_floored_bbox(
                    text,
                    ctx.measurer,
                    ctx.loop_text_style,
                    wrap_w,
                );
                render_sequence_message_text_lines(
                    out,
                    raw_lines.iter().map(String::as_str),
                    lbl.y,
                    label_x,
                    label_anchor,
                    line_step,
                    ctx.actor_label_font_size,
                );
            } else {
                render_sequence_message_text_lines(
                    out,
                    crate::text::split_html_br_lines(text),
                    lbl.y,
                    label_x,
                    label_anchor,
                    line_step,
                    ctx.actor_label_font_size,
                );
            }
        }

        let class = match msg.message_type {
            1 | 4 | 6 | 25 | 34 => "messageLine1",
            _ => "messageLine0",
        };
        let style = match msg.message_type {
            1 | 4 | 6 | 25 | 34 => r#" style="stroke-dasharray: 3, 3; fill: none;""#,
            _ => r#" style="fill: none;""#,
        };

        let marker_start = match msg.message_type {
            33 | 34 => Some(r#" marker-start="url(#arrowhead)""#),
            _ => None,
        };
        let marker_end = match msg.message_type {
            // open arrow variants: no marker.
            5 | 6 => None,
            // cross arrow variants
            3 | 4 => Some(r#" marker-end="url(#crosshead)""#),
            // filled-head variants
            24 | 25 => Some(r#" marker-end="url(#filled-head)""#),
            // default arrowhead variants
            _ => Some(r#" marker-end="url(#arrowhead)""#),
        };

        // Mermaid uses `stroke="none"` and assigns actual stroke via CSS.
        if from == to {
            let startx = p0.x;
            let y = p0.y;
            let d = if ctx.right_angles {
                let actor_w = ctx
                    .nodes_by_id
                    .get(format!("actor-top-{from}").as_str())
                    .map(|n| n.width)
                    .unwrap_or(ctx.actor_height);
                let text_dx = edge.label.as_ref().map(|l| l.width / 2.0).unwrap_or(0.0);
                let dx = (actor_w / 2.0).max(text_dx);
                format!(
                    "M  {x},{y} H {hx} V {vy} H {x}",
                    x = fmt(startx),
                    y = fmt(y),
                    hx = fmt(startx + dx),
                    vy = fmt(y + 25.0)
                )
            } else {
                format!(
                    "M {x},{y} C {x2},{y2} {x2},{y3} {x},{y4}",
                    x = fmt(startx),
                    y = fmt(y),
                    x2 = fmt(startx + 60.0),
                    y2 = fmt(y - 10.0),
                    y3 = fmt(y + 30.0),
                    y4 = fmt(y + 20.0)
                )
            };
            // Mermaid attaches an `x1` attribute to bidirectional self-reference message paths
            // when sequence numbers are visible (autonumber), even though the geometry lives in
            // the `d` attribute. This keeps DOM parity with upstream Cypress baselines.
            let path_x1 = if sequence_number_visible && marker_start.is_some() {
                Some(p0.x + 6.0)
            } else {
                None
            };
            let _ = write!(
                out,
                r#"<path d="{d}" class="{class}" stroke-width="2" stroke="none"{marker_start}{marker_end}{x1}{style}/>"#,
                d = d,
                class = class,
                marker_start = marker_start.unwrap_or(""),
                marker_end = marker_end.unwrap_or(""),
                x1 = path_x1
                    .map(|x1| format!(r#" x1="{x1}""#, x1 = fmt(x1)))
                    .unwrap_or_default(),
                style = style
            );
        } else {
            let _ = write!(
                out,
                r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" class="{class}" stroke-width="2" stroke="none"{marker_start}{marker_end}{style}/>"#,
                x1 = fmt(p0.x),
                y1 = fmt(p0.y),
                x2 = fmt(p1.x),
                y2 = fmt(p1.y),
                class = class,
                marker_start = marker_start.unwrap_or(""),
                marker_end = marker_end.unwrap_or(""),
                style = style
            );
        }

        if sequence_number_visible {
            let x = p0.x;
            let y = p0.y;
            let _ = write!(
                out,
                r#"<line x1="{x}" y1="{y}" x2="{x}" y2="{y}" stroke-width="0" marker-start="url(#sequencenumber)"/>"#,
                x = fmt(x),
                y = fmt(y),
            );
            let _ = write!(
                out,
                r#"<text x="{x}" y="{y}" font-family="sans-serif" font-size="12px" text-anchor="middle" class="sequenceNumber">{n}</text>"#,
                x = fmt(x),
                y = fmt(y + 4.0),
                n = sequence_number,
            );
            sequence_number = sequence_number.saturating_add(sequence_number_step);
        }

        let _ = (from, to);
    }
}

fn render_sequence_message_text_lines<'a>(
    out: &mut String,
    raw_lines: impl IntoIterator<Item = &'a str>,
    label_y: f64,
    label_x: f64,
    label_anchor: &str,
    line_step: f64,
    actor_label_font_size: f64,
) {
    for (i, raw) in raw_lines.into_iter().enumerate() {
        let y = label_y + (i as f64) * line_step;
        let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(raw);
        let line = if decoded.as_ref().is_empty() {
            "\u{200B}"
        } else {
            decoded.as_ref()
        };
        let _ = write!(
            out,
            r#"<text x="{x}" y="{y}" text-anchor="{anchor}" dominant-baseline="middle" alignment-baseline="middle" class="messageText" dy="1em" style="font-size: {fs}px; font-weight: 400;">{text}</text>"#,
            x = fmt(label_x.round()),
            y = fmt(y),
            anchor = label_anchor,
            fs = fmt(actor_label_font_size),
            text = escape_xml(line)
        );
    }
}

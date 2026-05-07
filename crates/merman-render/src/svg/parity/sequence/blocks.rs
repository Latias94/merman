use super::super::*;
use super::block_collection::AltSection;
use super::block_text::{display_block_label, wrap_svg_text_lines, write_loop_text_lines};
use super::model::SequenceSvgModel;
use crate::generated::sequence_text_overrides_11_12_2 as sequence_text_overrides;
use rustc_hash::FxHashMap;

pub(super) struct SequenceBlockRenderContext<'a> {
    pub(super) default_frame_x1: f64,
    pub(super) default_frame_x2: f64,
    pub(super) msg_endpoints: &'a FxHashMap<&'a str, (&'a str, &'a str)>,
    pub(super) actor_nodes_by_id: &'a FxHashMap<&'a str, &'a LayoutNode>,
    pub(super) edges_by_id: &'a FxHashMap<&'a str, &'a crate::model::LayoutEdge>,
    pub(super) nodes_by_id: &'a FxHashMap<&'a str, &'a LayoutNode>,
    pub(super) label_box_height: f64,
    pub(super) box_text_margin: f64,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) loop_text_style: &'a TextStyle,
}

pub(super) fn write_block_frame(
    out: &mut String,
    frame_x1: f64,
    frame_x2: f64,
    frame_y1: f64,
    frame_y2: f64,
) {
    let _ = write!(
        out,
        r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y1}" class="loopLine"/>"#,
        x1 = fmt(frame_x1),
        x2 = fmt(frame_x2),
        y1 = fmt(frame_y1)
    );
    let _ = write!(
        out,
        r#"<line x1="{x2}" y1="{y1}" x2="{x2}" y2="{y2}" class="loopLine"/>"#,
        x2 = fmt(frame_x2),
        y1 = fmt(frame_y1),
        y2 = fmt(frame_y2)
    );
    let _ = write!(
        out,
        r#"<line x1="{x1}" y1="{y2}" x2="{x2}" y2="{y2}" class="loopLine"/>"#,
        x1 = fmt(frame_x1),
        x2 = fmt(frame_x2),
        y2 = fmt(frame_y2)
    );
    let _ = write!(
        out,
        r#"<line x1="{x1}" y1="{y1}" x2="{x1}" y2="{y2}" class="loopLine"/>"#,
        x1 = fmt(frame_x1),
        y1 = fmt(frame_y1),
        y2 = fmt(frame_y2)
    );
}

pub(super) fn write_block_label_box(out: &mut String, frame_x1: f64, frame_y1: f64, label: &str) {
    let x1 = frame_x1;
    let y1 = frame_y1;
    let x2 = x1 + 50.0;
    let y2 = y1 + 13.0;
    let y3 = y1 + 20.0;
    let x3 = x2 - 8.4;
    let _ = write!(
        out,
        r#"<polygon points="{x1},{y1} {x2},{y1} {x2},{y2} {x3},{y3} {x1},{y3}" class="labelBox"/>"#,
        x1 = fmt(x1),
        y1 = fmt(y1),
        x2 = fmt(x2),
        y2 = fmt(y2),
        x3 = fmt(x3),
        y3 = fmt(y3)
    );
    let label_cx = (x1 + 25.0).round();
    let label_cy = y1 + 13.0;
    let _ = write!(
        out,
        r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="labelText" style="font-size: 16px; font-weight: 400;">{label}</text>"#,
        x = fmt(label_cx),
        y = fmt(label_cy),
        label = escape_xml(label)
    );
}

pub(super) fn render_simple_sequence_block(
    out: &mut String,
    block_label: &str,
    raw_label: &str,
    message_ids: &[String],
    ctx: &SequenceBlockRenderContext<'_>,
) {
    let Some((min_y, max_y)) = message_ids_y_range(
        message_ids.iter(),
        ctx.edges_by_id,
        ctx.nodes_by_id,
        ctx.msg_endpoints,
        false,
    ) else {
        return;
    };

    let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
        message_ids.iter(),
        ctx.msg_endpoints,
        ctx.actor_nodes_by_id,
        ctx.edges_by_id,
        ctx.nodes_by_id,
    )
    .unwrap_or((ctx.default_frame_x1, ctx.default_frame_x2, f64::INFINITY));

    let header_offset = if block_label == "break" {
        93.0
    } else if raw_label.trim().is_empty() {
        (79.0 - ctx.label_box_height).max(0.0)
    } else {
        79.0
    };
    let frame_y1 = min_y - header_offset;
    let frame_y2 = max_y + 10.0;

    out.push_str(r#"<g>"#);
    write_block_frame(out, frame_x1, frame_x2, frame_y1, frame_y2);
    write_block_label_box(out, frame_x1, frame_y1, block_label);
    let label_box_right = frame_x1 + 50.0;
    let text_x = (label_box_right + frame_x2) / 2.0;
    let text_y = frame_y1 + 18.0;
    let label = display_block_label(raw_label, true).unwrap_or_else(|| "\u{200B}".to_string());
    let max_w = (frame_x2 - label_box_right).max(0.0);
    write_loop_text_lines(
        out,
        ctx.measurer,
        ctx.loop_text_style,
        text_x,
        text_y,
        Some(max_w),
        &label,
        true,
    );
    out.push_str("</g>");
}

pub(super) fn render_sectioned_sequence_block(
    out: &mut String,
    block_label: &str,
    sections: &[AltSection],
    adjust_header_for_wrap: bool,
    ctx: &SequenceBlockRenderContext<'_>,
) {
    if sections.is_empty() {
        return;
    }

    let Some((min_y, max_y)) = section_message_y_range(
        sections,
        ctx.edges_by_id,
        ctx.nodes_by_id,
        ctx.msg_endpoints,
        false,
    ) else {
        return;
    };

    let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
        sections.iter().flat_map(|s| s.message_ids.iter()),
        ctx.msg_endpoints,
        ctx.actor_nodes_by_id,
        ctx.edges_by_id,
        ctx.nodes_by_id,
    )
    .unwrap_or((ctx.default_frame_x1, ctx.default_frame_x2, f64::INFINITY));

    let header_offset = section_header_offset(
        sections,
        frame_x1,
        frame_x2,
        adjust_header_for_wrap,
        ctx.label_box_height,
        ctx.box_text_margin,
        ctx.measurer,
        ctx.loop_text_style,
    );
    let frame_y1 = min_y - header_offset;
    let frame_y2 = max_y + 10.0;

    out.push_str(r#"<g>"#);

    // frame
    write_block_frame(out, frame_x1, frame_x2, frame_y1, frame_y2);

    // separators (dashed)
    // Keep separator endpoints identical to the frame endpoints to match upstream
    // Mermaid output and avoid sub-pixel gaps at the frame border.
    let dash_x1 = frame_x1;
    let dash_x2 = frame_x2;
    let sep_ys = section_separator_ys(
        sections,
        min_y,
        ctx.edges_by_id,
        ctx.nodes_by_id,
        ctx.msg_endpoints,
    );
    for y in &sep_ys {
        let _ = write!(
            out,
            r#"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="loopLine" style="stroke-dasharray: 3, 3;"/>"#,
            x1 = fmt(dash_x1),
            x2 = fmt(dash_x2),
            y = fmt(*y)
        );
    }

    // label box + label text
    write_block_label_box(out, frame_x1, frame_y1, block_label);

    // section labels
    let label_box_right = frame_x1 + 50.0;
    let main_text_x = (label_box_right + frame_x2) / 2.0;
    let center_text_x = (frame_x1 + frame_x2) / 2.0;
    for (i, sec) in sections.iter().enumerate() {
        let Some(label_text) = display_block_label(&sec.raw_label, i == 0) else {
            continue;
        };
        if i == 0 {
            let y = frame_y1 + 18.0;
            let max_w = (frame_x2 - label_box_right).max(0.0);
            write_loop_text_lines(
                out,
                ctx.measurer,
                ctx.loop_text_style,
                main_text_x,
                y,
                Some(max_w),
                &label_text,
                true,
            );
            continue;
        }
        let y = sep_ys.get(i - 1).copied().unwrap_or(frame_y1) + 18.0;
        write_loop_text_lines(
            out,
            ctx.measurer,
            ctx.loop_text_style,
            center_text_x,
            y,
            None,
            &label_text,
            false,
        );
    }

    out.push_str("</g>");
}

pub(super) fn render_critical_sequence_block(
    out: &mut String,
    sections: &[AltSection],
    ctx: &SequenceBlockRenderContext<'_>,
) {
    if sections.is_empty() {
        return;
    }

    let Some((min_y, max_y)) = section_message_y_range(
        sections,
        ctx.edges_by_id,
        ctx.nodes_by_id,
        ctx.msg_endpoints,
        false,
    ) else {
        return;
    };

    let (mut frame_x1, frame_x2, min_left) = frame_x_from_message_ids(
        sections.iter().flat_map(|s| s.message_ids.iter()),
        ctx.msg_endpoints,
        ctx.actor_nodes_by_id,
        ctx.edges_by_id,
        ctx.nodes_by_id,
    )
    .unwrap_or((ctx.default_frame_x1, ctx.default_frame_x2, f64::INFINITY));
    if sections.len() > 1 && min_left.is_finite() {
        // Mermaid's `critical` w/ `option` sections widens the frame to the left.
        frame_x1 = frame_x1.min(min_left - 9.0);
    }

    let header_offset = if sections
        .first()
        .is_some_and(|s| s.raw_label.trim().is_empty())
    {
        (79.0 - ctx.label_box_height).max(0.0)
    } else if sections.len() > 1 {
        // Mermaid does not apply the wrap height adjustment for multi-section
        // `critical` blocks (those with one or more `option` sections).
        79.0
    } else {
        // Mermaid's `adjustLoopHeightForWrap(...)` expands the header height when the
        // section label wraps to multiple lines. This affects the frame's top y.
        let label_text = display_block_label(&sections[0].raw_label, true)
            .unwrap_or_else(|| "\u{200B}".to_string());
        let label_box_right = frame_x1 + 50.0;
        let max_w = (frame_x2 - label_box_right).max(0.0);
        let wrapped =
            wrap_svg_text_lines(&label_text, ctx.measurer, ctx.loop_text_style, Some(max_w));
        let extra_lines = wrapped.len().saturating_sub(1) as f64;
        let extra_per_line =
            (sequence_text_overrides::sequence_text_line_step_px(ctx.loop_text_style.font_size)
                - ctx.box_text_margin)
                .max(0.0);
        79.0 + extra_lines * extra_per_line
    };
    let frame_y1 = min_y - header_offset;
    let frame_y2 = max_y + 10.0;

    out.push_str(r#"<g>"#);

    // frame
    write_block_frame(out, frame_x1, frame_x2, frame_y1, frame_y2);

    // separators (dashed)
    let dash_x1 = frame_x1;
    let dash_x2 = frame_x2;
    let sep_ys = section_separator_ys(
        sections,
        min_y,
        ctx.edges_by_id,
        ctx.nodes_by_id,
        ctx.msg_endpoints,
    );
    for y in &sep_ys {
        let _ = write!(
            out,
            r#"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="loopLine" style="stroke-dasharray: 3, 3;"/>"#,
            x1 = fmt(dash_x1),
            x2 = fmt(dash_x2),
            y = fmt(*y)
        );
    }

    // label box + label text
    write_block_label_box(out, frame_x1, frame_y1, "critical");

    // section labels
    let label_box_right = frame_x1 + 50.0;
    let main_text_x = (label_box_right + frame_x2) / 2.0;
    let center_text_x = (frame_x1 + frame_x2) / 2.0;
    for (i, sec) in sections.iter().enumerate() {
        let Some(label_text) = display_block_label(&sec.raw_label, i == 0) else {
            continue;
        };
        if i == 0 {
            let y = frame_y1 + 18.0;
            let max_w = (frame_x2 - label_box_right).max(0.0);
            write_loop_text_lines(
                out,
                ctx.measurer,
                ctx.loop_text_style,
                main_text_x,
                y,
                Some(max_w),
                &label_text,
                true,
            );
            continue;
        }
        let y = sep_ys.get(i - 1).copied().unwrap_or(frame_y1) + 18.0;
        write_loop_text_lines(
            out,
            ctx.measurer,
            ctx.loop_text_style,
            center_text_x,
            y,
            None,
            &label_text,
            false,
        );
    }

    out.push_str("</g>");
}

fn section_header_offset(
    sections: &[AltSection],
    frame_x1: f64,
    frame_x2: f64,
    adjust_header_for_wrap: bool,
    label_box_height: f64,
    box_text_margin: f64,
    measurer: &dyn TextMeasurer,
    loop_text_style: &TextStyle,
) -> f64 {
    if sections
        .first()
        .is_some_and(|s| s.raw_label.trim().is_empty())
    {
        return (79.0 - label_box_height).max(0.0);
    }
    if !adjust_header_for_wrap {
        return 79.0;
    }

    let base = 79.0;
    let label_box_right = frame_x1 + 50.0;
    let max_w = (frame_x2 - label_box_right).max(0.0);
    let label =
        display_block_label(&sections[0].raw_label, true).unwrap_or_else(|| "\u{200B}".to_string());
    let wrapped = wrap_svg_text_lines(&label, measurer, loop_text_style, Some(max_w));
    let extra_lines = wrapped.len().saturating_sub(1) as f64;
    let extra_per_line =
        (sequence_text_overrides::sequence_text_line_step_px(loop_text_style.font_size)
            - box_text_margin)
            .max(0.0);
    base + extra_lines * extra_per_line
}

pub(super) fn frame_x_from_actors(
    model: &SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
) -> Option<(f64, f64)> {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    for actor_id in &model.actor_order {
        let node_id = format!("actor-top-{actor_id}");
        let n = nodes_by_id.get(node_id.as_str()).copied()?;
        min_x = min_x.min(n.x);
        max_x = max_x.max(n.x);
    }
    if !min_x.is_finite() || !max_x.is_finite() {
        return None;
    }
    Some((
        min_x - sequence_text_overrides::sequence_frame_side_pad_px(),
        max_x + sequence_text_overrides::sequence_frame_side_pad_px(),
    ))
}

pub(super) fn frame_x_from_message_ids<'a>(
    message_ids: impl IntoIterator<Item = &'a String>,
    msg_endpoints: &FxHashMap<&str, (&str, &str)>,
    actor_nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
) -> Option<(f64, f64, f64)> {
    // For single-actor frames containing only self-messages, upstream Mermaid expands the
    // frame to cover at least the actor box width (plus a small asymmetric pad that leaves
    // room for the self-arrow loop on the right). Our deterministic layout edge points can
    // be too narrow for short self-message labels, which would over-wrap frame titles.
    let mut min_left = f64::INFINITY;
    let mut geom_min_x = f64::INFINITY;
    let mut geom_max_x = f64::NEG_INFINITY;
    let mut min_cx = f64::INFINITY;
    let mut max_cx = f64::NEG_INFINITY;
    let mut self_only_actor: Option<&str> = None;

    for msg_id in message_ids {
        // Notes are nodes (not edges); include their bounding boxes in frame extents.
        let note_node_id = format!("note-{msg_id}");
        if let Some(n) = nodes_by_id.get(note_node_id.as_str()).copied() {
            geom_min_x = geom_min_x
                .min(n.x - n.width / 2.0 - sequence_text_overrides::sequence_frame_geom_pad_px());
            geom_max_x = geom_max_x
                .max(n.x + n.width / 2.0 + sequence_text_overrides::sequence_frame_geom_pad_px());
        }

        let Some((from, to)) = msg_endpoints.get(msg_id.as_str()).copied() else {
            continue;
        };
        if from == to {
            self_only_actor = match self_only_actor {
                None => Some(from),
                Some(prev) if prev == from => Some(prev),
                _ => Some(""),
            };
        } else {
            self_only_actor = Some("");
        }

        // Expand frames to cover message geometry and label overflow (especially important
        // for single-actor blocks containing long self-message labels).
        let edge_id = format!("msg-{msg_id}");
        if let Some(e) = edges_by_id.get(edge_id.as_str()).copied() {
            for p in &e.points {
                geom_min_x = geom_min_x.min(p.x);
                geom_max_x = geom_max_x.max(p.x);
            }
            if let Some(label) = e.label.as_ref() {
                geom_min_x = geom_min_x.min(
                    label.x
                        - (label.width / 2.0)
                        - sequence_text_overrides::sequence_frame_geom_pad_px(),
                );
                geom_max_x = geom_max_x.max(
                    label.x
                        + (label.width / 2.0)
                        + sequence_text_overrides::sequence_frame_geom_pad_px(),
                );
            }
        }
        for actor_id in [from, to] {
            let Some(n) = actor_nodes_by_id.get(actor_id).copied() else {
                continue;
            };
            min_cx = min_cx.min(n.x);
            max_cx = max_cx.max(n.x);
            min_left = min_left.min(n.x - n.width / 2.0);
        }
    }

    if !min_cx.is_finite() || !max_cx.is_finite() {
        return None;
    }
    let mut x1 = min_cx - sequence_text_overrides::sequence_frame_side_pad_px();
    let mut x2 = max_cx + sequence_text_overrides::sequence_frame_side_pad_px();
    if geom_min_x.is_finite() {
        x1 = x1.min(geom_min_x);
    }
    if geom_max_x.is_finite() {
        x2 = x2.max(geom_max_x);
    }
    if matches!(self_only_actor, Some(a) if !a.is_empty()) {
        if let Some(n) = actor_nodes_by_id.get(self_only_actor.unwrap()).copied() {
            let left = n.x - n.width / 2.0;
            let right = n.x + n.width / 2.0;
            let min_x1 = left - sequence_text_overrides::sequence_self_only_frame_min_pad_left_px();
            let min_x2 =
                right + sequence_text_overrides::sequence_self_only_frame_min_pad_right_px();
            // Only widen when the computed geometry is suspiciously narrow; avoid shifting
            // frames that already match upstream due to message label geometry.
            if (x2 - x1) < (min_x2 - min_x1) - 1.0 {
                x1 = x1.min(min_x1);
                x2 = x2.max(min_x2);
            }
        }
    }
    Some((x1, x2, min_left))
}

pub(super) fn item_y_range(
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    msg_endpoints: &FxHashMap<&str, (&str, &str)>,
    item_id: &str,
    is_separator: bool,
) -> Option<(f64, f64)> {
    let msg_range = if is_separator {
        msg_y_range_for_separators(edges_by_id, msg_endpoints, item_id)
    } else {
        msg_y_range_for_frame(edges_by_id, msg_endpoints, item_id)
    };
    if let Some((y0, y1)) = msg_range {
        return Some((y0, y1));
    }
    let note_node_id = format!("note-{item_id}");
    let n = nodes_by_id.get(note_node_id.as_str()).copied()?;
    let top = n.y - n.height / 2.0;
    let bottom = n.y + n.height / 2.0;
    Some((top, bottom))
}

pub(super) fn message_ids_y_range<'a>(
    message_ids: impl IntoIterator<Item = &'a String>,
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    msg_endpoints: &FxHashMap<&str, (&str, &str)>,
    is_separator: bool,
) -> Option<(f64, f64)> {
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for msg_id in message_ids {
        if let Some((y0, y1)) = item_y_range(
            edges_by_id,
            nodes_by_id,
            msg_endpoints,
            msg_id,
            is_separator,
        ) {
            min_y = min_y.min(y0);
            max_y = max_y.max(y1);
        }
    }
    if !min_y.is_finite() || !max_y.is_finite() {
        return None;
    }
    Some((min_y, max_y))
}

pub(super) fn section_message_y_range(
    sections: &[AltSection],
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    msg_endpoints: &FxHashMap<&str, (&str, &str)>,
    is_separator: bool,
) -> Option<(f64, f64)> {
    message_ids_y_range(
        sections.iter().flat_map(|s| s.message_ids.iter()),
        edges_by_id,
        nodes_by_id,
        msg_endpoints,
        is_separator,
    )
}

pub(super) fn section_separator_ys(
    sections: &[AltSection],
    min_y: f64,
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    msg_endpoints: &FxHashMap<&str, (&str, &str)>,
) -> Vec<f64> {
    let mut section_max_ys: Vec<f64> = Vec::new();
    for sec in sections {
        let sec_max_y = message_ids_y_range(
            sec.message_ids.iter(),
            edges_by_id,
            nodes_by_id,
            msg_endpoints,
            true,
        )
        .map(|(_y0, y1)| y1)
        .unwrap_or(min_y);
        section_max_ys.push(sec_max_y);
    }
    section_max_ys
        .iter()
        .take(section_max_ys.len().saturating_sub(1))
        .map(|sec_max_y| *sec_max_y + 15.0)
        .collect()
}

fn msg_line_y(
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    msg_id: &str,
) -> Option<f64> {
    let edge_id = format!("msg-{msg_id}");
    let e = edges_by_id.get(edge_id.as_str()).copied()?;
    Some(e.points.first()?.y)
}

fn msg_y_range_with_self_extra(
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    msg_endpoints: &FxHashMap<&str, (&str, &str)>,
    msg_id: &str,
    self_extra_y: f64,
) -> Option<(f64, f64)> {
    let y = msg_line_y(edges_by_id, msg_id)?;
    let extra = msg_endpoints
        .get(msg_id)
        .copied()
        .filter(|(from, to)| from == to)
        .map(|_| self_extra_y)
        .unwrap_or(0.0);
    Some((y, y + extra))
}

fn msg_y_range_for_frame(
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    msg_endpoints: &FxHashMap<&str, (&str, &str)>,
    msg_id: &str,
) -> Option<(f64, f64)> {
    // Mermaid's `boundMessage(...)` self-message branch expands the inserted bounds by 60px
    // below `lineStartY` (see the `+ 30 + totalOffset` bottom coordinate, where `totalOffset`
    // already includes a `+30` bump).
    msg_y_range_with_self_extra(
        edges_by_id,
        msg_endpoints,
        msg_id,
        sequence_text_overrides::sequence_self_message_frame_extra_y_px(),
    )
}

fn msg_y_range_for_separators(
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    msg_endpoints: &FxHashMap<&str, (&str, &str)>,
    msg_id: &str,
) -> Option<(f64, f64)> {
    // The self-message loop curve itself extends ~30px below the message line.
    // Mermaid's dashed section separators follow the curve geometry, not the full `bounds.insert(...)`
    // envelope used for frame sizing.
    msg_y_range_with_self_extra(
        edges_by_id,
        msg_endpoints,
        msg_id,
        sequence_text_overrides::sequence_self_message_separator_extra_y_px(),
    )
}

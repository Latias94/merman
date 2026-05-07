use super::super::*;
use super::block_collection::AltSection;
use super::model::SequenceSvgModel;
use crate::generated::sequence_text_overrides_11_12_2 as sequence_text_overrides;
use rustc_hash::FxHashMap;

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
        Some(bracketize(t))
    }
}

pub(super) fn wrap_svg_text_lines(
    text: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_width: Option<f64>,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in crate::text::split_html_br_lines(text) {
        if let Some(w) = max_width {
            lines.extend(wrap_svg_text_line(line, measurer, style, w));
        } else {
            lines.push(line.to_string());
        }
    }
    if lines.is_empty() {
        vec!["".to_string()]
    } else {
        lines
    }
}

pub(super) fn write_loop_text_lines(
    out: &mut String,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    x: f64,
    y0: f64,
    max_width: Option<f64>,
    text: &str,
    use_tspan: bool,
) {
    let line_step = sequence_text_overrides::sequence_text_line_step_px(style.font_size);
    let lines = wrap_svg_text_lines(text, measurer, style, max_width);
    for (i, line) in lines.into_iter().enumerate() {
        let y = y0 + (i as f64) * line_step;
        if use_tspan {
            let _ = write!(
                out,
                r#"<text x="{x}" y="{y}" text-anchor="middle" class="loopText" style="font-size: {fs}px; font-weight: 400;"><tspan x="{x}">{text}</tspan></text>"#,
                x = fmt(x),
                y = fmt(y),
                fs = fmt(style.font_size),
                text = escape_xml(&line)
            );
        } else {
            let _ = write!(
                out,
                r#"<text x="{x}" y="{y}" text-anchor="middle" class="loopText" style="font-size: {fs}px; font-weight: 400;">{text}</text>"#,
                x = fmt(x),
                y = fmt(y),
                fs = fmt(style.font_size),
                text = escape_xml(&line)
            );
        }
    }
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
    default_frame_x1: f64,
    default_frame_x2: f64,
    msg_endpoints: &FxHashMap<&str, (&str, &str)>,
    actor_nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    label_box_height: f64,
    measurer: &dyn TextMeasurer,
    loop_text_style: &TextStyle,
) {
    let Some((min_y, max_y)) = message_ids_y_range(
        message_ids.iter(),
        edges_by_id,
        nodes_by_id,
        msg_endpoints,
        false,
    ) else {
        return;
    };

    let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
        message_ids.iter(),
        msg_endpoints,
        actor_nodes_by_id,
        edges_by_id,
        nodes_by_id,
    )
    .unwrap_or((default_frame_x1, default_frame_x2, f64::INFINITY));

    let header_offset = if block_label == "break" {
        93.0
    } else if raw_label.trim().is_empty() {
        (79.0 - label_box_height).max(0.0)
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
        measurer,
        loop_text_style,
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
    default_frame_x1: f64,
    default_frame_x2: f64,
    msg_endpoints: &FxHashMap<&str, (&str, &str)>,
    actor_nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    label_box_height: f64,
    box_text_margin: f64,
    measurer: &dyn TextMeasurer,
    loop_text_style: &TextStyle,
) {
    if sections.is_empty() {
        return;
    }

    let Some((min_y, max_y)) =
        section_message_y_range(sections, edges_by_id, nodes_by_id, msg_endpoints, false)
    else {
        return;
    };

    let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
        sections.iter().flat_map(|s| s.message_ids.iter()),
        msg_endpoints,
        actor_nodes_by_id,
        edges_by_id,
        nodes_by_id,
    )
    .unwrap_or((default_frame_x1, default_frame_x2, f64::INFINITY));

    let header_offset = section_header_offset(
        sections,
        frame_x1,
        frame_x2,
        adjust_header_for_wrap,
        label_box_height,
        box_text_margin,
        measurer,
        loop_text_style,
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
    let sep_ys = section_separator_ys(sections, min_y, edges_by_id, nodes_by_id, msg_endpoints);
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
                measurer,
                loop_text_style,
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
            measurer,
            loop_text_style,
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
    default_frame_x1: f64,
    default_frame_x2: f64,
    msg_endpoints: &FxHashMap<&str, (&str, &str)>,
    actor_nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    label_box_height: f64,
    box_text_margin: f64,
    measurer: &dyn TextMeasurer,
    loop_text_style: &TextStyle,
) {
    if sections.is_empty() {
        return;
    }

    let Some((min_y, max_y)) =
        section_message_y_range(sections, edges_by_id, nodes_by_id, msg_endpoints, false)
    else {
        return;
    };

    let (mut frame_x1, frame_x2, min_left) = frame_x_from_message_ids(
        sections.iter().flat_map(|s| s.message_ids.iter()),
        msg_endpoints,
        actor_nodes_by_id,
        edges_by_id,
        nodes_by_id,
    )
    .unwrap_or((default_frame_x1, default_frame_x2, f64::INFINITY));
    if sections.len() > 1 && min_left.is_finite() {
        // Mermaid's `critical` w/ `option` sections widens the frame to the left.
        frame_x1 = frame_x1.min(min_left - 9.0);
    }

    let header_offset = if sections
        .first()
        .is_some_and(|s| s.raw_label.trim().is_empty())
    {
        (79.0 - label_box_height).max(0.0)
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
        let wrapped = wrap_svg_text_lines(&label_text, measurer, loop_text_style, Some(max_w));
        let extra_lines = wrapped.len().saturating_sub(1) as f64;
        let extra_per_line =
            (sequence_text_overrides::sequence_text_line_step_px(loop_text_style.font_size)
                - box_text_margin)
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
    let sep_ys = section_separator_ys(sections, min_y, edges_by_id, nodes_by_id, msg_endpoints);
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
                measurer,
                loop_text_style,
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
            measurer,
            loop_text_style,
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

fn bracketize(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        return "\u{200B}".to_string();
    }
    if t.starts_with('[') && t.ends_with(']') {
        return t.to_string();
    }
    format!("[{t}]")
}

fn split_line_to_words(text: &str) -> Vec<String> {
    let parts = text.split(' ').collect::<Vec<_>>();
    let mut out: Vec<String> = Vec::new();
    for part in parts {
        if !part.is_empty() {
            out.push(part.to_string());
        }
        out.push(" ".to_string());
    }
    while out.last().is_some_and(|s| s == " ") {
        out.pop();
    }
    out
}

fn wrap_svg_text_line(
    line: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_width: f64,
) -> Vec<String> {
    use std::collections::VecDeque;

    if !max_width.is_finite() || max_width <= 0.0 {
        return vec![line.to_string()];
    }

    // Mermaid's frame-label wrapping behaves as if the available width were slightly smaller
    // than the raw `frame_x2 - (frame_x1 + label_box_width)` span, especially for narrow
    // (single-actor-ish) frames. Apply a small pad only in that regime to avoid over-wrapping
    // wide frames like `critical` headers.
    let pad = if max_width <= 160.0 {
        15.0
    } else if max_width <= 230.0 {
        8.0
    } else {
        0.0
    };
    let max_width = (max_width - pad).max(1.0);

    fn svg_bbox_width_px(measurer: &dyn TextMeasurer, style: &TextStyle, text: &str) -> f64 {
        let (l, r) = measurer.measure_svg_text_bbox_x(text, style);
        (l + r).max(0.0)
    }

    let mut tokens = VecDeque::from(split_line_to_words(line));
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut force_break_after_next_non_space: bool = false;

    while let Some(tok) = tokens.pop_front() {
        if cur.is_empty() && tok == " " {
            continue;
        }

        let candidate = format!("{cur}{tok}");
        if svg_bbox_width_px(measurer, style, &candidate) <= max_width {
            cur = candidate;
            if force_break_after_next_non_space && tok != " " {
                out.push(cur.trim_end().to_string());
                cur.clear();
                force_break_after_next_non_space = false;
            }
            continue;
        }

        if !cur.trim().is_empty() {
            out.push(cur.trim_end().to_string());
            cur.clear();
            tokens.push_front(tok);
            continue;
        }

        if tok == " " {
            continue;
        }

        // `tok` itself does not fit on an empty line; split by characters.
        let chars = tok.chars().collect::<Vec<_>>();
        let mut cut = 1usize;
        while cut < chars.len() {
            let mut head: String = chars[..cut].iter().collect();
            let tail_len = chars.len().saturating_sub(cut);
            let should_hyphenate = tail_len > 0
                && !head.ends_with('-')
                && head
                    .chars()
                    .last()
                    .is_some_and(|ch| ch.is_ascii_alphanumeric());
            if should_hyphenate {
                head.push('-');
            }
            if svg_bbox_width_px(measurer, style, &head) > max_width {
                break;
            }
            cut += 1;
        }
        cut = cut.saturating_sub(1).max(1);
        let mut head: String = chars[..cut].iter().collect();
        let tail: String = chars[cut..].iter().collect();
        let mut hyphenated = false;
        if !tail.is_empty()
            && !head.ends_with('-')
            && head
                .chars()
                .last()
                .is_some_and(|ch| ch.is_ascii_alphanumeric())
            && svg_bbox_width_px(measurer, style, &(head.clone() + "-")) <= max_width
        {
            head.push('-');
            hyphenated = true;
        }
        out.push(head);
        if !tail.is_empty() {
            tokens.push_front(tail);
            if hyphenated {
                force_break_after_next_non_space = true;
            }
        }
    }

    if !cur.trim().is_empty() {
        out.push(cur.trim_end().to_string());
    }

    if out.is_empty() {
        vec!["".to_string()]
    } else {
        out
    }
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

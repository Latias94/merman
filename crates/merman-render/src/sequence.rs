use crate::model::{
    Bounds, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint, SequenceDiagramLayout,
};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
struct SequenceActor {
    name: String,
    description: String,
    #[serde(rename = "type")]
    actor_type: String,
    wrap: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct SequenceMessage {
    id: String,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
    #[serde(rename = "type")]
    message_type: i32,
    message: Value,
    wrap: bool,
    activate: bool,
    #[serde(default)]
    placement: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
struct SequenceModel {
    #[serde(rename = "actorOrder")]
    actor_order: Vec<String>,
    actors: std::collections::BTreeMap<String, SequenceActor>,
    messages: Vec<SequenceMessage>,
    title: Option<String>,
}

fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_f64()
        .or_else(|| cur.as_i64().map(|n| n as f64))
        .or_else(|| cur.as_u64().map(|n| n as f64))
}

fn config_string(cfg: &Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn split_html_br_lines(text: &str) -> Vec<&str> {
    let b = text.as_bytes();
    let mut parts: Vec<&str> = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i + 3 < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let b1 = b[i + 1];
        let b2 = b[i + 2];
        if !matches!(b1, b'b' | b'B') || !matches!(b2, b'r' | b'R') {
            i += 1;
            continue;
        }
        let mut j = i + 3;
        while j < b.len() && matches!(b[j], b' ' | b'\t' | b'\r' | b'\n') {
            j += 1;
        }
        if j < b.len() && b[j] == b'/' {
            j += 1;
        }
        if j < b.len() && b[j] == b'>' {
            parts.push(&text[start..i]);
            start = j + 1;
            i = start;
            continue;
        }
        i += 1;
    }
    parts.push(&text[start..]);
    parts
}

fn measure_svg_like_with_html_br(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
) -> (f64, f64) {
    let lines = split_html_br_lines(text);
    let default_line_height = (style.font_size.max(1.0) * 1.1).max(1.0);
    if lines.len() <= 1 {
        let metrics = measurer.measure_wrapped(text, style, None, WrapMode::SvgLikeSingleRun);
        return (
            metrics.width.max(0.0),
            metrics.height.max(default_line_height),
        );
    }
    let mut max_w: f64 = 0.0;
    let mut line_h: f64 = 0.0;
    for line in &lines {
        let metrics = measurer.measure_wrapped(line, style, None, WrapMode::SvgLikeSingleRun);
        max_w = max_w.max(metrics.width.max(0.0));
        line_h = line_h.max(metrics.height.max(default_line_height));
    }
    (
        max_w,
        (line_h * lines.len() as f64).max(default_line_height),
    )
}

fn sequence_actor_visual_height(actor_type: &str, base_height: f64, label_box_height: f64) -> f64 {
    match actor_type {
        // Mermaid (11.12.2) derives these from the actor-type glyph bbox + label box height.
        // These heights are used by the footer actor rendering and affect the final SVG viewBox.
        "boundary" => (60.0 + label_box_height).max(1.0),
        "entity" => (36.0 + label_box_height).max(1.0),
        // Control uses an extra label-box height in Mermaid.
        "control" => (36.0 + 2.0 * label_box_height).max(1.0),
        _ => base_height.max(1.0),
    }
}

fn sequence_actor_lifeline_start_y(
    actor_type: &str,
    base_height: f64,
    box_text_margin: f64,
) -> f64 {
    match actor_type {
        // Hard-coded in Mermaid's sequence svgDraw.js for these actor types.
        "boundary" => 80.0,
        "control" | "entity" => 75.0,
        // For database, Mermaid starts the lifeline slightly below the actor box.
        "database" => base_height + 2.0 * box_text_margin,
        _ => base_height,
    }
}

pub fn layout_sequence_diagram(
    semantic: &Value,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
) -> Result<SequenceDiagramLayout> {
    let model: SequenceModel = serde_json::from_value(semantic.clone())?;

    let seq_cfg = effective_config.get("sequence").unwrap_or(&Value::Null);
    let diagram_margin_x = config_f64(seq_cfg, &["diagramMarginX"]).unwrap_or(50.0);
    let diagram_margin_y = config_f64(seq_cfg, &["diagramMarginY"]).unwrap_or(10.0);
    let bottom_margin_adj = config_f64(seq_cfg, &["bottomMarginAdj"]).unwrap_or(1.0);
    let actor_margin = config_f64(seq_cfg, &["actorMargin"]).unwrap_or(50.0);
    let actor_width_min = config_f64(seq_cfg, &["width"]).unwrap_or(150.0);
    let actor_height = config_f64(seq_cfg, &["height"]).unwrap_or(65.0);
    let message_margin = config_f64(seq_cfg, &["messageMargin"]).unwrap_or(35.0);
    let wrap_padding = config_f64(seq_cfg, &["wrapPadding"]).unwrap_or(10.0);
    let box_text_margin = config_f64(seq_cfg, &["boxTextMargin"]).unwrap_or(5.0);
    let label_box_height = config_f64(seq_cfg, &["labelBoxHeight"]).unwrap_or(20.0);

    // Mermaid's `sequenceRenderer.setConf(...)` overrides per-sequence font settings whenever the
    // global `fontFamily` / `fontSize` / `fontWeight` are present (defaults are always present).
    let global_font_family = config_string(effective_config, &["fontFamily"]);
    let global_font_size = config_f64(effective_config, &["fontSize"]);
    let global_font_weight = config_string(effective_config, &["fontWeight"]);

    let message_font_family = global_font_family
        .clone()
        .or_else(|| config_string(seq_cfg, &["messageFontFamily"]));
    let message_font_size = global_font_size
        .or_else(|| config_f64(seq_cfg, &["messageFontSize"]))
        .unwrap_or(16.0);
    let message_font_weight = global_font_weight
        .clone()
        .or_else(|| config_string(seq_cfg, &["messageFontWeight"]));

    let actor_font_family = global_font_family
        .clone()
        .or_else(|| config_string(seq_cfg, &["actorFontFamily"]));
    let actor_font_size = global_font_size
        .or_else(|| config_f64(seq_cfg, &["actorFontSize"]))
        .unwrap_or(16.0);
    let actor_font_weight = global_font_weight
        .clone()
        .or_else(|| config_string(seq_cfg, &["actorFontWeight"]));

    // Upstream sequence uses `calculateTextDimensions(...).width` (SVG `getBBox`) when computing
    // message widths for spacing. Keep this scale at 1.0 and handle any residual differences via
    // the SVG-backed `TextMeasurer` implementation.
    let message_width_scale = 1.0;

    let actor_text_style = TextStyle {
        font_family: actor_font_family,
        font_size: actor_font_size,
        font_weight: actor_font_weight,
    };
    let note_font_family = global_font_family
        .clone()
        .or_else(|| config_string(seq_cfg, &["noteFontFamily"]));
    let note_font_size = global_font_size
        .or_else(|| config_f64(seq_cfg, &["noteFontSize"]))
        .unwrap_or(16.0);
    let note_font_weight = global_font_weight
        .clone()
        .or_else(|| config_string(seq_cfg, &["noteFontWeight"]));
    let note_text_style = TextStyle {
        font_family: note_font_family,
        font_size: note_font_size,
        font_weight: note_font_weight,
    };
    let msg_text_style = TextStyle {
        font_family: message_font_family,
        font_size: message_font_size,
        font_weight: message_font_weight,
    };

    if model.actor_order.is_empty() {
        return Err(Error::InvalidModel {
            message: "sequence model has no actorOrder".to_string(),
        });
    }

    // Measure participant boxes.
    let mut actor_widths: Vec<f64> = Vec::with_capacity(model.actor_order.len());
    for id in &model.actor_order {
        let a = model.actors.get(id).ok_or_else(|| Error::InvalidModel {
            message: format!("missing actor {id}"),
        })?;
        let (w0, _h0) = measure_svg_like_with_html_br(measurer, &a.description, &actor_text_style);
        let w = (w0 + 2.0 * wrap_padding).max(actor_width_min);
        actor_widths.push(w.max(1.0));
    }

    // Determine the per-actor margins using Mermaid's `getMaxMessageWidthPerActor(...)` rules,
    // then compute actor x positions from those margins (see upstream `boundActorData`).
    let mut actor_index: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (i, id) in model.actor_order.iter().enumerate() {
        actor_index.insert(id.as_str(), i);
    }

    let mut actor_to_message_width: Vec<f64> = vec![0.0; model.actor_order.len()];
    for msg in &model.messages {
        let (Some(from), Some(to)) = (msg.from.as_deref(), msg.to.as_deref()) else {
            continue;
        };
        let Some(&from_idx) = actor_index.get(from) else {
            continue;
        };
        let Some(&to_idx) = actor_index.get(to) else {
            continue;
        };

        let placement = msg.placement;
        // If this is the first actor, and the note is left of it, no need to calculate the margin.
        if placement == Some(0) && to_idx == 0 {
            continue;
        }
        // If this is the last actor, and the note is right of it, no need to calculate the margin.
        if placement == Some(1) && to_idx + 1 == model.actor_order.len() {
            continue;
        }

        let is_note = placement.is_some();
        let is_message = !is_note;
        let style = if is_note {
            &note_text_style
        } else {
            &msg_text_style
        };
        let text = msg.message.as_str().unwrap_or_default();
        if text.is_empty() {
            continue;
        }

        let (w0, _h0) = measure_svg_like_with_html_br(measurer, text, style);
        let w0 = w0 * message_width_scale;
        let message_w = (w0 + 2.0 * wrap_padding).max(0.0);

        let prev_idx = if to_idx > 0 { Some(to_idx - 1) } else { None };
        let next_idx = if to_idx + 1 < model.actor_order.len() {
            Some(to_idx + 1)
        } else {
            None
        };

        if is_message && next_idx.is_some_and(|n| n == from_idx) {
            actor_to_message_width[to_idx] = actor_to_message_width[to_idx].max(message_w);
        } else if is_message && prev_idx.is_some_and(|p| p == from_idx) {
            actor_to_message_width[from_idx] = actor_to_message_width[from_idx].max(message_w);
        } else if is_message && from_idx == to_idx {
            let half = message_w / 2.0;
            actor_to_message_width[from_idx] = actor_to_message_width[from_idx].max(half);
            actor_to_message_width[to_idx] = actor_to_message_width[to_idx].max(half);
        } else if placement == Some(1) {
            // RIGHTOF
            actor_to_message_width[from_idx] = actor_to_message_width[from_idx].max(message_w);
        } else if placement == Some(0) {
            // LEFTOF
            if let Some(p) = prev_idx {
                actor_to_message_width[p] = actor_to_message_width[p].max(message_w);
            }
        } else if placement == Some(2) {
            // OVER
            if let Some(p) = prev_idx {
                actor_to_message_width[p] = actor_to_message_width[p].max(message_w / 2.0);
            }
            if next_idx.is_some() {
                actor_to_message_width[from_idx] =
                    actor_to_message_width[from_idx].max(message_w / 2.0);
            }
        }
    }

    let mut actor_margins: Vec<f64> = vec![actor_margin; model.actor_order.len()];
    for i in 0..model.actor_order.len() {
        let msg_w = actor_to_message_width[i];
        if msg_w <= 0.0 {
            continue;
        }
        let w0 = actor_widths[i];
        let actor_w = if i + 1 < model.actor_order.len() {
            let w1 = actor_widths[i + 1];
            msg_w + actor_margin - (w0 / 2.0) - (w1 / 2.0)
        } else {
            msg_w + actor_margin - (w0 / 2.0)
        };
        actor_margins[i] = actor_w.max(actor_margin);
    }

    let mut actor_left_x: Vec<f64> = Vec::with_capacity(model.actor_order.len());
    let mut prev_width = 0.0;
    let mut prev_margin = 0.0;
    for i in 0..model.actor_order.len() {
        let x = prev_width + prev_margin;
        actor_left_x.push(x);
        prev_width += actor_widths[i] + prev_margin;
        prev_margin = actor_margins[i];
    }

    let mut actor_centers_x: Vec<f64> = Vec::with_capacity(model.actor_order.len());
    for i in 0..model.actor_order.len() {
        actor_centers_x.push(actor_left_x[i] + actor_widths[i] / 2.0);
    }

    let message_step = message_margin + (message_font_size / 2.0) + bottom_margin_adj;
    let msg_label_offset = (message_step - message_font_size) + bottom_margin_adj;

    let mut edges: Vec<LayoutEdge> = Vec::new();
    let mut nodes: Vec<LayoutNode> = Vec::new();
    let clusters: Vec<LayoutCluster> = Vec::new();

    // Actor boxes: Mermaid renders both a "top" and "bottom" actor box.
    // The bottom boxes start after all messages are placed.
    for (idx, id) in model.actor_order.iter().enumerate() {
        let w = actor_widths[idx];
        let cx = actor_centers_x[idx];
        let actor_type = model
            .actors
            .get(id)
            .map(|a| a.actor_type.as_str())
            .unwrap_or("participant");
        let visual_h = sequence_actor_visual_height(actor_type, actor_height, label_box_height);
        let top_y = visual_h / 2.0;
        nodes.push(LayoutNode {
            id: format!("actor-top-{id}"),
            x: cx,
            y: top_y,
            width: w,
            height: visual_h,
            is_cluster: false,
        });
    }

    // Message edges.

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

    fn block_label_text(raw_label: &str) -> String {
        bracketize(raw_label)
    }

    // Mermaid advances the "cursor" for sequence blocks (loop/alt/opt/par/break/critical) even
    // though these directives are not message edges. The cursor increment depends on the wrapped
    // block label height; precompute these increments per directive message id.
    let block_base_step = message_step + bottom_margin_adj;
    let line_step = message_font_size * 1.1875;
    let block_extra_per_line = (line_step - box_text_margin).max(0.0);
    let block_end_step = 10.0;

    let mut msg_by_id: std::collections::HashMap<&str, &SequenceMessage> =
        std::collections::HashMap::new();
    for msg in &model.messages {
        msg_by_id.insert(msg.id.as_str(), msg);
    }

    fn is_self_message_id(
        msg_id: &str,
        msg_by_id: &std::collections::HashMap<&str, &SequenceMessage>,
    ) -> bool {
        let Some(msg) = msg_by_id.get(msg_id).copied() else {
            return false;
        };
        // Notes can use `from==to` for `rightOf`/`leftOf`; do not treat them as self-messages.
        if msg.message_type == 2 {
            return false;
        }
        msg.from
            .as_deref()
            .is_some_and(|from| Some(from) == msg.to.as_deref())
    }

    fn message_span_x(
        msg: &SequenceMessage,
        actor_index: &std::collections::HashMap<&str, usize>,
        actor_centers_x: &[f64],
        measurer: &dyn TextMeasurer,
        msg_text_style: &TextStyle,
        message_width_scale: f64,
    ) -> Option<(f64, f64)> {
        let (Some(from), Some(to)) = (msg.from.as_deref(), msg.to.as_deref()) else {
            return None;
        };
        let (Some(fi), Some(ti)) = (actor_index.get(from).copied(), actor_index.get(to).copied())
        else {
            return None;
        };
        let from_x = actor_centers_x[fi];
        let to_x = actor_centers_x[ti];
        let sign = if to_x >= from_x { 1.0 } else { -1.0 };
        let x1 = from_x + sign * 1.0;
        let x2 = if from == to { x1 } else { to_x - sign * 4.0 };
        let cx = (x1 + x2) / 2.0;

        let text = msg.message.as_str().unwrap_or_default();
        let w = if text.is_empty() {
            1.0
        } else {
            let (w, _h) = measure_svg_like_with_html_br(measurer, text, msg_text_style);
            (w * message_width_scale).max(1.0)
        };
        Some((cx - w / 2.0, cx + w / 2.0))
    }

    fn block_frame_width(
        message_ids: &[String],
        msg_by_id: &std::collections::HashMap<&str, &SequenceMessage>,
        actor_index: &std::collections::HashMap<&str, usize>,
        actor_centers_x: &[f64],
        actor_widths: &[f64],
        message_margin: f64,
        box_text_margin: f64,
        bottom_margin_adj: f64,
        measurer: &dyn TextMeasurer,
        msg_text_style: &TextStyle,
        message_width_scale: f64,
    ) -> Option<f64> {
        let mut actor_idxs: Vec<usize> = Vec::new();
        for msg_id in message_ids {
            let Some(msg) = msg_by_id.get(msg_id.as_str()).copied() else {
                continue;
            };
            let (Some(from), Some(to)) = (msg.from.as_deref(), msg.to.as_deref()) else {
                continue;
            };
            if let Some(i) = actor_index.get(from).copied() {
                actor_idxs.push(i);
            }
            if let Some(i) = actor_index.get(to).copied() {
                actor_idxs.push(i);
            }
        }
        actor_idxs.sort();
        actor_idxs.dedup();
        if actor_idxs.is_empty() {
            return None;
        }

        if actor_idxs.len() == 1 {
            let i = actor_idxs[0];
            let actor_w = actor_widths.get(i).copied().unwrap_or(150.0);
            let center_x = message_ids
                .iter()
                .find_map(|id| {
                    let msg = msg_by_id.get(id.as_str()).copied()?;
                    let (l, r) = message_span_x(
                        msg,
                        actor_index,
                        actor_centers_x,
                        measurer,
                        msg_text_style,
                        message_width_scale,
                    )?;
                    Some((l + r) / 2.0)
                })
                .unwrap_or(actor_centers_x[i] + 1.0);

            let half_width =
                actor_w / 2.0 + (message_margin / 2.0) + box_text_margin + bottom_margin_adj;
            let w = (2.0 * half_width).max(1.0);
            return Some(w);
        }

        let min_i = actor_idxs.first().copied()?;
        let max_i = actor_idxs.last().copied()?;
        let mut x1 = actor_centers_x[min_i] - 11.0;
        let mut x2 = actor_centers_x[max_i] + 11.0;

        // Expand multi-actor blocks to include overflowing message labels (e.g. long self messages).
        for msg_id in message_ids {
            let Some(msg) = msg_by_id.get(msg_id.as_str()).copied() else {
                continue;
            };
            let Some((l, r)) = message_span_x(
                msg,
                actor_index,
                actor_centers_x,
                measurer,
                msg_text_style,
                message_width_scale,
            ) else {
                continue;
            };
            if l < x1 {
                x1 = l.floor();
            }
            if r > x2 {
                x2 = r.ceil();
            }
        }

        Some((x2 - x1).max(1.0))
    }

    #[derive(Debug, Clone)]
    enum BlockStackEntry {
        Loop {
            start_id: String,
            raw_label: String,
            messages: Vec<String>,
        },
        Opt {
            start_id: String,
            raw_label: String,
            messages: Vec<String>,
        },
        Break {
            start_id: String,
            raw_label: String,
            messages: Vec<String>,
        },
        Alt {
            section_directives: Vec<(String, String)>,
            sections: Vec<Vec<String>>,
        },
        Par {
            section_directives: Vec<(String, String)>,
            sections: Vec<Vec<String>>,
        },
        Critical {
            section_directives: Vec<(String, String)>,
            sections: Vec<Vec<String>>,
        },
    }

    let mut directive_steps: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();
    let mut stack: Vec<BlockStackEntry> = Vec::new();
    for msg in &model.messages {
        let raw_label = msg.message.as_str().unwrap_or_default();
        match msg.message_type {
            // loop start/end
            10 => stack.push(BlockStackEntry::Loop {
                start_id: msg.id.clone(),
                raw_label: raw_label.to_string(),
                messages: Vec::new(),
            }),
            11 => {
                if let Some(BlockStackEntry::Loop {
                    start_id,
                    raw_label,
                    messages,
                }) = stack.pop()
                {
                    let loop_has_self_message = messages
                        .iter()
                        .any(|msg_id| is_self_message_id(msg_id.as_str(), &msg_by_id));
                    let loop_end_step = if loop_has_self_message {
                        40.0
                    } else {
                        block_end_step
                    };

                    if let Some(w) = block_frame_width(
                        &messages,
                        &msg_by_id,
                        &actor_index,
                        &actor_centers_x,
                        &actor_widths,
                        message_margin,
                        box_text_margin,
                        bottom_margin_adj,
                        measurer,
                        &msg_text_style,
                        message_width_scale,
                    ) {
                        let label = block_label_text(&raw_label);
                        let metrics = measurer.measure_wrapped(
                            &label,
                            &msg_text_style,
                            Some(w),
                            WrapMode::SvgLikeSingleRun,
                        );
                        let extra =
                            (metrics.line_count.saturating_sub(1) as f64) * block_extra_per_line;
                        directive_steps.insert(start_id, block_base_step + extra);
                    } else {
                        directive_steps.insert(start_id, block_base_step);
                    }

                    directive_steps.insert(msg.id.clone(), loop_end_step);
                }
            }
            // opt start/end
            15 => stack.push(BlockStackEntry::Opt {
                start_id: msg.id.clone(),
                raw_label: raw_label.to_string(),
                messages: Vec::new(),
            }),
            16 => {
                let mut end_step = block_end_step;
                if let Some(BlockStackEntry::Opt {
                    start_id,
                    raw_label,
                    messages,
                }) = stack.pop()
                {
                    let has_self = messages
                        .iter()
                        .any(|msg_id| is_self_message_id(msg_id.as_str(), &msg_by_id));
                    end_step = if has_self { 40.0 } else { block_end_step };
                    if let Some(w) = block_frame_width(
                        &messages,
                        &msg_by_id,
                        &actor_index,
                        &actor_centers_x,
                        &actor_widths,
                        message_margin,
                        box_text_margin,
                        bottom_margin_adj,
                        measurer,
                        &msg_text_style,
                        message_width_scale,
                    ) {
                        let label = block_label_text(&raw_label);
                        let metrics = measurer.measure_wrapped(
                            &label,
                            &msg_text_style,
                            Some(w),
                            WrapMode::SvgLikeSingleRun,
                        );
                        let extra =
                            (metrics.line_count.saturating_sub(1) as f64) * block_extra_per_line;
                        directive_steps.insert(start_id, block_base_step + extra);
                    } else {
                        directive_steps.insert(start_id, block_base_step);
                    }
                }
                directive_steps.insert(msg.id.clone(), end_step);
            }
            // break start/end
            30 => stack.push(BlockStackEntry::Break {
                start_id: msg.id.clone(),
                raw_label: raw_label.to_string(),
                messages: Vec::new(),
            }),
            31 => {
                let mut end_step = block_end_step;
                if let Some(BlockStackEntry::Break {
                    start_id,
                    raw_label,
                    messages,
                }) = stack.pop()
                {
                    let has_self = messages
                        .iter()
                        .any(|msg_id| is_self_message_id(msg_id.as_str(), &msg_by_id));
                    end_step = if has_self { 40.0 } else { block_end_step };
                    if let Some(w) = block_frame_width(
                        &messages,
                        &msg_by_id,
                        &actor_index,
                        &actor_centers_x,
                        &actor_widths,
                        message_margin,
                        box_text_margin,
                        bottom_margin_adj,
                        measurer,
                        &msg_text_style,
                        message_width_scale,
                    ) {
                        let label = block_label_text(&raw_label);
                        let metrics = measurer.measure_wrapped(
                            &label,
                            &msg_text_style,
                            Some(w),
                            WrapMode::SvgLikeSingleRun,
                        );
                        let extra =
                            (metrics.line_count.saturating_sub(1) as f64) * block_extra_per_line;
                        directive_steps.insert(start_id, block_base_step + extra);
                    } else {
                        directive_steps.insert(start_id, block_base_step);
                    }
                }
                directive_steps.insert(msg.id.clone(), end_step);
            }
            // alt start/else/end
            12 => stack.push(BlockStackEntry::Alt {
                section_directives: vec![(msg.id.clone(), raw_label.to_string())],
                sections: vec![Vec::new()],
            }),
            13 => {
                if let Some(BlockStackEntry::Alt {
                    section_directives,
                    sections,
                }) = stack.last_mut()
                {
                    section_directives.push((msg.id.clone(), raw_label.to_string()));
                    sections.push(Vec::new());
                }
            }
            14 => {
                let mut end_step = block_end_step;
                if let Some(BlockStackEntry::Alt {
                    section_directives,
                    sections,
                }) = stack.pop()
                {
                    let has_self = sections
                        .iter()
                        .flatten()
                        .any(|msg_id| is_self_message_id(msg_id.as_str(), &msg_by_id));
                    end_step = if has_self { 40.0 } else { block_end_step };
                    let mut message_ids: Vec<String> = Vec::new();
                    for sec in &sections {
                        message_ids.extend(sec.iter().cloned());
                    }
                    if let Some(w) = block_frame_width(
                        &message_ids,
                        &msg_by_id,
                        &actor_index,
                        &actor_centers_x,
                        &actor_widths,
                        message_margin,
                        box_text_margin,
                        bottom_margin_adj,
                        measurer,
                        &msg_text_style,
                        message_width_scale,
                    ) {
                        for (idx, (id, raw)) in section_directives.into_iter().enumerate() {
                            let label = if raw.trim().is_empty() && idx != 0 {
                                "\u{200B}".to_string()
                            } else {
                                block_label_text(&raw)
                            };
                            let metrics = measurer.measure_wrapped(
                                &label,
                                &msg_text_style,
                                Some(w),
                                WrapMode::SvgLikeSingleRun,
                            );
                            let extra = (metrics.line_count.saturating_sub(1) as f64)
                                * block_extra_per_line;
                            directive_steps.insert(id, block_base_step + extra);
                        }
                    } else {
                        for (id, _raw) in section_directives {
                            directive_steps.insert(id, block_base_step);
                        }
                    }
                }
                directive_steps.insert(msg.id.clone(), end_step);
            }
            // par start/and/end
            19 | 32 => stack.push(BlockStackEntry::Par {
                section_directives: vec![(msg.id.clone(), raw_label.to_string())],
                sections: vec![Vec::new()],
            }),
            20 => {
                if let Some(BlockStackEntry::Par {
                    section_directives,
                    sections,
                }) = stack.last_mut()
                {
                    section_directives.push((msg.id.clone(), raw_label.to_string()));
                    sections.push(Vec::new());
                }
            }
            21 => {
                let mut end_step = block_end_step;
                if let Some(BlockStackEntry::Par {
                    section_directives,
                    sections,
                }) = stack.pop()
                {
                    let has_self = sections
                        .iter()
                        .flatten()
                        .any(|msg_id| is_self_message_id(msg_id.as_str(), &msg_by_id));
                    end_step = if has_self { 40.0 } else { block_end_step };
                    let mut message_ids: Vec<String> = Vec::new();
                    for sec in &sections {
                        message_ids.extend(sec.iter().cloned());
                    }
                    if let Some(w) = block_frame_width(
                        &message_ids,
                        &msg_by_id,
                        &actor_index,
                        &actor_centers_x,
                        &actor_widths,
                        message_margin,
                        box_text_margin,
                        bottom_margin_adj,
                        measurer,
                        &msg_text_style,
                        message_width_scale,
                    ) {
                        for (idx, (id, raw)) in section_directives.into_iter().enumerate() {
                            let label = if raw.trim().is_empty() && idx != 0 {
                                "\u{200B}".to_string()
                            } else {
                                block_label_text(&raw)
                            };
                            let metrics = measurer.measure_wrapped(
                                &label,
                                &msg_text_style,
                                Some(w),
                                WrapMode::SvgLikeSingleRun,
                            );
                            let extra = (metrics.line_count.saturating_sub(1) as f64)
                                * block_extra_per_line;
                            directive_steps.insert(id, block_base_step + extra);
                        }
                    } else {
                        for (id, _raw) in section_directives {
                            directive_steps.insert(id, block_base_step);
                        }
                    }
                }
                directive_steps.insert(msg.id.clone(), end_step);
            }
            // critical start/option/end
            27 => stack.push(BlockStackEntry::Critical {
                section_directives: vec![(msg.id.clone(), raw_label.to_string())],
                sections: vec![Vec::new()],
            }),
            28 => {
                if let Some(BlockStackEntry::Critical {
                    section_directives,
                    sections,
                }) = stack.last_mut()
                {
                    section_directives.push((msg.id.clone(), raw_label.to_string()));
                    sections.push(Vec::new());
                }
            }
            29 => {
                let mut end_step = block_end_step;
                if let Some(BlockStackEntry::Critical {
                    section_directives,
                    sections,
                }) = stack.pop()
                {
                    let has_self = sections
                        .iter()
                        .flatten()
                        .any(|msg_id| is_self_message_id(msg_id.as_str(), &msg_by_id));
                    end_step = if has_self { 40.0 } else { block_end_step };
                    let mut message_ids: Vec<String> = Vec::new();
                    for sec in &sections {
                        message_ids.extend(sec.iter().cloned());
                    }
                    if let Some(w) = block_frame_width(
                        &message_ids,
                        &msg_by_id,
                        &actor_index,
                        &actor_centers_x,
                        &actor_widths,
                        message_margin,
                        box_text_margin,
                        bottom_margin_adj,
                        measurer,
                        &msg_text_style,
                        message_width_scale,
                    ) {
                        for (idx, (id, raw)) in section_directives.into_iter().enumerate() {
                            let label = if raw.trim().is_empty() && idx != 0 {
                                "\u{200B}".to_string()
                            } else {
                                block_label_text(&raw)
                            };
                            let metrics = measurer.measure_wrapped(
                                &label,
                                &msg_text_style,
                                Some(w),
                                WrapMode::SvgLikeSingleRun,
                            );
                            let extra = (metrics.line_count.saturating_sub(1) as f64)
                                * block_extra_per_line;
                            directive_steps.insert(id, block_base_step + extra);
                        }
                    } else {
                        for (id, _raw) in section_directives {
                            directive_steps.insert(id, block_base_step);
                        }
                    }
                }
                directive_steps.insert(msg.id.clone(), end_step);
            }
            _ => {
                // If this is a "real" message edge, attach it to all active block scopes so block
                // width computations can account for overflowing message labels.
                if msg.from.is_some() && msg.to.is_some() {
                    for entry in stack.iter_mut() {
                        match entry {
                            BlockStackEntry::Alt { sections, .. }
                            | BlockStackEntry::Par { sections, .. }
                            | BlockStackEntry::Critical { sections, .. } => {
                                if let Some(cur) = sections.last_mut() {
                                    cur.push(msg.id.clone());
                                }
                            }
                            BlockStackEntry::Loop { messages, .. }
                            | BlockStackEntry::Opt { messages, .. }
                            | BlockStackEntry::Break { messages, .. } => {
                                messages.push(msg.id.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    #[derive(Debug, Clone)]
    struct RectOpen {
        start_id: String,
        top_y: f64,
        min_x: f64,
        max_x: f64,
        max_y: f64,
    }

    // Mermaid's sequence renderer advances a "cursor" even for non-message directives (notes,
    // rect blocks). To avoid overlapping bottom actors and to match upstream viewBox sizes, we
    // model these increments in headless layout as well.
    let note_width_single = actor_width_min;
    let rect_step_start = 20.0;
    let rect_step_end = 10.0;
    let note_gap = 10.0;
    // Mermaid note boxes use 10px vertical padding on both sides (20px total), on top of the
    // SVG `getBBox().height` of the note text.
    let note_text_pad_total = 20.0;
    let note_top_offset = message_step - note_gap;

    let mut cursor_y = actor_height + message_step;
    let mut rect_stack: Vec<RectOpen> = Vec::new();
    let activation_width = config_f64(seq_cfg, &["activationWidth"])
        .unwrap_or(10.0)
        .max(1.0);
    let mut activation_stacks: std::collections::BTreeMap<&str, Vec<f64>> =
        std::collections::BTreeMap::new();

    for msg in &model.messages {
        match msg.message_type {
            // ACTIVE_START
            17 => {
                let Some(actor_id) = msg.from.as_deref() else {
                    continue;
                };
                let Some(&idx) = actor_index.get(actor_id) else {
                    continue;
                };
                let cx = actor_centers_x[idx];
                let stack = activation_stacks.entry(actor_id).or_default();
                let stacked_size = stack.len();
                let startx = cx + (((stacked_size as f64) - 1.0) * activation_width) / 2.0;
                stack.push(startx);
                continue;
            }
            // ACTIVE_END
            18 => {
                let Some(actor_id) = msg.from.as_deref() else {
                    continue;
                };
                if let Some(stack) = activation_stacks.get_mut(actor_id) {
                    let _ = stack.pop();
                }
                continue;
            }
            _ => {}
        }

        if let Some(step) = directive_steps.get(msg.id.as_str()).copied() {
            cursor_y += step;
            continue;
        }
        match msg.message_type {
            // rect start: advances cursor but draws later as a background `<rect>`.
            22 => {
                rect_stack.push(RectOpen {
                    start_id: msg.id.clone(),
                    top_y: cursor_y - note_top_offset,
                    min_x: f64::INFINITY,
                    max_x: f64::NEG_INFINITY,
                    max_y: f64::NEG_INFINITY,
                });
                cursor_y += rect_step_start;
                continue;
            }
            // rect end
            23 => {
                if let Some(open) = rect_stack.pop() {
                    let rect_left = if open.min_x.is_finite() {
                        open.min_x
                    } else {
                        actor_centers_x
                            .iter()
                            .copied()
                            .fold(f64::INFINITY, f64::min)
                            - 11.0
                    };
                    let rect_right = if open.max_x.is_finite() {
                        open.max_x
                    } else {
                        actor_centers_x
                            .iter()
                            .copied()
                            .fold(f64::NEG_INFINITY, f64::max)
                            + 11.0
                    };
                    let rect_bottom = if open.max_y.is_finite() {
                        open.max_y + 10.0
                    } else {
                        open.top_y + 10.0
                    };
                    let rect_w = (rect_right - rect_left).max(1.0);
                    let rect_h = (rect_bottom - open.top_y).max(1.0);

                    nodes.push(LayoutNode {
                        id: format!("rect-{}", open.start_id),
                        x: rect_left + rect_w / 2.0,
                        y: open.top_y + rect_h / 2.0,
                        width: rect_w,
                        height: rect_h,
                        is_cluster: false,
                    });

                    if let Some(parent) = rect_stack.last_mut() {
                        parent.min_x = parent.min_x.min(rect_left - 10.0);
                        parent.max_x = parent.max_x.max(rect_right + 10.0);
                        parent.max_y = parent.max_y.max(rect_bottom);
                    }
                }
                cursor_y += rect_step_end;
                continue;
            }
            _ => {}
        }

        // Notes (type=2) are laid out as nodes, not message edges.
        if msg.message_type == 2 {
            let (Some(from), Some(to)) = (msg.from.as_deref(), msg.to.as_deref()) else {
                continue;
            };
            let (Some(fi), Some(ti)) =
                (actor_index.get(from).copied(), actor_index.get(to).copied())
            else {
                continue;
            };
            let fx = actor_centers_x[fi];
            let tx = actor_centers_x[ti];

            let placement = msg.placement.unwrap_or(2);
            let (note_x, note_w) = match placement {
                // leftOf
                0 => (fx - 25.0 - note_width_single, note_width_single),
                // rightOf
                1 => (fx + 25.0, note_width_single),
                // over
                _ => {
                    if (fx - tx).abs() < 0.0001 {
                        (fx - (note_width_single / 2.0), note_width_single)
                    } else {
                        let left = fx.min(tx) - 25.0;
                        let right = fx.max(tx) + 25.0;
                        let w = (right - left).max(note_width_single);
                        (left, w)
                    }
                }
            };

            let text = msg.message.as_str().unwrap_or_default();
            let (_w, h) = measure_svg_like_with_html_br(measurer, text, &note_text_style);
            let note_h = (h + note_text_pad_total).round().max(1.0);
            let note_y = (cursor_y - note_top_offset).round();

            nodes.push(LayoutNode {
                id: format!("note-{}", msg.id),
                x: note_x + note_w / 2.0,
                y: note_y + note_h / 2.0,
                width: note_w.max(1.0),
                height: note_h,
                is_cluster: false,
            });

            for open in rect_stack.iter_mut() {
                open.min_x = open.min_x.min(note_x - 10.0);
                open.max_x = open.max_x.max(note_x + note_w + 10.0);
                open.max_y = open.max_y.max(note_y + note_h);
            }

            cursor_y += note_h + note_gap;
            continue;
        }

        // Regular message edges.
        let (Some(from), Some(to)) = (msg.from.as_deref(), msg.to.as_deref()) else {
            continue;
        };
        let (Some(fi), Some(ti)) = (actor_index.get(from).copied(), actor_index.get(to).copied())
        else {
            continue;
        };
        let from_x = actor_centers_x[fi];
        let to_x = actor_centers_x[ti];

        let (from_left, from_right) = activation_stacks
            .get(from)
            .and_then(|s| s.last().copied())
            .map(|startx| (startx, startx + activation_width))
            .unwrap_or((from_x - 1.0, from_x + 1.0));

        let (to_left, to_right) = activation_stacks
            .get(to)
            .and_then(|s| s.last().copied())
            .map(|startx| (startx, startx + activation_width))
            .unwrap_or((to_x - 1.0, to_x + 1.0));

        let is_arrow_to_right = from_left <= to_left;
        let mut startx = if is_arrow_to_right {
            from_right
        } else {
            from_left
        };
        let mut stopx = if is_arrow_to_right { to_left } else { to_right };

        let adjust_value = |v: f64| if is_arrow_to_right { -v } else { v };
        let is_arrow_to_activation = (to_left - to_right).abs() > 2.0;

        let is_self = from == to;
        if is_self {
            stopx = startx;
        } else {
            if msg.activate && !is_arrow_to_activation {
                stopx += adjust_value(activation_width / 2.0 - 1.0);
            }

            if !matches!(msg.message_type, 5 | 6) {
                stopx += adjust_value(3.0);
            }

            if matches!(msg.message_type, 33 | 34) {
                startx -= adjust_value(3.0);
            }
        }

        let x1 = startx;
        let x2 = stopx;
        let y = cursor_y;

        let text = msg.message.as_str().unwrap_or_default();
        let label = if text.is_empty() {
            // Mermaid renders an (empty) message text node even when the label is empty (e.g.
            // trailing colon `Alice->Bob:`). Keep a placeholder label to preserve DOM structure.
            Some(LayoutLabel {
                x: ((x1 + x2) / 2.0).round(),
                y: (y - msg_label_offset).round(),
                width: 1.0,
                height: message_font_size.max(1.0),
            })
        } else {
            let (w, h) = measure_svg_like_with_html_br(measurer, text, &msg_text_style);
            Some(LayoutLabel {
                x: ((x1 + x2) / 2.0).round(),
                y: (y - msg_label_offset).round(),
                width: (w * message_width_scale).max(1.0),
                height: h.max(1.0),
            })
        };

        edges.push(LayoutEdge {
            id: format!("msg-{}", msg.id),
            from: from.to_string(),
            to: to.to_string(),
            from_cluster: None,
            to_cluster: None,
            points: vec![LayoutPoint { x: x1, y }, LayoutPoint { x: x2, y }],
            label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        });

        for open in rect_stack.iter_mut() {
            let lx = from_x.min(to_x) - 11.0;
            let rx = from_x.max(to_x) + 11.0;
            open.min_x = open.min_x.min(lx);
            open.max_x = open.max_x.max(rx);
            open.max_y = open.max_y.max(y);
        }

        cursor_y += message_step;
        if is_self {
            // Mermaid adds extra vertical space for self-messages to accommodate the loop curve.
            cursor_y += 30.0;
        }
    }

    let bottom_margin = message_margin - message_font_size + bottom_margin_adj;
    let bottom_box_top_y = (cursor_y - message_step) + bottom_margin;
    for (idx, id) in model.actor_order.iter().enumerate() {
        let w = actor_widths[idx];
        let cx = actor_centers_x[idx];
        let actor_type = model
            .actors
            .get(id)
            .map(|a| a.actor_type.as_str())
            .unwrap_or("participant");
        let visual_h = sequence_actor_visual_height(actor_type, actor_height, label_box_height);
        nodes.push(LayoutNode {
            id: format!("actor-bottom-{id}"),
            x: cx,
            y: bottom_box_top_y + visual_h / 2.0,
            width: w,
            height: visual_h,
            is_cluster: false,
        });

        edges.push(LayoutEdge {
            id: format!("lifeline-{id}"),
            from: format!("actor-top-{id}"),
            to: format!("actor-bottom-{id}"),
            from_cluster: None,
            to_cluster: None,
            points: vec![
                LayoutPoint {
                    x: cx,
                    y: sequence_actor_lifeline_start_y(actor_type, actor_height, box_text_margin),
                },
                LayoutPoint {
                    x: cx,
                    y: bottom_box_top_y,
                },
            ],
            label: None,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        });
    }

    // Mermaid's SVG `viewBox` is derived from `svg.getBBox()` plus diagram margins. Block frames
    // (`alt`, `par`, `loop`, `opt`, `break`, `critical`) can extend beyond the node/edge graph we
    // model in headless layout. Capture their extents so we can expand bounds before emitting the
    // final `viewBox`.
    let block_bounds = {
        use std::collections::HashMap;

        let nodes_by_id: HashMap<&str, &LayoutNode> = nodes
            .iter()
            .map(|n| (n.id.as_str(), n))
            .collect::<HashMap<_, _>>();
        let edges_by_id: HashMap<&str, &LayoutEdge> = edges
            .iter()
            .map(|e| (e.id.as_str(), e))
            .collect::<HashMap<_, _>>();

        let mut msg_endpoints: HashMap<&str, (&str, &str)> = HashMap::new();
        for msg in &model.messages {
            let (Some(from), Some(to)) = (msg.from.as_deref(), msg.to.as_deref()) else {
                continue;
            };
            msg_endpoints.insert(msg.id.as_str(), (from, to));
        }

        fn item_y_range(
            item_id: &str,
            nodes_by_id: &HashMap<&str, &LayoutNode>,
            edges_by_id: &HashMap<&str, &LayoutEdge>,
            msg_endpoints: &HashMap<&str, (&str, &str)>,
        ) -> Option<(f64, f64)> {
            // Mermaid's self-message branch expands bounds by 60px below the message line y
            // coordinate (see the `+ 30 + totalOffset` bottom coordinate, where `totalOffset`
            // already includes a `+30` bump).
            const SELF_MESSAGE_EXTRA_Y: f64 = 60.0;
            let edge_id = format!("msg-{item_id}");
            if let Some(e) = edges_by_id.get(edge_id.as_str()).copied() {
                let y = e.points.first()?.y;
                let extra = msg_endpoints
                    .get(item_id)
                    .copied()
                    .filter(|(from, to)| from == to)
                    .map(|_| SELF_MESSAGE_EXTRA_Y)
                    .unwrap_or(0.0);
                return Some((y, y + extra));
            }

            let node_id = format!("note-{item_id}");
            let n = nodes_by_id.get(node_id.as_str()).copied()?;
            let top = n.y - n.height / 2.0;
            let bottom = n.y + n.height / 2.0;
            Some((top, bottom))
        }

        fn frame_x_from_item_ids<'a>(
            item_ids: impl IntoIterator<Item = &'a String>,
            nodes_by_id: &HashMap<&str, &LayoutNode>,
            edges_by_id: &HashMap<&str, &LayoutEdge>,
            msg_endpoints: &HashMap<&str, (&str, &str)>,
        ) -> Option<(f64, f64, f64)> {
            const SIDE_PAD: f64 = 11.0;
            const GEOM_PAD: f64 = 10.0;
            let mut min_cx = f64::INFINITY;
            let mut max_cx = f64::NEG_INFINITY;
            let mut min_left = f64::INFINITY;
            let mut geom_min_x = f64::INFINITY;
            let mut geom_max_x = f64::NEG_INFINITY;

            for id in item_ids {
                // Notes contribute directly via their node bounds.
                let note_id = format!("note-{id}");
                if let Some(n) = nodes_by_id.get(note_id.as_str()).copied() {
                    geom_min_x = geom_min_x.min(n.x - n.width / 2.0 - GEOM_PAD);
                    geom_max_x = geom_max_x.max(n.x + n.width / 2.0 + GEOM_PAD);
                }

                let Some((from, to)) = msg_endpoints.get(id.as_str()).copied() else {
                    continue;
                };
                for actor_id in [from, to] {
                    let actor_node_id = format!("actor-top-{actor_id}");
                    let Some(n) = nodes_by_id.get(actor_node_id.as_str()).copied() else {
                        continue;
                    };
                    min_cx = min_cx.min(n.x);
                    max_cx = max_cx.max(n.x);
                    min_left = min_left.min(n.x - n.width / 2.0);
                }

                // Message edges can overflow via label widths.
                let edge_id = format!("msg-{id}");
                if let Some(e) = edges_by_id.get(edge_id.as_str()).copied() {
                    for p in &e.points {
                        geom_min_x = geom_min_x.min(p.x);
                        geom_max_x = geom_max_x.max(p.x);
                    }
                    if let Some(label) = e.label.as_ref() {
                        geom_min_x = geom_min_x.min(label.x - (label.width / 2.0) - GEOM_PAD);
                        geom_max_x = geom_max_x.max(label.x + (label.width / 2.0) + GEOM_PAD);
                    }
                }
            }

            if !min_cx.is_finite() || !max_cx.is_finite() {
                return None;
            }
            let mut x1 = min_cx - SIDE_PAD;
            let mut x2 = max_cx + SIDE_PAD;
            if geom_min_x.is_finite() {
                x1 = x1.min(geom_min_x);
            }
            if geom_max_x.is_finite() {
                x2 = x2.max(geom_max_x);
            }
            Some((x1, x2, min_left))
        }

        #[derive(Debug)]
        enum BlockStackEntry {
            Loop { items: Vec<String> },
            Opt { items: Vec<String> },
            Break { items: Vec<String> },
            Alt { sections: Vec<Vec<String>> },
            Par { sections: Vec<Vec<String>> },
            Critical { sections: Vec<Vec<String>> },
        }

        let mut block_min_x = f64::INFINITY;
        let mut block_min_y = f64::INFINITY;
        let mut block_max_x = f64::NEG_INFINITY;
        let mut block_max_y = f64::NEG_INFINITY;

        let mut stack: Vec<BlockStackEntry> = Vec::new();
        for msg in &model.messages {
            let msg_id = msg.id.clone();
            match msg.message_type {
                10 => stack.push(BlockStackEntry::Loop { items: Vec::new() }),
                11 => {
                    if let Some(BlockStackEntry::Loop { items }) = stack.pop() {
                        if let (Some((x1, x2, _min_left)), Some((y0, y1))) = (
                            frame_x_from_item_ids(
                                &items,
                                &nodes_by_id,
                                &edges_by_id,
                                &msg_endpoints,
                            ),
                            items
                                .iter()
                                .filter_map(|id| {
                                    item_y_range(id, &nodes_by_id, &edges_by_id, &msg_endpoints)
                                })
                                .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1))),
                        ) {
                            let frame_y1 = y0 - 79.0;
                            let frame_y2 = y1 + 10.0;
                            block_min_x = block_min_x.min(x1);
                            block_max_x = block_max_x.max(x2);
                            block_min_y = block_min_y.min(frame_y1);
                            block_max_y = block_max_y.max(frame_y2);
                        }
                    }
                }
                15 => stack.push(BlockStackEntry::Opt { items: Vec::new() }),
                16 => {
                    if let Some(BlockStackEntry::Opt { items }) = stack.pop() {
                        if let (Some((x1, x2, _min_left)), Some((y0, y1))) = (
                            frame_x_from_item_ids(
                                &items,
                                &nodes_by_id,
                                &edges_by_id,
                                &msg_endpoints,
                            ),
                            items
                                .iter()
                                .filter_map(|id| {
                                    item_y_range(id, &nodes_by_id, &edges_by_id, &msg_endpoints)
                                })
                                .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1))),
                        ) {
                            let frame_y1 = y0 - 79.0;
                            let frame_y2 = y1 + 10.0;
                            block_min_x = block_min_x.min(x1);
                            block_max_x = block_max_x.max(x2);
                            block_min_y = block_min_y.min(frame_y1);
                            block_max_y = block_max_y.max(frame_y2);
                        }
                    }
                }
                30 => stack.push(BlockStackEntry::Break { items: Vec::new() }),
                31 => {
                    if let Some(BlockStackEntry::Break { items }) = stack.pop() {
                        if let (Some((x1, x2, _min_left)), Some((y0, y1))) = (
                            frame_x_from_item_ids(
                                &items,
                                &nodes_by_id,
                                &edges_by_id,
                                &msg_endpoints,
                            ),
                            items
                                .iter()
                                .filter_map(|id| {
                                    item_y_range(id, &nodes_by_id, &edges_by_id, &msg_endpoints)
                                })
                                .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1))),
                        ) {
                            let frame_y1 = y0 - 93.0;
                            let frame_y2 = y1 + 10.0;
                            block_min_x = block_min_x.min(x1);
                            block_max_x = block_max_x.max(x2);
                            block_min_y = block_min_y.min(frame_y1);
                            block_max_y = block_max_y.max(frame_y2);
                        }
                    }
                }
                12 => stack.push(BlockStackEntry::Alt {
                    sections: vec![Vec::new()],
                }),
                13 => {
                    if let Some(BlockStackEntry::Alt { sections }) = stack.last_mut() {
                        sections.push(Vec::new());
                    }
                }
                14 => {
                    if let Some(BlockStackEntry::Alt { sections }) = stack.pop() {
                        let items: Vec<String> = sections.into_iter().flatten().collect();
                        if let (Some((x1, x2, _min_left)), Some((y0, y1))) = (
                            frame_x_from_item_ids(
                                &items,
                                &nodes_by_id,
                                &edges_by_id,
                                &msg_endpoints,
                            ),
                            items
                                .iter()
                                .filter_map(|id| {
                                    item_y_range(id, &nodes_by_id, &edges_by_id, &msg_endpoints)
                                })
                                .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1))),
                        ) {
                            let frame_y1 = y0 - 79.0;
                            let frame_y2 = y1 + 10.0;
                            block_min_x = block_min_x.min(x1);
                            block_max_x = block_max_x.max(x2);
                            block_min_y = block_min_y.min(frame_y1);
                            block_max_y = block_max_y.max(frame_y2);
                        }
                    }
                }
                19 | 32 => stack.push(BlockStackEntry::Par {
                    sections: vec![Vec::new()],
                }),
                20 => {
                    if let Some(BlockStackEntry::Par { sections }) = stack.last_mut() {
                        sections.push(Vec::new());
                    }
                }
                21 => {
                    if let Some(BlockStackEntry::Par { sections }) = stack.pop() {
                        let items: Vec<String> = sections.into_iter().flatten().collect();
                        if let (Some((x1, x2, _min_left)), Some((y0, y1))) = (
                            frame_x_from_item_ids(
                                &items,
                                &nodes_by_id,
                                &edges_by_id,
                                &msg_endpoints,
                            ),
                            items
                                .iter()
                                .filter_map(|id| {
                                    item_y_range(id, &nodes_by_id, &edges_by_id, &msg_endpoints)
                                })
                                .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1))),
                        ) {
                            let frame_y1 = y0 - 79.0;
                            let frame_y2 = y1 + 10.0;
                            block_min_x = block_min_x.min(x1);
                            block_max_x = block_max_x.max(x2);
                            block_min_y = block_min_y.min(frame_y1);
                            block_max_y = block_max_y.max(frame_y2);
                        }
                    }
                }
                27 => stack.push(BlockStackEntry::Critical {
                    sections: vec![Vec::new()],
                }),
                28 => {
                    if let Some(BlockStackEntry::Critical { sections }) = stack.last_mut() {
                        sections.push(Vec::new());
                    }
                }
                29 => {
                    if let Some(BlockStackEntry::Critical { sections }) = stack.pop() {
                        let section_count = sections.len();
                        let items: Vec<String> = sections.into_iter().flatten().collect();
                        if let (Some((mut x1, x2, min_left)), Some((y0, y1))) = (
                            frame_x_from_item_ids(
                                &items,
                                &nodes_by_id,
                                &edges_by_id,
                                &msg_endpoints,
                            ),
                            items
                                .iter()
                                .filter_map(|id| {
                                    item_y_range(id, &nodes_by_id, &edges_by_id, &msg_endpoints)
                                })
                                .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1))),
                        ) {
                            if min_left.is_finite() && !items.is_empty() && section_count > 1 {
                                x1 = x1.min(min_left - 9.0);
                            }
                            let frame_y1 = y0 - 79.0;
                            let frame_y2 = y1 + 10.0;
                            block_min_x = block_min_x.min(x1);
                            block_max_x = block_max_x.max(x2);
                            block_min_y = block_min_y.min(frame_y1);
                            block_max_y = block_max_y.max(frame_y2);
                        }
                    }
                }
                2 => {
                    for entry in stack.iter_mut() {
                        match entry {
                            BlockStackEntry::Alt { sections }
                            | BlockStackEntry::Par { sections }
                            | BlockStackEntry::Critical { sections } => {
                                if let Some(cur) = sections.last_mut() {
                                    cur.push(msg_id.clone());
                                }
                            }
                            BlockStackEntry::Loop { items }
                            | BlockStackEntry::Opt { items }
                            | BlockStackEntry::Break { items } => {
                                items.push(msg_id.clone());
                            }
                        }
                    }
                }
                _ => {
                    if msg.from.is_some() && msg.to.is_some() {
                        for entry in stack.iter_mut() {
                            match entry {
                                BlockStackEntry::Alt { sections }
                                | BlockStackEntry::Par { sections }
                                | BlockStackEntry::Critical { sections } => {
                                    if let Some(cur) = sections.last_mut() {
                                        cur.push(msg_id.clone());
                                    }
                                }
                                BlockStackEntry::Loop { items }
                                | BlockStackEntry::Opt { items }
                                | BlockStackEntry::Break { items } => {
                                    items.push(msg_id.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        if block_min_x.is_finite() && block_min_y.is_finite() {
            Some((block_min_x, block_min_y, block_max_x, block_max_y))
        } else {
            None
        }
    };

    let mut content_min_x = f64::INFINITY;
    let mut content_min_y = f64::INFINITY;
    let mut content_max_x = f64::NEG_INFINITY;
    let mut content_max_y = f64::NEG_INFINITY;
    for n in &nodes {
        let left = n.x - n.width / 2.0;
        let right = n.x + n.width / 2.0;
        let top = n.y - n.height / 2.0;
        let bottom = n.y + n.height / 2.0;
        content_min_x = content_min_x.min(left);
        content_min_y = content_min_y.min(top);
        content_max_x = content_max_x.max(right);
        content_max_y = content_max_y.max(bottom);
    }
    if !content_min_x.is_finite() || !content_min_y.is_finite() {
        content_min_x = 0.0;
        content_min_y = 0.0;
        content_max_x = actor_width_min.max(1.0);
        content_max_y = (bottom_box_top_y + actor_height).max(1.0);
    }

    if let Some((min_x, min_y, max_x, max_y)) = block_bounds {
        content_min_x = content_min_x.min(min_x);
        content_min_y = content_min_y.min(min_y);
        content_max_x = content_max_x.max(max_x);
        content_max_y = content_max_y.max(max_y);
    }

    // Mermaid (11.12.2) expands the viewBox vertically when a sequence title is present.
    // See `sequenceRenderer.ts`: `extraVertForTitle = title ? 40 : 0`.
    let extra_vert_for_title = if model.title.is_some() { 40.0 } else { 0.0 };

    let bounds = Some(Bounds {
        min_x: content_min_x - diagram_margin_x,
        min_y: content_min_y - diagram_margin_y - extra_vert_for_title,
        max_x: content_max_x + diagram_margin_x,
        max_y: content_max_y + diagram_margin_y + bottom_margin_adj,
    });

    Ok(SequenceDiagramLayout {
        nodes,
        edges,
        clusters,
        bounds,
    })
}

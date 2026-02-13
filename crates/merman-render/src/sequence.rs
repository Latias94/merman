#![allow(clippy::too_many_arguments)]

use crate::model::{
    Bounds, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint, SequenceDiagramLayout,
};
use crate::text::{
    TextMeasurer, TextStyle, WrapMode, split_html_br_lines, wrap_label_like_mermaid_lines,
    wrap_label_like_mermaid_lines_floored_bbox,
};
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
struct SequenceActor {
    #[allow(dead_code)]
    name: String,
    description: String,
    #[serde(rename = "type")]
    actor_type: String,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    wrap: bool,
    activate: bool,
    #[serde(default)]
    placement: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
struct SequenceBox {
    #[serde(rename = "actorKeys")]
    actor_keys: Vec<String>,
    #[allow(dead_code)]
    fill: String,
    name: Option<String>,
    #[allow(dead_code)]
    wrap: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct SequenceModel {
    #[serde(rename = "actorOrder")]
    actor_order: Vec<String>,
    actors: std::collections::BTreeMap<String, SequenceActor>,
    #[serde(default)]
    boxes: Vec<SequenceBox>,
    messages: Vec<SequenceMessage>,
    title: Option<String>,
    #[serde(rename = "createdActors", default)]
    created_actors: std::collections::BTreeMap<String, usize>,
    #[serde(rename = "destroyedActors", default)]
    destroyed_actors: std::collections::BTreeMap<String, usize>,
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

fn measure_svg_like_with_html_br(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
) -> (f64, f64) {
    let lines = split_html_br_lines(text);
    let default_line_height = (style.font_size.max(1.0) * 1.1).max(1.0);
    if lines.len() <= 1 {
        let metrics = measurer.measure_wrapped(text, style, None, WrapMode::SvgLikeSingleRun);
        let h = if metrics.height > 0.0 {
            metrics.height
        } else {
            default_line_height
        };
        return (metrics.width.max(0.0), h.max(0.0));
    }
    let mut max_w: f64 = 0.0;
    let mut line_h: f64 = 0.0;
    for line in &lines {
        let metrics = measurer.measure_wrapped(line, style, None, WrapMode::SvgLikeSingleRun);
        max_w = max_w.max(metrics.width.max(0.0));
        let h = if metrics.height > 0.0 {
            metrics.height
        } else {
            default_line_height
        };
        line_h = line_h.max(h.max(0.0));
    }
    (
        max_w,
        (line_h * lines.len() as f64).max(default_line_height),
    )
}

fn sequence_actor_visual_height(
    actor_type: &str,
    base_width: f64,
    base_height: f64,
    label_box_height: f64,
) -> f64 {
    match actor_type {
        // Mermaid (11.12.2) derives these from the actor-type glyph bbox + label box height.
        // These heights are used by the footer actor rendering and affect the final SVG viewBox.
        "boundary" => (60.0 + label_box_height).max(1.0),
        // Mermaid's database actor updates the actor height after the top render.
        // The footer render uses that updated height: ≈ width/4 + labelBoxHeight.
        "database" => ((base_width / 4.0) + label_box_height).max(1.0),
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
        "actor" | "boundary" => 80.0,
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
    let model: SequenceModel = crate::json::from_value_ref(semantic)?;

    let seq_cfg = effective_config.get("sequence").unwrap_or(&Value::Null);
    let diagram_margin_x = config_f64(seq_cfg, &["diagramMarginX"]).unwrap_or(50.0);
    let diagram_margin_y = config_f64(seq_cfg, &["diagramMarginY"]).unwrap_or(10.0);
    let bottom_margin_adj = config_f64(seq_cfg, &["bottomMarginAdj"]).unwrap_or(1.0);
    let box_margin = config_f64(seq_cfg, &["boxMargin"]).unwrap_or(10.0);
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

    let has_boxes = !model.boxes.is_empty();
    let has_box_titles = model
        .boxes
        .iter()
        .any(|b| b.name.as_deref().is_some_and(|s| !s.trim().is_empty()));

    // Mermaid uses `utils.calculateTextDimensions(...).height` for box titles and stores the max
    // across boxes in `box.textMaxHeight` (used for bumping actor `starty` when any title exists).
    //
    // In Mermaid 11.12.2 with 16px fonts, this height comes out as 17px (not the larger SVG
    // `getBBox()` height used elsewhere). Keep this model-level constant to match upstream DOM.
    fn mermaid_text_dimensions_height_px(font_size: f64) -> f64 {
        // 16px -> 17px in upstream.
        (font_size.max(1.0) * (17.0 / 16.0)).max(1.0)
    }

    let max_box_title_height = if has_box_titles {
        let line_h = mermaid_text_dimensions_height_px(message_font_size);
        model
            .boxes
            .iter()
            .filter_map(|b| b.name.as_deref())
            .map(|s| split_html_br_lines(s).len().max(1) as f64 * line_h)
            .fold(0.0, f64::max)
    } else {
        0.0
    };

    if model.actor_order.is_empty() {
        return Err(Error::InvalidModel {
            message: "sequence model has no actorOrder".to_string(),
        });
    }

    // Measure participant boxes.
    let mut actor_widths: Vec<f64> = Vec::with_capacity(model.actor_order.len());
    let mut actor_base_heights: Vec<f64> = Vec::with_capacity(model.actor_order.len());
    for id in &model.actor_order {
        let a = model.actors.get(id).ok_or_else(|| Error::InvalidModel {
            message: format!("missing actor {id}"),
        })?;
        if a.wrap {
            // Upstream wraps actor descriptions to `conf.width - 2*wrapPadding` and clamps the
            // actor box width to `conf.width`.
            let wrap_w = (actor_width_min - 2.0 * wrap_padding).max(1.0);
            let wrapped_lines =
                wrap_label_like_mermaid_lines(&a.description, measurer, &actor_text_style, wrap_w);
            let line_count = wrapped_lines.len().max(1) as f64;
            let text_h = mermaid_text_dimensions_height_px(actor_font_size) * line_count;
            actor_base_heights.push(actor_height.max(text_h).max(1.0));
            actor_widths.push(actor_width_min.max(1.0));
        } else {
            let (w0, _h0) =
                measure_svg_like_with_html_br(measurer, &a.description, &actor_text_style);
            let w = (w0 + 2.0 * wrap_padding).max(actor_width_min);
            actor_base_heights.push(actor_height.max(1.0));
            actor_widths.push(w.max(1.0));
        }
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

        let measured_text = if msg.wrap {
            // Upstream uses `wrapLabel(message, conf.width - 2*wrapPadding, ...)` when computing
            // max per-actor message widths for spacing.
            let wrap_w = (actor_width_min - 2.0 * wrap_padding).max(1.0);
            let lines = wrap_label_like_mermaid_lines(text, measurer, style, wrap_w);
            lines.join("<br>")
        } else {
            text.to_string()
        };
        let (w0, _h0) = measure_svg_like_with_html_br(measurer, &measured_text, style);
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

    // Mermaid's `calculateActorMargins(...)` computes per-box `box.margin` based on total actor
    // widths/margins and the box title width. For totalWidth, Mermaid only counts `actor.margin`
    // if it was set (actors without messages have `margin === undefined` until render-time).
    let mut box_margins: Vec<f64> = vec![box_text_margin; model.boxes.len()];
    for (box_idx, b) in model.boxes.iter().enumerate() {
        let mut total_width = 0.0;
        for actor_key in &b.actor_keys {
            let Some(&i) = actor_index.get(actor_key.as_str()) else {
                continue;
            };
            let actor_margin_for_box = if actor_to_message_width[i] > 0.0 {
                actor_margins[i]
            } else {
                0.0
            };
            total_width += actor_widths[i] + actor_margin_for_box;
        }

        total_width += box_margin * 8.0;
        total_width -= 2.0 * box_text_margin;

        let Some(name) = b.name.as_deref().filter(|s| !s.trim().is_empty()) else {
            continue;
        };

        let (text_w, _text_h) = measure_svg_like_with_html_br(measurer, name, &msg_text_style);
        let min_width = total_width.max(text_w + 2.0 * wrap_padding);
        if total_width < min_width {
            box_margins[box_idx] += (min_width - total_width) / 2.0;
        }
    }

    // Actors start lower when boxes exist, to make room for box headers.
    let mut actor_top_offset_y = 0.0;
    if has_boxes {
        actor_top_offset_y += box_margin;
        if has_box_titles {
            actor_top_offset_y += max_box_title_height;
        }
    }

    // Assign each actor to at most one box (Mermaid's db assigns a single `actor.box` reference).
    let mut actor_box: Vec<Option<usize>> = vec![None; model.actor_order.len()];
    for (box_idx, b) in model.boxes.iter().enumerate() {
        for actor_key in &b.actor_keys {
            let Some(&i) = actor_index.get(actor_key.as_str()) else {
                continue;
            };
            actor_box[i] = Some(box_idx);
        }
    }

    let mut actor_left_x: Vec<f64> = Vec::with_capacity(model.actor_order.len());
    let mut prev_width = 0.0;
    let mut prev_margin = 0.0;
    let mut prev_box: Option<usize> = None;
    for i in 0..model.actor_order.len() {
        let w = actor_widths[i];
        let cur_box = actor_box[i];

        // end of box
        if prev_box.is_some() && prev_box != cur_box {
            if let Some(prev) = prev_box {
                prev_margin += box_margin + box_margins[prev];
            }
        }

        // new box
        if cur_box.is_some() && cur_box != prev_box {
            if let Some(bi) = cur_box {
                prev_margin += box_margins[bi];
            }
        }

        // Mermaid widens the margin before a created actor by `actor.width / 2`.
        if model.created_actors.contains_key(&model.actor_order[i]) {
            prev_margin += w / 2.0;
        }
        let x = prev_width + prev_margin;
        actor_left_x.push(x);
        prev_width += w + prev_margin;
        prev_margin = actor_margins[i];
        prev_box = cur_box;
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
    // The bottom boxes start after all messages are placed. Created actors will have their `y`
    // adjusted later once we know the creation message position.
    let mut max_actor_visual_height: f64 = 0.0;
    for (idx, id) in model.actor_order.iter().enumerate() {
        let w = actor_widths[idx];
        let cx = actor_centers_x[idx];
        let base_h = actor_base_heights[idx];
        let actor_type = model
            .actors
            .get(id)
            .map(|a| a.actor_type.as_str())
            .unwrap_or("participant");
        let visual_h = sequence_actor_visual_height(actor_type, w, base_h, label_box_height);
        max_actor_visual_height = max_actor_visual_height.max(visual_h.max(1.0));
        let top_y = actor_top_offset_y + visual_h / 2.0;
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
    // `adjustLoopHeightForWrap(...)` advances the Mermaid bounds cursor by:
    // - `preMargin` (either `boxMargin` or `boxMargin + boxTextMargin`)
    // - plus `heightAdjust`, where `heightAdjust` is:
    //   - `postMargin` when the block label is empty
    //   - `postMargin + max(labelTextHeight, labelBoxHeight)` when the label is present
    //
    // For the common 1-line label case, this reduces to:
    //   preMargin + postMargin + labelBoxHeight
    //
    // We model this as a base step and subtract `labelBoxHeight` for empty labels.
    let block_base_step = (2.0 * box_margin + box_text_margin + label_box_height).max(0.0);
    let block_base_step_empty = (block_base_step - label_box_height).max(0.0);
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

                    if raw_label.trim().is_empty() {
                        directive_steps.insert(start_id, block_base_step_empty);
                    } else if let Some(w) = block_frame_width(
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
                    if raw_label.trim().is_empty() {
                        directive_steps.insert(start_id, block_base_step_empty);
                    } else if let Some(w) = block_frame_width(
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
                    if raw_label.trim().is_empty() {
                        directive_steps.insert(start_id, block_base_step_empty);
                    } else if let Some(w) = block_frame_width(
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
                            let is_empty = raw.trim().is_empty();
                            if is_empty {
                                directive_steps.insert(id, block_base_step_empty);
                                continue;
                            }
                            let _ = idx;
                            let label = block_label_text(&raw);
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
                        for (id, raw) in section_directives {
                            let step = if raw.trim().is_empty() {
                                block_base_step_empty
                            } else {
                                block_base_step
                            };
                            directive_steps.insert(id, step);
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
                            let is_empty = raw.trim().is_empty();
                            if is_empty {
                                directive_steps.insert(id, block_base_step_empty);
                                continue;
                            }
                            let _ = idx;
                            let label = block_label_text(&raw);
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
                        for (id, raw) in section_directives {
                            let step = if raw.trim().is_empty() {
                                block_base_step_empty
                            } else {
                                block_base_step
                            };
                            directive_steps.insert(id, step);
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
                            let is_empty = raw.trim().is_empty();
                            if is_empty {
                                directive_steps.insert(id, block_base_step_empty);
                                continue;
                            }
                            let _ = idx;
                            let label = block_label_text(&raw);
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
                        for (id, raw) in section_directives {
                            let step = if raw.trim().is_empty() {
                                block_base_step_empty
                            } else {
                                block_base_step
                            };
                            directive_steps.insert(id, step);
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
        bounds: Option<merman_core::geom::Box2>,
    }

    impl RectOpen {
        fn include_min_max(&mut self, min_x: f64, max_x: f64, max_y: f64) {
            let r = merman_core::geom::Box2::from_min_max(min_x, self.top_y, max_x, max_y);
            if let Some(ref mut cur) = self.bounds {
                cur.union(r);
            } else {
                self.bounds = Some(r);
            }
        }
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

    let mut cursor_y = actor_top_offset_y + max_actor_visual_height + message_step;
    let mut rect_stack: Vec<RectOpen> = Vec::new();
    let activation_width = config_f64(seq_cfg, &["activationWidth"])
        .unwrap_or(10.0)
        .max(1.0);
    let mut activation_stacks: std::collections::BTreeMap<&str, Vec<f64>> =
        std::collections::BTreeMap::new();

    // Mermaid adjusts created/destroyed actors while processing messages:
    // - created actor: `starty = lineStartY - actor.height/2`
    // - destroyed actor: `stopy = lineStartY - actor.height/2`
    // It also bumps the cursor by `actor.height/2` to avoid overlaps.
    let mut created_actor_top_center_y: std::collections::BTreeMap<String, f64> =
        std::collections::BTreeMap::new();
    let mut destroyed_actor_bottom_top_y: std::collections::BTreeMap<String, f64> =
        std::collections::BTreeMap::new();

    let actor_visual_height_for_id = |actor_id: &str| -> f64 {
        let Some(idx) = actor_index.get(actor_id).copied() else {
            return actor_height.max(1.0);
        };
        let w = actor_widths.get(idx).copied().unwrap_or(actor_width_min);
        let base_h = actor_base_heights.get(idx).copied().unwrap_or(actor_height);
        model
            .actors
            .get(actor_id)
            .map(|a| a.actor_type.as_str())
            .map(|t| sequence_actor_visual_height(t, w, base_h, label_box_height))
            .unwrap_or(base_h.max(1.0))
    };
    let actor_is_type_width_limited = |actor_id: &str| -> bool {
        model
            .actors
            .get(actor_id)
            .map(|a| {
                matches!(
                    a.actor_type.as_str(),
                    "actor" | "control" | "entity" | "database"
                )
            })
            .unwrap_or(false)
    };

    for (msg_idx, msg) in model.messages.iter().enumerate() {
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
                    bounds: None,
                });
                cursor_y += rect_step_start;
                continue;
            }
            // rect end
            23 => {
                if let Some(open) = rect_stack.pop() {
                    let rect_left = open.bounds.map(|b| b.min_x()).unwrap_or_else(|| {
                        actor_centers_x
                            .iter()
                            .copied()
                            .fold(f64::INFINITY, f64::min)
                            - 11.0
                    });
                    let rect_right = open.bounds.map(|b| b.max_x()).unwrap_or_else(|| {
                        actor_centers_x
                            .iter()
                            .copied()
                            .fold(f64::NEG_INFINITY, f64::max)
                            + 11.0
                    });
                    let rect_bottom = open
                        .bounds
                        .map(|b| b.max_y() + 10.0)
                        .unwrap_or(open.top_y + 10.0);
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
                        parent.include_min_max(rect_left - 10.0, rect_right + 10.0, rect_bottom);
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
            let (mut note_x, mut note_w) = match placement {
                // leftOf
                0 => (fx - 25.0 - note_width_single, note_width_single),
                // rightOf
                1 => (fx + 25.0, note_width_single),
                // over
                _ => {
                    if (fx - tx).abs() < 0.0001 {
                        // Mermaid's `buildNoteModel(...)` widens "over self" notes when `wrap: true`:
                        //   noteModel.width = max(conf.width, fromActor.width)
                        //
                        // This is observable in upstream SVG baselines for participants with
                        // type-driven widths (e.g. `queue`), where the note box matches the actor
                        // width rather than the configured default `conf.width`.
                        let mut w = note_width_single;
                        if msg.wrap {
                            w = w.max(actor_widths.get(fi).copied().unwrap_or(note_width_single));
                        }
                        (fx - (w / 2.0), w)
                    } else {
                        let left = fx.min(tx) - 25.0;
                        let right = fx.max(tx) + 25.0;
                        let w = (right - left).max(note_width_single);
                        (left, w)
                    }
                }
            };

            let text = msg.message.as_str().unwrap_or_default();
            let (text_w, h) = if msg.wrap {
                // Mermaid Sequence notes are wrapped via `wrapLabel(...)`, then measured via SVG
                // bbox probes (not HTML wrapping). Model this by producing wrapped `<br/>` lines
                // and then measuring them.
                //
                // Important: Mermaid widens *leftOf* wrapped notes based on the initially wrapped
                // text width (+ margins) before re-wrapping to the final width. This affects the
                // final wrap width and thus the rendered line breaks.
                let w0 = {
                    let init_lines = wrap_label_like_mermaid_lines_floored_bbox(
                        text,
                        measurer,
                        &note_text_style,
                        note_width_single.max(1.0),
                    );
                    let init_wrapped = init_lines.join("<br/>");
                    let (w, _h) =
                        measure_svg_like_with_html_br(measurer, &init_wrapped, &note_text_style);
                    w.max(0.0)
                };

                if placement == 0 {
                    // Mermaid (LEFTOF + wrap): `noteModel.width = max(conf.width, textWidth + 2*noteMargin)`.
                    // Our note padding total is `2*noteMargin`/`2*wrapPadding` in the default config.
                    note_w = note_w.max((w0 + note_text_pad_total).round().max(1.0));
                    note_x = fx - 25.0 - note_w;
                }

                let wrap_w = (note_w - note_text_pad_total).max(1.0);
                let lines = wrap_label_like_mermaid_lines_floored_bbox(
                    text,
                    measurer,
                    &note_text_style,
                    wrap_w,
                );
                let wrapped = lines.join("<br/>");
                let (w, h) = measure_svg_like_with_html_br(measurer, &wrapped, &note_text_style);
                (w.max(0.0), h.max(0.0))
            } else {
                measure_svg_like_with_html_br(measurer, text, &note_text_style)
            };

            // Mermaid's `buildNoteModel(...)` widens the note box when the text would overflow the
            // configured default width. This is observable in strict SVG XML baselines when the
            // note contains literal `<br ...>` markup that is *not* treated as a line break.
            let padded_w = (text_w + note_text_pad_total).round().max(1.0);
            if !msg.wrap {
                match placement {
                    // leftOf / rightOf notes clamp width to fit label text.
                    0 | 1 => {
                        note_w = note_w.max(padded_w);
                    }
                    // over: only clamp when the note is over a single actor (`from == to`).
                    _ => {
                        if (fx - tx).abs() < 0.0001 {
                            note_w = note_w.max(padded_w);
                        }
                    }
                }
            }
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
                open.include_min_max(note_x - 10.0, note_x + note_w + 10.0, note_y + note_h);
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

        if !is_self {
            // Mermaid adjusts creating/destroying messages so arrowheads land outside the actor box.
            const ACTOR_TYPE_WIDTH_HALF: f64 = 18.0;
            if model
                .created_actors
                .get(to)
                .is_some_and(|&idx| idx == msg_idx)
            {
                let adjustment = if actor_is_type_width_limited(to) {
                    ACTOR_TYPE_WIDTH_HALF + 3.0
                } else {
                    actor_widths[ti] / 2.0 + 3.0
                };
                if to_x < from_x {
                    stopx += adjustment;
                } else {
                    stopx -= adjustment;
                }
            } else if model
                .destroyed_actors
                .get(from)
                .is_some_and(|&idx| idx == msg_idx)
            {
                let adjustment = if actor_is_type_width_limited(from) {
                    ACTOR_TYPE_WIDTH_HALF
                } else {
                    actor_widths[fi] / 2.0
                };
                if from_x < to_x {
                    startx += adjustment;
                } else {
                    startx -= adjustment;
                }
            } else if model
                .destroyed_actors
                .get(to)
                .is_some_and(|&idx| idx == msg_idx)
            {
                let adjustment = if actor_is_type_width_limited(to) {
                    ACTOR_TYPE_WIDTH_HALF + 3.0
                } else {
                    actor_widths[ti] / 2.0 + 3.0
                };
                if to_x < from_x {
                    stopx += adjustment;
                } else {
                    stopx -= adjustment;
                }
            }
        }

        let text = msg.message.as_str().unwrap_or_default();
        let bounded_width = (startx - stopx).abs().max(0.0);
        let wrapped_text = if !text.is_empty() && msg.wrap {
            // Upstream wraps message labels to `max(boundedWidth + 2*wrapPadding, conf.width)`.
            // Note: a small extra margin helps keep wrap breakpoints aligned with upstream SVG
            // baselines for long sentences under our vendored metrics.
            let wrap_w = (bounded_width + 3.0 * wrap_padding)
                .max(actor_width_min)
                .max(1.0);
            let lines =
                wrap_label_like_mermaid_lines_floored_bbox(text, measurer, &msg_text_style, wrap_w);
            Some(lines.join("<br>"))
        } else {
            None
        };
        let effective_text = wrapped_text.as_deref().unwrap_or(text);

        let (line_y, label_base_y, cursor_step) = if effective_text.is_empty() {
            // Mermaid's `boundMessage(...)` uses the measured text bbox height. For empty labels
            // (trailing colon `Alice->Bob:`) the bbox height becomes 0, collapsing the extra
            // vertical offset and producing a much earlier message line.
            //
            // Our cursor model uses `message_step` (a typical 1-line height) as the baseline.
            // Shift the line up and only advance by `boxMargin` to match the upstream footer actor
            // placement and overall viewBox height.
            let line_y = cursor_y - (message_step - box_margin);
            (line_y, cursor_y, box_margin)
        } else {
            // Mermaid's `boundMessage(...)` uses `common.splitBreaks(message)` to derive a
            // `lines` count and adjusts the message line y-position and cursor increment by the
            // per-line height. This applies both to explicit `<br>` breaks and to `wrap: true`
            // labels (which are wrapped via `wrapLabel(...)` and stored with `<br/>` separators).
            let lines = split_html_br_lines(effective_text).len().max(1);
            // Mermaid's `calculateTextDimensions(...).height` is consistently ~2px smaller per
            // line than the rendered `drawText(...)` getBBox, so use a bbox-like per-line height
            // for the cursor math here.
            let bbox_line_h = (message_font_size + bottom_margin_adj).max(0.0);
            let extra = (lines.saturating_sub(1) as f64) * bbox_line_h;
            (cursor_y + extra, cursor_y, message_step + extra)
        };

        let x1 = startx;
        let x2 = stopx;

        let label = if effective_text.is_empty() {
            // Mermaid renders an (empty) message text node even when the label is empty (e.g.
            // trailing colon `Alice->Bob:`). Keep a placeholder label to preserve DOM structure.
            Some(LayoutLabel {
                x: ((x1 + x2) / 2.0).round(),
                y: (label_base_y - msg_label_offset).round(),
                width: 1.0,
                height: message_font_size.max(1.0),
            })
        } else {
            let (w, h) = measure_svg_like_with_html_br(measurer, effective_text, &msg_text_style);
            Some(LayoutLabel {
                x: ((x1 + x2) / 2.0).round(),
                y: (label_base_y - msg_label_offset).round(),
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
            points: vec![
                LayoutPoint { x: x1, y: line_y },
                LayoutPoint { x: x2, y: line_y },
            ],
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
            open.include_min_max(lx, rx, line_y);
        }

        cursor_y += cursor_step;
        if is_self {
            // Mermaid adds extra vertical space for self-messages to accommodate the loop curve.
            cursor_y += 30.0;
        }

        // Apply Mermaid's created/destroyed actor y adjustments and spacing bumps.
        if model
            .created_actors
            .get(to)
            .is_some_and(|&idx| idx == msg_idx)
        {
            let h = actor_visual_height_for_id(to);
            created_actor_top_center_y.insert(to.to_string(), line_y);
            cursor_y += h / 2.0;
        } else if model
            .destroyed_actors
            .get(from)
            .is_some_and(|&idx| idx == msg_idx)
        {
            let h = actor_visual_height_for_id(from);
            destroyed_actor_bottom_top_y.insert(from.to_string(), line_y - h / 2.0);
            cursor_y += h / 2.0;
        } else if model
            .destroyed_actors
            .get(to)
            .is_some_and(|&idx| idx == msg_idx)
        {
            let h = actor_visual_height_for_id(to);
            destroyed_actor_bottom_top_y.insert(to.to_string(), line_y - h / 2.0);
            cursor_y += h / 2.0;
        }
    }

    let bottom_margin = message_margin - message_font_size + bottom_margin_adj;
    let bottom_box_top_y = (cursor_y - message_step) + bottom_margin;

    // Apply created-actor `starty` overrides now that we know the creation message y.
    for n in nodes.iter_mut() {
        let Some(actor_id) = n.id.strip_prefix("actor-top-") else {
            continue;
        };
        if let Some(y) = created_actor_top_center_y.get(actor_id).copied() {
            n.y = y;
        }
    }

    for (idx, id) in model.actor_order.iter().enumerate() {
        let w = actor_widths[idx];
        let cx = actor_centers_x[idx];
        let base_h = actor_base_heights[idx];
        let actor_type = model
            .actors
            .get(id)
            .map(|a| a.actor_type.as_str())
            .unwrap_or("participant");
        let visual_h = sequence_actor_visual_height(actor_type, w, base_h, label_box_height);
        let bottom_top_y = destroyed_actor_bottom_top_y
            .get(id)
            .copied()
            .unwrap_or(bottom_box_top_y);
        nodes.push(LayoutNode {
            id: format!("actor-bottom-{id}"),
            x: cx,
            y: bottom_top_y + visual_h / 2.0,
            width: w,
            height: visual_h,
            is_cluster: false,
        });

        let top_center_y = created_actor_top_center_y
            .get(id)
            .copied()
            .unwrap_or(actor_top_offset_y + visual_h / 2.0);
        let top_left_y = top_center_y - visual_h / 2.0;
        let lifeline_start_y =
            top_left_y + sequence_actor_lifeline_start_y(actor_type, base_h, box_text_margin);

        edges.push(LayoutEdge {
            id: format!("lifeline-{id}"),
            from: format!("actor-top-{id}"),
            to: format!("actor-bottom-{id}"),
            from_cluster: None,
            to_cluster: None,
            points: vec![
                LayoutPoint {
                    x: cx,
                    y: lifeline_start_y,
                },
                LayoutPoint {
                    x: cx,
                    y: bottom_top_y,
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
    let mut content_max_x = f64::NEG_INFINITY;
    let mut content_max_y = f64::NEG_INFINITY;
    for n in &nodes {
        let left = n.x - n.width / 2.0;
        let right = n.x + n.width / 2.0;
        let bottom = n.y + n.height / 2.0;
        content_min_x = content_min_x.min(left);
        content_max_x = content_max_x.max(right);
        content_max_y = content_max_y.max(bottom);
    }
    if !content_min_x.is_finite() {
        content_min_x = 0.0;
        content_max_x = actor_width_min.max(1.0);
        content_max_y = (bottom_box_top_y + actor_height).max(1.0);
    }

    if let Some((min_x, _min_y, max_x, max_y)) = block_bounds {
        content_min_x = content_min_x.min(min_x);
        content_max_x = content_max_x.max(max_x);
        content_max_y = content_max_y.max(max_y);
    }

    // Mermaid (11.12.2) expands the viewBox vertically when a sequence title is present.
    // See `sequenceRenderer.ts`: `extraVertForTitle = title ? 40 : 0`.
    let extra_vert_for_title = if model.title.is_some() { 40.0 } else { 0.0 };

    // Mermaid's sequence renderer sets the viewBox y origin to `-(diagramMarginY + extraVertForTitle)`
    // regardless of diagram contents.
    let vb_min_y = -(diagram_margin_y + extra_vert_for_title);

    // Mermaid's sequence renderer uses a bounds box with `starty = 0` and computes `height` from
    // `stopy - starty`. Our headless layout models message spacing in content coordinates, but for
    // viewBox parity we must follow the upstream formula.
    //
    // When boxes exist, Mermaid's bounds logic ends up extending the vertical bounds by `boxMargin`
    // (diagramMarginY covers the remaining box padding), so include it here.
    let mut bounds_box_stopy = (content_max_y + bottom_margin_adj).max(0.0);
    if has_boxes {
        bounds_box_stopy += box_margin;
    }

    // Mermaid's bounds box includes the per-box inner margins (`box.margin`) when boxes exist.
    // Approximate this by extending actor bounds by their enclosing box margin.
    let mut bounds_box_startx = content_min_x;
    let mut bounds_box_stopx = content_max_x;
    for i in 0..model.actor_order.len() {
        let left = actor_left_x[i];
        let right = left + actor_widths[i];
        if let Some(bi) = actor_box[i] {
            let m = box_margins[bi];
            bounds_box_startx = bounds_box_startx.min(left - m);
            bounds_box_stopx = bounds_box_stopx.max(right + m);
        } else {
            bounds_box_startx = bounds_box_startx.min(left);
            bounds_box_stopx = bounds_box_stopx.max(right);
        }
    }

    // Mermaid's self-message bounds insert expands horizontally by `dx = max(textWidth/2, conf.width/2)`,
    // where `conf.width` is the configured actor width (150 by default). This can increase `box.stopx`
    // by ~1px due to `from_x + 1` rounding behavior in message geometry, affecting viewBox width.
    for msg in &model.messages {
        let (Some(from), Some(to)) = (msg.from.as_deref(), msg.to.as_deref()) else {
            continue;
        };
        if from != to {
            continue;
        }
        // Notes can use `from==to` for `rightOf`/`leftOf`; ignore them here.
        if msg.message_type == 2 {
            continue;
        }
        let Some(&i) = actor_index.get(from) else {
            continue;
        };
        let center_x = actor_centers_x[i] + 1.0;
        let text = msg.message.as_str().unwrap_or_default();
        let (text_w, _text_h) = if text.is_empty() {
            (1.0, 1.0)
        } else {
            measure_svg_like_with_html_br(measurer, text, &msg_text_style)
        };
        let dx = (text_w.max(1.0) / 2.0).max(actor_width_min / 2.0);
        bounds_box_startx = bounds_box_startx.min(center_x - dx);
        bounds_box_stopx = bounds_box_stopx.max(center_x + dx);
    }

    let bounds = Some(Bounds {
        min_x: bounds_box_startx - diagram_margin_x,
        min_y: vb_min_y,
        max_x: bounds_box_stopx + diagram_margin_x,
        max_y: bounds_box_stopy + diagram_margin_y,
    });

    Ok(SequenceDiagramLayout {
        nodes,
        edges,
        clusters,
        bounds,
    })
}

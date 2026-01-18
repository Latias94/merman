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
        let metrics = measurer.measure_wrapped(text, style, None, WrapMode::SvgLike);
        return (
            metrics.width.max(0.0),
            metrics.height.max(default_line_height),
        );
    }
    let mut max_w: f64 = 0.0;
    let mut line_h: f64 = 0.0;
    for line in &lines {
        let metrics = measurer.measure_wrapped(line, style, None, WrapMode::SvgLike);
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

    let message_font_family = config_string(seq_cfg, &["messageFontFamily"])
        .or_else(|| config_string(effective_config, &["fontFamily"]));
    let message_font_size = config_f64(seq_cfg, &["messageFontSize"])
        .or_else(|| config_f64(effective_config, &["fontSize"]))
        .unwrap_or(16.0);
    let message_font_weight = config_string(seq_cfg, &["messageFontWeight"])
        .or_else(|| config_string(effective_config, &["fontWeight"]));

    let actor_font_family = config_string(seq_cfg, &["actorFontFamily"])
        .or_else(|| config_string(effective_config, &["fontFamily"]));
    let actor_font_size = config_f64(seq_cfg, &["actorFontSize"]).unwrap_or(14.0);
    let actor_font_weight = config_string(seq_cfg, &["actorFontWeight"])
        .or_else(|| config_string(effective_config, &["fontWeight"]));

    // Mermaid measures SVG text widths using actual font metrics (default: trebuchet).
    // Our deterministic headless measurer underestimates some glyph widths for this font, which
    // makes participant spacing too tight and cascades into block widths (e.g. `rect`) and note
    // placements. Apply a small, sequence-specific correction factor for message text widths.
    let message_width_scale = 1.316;

    let actor_text_style = TextStyle {
        font_family: actor_font_family,
        font_size: actor_font_size,
        font_weight: actor_font_weight,
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

    // Determine center-to-center gaps between adjacent actors, accounting for message label widths.
    let mut gaps: Vec<f64> = Vec::with_capacity(model.actor_order.len().saturating_sub(1));
    for i in 0..model.actor_order.len().saturating_sub(1) {
        let w0 = actor_widths[i];
        let w1 = actor_widths[i + 1];
        let base_gap = (w0 / 2.0) + (w1 / 2.0) + actor_margin;

        let left = model.actor_order[i].as_str();
        let right = model.actor_order[i + 1].as_str();

        let mut max_label_w: f64 = 0.0;
        for msg in &model.messages {
            let (Some(from), Some(to)) = (msg.from.as_deref(), msg.to.as_deref()) else {
                continue;
            };
            if msg.message_type == 2 {
                // Notes do not affect participant spacing in Mermaid.
                continue;
            }
            let touches_pair = (from == left && to == right) || (from == right && to == left);
            if !touches_pair {
                continue;
            }
            let text = msg.message.as_str().unwrap_or_default();
            if text.is_empty() {
                continue;
            }
            let (w, _h) = measure_svg_like_with_html_br(measurer, text, &msg_text_style);
            max_label_w = max_label_w.max(w * message_width_scale);
        }

        let required_gap = (max_label_w + 2.0 * wrap_padding).max(base_gap).round();
        gaps.push(required_gap);
    }

    // Compute actor centers (top and bottom boxes share the same x).
    let mut actor_centers_x: Vec<f64> = Vec::with_capacity(model.actor_order.len());
    let left_edge = 0.0;
    actor_centers_x.push(left_edge + actor_widths[0] / 2.0);
    for i in 1..model.actor_order.len() {
        let prev = actor_centers_x[i - 1];
        let gap = gaps
            .get(i - 1)
            .copied()
            .unwrap_or(actor_width_min + actor_margin);
        actor_centers_x.push(prev + gap);
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
    let mut actor_index: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (i, id) in model.actor_order.iter().enumerate() {
        actor_index.insert(id.as_str(), i);
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
    let note_text_pad_total = message_font_size * 1.3375; // 16px -> 21.4px, yielding 39px total for 1 line.
    let note_top_offset = message_step - note_gap;

    let mut cursor_y = actor_height + message_step;
    let mut rect_stack: Vec<RectOpen> = Vec::new();

    for msg in &model.messages {
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
            let (_w, h) = measure_svg_like_with_html_br(measurer, text, &msg_text_style);
            let note_h = (h + note_text_pad_total).max(1.0);
            let note_y = cursor_y - note_top_offset;

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
        let sign = if to_x >= from_x { 1.0 } else { -1.0 };

        // These small offsets match Mermaid's default arrow rendering (marker-end).
        let x1 = from_x + sign * 1.0;
        let is_self = from == to;
        let x2 = if is_self {
            // Mermaid uses `startx === stopx` to render self-messages as a `<path>` (curved or
            // right-angled). Keep start/stop identical in layout so SVG can emit the correct DOM.
            x1
        } else {
            to_x - sign * 4.0
        };
        let y = cursor_y;

        let text = msg.message.as_str().unwrap_or_default();
        let label = if text.is_empty() {
            // Mermaid renders an (empty) message text node even when the label is empty (e.g.
            // trailing colon `Alice->Bob:`). Keep a placeholder label to preserve DOM structure.
            Some(LayoutLabel {
                x: (x1 + x2) / 2.0,
                y: y - msg_label_offset,
                width: 1.0,
                height: message_font_size.max(1.0),
            })
        } else {
            let (w, h) = measure_svg_like_with_html_br(measurer, text, &msg_text_style);
            Some(LayoutLabel {
                x: (x1 + x2) / 2.0,
                y: y - msg_label_offset,
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

    let bounds = Some(Bounds {
        min_x: content_min_x - diagram_margin_x,
        min_y: content_min_y - diagram_margin_y,
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

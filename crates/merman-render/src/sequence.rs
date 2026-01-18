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
        let metrics =
            measurer.measure_wrapped(&a.description, &actor_text_style, None, WrapMode::SvgLike);
        let w = (metrics.width + 2.0 * wrap_padding).max(actor_width_min);
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
            let touches_pair = (from == left && to == right) || (from == right && to == left);
            if !touches_pair {
                continue;
            }
            let text = msg.message.as_str().unwrap_or_default();
            if text.is_empty() {
                continue;
            }
            let metrics = measurer.measure_wrapped(text, &msg_text_style, None, WrapMode::SvgLike);
            max_label_w = max_label_w.max(metrics.width);
        }

        let required_gap = (max_label_w + 2.0 * wrap_padding).max(base_gap);
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

    // Messages are laid out below the top actor boxes.
    let mut msg_line_y = actor_height + message_step;
    let mut last_message_y = actor_height;

    let mut edges: Vec<LayoutEdge> = Vec::new();
    let mut nodes: Vec<LayoutNode> = Vec::new();
    let clusters: Vec<LayoutCluster> = Vec::new();

    // Actor boxes: Mermaid renders both a "top" and "bottom" actor box.
    // The bottom boxes start after all messages are placed.
    for (idx, id) in model.actor_order.iter().enumerate() {
        let w = actor_widths[idx];
        let cx = actor_centers_x[idx];
        let top_y = actor_height / 2.0;
        nodes.push(LayoutNode {
            id: format!("actor-top-{id}"),
            x: cx,
            y: top_y,
            width: w,
            height: actor_height,
            is_cluster: false,
        });
    }

    // Message edges.
    let mut actor_index: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (i, id) in model.actor_order.iter().enumerate() {
        actor_index.insert(id.as_str(), i);
    }

    for msg in &model.messages {
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
        let x2 = to_x - sign * 4.0;
        let y = msg_line_y;

        let text = msg.message.as_str().unwrap_or_default();
        let label = if text.is_empty() {
            None
        } else {
            let metrics = measurer.measure_wrapped(text, &msg_text_style, None, WrapMode::SvgLike);
            Some(LayoutLabel {
                x: (x1 + x2) / 2.0,
                y: y - msg_label_offset,
                width: metrics.width.max(1.0),
                height: metrics.height.max(1.0),
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

        last_message_y = y;
        msg_line_y += message_step;
    }

    let bottom_box_top_y =
        last_message_y + (message_margin - message_font_size + bottom_margin_adj);
    for (idx, id) in model.actor_order.iter().enumerate() {
        let w = actor_widths[idx];
        let cx = actor_centers_x[idx];
        nodes.push(LayoutNode {
            id: format!("actor-bottom-{id}"),
            x: cx,
            y: bottom_box_top_y + actor_height / 2.0,
            width: w,
            height: actor_height,
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
                    y: actor_height,
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

    let content_width = {
        let last = model.actor_order.len() - 1;
        let last_center = actor_centers_x[last];
        let last_w = actor_widths[last];
        (last_center + last_w / 2.0).max(1.0)
    };
    let content_height = (bottom_box_top_y + actor_height).max(1.0);

    let bounds = Bounds::from_points([
        (-diagram_margin_x, -diagram_margin_y),
        (
            content_width + diagram_margin_x,
            content_height + diagram_margin_y + bottom_margin_adj,
        ),
    ])
    .or_else(|| {
        Some(Bounds {
            min_x: -diagram_margin_x,
            min_y: -diagram_margin_y,
            max_x: content_width + diagram_margin_x,
            max_y: content_height + diagram_margin_y + bottom_margin_adj,
        })
    });

    Ok(SequenceDiagramLayout {
        nodes,
        edges,
        clusters,
        bounds,
    })
}

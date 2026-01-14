use crate::model::{Bounds, ErDiagramLayout, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
struct ErModel {
    pub direction: String,
    pub entities: HashMap<String, ErEntity>,
    #[serde(default)]
    pub relationships: Vec<ErRelationship>,
}

#[derive(Debug, Clone, Deserialize)]
struct ErEntity {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub alias: String,
    #[serde(default)]
    pub attributes: Vec<ErAttribute>,
}

#[derive(Debug, Clone, Deserialize)]
struct ErAttribute {
    #[serde(rename = "type")]
    pub ty: String,
    pub name: String,
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub comment: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ErRelationship {
    #[serde(rename = "entityA")]
    pub entity_a: String,
    #[serde(rename = "entityB")]
    pub entity_b: String,
    #[serde(rename = "roleA")]
    pub role_a: String,
    #[allow(dead_code)]
    #[serde(rename = "relSpec")]
    pub rel_spec: Value,
}

fn json_f64(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_i64().map(|n| n as f64))
        .or_else(|| v.as_u64().map(|n| n as f64))
}

fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    json_f64(cur)
}

fn config_string(cfg: &Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn normalize_dir(direction: &str) -> String {
    match direction.trim().to_uppercase().as_str() {
        "TB" | "TD" => "TB".to_string(),
        "BT" => "BT".to_string(),
        "LR" => "LR".to_string(),
        "RL" => "RL".to_string(),
        other => other.to_string(),
    }
}

fn rank_dir_from(direction: &str) -> RankDir {
    match normalize_dir(direction).as_str() {
        "TB" => RankDir::TB,
        "BT" => RankDir::BT,
        "LR" => RankDir::LR,
        "RL" => RankDir::RL,
        _ => RankDir::TB,
    }
}

fn parse_generic_types_like_mermaid(text: &str) -> String {
    // Mermaid `parseGenericTypes` turns `Foo~T~` into `Foo<T>` for display.
    let mut out = String::with_capacity(text.len());
    let mut it = text.split('~').peekable();
    let mut open = false;
    while let Some(part) = it.next() {
        out.push_str(part);
        if it.peek().is_none() {
            break;
        }
        if !open {
            out.push('<');
            open = true;
        } else {
            out.push('>');
            open = false;
        }
    }
    if open {
        out.push('>');
    }
    out
}

fn er_text_style(effective_config: &Value) -> TextStyle {
    let font_family = config_string(effective_config, &["fontFamily"]);
    let font_size = config_f64(effective_config, &["er", "fontSize"])
        .or_else(|| config_f64(effective_config, &["fontSize"]))
        .unwrap_or(12.0);
    TextStyle {
        font_family,
        font_size,
        font_weight: None,
    }
}

fn entity_box_dimensions(
    entity: &ErEntity,
    measurer: &dyn TextMeasurer,
    label_style: &TextStyle,
    attr_style: &TextStyle,
    effective_config: &Value,
) -> (f64, f64) {
    let padding = config_f64(effective_config, &["er", "entityPadding"]).unwrap_or(15.0);
    let min_w = config_f64(effective_config, &["er", "minEntityWidth"]).unwrap_or(100.0);
    let min_h = config_f64(effective_config, &["er", "minEntityHeight"]).unwrap_or(75.0);

    let height_padding = padding / 3.0;
    let width_padding = padding / 3.0;

    let label_text = if entity.alias.trim().is_empty() {
        entity.label.as_str()
    } else {
        entity.alias.as_str()
    };
    let label_metrics = measurer.measure_wrapped(label_text, label_style, None, WrapMode::SvgLike);

    let mut has_key = false;
    let mut has_comment = false;
    for a in &entity.attributes {
        if !a.keys.is_empty() {
            has_key = true;
        }
        if !a.comment.is_empty() {
            has_comment = true;
        }
    }

    let mut max_type_w: f64 = 0.0;
    let mut max_name_w: f64 = 0.0;
    let mut max_key_w: f64 = 0.0;
    let mut max_comment_w: f64 = 0.0;

    let mut cumulative_h = label_metrics.height + height_padding * 2.0;
    for (idx, a) in entity.attributes.iter().enumerate() {
        let _ = idx;
        let ty = parse_generic_types_like_mermaid(&a.ty);
        let m_type = measurer.measure_wrapped(&ty, attr_style, None, WrapMode::SvgLike);
        let m_name = measurer.measure_wrapped(&a.name, attr_style, None, WrapMode::SvgLike);
        max_type_w = max_type_w.max(m_type.width);
        max_name_w = max_name_w.max(m_name.width);
        let mut row_h = m_type.height.max(m_name.height);

        if has_key {
            let key_text = if a.keys.is_empty() {
                String::new()
            } else {
                a.keys.join(",")
            };
            let m_key = measurer.measure_wrapped(&key_text, attr_style, None, WrapMode::SvgLike);
            max_key_w = max_key_w.max(m_key.width);
            row_h = row_h.max(m_key.height);
        }

        if has_comment {
            let m_comment =
                measurer.measure_wrapped(&a.comment, attr_style, None, WrapMode::SvgLike);
            max_comment_w = max_comment_w.max(m_comment.width);
            row_h = row_h.max(m_comment.height);
        }

        cumulative_h += row_h + height_padding * 2.0;
    }

    let mut width_padding_factor = 4.0;
    if has_key {
        width_padding_factor += 2.0;
    }
    if has_comment {
        width_padding_factor += 2.0;
    }

    let max_width = max_type_w + max_name_w + max_key_w + max_comment_w;
    let w = min_w.max(
        (label_metrics.width + padding * 2.0).max(max_width + width_padding * width_padding_factor),
    );

    let h = if entity.attributes.is_empty() {
        min_h.max(label_metrics.height + padding * 2.0)
    } else {
        cumulative_h
    };

    (w.max(1.0), h.max(1.0))
}

fn edge_label_metrics(text: &str, measurer: &dyn TextMeasurer, style: &TextStyle) -> (f64, f64) {
    if text.trim().is_empty() {
        return (0.0, 0.0);
    }
    let m = measurer.measure_wrapped(text, style, None, WrapMode::SvgLike);
    (m.width.max(0.0), m.height.max(0.0))
}

#[derive(Debug, Clone)]
struct LayoutEdgeParts {
    id: String,
    from: String,
    to: String,
    points: Vec<LayoutPoint>,
    label: Option<LayoutLabel>,
    start_marker: Option<String>,
    end_marker: Option<String>,
    stroke_dasharray: Option<String>,
}

fn calc_label_position(points: &[LayoutPoint]) -> Option<(f64, f64)> {
    if points.is_empty() {
        return None;
    }
    if points.len() == 1 {
        return Some((points[0].x, points[0].y));
    }

    let mut total = 0.0;
    for i in 1..points.len() {
        let dx = points[i].x - points[i - 1].x;
        let dy = points[i].y - points[i - 1].y;
        total += (dx * dx + dy * dy).sqrt();
    }
    let mut remaining = total / 2.0;
    for i in 1..points.len() {
        let p0 = &points[i - 1];
        let p1 = &points[i];
        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;
        let seg = (dx * dx + dy * dy).sqrt();
        if seg == 0.0 {
            continue;
        }
        if seg < remaining {
            remaining -= seg;
            continue;
        }
        let t = (remaining / seg).clamp(0.0, 1.0);
        return Some((p0.x + t * dx, p0.y + t * dy));
    }
    Some((points.last()?.x, points.last()?.y))
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Rect {
    fn from_center(x: f64, y: f64, width: f64, height: f64) -> Self {
        let hw = width / 2.0;
        let hh = height / 2.0;
        Self {
            min_x: x - hw,
            min_y: y - hh,
            max_x: x + hw,
            max_y: y + hh,
        }
    }

    fn contains_point(&self, x: f64, y: f64) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }
}

fn intersect_segment_with_rect(
    p0: &LayoutPoint,
    p1: &LayoutPoint,
    rect: Rect,
) -> Option<LayoutPoint> {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    if dx == 0.0 && dy == 0.0 {
        return None;
    }

    let mut candidates: Vec<(f64, LayoutPoint)> = Vec::new();
    let eps = 1e-9;

    if dx.abs() > eps {
        for x_edge in [rect.min_x, rect.max_x] {
            let t = (x_edge - p0.x) / dx;
            if t < -eps || t > 1.0 + eps {
                continue;
            }
            let y = p0.y + t * dy;
            if y + eps >= rect.min_y && y <= rect.max_y + eps {
                candidates.push((t, LayoutPoint { x: x_edge, y }));
            }
        }
    }

    if dy.abs() > eps {
        for y_edge in [rect.min_y, rect.max_y] {
            let t = (y_edge - p0.y) / dy;
            if t < -eps || t > 1.0 + eps {
                continue;
            }
            let x = p0.x + t * dx;
            if x + eps >= rect.min_x && x <= rect.max_x + eps {
                candidates.push((t, LayoutPoint { x, y: y_edge }));
            }
        }
    }

    candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    candidates
        .into_iter()
        .find(|(t, _)| *t >= 0.0)
        .map(|(_, p)| p)
}

fn clip_edge_endpoints(points: &mut [LayoutPoint], from: Rect, to: Rect) {
    if points.len() < 2 {
        return;
    }
    if from.contains_point(points[0].x, points[0].y) {
        if let Some(p) = intersect_segment_with_rect(&points[0], &points[1], from) {
            points[0] = p;
        }
    }
    let last = points.len() - 1;
    if to.contains_point(points[last].x, points[last].y) {
        if let Some(p) = intersect_segment_with_rect(&points[last], &points[last - 1], to) {
            points[last] = p;
        }
    }
}

fn er_marker_id(card: &str, suffix: &str) -> Option<String> {
    match card {
        "ONLY_ONE" => Some(format!("ONLY_ONE_{suffix}")),
        "ZERO_OR_ONE" => Some(format!("ZERO_OR_ONE_{suffix}")),
        "ONE_OR_MORE" => Some(format!("ONE_OR_MORE_{suffix}")),
        "ZERO_OR_MORE" => Some(format!("ZERO_OR_MORE_{suffix}")),
        "MD_PARENT" => Some(format!("MD_PARENT_{suffix}")),
        _ => None,
    }
}

pub fn layout_er_diagram(
    semantic: &Value,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
) -> Result<ErDiagramLayout> {
    let model: ErModel = serde_json::from_value(semantic.clone())?;

    let nodesep = config_f64(effective_config, &["er", "nodeSpacing"]).unwrap_or(140.0);
    let ranksep = config_f64(effective_config, &["er", "rankSpacing"]).unwrap_or(80.0);
    let edgesep = nodesep;
    let dir = rank_dir_from(&model.direction);

    let label_style = er_text_style(effective_config);
    let attr_style = TextStyle {
        font_family: label_style.font_family.clone(),
        font_size: (label_style.font_size * 0.85).max(1.0),
        font_weight: None,
    };

    let mut g = Graph::<NodeLabel, EdgeLabel, GraphLabel>::new(GraphOptions {
        directed: true,
        multigraph: true,
        compound: false,
    });
    g.set_graph(GraphLabel {
        rankdir: dir,
        nodesep,
        ranksep,
        edgesep,
        ..Default::default()
    });

    // Nodes.
    for (_name, e) in &model.entities {
        let (w, h) =
            entity_box_dimensions(e, measurer, &label_style, &attr_style, effective_config);
        g.set_node(
            e.id.clone(),
            NodeLabel {
                width: w,
                height: h,
                ..Default::default()
            },
        );
    }

    // Edges. Upstream ER layout does not include label sizes in Dagre, but we still compute label
    // bbox for consumers after layout.
    for (idx, r) in model.relationships.iter().enumerate() {
        if g.node(&r.entity_a).is_none() || g.node(&r.entity_b).is_none() {
            return Err(Error::InvalidModel {
                message: format!(
                    "relationship references missing entities: {} -> {}",
                    r.entity_a, r.entity_b
                ),
            });
        }
        let name = format!("er-rel-{idx}");
        let el = EdgeLabel {
            width: 0.0,
            height: 0.0,
            labelpos: LabelPos::C,
            labeloffset: 10.0,
            minlen: 1,
            weight: 1.0,
            ..Default::default()
        };
        g.set_edge_named(r.entity_a.clone(), r.entity_b.clone(), Some(name), Some(el));
    }

    dugong::layout(&mut g);

    let mut nodes: Vec<LayoutNode> = Vec::new();
    for id in g.node_ids() {
        let Some(n) = g.node(&id) else {
            continue;
        };
        nodes.push(LayoutNode {
            id: id.clone(),
            x: n.x.unwrap_or(0.0),
            y: n.y.unwrap_or(0.0),
            width: n.width,
            height: n.height,
            is_cluster: false,
        });
    }
    nodes.sort_by(|a, b| a.id.cmp(&b.id));

    let mut node_rect_by_id: HashMap<String, Rect> = HashMap::new();
    for n in &nodes {
        node_rect_by_id.insert(n.id.clone(), Rect::from_center(n.x, n.y, n.width, n.height));
    }

    let mut edges: Vec<LayoutEdgeParts> = Vec::new();
    for key in g.edge_keys() {
        let Some(e) = g.edge_by_key(&key) else {
            continue;
        };
        let mut points = e
            .points
            .iter()
            .map(|p| LayoutPoint { x: p.x, y: p.y })
            .collect::<Vec<_>>();

        let id = key
            .name
            .clone()
            .unwrap_or_else(|| format!("edge:{}:{}", key.v, key.w));

        let rel_idx = key
            .name
            .as_ref()
            .and_then(|name| name.strip_prefix("er-rel-"))
            .and_then(|idx| idx.parse::<usize>().ok())
            .and_then(|idx| model.relationships.get(idx).map(|_| idx));

        let rel = rel_idx.and_then(|idx| model.relationships.get(idx));
        let role = rel.map(|r| r.role_a.clone()).unwrap_or_default();

        let (start_marker, end_marker, stroke_dasharray) = if let Some(rel) = rel {
            let card_a = rel
                .rel_spec
                .get("cardA")
                .and_then(Value::as_str)
                .unwrap_or("");
            let card_b = rel
                .rel_spec
                .get("cardB")
                .and_then(Value::as_str)
                .unwrap_or("");
            let rel_type = rel
                .rel_spec
                .get("relType")
                .and_then(Value::as_str)
                .unwrap_or("");
            let start_marker = er_marker_id(card_b, "START");
            let end_marker = er_marker_id(card_a, "END");
            let stroke_dasharray = if rel_type == "NON_IDENTIFYING" {
                Some("8,8".to_string())
            } else {
                None
            };
            (start_marker, end_marker, stroke_dasharray)
        } else {
            (None, None, None)
        };

        if let (Some(from_rect), Some(to_rect)) = (
            node_rect_by_id.get(&key.v).copied(),
            node_rect_by_id.get(&key.w).copied(),
        ) {
            clip_edge_endpoints(&mut points, from_rect, to_rect);
        }

        let label = if role.trim().is_empty() {
            None
        } else {
            let (w, h) = edge_label_metrics(&role, measurer, &label_style);
            calc_label_position(&points).map(|(x, y)| LayoutLabel {
                x,
                y,
                width: w.max(1.0),
                height: h.max(1.0),
            })
        };

        edges.push(LayoutEdgeParts {
            id,
            from: key.v.clone(),
            to: key.w.clone(),
            points,
            label,
            start_marker,
            end_marker,
            stroke_dasharray,
        });
    }
    edges.sort_by(|a, b| a.id.cmp(&b.id));

    let mut out_edges: Vec<LayoutEdge> = Vec::new();
    for e in edges {
        out_edges.push(LayoutEdge {
            id: e.id,
            from: e.from,
            to: e.to,
            from_cluster: None,
            to_cluster: None,
            points: e.points,
            label: e.label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: e.start_marker,
            end_marker: e.end_marker,
            stroke_dasharray: e.stroke_dasharray,
        });
    }

    let bounds = {
        let mut points: Vec<(f64, f64)> = Vec::new();
        for n in &nodes {
            let hw = n.width / 2.0;
            let hh = n.height / 2.0;
            points.push((n.x - hw, n.y - hh));
            points.push((n.x + hw, n.y + hh));
        }
        for e in &out_edges {
            for p in &e.points {
                points.push((p.x, p.y));
            }
            if let Some(l) = &e.label {
                let hw = l.width / 2.0;
                let hh = l.height / 2.0;
                points.push((l.x - hw, l.y - hh));
                points.push((l.x + hw, l.y + hh));
            }
        }
        Bounds::from_points(points)
    };

    Ok(ErDiagramLayout {
        nodes,
        edges: out_edges,
        bounds,
    })
}

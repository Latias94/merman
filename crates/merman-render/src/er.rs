use crate::model::{Bounds, ErDiagramLayout, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ErModel {
    pub direction: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub classes: BTreeMap<String, ErClassDef>,
    pub entities: BTreeMap<String, ErEntity>,
    #[serde(default)]
    pub relationships: Vec<ErRelationship>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ErEntity {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub alias: String,
    #[serde(default, rename = "cssClasses")]
    pub css_classes: String,
    #[serde(default, rename = "cssStyles")]
    pub css_styles: Vec<String>,
    #[serde(default)]
    pub attributes: Vec<ErAttribute>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ErAttribute {
    #[serde(rename = "type")]
    pub ty: String,
    pub name: String,
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub comment: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ErRelationship {
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

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ErClassDef {
    #[allow(dead_code)]
    pub id: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub styles: Vec<String>,
    #[serde(default, rename = "textStyles")]
    #[allow(dead_code)]
    pub text_styles: Vec<String>,
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

pub(crate) fn parse_generic_types_like_mermaid(text: &str) -> String {
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
    // Mermaid ER unified renderer output uses the global Mermaid `fontSize` (defaults to 16px)
    // via the root `#id{font-size:...}` rule. Prefer the global value for parity.
    let font_size = config_f64(effective_config, &["fontSize"])
        .or_else(|| config_f64(effective_config, &["er", "fontSize"]))
        .unwrap_or(16.0);
    TextStyle {
        font_family,
        font_size,
        font_weight: None,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ErEntityMeasureRow {
    pub type_text: String,
    pub name_text: String,
    pub key_text: String,
    pub comment_text: String,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct ErEntityMeasure {
    pub width: f64,
    pub height: f64,
    pub padding: f64,
    pub text_padding: f64,
    pub label_text: String,
    pub label_height: f64,
    pub has_key: bool,
    pub has_comment: bool,
    pub type_col_w: f64,
    pub name_col_w: f64,
    pub key_col_w: f64,
    pub comment_col_w: f64,
    pub rows: Vec<ErEntityMeasureRow>,
}

pub(crate) fn measure_entity_box(
    entity: &ErEntity,
    measurer: &dyn TextMeasurer,
    label_style: &TextStyle,
    attr_style: &TextStyle,
    effective_config: &Value,
) -> ErEntityMeasure {
    // Mermaid measures ER attribute table text via HTML labels (`foreignObject`) and browser font
    // metrics. Our headless measurer is an approximation; apply a small, ER-specific width bump so
    // attribute column widths are closer to upstream fixtures.
    const ATTR_TEXT_WIDTH_SCALE: f64 = 1.15;

    let html_labels = config_bool(effective_config, &["htmlLabels"]).unwrap_or(true);

    // Mermaid ER unified shape (`erBox.ts`) uses:
    // - PADDING = config.er.diagramPadding (default 20 in Mermaid 11.12.2 schema defaults)
    // - TEXT_PADDING = config.er.entityPadding (default 15)
    let mut padding = config_f64(effective_config, &["er", "diagramPadding"]).unwrap_or(20.0);
    let mut text_padding = config_f64(effective_config, &["er", "entityPadding"]).unwrap_or(15.0);
    let min_w = config_f64(effective_config, &["er", "minEntityWidth"]).unwrap_or(100.0);

    if !html_labels {
        padding *= 1.25;
        text_padding *= 1.25;
    }

    let wrap_mode = if html_labels {
        WrapMode::HtmlLike
    } else {
        WrapMode::SvgLike
    };

    let label_text = if entity.alias.trim().is_empty() {
        entity.label.as_str()
    } else {
        entity.alias.as_str()
    }
    .to_string();
    let label_metrics = measurer.measure_wrapped(&label_text, label_style, None, wrap_mode);

    // No attributes: use `drawRect`-like padding rules from Mermaid erBox.ts.
    if entity.attributes.is_empty() {
        let label_pad_x = padding;
        let label_pad_y = padding * 1.5;
        let width = (label_metrics.width + label_pad_x * 2.0).max(min_w);
        let height = label_metrics.height + label_pad_y * 2.0;
        return ErEntityMeasure {
            width: width.max(1.0),
            height: height.max(1.0),
            padding,
            text_padding,
            label_text,
            label_height: label_metrics.height.max(0.0),
            has_key: false,
            has_comment: false,
            type_col_w: 0.0,
            name_col_w: 0.0,
            key_col_w: 0.0,
            comment_col_w: 0.0,
            rows: Vec::new(),
        };
    }

    let mut rows: Vec<ErEntityMeasureRow> = Vec::new();

    let mut max_type_raw_w: f64 = 0.0;
    let mut max_name_raw_w: f64 = 0.0;
    let mut max_keys_raw_w: f64 = 0.0;
    let mut max_comment_raw_w: f64 = 0.0;

    let mut max_type_col_w: f64 = 0.0;
    let mut max_name_col_w: f64 = 0.0;
    let mut max_keys_col_w: f64 = 0.0;
    let mut max_comment_col_w: f64 = 0.0;

    let mut total_rows_h = 0.0;

    for a in &entity.attributes {
        let ty = parse_generic_types_like_mermaid(&a.ty);
        let type_m = measurer.measure_wrapped(&ty, attr_style, None, wrap_mode);
        let name_m = measurer.measure_wrapped(&a.name, attr_style, None, wrap_mode);

        let type_w = type_m.width * ATTR_TEXT_WIDTH_SCALE;
        let name_w = name_m.width * ATTR_TEXT_WIDTH_SCALE;
        max_type_raw_w = max_type_raw_w.max(type_w);
        max_name_raw_w = max_name_raw_w.max(name_w);
        max_type_col_w = max_type_col_w.max(type_w + padding);
        max_name_col_w = max_name_col_w.max(name_w + padding);

        let key_text = a.keys.join(",");
        let keys_m = measurer.measure_wrapped(&key_text, attr_style, None, wrap_mode);
        let keys_w = keys_m.width * ATTR_TEXT_WIDTH_SCALE;
        max_keys_raw_w = max_keys_raw_w.max(keys_w);
        max_keys_col_w = max_keys_col_w.max(keys_w + padding);

        let comment_text = a.comment.clone();
        let comment_m = measurer.measure_wrapped(&comment_text, attr_style, None, wrap_mode);
        let comment_w = comment_m.width * ATTR_TEXT_WIDTH_SCALE;
        max_comment_raw_w = max_comment_raw_w.max(comment_w);
        max_comment_col_w = max_comment_col_w.max(comment_w + padding);

        let row_h = type_m
            .height
            .max(name_m.height)
            .max(keys_m.height)
            .max(comment_m.height)
            + text_padding;

        rows.push(ErEntityMeasureRow {
            type_text: ty,
            name_text: a.name.clone(),
            key_text,
            comment_text,
            height: row_h.max(1.0),
        });
        total_rows_h += row_h.max(1.0);
    }

    let mut total_width_sections = 4usize;
    let mut has_key = true;
    let mut has_comment = true;
    if max_keys_col_w <= padding {
        has_key = false;
        max_keys_col_w = 0.0;
        total_width_sections = total_width_sections.saturating_sub(1);
    }
    if max_comment_col_w <= padding {
        has_comment = false;
        max_comment_col_w = 0.0;
        total_width_sections = total_width_sections.saturating_sub(1);
    }

    // Mermaid adds extra padding to attribute components to accommodate the entity name width.
    let name_w_min = label_metrics.width + padding * 2.0;
    let mut max_width = max_type_col_w + max_name_col_w + max_keys_col_w + max_comment_col_w;
    if name_w_min - max_width > 0.0 && total_width_sections > 0 {
        let diff = name_w_min - max_width;
        let per = diff / total_width_sections as f64;
        max_type_col_w += per;
        max_name_col_w += per;
        if has_key {
            max_keys_col_w += per;
        }
        if has_comment {
            max_comment_col_w += per;
        }
        max_width = max_type_col_w + max_name_col_w + max_keys_col_w + max_comment_col_w;
    }

    let shape_bbox_w = label_metrics
        .width
        .max(max_type_raw_w)
        .max(max_name_raw_w)
        .max(max_keys_raw_w)
        .max(max_comment_raw_w);

    let width = (shape_bbox_w + padding * 2.0).max(max_width);
    let name_h = label_metrics.height + text_padding;
    let height = total_rows_h + name_h;

    ErEntityMeasure {
        width: width.max(1.0),
        height: height.max(1.0),
        padding,
        text_padding,
        label_text,
        label_height: label_metrics.height.max(0.0),
        has_key,
        has_comment,
        type_col_w: max_type_col_w.max(0.0),
        name_col_w: max_name_col_w.max(0.0),
        key_col_w: max_keys_col_w.max(0.0),
        comment_col_w: max_comment_col_w.max(0.0),
        rows,
    }
}

fn entity_box_dimensions(
    entity: &ErEntity,
    measurer: &dyn TextMeasurer,
    label_style: &TextStyle,
    attr_style: &TextStyle,
    effective_config: &Value,
) -> (f64, f64) {
    let m = measure_entity_box(entity, measurer, label_style, attr_style, effective_config);
    (m.width, m.height)
}

fn edge_label_metrics(text: &str, measurer: &dyn TextMeasurer, style: &TextStyle) -> (f64, f64) {
    if text.trim().is_empty() {
        return (0.0, 0.0);
    }
    // Mermaid ER uses HTML labels by default (foreignObject) and uses line-height: 1.5.
    let m = measurer.measure_wrapped(text, style, None, WrapMode::HtmlLike);
    (m.width.max(0.0), m.height.max(0.0))
}

fn parse_er_rel_idx_from_edge_name(name: &str) -> Option<usize> {
    let rest = name.strip_prefix("er-rel-")?;
    let mut end = 0usize;
    for (idx, ch) in rest.char_indices() {
        if !ch.is_ascii_digit() {
            break;
        }
        end = idx + ch.len_utf8();
    }
    if end == 0 {
        return None;
    }
    rest[..end].parse::<usize>().ok()
}

fn is_er_self_loop_dummy_node_id(id: &str) -> bool {
    // Mermaid's dagre renderer creates self-loop helper nodes using `${nodeId}---${nodeId}---{1|2}`.
    id.contains("---")
}

fn config_bool(cfg: &Value, path: &[&str]) -> Option<bool> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_bool()
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
        // Mermaid ER unified renderer does not enable a dedicated MD_PARENT marker. In practice,
        // Mermaid CLI output maps `u` cardinality to the same marker as `ONLY_ONE`.
        "MD_PARENT" => Some(format!("ONLY_ONE_{suffix}")),
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
    let dir = rank_dir_from(&model.direction);

    let label_style = er_text_style(effective_config);
    let attr_style = TextStyle {
        font_family: label_style.font_family.clone(),
        font_size: label_style.font_size.max(1.0),
        font_weight: None,
    };
    let rel_label_style = TextStyle {
        font_family: label_style.font_family.clone(),
        // Mermaid ER edge labels default to 14px when `fontSize=16`.
        font_size: (label_style.font_size - 2.0).max(1.0),
        font_weight: None,
    };

    let mut g = Graph::<NodeLabel, EdgeLabel, GraphLabel>::new(GraphOptions {
        directed: true,
        multigraph: true,
        // Mermaid's dagre adapter always enables `compound: true` (even if there are no clusters).
        // This also makes the ranker behavior match upstream for disconnected ER graphs.
        compound: true,
    });
    g.set_graph(GraphLabel {
        rankdir: dir,
        nodesep,
        ranksep,
        // Dagre's default `acyclicer` is "greedy" (Mermaid relies on this default).
        acyclicer: Some("greedy".to_string()),
        ..Default::default()
    });

    fn parse_entity_counter_from_id(id: &str) -> Option<usize> {
        let (_prefix, tail) = id.rsplit_once('-')?;
        tail.parse::<usize>().ok()
    }

    // Nodes.
    let mut entities_in_layout_order: Vec<&ErEntity> = model.entities.values().collect();
    entities_in_layout_order.sort_by(|a, b| {
        let a_key = (parse_entity_counter_from_id(&a.id), a.id.as_str());
        let b_key = (parse_entity_counter_from_id(&b.id), b.id.as_str());
        a_key.cmp(&b_key)
    });

    for e in entities_in_layout_order {
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

    // Edges. Mermaid ER uses edge labels ("roleA") and the unified renderer routes through the
    // generic dagre pipeline, which accounts for label bbox in spacing. Mirror that by giving
    // dagre real label sizes here.
    for (idx, r) in model.relationships.iter().enumerate() {
        if g.node(&r.entity_a).is_none() || g.node(&r.entity_b).is_none() {
            return Err(Error::InvalidModel {
                message: format!(
                    "relationship references missing entities: {} -> {}",
                    r.entity_a, r.entity_b
                ),
            });
        }

        // Mermaid's dagre renderer splits self-loops into three edges and introduces two 10x10
        // helper nodes. Mirror that so the layout/bounds match upstream more closely.
        if r.entity_a == r.entity_b {
            let node_id = r.entity_a.as_str();
            let special_1 = format!("{node_id}---{node_id}---1");
            let special_2 = format!("{node_id}---{node_id}---2");

            if g.node(&special_1).is_none() {
                g.set_node(
                    special_1.clone(),
                    NodeLabel {
                        width: 10.0,
                        height: 10.0,
                        ..Default::default()
                    },
                );
            }
            if g.node(&special_2).is_none() {
                g.set_node(
                    special_2.clone(),
                    NodeLabel {
                        width: 10.0,
                        height: 10.0,
                        ..Default::default()
                    },
                );
            }

            let (label_w, label_h) = if r.role_a.trim().is_empty() {
                (0.0, 0.0)
            } else {
                edge_label_metrics(&r.role_a, measurer, &rel_label_style)
            };

            // First segment: keep start marker, no label.
            g.set_edge_named(
                r.entity_a.clone(),
                special_1.clone(),
                Some(format!("er-rel-{idx}-cyclic-0")),
                Some(EdgeLabel {
                    width: 0.0,
                    height: 0.0,
                    labelpos: LabelPos::C,
                    labeloffset: 10.0,
                    minlen: 1,
                    weight: 1.0,
                    ..Default::default()
                }),
            );

            // Mid segment: carries the relationship label, no markers.
            g.set_edge_named(
                special_1.clone(),
                special_2.clone(),
                Some(format!("er-rel-{idx}")),
                Some(EdgeLabel {
                    width: label_w.max(0.0),
                    height: label_h.max(0.0),
                    labelpos: LabelPos::C,
                    labeloffset: 10.0,
                    minlen: 1,
                    weight: 1.0,
                    ..Default::default()
                }),
            );

            // Last segment: keep end marker, no label.
            g.set_edge_named(
                special_2.clone(),
                r.entity_a.clone(),
                Some(format!("er-rel-{idx}-cyclic-2")),
                Some(EdgeLabel {
                    width: 0.0,
                    height: 0.0,
                    labelpos: LabelPos::C,
                    labeloffset: 10.0,
                    minlen: 1,
                    weight: 1.0,
                    ..Default::default()
                }),
            );

            continue;
        }

        let name = format!("er-rel-{idx}");
        let (label_w, label_h) = if r.role_a.trim().is_empty() {
            (0.0, 0.0)
        } else {
            edge_label_metrics(&r.role_a, measurer, &rel_label_style)
        };
        g.set_edge_named(
            r.entity_a.clone(),
            r.entity_b.clone(),
            Some(name),
            Some(EdgeLabel {
                width: label_w.max(0.0),
                height: label_h.max(0.0),
                labelpos: LabelPos::C,
                labeloffset: 10.0,
                minlen: 1,
                weight: 1.0,
                ..Default::default()
            }),
        );
    }

    dugong::layout_dagreish(&mut g);

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
            .and_then(|name| parse_er_rel_idx_from_edge_name(name))
            .and_then(|idx| model.relationships.get(idx).map(|_| idx));

        let rel = rel_idx.and_then(|idx| model.relationships.get(idx));
        let role = rel.map(|r| r.role_a.clone()).unwrap_or_default();

        let (base_start_marker, base_end_marker, stroke_dasharray) = if let Some(rel) = rel {
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

        if !is_er_self_loop_dummy_node_id(&key.v) && !is_er_self_loop_dummy_node_id(&key.w) {
            if let (Some(from_rect), Some(to_rect)) = (
                node_rect_by_id.get(&key.v).copied(),
                node_rect_by_id.get(&key.w).copied(),
            ) {
                clip_edge_endpoints(&mut points, from_rect, to_rect);
            }
        }

        let (start_marker, end_marker) =
            if is_er_self_loop_dummy_node_id(&key.v) && is_er_self_loop_dummy_node_id(&key.w) {
                (None, None)
            } else if id.ends_with("-cyclic-0") {
                (base_start_marker, None)
            } else if id.ends_with("-cyclic-2") {
                (None, base_end_marker)
            } else {
                (base_start_marker, base_end_marker)
            };

        let label =
            if role.trim().is_empty() || id.ends_with("-cyclic-0") || id.ends_with("-cyclic-2") {
                None
            } else {
                let (w, h) = edge_label_metrics(&role, measurer, &rel_label_style);
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

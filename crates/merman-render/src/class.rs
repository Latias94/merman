use crate::model::{
    Bounds, ClassDiagramV2Layout, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Deserialize)]
struct ClassDiagramModel {
    pub direction: String,
    pub classes: BTreeMap<String, ClassNode>,
    #[serde(default)]
    pub relations: Vec<ClassRelation>,
    #[serde(default)]
    pub notes: Vec<ClassNote>,
    #[serde(default)]
    pub namespaces: BTreeMap<String, Namespace>,
}

#[derive(Debug, Clone, Deserialize)]
struct ClassNode {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub annotations: Vec<String>,
    #[serde(default)]
    pub members: Vec<ClassMember>,
    #[serde(default)]
    pub methods: Vec<ClassMember>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub r#type: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ClassMember {
    #[serde(rename = "displayText")]
    pub display_text: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ClassRelation {
    pub id: String,
    pub id1: String,
    pub id2: String,
    #[serde(rename = "relationTitle1")]
    pub relation_title_1: String,
    #[serde(rename = "relationTitle2")]
    pub relation_title_2: String,
    pub title: String,
    pub relation: RelationShape,
}

#[derive(Debug, Clone, Deserialize)]
struct RelationShape {
    pub type1: i32,
    pub type2: i32,
    #[serde(rename = "lineType")]
    #[allow(dead_code)]
    pub line_type: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct ClassNote {
    pub id: String,
    #[serde(rename = "class")]
    pub class_id: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Namespace {
    #[allow(dead_code)]
    pub id: String,
    #[serde(rename = "classIds")]
    pub class_ids: Vec<String>,
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

fn config_bool(cfg: &Value, path: &[&str]) -> Option<bool> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_bool()
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

fn decode_entities_minimal(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
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

    fn width(&self) -> f64 {
        self.max_x - self.min_x
    }

    fn height(&self) -> f64 {
        self.max_y - self.min_y
    }

    fn center(&self) -> (f64, f64) {
        (
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
        )
    }

    fn union(&mut self, other: Rect) {
        self.min_x = self.min_x.min(other.min_x);
        self.min_y = self.min_y.min(other.min_y);
        self.max_x = self.max_x.max(other.max_x);
        self.max_y = self.max_y.max(other.max_y);
    }

    fn expand(&mut self, pad: f64) {
        let p = pad.max(0.0);
        self.min_x -= p;
        self.min_y -= p;
        self.max_x += p;
        self.max_y += p;
    }
}

struct PreparedGraph {
    graph: Graph<NodeLabel, EdgeLabel, GraphLabel>,
    extracted: BTreeMap<String, PreparedGraph>,
}

fn extract_descendants(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    id: &str,
    out: &mut Vec<String>,
) {
    for child in graph.children(id) {
        out.push(child.to_string());
        extract_descendants(graph, child, out);
    }
}

fn is_descendant(descendants: &HashMap<String, HashSet<String>>, id: &str, ancestor: &str) -> bool {
    descendants
        .get(ancestor)
        .is_some_and(|set| set.contains(id))
}

fn prepare_graph(
    mut graph: Graph<NodeLabel, EdgeLabel, GraphLabel>,
    depth: usize,
) -> Result<PreparedGraph> {
    if depth > 10 {
        return Ok(PreparedGraph {
            graph,
            extracted: BTreeMap::new(),
        });
    }

    let cluster_ids: Vec<String> = graph
        .node_ids()
        .into_iter()
        .filter(|id| !graph.children(id).is_empty())
        .collect();

    let mut descendants: HashMap<String, HashSet<String>> = HashMap::new();
    for id in &cluster_ids {
        let mut vec: Vec<String> = Vec::new();
        extract_descendants(&graph, id, &mut vec);
        descendants.insert(id.clone(), vec.into_iter().collect());
    }

    let mut external: HashMap<String, bool> =
        cluster_ids.iter().map(|id| (id.clone(), false)).collect();
    for id in &cluster_ids {
        for e in graph.edge_keys() {
            let d1 = is_descendant(&descendants, &e.v, id);
            let d2 = is_descendant(&descendants, &e.w, id);
            if d1 ^ d2 {
                external.insert(id.clone(), true);
                break;
            }
        }
    }

    let mut extracted: BTreeMap<String, PreparedGraph> = BTreeMap::new();
    let mut candidate_roots: Vec<String> = Vec::new();
    for id in graph.node_ids() {
        if graph.children(&id).is_empty() {
            continue;
        }
        if graph.parent(&id).is_some() {
            continue;
        }
        if *external.get(&id).unwrap_or(&false) {
            continue;
        }
        candidate_roots.push(id);
    }
    candidate_roots.sort();

    for cluster_id in candidate_roots {
        let parent_dir = graph.graph().rankdir;
        let dir = if parent_dir == RankDir::TB {
            RankDir::LR
        } else {
            RankDir::TB
        };
        let nodesep = graph.graph().nodesep;
        let ranksep = graph.graph().ranksep;

        let mut subgraph = extract_cluster_graph(&cluster_id, &mut graph)?;
        subgraph.graph_mut().rankdir = dir;
        subgraph.graph_mut().nodesep = nodesep;
        subgraph.graph_mut().ranksep = ranksep;

        let prepared = prepare_graph(subgraph, depth + 1)?;
        extracted.insert(cluster_id, prepared);
    }

    Ok(PreparedGraph { graph, extracted })
}

fn extract_cluster_graph(
    cluster_id: &str,
    graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<Graph<NodeLabel, EdgeLabel, GraphLabel>> {
    if graph.children(cluster_id).is_empty() {
        return Err(Error::InvalidModel {
            message: format!("cluster has no children: {cluster_id}"),
        });
    }

    let mut descendants: Vec<String> = Vec::new();
    extract_descendants(graph, cluster_id, &mut descendants);
    descendants.sort();
    descendants.dedup();

    let moved_set: HashSet<String> = descendants.iter().cloned().collect();

    let mut sub = Graph::<NodeLabel, EdgeLabel, GraphLabel>::new(GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    });

    for id in &descendants {
        let Some(label) = graph.node(id).cloned() else {
            continue;
        };
        sub.set_node(id.clone(), label);
    }

    for key in graph.edge_keys() {
        if moved_set.contains(&key.v) && moved_set.contains(&key.w) {
            if let Some(label) = graph.edge_by_key(&key).cloned() {
                sub.set_edge_named(key.v.clone(), key.w.clone(), key.name.clone(), Some(label));
            }
        }
    }

    for id in &descendants {
        let Some(parent) = graph.parent(id) else {
            continue;
        };
        if parent == cluster_id {
            continue;
        }
        if moved_set.contains(parent) {
            sub.set_parent(id.clone(), parent.to_string());
        }
    }

    for id in &descendants {
        let _ = graph.remove_node(id);
    }

    Ok(sub)
}

#[derive(Debug, Clone)]
struct EdgeTerminalMetrics {
    start_left: Option<(f64, f64)>,
    start_right: Option<(f64, f64)>,
    end_left: Option<(f64, f64)>,
    end_right: Option<(f64, f64)>,
    start_marker: f64,
    end_marker: f64,
}

fn edge_terminal_metrics_from_extras(e: &EdgeLabel) -> EdgeTerminalMetrics {
    let get_pair = |key: &str| -> Option<(f64, f64)> {
        let obj = e.extras.get(key)?;
        let w = obj.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let h = obj.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0);
        if w > 0.0 && h > 0.0 {
            Some((w, h))
        } else {
            None
        }
    };
    let start_marker = e
        .extras
        .get("startMarker")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let end_marker = e
        .extras
        .get("endMarker")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    EdgeTerminalMetrics {
        start_left: get_pair("startLeft"),
        start_right: get_pair("startRight"),
        end_left: get_pair("endLeft"),
        end_right: get_pair("endRight"),
        start_marker,
        end_marker,
    }
}

#[derive(Debug, Clone)]
struct LayoutFragments {
    nodes: HashMap<String, LayoutNode>,
    edges: Vec<(LayoutEdge, Option<EdgeTerminalMetrics>)>,
}

fn round_number(num: f64, precision: i32) -> f64 {
    if !num.is_finite() {
        return 0.0;
    }
    let factor = 10_f64.powi(precision);
    (num * factor).round() / factor
}

fn distance(a: &LayoutPoint, b: Option<&LayoutPoint>) -> f64 {
    let Some(b) = b else {
        return 0.0;
    };
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

fn calculate_point(points: &[LayoutPoint], distance_to_traverse: f64) -> Option<LayoutPoint> {
    if points.is_empty() {
        return None;
    }
    let mut prev: Option<&LayoutPoint> = None;
    let mut remaining = distance_to_traverse.max(0.0);
    for p in points {
        if let Some(prev_p) = prev {
            let vector_distance = distance(p, Some(prev_p));
            if vector_distance == 0.0 {
                return Some(prev_p.clone());
            }
            if vector_distance < remaining {
                remaining -= vector_distance;
            } else {
                let ratio = remaining / vector_distance;
                if ratio <= 0.0 {
                    return Some(prev_p.clone());
                }
                if ratio >= 1.0 {
                    return Some(p.clone());
                }
                return Some(LayoutPoint {
                    x: round_number((1.0 - ratio) * prev_p.x + ratio * p.x, 5),
                    y: round_number((1.0 - ratio) * prev_p.y + ratio * p.y, 5),
                });
            }
        }
        prev = Some(p);
    }
    None
}

#[derive(Debug, Clone, Copy)]
enum TerminalPos {
    StartLeft,
    StartRight,
    EndLeft,
    EndRight,
}

fn point_inside_rect(rect: Rect, x: f64, y: f64, eps: f64) -> bool {
    x > rect.min_x + eps && x < rect.max_x - eps && y > rect.min_y + eps && y < rect.max_y - eps
}

fn nudge_point_outside_rect(mut x: f64, mut y: f64, rect: Rect) -> (f64, f64) {
    let eps = 0.01;
    if !point_inside_rect(rect, x, y, eps) {
        return (x, y);
    }

    let (cx, cy) = rect.center();
    let mut dx = x - cx;
    let mut dy = y - cy;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 {
        dx = 1.0;
        dy = 0.0;
    } else {
        dx /= len;
        dy /= len;
    }

    let mut t_exit = f64::INFINITY;
    if dx > 1e-9 {
        t_exit = t_exit.min((rect.max_x - x) / dx);
    } else if dx < -1e-9 {
        t_exit = t_exit.min((rect.min_x - x) / dx);
    }
    if dy > 1e-9 {
        t_exit = t_exit.min((rect.max_y - y) / dy);
    } else if dy < -1e-9 {
        t_exit = t_exit.min((rect.min_y - y) / dy);
    }

    if t_exit.is_finite() && t_exit >= 0.0 {
        let margin = 0.5;
        x += dx * (t_exit + margin);
        y += dy * (t_exit + margin);
    }

    (x, y)
}

fn calc_terminal_label_position(
    terminal_marker_size: f64,
    position: TerminalPos,
    points: &[LayoutPoint],
) -> Option<(f64, f64)> {
    if points.len() < 2 {
        return None;
    }

    let mut pts = points.to_vec();
    match position {
        TerminalPos::StartLeft | TerminalPos::StartRight => {}
        TerminalPos::EndLeft | TerminalPos::EndRight => pts.reverse(),
    }

    let distance_to_cardinality_point = 25.0 + terminal_marker_size;
    let center = calculate_point(&pts, distance_to_cardinality_point)?;
    let d = 10.0 + terminal_marker_size * 0.5;
    let angle = (pts[0].y - center.y).atan2(pts[0].x - center.x);

    let (x, y) = match position {
        TerminalPos::StartLeft => {
            let a = angle + std::f64::consts::PI;
            (
                a.sin() * d + (pts[0].x + center.x) / 2.0,
                -a.cos() * d + (pts[0].y + center.y) / 2.0,
            )
        }
        TerminalPos::StartRight => (
            angle.sin() * d + (pts[0].x + center.x) / 2.0,
            -angle.cos() * d + (pts[0].y + center.y) / 2.0,
        ),
        TerminalPos::EndLeft => (
            angle.sin() * d + (pts[0].x + center.x) / 2.0 - 5.0,
            -angle.cos() * d + (pts[0].y + center.y) / 2.0 - 5.0,
        ),
        TerminalPos::EndRight => {
            let a = angle - std::f64::consts::PI;
            (
                a.sin() * d + (pts[0].x + center.x) / 2.0 - 5.0,
                -a.cos() * d + (pts[0].y + center.y) / 2.0 - 5.0,
            )
        }
    };
    Some((x, y))
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

fn terminal_path_for_edge(
    points: &[LayoutPoint],
    from_rect: Rect,
    to_rect: Rect,
) -> Vec<LayoutPoint> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let mut out = points.to_vec();

    if let Some(p) = intersect_segment_with_rect(&out[0], &out[1], from_rect) {
        out[0] = p;
    }
    let last = out.len() - 1;
    if let Some(p) = intersect_segment_with_rect(&out[last], &out[last - 1], to_rect) {
        out[last] = p;
    }

    out
}

fn layout_prepared(prepared: &mut PreparedGraph) -> Result<(LayoutFragments, Rect)> {
    let mut fragments = LayoutFragments {
        nodes: HashMap::new(),
        edges: Vec::new(),
    };

    let extracted_ids: Vec<String> = prepared.extracted.keys().cloned().collect();
    let mut extracted_fragments: HashMap<String, (LayoutFragments, Rect)> = HashMap::new();
    for id in extracted_ids {
        let sub = prepared.extracted.get_mut(&id).expect("exists");
        let (sub_frag, sub_bounds) = layout_prepared(sub)?;
        extracted_fragments.insert(id, (sub_frag, sub_bounds));
    }

    for (id, (_sub_frag, bounds)) in &extracted_fragments {
        let Some(n) = prepared.graph.node_mut(id) else {
            return Err(Error::InvalidModel {
                message: format!("missing cluster placeholder node: {id}"),
            });
        };
        n.width = bounds.width().max(1.0);
        n.height = bounds.height().max(1.0);
    }

    dugong::layout(&mut prepared.graph);

    for id in prepared.graph.node_ids() {
        let Some(n) = prepared.graph.node(&id) else {
            continue;
        };
        fragments.nodes.insert(
            id.clone(),
            LayoutNode {
                id: id.clone(),
                x: n.x.unwrap_or(0.0),
                y: n.y.unwrap_or(0.0),
                width: n.width,
                height: n.height,
                is_cluster: false,
            },
        );
    }

    for key in prepared.graph.edge_keys() {
        let Some(e) = prepared.graph.edge_by_key(&key) else {
            continue;
        };
        let id = key
            .name
            .clone()
            .unwrap_or_else(|| format!("edge:{}:{}", key.v, key.w));

        let label = if e.width > 0.0 && e.height > 0.0 {
            Some(LayoutLabel {
                x: e.x.unwrap_or(0.0),
                y: e.y.unwrap_or(0.0),
                width: e.width,
                height: e.height,
            })
        } else {
            None
        };

        let points = e
            .points
            .iter()
            .map(|p| LayoutPoint { x: p.x, y: p.y })
            .collect::<Vec<_>>();

        let edge = LayoutEdge {
            id,
            from: key.v.clone(),
            to: key.w.clone(),
            from_cluster: None,
            to_cluster: None,
            points,
            label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        };

        let terminals = edge_terminal_metrics_from_extras(e);
        let has_terminals = terminals.start_left.is_some()
            || terminals.start_right.is_some()
            || terminals.end_left.is_some()
            || terminals.end_right.is_some();
        let terminal_meta = if has_terminals { Some(terminals) } else { None };

        fragments.edges.push((edge, terminal_meta));
    }

    for (cluster_id, (mut sub_frag, sub_bounds)) in extracted_fragments {
        let Some(cluster_node) = fragments.nodes.get(&cluster_id).cloned() else {
            return Err(Error::InvalidModel {
                message: format!("missing cluster placeholder layout: {cluster_id}"),
            });
        };
        let (sub_cx, sub_cy) = sub_bounds.center();
        let dx = cluster_node.x - sub_cx;
        let dy = cluster_node.y - sub_cy;

        for n in sub_frag.nodes.values_mut() {
            n.x += dx;
            n.y += dy;
        }
        for (e, _t) in &mut sub_frag.edges {
            for p in &mut e.points {
                p.x += dx;
                p.y += dy;
            }
            if let Some(l) = e.label.as_mut() {
                l.x += dx;
                l.y += dy;
            }
        }

        fragments.nodes.extend(sub_frag.nodes);
        fragments.edges.extend(sub_frag.edges);
    }

    let mut points: Vec<(f64, f64)> = Vec::new();
    for n in fragments.nodes.values() {
        let r = Rect::from_center(n.x, n.y, n.width, n.height);
        points.push((r.min_x, r.min_y));
        points.push((r.max_x, r.max_y));
    }
    for (e, _t) in &fragments.edges {
        for p in &e.points {
            points.push((p.x, p.y));
        }
        if let Some(l) = &e.label {
            let r = Rect::from_center(l.x, l.y, l.width, l.height);
            points.push((r.min_x, r.min_y));
            points.push((r.max_x, r.max_y));
        }
    }
    let bounds = Bounds::from_points(points)
        .map(|b| Rect {
            min_x: b.min_x,
            min_y: b.min_y,
            max_x: b.max_x,
            max_y: b.max_y,
        })
        .unwrap_or(Rect {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 0.0,
            max_y: 0.0,
        });

    Ok((fragments, bounds))
}

fn class_text_style(effective_config: &Value) -> TextStyle {
    let font_family = config_string(effective_config, &["fontFamily"]);
    // Mermaid class diagram node labels inherit the global `fontSize` (via the root `#id{font-size}` rule)
    // and render via HTML labels (`foreignObject`). Prefer the global value for sizing/layout parity.
    let font_size = config_f64(effective_config, &["fontSize"])
        .or_else(|| config_f64(effective_config, &["class", "fontSize"]))
        .unwrap_or(16.0)
        .max(1.0);
    TextStyle {
        font_family,
        font_size,
        font_weight: None,
    }
}

fn class_box_dimensions(
    node: &ClassNode,
    measurer: &dyn TextMeasurer,
    text_style: &TextStyle,
    wrap_mode: WrapMode,
    _padding: f64,
    hide_empty_members_box: bool,
) -> (f64, f64) {
    let font_size = text_style.font_size.max(1.0);
    let line_height = font_size * 1.5;

    let label_text = decode_entities_minimal(&node.text);
    let mut max_w = measurer
        .measure_wrapped(&label_text, text_style, None, wrap_mode)
        .width;

    for a in &node.annotations {
        let t = format!("\u{00AB}{}\u{00BB}", decode_entities_minimal(a.trim()));
        let m = measurer.measure_wrapped(&t, text_style, None, wrap_mode);
        max_w = max_w.max(m.width);
    }

    for m in &node.members {
        let t = decode_entities_minimal(m.display_text.trim());
        let metrics = measurer.measure_wrapped(&t, text_style, None, wrap_mode);
        max_w = max_w.max(metrics.width);
    }

    for m in &node.methods {
        let t = decode_entities_minimal(m.display_text.trim());
        let metrics = measurer.measure_wrapped(&t, text_style, None, wrap_mode);
        max_w = max_w.max(metrics.width);
    }

    let has_members = !node.members.is_empty();
    let has_methods = !node.methods.is_empty();

    // Mermaid class node sizing is row-based (via HTML labels) with a fixed line-height of `1.5`.
    // Use a deterministic, layout-friendly approximation matching upstream structure:
    // - width: add one line-height of horizontal padding for title-only nodes, two for nodes with
    //   members/methods compartments.
    // - height: top padding = 0.5 line-height; bottom padding = 0 for title-only nodes, 0.5 otherwise.
    let rect_w = if has_members || has_methods {
        max_w + 2.0 * line_height
    } else {
        max_w + line_height
    };

    let ann_rows = node.annotations.len();
    let member_rows = if has_members { node.members.len() } else { 0 };
    let method_rows = if has_methods { node.methods.len() } else { 0 };
    let divider_rows = if hide_empty_members_box && !has_members && !has_methods {
        0
    } else {
        2
    };
    let title_rows = 1;
    let total_rows = ann_rows + title_rows + divider_rows + member_rows + method_rows;

    let top_pad = line_height / 2.0;
    let bottom_pad = if member_rows == 0 && method_rows == 0 {
        0.0
    } else {
        line_height / 2.0
    };
    let rect_h = top_pad + (total_rows as f64) * line_height + bottom_pad;

    let mut rect_w = rect_w;
    if node.r#type == "group" {
        rect_w = rect_w.max(500.0);
    }

    (rect_w.max(1.0), rect_h.max(1.0))
}

fn note_dimensions(
    text: &str,
    measurer: &dyn TextMeasurer,
    text_style: &TextStyle,
    wrap_mode: WrapMode,
    padding: f64,
) -> (f64, f64) {
    let p = padding.max(0.0);
    let label = decode_entities_minimal(text);
    let m = measurer.measure_wrapped(&label, text_style, None, wrap_mode);
    (m.width + p, m.height + p)
}

fn label_metrics(
    text: &str,
    measurer: &dyn TextMeasurer,
    text_style: &TextStyle,
    wrap_mode: WrapMode,
) -> (f64, f64) {
    if text.trim().is_empty() {
        return (0.0, 0.0);
    }
    let t = decode_entities_minimal(text);
    let m = measurer.measure_wrapped(&t, text_style, None, wrap_mode);
    (m.width.max(0.0), m.height.max(0.0))
}

fn set_extras_label_metrics(extras: &mut BTreeMap<String, Value>, key: &str, w: f64, h: f64) {
    let obj = Value::Object(
        [
            ("width".to_string(), Value::from(w)),
            ("height".to_string(), Value::from(h)),
        ]
        .into_iter()
        .collect(),
    );
    extras.insert(key.to_string(), obj);
}

pub fn layout_class_diagram_v2(
    semantic: &Value,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
) -> Result<ClassDiagramV2Layout> {
    let model: ClassDiagramModel = serde_json::from_value(semantic.clone())?;

    let diagram_dir = rank_dir_from(&model.direction);
    let conf = effective_config
        .get("flowchart")
        .or_else(|| effective_config.get("class"))
        .unwrap_or(effective_config);
    let nodesep = config_f64(conf, &["nodeSpacing"]).unwrap_or(50.0);
    let ranksep = config_f64(conf, &["rankSpacing"]).unwrap_or(50.0);

    let global_html_labels = config_bool(effective_config, &["htmlLabels"]).unwrap_or(false);
    let flowchart_html_labels =
        config_bool(effective_config, &["flowchart", "htmlLabels"]).unwrap_or(true);
    let wrap_mode_node = if global_html_labels {
        WrapMode::HtmlLike
    } else {
        WrapMode::SvgLike
    };
    let wrap_mode_label = if flowchart_html_labels {
        WrapMode::HtmlLike
    } else {
        WrapMode::SvgLike
    };

    let class_padding = config_f64(effective_config, &["class", "padding"]).unwrap_or(5.0);
    let namespace_padding = config_f64(effective_config, &["flowchart", "padding"]).unwrap_or(15.0);
    let hide_empty_members_box =
        config_bool(effective_config, &["class", "hideEmptyMembersBox"]).unwrap_or(false);

    let text_style = class_text_style(effective_config);

    let mut g = Graph::<NodeLabel, EdgeLabel, GraphLabel>::new(GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    });
    g.set_graph(GraphLabel {
        rankdir: diagram_dir,
        nodesep,
        ranksep,
        edgesep: 10.0,
        ..Default::default()
    });

    for (id, _ns) in &model.namespaces {
        g.set_node(
            id.clone(),
            NodeLabel {
                width: 1.0,
                height: 1.0,
                ..Default::default()
            },
        );
    }

    for (_id, c) in &model.classes {
        let (w, h) = class_box_dimensions(
            c,
            measurer,
            &text_style,
            wrap_mode_node,
            class_padding,
            hide_empty_members_box,
        );
        g.set_node(
            c.id.clone(),
            NodeLabel {
                width: w,
                height: h,
                ..Default::default()
            },
        );
    }

    for n in &model.notes {
        let (w, h) = note_dimensions(
            &n.text,
            measurer,
            &text_style,
            wrap_mode_label,
            namespace_padding,
        );
        g.set_node(
            n.id.clone(),
            NodeLabel {
                width: w.max(1.0),
                height: h.max(1.0),
                ..Default::default()
            },
        );
    }

    if g.options().compound {
        for (_id, c) in &model.classes {
            if let Some(p) = c.parent.as_ref() {
                if model.namespaces.contains_key(p) {
                    g.set_parent(c.id.clone(), p.clone());
                }
            }
        }
    }

    for rel in &model.relations {
        let (lw, lh) = label_metrics(&rel.title, measurer, &text_style, wrap_mode_label);
        let start_text = if rel.relation_title_1 == "none" {
            String::new()
        } else {
            rel.relation_title_1.clone()
        };
        let end_text = if rel.relation_title_2 == "none" {
            String::new()
        } else {
            rel.relation_title_2.clone()
        };

        let (srw, srh) = label_metrics(&start_text, measurer, &text_style, wrap_mode_label);
        let (elw, elh) = label_metrics(&end_text, measurer, &text_style, wrap_mode_label);

        let start_marker = if rel.relation.type1 == -1 { 0.0 } else { 10.0 };
        let end_marker = if rel.relation.type2 == -1 { 0.0 } else { 10.0 };

        let mut el = EdgeLabel {
            width: lw,
            height: lh,
            labelpos: LabelPos::C,
            labeloffset: 10.0,
            minlen: 1,
            weight: 1.0,
            ..Default::default()
        };
        if srw > 0.0 && srh > 0.0 {
            set_extras_label_metrics(&mut el.extras, "startRight", srw, srh);
        }
        if elw > 0.0 && elh > 0.0 {
            set_extras_label_metrics(&mut el.extras, "endLeft", elw, elh);
        }
        el.extras
            .insert("startMarker".to_string(), Value::from(start_marker));
        el.extras
            .insert("endMarker".to_string(), Value::from(end_marker));

        g.set_edge_named(
            rel.id1.clone(),
            rel.id2.clone(),
            Some(rel.id.clone()),
            Some(el),
        );
    }

    let start_note_edge_id = model.relations.len() + 1;
    for (i, note) in model.notes.iter().enumerate() {
        let Some(class_id) = note.class_id.as_ref() else {
            continue;
        };
        if !model.classes.contains_key(class_id) {
            continue;
        }
        let edge_id = format!("edgeNote{}", start_note_edge_id + i);
        let el = EdgeLabel {
            width: 0.0,
            height: 0.0,
            labelpos: LabelPos::C,
            labeloffset: 10.0,
            minlen: 1,
            weight: 1.0,
            ..Default::default()
        };
        g.set_edge_named(note.id.clone(), class_id.clone(), Some(edge_id), Some(el));
    }

    let mut prepared = prepare_graph(g, 0)?;
    let (mut fragments, _bounds) = layout_prepared(&mut prepared)?;

    let mut node_rect_by_id: HashMap<String, Rect> = HashMap::new();
    for n in fragments.nodes.values() {
        node_rect_by_id.insert(n.id.clone(), Rect::from_center(n.x, n.y, n.width, n.height));
    }

    for (edge, terminal_meta) in fragments.edges.iter_mut() {
        let Some(meta) = terminal_meta.clone() else {
            continue;
        };
        let (from_rect, to_rect, points) = if let (Some(from), Some(to)) = (
            node_rect_by_id.get(edge.from.as_str()).copied(),
            node_rect_by_id.get(edge.to.as_str()).copied(),
        ) {
            (
                Some(from),
                Some(to),
                terminal_path_for_edge(&edge.points, from, to),
            )
        } else {
            (None, None, edge.points.clone())
        };

        if let Some((w, h)) = meta.start_left {
            if let Some((x, y)) =
                calc_terminal_label_position(meta.start_marker, TerminalPos::StartLeft, &points)
            {
                let (x, y) = from_rect
                    .map(|r| nudge_point_outside_rect(x, y, r))
                    .unwrap_or((x, y));
                edge.start_label_left = Some(LayoutLabel {
                    x,
                    y,
                    width: w,
                    height: h,
                });
            }
        }
        if let Some((w, h)) = meta.start_right {
            if let Some((x, y)) =
                calc_terminal_label_position(meta.start_marker, TerminalPos::StartRight, &points)
            {
                let (x, y) = from_rect
                    .map(|r| nudge_point_outside_rect(x, y, r))
                    .unwrap_or((x, y));
                edge.start_label_right = Some(LayoutLabel {
                    x,
                    y,
                    width: w,
                    height: h,
                });
            }
        }
        if let Some((w, h)) = meta.end_left {
            if let Some((x, y)) =
                calc_terminal_label_position(meta.end_marker, TerminalPos::EndLeft, &points)
            {
                let (x, y) = to_rect
                    .map(|r| nudge_point_outside_rect(x, y, r))
                    .unwrap_or((x, y));
                edge.end_label_left = Some(LayoutLabel {
                    x,
                    y,
                    width: w,
                    height: h,
                });
            }
        }
        if let Some((w, h)) = meta.end_right {
            if let Some((x, y)) =
                calc_terminal_label_position(meta.end_marker, TerminalPos::EndRight, &points)
            {
                let (x, y) = to_rect
                    .map(|r| nudge_point_outside_rect(x, y, r))
                    .unwrap_or((x, y));
                edge.end_label_right = Some(LayoutLabel {
                    x,
                    y,
                    width: w,
                    height: h,
                });
            }
        }
    }

    let mut leaf_rects: HashMap<String, Rect> = HashMap::new();
    for n in fragments.nodes.values() {
        leaf_rects.insert(n.id.clone(), Rect::from_center(n.x, n.y, n.width, n.height));
    }

    let mut cluster_rects: HashMap<String, Rect> = HashMap::new();
    for (id, ns) in &model.namespaces {
        let mut rect_opt: Option<Rect> = None;
        for class_id in &ns.class_ids {
            if let Some(r) = leaf_rects.get(class_id).copied() {
                if let Some(ref mut cur) = rect_opt {
                    cur.union(r);
                } else {
                    rect_opt = Some(r);
                }
            }
        }
        let Some(mut rect) = rect_opt else {
            continue;
        };
        rect.expand(namespace_padding);
        cluster_rects.insert(id.clone(), rect);
    }

    let title_margin_top = config_f64(
        effective_config,
        &["flowchart", "subGraphTitleMargin", "top"],
    )
    .unwrap_or(0.0);
    let title_margin_bottom = config_f64(
        effective_config,
        &["flowchart", "subGraphTitleMargin", "bottom"],
    )
    .unwrap_or(0.0);

    let mut clusters: Vec<LayoutCluster> = Vec::new();
    let mut nodes: Vec<LayoutNode> = Vec::new();

    for (id, r) in &cluster_rects {
        let (cx, cy) = r.center();
        let title = id.clone();
        let (tw, th) = label_metrics(&title, measurer, &text_style, wrap_mode_label);

        let base_width = r.width();
        let width = base_width.max(tw);
        let diff = if base_width <= tw {
            (tw - base_width) / 2.0 - namespace_padding / 2.0
        } else {
            -namespace_padding / 2.0
        };
        let offset_y = th - namespace_padding / 2.0;

        let title_label = LayoutLabel {
            x: cx,
            y: (cy - r.height() / 2.0) + title_margin_top + th / 2.0,
            width: tw,
            height: th,
        };

        clusters.push(LayoutCluster {
            id: id.clone(),
            x: cx,
            y: cy,
            width,
            height: r.height(),
            diff,
            offset_y,
            title: title.clone(),
            title_label,
            requested_dir: None,
            effective_dir: normalize_dir(&model.direction),
            padding: namespace_padding,
            title_margin_top,
            title_margin_bottom,
        });

        nodes.push(LayoutNode {
            id: id.clone(),
            x: cx,
            y: cy,
            width,
            height: r.height(),
            is_cluster: true,
        });
    }
    clusters.sort_by(|a, b| a.id.cmp(&b.id));

    for n in fragments.nodes.values() {
        nodes.push(n.clone());
    }
    nodes.sort_by(|a, b| a.id.cmp(&b.id));

    let mut edges: Vec<LayoutEdge> = fragments.edges.into_iter().map(|(e, _)| e).collect();
    edges.sort_by(|a, b| a.id.cmp(&b.id));

    let bounds = compute_bounds(&nodes, &edges, &clusters);

    Ok(ClassDiagramV2Layout {
        nodes,
        edges,
        clusters,
        bounds,
    })
}

fn compute_bounds(
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
    clusters: &[LayoutCluster],
) -> Option<Bounds> {
    let mut points: Vec<(f64, f64)> = Vec::new();

    for c in clusters {
        let r = Rect::from_center(c.x, c.y, c.width, c.height);
        points.push((r.min_x, r.min_y));
        points.push((r.max_x, r.max_y));
        let lr = Rect::from_center(
            c.title_label.x,
            c.title_label.y,
            c.title_label.width,
            c.title_label.height,
        );
        points.push((lr.min_x, lr.min_y));
        points.push((lr.max_x, lr.max_y));
    }

    for n in nodes {
        let r = Rect::from_center(n.x, n.y, n.width, n.height);
        points.push((r.min_x, r.min_y));
        points.push((r.max_x, r.max_y));
    }

    for e in edges {
        for p in &e.points {
            points.push((p.x, p.y));
        }
        for lbl in [
            e.label.as_ref(),
            e.start_label_left.as_ref(),
            e.start_label_right.as_ref(),
            e.end_label_left.as_ref(),
            e.end_label_right.as_ref(),
        ] {
            if let Some(l) = lbl {
                let r = Rect::from_center(l.x, l.y, l.width, l.height);
                points.push((r.min_x, r.min_y));
                points.push((r.max_x, r.max_y));
            }
        }
    }

    Bounds::from_points(points)
}

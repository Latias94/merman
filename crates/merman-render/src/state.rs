use crate::model::{
    Bounds, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint, StateDiagramV2Layout,
};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Deserialize)]
struct StateDiagramModel {
    #[serde(default = "default_dir")]
    pub direction: String,
    pub nodes: Vec<StateNode>,
    pub edges: Vec<StateEdge>,
}

fn default_dir() -> String {
    "TB".to_string()
}

#[derive(Debug, Clone, Deserialize)]
struct StateNode {
    pub id: String,
    pub label: Option<Value>,
    #[serde(default)]
    pub description: Option<Vec<String>>,
    #[serde(rename = "isGroup")]
    pub is_group: bool,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    pub dir: Option<String>,
    pub padding: Option<f64>,
    pub rx: Option<f64>,
    pub ry: Option<f64>,
    pub shape: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub position: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct StateEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    #[serde(default)]
    pub label: String,
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

fn toggle_rank_dir(dir: RankDir) -> RankDir {
    match dir {
        RankDir::TB => RankDir::LR,
        RankDir::LR => RankDir::TB,
        RankDir::BT => RankDir::RL,
        RankDir::RL => RankDir::BT,
    }
}

fn value_to_label_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .first()
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        _ => "".to_string(),
    }
}

fn state_text_style(effective_config: &Value) -> TextStyle {
    // Mermaid's state diagram CSS uses 10px by default; model it via `state.textHeight`.
    let font_size = config_f64(effective_config, &["state", "textHeight"]).unwrap_or(10.0);
    TextStyle {
        font_family: None,
        font_size,
        font_weight: None,
    }
}

struct PreparedGraph {
    graph: Graph<NodeLabel, EdgeLabel, GraphLabel>,
    extracted: BTreeMap<String, PreparedGraph>,
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
}

#[derive(Debug, Clone)]
struct EdgeSegment {
    original_id: String,
    segment: i32,
    original_from: String,
    original_to: String,
    from_cluster: Option<String>,
    to_cluster: Option<String>,
    points: Vec<LayoutPoint>,
    label: Option<LayoutLabel>,
}

#[derive(Debug, Clone)]
struct LayoutFragments {
    nodes: HashMap<String, LayoutNode>,
    edge_segments: Vec<EdgeSegment>,
}

fn get_extras_string(
    extras: &std::collections::BTreeMap<String, Value>,
    key: &str,
) -> Option<String> {
    extras
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn set_extras_string(
    extras: &mut std::collections::BTreeMap<String, Value>,
    key: &str,
    value: &str,
) {
    extras.insert(key.to_string(), Value::String(value.to_string()));
}

fn set_extras_i32(extras: &mut std::collections::BTreeMap<String, Value>, key: &str, value: i32) {
    extras.insert(key.to_string(), Value::Number(value.into()));
}

fn edge_label_metrics(
    label: &str,
    measurer: &dyn TextMeasurer,
    text_style: &TextStyle,
    wrap_mode: WrapMode,
) -> (f64, f64) {
    if label.trim().is_empty() {
        return (0.0, 0.0);
    }
    // Mermaid uses `createText(...)` for edge labels without specifying `width`, which defaults to
    // 200.
    let mut metrics = measurer.measure_wrapped(label, text_style, Some(200.0), wrap_mode);
    // For SVG edge labels, `createText(..., addSvgBackground=true)` adds a background rect with a
    // 2px padding.
    if wrap_mode == WrapMode::SvgLike {
        metrics.width += 4.0;
        metrics.height += 4.0;
    }
    (metrics.width.max(0.0), metrics.height.max(0.0))
}

fn node_label_metrics(
    label: &str,
    wrapping_width: f64,
    measurer: &dyn TextMeasurer,
    text_style: &TextStyle,
    wrap_mode: WrapMode,
) -> (f64, f64) {
    let metrics = measurer.measure_wrapped(label, text_style, Some(wrapping_width), wrap_mode);
    (metrics.width.max(0.0), metrics.height.max(0.0))
}

fn title_label_metrics(
    label: &str,
    measurer: &dyn TextMeasurer,
    text_style: &TextStyle,
    wrap_mode: WrapMode,
) -> (f64, f64) {
    // Mermaid state diagram cluster titles use `createLabel(...)` (nowrap) rather than
    // `createText(...)` (width constrained).
    let metrics = measurer.measure_wrapped(label, text_style, None, wrap_mode);
    (metrics.width.max(0.0), metrics.height.max(0.0))
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

fn find_non_cluster_child(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    id: &str,
    cluster_id: &str,
) -> Option<String> {
    let children = graph.children(id);
    if children.is_empty() {
        return Some(id.to_string());
    }
    let mut reserve: Option<String> = None;
    for child in children {
        let Some(candidate) = find_non_cluster_child(graph, child, cluster_id) else {
            continue;
        };
        let has_edge = graph.edge_keys().iter().any(|e| {
            (e.v == cluster_id && e.w == candidate) || (e.v == candidate && e.w == cluster_id)
        });
        if has_edge {
            reserve = Some(candidate);
        } else {
            return Some(candidate);
        }
    }
    reserve
}

fn prepare_graph(
    mut graph: Graph<NodeLabel, EdgeLabel, GraphLabel>,
    cluster_dir: &impl Fn(&str) -> Option<String>,
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

    let mut anchor: HashMap<String, String> = HashMap::new();
    for id in &cluster_ids {
        let Some(a) = find_non_cluster_child(&graph, id, id) else {
            continue;
        };
        anchor.insert(id.clone(), a);
    }

    // Adjust edges that point to cluster ids by rewriting them to anchor nodes.
    let edge_keys = graph.edge_keys();
    for key in edge_keys {
        let mut from_cluster: Option<String> = None;
        let mut to_cluster: Option<String> = None;
        let mut v = key.v.clone();
        let mut w = key.w.clone();

        if cluster_ids.iter().any(|c| c == &v) && *external.get(&v).unwrap_or(&false) {
            if let Some(a) = anchor.get(&v) {
                from_cluster = Some(v.clone());
                v = a.clone();
            }
        }
        if cluster_ids.iter().any(|c| c == &w) && *external.get(&w).unwrap_or(&false) {
            if let Some(a) = anchor.get(&w) {
                to_cluster = Some(w.clone());
                w = a.clone();
            }
        }

        if v == key.v && w == key.w {
            continue;
        }

        let Some(old_label) = graph.edge_by_key(&key).cloned() else {
            continue;
        };
        let _ = graph.remove_edge_key(&key);

        let mut new_label = old_label;
        if let Some(fc) = from_cluster.as_deref() {
            set_extras_string(&mut new_label.extras, "fromCluster", fc);
        }
        if let Some(tc) = to_cluster.as_deref() {
            set_extras_string(&mut new_label.extras, "toCluster", tc);
        }
        graph.set_edge_named(v, w, key.name.clone(), Some(new_label));
    }

    // Extract root clusters without external connections into subgraphs for recursive layout.
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
        let requested = cluster_dir(&cluster_id).map(|d| rank_dir_from(&d));
        let dir = requested.unwrap_or_else(|| toggle_rank_dir(parent_dir));
        let nodesep = graph.graph().nodesep;
        let ranksep = graph.graph().ranksep + 25.0;

        let mut subgraph = extract_cluster_graph(&cluster_id, &mut graph)?;
        subgraph.graph_mut().rankdir = dir;
        subgraph.graph_mut().nodesep = nodesep;
        subgraph.graph_mut().ranksep = ranksep;

        let prepared = prepare_graph(subgraph, cluster_dir, depth + 1)?;
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

    // Copy node labels.
    for id in &descendants {
        let Some(label) = graph.node(id).cloned() else {
            continue;
        };
        sub.set_node(id.clone(), label);
    }

    // Copy edges that are fully inside the cluster.
    for key in graph.edge_keys() {
        if moved_set.contains(&key.v) && moved_set.contains(&key.w) {
            if let Some(label) = graph.edge_by_key(&key).cloned() {
                sub.set_edge_named(key.v.clone(), key.w.clone(), key.name.clone(), Some(label));
            }
        }
    }

    // Copy compound relationships, excluding the cluster root itself.
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

    // Remove descendant nodes from the parent graph (also removes incident edges).
    for id in &descendants {
        let _ = graph.remove_node(id);
    }

    Ok(sub)
}

fn layout_prepared(prepared: &mut PreparedGraph) -> Result<(LayoutFragments, Rect)> {
    let mut fragments = LayoutFragments {
        nodes: HashMap::new(),
        edge_segments: Vec::new(),
    };

    // Layout extracted subgraphs first to size their placeholder nodes in the parent graph.
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
        let original_id = get_extras_string(&e.extras, "originalId").unwrap_or_else(|| {
            key.name
                .clone()
                .unwrap_or_else(|| format!("edge:{}:{}", key.v, key.w))
        });
        let segment = e
            .extras
            .get("segment")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let original_from =
            get_extras_string(&e.extras, "originalFrom").unwrap_or_else(|| key.v.clone());
        let original_to =
            get_extras_string(&e.extras, "originalTo").unwrap_or_else(|| key.w.clone());
        let from_cluster = get_extras_string(&e.extras, "fromCluster");
        let to_cluster = get_extras_string(&e.extras, "toCluster");

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

        fragments.edge_segments.push(EdgeSegment {
            original_id,
            segment,
            original_from,
            original_to,
            from_cluster,
            to_cluster,
            points,
            label,
        });
    }

    // Merge extracted fragments into this graph, translating them by the cluster placeholder
    // position.
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
        for seg in &mut sub_frag.edge_segments {
            for p in &mut seg.points {
                p.x += dx;
                p.y += dy;
            }
            if let Some(l) = seg.label.as_mut() {
                l.x += dx;
                l.y += dy;
            }
        }

        fragments.nodes.extend(sub_frag.nodes);
        fragments.edge_segments.extend(sub_frag.edge_segments);
    }

    let mut points: Vec<(f64, f64)> = Vec::new();
    for n in fragments.nodes.values() {
        let r = Rect::from_center(n.x, n.y, n.width, n.height);
        points.push((r.min_x, r.min_y));
        points.push((r.max_x, r.max_y));
    }
    for e in &fragments.edge_segments {
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

fn merge_edge_segments(mut segments: Vec<EdgeSegment>) -> Vec<LayoutEdge> {
    segments.sort_by(|a, b| {
        a.original_id
            .cmp(&b.original_id)
            .then_with(|| a.segment.cmp(&b.segment))
    });

    let mut out: Vec<LayoutEdge> = Vec::new();
    let mut i = 0usize;
    while i < segments.len() {
        let id = segments[i].original_id.clone();
        let from = segments[i].original_from.clone();
        let to = segments[i].original_to.clone();

        let mut from_cluster = segments[i].from_cluster.clone();
        let mut to_cluster = segments[i].to_cluster.clone();

        let mut points: Vec<LayoutPoint> = Vec::new();
        let mut label: Option<LayoutLabel> = None;

        while i < segments.len() && segments[i].original_id == id {
            let seg = &segments[i];
            if from_cluster.is_none() {
                from_cluster = seg.from_cluster.clone();
            }
            if to_cluster.is_none() {
                to_cluster = seg.to_cluster.clone();
            }
            if label.is_none() {
                label = seg.label.clone();
            }

            for (idx, p) in seg.points.iter().enumerate() {
                if points.is_empty() {
                    points.push(p.clone());
                    continue;
                }
                if idx == 0 {
                    let last = points.last().unwrap();
                    if (last.x - p.x).abs() < 1e-9 && (last.y - p.y).abs() < 1e-9 {
                        continue;
                    }
                }
                points.push(p.clone());
            }

            i += 1;
        }

        out.push(LayoutEdge {
            id,
            from,
            to,
            from_cluster,
            to_cluster,
            points,
            label,
        });
    }

    out
}

fn cluster_title_extra_y(title_height: f64) -> f64 {
    // Mermaid's state `roundedWithTitle` uses `innerHeight = height - titleHeight - 6`.
    title_height + 6.0
}

fn compute_cluster_rects(
    nodes_by_id: &HashMap<String, &StateNode>,
    leaf_rects: &HashMap<String, Rect>,
    measurer: &dyn TextMeasurer,
    text_style: &TextStyle,
    wrap_mode: WrapMode,
) -> Result<HashMap<String, Rect>> {
    let mut cluster_rects: HashMap<String, Rect> = HashMap::new();
    let mut visiting: HashSet<String> = HashSet::new();

    fn compute(
        id: &str,
        nodes_by_id: &HashMap<String, &StateNode>,
        leaf_rects: &HashMap<String, Rect>,
        cluster_rects: &mut HashMap<String, Rect>,
        visiting: &mut HashSet<String>,
        measurer: &dyn TextMeasurer,
        text_style: &TextStyle,
        wrap_mode: WrapMode,
    ) -> Result<Rect> {
        if let Some(r) = cluster_rects.get(id).copied() {
            return Ok(r);
        }
        if visiting.contains(id) {
            return Err(Error::InvalidModel {
                message: format!("cycle in cluster parenting at {id}"),
            });
        }
        visiting.insert(id.to_string());

        let node = nodes_by_id.get(id).ok_or_else(|| Error::InvalidModel {
            message: format!("unknown cluster id: {id}"),
        })?;
        if !node.is_group {
            return Err(Error::InvalidModel {
                message: format!("node is not a cluster: {id}"),
            });
        }

        let mut union: Option<Rect> = None;
        for (cid, child) in nodes_by_id {
            if child.parent_id.as_deref() != Some(id) {
                continue;
            }
            let child_rect = if child.is_group {
                compute(
                    cid,
                    nodes_by_id,
                    leaf_rects,
                    cluster_rects,
                    visiting,
                    measurer,
                    text_style,
                    wrap_mode,
                )?
            } else {
                *leaf_rects.get(cid).ok_or_else(|| Error::InvalidModel {
                    message: format!("missing leaf rect: {cid}"),
                })?
            };

            if let Some(u) = union.as_mut() {
                u.union(child_rect);
            } else {
                union = Some(child_rect);
            }
        }

        let mut rect = union.ok_or_else(|| Error::InvalidModel {
            message: format!("cluster has no members: {id}"),
        })?;

        let pad = node.padding.unwrap_or(8.0).max(0.0);
        let shape = node.shape.as_str();

        // Invisible grouping cluster for notes: keep tight bounds.
        if shape != "noteGroup" {
            rect.min_x -= pad;
            rect.max_x += pad;
            rect.min_y -= pad;
            rect.max_y += pad;
        }

        if shape == "roundedWithTitle" {
            let title = node
                .label
                .as_ref()
                .map(value_to_label_text)
                .unwrap_or_default();
            if !title.trim().is_empty() {
                let (_tw, th) = title_label_metrics(&title, measurer, text_style, wrap_mode);
                rect.min_y -= cluster_title_extra_y(th);
            }
        }

        visiting.remove(id);
        cluster_rects.insert(id.to_string(), rect);
        Ok(rect)
    }

    for (id, node) in nodes_by_id {
        if !node.is_group {
            continue;
        }
        let _ = compute(
            id,
            nodes_by_id,
            leaf_rects,
            &mut cluster_rects,
            &mut visiting,
            measurer,
            text_style,
            wrap_mode,
        )?;
    }

    Ok(cluster_rects)
}

pub fn layout_state_diagram_v2(
    semantic: &Value,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
) -> Result<StateDiagramV2Layout> {
    let model: StateDiagramModel = serde_json::from_value(semantic.clone())?;
    let nodes_by_id: HashMap<String, &StateNode> =
        model.nodes.iter().map(|n| (n.id.clone(), n)).collect();

    let diagram_dir = rank_dir_from(&model.direction);
    let nodesep = config_f64(effective_config, &["state", "nodeSpacing"]).unwrap_or(50.0);
    let ranksep = config_f64(effective_config, &["state", "rankSpacing"]).unwrap_or(50.0);
    let html_labels = config_bool(effective_config, &["flowchart", "htmlLabels"]).unwrap_or(true);
    let wrap_mode = if html_labels {
        WrapMode::HtmlLike
    } else {
        WrapMode::SvgLike
    };
    let wrapping_width =
        config_f64(effective_config, &["flowchart", "wrappingWidth"]).unwrap_or(200.0);
    let state_padding = config_f64(effective_config, &["state", "padding"]).unwrap_or(8.0);
    let text_style = state_text_style(effective_config);

    let cluster_dir = |id: &str| -> Option<String> {
        let n = nodes_by_id.get(id)?;
        n.dir.as_ref().map(|s| normalize_dir(s))
    };

    let mut g = Graph::<NodeLabel, EdgeLabel, GraphLabel>::new(GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    });
    g.set_graph(GraphLabel {
        rankdir: diagram_dir,
        nodesep,
        ranksep,
        ..Default::default()
    });

    // Pre-size nodes (leaf nodes only). Cluster nodes start with a tiny placeholder size.
    for n in &model.nodes {
        if n.is_group {
            g.set_node(
                n.id.clone(),
                NodeLabel {
                    width: 1.0,
                    height: 1.0,
                    ..Default::default()
                },
            );
            continue;
        }

        let padding = n.padding.unwrap_or(0.0).max(0.0);
        let label_text = n
            .label
            .as_ref()
            .map(value_to_label_text)
            .unwrap_or_else(|| n.id.clone());

        let (w, h) = match n.shape.as_str() {
            "stateStart" | "stateEnd" => (14.0, 14.0),
            "choice" => (28.0, 28.0),
            "fork" | "join" => {
                let (mut width, mut height) = if matches!(diagram_dir, RankDir::LR | RankDir::RL) {
                    (10.0, 70.0)
                } else {
                    (70.0, 10.0)
                };
                width += state_padding / 2.0;
                height += state_padding / 2.0;
                (width, height)
            }
            "note" => {
                let (tw, th) = node_label_metrics(
                    &label_text,
                    wrapping_width,
                    measurer,
                    &text_style,
                    wrap_mode,
                );
                (tw + padding * 2.0, th + padding * 2.0)
            }
            "rectWithTitle" => {
                let desc = n
                    .description
                    .as_ref()
                    .map(|v| v.join("\n"))
                    .unwrap_or_default();
                let (title_w, title_h) =
                    title_label_metrics(&label_text, measurer, &text_style, wrap_mode);
                let (desc_w, desc_h) = title_label_metrics(&desc, measurer, &text_style, wrap_mode);
                let combined_w = title_w.max(desc_w);
                let half_pad = padding / 2.0;
                let combined_h = title_h + half_pad + 5.0 + desc_h;
                (combined_w + padding, combined_h + padding)
            }
            "rect" => {
                let (tw, th) = node_label_metrics(
                    &label_text,
                    wrapping_width,
                    measurer,
                    &text_style,
                    wrap_mode,
                );
                // Mermaid converts `rect` into `roundedRect` when rx/ry is set.
                let has_rounding = n.rx.unwrap_or(0.0) > 0.0 && n.ry.unwrap_or(0.0) > 0.0;
                let pad_x = if has_rounding { padding } else { padding * 2.0 };
                let pad_y = if has_rounding { padding } else { padding };
                (tw + pad_x * 2.0, th + pad_y * 2.0)
            }
            other => {
                return Err(Error::InvalidModel {
                    message: format!("unsupported state node shape: {other}"),
                });
            }
        };

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
        for n in &model.nodes {
            if let Some(p) = n.parent_id.as_ref() {
                g.set_parent(n.id.clone(), p.clone());
            }
        }
    }

    // Add edges. For self-loops, split into 3 edges with 2 tiny dummy nodes (Mermaid wrapper
    // behavior).
    for e in &model.edges {
        let (lw, lh) = edge_label_metrics(&e.label, measurer, &text_style, wrap_mode);
        let mut base = EdgeLabel {
            width: lw,
            height: lh,
            labelpos: LabelPos::C,
            labeloffset: 10.0,
            minlen: 1,
            weight: 1.0,
            ..Default::default()
        };
        set_extras_string(&mut base.extras, "originalId", &e.id);
        set_extras_string(&mut base.extras, "originalFrom", &e.start);
        set_extras_string(&mut base.extras, "originalTo", &e.end);
        set_extras_i32(&mut base.extras, "segment", 0);

        if e.start != e.end {
            g.set_edge_named(
                e.start.clone(),
                e.end.clone(),
                Some(e.id.clone()),
                Some(base),
            );
            continue;
        }

        let node_id = e.start.clone();
        let special1 = format!("{node_id}---{id}---1", id = e.id);
        let special2 = format!("{node_id}---{id}---2", id = e.id);

        g.set_node(
            special1.clone(),
            NodeLabel {
                width: 10.0,
                height: 10.0,
                ..Default::default()
            },
        );
        g.set_node(
            special2.clone(),
            NodeLabel {
                width: 10.0,
                height: 10.0,
                ..Default::default()
            },
        );
        if let Some(parent) = g.parent(&node_id).map(|s| s.to_string()) {
            g.set_parent(special1.clone(), parent.clone());
            g.set_parent(special2.clone(), parent);
        }

        let mut edge1 = base.clone();
        edge1.width = 0.0;
        edge1.height = 0.0;
        set_extras_i32(&mut edge1.extras, "segment", 0);

        let mut edge_mid = base.clone();
        set_extras_i32(&mut edge_mid.extras, "segment", 1);

        let mut edge2 = base.clone();
        edge2.width = 0.0;
        edge2.height = 0.0;
        set_extras_i32(&mut edge2.extras, "segment", 2);

        g.set_edge_named(
            node_id.clone(),
            special1.clone(),
            Some(format!("{}-cyclic-special-0", e.id)),
            Some(edge1),
        );
        g.set_edge_named(
            special1,
            special2.clone(),
            Some(format!("{}-cyclic-special-1", e.id)),
            Some(edge_mid),
        );
        g.set_edge_named(
            special2,
            node_id,
            Some(format!("{}-cyclic-special-2", e.id)),
            Some(edge2),
        );
    }

    let mut prepared = prepare_graph(g, &cluster_dir, 0)?;
    let (fragments, _layout_bounds) = layout_prepared(&mut prepared)?;

    let semantic_ids: HashSet<&str> = model.nodes.iter().map(|n| n.id.as_str()).collect();

    // Build output nodes from semantic nodes only.
    let mut out_nodes: Vec<LayoutNode> = Vec::new();
    let mut leaf_rects: HashMap<String, Rect> = HashMap::new();
    for n in &model.nodes {
        let Some(pos) = fragments.nodes.get(&n.id) else {
            return Err(Error::InvalidModel {
                message: format!("missing positioned node: {}", n.id),
            });
        };

        if !n.is_group {
            out_nodes.push(LayoutNode {
                id: n.id.clone(),
                x: pos.x,
                y: pos.y,
                width: pos.width,
                height: pos.height,
                is_cluster: false,
            });
            leaf_rects.insert(
                n.id.clone(),
                Rect::from_center(pos.x, pos.y, pos.width, pos.height),
            );
        }
    }

    let cluster_rects =
        compute_cluster_rects(&nodes_by_id, &leaf_rects, measurer, &text_style, wrap_mode)?;

    let mut clusters: Vec<LayoutCluster> = Vec::new();
    for n in &model.nodes {
        if !n.is_group {
            continue;
        }
        let rect = *cluster_rects
            .get(&n.id)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing cluster rect: {}", n.id),
            })?;
        let (cx, cy) = rect.center();

        let title = n
            .label
            .as_ref()
            .map(value_to_label_text)
            .unwrap_or_default();
        let pad = n.padding.unwrap_or(8.0).max(0.0);
        let (tw, th) = if title.trim().is_empty() {
            (0.0, 0.0)
        } else {
            title_label_metrics(&title, measurer, &text_style, wrap_mode)
        };

        let title_top_adjust = if html_labels { 0.0 } else { 3.0 };
        let title_label = LayoutLabel {
            x: cx,
            y: rect.min_y + 1.0 - title_top_adjust + th / 2.0,
            width: tw,
            height: th,
        };

        let diff = match n.shape.as_str() {
            "divider" => -pad,
            "noteGroup" => 0.0,
            _ => {
                let padded_label_width = tw + pad;
                if rect.width() <= padded_label_width {
                    (padded_label_width - rect.width()) / 2.0 - pad
                } else {
                    -pad
                }
            }
        };
        let offset_y = if n.shape == "roundedWithTitle" {
            th - pad / 2.0
        } else {
            0.0
        };

        let requested_dir = n.dir.as_ref().map(|s| normalize_dir(s));
        let effective_dir = requested_dir
            .clone()
            .unwrap_or_else(|| normalize_dir(&model.direction));

        clusters.push(LayoutCluster {
            id: n.id.clone(),
            x: cx,
            y: cy,
            width: rect.width(),
            height: rect.height(),
            diff,
            offset_y,
            title,
            title_label,
            requested_dir,
            effective_dir,
            padding: pad,
            title_margin_top: 0.0,
            title_margin_bottom: 0.0,
        });

        out_nodes.push(LayoutNode {
            id: n.id.clone(),
            x: cx,
            y: cy,
            width: rect.width(),
            height: rect.height(),
            is_cluster: true,
        });
    }

    out_nodes.sort_by(|a, b| a.id.cmp(&b.id));
    clusters.sort_by(|a, b| a.id.cmp(&b.id));

    let mut out_edges = merge_edge_segments(
        fragments
            .edge_segments
            .into_iter()
            .filter(|s| {
                semantic_ids.contains(s.original_from.as_str())
                    && semantic_ids.contains(s.original_to.as_str())
            })
            .collect(),
    );
    out_edges.sort_by(|a, b| a.id.cmp(&b.id));

    let bounds = {
        let mut points: Vec<(f64, f64)> = Vec::new();
        for n in &out_nodes {
            let r = Rect::from_center(n.x, n.y, n.width, n.height);
            points.push((r.min_x, r.min_y));
            points.push((r.max_x, r.max_y));
        }
        for e in &out_edges {
            for p in &e.points {
                points.push((p.x, p.y));
            }
            if let Some(l) = &e.label {
                let r = Rect::from_center(l.x, l.y, l.width, l.height);
                points.push((r.min_x, r.min_y));
                points.push((r.max_x, r.max_y));
            }
        }
        Bounds::from_points(points)
    };

    Ok(StateDiagramV2Layout {
        nodes: out_nodes,
        edges: out_edges,
        clusters,
        bounds,
    })
}

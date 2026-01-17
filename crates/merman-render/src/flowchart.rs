use crate::model::{
    Bounds, FlowchartV2Layout, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FlowchartV2Model {
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "classDefs")]
    pub class_defs: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default, rename = "edgeDefaults")]
    pub edge_defaults: Option<FlowEdgeDefaults>,
    #[serde(default, rename = "vertexCalls")]
    pub vertex_calls: Vec<String>,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
    #[serde(default)]
    pub subgraphs: Vec<FlowSubgraph>,
    #[serde(default)]
    pub tooltips: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FlowEdgeDefaults {
    #[serde(default)]
    pub interpolate: Option<String>,
    #[serde(default)]
    pub style: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FlowNode {
    pub id: String,
    pub label: Option<String>,
    #[serde(default, rename = "labelType")]
    pub label_type: Option<String>,
    #[serde(rename = "layoutShape")]
    pub layout_shape: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default, rename = "linkTarget")]
    pub link_target: Option<String>,
    #[serde(default, rename = "haveCallback")]
    pub have_callback: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FlowEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    #[serde(default, rename = "labelType")]
    pub label_type: Option<String>,
    #[serde(default, rename = "type")]
    pub edge_type: Option<String>,
    #[serde(default)]
    pub stroke: Option<String>,
    #[serde(default)]
    pub interpolate: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub style: Vec<String>,
    pub length: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FlowSubgraph {
    pub id: String,
    pub title: String,
    pub dir: Option<String>,
    #[serde(default, rename = "labelType")]
    pub label_type: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
    pub nodes: Vec<String>,
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

fn rank_dir_from_flow(direction: &str) -> RankDir {
    match direction.trim().to_uppercase().as_str() {
        "TB" | "TD" => RankDir::TB,
        "BT" => RankDir::BT,
        "LR" => RankDir::LR,
        "RL" => RankDir::RL,
        _ => RankDir::TB,
    }
}

fn normalize_dir(s: &str) -> String {
    s.trim().to_uppercase()
}

fn toggled_dir(parent: &str) -> String {
    let parent = normalize_dir(parent);
    if parent == "TB" || parent == "TD" {
        "LR".to_string()
    } else {
        "TB".to_string()
    }
}

fn flow_dir_from_rankdir(rankdir: RankDir) -> &'static str {
    match rankdir {
        RankDir::TB => "TB",
        RankDir::BT => "BT",
        RankDir::LR => "LR",
        RankDir::RL => "RL",
    }
}

fn effective_cluster_dir(sg: &FlowSubgraph, parent_dir: &str, inherit_dir: bool) -> String {
    if let Some(dir) = sg.dir.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        return normalize_dir(dir);
    }
    if inherit_dir {
        return normalize_dir(parent_dir);
    }
    toggled_dir(parent_dir)
}

fn compute_effective_dir_by_id(
    subgraphs_by_id: &HashMap<String, FlowSubgraph>,
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    diagram_dir: &str,
    inherit_dir: bool,
) -> HashMap<String, String> {
    fn compute_one(
        id: &str,
        subgraphs_by_id: &HashMap<String, FlowSubgraph>,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        diagram_dir: &str,
        inherit_dir: bool,
        visiting: &mut std::collections::HashSet<String>,
        memo: &mut HashMap<String, String>,
    ) -> String {
        if let Some(dir) = memo.get(id) {
            return dir.clone();
        }
        if !visiting.insert(id.to_string()) {
            let dir = toggled_dir(diagram_dir);
            memo.insert(id.to_string(), dir.clone());
            return dir;
        }

        let parent_dir = g
            .parent(id)
            .and_then(|p| subgraphs_by_id.contains_key(p).then_some(p.to_string()))
            .map(|p| {
                compute_one(
                    &p,
                    subgraphs_by_id,
                    g,
                    diagram_dir,
                    inherit_dir,
                    visiting,
                    memo,
                )
            })
            .unwrap_or_else(|| normalize_dir(diagram_dir));

        let dir = subgraphs_by_id
            .get(id)
            .map(|sg| effective_cluster_dir(sg, &parent_dir, inherit_dir))
            .unwrap_or_else(|| toggled_dir(&parent_dir));

        memo.insert(id.to_string(), dir.clone());
        let _ = visiting.remove(id);
        dir
    }

    let mut memo: HashMap<String, String> = HashMap::new();
    let mut visiting: std::collections::HashSet<String> = std::collections::HashSet::new();
    for id in subgraphs_by_id.keys() {
        let _ = compute_one(
            id,
            subgraphs_by_id,
            g,
            diagram_dir,
            inherit_dir,
            &mut visiting,
            &mut memo,
        );
    }
    memo
}

fn dir_to_rankdir(dir: &str) -> RankDir {
    match normalize_dir(dir).as_str() {
        "TB" | "TD" => RankDir::TB,
        "BT" => RankDir::BT,
        "LR" => RankDir::LR,
        "RL" => RankDir::RL,
        _ => RankDir::TB,
    }
}

fn edge_label_is_non_empty(edge: &FlowEdge) -> bool {
    edge.label
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

fn edge_label_leaf_id(edge: &FlowEdge) -> String {
    format!("edge-label::{}", edge.id)
}

fn lowest_common_parent(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    a: &str,
    b: &str,
) -> Option<String> {
    if !g.options().compound {
        return None;
    }

    let mut ancestors: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut cur = g.parent(a);
    while let Some(p) = cur {
        ancestors.insert(p.to_string());
        cur = g.parent(p);
    }

    let mut cur = g.parent(b);
    while let Some(p) = cur {
        if ancestors.contains(p) {
            return Some(p.to_string());
        }
        cur = g.parent(p);
    }

    None
}

fn extract_descendants(id: &str, g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> Vec<String> {
    let children = g.children(id);
    let mut out: Vec<String> = children.iter().map(|s| s.to_string()).collect();
    for child in children {
        out.extend(extract_descendants(child, g));
    }
    out
}

fn edge_in_cluster(
    edge: &dugong::graphlib::EdgeKey,
    cluster_id: &str,
    descendants: &std::collections::HashMap<String, Vec<String>>,
) -> bool {
    if edge.v == cluster_id || edge.w == cluster_id {
        return false;
    }
    let Some(cluster_desc) = descendants.get(cluster_id) else {
        return false;
    };
    cluster_desc.contains(&edge.v) || cluster_desc.contains(&edge.w)
}

#[derive(Debug, Clone)]
struct FlowchartClusterDbEntry {
    anchor_id: String,
    external_connections: bool,
}

fn flowchart_find_common_edges(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    id1: &str,
    id2: &str,
) -> Vec<(String, String)> {
    let edges1: Vec<(String, String)> = graph
        .edge_keys()
        .into_iter()
        .filter(|edge| edge.v == id1 || edge.w == id1)
        .map(|edge| (edge.v, edge.w))
        .collect();
    let edges2: Vec<(String, String)> = graph
        .edge_keys()
        .into_iter()
        .filter(|edge| edge.v == id2 || edge.w == id2)
        .map(|edge| (edge.v, edge.w))
        .collect();

    let edges1_prim: Vec<(String, String)> = edges1
        .into_iter()
        .map(|(v, w)| {
            (
                if v == id1 { id2.to_string() } else { v },
                if w == id1 { id2.to_string() } else { w },
            )
        })
        .collect();

    let mut out = Vec::new();
    for e1 in edges1_prim {
        if edges2.iter().any(|e2| *e2 == e1) {
            out.push(e1);
        }
    }
    out
}

fn flowchart_find_non_cluster_child(
    id: &str,
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    cluster_id: &str,
) -> Option<String> {
    let children = graph.children(id);
    if children.is_empty() {
        return Some(id.to_string());
    }

    let mut reserve: Option<String> = None;
    for child in children {
        let Some(child_id) = flowchart_find_non_cluster_child(child, graph, cluster_id) else {
            continue;
        };
        let common_edges = flowchart_find_common_edges(graph, cluster_id, &child_id);
        if !common_edges.is_empty() {
            reserve = Some(child_id);
        } else {
            return Some(child_id);
        }
    }
    reserve
}

fn adjust_flowchart_clusters_and_edges(graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    use serde_json::Value;

    fn is_descendant(
        node_id: &str,
        cluster_id: &str,
        descendants: &std::collections::HashMap<String, Vec<String>>,
    ) -> bool {
        descendants
            .get(cluster_id)
            .is_some_and(|ds| ds.iter().any(|n| n == node_id))
    }

    let mut descendants: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut cluster_db: std::collections::HashMap<String, FlowchartClusterDbEntry> =
        std::collections::HashMap::new();

    for id in graph.node_ids() {
        if graph.children(&id).is_empty() {
            continue;
        }
        descendants.insert(id.clone(), extract_descendants(&id, graph));
        let anchor_id =
            flowchart_find_non_cluster_child(&id, graph, &id).unwrap_or_else(|| id.clone());
        cluster_db.insert(
            id,
            FlowchartClusterDbEntry {
                anchor_id,
                external_connections: false,
            },
        );
    }

    for id in cluster_db.keys().cloned().collect::<Vec<_>>() {
        if graph.children(&id).is_empty() {
            continue;
        }
        let mut has_external = false;
        for e in graph.edges() {
            let d1 = is_descendant(e.v.as_str(), id.as_str(), &descendants);
            let d2 = is_descendant(e.w.as_str(), id.as_str(), &descendants);
            if d1 ^ d2 {
                has_external = true;
                break;
            }
        }
        if let Some(entry) = cluster_db.get_mut(&id) {
            entry.external_connections = has_external;
        }
    }

    for id in cluster_db.keys().cloned().collect::<Vec<_>>() {
        let Some(non_cluster_child) = cluster_db.get(&id).map(|e| e.anchor_id.clone()) else {
            continue;
        };
        let parent = graph.parent(&non_cluster_child);
        if parent.is_some_and(|p| p != id.as_str())
            && parent.is_some_and(|p| cluster_db.contains_key(p))
            && parent.is_some_and(|p| !cluster_db.get(p).is_some_and(|e| e.external_connections))
        {
            if let Some(p) = parent {
                if let Some(entry) = cluster_db.get_mut(&id) {
                    entry.anchor_id = p.to_string();
                }
            }
        }
    }

    fn get_anchor_id(
        id: &str,
        cluster_db: &std::collections::HashMap<String, FlowchartClusterDbEntry>,
    ) -> String {
        let Some(entry) = cluster_db.get(id) else {
            return id.to_string();
        };
        if !entry.external_connections {
            return id.to_string();
        }
        entry.anchor_id.clone()
    }

    let edge_keys = graph.edge_keys();
    for ek in edge_keys {
        if !cluster_db.contains_key(&ek.v) && !cluster_db.contains_key(&ek.w) {
            continue;
        }

        let Some(mut edge_label) = graph.edge_by_key(&ek).cloned() else {
            continue;
        };

        let v = get_anchor_id(&ek.v, &cluster_db);
        let w = get_anchor_id(&ek.w, &cluster_db);

        // Match Mermaid `adjustClustersAndEdges`: edges that touch cluster nodes are removed and
        // re-inserted even when their endpoints do not change. This affects edge iteration order
        // and therefore cycle-breaking determinism in Dagre's acyclic pass.
        let _ = graph.remove_edge_key(&ek);

        if v != ek.v {
            if let Some(parent) = graph.parent(&v) {
                if let Some(entry) = cluster_db.get_mut(parent) {
                    entry.external_connections = true;
                }
            }
            edge_label
                .extras
                .insert("fromCluster".to_string(), Value::String(ek.v.clone()));
        }

        if w != ek.w {
            if let Some(parent) = graph.parent(&w) {
                if let Some(entry) = cluster_db.get_mut(parent) {
                    entry.external_connections = true;
                }
            }
            edge_label
                .extras
                .insert("toCluster".to_string(), Value::String(ek.w.clone()));
        }

        graph.set_edge_named(v, w, ek.name, Some(edge_label));
    }
}

fn copy_cluster(
    cluster_id: &str,
    graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    new_graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    root_id: &str,
    descendants: &std::collections::HashMap<String, Vec<String>>,
) {
    let mut nodes: Vec<String> = graph
        .children(cluster_id)
        .iter()
        .map(|s| s.to_string())
        .collect();
    if cluster_id != root_id {
        nodes.push(cluster_id.to_string());
    }

    for node in nodes {
        if !graph.has_node(&node) {
            continue;
        }

        if !graph.children(&node).is_empty() {
            copy_cluster(&node, graph, new_graph, root_id, descendants);
        } else {
            let data = graph.node(&node).cloned().unwrap_or_default();
            new_graph.set_node(node.clone(), data);

            if let Some(parent) = graph.parent(&node) {
                if parent != root_id {
                    new_graph.set_parent(node.clone(), parent.to_string());
                }
            }
            if cluster_id != root_id && node != cluster_id {
                new_graph.set_parent(node.clone(), cluster_id.to_string());
            }

            // Copy edges that are internal to this cluster. Mermaid performs this while iterating
            // nodes because the source graph is mutated (nodes and incident edges are removed).
            let edge_keys = graph.edge_keys();
            for ek in edge_keys {
                if !edge_in_cluster(&ek, root_id, descendants) {
                    continue;
                }
                if new_graph.has_edge(&ek.v, &ek.w, ek.name.as_deref()) {
                    continue;
                }
                let Some(lbl) = graph.edge_by_key(&ek).cloned() else {
                    continue;
                };
                new_graph.set_edge_named(ek.v, ek.w, ek.name, Some(lbl));
            }
        }

        let _ = graph.remove_node(&node);
    }
}

fn extract_clusters_recursively(
    graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    subgraphs_by_id: &std::collections::HashMap<String, FlowSubgraph>,
    effective_dir_by_id: &std::collections::HashMap<String, String>,
    extracted: &mut std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
    depth: usize,
) {
    if depth > 10 {
        return;
    }

    let node_ids = graph.node_ids();
    let mut descendants: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for id in &node_ids {
        if graph.children(id).is_empty() {
            continue;
        }
        descendants.insert(id.clone(), extract_descendants(id, graph));
    }

    let mut external: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    for id in descendants.keys() {
        let Some(ds) = descendants.get(id) else {
            continue;
        };
        let mut has_external = false;
        for e in graph.edges() {
            let d1 = ds.contains(&e.v);
            let d2 = ds.contains(&e.w);
            if d1 ^ d2 {
                has_external = true;
                break;
            }
        }
        external.insert(id.clone(), has_external);
    }

    let mut extracted_here: Vec<(String, Graph<NodeLabel, EdgeLabel, GraphLabel>)> = Vec::new();

    let candidates: Vec<String> = node_ids
        .into_iter()
        .filter(|id| graph.has_node(id))
        .filter(|id| !graph.children(id).is_empty())
        .filter(|id| !external.get(id).copied().unwrap_or(false))
        .collect();

    for id in candidates {
        if !graph.has_node(&id) {
            continue;
        }
        if graph.children(&id).is_empty() {
            continue;
        }

        let mut cluster_graph: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
            directed: true,
        });

        // Mermaid's `extractor(...)` uses:
        // - `clusterData.dir` when explicitly set for the subgraph
        // - otherwise: toggle relative to the current graph's rankdir (TB<->LR)
        let dir = subgraphs_by_id
            .get(&id)
            .and_then(|sg| sg.dir.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(normalize_dir)
            .unwrap_or_else(|| toggled_dir(flow_dir_from_rankdir(graph.graph().rankdir)));

        cluster_graph.set_graph(GraphLabel {
            rankdir: dir_to_rankdir(&dir),
            nodesep: 50.0,
            ranksep: 50.0,
            edgesep: 20.0,
            acyclicer: None,
            ..Default::default()
        });

        copy_cluster(&id, graph, &mut cluster_graph, &id, &descendants);
        extracted_here.push((id, cluster_graph));
    }

    for (id, mut g) in extracted_here {
        extract_clusters_recursively(
            &mut g,
            subgraphs_by_id,
            effective_dir_by_id,
            extracted,
            depth + 1,
        );
        extracted.insert(id, g);
    }
}

pub fn layout_flowchart_v2(
    semantic: &Value,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
) -> Result<FlowchartV2Layout> {
    let model: FlowchartV2Model = serde_json::from_value(semantic.clone())?;

    // Mermaid's dagre adapter expands self-loop edges into a chain of two special label nodes plus
    // three edges. This avoids `v == w` edges in Dagre and is required for SVG parity (Mermaid
    // uses `*-cyclic-special-*` ids when rendering self-loops).
    let mut render_edges: Vec<FlowEdge> = Vec::new();
    let mut self_loop_label_node_ids: Vec<String> = Vec::new();
    let mut self_loop_label_node_id_set: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for e in &model.edges {
        if e.from != e.to {
            render_edges.push(e.clone());
            continue;
        }

        let node_id = e.from.clone();
        let special_id_1 = format!("{node_id}---{node_id}---1");
        let special_id_2 = format!("{node_id}---{node_id}---2");
        if self_loop_label_node_id_set.insert(special_id_1.clone()) {
            self_loop_label_node_ids.push(special_id_1.clone());
        }
        if self_loop_label_node_id_set.insert(special_id_2.clone()) {
            self_loop_label_node_ids.push(special_id_2.clone());
        }

        let mut edge1 = e.clone();
        edge1.id = format!("{node_id}-cyclic-special-1");
        edge1.from = node_id.clone();
        edge1.to = special_id_1.clone();
        edge1.label = None;
        edge1.label_type = None;
        edge1.edge_type = Some("arrow_open".to_string());

        let mut edge_mid = e.clone();
        edge_mid.id = format!("{node_id}-cyclic-special-mid");
        edge_mid.from = special_id_1.clone();
        edge_mid.to = special_id_2.clone();
        edge_mid.label = None;
        edge_mid.label_type = None;
        edge_mid.edge_type = Some("arrow_open".to_string());

        let mut edge2 = e.clone();
        edge2.id = format!("{node_id}-cyclic-special-2");
        edge2.from = special_id_2.clone();
        edge2.to = node_id.clone();
        edge2.label = None;
        edge2.label_type = None;

        render_edges.push(edge1);
        render_edges.push(edge_mid);
        render_edges.push(edge2);
    }

    let nodesep = config_f64(effective_config, &["flowchart", "nodeSpacing"]).unwrap_or(50.0);
    let ranksep = config_f64(effective_config, &["flowchart", "rankSpacing"]).unwrap_or(50.0);
    let node_padding = config_f64(effective_config, &["flowchart", "padding"]).unwrap_or(15.0);
    let wrapping_width =
        config_f64(effective_config, &["flowchart", "wrappingWidth"]).unwrap_or(200.0);
    // Mermaid subgraph labels are rendered via `createText(...)` without an explicit `width`,
    // which defaults to 200.
    let cluster_title_wrapping_width = 200.0;
    let html_labels = effective_config
        .get("flowchart")
        .and_then(|v| v.get("htmlLabels"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let wrap_mode = if html_labels {
        WrapMode::HtmlLike
    } else {
        WrapMode::SvgLike
    };
    let cluster_padding = 8.0;
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
    let title_total_margin = title_margin_top + title_margin_bottom;
    let y_shift = title_total_margin / 2.0;
    let inherit_dir = effective_config
        .get("flowchart")
        .and_then(|v| v.get("inheritDir"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let font_family = config_string(effective_config, &["fontFamily"]);
    let font_size = config_f64(effective_config, &["fontSize"]).unwrap_or(16.0);
    let font_weight = config_string(effective_config, &["fontWeight"]);
    let text_style = TextStyle {
        font_family,
        font_size,
        font_weight,
    };

    let diagram_direction = normalize_dir(model.direction.as_deref().unwrap_or("TB"));
    let has_subgraphs = !model.subgraphs.is_empty();
    let mut subgraphs_by_id: std::collections::HashMap<String, FlowSubgraph> =
        std::collections::HashMap::new();
    for sg in &model.subgraphs {
        subgraphs_by_id.insert(sg.id.clone(), sg.clone());
    }
    let subgraph_ids: std::collections::HashSet<&str> =
        model.subgraphs.iter().map(|sg| sg.id.as_str()).collect();
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        // Mermaid's Dagre adapter always enables `compound: true`, even if there are no explicit
        // subgraphs. This also allows `nestingGraph.run` to connect components during ranking.
        compound: true,
        directed: true,
    });
    g.set_graph(GraphLabel {
        rankdir: rank_dir_from_flow(&diagram_direction),
        nodesep,
        ranksep,
        // Dagre layout defaults `edgesep` to 20 when unspecified.
        edgesep: 20.0,
        acyclicer: None,
        ..Default::default()
    });

    // Mermaid's flowchart Dagre adapter inserts subgraph ("group") nodes before laying out the
    // leaf nodes they contain.
    for sg in &model.subgraphs {
        let metrics = measurer.measure_wrapped(
            &sg.title,
            &text_style,
            Some(cluster_title_wrapping_width),
            wrap_mode,
        );
        let width = metrics.width + cluster_padding * 2.0;
        let height = metrics.height + cluster_padding * 2.0;
        g.set_node(
            sg.id.clone(),
            NodeLabel {
                width,
                height,
                ..Default::default()
            },
        );
    }

    for n in &model.nodes {
        // Mermaid treats the subgraph id as the "group node" id (a cluster can be referenced in
        // edges). Avoid introducing a separate leaf node that would collide with the cluster node
        // of the same id.
        if subgraph_ids.contains(n.id.as_str()) {
            continue;
        }
        let label = n.label.as_deref().unwrap_or(&n.id);
        let metrics = measurer.measure_wrapped(label, &text_style, Some(wrapping_width), wrap_mode);
        let (width, height) = node_dimensions(n.layout_shape.as_deref(), metrics, node_padding);
        g.set_node(
            n.id.clone(),
            NodeLabel {
                width,
                height,
                ..Default::default()
            },
        );
    }

    if has_subgraphs {
        for sg in &model.subgraphs {
            for child in &sg.nodes {
                g.set_parent(child.clone(), sg.id.clone());
            }
        }
    }

    // Materialize self-loop helper label nodes and place them in the same parent cluster as the
    // base node (if any), matching Mermaid `@11.12.2` dagre layout adapter behavior.
    for id in &self_loop_label_node_ids {
        if !g.has_node(id) {
            g.set_node(
                id.clone(),
                NodeLabel {
                    width: 10.0,
                    height: 10.0,
                    ..Default::default()
                },
            );
        }
        let Some((base, _)) = id.split_once("---") else {
            continue;
        };
        if let Some(p) = g.parent(base) {
            g.set_parent(id.clone(), p.to_string());
        }
    }

    let effective_dir_by_id = if has_subgraphs {
        compute_effective_dir_by_id(&subgraphs_by_id, &g, &diagram_direction, inherit_dir)
    } else {
        HashMap::new()
    };

    for e in &render_edges {
        if edge_label_is_non_empty(e) {
            let label_text = e.label.as_deref().unwrap_or_default();
            let metrics =
                measurer.measure_wrapped(label_text, &text_style, Some(wrapping_width), wrap_mode);
            let label_width = metrics.width.max(1.0);
            let label_height = metrics.height.max(1.0);

            let minlen = e.length.max(1);
            let el = EdgeLabel {
                width: label_width,
                height: label_height,
                labelpos: LabelPos::C,
                // Dagre layout defaults `labeloffset` to 10 when unspecified.
                labeloffset: 10.0,
                minlen,
                weight: 1.0,
                ..Default::default()
            };

            g.set_edge_named(e.from.clone(), e.to.clone(), Some(e.id.clone()), Some(el));
        } else {
            let el = EdgeLabel {
                width: 0.0,
                height: 0.0,
                labelpos: LabelPos::C,
                // Dagre layout defaults `labeloffset` to 10 when unspecified.
                labeloffset: 10.0,
                minlen: e.length.max(1),
                weight: 1.0,
                ..Default::default()
            };
            g.set_edge_named(e.from.clone(), e.to.clone(), Some(e.id.clone()), Some(el));
        }
    }

    if has_subgraphs {
        adjust_flowchart_clusters_and_edges(&mut g);
    }

    let mut edge_endpoints_by_id: HashMap<String, (String, String)> = HashMap::new();
    for ek in g.edge_keys() {
        let Some(edge_id) = ek.name.as_deref() else {
            continue;
        };
        edge_endpoints_by_id.insert(edge_id.to_string(), (ek.v.clone(), ek.w.clone()));
    }

    let mut extracted_graphs: std::collections::HashMap<
        String,
        Graph<NodeLabel, EdgeLabel, GraphLabel>,
    > = std::collections::HashMap::new();
    if has_subgraphs {
        extract_clusters_recursively(
            &mut g,
            &subgraphs_by_id,
            &effective_dir_by_id,
            &mut extracted_graphs,
            0,
        );
    }

    dugong::layout_dagreish(&mut g);

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

        fn translate(&mut self, dx: f64, dy: f64) {
            self.min_x += dx;
            self.max_x += dx;
            self.min_y += dy;
            self.max_y += dy;
        }
    }

    let mut leaf_rects: std::collections::HashMap<String, Rect> = std::collections::HashMap::new();
    let mut base_pos: std::collections::HashMap<String, (f64, f64)> =
        std::collections::HashMap::new();
    let mut edge_override_points: std::collections::HashMap<String, Vec<LayoutPoint>> =
        std::collections::HashMap::new();
    let mut edge_override_label: std::collections::HashMap<String, Option<LayoutLabel>> =
        std::collections::HashMap::new();
    let mut edge_override_from_cluster: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    let mut edge_override_to_cluster: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    let mut edge_packed_shift: std::collections::HashMap<String, (f64, f64)> =
        std::collections::HashMap::new();

    for cg in extracted_graphs.values_mut() {
        dugong::layout_dagreish(cg);
    }

    let mut leaf_node_ids: std::collections::HashSet<String> = model
        .nodes
        .iter()
        .filter(|n| !subgraph_ids.contains(n.id.as_str()))
        .map(|n| n.id.clone())
        .collect();
    for id in &self_loop_label_node_ids {
        leaf_node_ids.insert(id.clone());
    }

    fn graph_content_rect(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> Option<Rect> {
        let mut out: Option<Rect> = None;
        for id in g.node_ids() {
            let Some(n) = g.node(&id) else { continue };
            let (Some(x), Some(y)) = (n.x, n.y) else {
                continue;
            };
            let r = Rect::from_center(x, y, n.width, n.height);
            if let Some(ref mut cur) = out {
                cur.union(r);
            } else {
                out = Some(r);
            }
        }
        for ek in g.edge_keys() {
            let Some(e) = g.edge_by_key(&ek) else {
                continue;
            };
            let (Some(x), Some(y)) = (e.x, e.y) else {
                continue;
            };
            if e.width <= 0.0 && e.height <= 0.0 {
                continue;
            }
            let r = Rect::from_center(x, y, e.width, e.height);
            if let Some(ref mut cur) = out {
                cur.union(r);
            } else {
                out = Some(r);
            }
        }
        out
    }

    fn place_graph(
        graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        offset: (f64, f64),
        is_root: bool,
        extracted_graphs: &std::collections::HashMap<
            String,
            Graph<NodeLabel, EdgeLabel, GraphLabel>,
        >,
        leaf_node_ids: &std::collections::HashSet<String>,
        y_shift: f64,
        base_pos: &mut std::collections::HashMap<String, (f64, f64)>,
        leaf_rects: &mut std::collections::HashMap<String, Rect>,
        edge_override_points: &mut std::collections::HashMap<String, Vec<LayoutPoint>>,
        edge_override_label: &mut std::collections::HashMap<String, Option<LayoutLabel>>,
        edge_override_from_cluster: &mut std::collections::HashMap<String, Option<String>>,
        edge_override_to_cluster: &mut std::collections::HashMap<String, Option<String>>,
    ) {
        for id in graph.node_ids() {
            if !leaf_node_ids.contains(&id) {
                continue;
            }
            let Some(n) = graph.node(&id) else { continue };
            let x = n.x.unwrap_or(0.0) + offset.0;
            let y = n.y.unwrap_or(0.0) + offset.1;
            base_pos.insert(id.clone(), (x, y));
            leaf_rects.insert(id, Rect::from_center(x, y, n.width, n.height));
        }

        for ek in graph.edge_keys() {
            let Some(edge_id) = ek.name.as_deref() else {
                continue;
            };
            let Some(lbl) = graph.edge_by_key(&ek) else {
                continue;
            };

            if let (Some(x), Some(y)) = (lbl.x, lbl.y) {
                if lbl.width > 0.0 || lbl.height > 0.0 {
                    let lx = x + offset.0;
                    let ly = y + offset.1;
                    let leaf_id = format!("edge-label::{edge_id}");
                    base_pos.insert(leaf_id.clone(), (lx, ly));
                    leaf_rects.insert(leaf_id, Rect::from_center(lx, ly, lbl.width, lbl.height));
                }
            }

            if !is_root {
                let points = lbl
                    .points
                    .iter()
                    .map(|p| LayoutPoint {
                        x: p.x + offset.0,
                        y: p.y + offset.1 + y_shift,
                    })
                    .collect::<Vec<_>>();
                let label_pos = match (lbl.x, lbl.y) {
                    (Some(x), Some(y)) if lbl.width > 0.0 || lbl.height > 0.0 => {
                        Some(LayoutLabel {
                            x: x + offset.0,
                            y: y + offset.1 + y_shift,
                            width: lbl.width,
                            height: lbl.height,
                        })
                    }
                    _ => None,
                };
                edge_override_points.insert(edge_id.to_string(), points);
                edge_override_label.insert(edge_id.to_string(), label_pos);
                let from_cluster = lbl
                    .extras
                    .get("fromCluster")
                    .and_then(|v| v.as_str().map(|s| s.to_string()));
                let to_cluster = lbl
                    .extras
                    .get("toCluster")
                    .and_then(|v| v.as_str().map(|s| s.to_string()));
                edge_override_from_cluster.insert(edge_id.to_string(), from_cluster);
                edge_override_to_cluster.insert(edge_id.to_string(), to_cluster);
            }
        }

        for id in graph.node_ids() {
            let Some(child) = extracted_graphs.get(&id) else {
                continue;
            };
            let Some(n) = graph.node(&id) else {
                continue;
            };
            let (Some(px), Some(py)) = (n.x, n.y) else {
                continue;
            };
            let parent_x = px + offset.0;
            let parent_y = py + offset.1;
            let Some(r) = graph_content_rect(child) else {
                continue;
            };
            let (cx, cy) = r.center();
            let child_offset = (parent_x - cx, parent_y - cy);
            place_graph(
                child,
                child_offset,
                false,
                extracted_graphs,
                leaf_node_ids,
                y_shift,
                base_pos,
                leaf_rects,
                edge_override_points,
                edge_override_label,
                edge_override_from_cluster,
                edge_override_to_cluster,
            );
        }
    }

    place_graph(
        &g,
        (0.0, 0.0),
        true,
        &extracted_graphs,
        &leaf_node_ids,
        y_shift,
        &mut base_pos,
        &mut leaf_rects,
        &mut edge_override_points,
        &mut edge_override_label,
        &mut edge_override_from_cluster,
        &mut edge_override_to_cluster,
    );

    let mut extra_children: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let labeled_edges: std::collections::HashSet<&str> = render_edges
        .iter()
        .filter(|e| edge_label_is_non_empty(e))
        .map(|e| e.id.as_str())
        .collect();

    fn collect_extra_children(
        graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        labeled_edges: &std::collections::HashSet<&str>,
        implicit_root: Option<&str>,
        out: &mut std::collections::HashMap<String, Vec<String>>,
    ) {
        for ek in graph.edge_keys() {
            let Some(edge_id) = ek.name.as_deref() else {
                continue;
            };
            if !labeled_edges.contains(edge_id) {
                continue;
            }
            // Mermaid's recursive cluster extractor removes the root cluster node from the
            // extracted graph. In that case, the "lowest common parent" of edges whose endpoints
            // belong to the extracted cluster becomes `None`, even though the label should still
            // participate in the extracted cluster's bounding box. Use `implicit_root` to map
            // those labels back to the extracted cluster id.
            let parent = lowest_common_parent(graph, &ek.v, &ek.w)
                .or_else(|| implicit_root.map(|s| s.to_string()));
            let Some(parent) = parent else {
                continue;
            };
            out.entry(parent)
                .or_default()
                .push(format!("edge-label::{edge_id}"));
        }
    }

    collect_extra_children(&g, &labeled_edges, None, &mut extra_children);
    for (cluster_id, cg) in &extracted_graphs {
        collect_extra_children(
            cg,
            &labeled_edges,
            Some(cluster_id.as_str()),
            &mut extra_children,
        );
    }

    // Ensure Mermaid-style self-loop helper nodes participate in cluster bounding/packing.
    // These nodes are not part of the semantic `subgraph ... end` membership list, but are
    // parented into the same clusters as their base node.
    for id in &self_loop_label_node_ids {
        if let Some(p) = g.parent(id) {
            extra_children
                .entry(p.to_string())
                .or_default()
                .push(id.clone());
        }
    }

    // The Mermaid-like recursive cluster behavior above can increase the effective size of a root
    // cluster (e.g. toggling TB->LR). Mermaid accounts for that via clusterNodes and then lays out
    // the top-level graph using those expanded dimensions.
    //
    // Our headless output keeps all members in one graph, so apply a deterministic packing step
    // for root, isolated clusters to avoid overlaps after the recursive step.
    //
    // This is especially visible when multiple disconnected subgraphs exist: their member nodes
    // do not overlap, but the post-layout cluster padding/title extents can.
    {
        let subgraph_ids: std::collections::HashSet<&str> =
            model.subgraphs.iter().map(|s| s.id.as_str()).collect();

        let mut subgraph_has_parent: std::collections::HashSet<&str> =
            std::collections::HashSet::new();
        for sg in &model.subgraphs {
            for child in &sg.nodes {
                if subgraph_ids.contains(child.as_str()) {
                    subgraph_has_parent.insert(child.as_str());
                }
            }
        }

        fn collect_descendant_leaf_nodes<'a>(
            id: &'a str,
            subgraphs_by_id: &'a std::collections::HashMap<String, FlowSubgraph>,
            subgraph_ids: &std::collections::HashSet<&'a str>,
            out: &mut std::collections::HashSet<String>,
            visiting: &mut std::collections::HashSet<&'a str>,
        ) {
            if !visiting.insert(id) {
                return;
            }
            let Some(sg) = subgraphs_by_id.get(id) else {
                visiting.remove(id);
                return;
            };
            for member in &sg.nodes {
                if subgraph_ids.contains(member.as_str()) {
                    collect_descendant_leaf_nodes(
                        member,
                        subgraphs_by_id,
                        subgraph_ids,
                        out,
                        visiting,
                    );
                } else {
                    out.insert(member.clone());
                }
            }
            visiting.remove(id);
        }

        fn has_external_edges(
            leaves: &std::collections::HashSet<String>,
            edges: &[FlowEdge],
        ) -> bool {
            for e in edges {
                let in_from = leaves.contains(&e.from);
                let in_to = leaves.contains(&e.to);
                if in_from ^ in_to {
                    return true;
                }
            }
            false
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum PackAxis {
            X,
            Y,
        }

        let pack_axis = match diagram_direction.as_str() {
            "LR" | "RL" => PackAxis::Y,
            _ => PackAxis::X,
        };

        let mut pack_rects: std::collections::HashMap<String, Rect> =
            std::collections::HashMap::new();
        let mut pack_visiting: std::collections::HashSet<String> = std::collections::HashSet::new();

        fn compute_pack_rect(
            id: &str,
            subgraphs_by_id: &std::collections::HashMap<String, FlowSubgraph>,
            leaf_rects: &std::collections::HashMap<String, Rect>,
            extra_children: &std::collections::HashMap<String, Vec<String>>,
            pack_rects: &mut std::collections::HashMap<String, Rect>,
            pack_visiting: &mut std::collections::HashSet<String>,
            measurer: &dyn TextMeasurer,
            text_style: &TextStyle,
            title_wrapping_width: f64,
            wrap_mode: WrapMode,
            cluster_padding: f64,
            title_total_margin: f64,
            node_padding: f64,
        ) -> Option<Rect> {
            if let Some(r) = pack_rects.get(id).copied() {
                return Some(r);
            }
            if !pack_visiting.insert(id.to_string()) {
                return None;
            }
            let Some(sg) = subgraphs_by_id.get(id) else {
                pack_visiting.remove(id);
                return None;
            };

            let mut content: Option<Rect> = None;
            for member in &sg.nodes {
                let member_rect = if let Some(r) = leaf_rects.get(member).copied() {
                    Some(r)
                } else if subgraphs_by_id.contains_key(member) {
                    compute_pack_rect(
                        member,
                        subgraphs_by_id,
                        leaf_rects,
                        extra_children,
                        pack_rects,
                        pack_visiting,
                        measurer,
                        text_style,
                        title_wrapping_width,
                        wrap_mode,
                        cluster_padding,
                        title_total_margin,
                        node_padding,
                    )
                } else {
                    None
                };

                if let Some(r) = member_rect {
                    if let Some(ref mut cur) = content {
                        cur.union(r);
                    } else {
                        content = Some(r);
                    }
                }
            }

            if let Some(extra) = extra_children.get(id) {
                for child in extra {
                    if let Some(r) = leaf_rects.get(child).copied() {
                        if let Some(ref mut cur) = content {
                            cur.union(r);
                        } else {
                            content = Some(r);
                        }
                    }
                }
            }

            let title_metrics = measurer.measure_wrapped(
                &sg.title,
                text_style,
                Some(title_wrapping_width),
                wrap_mode,
            );

            let mut rect = if let Some(r) = content {
                r
            } else {
                Rect::from_center(
                    0.0,
                    0.0,
                    title_metrics.width.max(1.0),
                    title_metrics.height.max(1.0),
                )
            };

            rect.min_x -= cluster_padding;
            rect.max_x += cluster_padding;
            rect.min_y -= cluster_padding;
            rect.max_y += cluster_padding;

            let min_width = title_metrics.width + node_padding;
            if rect.width() < min_width {
                let (cx, cy) = rect.center();
                rect = Rect::from_center(cx, cy, min_width, rect.height());
            }

            if title_total_margin > 0.0 {
                let (cx, cy) = rect.center();
                rect = Rect::from_center(cx, cy, rect.width(), rect.height() + title_total_margin);
            }

            let min_height = title_metrics.height + cluster_padding * 2.0 + title_total_margin;
            if rect.height() < min_height {
                let (cx, cy) = rect.center();
                rect = Rect::from_center(cx, cy, rect.width(), min_height);
            }

            pack_visiting.remove(id);
            pack_rects.insert(id.to_string(), rect);
            Some(rect)
        }

        struct PackItem {
            rect: Rect,
            members: Vec<String>,
            internal_edge_ids: Vec<String>,
        }

        let mut items: Vec<PackItem> = Vec::new();
        for sg in &model.subgraphs {
            if subgraph_has_parent.contains(sg.id.as_str()) {
                continue;
            }

            let mut leaves: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut visiting: std::collections::HashSet<&str> = std::collections::HashSet::new();
            collect_descendant_leaf_nodes(
                &sg.id,
                &subgraphs_by_id,
                &subgraph_ids,
                &mut leaves,
                &mut visiting,
            );
            if leaves.is_empty() {
                continue;
            }
            if has_external_edges(&leaves, &render_edges) {
                continue;
            }

            let Some(rect) = compute_pack_rect(
                &sg.id,
                &subgraphs_by_id,
                &leaf_rects,
                &extra_children,
                &mut pack_rects,
                &mut pack_visiting,
                measurer,
                &text_style,
                cluster_title_wrapping_width,
                wrap_mode,
                cluster_padding,
                title_total_margin,
                node_padding,
            ) else {
                continue;
            };

            let mut members = leaves.iter().cloned().collect::<Vec<_>>();
            if let Some(extra) = extra_children.get(&sg.id) {
                members.extend(extra.iter().cloned());
            }

            // Ensure internal labeled edge nodes participate in translation.
            let mut internal_edge_ids: Vec<String> = Vec::new();
            for e in &render_edges {
                if leaves.contains(&e.from) && leaves.contains(&e.to) {
                    internal_edge_ids.push(e.id.clone());
                    if edge_label_is_non_empty(e) {
                        members.push(edge_label_leaf_id(e));
                    }
                }
            }

            items.push(PackItem {
                rect,
                members,
                internal_edge_ids,
            });
        }

        if !items.is_empty() {
            items.sort_by(|a, b| match pack_axis {
                PackAxis::X => a.rect.min_x.total_cmp(&b.rect.min_x),
                PackAxis::Y => a.rect.min_y.total_cmp(&b.rect.min_y),
            });

            let mut cursor = match pack_axis {
                PackAxis::X => items.first().unwrap().rect.min_x,
                PackAxis::Y => items.first().unwrap().rect.min_y,
            };

            for item in items {
                let (cx, cy) = item.rect.center();
                let desired_center = match pack_axis {
                    PackAxis::X => cursor + item.rect.width() / 2.0,
                    PackAxis::Y => cursor + item.rect.height() / 2.0,
                };
                let (dx, dy) = match pack_axis {
                    PackAxis::X => (desired_center - cx, 0.0),
                    PackAxis::Y => (0.0, desired_center - cy),
                };

                if dx.abs() > 1e-6 || dy.abs() > 1e-6 {
                    for id in &item.members {
                        if let Some((x, y)) = base_pos.get_mut(id) {
                            *x += dx;
                            *y += dy;
                        }
                        if let Some(r) = leaf_rects.get_mut(id) {
                            r.translate(dx, dy);
                        }
                    }
                    for edge_id in &item.internal_edge_ids {
                        edge_packed_shift.insert(edge_id.clone(), (dx, dy));
                    }
                }

                cursor += match pack_axis {
                    PackAxis::X => item.rect.width() + nodesep,
                    PackAxis::Y => item.rect.height() + nodesep,
                };
            }
        }
    }

    let mut out_nodes: Vec<LayoutNode> = Vec::new();
    for n in &model.nodes {
        if subgraph_ids.contains(n.id.as_str()) {
            continue;
        }
        let (x, y) = base_pos
            .get(&n.id)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing positioned node {}", n.id),
            })?;
        let (width, height) = leaf_rects
            .get(&n.id)
            .map(|r| (r.width(), r.height()))
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing sized node {}", n.id),
            })?;
        out_nodes.push(LayoutNode {
            id: n.id.clone(),
            x,
            // Mermaid shifts regular nodes by `subGraphTitleTotalMargin / 2` after Dagre layout.
            y: y + y_shift,
            width,
            height,
            is_cluster: false,
        });
    }
    for id in &self_loop_label_node_ids {
        let (x, y) = base_pos
            .get(id)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing positioned node {id}"),
            })?;
        let (width, height) = leaf_rects
            .get(id)
            .map(|r| (r.width(), r.height()))
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing sized node {id}"),
            })?;
        out_nodes.push(LayoutNode {
            id: id.clone(),
            x,
            // Treat self-loop helper nodes like regular nodes for the subgraph-title y-shift.
            y: y + y_shift,
            width,
            height,
            is_cluster: false,
        });
    }

    let mut clusters: Vec<LayoutCluster> = Vec::new();

    let mut cluster_rects: std::collections::HashMap<String, Rect> =
        std::collections::HashMap::new();
    let mut cluster_base_widths: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();
    let mut visiting: std::collections::HashSet<String> = std::collections::HashSet::new();

    fn compute_cluster_rect(
        id: &str,
        subgraphs_by_id: &std::collections::HashMap<String, FlowSubgraph>,
        leaf_rects: &std::collections::HashMap<String, Rect>,
        extra_children: &std::collections::HashMap<String, Vec<String>>,
        cluster_rects: &mut std::collections::HashMap<String, Rect>,
        cluster_base_widths: &mut std::collections::HashMap<String, f64>,
        visiting: &mut std::collections::HashSet<String>,
        measurer: &dyn TextMeasurer,
        text_style: &TextStyle,
        title_wrapping_width: f64,
        wrap_mode: WrapMode,
        cluster_padding: f64,
        title_total_margin: f64,
        node_padding: f64,
    ) -> Result<Rect> {
        if let Some(r) = cluster_rects.get(id).copied() {
            return Ok(r);
        }
        if !visiting.insert(id.to_string()) {
            return Err(Error::InvalidModel {
                message: format!("cycle in subgraph membership involving {id}"),
            });
        }

        let Some(sg) = subgraphs_by_id.get(id) else {
            return Err(Error::InvalidModel {
                message: format!("missing subgraph definition for {id}"),
            });
        };

        let mut content: Option<Rect> = None;
        for member in &sg.nodes {
            let member_rect = if let Some(r) = leaf_rects.get(member).copied() {
                Some(r)
            } else if subgraphs_by_id.contains_key(member) {
                Some(compute_cluster_rect(
                    member,
                    subgraphs_by_id,
                    leaf_rects,
                    extra_children,
                    cluster_rects,
                    cluster_base_widths,
                    visiting,
                    measurer,
                    text_style,
                    title_wrapping_width,
                    wrap_mode,
                    cluster_padding,
                    title_total_margin,
                    node_padding,
                )?)
            } else {
                None
            };

            if let Some(r) = member_rect {
                if let Some(ref mut cur) = content {
                    cur.union(r);
                } else {
                    content = Some(r);
                }
            }
        }

        if let Some(extra) = extra_children.get(id) {
            for child in extra {
                if let Some(r) = leaf_rects.get(child).copied() {
                    if let Some(ref mut cur) = content {
                        cur.union(r);
                    } else {
                        content = Some(r);
                    }
                }
            }
        }

        let title_metrics =
            measurer.measure_wrapped(&sg.title, text_style, Some(title_wrapping_width), wrap_mode);
        let mut rect = if let Some(r) = content {
            r
        } else {
            Rect::from_center(
                0.0,
                0.0,
                title_metrics.width.max(1.0),
                title_metrics.height.max(1.0),
            )
        };

        // Expand to provide the cluster's internal padding.
        rect.min_x -= cluster_padding;
        rect.max_x += cluster_padding;
        rect.min_y -= cluster_padding;
        rect.max_y += cluster_padding;

        // Ensure the cluster is wide enough to fit the title.
        let base_width = rect.width();
        cluster_base_widths.insert(id.to_string(), base_width);

        // Mermaid uses `bbox.width + node.padding` (not `2x`) when determining if the cluster
        // needs to widen for the title label.
        let min_width = title_metrics.width + node_padding;
        if rect.width() < min_width {
            let (cx, cy) = rect.center();
            rect = Rect::from_center(cx, cy, min_width, rect.height());
        }

        // Extend height to reserve space for subgraph title margins (Mermaid does this after layout).
        if title_total_margin > 0.0 {
            let (cx, cy) = rect.center();
            rect = Rect::from_center(cx, cy, rect.width(), rect.height() + title_total_margin);
        }

        // Ensure the cluster is tall enough to fit the title placeholder.
        // When a cluster contains small nodes but a multi-line title, the member union can be
        // shorter than the title itself. Mermaid's rendering always accommodates the title bbox.
        let min_height = title_metrics.height + cluster_padding * 2.0 + title_total_margin;
        if rect.height() < min_height {
            let (cx, cy) = rect.center();
            rect = Rect::from_center(cx, cy, rect.width(), min_height);
        }

        visiting.remove(id);
        cluster_rects.insert(id.to_string(), rect);
        Ok(rect)
    }

    for sg in &model.subgraphs {
        let rect = compute_cluster_rect(
            &sg.id,
            &subgraphs_by_id,
            &leaf_rects,
            &extra_children,
            &mut cluster_rects,
            &mut cluster_base_widths,
            &mut visiting,
            measurer,
            &text_style,
            cluster_title_wrapping_width,
            wrap_mode,
            cluster_padding,
            title_total_margin,
            node_padding,
        )?;
        let (cx, cy) = rect.center();

        let title_metrics = measurer.measure_wrapped(
            &sg.title,
            &text_style,
            Some(cluster_title_wrapping_width),
            wrap_mode,
        );
        let title_label = LayoutLabel {
            x: cx,
            y: cy - rect.height() / 2.0 + title_margin_top + title_metrics.height / 2.0,
            width: title_metrics.width,
            height: title_metrics.height,
        };

        let base_width = cluster_base_widths
            .get(&sg.id)
            .copied()
            .unwrap_or(rect.width());
        let padded_label_width = title_metrics.width + node_padding;
        let diff = if base_width <= padded_label_width {
            (padded_label_width - base_width) / 2.0 - node_padding
        } else {
            -node_padding
        };
        let offset_y = title_metrics.height - node_padding / 2.0;

        let effective_dir = effective_dir_by_id
            .get(&sg.id)
            .cloned()
            .unwrap_or_else(|| effective_cluster_dir(sg, &diagram_direction, inherit_dir));

        clusters.push(LayoutCluster {
            id: sg.id.clone(),
            x: cx,
            y: cy,
            width: rect.width(),
            height: rect.height(),
            diff,
            offset_y,
            title: sg.title.clone(),
            title_label,
            requested_dir: sg.dir.as_ref().map(|s| normalize_dir(s)),
            effective_dir,
            padding: cluster_padding,
            title_margin_top,
            title_margin_bottom,
        });

        out_nodes.push(LayoutNode {
            id: sg.id.clone(),
            x: cx,
            // Mermaid does not shift pure cluster nodes by `subGraphTitleTotalMargin / 2`.
            y: cy,
            width: rect.width(),
            height: rect.height(),
            is_cluster: true,
        });
    }
    clusters.sort_by(|a, b| a.id.cmp(&b.id));

    let mut out_edges: Vec<LayoutEdge> = Vec::new();
    for e in &render_edges {
        let (dx, dy) = edge_packed_shift.get(&e.id).copied().unwrap_or((0.0, 0.0));
        let (
            mut points,
            mut label_pos,
            label_pos_already_shifted,
            mut from_cluster,
            mut to_cluster,
        ) = if let Some(points) = edge_override_points.get(&e.id) {
            let from_cluster = edge_override_from_cluster
                .get(&e.id)
                .cloned()
                .unwrap_or(None);
            let to_cluster = edge_override_to_cluster.get(&e.id).cloned().unwrap_or(None);
            (
                points.clone(),
                edge_override_label.get(&e.id).cloned().unwrap_or(None),
                false,
                from_cluster,
                to_cluster,
            )
        } else {
            let (v, w) = edge_endpoints_by_id
                .get(&e.id)
                .cloned()
                .unwrap_or_else(|| (e.from.clone(), e.to.clone()));
            let Some(label) = g.edge(&v, &w, Some(&e.id)) else {
                return Err(Error::InvalidModel {
                    message: format!("missing layout edge {}", e.id),
                });
            };
            let from_cluster = label
                .extras
                .get("fromCluster")
                .and_then(|v| v.as_str().map(|s| s.to_string()));
            let to_cluster = label
                .extras
                .get("toCluster")
                .and_then(|v| v.as_str().map(|s| s.to_string()));
            let points = label
                .points
                .iter()
                .map(|p| LayoutPoint {
                    x: p.x,
                    // Mermaid shifts all edge points by `subGraphTitleTotalMargin / 2` after Dagre layout.
                    y: p.y + y_shift,
                })
                .collect::<Vec<_>>();
            let label_pos = match (label.x, label.y) {
                (Some(x), Some(y)) if label.width > 0.0 || label.height > 0.0 => {
                    Some(LayoutLabel {
                        x,
                        // Mermaid shifts edge label y by `subGraphTitleTotalMargin / 2` when positioning.
                        y: y + y_shift,
                        width: label.width,
                        height: label.height,
                    })
                }
                _ => None,
            };
            (points, label_pos, false, from_cluster, to_cluster)
        };

        // Match Mermaid's dagre adapter: self-loop special edges on group nodes are annotated with
        // `fromCluster` / `toCluster` so downstream renderers can clip routes to the cluster
        // boundary.
        if subgraph_ids.contains(e.from.as_str()) && e.id.ends_with("-cyclic-special-1") {
            from_cluster = Some(e.from.clone());
        }
        if subgraph_ids.contains(e.to.as_str()) && e.id.ends_with("-cyclic-special-2") {
            to_cluster = Some(e.to.clone());
        }

        if dx.abs() > 1e-6 || dy.abs() > 1e-6 {
            for p in &mut points {
                p.x += dx;
                p.y += dy;
            }
            if !label_pos_already_shifted {
                if let Some(l) = label_pos.as_mut() {
                    l.x += dx;
                    l.y += dy;
                }
            }
        }
        out_edges.push(LayoutEdge {
            id: e.id.clone(),
            from: e.from.clone(),
            to: e.to.clone(),
            from_cluster,
            to_cluster,
            points,
            label: label_pos,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        });
    }

    let bounds = compute_bounds(&out_nodes, &out_edges);

    Ok(FlowchartV2Layout {
        nodes: out_nodes,
        edges: out_edges,
        clusters,
        bounds,
    })
}

fn node_dimensions(
    layout_shape: Option<&str>,
    metrics: crate::text::TextMetrics,
    padding: f64,
) -> (f64, f64) {
    // This function mirrors Mermaid `@11.12.2` node shape sizing rules at the "rendering-elements"
    // layer, but uses our headless `TextMeasurer` metrics instead of DOM `getBBox()`.
    //
    // References:
    // - `packages/mermaid/src/diagrams/flowchart/flowDb.ts` (shape assignment + padding)
    // - `packages/mermaid/src/rendering-util/rendering-elements/shapes/*.ts` (shape bounds)
    let text_w = metrics.width.max(1.0);
    let text_h = metrics.height.max(1.0);
    let p = padding.max(0.0);

    let shape = layout_shape.unwrap_or("squareRect");

    match shape {
        // Default flowchart process node.
        "squareRect" => (text_w + 4.0 * p, text_h + 2.0 * p),

        // Flowchart "round" node type maps to `roundedRect` in FlowDB.
        "roundedRect" => (text_w + 2.0 * p, text_h + 2.0 * p),

        // Diamond (decision/question).
        "diamond" | "question" | "diam" => {
            let w = text_w + p;
            let h = text_h + p;
            let s = w + h;
            (s, s)
        }

        // Hexagon.
        "hexagon" | "hex" => {
            let h = text_h + p;
            let w0 = text_w + 2.5 * p;
            // The current Mermaid implementation expands the half-width by `m = (w/2)/6`,
            // resulting in a total width of `7/6 * w`.
            (w0 * (7.0 / 6.0), h)
        }

        // Stadium/terminator.
        "stadium" => {
            let h = text_h + p;
            let w = text_w + h / 4.0 + p;
            (w, h)
        }

        // Subroutine (framed rectangle): adds an 8px "frame" on both sides.
        "subroutine" | "fr-rect" => {
            let w = text_w + p;
            let h = text_h + p;
            (w + 16.0, h)
        }

        // Cylinder/database.
        "cylinder" | "cyl" => {
            let w = text_w + p;
            let rx = w / 2.0;
            let ry = rx / (2.5 + w / 50.0);
            // Mermaid's cylinder path height ends up including two extra `ry` from the ellipses.
            // See `createCylinderPathD` + `translate(..., -(h/2 + ry))`.
            let height = text_h + p + 3.0 * ry;
            (w, height)
        }

        // Circle.
        "circle" | "circ" => {
            // Mermaid uses half-padding for circles and bases radius on label width.
            let d = text_w + p;
            (d, d)
        }

        // Double circle.
        "doublecircle" | "dbl-circ" => {
            // `gap = 5` is hard-coded in Mermaid.
            let d = text_w + p + 10.0;
            (d, d)
        }

        // Lean and trapezoid variants (parallelograms/trapezoids).
        "lean_right" | "lean-r" | "lean-right" | "lean_left" | "lean-l" | "lean-left"
        | "trapezoid" => {
            let w = text_w + p;
            let h = text_h + p;
            (w + h, h)
        }

        // Inverted trapezoid uses `2 * padding` on both axes in Mermaid.
        "inv_trapezoid" | "inv-trapezoid" => {
            let w = text_w + 2.0 * p;
            let h = text_h + 2.0 * p;
            (w + h, h)
        }

        // Odd node (`>... ]`) is rendered using `rect_left_inv_arrow`.
        "odd" | "rect_left_inv_arrow" => {
            let w = text_w + p;
            let h = text_h + p;
            (w + h / 4.0, h)
        }

        // Ellipses are currently broken upstream but still emitted by FlowDB.
        // Keep a reasonable headless size for layout stability.
        "ellipse" => (text_w + 2.0 * p, text_h + 2.0 * p),

        // Fallback: treat unknown shapes as default rectangles.
        _ => (text_w + 4.0 * p, text_h + 2.0 * p),
    }
}

fn compute_bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for n in nodes {
        let hw = n.width / 2.0;
        let hh = n.height / 2.0;
        pts.push((n.x - hw, n.y - hh));
        pts.push((n.x + hw, n.y + hh));
    }
    for e in edges {
        for p in &e.points {
            pts.push((p.x, p.y));
        }
        if let Some(l) = &e.label {
            let hw = l.width / 2.0;
            let hh = l.height / 2.0;
            pts.push((l.x - hw, l.y - hh));
            pts.push((l.x + hw, l.y + hh));
        }
    }
    Bounds::from_points(pts)
}

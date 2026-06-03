//! Dagre JS reference adapter shared by debug commands.
//!
//! This module owns the JSON shape consumed by `tools/dagre-harness/run.mjs` and the small
//! Mermaid-style compound-edge normalization needed before both JS and Rust layout runs.

use crate::XtaskError;
use dugong::graphlib::Graph;
use dugong::graphlib::json as graphlib_json;
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) type DagreLayoutGraph = Graph<NodeLabel, EdgeLabel, GraphLabel>;

pub(crate) struct DagreReferenceArtifacts {
    pub(crate) input_path: PathBuf,
    pub(crate) js_path: PathBuf,
    pub(crate) rust_path: PathBuf,
}

impl DagreReferenceArtifacts {
    pub(crate) fn for_fixture(out_dir: &Path, fixture: &str) -> Self {
        Self {
            input_path: out_dir.join(format!("{fixture}.input.json")),
            js_path: out_dir.join(format!("{fixture}.js.json")),
            rust_path: out_dir.join(format!("{fixture}.rust.json")),
        }
    }
}

pub(crate) struct DagreReferenceComparison {
    pub(crate) graph_width_delta: f64,
    pub(crate) graph_height_delta: f64,
    pub(crate) max_node_delta: f64,
    pub(crate) max_node_id: Option<String>,
    pub(crate) max_edge_delta: f64,
    pub(crate) max_edge_id: Option<String>,
    pub(crate) rust_only_node_ids: Vec<String>,
    pub(crate) js_only_node_ids: Vec<String>,
    pub(crate) rust_only_edge_ids: Vec<String>,
    pub(crate) js_only_edge_ids: Vec<String>,
}

pub(crate) fn write_dagre_reference_input(
    graph: &DagreLayoutGraph,
    input_path: &Path,
) -> Result<(), XtaskError> {
    let input = snapshot_dagre_input(graph)?;
    fs::write(input_path, serde_json::to_string_pretty(&input)?).map_err(|source| {
        XtaskError::WriteFile {
            path: input_path.display().to_string(),
            source,
        }
    })
}

pub(crate) fn write_rust_dagre_output(
    graph: &DagreLayoutGraph,
    rust_path: &Path,
) -> Result<(), XtaskError> {
    let output = snapshot_rust_dagre_output(graph)?;
    fs::write(rust_path, serde_json::to_string_pretty(&output)?).map_err(|source| {
        XtaskError::WriteFile {
            path: rust_path.display().to_string(),
            source,
        }
    })
}

pub(crate) fn run_js_dagre_harness(
    workspace_root: &Path,
    input_path: &Path,
    js_path: &Path,
) -> Result<(), XtaskError> {
    let script_path = workspace_root
        .join("tools")
        .join("dagre-harness")
        .join("run.mjs");

    let status = Command::new("node")
        .arg(&script_path)
        .arg("--in")
        .arg(input_path)
        .arg("--out")
        .arg(js_path)
        .status()
        .map_err(|e| XtaskError::DebugSvgFailed(format!("failed to spawn node: {e}")))?;
    if !status.success() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "node dagre harness failed (exit={})",
            status.code().unwrap_or(-1)
        )));
    }

    Ok(())
}

pub(crate) fn compare_graph_to_js_reference(
    graph: &DagreLayoutGraph,
    js_path: &Path,
) -> Result<DagreReferenceComparison, XtaskError> {
    let js_raw = fs::read_to_string(js_path).map_err(|source| XtaskError::ReadFile {
        path: js_path.display().to_string(),
        source,
    })?;
    let js_out: JsonValue = serde_json::from_str(&js_raw)?;
    let js_graph_label = js_out.get("value").and_then(|v| v.as_object());
    let js_graph_width = js_graph_label
        .and_then(|label| label.get("width"))
        .and_then(read_f64);
    let js_graph_height = js_graph_label
        .and_then(|label| label.get("height"))
        .and_then(read_f64);

    let mut js_node_ids: BTreeSet<String> = BTreeSet::new();
    let mut js_node_positions: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    if let Some(arr) = js_out.get("nodes").and_then(|v| v.as_array()) {
        for n in arr {
            let Some(id) = node_json_id(n) else {
                continue;
            };
            js_node_ids.insert(id.to_string());
            let Some(label) = graph_json_label(n) else {
                continue;
            };
            let Some(x) = label.get("x").and_then(read_f64) else {
                continue;
            };
            let Some(y) = label.get("y").and_then(read_f64) else {
                continue;
            };
            js_node_positions.insert(id.to_string(), (x, y));
        }
    }

    let mut js_edge_ids: BTreeSet<String> = BTreeSet::new();
    let mut js_edge_points: BTreeMap<String, Vec<(f64, f64)>> = BTreeMap::new();
    if let Some(arr) = js_out.get("edges").and_then(|v| v.as_array()) {
        for e in arr {
            let Some(v) = e.get("v").and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(w) = e.get("w").and_then(|v| v.as_str()) else {
                continue;
            };
            let name = e.get("name").and_then(|v| v.as_str());
            let key = edge_key_string(v, w, name);
            js_edge_ids.insert(key.clone());
            let Some(label) = graph_json_label(e) else {
                continue;
            };
            let Some(points) = label.get("points").and_then(|v| v.as_array()) else {
                continue;
            };

            let mut pts: Vec<(f64, f64)> = Vec::new();
            for p in points {
                let Some(px) = p.get("x").and_then(read_f64) else {
                    continue;
                };
                let Some(py) = p.get("y").and_then(read_f64) else {
                    continue;
                };
                pts.push((px, py));
            }
            js_edge_points.insert(key, pts);
        }
    }

    Ok(compare_graph_points_to_reference(
        graph,
        js_graph_width,
        js_graph_height,
        &js_node_ids,
        &js_node_positions,
        &js_edge_ids,
        &js_edge_points,
    ))
}

pub(crate) fn normalize_cluster_edge_endpoints_like_harness(graph: &mut DagreLayoutGraph) {
    let cluster_ids: Vec<String> = graph
        .node_ids()
        .into_iter()
        .filter(|id| !graph.children(id).is_empty())
        .collect();
    if cluster_ids.is_empty() {
        return;
    }

    let mut anchor: HashMap<String, String> = HashMap::new();
    for id in &cluster_ids {
        let Some(a) = find_non_cluster_child(graph, id, id) else {
            continue;
        };
        anchor.insert(id.clone(), a);
    }

    // Dagre assumes edges never touch compound nodes. Mirror the JS harness so both reference and
    // Rust layout runs operate on the same transformed graph.
    let edge_keys = graph.edge_keys();
    for key in edge_keys {
        let mut v = key.v.clone();
        let mut w = key.w.clone();
        if cluster_ids.iter().any(|c| c == &v) {
            if let Some(a) = anchor.get(&v) {
                v = a.clone();
            }
        }
        if cluster_ids.iter().any(|c| c == &w) {
            if let Some(a) = anchor.get(&w) {
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
        graph.set_edge_named(v, w, key.name.clone(), Some(old_label));
    }
}

fn snapshot_dagre_input(
    graph: &DagreLayoutGraph,
) -> Result<graphlib_json::GraphJson, serde_json::Error> {
    snapshot_dagre_graph_json(graph, DagreSnapshotPhase::Input)
}

fn snapshot_rust_dagre_output(
    graph: &DagreLayoutGraph,
) -> Result<graphlib_json::GraphJson, serde_json::Error> {
    snapshot_dagre_graph_json(graph, DagreSnapshotPhase::Output)
}

#[derive(Debug, Clone, Copy)]
enum DagreSnapshotPhase {
    Input,
    Output,
}

fn snapshot_dagre_graph_json(
    graph: &DagreLayoutGraph,
    phase: DagreSnapshotPhase,
) -> Result<graphlib_json::GraphJson, serde_json::Error> {
    let mut snapshot: Graph<Option<JsonValue>, Option<JsonValue>, Option<JsonValue>> =
        Graph::new(graph.options());
    snapshot.set_graph(Some(graph_label_to_json(graph.graph(), phase)));

    for id in graph.node_ids() {
        let Some(label) = graph.node(&id) else {
            continue;
        };
        snapshot.set_node(id.clone(), Some(node_label_to_json(label, phase)));
        if let Some(parent) = graph.parent(&id) {
            snapshot.set_parent(id, parent.to_string());
        }
    }

    for key in graph.edge_keys() {
        let Some(label) = graph.edge_by_key(&key) else {
            continue;
        };
        snapshot.set_edge_named(
            key.v,
            key.w,
            key.name,
            Some(Some(edge_label_to_json(label, phase))),
        );
    }

    graphlib_json::write(&snapshot)
}

fn graph_label_to_json(graph_label: &GraphLabel, phase: DagreSnapshotPhase) -> JsonValue {
    let mut graph_obj = serde_json::Map::new();
    graph_obj.insert(
        "rankdir".to_string(),
        JsonValue::from(rankdir_to_string(graph_label.rankdir)),
    );
    graph_obj.insert("nodesep".to_string(), JsonValue::from(graph_label.nodesep));
    graph_obj.insert("ranksep".to_string(), JsonValue::from(graph_label.ranksep));
    graph_obj.insert("edgesep".to_string(), JsonValue::from(graph_label.edgesep));
    graph_obj.insert("marginx".to_string(), JsonValue::from(graph_label.marginx));
    graph_obj.insert("marginy".to_string(), JsonValue::from(graph_label.marginy));
    if matches!(phase, DagreSnapshotPhase::Output) {
        graph_obj.insert("width".to_string(), JsonValue::from(graph_label.width));
        graph_obj.insert("height".to_string(), JsonValue::from(graph_label.height));
    }
    graph_obj.insert(
        "align".to_string(),
        graph_label
            .align
            .as_ref()
            .map(|s| JsonValue::from(s.clone()))
            .unwrap_or(JsonValue::Null),
    );
    graph_obj.insert(
        "ranker".to_string(),
        graph_label
            .ranker
            .as_ref()
            .map(|s| JsonValue::from(s.clone()))
            .unwrap_or(JsonValue::Null),
    );
    graph_obj.insert(
        "acyclicer".to_string(),
        graph_label
            .acyclicer
            .as_ref()
            .map(|s| JsonValue::from(s.clone()))
            .unwrap_or(JsonValue::Null),
    );

    JsonValue::Object(graph_obj)
}

fn node_label_to_json(label: &NodeLabel, phase: DagreSnapshotPhase) -> JsonValue {
    let mut obj = serde_json::Map::new();
    obj.insert("width".to_string(), JsonValue::from(label.width));
    obj.insert("height".to_string(), JsonValue::from(label.height));

    if matches!(phase, DagreSnapshotPhase::Output) {
        obj.insert(
            "x".to_string(),
            label.x.map(JsonValue::from).unwrap_or(JsonValue::Null),
        );
        obj.insert(
            "y".to_string(),
            label.y.map(JsonValue::from).unwrap_or(JsonValue::Null),
        );
        obj.insert(
            "rank".to_string(),
            label
                .rank
                .map(|rank| JsonValue::from(rank as i64))
                .unwrap_or(JsonValue::Null),
        );
        obj.insert(
            "order".to_string(),
            label
                .order
                .map(|order| JsonValue::from(order as u64))
                .unwrap_or(JsonValue::Null),
        );
    }

    JsonValue::Object(obj)
}

fn edge_label_to_json(label: &EdgeLabel, phase: DagreSnapshotPhase) -> JsonValue {
    let mut obj = serde_json::Map::new();
    obj.insert("width".to_string(), JsonValue::from(label.width));
    obj.insert("height".to_string(), JsonValue::from(label.height));
    obj.insert("minlen".to_string(), JsonValue::from(label.minlen as u64));
    obj.insert("weight".to_string(), JsonValue::from(label.weight));
    obj.insert(
        "labeloffset".to_string(),
        JsonValue::from(label.labeloffset),
    );
    obj.insert(
        "labelpos".to_string(),
        JsonValue::from(labelpos_to_string(label.labelpos)),
    );

    if matches!(phase, DagreSnapshotPhase::Output) {
        obj.insert(
            "x".to_string(),
            label.x.map(JsonValue::from).unwrap_or(JsonValue::Null),
        );
        obj.insert(
            "y".to_string(),
            label.y.map(JsonValue::from).unwrap_or(JsonValue::Null),
        );
        obj.insert(
            "points".to_string(),
            JsonValue::Array(
                label
                    .points
                    .iter()
                    .map(|p| {
                        serde_json::json!({
                            "x": p.x,
                            "y": p.y,
                        })
                    })
                    .collect(),
            ),
        );
    }

    JsonValue::Object(obj)
}

fn compare_graph_points_to_reference(
    graph: &DagreLayoutGraph,
    js_graph_width: Option<f64>,
    js_graph_height: Option<f64>,
    js_node_ids: &BTreeSet<String>,
    js_node_positions: &BTreeMap<String, (f64, f64)>,
    js_edge_ids: &BTreeSet<String>,
    js_edge_points: &BTreeMap<String, Vec<(f64, f64)>>,
) -> DagreReferenceComparison {
    let rust_node_ids: BTreeSet<String> = graph.node_ids().into_iter().collect();
    let rust_only_node_ids: Vec<String> = rust_node_ids.difference(js_node_ids).cloned().collect();
    let js_only_node_ids: Vec<String> = js_node_ids.difference(&rust_node_ids).cloned().collect();

    let rust_edge_ids: BTreeSet<String> = graph
        .edge_keys()
        .into_iter()
        .map(|ek| edge_key_string(&ek.v, &ek.w, ek.name.as_deref()))
        .collect();
    let rust_only_edge_ids: Vec<String> = rust_edge_ids.difference(js_edge_ids).cloned().collect();
    let js_only_edge_ids: Vec<String> = js_edge_ids.difference(&rust_edge_ids).cloned().collect();

    let mut max_node_delta = 0.0f64;
    let mut max_node_id: Option<String> = None;

    for id in &rust_node_ids {
        let Some(n) = graph.node(id) else {
            continue;
        };
        let (Some(rx), Some(ry)) = (n.x, n.y) else {
            continue;
        };
        let Some((jx, jy)) = js_node_positions.get(id) else {
            if js_node_ids.contains(id) {
                max_node_delta = f64::INFINITY;
                max_node_id.get_or_insert_with(|| id.clone());
            }
            continue;
        };
        let dx = jx - rx;
        let dy = jy - ry;
        let d = dx.abs().max(dy.abs());
        if d > max_node_delta {
            max_node_delta = d;
            max_node_id = Some(id.clone());
        }
    }

    let mut max_edge_delta = 0.0f64;
    let mut max_edge_id: Option<String> = None;

    for ek in graph.edge_keys() {
        let Some(e) = graph.edge_by_key(&ek) else {
            continue;
        };
        let key = edge_key_string(&ek.v, &ek.w, ek.name.as_deref());
        if !js_edge_ids.contains(&key) {
            continue;
        }
        let Some(jpts) = js_edge_points.get(&key) else {
            max_edge_delta = f64::INFINITY;
            max_edge_id.get_or_insert(key);
            continue;
        };
        if e.points.len() != jpts.len() {
            max_edge_delta = f64::INFINITY;
            max_edge_id = Some(key);
            break;
        }
        for (rp, (jx, jy)) in e.points.iter().zip(jpts.iter()) {
            let dx = jx - rp.x;
            let dy = jy - rp.y;
            let d = dx.abs().max(dy.abs());
            if d > max_edge_delta {
                max_edge_delta = d;
                max_edge_id = Some(key.clone());
            }
        }
    }

    DagreReferenceComparison {
        graph_width_delta: dimension_delta(js_graph_width, graph.graph().width),
        graph_height_delta: dimension_delta(js_graph_height, graph.graph().height),
        max_node_delta,
        max_node_id,
        max_edge_delta,
        max_edge_id,
        rust_only_node_ids,
        js_only_node_ids,
        rust_only_edge_ids,
        js_only_edge_ids,
    }
}

fn dimension_delta(reference: Option<f64>, actual: f64) -> f64 {
    reference
        .map(|reference| (reference - actual).abs())
        .unwrap_or(f64::INFINITY)
}

fn find_common_edges(graph: &DagreLayoutGraph, id1: &str, id2: &str) -> Vec<(String, String)> {
    let edges1: Vec<(String, String)> = graph
        .edge_keys()
        .into_iter()
        .filter(|e| e.v == id1 || e.w == id1)
        .map(|e| (e.v, e.w))
        .collect();
    let edges2: Vec<(String, String)> = graph
        .edge_keys()
        .into_iter()
        .filter(|e| e.v == id2 || e.w == id2)
        .map(|e| (e.v, e.w))
        .collect();

    let edges1_prim: Vec<(String, String)> = edges1
        .into_iter()
        .map(|(v, w)| {
            (
                if v == id1 { id2.to_string() } else { v },
                // Mermaid's `findCommonEdges(...)` has an asymmetry here: it maps the `w` side
                // back to `id1` rather than `id2` in the pinned Mermaid baseline.
                if w == id1 { id1.to_string() } else { w },
            )
        })
        .collect();

    let mut out = Vec::new();
    for e1 in edges1_prim {
        if edges2.contains(&e1) {
            out.push(e1);
        }
    }
    out
}

fn find_non_cluster_child(graph: &DagreLayoutGraph, id: &str, cluster_id: &str) -> Option<String> {
    let children = graph.children(id);
    if children.is_empty() {
        return Some(id.to_string());
    }
    let mut reserve: Option<String> = None;
    for child in children {
        let Some(candidate) = find_non_cluster_child(graph, child, cluster_id) else {
            continue;
        };
        let common_edges = find_common_edges(graph, cluster_id, &candidate);
        if !common_edges.is_empty() {
            reserve = Some(candidate);
        } else {
            return Some(candidate);
        }
    }
    reserve
}

fn rankdir_to_string(d: RankDir) -> &'static str {
    match d {
        RankDir::TB => "TB",
        RankDir::BT => "BT",
        RankDir::LR => "LR",
        RankDir::RL => "RL",
    }
}

fn labelpos_to_string(p: LabelPos) -> &'static str {
    match p {
        LabelPos::C => "c",
        LabelPos::L => "l",
        LabelPos::R => "r",
    }
}

fn read_f64(v: &JsonValue) -> Option<f64> {
    match v {
        JsonValue::Number(n) => n.as_f64(),
        _ => None,
    }
}

fn node_json_id(value: &JsonValue) -> Option<&str> {
    value
        .get("v")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("id").and_then(|v| v.as_str()))
}

fn graph_json_label(value: &JsonValue) -> Option<&serde_json::Map<String, JsonValue>> {
    value
        .get("value")
        .or_else(|| value.get("label"))
        .and_then(|v| v.as_object())
}

fn edge_key_string(v: &str, w: &str, name: Option<&str>) -> String {
    let name = name.unwrap_or("");
    format!("{v}\u{1f}{w}\u{1f}{name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use dugong::graphlib::GraphOptions;

    #[test]
    fn compound_edge_normalization_moves_edges_to_non_cluster_child() {
        let mut graph = DagreLayoutGraph::new(GraphOptions {
            directed: true,
            multigraph: true,
            compound: true,
        });
        graph.set_node("cluster", NodeLabel::default());
        graph.set_node("child", NodeLabel::default());
        graph.set_node("other", NodeLabel::default());
        graph.set_parent("child", "cluster");
        graph.set_edge_named(
            "cluster",
            "other",
            Some("edge"),
            Some(EdgeLabel {
                width: 12.0,
                ..Default::default()
            }),
        );

        normalize_cluster_edge_endpoints_like_harness(&mut graph);

        assert!(!graph.has_edge("cluster", "other", Some("edge")));
        assert!(graph.has_edge("child", "other", Some("edge")));
        assert_eq!(
            graph
                .edge("child", "other", Some("edge"))
                .map(|label| label.width),
            Some(12.0)
        );
    }

    #[test]
    fn dagre_reference_input_uses_graphlib_json_shape() {
        let mut graph = DagreLayoutGraph::new(GraphOptions {
            directed: true,
            multigraph: true,
            compound: true,
        });
        graph.graph_mut().rankdir = RankDir::LR;
        graph.set_node(
            "cluster",
            NodeLabel {
                width: 1.0,
                height: 2.0,
                ..Default::default()
            },
        );
        graph.set_node(
            "child",
            NodeLabel {
                width: 10.0,
                height: 20.0,
                ..Default::default()
            },
        );
        graph.set_parent("child", "cluster");
        graph.set_edge_named(
            "child",
            "cluster",
            Some("named"),
            Some(EdgeLabel {
                width: 3.0,
                height: 4.0,
                minlen: 2,
                weight: 5.0,
                ..Default::default()
            }),
        );

        let input = serde_json::to_value(snapshot_dagre_input(&graph).expect("snapshot graph"))
            .expect("serialize graph json");

        assert_eq!(input["value"]["rankdir"], JsonValue::from("LR"));
        assert!(input["value"].get("width").is_none());
        assert!(input["value"].get("height").is_none());
        assert!(input.get("graph").is_none());

        let child = input["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .find(|node| node["v"] == JsonValue::from("child"))
            .expect("child node");
        assert_eq!(child["parent"], JsonValue::from("cluster"));
        assert_eq!(child["value"]["width"], JsonValue::from(10.0));
        assert!(child.get("id").is_none());
        assert!(child.get("label").is_none());

        let edge = &input["edges"][0];
        assert_eq!(edge["v"], JsonValue::from("child"));
        assert_eq!(edge["w"], JsonValue::from("cluster"));
        assert_eq!(edge["name"], JsonValue::from("named"));
        assert_eq!(edge["value"]["minlen"], JsonValue::from(2));
        assert!(edge.get("label").is_none());
    }

    #[test]
    fn dagre_reference_output_uses_graphlib_json_shape() {
        let mut graph = DagreLayoutGraph::new(GraphOptions {
            directed: true,
            multigraph: true,
            compound: true,
        });
        graph.graph_mut().rankdir = RankDir::BT;
        graph.graph_mut().width = 123.0;
        graph.graph_mut().height = 456.0;
        graph.set_node(
            "a",
            NodeLabel {
                width: 10.0,
                height: 20.0,
                x: Some(30.0),
                y: Some(40.0),
                rank: Some(2),
                order: Some(1),
                ..Default::default()
            },
        );
        graph.set_edge_with_label(
            "a",
            "b",
            EdgeLabel {
                width: 3.0,
                height: 4.0,
                x: Some(5.0),
                y: Some(6.0),
                points: vec![dugong::Point { x: 7.0, y: 8.0 }],
                ..Default::default()
            },
        );

        let output =
            serde_json::to_value(snapshot_rust_dagre_output(&graph).expect("snapshot graph"))
                .expect("serialize graph json");

        assert_eq!(output["value"]["rankdir"], JsonValue::from("BT"));
        assert_eq!(output["value"]["width"], JsonValue::from(123.0));
        assert_eq!(output["value"]["height"], JsonValue::from(456.0));
        assert!(output.get("graph").is_none());

        let node = output["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .find(|node| node["v"] == JsonValue::from("a"))
            .expect("node a");
        assert_eq!(node["value"]["x"], JsonValue::from(30.0));
        assert_eq!(node["value"]["rank"], JsonValue::from(2));
        assert!(node.get("id").is_none());
        assert!(node.get("label").is_none());

        let edge = &output["edges"][0];
        assert_eq!(edge["value"]["x"], JsonValue::from(5.0));
        assert_eq!(edge["value"]["points"][0]["x"], JsonValue::from(7.0));
        assert!(edge.get("label").is_none());
    }

    #[test]
    fn dagre_reference_comparison_reports_graph_identity_mismatches() {
        let mut graph = DagreLayoutGraph::new(GraphOptions {
            directed: true,
            multigraph: true,
            compound: false,
        });
        graph.set_node(
            "a",
            NodeLabel {
                x: Some(1.0),
                y: Some(2.0),
                ..Default::default()
            },
        );
        graph.set_node(
            "b",
            NodeLabel {
                x: Some(3.0),
                y: Some(4.0),
                ..Default::default()
            },
        );
        graph.set_edge_named(
            "a",
            "b",
            Some("ab"),
            Some(EdgeLabel {
                points: vec![dugong::Point { x: 5.0, y: 6.0 }],
                ..Default::default()
            }),
        );

        let js_node_ids = BTreeSet::from(["a".to_string(), "extra".to_string()]);
        let js_node_positions = BTreeMap::from([("a".to_string(), (1.0, 2.0))]);
        let js_edge_ids = BTreeSet::from(["extra\u{1f}a\u{1f}".to_string()]);
        let js_edge_points = BTreeMap::new();

        let comparison = compare_graph_points_to_reference(
            &graph,
            Some(0.0),
            Some(0.0),
            &js_node_ids,
            &js_node_positions,
            &js_edge_ids,
            &js_edge_points,
        );

        assert_eq!(comparison.rust_only_node_ids, vec!["b"]);
        assert_eq!(comparison.js_only_node_ids, vec!["extra"]);
        assert_eq!(comparison.rust_only_edge_ids, vec!["a\u{1f}b\u{1f}ab"]);
        assert_eq!(comparison.js_only_edge_ids, vec!["extra\u{1f}a\u{1f}"]);
        assert_eq!(comparison.graph_width_delta, 0.0);
        assert_eq!(comparison.graph_height_delta, 0.0);
        assert_eq!(comparison.max_node_delta, 0.0);
        assert_eq!(comparison.max_edge_delta, 0.0);
    }

    #[test]
    fn dagre_reference_comparison_reports_graph_dimension_deltas() {
        let mut graph = DagreLayoutGraph::new(GraphOptions {
            directed: true,
            multigraph: true,
            compound: false,
        });
        graph.graph_mut().width = 100.0;
        graph.graph_mut().height = 50.0;

        let comparison = compare_graph_points_to_reference(
            &graph,
            Some(103.5),
            Some(45.0),
            &BTreeSet::new(),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &BTreeMap::new(),
        );

        assert_eq!(comparison.graph_width_delta, 3.5);
        assert_eq!(comparison.graph_height_delta, 5.0);
        assert_eq!(comparison.max_node_delta, 0.0);
        assert_eq!(comparison.max_edge_delta, 0.0);
    }

    #[test]
    fn dagre_reference_comparison_marks_missing_coordinates_as_infinite_delta() {
        let mut graph = DagreLayoutGraph::new(GraphOptions {
            directed: true,
            multigraph: true,
            compound: false,
        });
        graph.set_node(
            "a",
            NodeLabel {
                x: Some(1.0),
                y: Some(2.0),
                ..Default::default()
            },
        );
        graph.set_edge_with_label(
            "a",
            "b",
            EdgeLabel {
                points: vec![dugong::Point { x: 5.0, y: 6.0 }],
                ..Default::default()
            },
        );

        let js_node_ids = BTreeSet::from(["a".to_string(), "b".to_string()]);
        let js_node_positions = BTreeMap::new();
        let js_edge_ids = BTreeSet::from(["a\u{1f}b\u{1f}".to_string()]);
        let js_edge_points = BTreeMap::new();

        let comparison = compare_graph_points_to_reference(
            &graph,
            None,
            None,
            &js_node_ids,
            &js_node_positions,
            &js_edge_ids,
            &js_edge_points,
        );

        assert_eq!(comparison.graph_width_delta, f64::INFINITY);
        assert_eq!(comparison.graph_height_delta, f64::INFINITY);
        assert_eq!(comparison.max_node_delta, f64::INFINITY);
        assert_eq!(comparison.max_node_id.as_deref(), Some("a"));
        assert_eq!(comparison.max_edge_delta, f64::INFINITY);
        assert_eq!(comparison.max_edge_id.as_deref(), Some("a\u{1f}b\u{1f}"));
    }
}

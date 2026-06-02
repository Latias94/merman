//! Dagre JS reference adapter shared by debug commands.
//!
//! This module owns the JSON shape consumed by `tools/dagre-harness/run.mjs` and the small
//! Mermaid-style compound-edge normalization needed before both JS and Rust layout runs.

use crate::XtaskError;
use dugong::graphlib::Graph;
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, HashMap};
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
    pub(crate) max_node_delta: f64,
    pub(crate) max_node_id: Option<String>,
    pub(crate) max_edge_delta: f64,
    pub(crate) max_edge_id: Option<String>,
}

pub(crate) fn write_dagre_reference_input(
    graph: &DagreLayoutGraph,
    input_path: &Path,
) -> Result<(), XtaskError> {
    let input = snapshot_dagre_input(graph);
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
    let output = snapshot_rust_dagre_output(graph);
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

    let mut js_nodes: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    if let Some(arr) = js_out.get("nodes").and_then(|v| v.as_array()) {
        for n in arr {
            let Some(id) = n.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(label) = n.get("label").and_then(|v| v.as_object()) else {
                continue;
            };
            let Some(x) = label.get("x").and_then(read_f64) else {
                continue;
            };
            let Some(y) = label.get("y").and_then(read_f64) else {
                continue;
            };
            js_nodes.insert(id.to_string(), (x, y));
        }
    }

    let mut js_edges: BTreeMap<String, Vec<(f64, f64)>> = BTreeMap::new();
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
            let Some(label) = e.get("label").and_then(|v| v.as_object()) else {
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
            js_edges.insert(key, pts);
        }
    }

    Ok(compare_graph_points_to_reference(
        graph, &js_nodes, &js_edges,
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

fn snapshot_dagre_input(graph: &DagreLayoutGraph) -> JsonValue {
    let opts = graph.options();
    let graph_label = graph.graph();
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

    let nodes = graph
        .node_ids()
        .into_iter()
        .filter_map(|id| {
            let n = graph.node(&id)?;
            let mut label = serde_json::Map::new();
            label.insert("width".to_string(), JsonValue::from(n.width));
            label.insert("height".to_string(), JsonValue::from(n.height));
            Some(JsonValue::Object({
                let mut obj = serde_json::Map::new();
                obj.insert("id".to_string(), JsonValue::from(id.clone()));
                obj.insert(
                    "parent".to_string(),
                    graph
                        .parent(&id)
                        .map(|p| JsonValue::from(p.to_string()))
                        .unwrap_or(JsonValue::Null),
                );
                obj.insert("label".to_string(), JsonValue::Object(label));
                obj
            }))
        })
        .collect::<Vec<_>>();

    let edges = graph
        .edge_keys()
        .into_iter()
        .filter_map(|ek| {
            let e = graph.edge_by_key(&ek)?;
            let mut label = serde_json::Map::new();
            label.insert("width".to_string(), JsonValue::from(e.width));
            label.insert("height".to_string(), JsonValue::from(e.height));
            label.insert("minlen".to_string(), JsonValue::from(e.minlen as u64));
            label.insert("weight".to_string(), JsonValue::from(e.weight));
            label.insert("labeloffset".to_string(), JsonValue::from(e.labeloffset));
            label.insert(
                "labelpos".to_string(),
                JsonValue::from(labelpos_to_string(e.labelpos)),
            );

            Some(JsonValue::Object({
                let mut obj = serde_json::Map::new();
                obj.insert("v".to_string(), JsonValue::from(ek.v.clone()));
                obj.insert("w".to_string(), JsonValue::from(ek.w.clone()));
                obj.insert(
                    "name".to_string(),
                    ek.name
                        .as_ref()
                        .map(|s| JsonValue::from(s.clone()))
                        .unwrap_or(JsonValue::Null),
                );
                obj.insert("label".to_string(), JsonValue::Object(label));
                obj
            }))
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "options": {
            "directed": opts.directed,
            "multigraph": opts.multigraph,
            "compound": opts.compound,
        },
        "graph": JsonValue::Object(graph_obj),
        "nodes": nodes,
        "edges": edges,
    })
}

fn snapshot_rust_dagre_output(graph: &DagreLayoutGraph) -> JsonValue {
    let nodes = graph
        .node_ids()
        .into_iter()
        .filter_map(|id| {
            let n = graph.node(&id)?;
            Some(serde_json::json!({
                "id": id,
                "x": n.x,
                "y": n.y,
                "width": n.width,
                "height": n.height,
                "rank": n.rank,
                "order": n.order,
            }))
        })
        .collect::<Vec<_>>();

    let edges = graph
        .edge_keys()
        .into_iter()
        .filter_map(|ek| {
            let e = graph.edge_by_key(&ek)?;
            Some(serde_json::json!({
                "v": ek.v,
                "w": ek.w,
                "name": ek.name,
                "x": e.x,
                "y": e.y,
                "points": e.points.iter().map(|p| serde_json::json!({"x": p.x, "y": p.y})).collect::<Vec<_>>(),
            }))
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "nodes": nodes,
        "edges": edges,
    })
}

fn compare_graph_points_to_reference(
    graph: &DagreLayoutGraph,
    js_nodes: &BTreeMap<String, (f64, f64)>,
    js_edges: &BTreeMap<String, Vec<(f64, f64)>>,
) -> DagreReferenceComparison {
    let mut max_node_delta = 0.0f64;
    let mut max_node_id: Option<String> = None;

    for id in graph.node_ids() {
        let Some(n) = graph.node(&id) else {
            continue;
        };
        let (Some(rx), Some(ry)) = (n.x, n.y) else {
            continue;
        };
        let Some((jx, jy)) = js_nodes.get(&id) else {
            continue;
        };
        let dx = jx - rx;
        let dy = jy - ry;
        let d = dx.abs().max(dy.abs());
        if d > max_node_delta {
            max_node_delta = d;
            max_node_id = Some(id);
        }
    }

    let mut max_edge_delta = 0.0f64;
    let mut max_edge_id: Option<String> = None;

    for ek in graph.edge_keys() {
        let Some(e) = graph.edge_by_key(&ek) else {
            continue;
        };
        let key = edge_key_string(&ek.v, &ek.w, ek.name.as_deref());
        let Some(jpts) = js_edges.get(&key) else {
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
        max_node_delta,
        max_node_id,
        max_edge_delta,
        max_edge_id,
    }
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
}

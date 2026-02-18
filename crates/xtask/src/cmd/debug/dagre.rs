//! Dagre layout debug utilities.

use crate::XtaskError;
use crate::util::*;
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn compare_dagre_layout(args: Vec<String>) -> Result<(), XtaskError> {
    use dugong::graphlib::Graph;
    use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
    use std::collections::HashMap;

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

    fn snapshot_input(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> Result<JsonValue, XtaskError> {
        let opts = g.options();
        let graph = g.graph();
        let mut graph_obj = serde_json::Map::new();
        graph_obj.insert(
            "rankdir".to_string(),
            JsonValue::from(rankdir_to_string(graph.rankdir)),
        );
        graph_obj.insert("nodesep".to_string(), JsonValue::from(graph.nodesep));
        graph_obj.insert("ranksep".to_string(), JsonValue::from(graph.ranksep));
        graph_obj.insert("edgesep".to_string(), JsonValue::from(graph.edgesep));
        graph_obj.insert("marginx".to_string(), JsonValue::from(graph.marginx));
        graph_obj.insert("marginy".to_string(), JsonValue::from(graph.marginy));
        graph_obj.insert(
            "align".to_string(),
            graph
                .align
                .as_ref()
                .map(|s| JsonValue::from(s.clone()))
                .unwrap_or(JsonValue::Null),
        );
        graph_obj.insert(
            "ranker".to_string(),
            graph
                .ranker
                .as_ref()
                .map(|s| JsonValue::from(s.clone()))
                .unwrap_or(JsonValue::Null),
        );
        graph_obj.insert(
            "acyclicer".to_string(),
            graph
                .acyclicer
                .as_ref()
                .map(|s| JsonValue::from(s.clone()))
                .unwrap_or(JsonValue::Null),
        );

        let nodes = g
            .node_ids()
            .into_iter()
            .filter_map(|id| {
                let n = g.node(&id)?;
                let mut label = serde_json::Map::new();
                label.insert("width".to_string(), JsonValue::from(n.width));
                label.insert("height".to_string(), JsonValue::from(n.height));
                Some(JsonValue::Object({
                    let mut obj = serde_json::Map::new();
                    obj.insert("id".to_string(), JsonValue::from(id.clone()));
                    obj.insert(
                        "parent".to_string(),
                        g.parent(&id)
                            .map(|p| JsonValue::from(p.to_string()))
                            .unwrap_or(JsonValue::Null),
                    );
                    obj.insert("label".to_string(), JsonValue::Object(label));
                    obj
                }))
            })
            .collect::<Vec<_>>();

        let edges = g
            .edge_keys()
            .into_iter()
            .filter_map(|ek| {
                let e = g.edge_by_key(&ek)?;
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

        Ok(serde_json::json!({
            "options": {
                "directed": opts.directed,
                "multigraph": opts.multigraph,
                "compound": opts.compound,
            },
            "graph": JsonValue::Object(graph_obj),
            "nodes": nodes,
            "edges": edges,
        }))
    }

    fn snapshot_output(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> Result<JsonValue, XtaskError> {
        let nodes = g
            .node_ids()
            .into_iter()
            .filter_map(|id| {
                let n = g.node(&id)?;
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

        let edges = g
            .edge_keys()
            .into_iter()
            .filter_map(|ek| {
                let e = g.edge_by_key(&ek)?;
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

        Ok(serde_json::json!({
            "nodes": nodes,
            "edges": edges,
        }))
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

    let mut diagram: String = "state".to_string();
    let mut fixture: Option<String> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut cluster: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "state".to_string());
            }
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.to_string());
            }
            "--out-dir" => {
                i += 1;
                out_dir = args.get(i).map(PathBuf::from);
            }
            "--cluster" => {
                i += 1;
                cluster = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let fixture = fixture.ok_or(XtaskError::Usage)?;
    if diagram != "state" {
        return Err(XtaskError::Usage);
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join(&diagram);
    let mmd_path = fixtures_dir.join(format!("{fixture}.mmd"));
    let text = fs::read_to_string(&mmd_path).map_err(|source| XtaskError::ReadFile {
        path: mmd_path.display().to_string(),
        source,
    })?;

    let out_dir = out_dir.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("dagre-layout")
    });
    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let engine = merman::Engine::new();
    let parsed = match futures::executor::block_on(
        engine.parse_diagram(&text, merman::ParseOptions::default()),
    ) {
        Ok(Some(v)) => v,
        Ok(None) => {
            return Err(XtaskError::DebugSvgFailed(
                "no diagram detected".to_string(),
            ));
        }
        Err(err) => return Err(XtaskError::DebugSvgFailed(format!("parse failed: {err}"))),
    };

    let measurer = merman_render::text::VendoredFontMetricsTextMeasurer::default();
    let mut g = merman_render::state::debug_build_state_diagram_v2_dagre_graph(
        &parsed.model,
        parsed.meta.effective_config.as_value(),
        &measurer,
    )
    .map_err(|e| XtaskError::DebugSvgFailed(format!("build dagre graph failed: {e}")))?;

    fn normalize_cluster_edge_endpoints_like_harness(
        graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) {
        fn find_common_edges(
            graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
            id1: &str,
            id2: &str,
        ) -> Vec<(String, String)> {
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
                        // Mermaid's `findCommonEdges(...)` has an asymmetry here: it maps the `w`
                        // side back to `id1` rather than `id2` (Mermaid@11.12.2).
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
                let common_edges = find_common_edges(graph, cluster_id, &candidate);
                if !common_edges.is_empty() {
                    reserve = Some(candidate);
                } else {
                    return Some(candidate);
                }
            }
            reserve
        }

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

        // Dagre assumes edges never touch compound nodes (nodes with children).
        //
        // Mirror `tools/dagre-harness/run.mjs` `normalizeClusterEdgeEndpoints(...)` so the Rust
        // and JS layout runs operate on the same transformed graph.
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

    fn inject_root_cluster_node(
        g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        root_id: &str,
    ) -> Result<(), XtaskError> {
        if !g.has_node(root_id) {
            g.set_node(
                root_id.to_string(),
                NodeLabel {
                    width: 1.0,
                    height: 1.0,
                    ..Default::default()
                },
            );
        }

        let node_ids: Vec<String> = g.node_ids().into_iter().collect();
        for v in node_ids {
            if v == root_id {
                continue;
            }
            if g.parent(&v).is_none() {
                g.set_parent(v, root_id.to_string());
            }
        }
        Ok(())
    }

    if let Some(cluster_id) = cluster.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let parent_label = g.graph().clone();
        let mut parent = g;
        let mut sub = merman_render::state::debug_extract_state_diagram_v2_cluster_graph(
            &mut parent,
            cluster_id,
        )
        .map_err(|e| XtaskError::DebugSvgFailed(format!("extract cluster graph failed: {e}")))?;

        // Mirror `prepare_graph(...)` overrides for extracted state subgraphs.
        sub.graph_mut().rankdir = parent_label.rankdir;
        sub.graph_mut().nodesep = parent_label.nodesep;
        sub.graph_mut().ranksep = parent_label.ranksep + 25.0;
        sub.graph_mut().edgesep = parent_label.edgesep;
        sub.graph_mut().marginx = parent_label.marginx;
        sub.graph_mut().marginy = parent_label.marginy;
        sub.graph_mut().align = parent_label.align;
        sub.graph_mut().ranker = parent_label.ranker;
        sub.graph_mut().acyclicer = parent_label.acyclicer;

        inject_root_cluster_node(&mut sub, cluster_id)?;
        g = sub;
    }

    // Mirror the JS dagre harness normalization for compound-edge endpoints so the input graph is
    // identical for both the JS and Rust layout runs.
    normalize_cluster_edge_endpoints_like_harness(&mut g);

    let input_path = out_dir.join(format!("{fixture}.input.json"));
    let js_path = out_dir.join(format!("{fixture}.js.json"));
    let rust_path = out_dir.join(format!("{fixture}.rust.json"));

    let input = snapshot_input(&g)?;
    fs::write(&input_path, serde_json::to_string_pretty(&input)?).map_err(|source| {
        XtaskError::WriteFile {
            path: input_path.display().to_string(),
            source,
        }
    })?;

    let script_path = workspace_root
        .join("tools")
        .join("dagre-harness")
        .join("run.mjs");

    let status = Command::new("node")
        .arg(&script_path)
        .arg("--in")
        .arg(&input_path)
        .arg("--out")
        .arg(&js_path)
        .status()
        .map_err(|e| XtaskError::DebugSvgFailed(format!("failed to spawn node: {e}")))?;
    if !status.success() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "node dagre harness failed (exit={})",
            status.code().unwrap_or(-1)
        )));
    }

    let js_raw = fs::read_to_string(&js_path).map_err(|source| XtaskError::ReadFile {
        path: js_path.display().to_string(),
        source,
    })?;
    let js_out: JsonValue = serde_json::from_str(&js_raw)?;

    dugong::layout_dagreish(&mut g);
    let rust_out = snapshot_output(&g)?;
    fs::write(&rust_path, serde_json::to_string_pretty(&rust_out)?).map_err(|source| {
        XtaskError::WriteFile {
            path: rust_path.display().to_string(),
            source,
        }
    })?;

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

    let mut max_node_delta = 0.0f64;
    let mut max_node_id: Option<String> = None;

    for id in g.node_ids() {
        let Some(n) = g.node(&id) else {
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

    for ek in g.edge_keys() {
        let Some(e) = g.edge_by_key(&ek) else {
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

    println!("diagram: {diagram}");
    println!("fixture: {fixture}");
    println!("input:   {}", input_path.display());
    println!("js:      {}", js_path.display());
    println!("rust:    {}", rust_path.display());
    println!(
        "max node delta: {:.6} (node={})",
        max_node_delta,
        max_node_id.as_deref().unwrap_or("<none>")
    );
    println!(
        "max edge delta: {:.6} (edge={})",
        max_edge_delta,
        max_edge_id.as_deref().unwrap_or("<none>")
    );

    Ok(())
}

//! Dagre layout debug utilities.

use super::dagre_reference::{
    DagreLayoutGraph, DagreReferenceArtifacts, compare_graph_to_js_reference,
    normalize_cluster_edge_endpoints_like_harness, run_js_dagre_harness,
    write_dagre_reference_input, write_rust_dagre_output,
};
use crate::XtaskError;
use std::fs;
use std::path::PathBuf;

pub(crate) fn compare_dagre_layout(args: Vec<String>) -> Result<(), XtaskError> {
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

    let workspace_root = crate::cmd::workspace_root();
    let fixtures_dir = crate::cmd::fixtures_root().join(&diagram);
    let mmd_path = fixtures_dir.join(format!("{fixture}.mmd"));
    let text = fs::read_to_string(&mmd_path).map_err(|source| XtaskError::ReadFile {
        path: mmd_path.display().to_string(),
        source,
    })?;

    let out_dir = out_dir.unwrap_or_else(|| {
        crate::cmd::target_root()
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

    fn inject_root_cluster_node(g: &mut DagreLayoutGraph, root_id: &str) -> Result<(), XtaskError> {
        if !g.has_node(root_id) {
            g.set_node(
                root_id.to_string(),
                dugong::NodeLabel {
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

    let artifacts = DagreReferenceArtifacts::for_fixture(&out_dir, &fixture);
    write_dagre_reference_input(&g, &artifacts.input_path)?;
    run_js_dagre_harness(&workspace_root, &artifacts.input_path, &artifacts.js_path)?;

    dugong::layout_dagreish(&mut g);
    write_rust_dagre_output(&g, &artifacts.rust_path)?;
    let comparison = compare_graph_to_js_reference(&g, &artifacts.js_path)?;

    println!("diagram: {diagram}");
    println!("fixture: {fixture}");
    println!("input:   {}", artifacts.input_path.display());
    println!("js:      {}", artifacts.js_path.display());
    println!("rust:    {}", artifacts.rust_path.display());
    println!(
        "max node delta: {:.6} (node={})",
        comparison.max_node_delta,
        comparison.max_node_id.as_deref().unwrap_or("<none>")
    );
    println!(
        "max edge delta: {:.6} (edge={})",
        comparison.max_edge_delta,
        comparison.max_edge_id.as_deref().unwrap_or("<none>")
    );

    Ok(())
}

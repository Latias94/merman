//! Compare all diagram SVGs under fixtures.

use crate::XtaskError;
use std::fs;
use std::path::PathBuf;

use super::diagrams::*;

pub(crate) fn compare_all_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut check_dom: bool = false;
    let mut dom_mode: Option<String> = None;
    let mut dom_decimals: Option<u32> = None;
    let mut filter: Option<String> = None;
    let mut flowchart_text_measurer: Option<String> = None;
    let mut report_root: bool = false;

    let mut only_diagrams: Vec<String> = Vec::new();
    let mut skip_diagrams: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--check-dom" => check_dom = true,
            "--dom-mode" => {
                i += 1;
                dom_mode = args.get(i).map(|s| s.trim().to_string());
            }
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.trim().parse::<u32>().ok());
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--flowchart-text-measurer" => {
                i += 1;
                flowchart_text_measurer = args.get(i).map(|s| s.trim().to_ascii_lowercase());
            }
            "--report-root" => report_root = true,
            "--diagram" => {
                i += 1;
                let d = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
                if !d.is_empty() {
                    only_diagrams.push(d);
                }
            }
            "--skip" => {
                i += 1;
                let d = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
                if !d.is_empty() {
                    skip_diagrams.push(d);
                }
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let mut diagrams: Vec<&str> = vec![
        "er",
        "flowchart",
        "state",
        "class",
        "sequence",
        "info",
        "pie",
        "sankey",
        "packet",
        "timeline",
        "journey",
        "kanban",
        "gitgraph",
        "gantt",
        "c4",
        "block",
        "radar",
        "requirement",
        "mindmap",
        "architecture",
        "quadrantchart",
        "treemap",
        "xychart",
    ];

    if !only_diagrams.is_empty() {
        let only: Vec<String> = only_diagrams
            .iter()
            .map(|s| s.trim().to_ascii_lowercase())
            .collect();
        diagrams.retain(|d| only.iter().any(|o| o == d));
    }

    if !skip_diagrams.is_empty() {
        let skip: Vec<String> = skip_diagrams
            .iter()
            .map(|s| s.trim().to_ascii_lowercase())
            .collect();
        diagrams.retain(|d| !skip.iter().any(|s| s == d));
    }

    if diagrams.is_empty() {
        return Err(XtaskError::Usage);
    }

    fn dom_mode_slug(mode: &str) -> String {
        let mut out = String::with_capacity(mode.len());
        let mut prev_underscore = false;
        for ch in mode.trim().chars() {
            if ch.is_ascii_alphanumeric() {
                prev_underscore = false;
                out.push(ch.to_ascii_lowercase());
            } else {
                if prev_underscore {
                    continue;
                }
                prev_underscore = true;
                out.push('_');
            }
        }
        out.trim_matches('_').to_string()
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let compare_dir = workspace_root.join("target").join("compare");
    fs::create_dir_all(&compare_dir).map_err(|source| XtaskError::WriteFile {
        path: compare_dir.display().to_string(),
        source,
    })?;

    fn common_compare_args(
        check_dom: bool,
        dom_mode: Option<&str>,
        dom_decimals: Option<u32>,
        filter: Option<&str>,
    ) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if check_dom {
            out.push("--check-dom".to_string());
        }
        if let Some(mode) = dom_mode {
            out.push("--dom-mode".to_string());
            out.push(mode.to_string());
        }
        if let Some(n) = dom_decimals {
            out.push("--dom-decimals".to_string());
            out.push(n.to_string());
        }
        if let Some(f) = filter {
            out.push("--filter".to_string());
            out.push(f.to_string());
        }
        out
    }

    let mut failures: Vec<String> = Vec::new();

    for diagram in diagrams {
        println!("\n== compare {diagram} ==");

        let mut cmd_args = common_compare_args(
            check_dom,
            dom_mode.as_deref(),
            dom_decimals,
            filter.as_deref(),
        );

        // Avoid overwriting reports across multiple runs (e.g. `parity` then `parity-root`).
        // When a dom mode is specified, we emit mode-suffixed reports:
        // `target/compare/<diagram>_report_<mode>.md` (e.g. `state_report_parity_root.md`).
        if let Some(ref mode) = dom_mode {
            let mode = dom_mode_slug(mode);
            if !mode.is_empty() {
                cmd_args.push("--out".to_string());
                cmd_args.push(
                    compare_dir
                        .join(format!("{diagram}_report_{mode}.md"))
                        .display()
                        .to_string(),
                );
            }
        }

        if diagram == "flowchart" {
            if let Some(tm) = flowchart_text_measurer.as_deref() {
                cmd_args.push("--text-measurer".to_string());
                cmd_args.push(tm.to_string());
            }
            if report_root {
                cmd_args.push("--report-root".to_string());
            }
        }

        let res = match diagram {
            "er" => compare_er_svgs(cmd_args),
            "flowchart" => compare_flowchart_svgs(cmd_args),
            "state" => compare_state_svgs(cmd_args),
            "class" => compare_class_svgs(cmd_args),
            "sequence" => compare_sequence_svgs(cmd_args),
            "info" => compare_info_svgs(cmd_args),
            "pie" => compare_pie_svgs(cmd_args),
            "sankey" => compare_sankey_svgs(cmd_args),
            "packet" => compare_packet_svgs(cmd_args),
            "timeline" => compare_timeline_svgs(cmd_args),
            "journey" => compare_journey_svgs(cmd_args),
            "kanban" => compare_kanban_svgs(cmd_args),
            "gitgraph" => compare_gitgraph_svgs(cmd_args),
            "gantt" => compare_gantt_svgs(cmd_args),
            "c4" => compare_c4_svgs(cmd_args),
            "block" => compare_block_svgs(cmd_args),
            "radar" => compare_radar_svgs(cmd_args),
            "requirement" => compare_requirement_svgs(cmd_args),
            "mindmap" => compare_mindmap_svgs(cmd_args),
            "architecture" => compare_architecture_svgs(cmd_args),
            "quadrantchart" => compare_quadrantchart_svgs(cmd_args),
            "treemap" => compare_treemap_svgs(cmd_args),
            "xychart" => compare_xychart_svgs(cmd_args),
            other => Err(XtaskError::SvgCompareFailed(format!(
                "unexpected diagram: {other}"
            ))),
        };

        match res {
            Ok(()) => {}
            Err(XtaskError::SvgCompareFailed(msg))
                if filter.is_some()
                    && only_diagrams.is_empty()
                    && msg.contains("no .mmd fixtures matched under ") =>
            {
                println!("(skipped: {msg})");
            }
            Err(err) => failures.push(format!("{diagram}: {err}")),
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

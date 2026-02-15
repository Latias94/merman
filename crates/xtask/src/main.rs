use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use regex::Regex;

mod state_svgdump;
mod svgdom;

#[derive(Debug, thiserror::Error)]
enum XtaskError {
    #[error("usage: xtask <command> ...")]
    Usage,
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("failed to read file {path}: {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write file {path}: {source}")]
    WriteFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse YAML schema: {0}")]
    ParseYaml(#[from] serde_yaml::Error),
    #[error("failed to process JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid $ref: {0}")]
    InvalidRef(String),
    #[error("unresolved $ref: {0}")]
    UnresolvedRef(String),
    #[error("failed to parse dompurify dist file: {0}")]
    ParseDompurify(String),
    #[error("verification failed:\n{0}")]
    VerifyFailed(String),
    #[error("snapshot update failed: {0}")]
    SnapshotUpdateFailed(String),
    #[error("layout snapshot update failed: {0}")]
    LayoutSnapshotUpdateFailed(String),
    #[error("alignment check failed:\n{0}")]
    AlignmentCheckFailed(String),
    #[error("debug svg generation failed:\n{0}")]
    DebugSvgFailed(String),
    #[error("upstream svg generation failed:\n{0}")]
    UpstreamSvgFailed(String),
    #[error("svg compare failed:\n{0}")]
    SvgCompareFailed(String),
}

fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension().is_some_and(|e| e == ext)
}

fn is_file_with_extension(path: &Path, ext: &str) -> bool {
    path.is_file() && has_extension(path, ext)
}

fn print_help(topic: Option<&str>) {
    if let Some(topic) = topic.filter(|t| !t.trim().is_empty()) {
        println!("usage: xtask {topic} ...");
        println!();
        println!("This repository uses a lightweight custom CLI parser for xtask commands.");
        println!("Most subcommands accept `--help`/`-h` and will show a usage error.");
        println!();
        println!("See: `crates/xtask/src/main.rs` for the full argument grammar.");
        return;
    }

    println!("usage: xtask <command> ...");
    println!();
    println!("Common commands:");
    println!("  check-alignment");
    println!("  import-upstream-docs");
    println!("  import-upstream-html");
    println!("  import-upstream-cypress");
    println!("  import-mmdr-fixtures");
    println!("  update-snapshots");
    println!("  update-layout-snapshots   (alias: gen-layout-goldens)");
    println!("  gen-upstream-svgs");
    println!("  check-upstream-svgs");
    println!("  compare-all-svgs");
    println!("  compare-svg-xml");
    println!("  canon-svg-xml");
    println!("  debug-svg-bbox");
    println!("  debug-svg-data-points");
    println!("  debug-architecture-delta");
    println!("  summarize-architecture-deltas");
    println!("  compare-dagre-layout");
    println!("  analyze-state-fixture");
    println!("  debug-mindmap-svg-positions");
    println!("  report-overrides");
    println!();
    println!("Per-diagram SVG compare commands:");
    println!("  compare-er-svgs");
    println!("  compare-flowchart-svgs");
    println!("  compare-sequence-svgs");
    println!("  compare-class-svgs");
    println!("  compare-state-svgs");
    println!("  compare-info-svgs");
    println!("  compare-pie-svgs");
    println!("  compare-sankey-svgs");
    println!("  compare-packet-svgs");
    println!("  compare-timeline-svgs");
    println!("  compare-journey-svgs");
    println!("  compare-kanban-svgs");
    println!("  compare-gitgraph-svgs");
    println!("  compare-gantt-svgs");
    println!("  compare-c4-svgs");
    println!("  compare-block-svgs");
    println!("  compare-radar-svgs");
    println!("  compare-requirement-svgs");
    println!("  compare-mindmap-svgs");
    println!("  compare-architecture-svgs");
    println!("  compare-quadrantchart-svgs");
    println!("  compare-treemap-svgs");
    println!("  compare-xychart-svgs");
    println!();
    println!("Tips:");
    println!("  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3`");
    println!("  - `cargo run -p xtask -- gen-upstream-svgs --diagram <name>`");
    println!();
    println!("Topics:");
    println!("  xtask help <command>");
}

fn main() -> Result<(), XtaskError> {
    let mut args = std::env::args().skip(1);
    let Some(cmd) = args.next() else {
        return Err(XtaskError::Usage);
    };

    if matches!(cmd.as_str(), "--help" | "-h") {
        print_help(None);
        return Ok(());
    }
    if cmd == "help" {
        print_help(args.next().as_deref());
        return Ok(());
    }

    match cmd.as_str() {
        "gen-default-config" => gen_default_config(args.collect()),
        "gen-dompurify-defaults" => gen_dompurify_defaults(args.collect()),
        "verify-generated" => verify_generated(args.collect()),
        "import-upstream-docs" => import_upstream_docs(args.collect()),
        "import-upstream-html" => import_upstream_html(args.collect()),
        "import-upstream-cypress" => import_upstream_cypress(args.collect()),
        "import-mmdr-fixtures" => import_mmdr_fixtures(args.collect()),
        "update-snapshots" => update_snapshots(args.collect()),
        "update-layout-snapshots" | "gen-layout-goldens" => update_layout_snapshots(args.collect()),
        "check-alignment" => check_alignment(args.collect()),
        "gen-debug-svgs" => gen_debug_svgs(args.collect()),
        "gen-er-svgs" => gen_er_svgs(args.collect()),
        "gen-flowchart-svgs" => gen_flowchart_svgs(args.collect()),
        "gen-state-svgs" => gen_state_svgs(args.collect()),
        "gen-class-svgs" => gen_class_svgs(args.collect()),
        "gen-c4-svgs" => gen_c4_svgs(args.collect()),
        "gen-c4-textlength" => gen_c4_textlength(args.collect()),
        "gen-font-metrics" => gen_font_metrics(args.collect()),
        "gen-svg-overrides" => gen_svg_overrides(args.collect()),
        "gen-er-text-overrides" => gen_er_text_overrides(args.collect()),
        "gen-mindmap-text-overrides" => gen_mindmap_text_overrides(args.collect()),
        "gen-gantt-text-overrides" => gen_gantt_text_overrides(args.collect()),
        "measure-text" => measure_text(args.collect()),
        "gen-upstream-svgs" => gen_upstream_svgs(args.collect()),
        "check-upstream-svgs" => check_upstream_svgs(args.collect()),
        "compare-er-svgs" => compare_er_svgs(args.collect()),
        "compare-flowchart-svgs" => compare_flowchart_svgs(args.collect()),
        "debug-flowchart-layout" => debug_flowchart_layout(args.collect()),
        "debug-flowchart-svg-roots" => debug_flowchart_svg_roots(args.collect()),
        "debug-flowchart-svg-positions" => debug_flowchart_svg_positions(args.collect()),
        "debug-flowchart-svg-diff" => debug_flowchart_svg_diff(args.collect()),
        "debug-flowchart-data-points" => debug_flowchart_data_points(args.collect()),
        "debug-flowchart-edge-trace" => debug_flowchart_edge_trace(args.collect()),
        "debug-mindmap-svg-positions" => debug_mindmap_svg_positions(args.collect()),
        "debug-svg-bbox" => debug_svg_bbox(args.collect()),
        "debug-svg-data-points" => debug_svg_data_points(args.collect()),
        "debug-architecture-delta" => debug_architecture_delta(args.collect()),
        "summarize-architecture-deltas" => summarize_architecture_deltas(args.collect()),
        "compare-dagre-layout" => compare_dagre_layout(args.collect()),
        "analyze-state-fixture" => state_svgdump::analyze_state_fixture(args.collect()),
        "compare-sequence-svgs" => compare_sequence_svgs(args.collect()),
        "compare-class-svgs" => compare_class_svgs(args.collect()),
        "compare-state-svgs" => compare_state_svgs(args.collect()),
        "compare-info-svgs" => compare_info_svgs(args.collect()),
        "compare-pie-svgs" => compare_pie_svgs(args.collect()),
        "compare-sankey-svgs" => compare_sankey_svgs(args.collect()),
        "compare-packet-svgs" => compare_packet_svgs(args.collect()),
        "compare-timeline-svgs" => compare_timeline_svgs(args.collect()),
        "compare-journey-svgs" => compare_journey_svgs(args.collect()),
        "compare-kanban-svgs" => compare_kanban_svgs(args.collect()),
        "compare-gitgraph-svgs" => compare_gitgraph_svgs(args.collect()),
        "compare-gantt-svgs" => compare_gantt_svgs(args.collect()),
        "compare-c4-svgs" => compare_c4_svgs(args.collect()),
        "compare-block-svgs" => compare_block_svgs(args.collect()),
        "compare-radar-svgs" => compare_radar_svgs(args.collect()),
        "compare-requirement-svgs" => compare_requirement_svgs(args.collect()),
        "compare-mindmap-svgs" => compare_mindmap_svgs(args.collect()),
        "compare-architecture-svgs" => compare_architecture_svgs(args.collect()),
        "compare-quadrantchart-svgs" => compare_quadrantchart_svgs(args.collect()),
        "compare-treemap-svgs" => compare_treemap_svgs(args.collect()),
        "compare-xychart-svgs" => compare_xychart_svgs(args.collect()),
        "compare-all-svgs" => compare_all_svgs(args.collect()),
        "compare-svg-xml" => compare_svg_xml(args.collect()),
        "canon-svg-xml" => canon_svg_xml(args.collect()),
        "report-overrides" => report_overrides(args.collect()),
        other => Err(XtaskError::UnknownCommand(other.to_string())),
    }
}

fn import_upstream_docs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;
    let mut limit: Option<usize> = None;
    let mut min_lines: Option<usize> = None;
    let mut prefer_complex: bool = false;
    let mut overwrite: bool = false;
    let mut with_baselines: bool = false;
    let mut install: bool = false;
    let mut docs_root: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--limit" => {
                i += 1;
                let raw = args.get(i).ok_or(XtaskError::Usage)?;
                limit = Some(raw.parse::<usize>().map_err(|_| XtaskError::Usage)?);
            }
            "--min-lines" => {
                i += 1;
                let raw = args.get(i).ok_or(XtaskError::Usage)?;
                min_lines = Some(raw.parse::<usize>().map_err(|_| XtaskError::Usage)?);
            }
            "--complex" => prefer_complex = true,
            "--overwrite" => overwrite = true,
            "--with-baselines" => with_baselines = true,
            "--install" => install = true,
            "--docs-root" => {
                i += 1;
                let raw = args.get(i).ok_or(XtaskError::Usage)?;
                docs_root = Some(PathBuf::from(raw));
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let docs_root = docs_root
        .map(|p| {
            if p.is_absolute() {
                p
            } else {
                workspace_root.join(p)
            }
        })
        .unwrap_or_else(|| {
            workspace_root
                .join("repo-ref")
                .join("mermaid")
                .join("docs")
                .join("syntax")
        });
    if !docs_root.exists() {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "upstream docs root not found: {} (expected repo-ref checkout of mermaid@11.12.2)",
            docs_root.display()
        )));
    }

    #[derive(Debug, Clone)]
    struct MdBlock {
        source_md: PathBuf,
        source_stem: String,
        idx_in_file: usize,
        heading: Option<String>,
        info: String,
        body: String,
    }

    #[derive(Debug, Clone)]
    struct CreatedFixture {
        diagram_dir: String,
        stem: String,
        path: PathBuf,
        source_md: PathBuf,
        source_idx_in_file: usize,
        source_info: String,
        source_heading: Option<String>,
    }

    fn slugify(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut prev_us = false;
        for ch in s.chars() {
            let ch = ch.to_ascii_lowercase();
            if ch.is_ascii_alphanumeric() {
                out.push(ch);
                prev_us = false;
            } else if !prev_us {
                out.push('_');
                prev_us = true;
            }
        }
        while out.starts_with('_') {
            out.remove(0);
        }
        while out.ends_with('_') {
            out.pop();
        }
        if out.is_empty() {
            "untitled".to_string()
        } else {
            out
        }
    }

    fn clamp_slug(mut s: String, max_len: usize) -> String {
        if s.len() <= max_len {
            return s;
        }
        s.truncate(max_len);
        while s.ends_with('_') {
            s.pop();
        }
        if s.is_empty() {
            "untitled".to_string()
        } else {
            s
        }
    }

    fn canonical_fixture_text(s: &str) -> String {
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        let s = s.trim_matches('\n');
        format!("{s}\n")
    }

    fn extract_md_blocks(md_path: &Path) -> Result<Vec<MdBlock>, XtaskError> {
        let text = fs::read_to_string(md_path).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to read markdown file {}: {err}",
                md_path.display()
            ))
        })?;

        let source_stem = md_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut out = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0usize;
        let mut current_heading: Option<String> = None;
        let mut idx_in_file = 0usize;
        while i < lines.len() {
            let line = lines[i];
            if let Some(h) = line.strip_prefix('#') {
                current_heading = Some(h.trim().trim_start_matches('#').trim().to_string());
            }

            let trimmed = line.trim_start();
            if trimmed.starts_with("```") {
                let ticks = trimmed.chars().take_while(|c| *c == '`').count();
                let info = trimmed[ticks..].trim().to_string();
                i += 1;
                let mut body_lines: Vec<&str> = Vec::new();
                while i < lines.len() {
                    let l = lines[i];
                    if l.trim_start().starts_with(&"`".repeat(ticks)) {
                        break;
                    }
                    body_lines.push(l);
                    i += 1;
                }

                let body = body_lines.join("\n");
                out.push(MdBlock {
                    source_md: md_path.to_path_buf(),
                    source_stem: source_stem.clone(),
                    idx_in_file,
                    heading: current_heading.clone(),
                    info,
                    body,
                });
                idx_in_file += 1;
            }

            i += 1;
        }

        Ok(out)
    }

    fn docs_md_for_diagram(diagram: &str) -> Option<&'static str> {
        match diagram {
            "all" => None,
            "architecture" => Some("architecture.md"),
            "block" => Some("block.md"),
            "c4" => Some("c4.md"),
            "class" => Some("classDiagram.md"),
            "er" => Some("entityRelationshipDiagram.md"),
            "flowchart" => Some("flowchart.md"),
            "gantt" => Some("gantt.md"),
            "gitgraph" => Some("gitgraph.md"),
            "kanban" => Some("kanban.md"),
            "mindmap" => Some("mindmap.md"),
            "packet" => Some("packet.md"),
            "pie" => Some("pie.md"),
            "quadrantchart" => Some("quadrantChart.md"),
            "radar" => Some("radar.md"),
            "requirement" => Some("requirementDiagram.md"),
            "sankey" => Some("sankey.md"),
            "sequence" => Some("sequenceDiagram.md"),
            "state" => Some("stateDiagram.md"),
            "timeline" => Some("timeline.md"),
            "treemap" => Some("treemap.md"),
            "journey" => Some("userJourney.md"),
            "xychart" => Some("xyChart.md"),
            _ => None,
        }
    }

    fn collect_markdown_files_recursively(
        root: &Path,
        out: &mut Vec<PathBuf>,
    ) -> Result<(), XtaskError> {
        if root.is_file() {
            if root.extension().is_some_and(|e| e == "md") {
                out.push(root.to_path_buf());
            }
            return Ok(());
        }
        let entries = fs::read_dir(root).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to list docs directory {}: {err}",
                root.display()
            ))
        })?;
        for entry in entries {
            let path = entry
                .map_err(|err| {
                    XtaskError::SnapshotUpdateFailed(format!(
                        "failed to read docs directory entry under {}: {err}",
                        root.display()
                    ))
                })?
                .path();
            if path.is_dir() {
                collect_markdown_files_recursively(&path, out)?;
            } else if path.extension().is_some_and(|e| e == "md") {
                out.push(path);
            }
        }
        Ok(())
    }

    fn normalize_diagram_dir(detected: &str) -> Option<String> {
        match detected {
            "flowchart" | "flowchart-v2" | "flowchart-elk" => Some("flowchart".to_string()),
            "state" | "stateDiagram" => Some("state".to_string()),
            "class" | "classDiagram" => Some("class".to_string()),
            "gitGraph" => Some("gitgraph".to_string()),
            "quadrantChart" => Some("quadrantchart".to_string()),
            "er" => Some("er".to_string()),
            "journey" => Some("journey".to_string()),
            "xychart" => Some("xychart".to_string()),
            "requirement" => Some("requirement".to_string()),
            "architecture-beta" => Some("architecture".to_string()),
            "architecture" | "block" | "c4" | "gantt" | "info" | "kanban" | "mindmap"
            | "packet" | "pie" | "radar" | "sankey" | "sequence" | "timeline" | "treemap" => {
                Some(detected.to_string())
            }
            _ => None,
        }
    }

    let mut md_files: Vec<PathBuf> = Vec::new();
    if diagram == "all" {
        collect_markdown_files_recursively(&docs_root, &mut md_files)?;
    } else if docs_root.ends_with(PathBuf::from("docs").join("syntax")) {
        let Some(name) = docs_md_for_diagram(&diagram) else {
            return Err(XtaskError::SnapshotUpdateFailed(format!(
                "unknown diagram: {diagram} (expected one of the fixtures/ subfolders, or 'all')"
            )));
        };
        md_files.push(docs_root.join(name));
    } else {
        // When a custom docs root is provided, scan all markdown files under it and rely on diagram detection.
        collect_markdown_files_recursively(&docs_root, &mut md_files)?;
    }
    md_files.sort();

    let allowed_infos = [
        "",
        "mermaid",
        "mermaid-example",
        "mermaid-nocode",
        "architecture",
        "block",
        "c4",
        "classDiagram",
        "erDiagram",
        "flowchart",
        "gantt",
        "gitGraph",
        "kanban",
        "mindmap",
        "packet",
        "pie",
        "quadrantChart",
        "radar",
        "requirementDiagram",
        "sankey",
        "sequenceDiagram",
        "stateDiagram",
        "timeline",
        "treemap",
        "userJourney",
        "xyChart",
        "xychart",
    ];

    let reg = merman::detect::DetectorRegistry::default_mermaid_11_12_2_full();
    let mut created: Vec<CreatedFixture> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    let mut existing_by_diagram: std::collections::HashMap<
        String,
        std::collections::HashMap<String, PathBuf>,
    > = std::collections::HashMap::new();

    fn load_existing_fixtures(fixtures_dir: &Path) -> std::collections::HashMap<String, PathBuf> {
        let mut map = std::collections::HashMap::new();
        let Ok(entries) = fs::read_dir(fixtures_dir) else {
            return map;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "mmd") {
                if let Ok(text) = fs::read_to_string(&path) {
                    let canon = canonical_fixture_text(&text);
                    map.insert(canon, path);
                }
            }
        }
        map
    }

    #[derive(Debug, Clone)]
    struct Candidate {
        md_block: MdBlock,
        diagram_dir: String,
        fixtures_dir: PathBuf,
        stem: String,
        body: String,
        score: i64,
    }

    fn complexity_score(body: &str, diagram_dir: &str) -> i64 {
        let line_count = body.lines().count() as i64;
        let mut score = line_count * 1_000 + (body.len() as i64);
        let lower = body.to_ascii_lowercase();

        fn bump(score: &mut i64, lower: &str, needle: &str, weight: i64) {
            if lower.contains(needle) {
                *score += weight;
            }
        }

        // Global "complexity" markers across diagrams.
        bump(&mut score, &lower, "%%{init", 5_000);
        bump(&mut score, &lower, "accdescr", 2_000);
        bump(&mut score, &lower, "acctitle", 2_000);
        bump(&mut score, &lower, "linkstyle", 2_000);
        bump(&mut score, &lower, "classdef", 2_000);
        bump(&mut score, &lower, "direction", 1_000);
        bump(&mut score, &lower, "click ", 1_500);
        bump(&mut score, &lower, "<img", 1_000);
        bump(&mut score, &lower, "<strong>", 1_000);
        bump(&mut score, &lower, "<em>", 1_000);

        match diagram_dir {
            "flowchart" => {
                bump(&mut score, &lower, "subgraph", 2_000);
                bump(&mut score, &lower, ":::", 1_000);
                bump(&mut score, &lower, "@{", 1_500);
            }
            "sequence" => {
                bump(&mut score, &lower, "alt", 1_500);
                bump(&mut score, &lower, "loop", 1_500);
                bump(&mut score, &lower, "par", 1_500);
                bump(&mut score, &lower, "opt", 1_000);
                bump(&mut score, &lower, "critical", 1_500);
                bump(&mut score, &lower, "rect", 1_000);
                bump(&mut score, &lower, "activate", 1_000);
                bump(&mut score, &lower, "deactivate", 1_000);
            }
            "class" => {
                bump(&mut score, &lower, "namespace", 1_000);
                bump(&mut score, &lower, "interface", 1_000);
                bump(&mut score, &lower, "enum", 1_000);
                bump(&mut score, &lower, "<<", 1_000);
            }
            "state" => {
                bump(&mut score, &lower, "fork", 1_000);
                bump(&mut score, &lower, "join", 1_000);
                bump(&mut score, &lower, "[*]", 1_000);
                bump(&mut score, &lower, "note", 1_000);
            }
            "gantt" => {
                bump(&mut score, &lower, "section", 1_000);
                bump(&mut score, &lower, "crit", 1_000);
                bump(&mut score, &lower, "milestone", 1_000);
                bump(&mut score, &lower, "after", 1_000);
            }
            _ => {}
        }

        score
    }

    let mut candidates: Vec<Candidate> = Vec::new();

    for md_path in md_files {
        if !md_path.is_file() {
            skipped.push(format!("missing markdown source: {}", md_path.display()));
            continue;
        }

        let blocks = extract_md_blocks(&md_path)?;
        let source_stem = md_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let source_slug = clamp_slug(slugify(source_stem), 48);

        for b in blocks {
            if !allowed_infos.iter().any(|v| *v == b.info) {
                continue;
            }
            if let Some(f) = filter.as_deref() {
                let h = b.heading.clone().unwrap_or_default();
                if !b.source_stem.contains(f) && !h.contains(f) {
                    continue;
                }
            }

            let body = canonical_fixture_text(&b.body);
            if body.trim().is_empty() {
                continue;
            }
            if let Some(min) = min_lines {
                if body.lines().count() < min {
                    continue;
                }
            }

            let mut cfg = merman::MermaidConfig::default();
            let detected = match reg.detect_type(body.as_str(), &mut cfg) {
                Ok(t) => t,
                Err(_) => {
                    skipped.push(format!(
                        "skip (type not detected): {} (info='{}', idx={})",
                        b.source_md.display(),
                        b.info,
                        b.idx_in_file
                    ));
                    continue;
                }
            };
            let Some(diagram_dir) = normalize_diagram_dir(detected) else {
                skipped.push(format!(
                    "skip (unsupported detected type '{detected}'): {}",
                    b.source_md.display()
                ));
                continue;
            };

            // External plugin diagrams (like zenuml) are out of scope for now.
            if diagram_dir == "zenuml" {
                continue;
            }
            if diagram != "all" && diagram_dir != diagram {
                continue;
            }

            let fixtures_dir = workspace_root.join("fixtures").join(&diagram_dir);
            if !fixtures_dir.is_dir() {
                skipped.push(format!(
                    "skip (fixtures dir missing): {}",
                    fixtures_dir.display()
                ));
                continue;
            }

            let heading_slug = clamp_slug(slugify(b.heading.as_deref().unwrap_or("example")), 64);
            let stem = format!(
                "upstream_docs_{source_slug}_{heading_slug}_{idx:03}",
                idx = b.idx_in_file + 1
            );

            let score = complexity_score(&body, &diagram_dir);
            candidates.push(Candidate {
                md_block: b,
                diagram_dir,
                fixtures_dir,
                stem,
                body,
                score,
            });
        }
    }

    if prefer_complex {
        candidates.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.stem.cmp(&b.stem)));
    }

    if install && !with_baselines {
        return Err(XtaskError::SnapshotUpdateFailed(
            "`--install` only applies when `--with-baselines` is set".to_string(),
        ));
    }

    fn deferred_with_baselines_reason(
        diagram_dir: &str,
        fixture_text: &str,
    ) -> Option<&'static str> {
        // Keep `--with-baselines` aligned with the current parity hardening scope.
        //
        // Some examples require upstream (browser) features we have not yet replicated in the
        // headless pipeline. Import them later in dedicated parity work items (tracked in
        // `docs/alignment/FIXTURE_EXPANSION_TODO.md`).
        match diagram_dir {
            "flowchart" => {
                // ELK layout is currently out of scope for the headless layout engine.
                if fixture_text.contains("\n  layout: elk")
                    || fixture_text.contains("\nlayout: elk")
                {
                    return Some("flowchart frontmatter config.layout=elk (deferred)");
                }
                // Flowchart "look" variants change DOM structure and markers; only classic is in scope.
                if fixture_text.contains("\n  look:") || fixture_text.contains("\nlook:") {
                    if !fixture_text.contains("\n  look: classic")
                        && !fixture_text.contains("\nlook: classic")
                    {
                        return Some("flowchart frontmatter config.look!=classic (deferred)");
                    }
                }
                // Math rendering depends on browser KaTeX + foreignObject details.
                if fixture_text.contains("$$") {
                    return Some("flowchart math (deferred)");
                }
            }
            "sequence" => {
                // Math rendering depends on browser KaTeX + font metrics.
                if fixture_text.contains("$$") {
                    return Some("sequence math (deferred)");
                }
                // Some docs examples rely on wrap/width behavior not yet matched in headless layout.
                if fixture_text.contains("%%{init:")
                    && (fixture_text.contains("\"wrap\": true")
                        || fixture_text.contains("\"width\""))
                {
                    return Some("sequence wrap/width directive (deferred)");
                }
            }
            _ => {}
        }
        None
    }

    fn is_suspicious_blank_svg(svg_path: &Path) -> bool {
        // Mermaid CLI often emits a tiny 16x16 SVG for "empty" diagrams (e.g. `graph LR` with
        // no nodes/edges). These are usually unhelpful as parity fixtures and tend to create
        // noisy root viewport diffs.
        //
        // Treat them as "output anomalies" for fixture import purposes: keep the candidate
        // traceable via the report and skip importing it for now.
        let Ok(head) = fs::read_to_string(svg_path) else {
            return false;
        };
        let first = head.lines().next().unwrap_or_default();
        first.contains(r#"viewBox="-8 -8 16 16""#)
            || first.contains(r#"viewBox="0 0 16 16""#)
            || first.contains(r#"style="max-width: 16px"#)
    }

    fn cleanup_fixture_files(workspace_root: &Path, f: &CreatedFixture) {
        let _ = fs::remove_file(&f.path);
        let _ = fs::remove_file(
            workspace_root
                .join("fixtures")
                .join("upstream-svgs")
                .join(&f.diagram_dir)
                .join(format!("{}.svg", f.stem)),
        );
        let _ = fs::remove_file(
            workspace_root
                .join("fixtures")
                .join(&f.diagram_dir)
                .join(format!("{}.golden.json", f.stem)),
        );
        let _ = fs::remove_file(
            workspace_root
                .join("fixtures")
                .join(&f.diagram_dir)
                .join(format!("{}.layout.golden.json", f.stem)),
        );
    }

    let report_path = workspace_root
        .join("target")
        .join("import-upstream-docs.report.txt");
    let mut report_lines: Vec<String> = Vec::new();

    let mut imported = 0usize;
    for c in candidates {
        let existing = existing_by_diagram
            .entry(c.diagram_dir.clone())
            .or_insert_with(|| load_existing_fixtures(&c.fixtures_dir));
        if let Some(existing_path) = existing.get(&c.body) {
            skipped.push(format!(
                "skip (duplicate content): {} -> {}",
                c.md_block.source_md.display(),
                existing_path.display()
            ));
            continue;
        }

        let out_path = c.fixtures_dir.join(format!("{}.mmd", c.stem));
        if out_path.exists() && !overwrite {
            skipped.push(format!("skip (exists): {}", out_path.display()));
            continue;
        }

        fs::write(&out_path, &c.body).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to write fixture {}: {err}",
                out_path.display()
            ))
        })?;

        let f = CreatedFixture {
            diagram_dir: c.diagram_dir,
            stem: c.stem,
            path: out_path.clone(),
            source_md: c.md_block.source_md.clone(),
            source_idx_in_file: c.md_block.idx_in_file,
            source_info: c.md_block.info.clone(),
            source_heading: c.md_block.heading.clone(),
        };

        if !with_baselines {
            existing.insert(c.body.clone(), out_path);
            created.push(f);
            imported += 1;
            if let Some(max) = limit {
                if imported >= max {
                    break;
                }
            }
            continue;
        }

        // `--with-baselines`: treat `--limit` as the number of fixtures that survive upstream
        // rendering + snapshot updates (instead of the number of files written).
        if let Some(reason) = deferred_with_baselines_reason(&f.diagram_dir, &c.body) {
            report_lines.push(format!(
                "DEFERRED_WITH_BASELINES\t{}\t{}\t{}\tblock_idx={}\tinfo={}\theading={}\treason={reason}",
                f.diagram_dir,
                f.stem,
                f.source_md.display(),
                f.source_idx_in_file,
                f.source_info,
                f.source_heading.clone().unwrap_or_default(),
            ));
            skipped.push(format!(
                "skip (deferred for --with-baselines): {} ({reason})",
                f.path.display(),
            ));
            cleanup_fixture_files(&workspace_root, &f);
            continue;
        }

        let mut svg_args = vec![
            "--diagram".to_string(),
            f.diagram_dir.clone(),
            "--filter".to_string(),
            f.stem.clone(),
        ];
        if install {
            svg_args.push("--install".to_string());
        }
        match gen_upstream_svgs(svg_args) {
            Ok(()) => {}
            Err(XtaskError::UpstreamSvgFailed(msg)) => {
                report_lines.push(format!(
                    "UPSTREAM_SVG_FAILED\t{}\t{}\t{}\tblock_idx={}\tinfo={}\theading={}\tmsg={}",
                    f.diagram_dir,
                    f.stem,
                    f.source_md.display(),
                    f.source_idx_in_file,
                    f.source_info,
                    f.source_heading.clone().unwrap_or_default(),
                    msg.lines().next().unwrap_or("unknown upstream error"),
                ));
                skipped.push(format!(
                    "skip (upstream svg failed): {} ({})",
                    f.path.display(),
                    msg.lines().next().unwrap_or("unknown upstream error")
                ));
                cleanup_fixture_files(&workspace_root, &f);
                continue;
            }
            Err(other) => return Err(other),
        }

        let svg_path = workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join(&f.diagram_dir)
            .join(format!("{}.svg", f.stem));
        if is_suspicious_blank_svg(&svg_path) {
            report_lines.push(format!(
                "UPSTREAM_SVG_SUSPICIOUS_BLANK\t{}\t{}\t{}\tblock_idx={}\tinfo={}\theading={}",
                f.diagram_dir,
                f.stem,
                f.source_md.display(),
                f.source_idx_in_file,
                f.source_info,
                f.source_heading.clone().unwrap_or_default(),
            ));
            skipped.push(format!(
                "skip (suspicious upstream svg output): {} (blank 16x16-like svg)",
                f.path.display(),
            ));
            cleanup_fixture_files(&workspace_root, &f);
            continue;
        }

        if let Err(err) = update_snapshots(vec![
            "--diagram".to_string(),
            f.diagram_dir.clone(),
            "--filter".to_string(),
            f.stem.clone(),
        ]) {
            report_lines.push(format!(
                "SNAPSHOT_UPDATE_FAILED\t{}\t{}\t{}\tblock_idx={}\tinfo={}\theading={}\terr={err}",
                f.diagram_dir,
                f.stem,
                f.source_md.display(),
                f.source_idx_in_file,
                f.source_info,
                f.source_heading.clone().unwrap_or_default(),
            ));
            skipped.push(format!(
                "skip (snapshot update failed): {} ({err})",
                f.path.display(),
            ));
            cleanup_fixture_files(&workspace_root, &f);
            continue;
        }
        if let Err(err) = update_layout_snapshots(vec![
            "--diagram".to_string(),
            f.diagram_dir.clone(),
            "--filter".to_string(),
            f.stem.clone(),
        ]) {
            report_lines.push(format!(
                "LAYOUT_SNAPSHOT_UPDATE_FAILED\t{}\t{}\t{}\tblock_idx={}\tinfo={}\theading={}\terr={err}",
                f.diagram_dir,
                f.stem,
                f.source_md.display(),
                f.source_idx_in_file,
                f.source_info,
                f.source_heading.clone().unwrap_or_default(),
            ));
            skipped.push(format!(
                "skip (layout snapshot update failed): {} ({err})",
                f.path.display(),
            ));
            cleanup_fixture_files(&workspace_root, &f);
            continue;
        }

        existing.insert(c.body.clone(), out_path);
        created.push(f);
        imported += 1;
        if let Some(max) = limit {
            if imported >= max {
                break;
            }
        }
    }

    if with_baselines && !report_lines.is_empty() {
        if let Some(parent) = report_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let header = format!(
            "# import-upstream-docs report (Mermaid@11.12.2)\n# generated_at={}\n",
            chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%z")
        );
        let mut out = String::new();
        out.push_str(&header);
        out.push_str(&report_lines.join("\n"));
        out.push('\n');
        let _ = fs::write(&report_path, out);
        eprintln!("Wrote import report: {}", report_path.display());
    }

    if created.is_empty() {
        return Err(XtaskError::SnapshotUpdateFailed(if with_baselines {
            "no fixtures were imported (all candidates failed upstream rendering)".to_string()
        } else {
            "no fixtures were imported (use --diagram <name> and optionally --filter/--limit)"
                .to_string()
        }));
    }

    eprintln!("Imported {} fixtures:", created.len());
    for f in &created {
        eprintln!("  {}", f.path.display());
    }
    if !skipped.is_empty() {
        eprintln!("Skipped {} blocks:", skipped.len());
        for s in skipped.iter().take(50) {
            eprintln!("  {s}");
        }
        if skipped.len() > 50 {
            eprintln!("  ... ({} more)", skipped.len() - 50);
        }
    }

    Ok(())
}

fn import_upstream_html(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;
    let mut limit: Option<usize> = None;
    let mut min_lines: Option<usize> = None;
    let mut prefer_complex: bool = false;
    let mut overwrite: bool = false;
    let mut with_baselines: bool = false;
    let mut install: bool = false;
    let mut html_root: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--limit" => {
                i += 1;
                let raw = args.get(i).ok_or(XtaskError::Usage)?;
                limit = Some(raw.parse::<usize>().map_err(|_| XtaskError::Usage)?);
            }
            "--min-lines" => {
                i += 1;
                let raw = args.get(i).ok_or(XtaskError::Usage)?;
                min_lines = Some(raw.parse::<usize>().map_err(|_| XtaskError::Usage)?);
            }
            "--complex" => prefer_complex = true,
            "--overwrite" => overwrite = true,
            "--with-baselines" => with_baselines = true,
            "--install" => install = true,
            "--html-root" => {
                i += 1;
                let raw = args.get(i).ok_or(XtaskError::Usage)?;
                html_root = Some(PathBuf::from(raw));
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let html_root = html_root
        .map(|p| {
            if p.is_absolute() {
                p
            } else {
                workspace_root.join(p)
            }
        })
        .unwrap_or_else(|| {
            workspace_root
                .join("repo-ref")
                .join("mermaid")
                .join("demos")
        });
    if !html_root.exists() {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "upstream html root not found: {} (expected repo-ref checkout of mermaid@11.12.2)",
            html_root.display()
        )));
    }

    fn slugify(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut prev_us = false;
        for ch in s.chars() {
            let ch = ch.to_ascii_lowercase();
            if ch.is_ascii_alphanumeric() {
                out.push(ch);
                prev_us = false;
            } else if !prev_us {
                out.push('_');
                prev_us = true;
            }
        }
        while out.starts_with('_') {
            out.remove(0);
        }
        while out.ends_with('_') {
            out.pop();
        }
        if out.is_empty() {
            "untitled".to_string()
        } else {
            out
        }
    }

    fn clamp_slug(mut s: String, max_len: usize) -> String {
        if s.len() <= max_len {
            return s;
        }
        s.truncate(max_len);
        while s.ends_with('_') {
            s.pop();
        }
        if s.is_empty() {
            "untitled".to_string()
        } else {
            s
        }
    }

    fn canonical_fixture_text(s: &str) -> String {
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        let s = s.trim_matches('\n');
        format!("{s}\n")
    }

    fn html_unescape_basic(s: &str) -> String {
        let s = s.replace("&amp;", "&");
        let s = s.replace("&lt;", "<").replace("&gt;", ">");
        let s = s.replace("&quot;", "\"").replace("&#39;", "'");
        let s = s.replace("&nbsp;", " ");
        let s = s.replace("&#160;", " ").replace("&#xA0;", " ");
        s
    }

    fn dedent(s: &str) -> String {
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        let lines: Vec<&str> = s.lines().collect();
        let min_indent = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                l.as_bytes()
                    .iter()
                    .take_while(|b| **b == b' ' || **b == b'\t')
                    .count()
            })
            .min()
            .unwrap_or(0);
        let mut out = String::with_capacity(s.len());
        for (idx, line) in lines.iter().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            if line.len() >= min_indent {
                out.push_str(&line[min_indent..]);
            } else {
                out.push_str(line);
            }
        }
        out
    }

    fn normalize_yaml_frontmatter_indentation(s: &str) -> String {
        fn trim_front_ws(line: &str, n: usize) -> &str {
            let mut removed = 0usize;
            for (idx, ch) in line.char_indices() {
                if removed >= n {
                    return &line[idx..];
                }
                if ch == ' ' || ch == '\t' {
                    removed += 1;
                    continue;
                }
                return &line[idx..];
            }
            if removed >= n { "" } else { line }
        }

        let lines: Vec<&str> = s.lines().collect();
        let mut first_non_empty = 0usize;
        while first_non_empty < lines.len() && lines[first_non_empty].trim().is_empty() {
            first_non_empty += 1;
        }
        if first_non_empty >= lines.len() {
            return s.to_string();
        }
        if lines[first_non_empty].trim() != "---" {
            return s.to_string();
        }

        let mut close_idx: Option<usize> = None;
        for i in (first_non_empty + 1)..lines.len() {
            if lines[i].trim() == "---" {
                close_idx = Some(i);
                break;
            }
        }
        let Some(close_idx) = close_idx else {
            return s.to_string();
        };

        let mut min_indent = None::<usize>;
        for l in &lines[(first_non_empty + 1)..close_idx] {
            if l.trim().is_empty() {
                continue;
            }
            let indent = l
                .as_bytes()
                .iter()
                .take_while(|b| **b == b' ' || **b == b'\t')
                .count();
            min_indent = Some(min_indent.map(|m| m.min(indent)).unwrap_or(indent));
        }
        let min_indent = min_indent.unwrap_or(0);

        let mut out = String::with_capacity(s.len());
        for (idx, line) in lines.iter().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            if idx == first_non_empty || idx == close_idx {
                out.push_str("---");
                continue;
            }
            if idx > first_non_empty && idx < close_idx {
                out.push_str(trim_front_ws(line, min_indent));
                continue;
            }
            out.push_str(line);
        }
        out
    }

    fn normalize_html_mermaid_block(raw: &str) -> String {
        let s = dedent(&html_unescape_basic(raw));
        normalize_yaml_frontmatter_indentation(&s)
    }

    fn collect_html_files_recursively(
        root: &Path,
        out: &mut Vec<PathBuf>,
    ) -> Result<(), XtaskError> {
        if root.is_file() {
            if root.extension().is_some_and(|e| e == "html") {
                out.push(root.to_path_buf());
            }
            return Ok(());
        }
        let entries = fs::read_dir(root).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to list html directory {}: {err}",
                root.display()
            ))
        })?;
        for entry in entries {
            let path = entry
                .map_err(|err| {
                    XtaskError::SnapshotUpdateFailed(format!(
                        "failed to read html directory entry under {}: {err}",
                        root.display()
                    ))
                })?
                .path();
            if path.is_dir() {
                collect_html_files_recursively(&path, out)?;
            } else if path.extension().is_some_and(|e| e == "html") {
                out.push(path);
            }
        }
        Ok(())
    }

    fn normalize_diagram_dir(detected: &str) -> Option<String> {
        match detected {
            "flowchart" | "flowchart-v2" | "flowchart-elk" => Some("flowchart".to_string()),
            "state" | "stateDiagram" => Some("state".to_string()),
            "class" | "classDiagram" => Some("class".to_string()),
            "gitGraph" => Some("gitgraph".to_string()),
            "quadrantChart" => Some("quadrantchart".to_string()),
            "er" => Some("er".to_string()),
            "journey" => Some("journey".to_string()),
            "xychart" => Some("xychart".to_string()),
            "requirement" => Some("requirement".to_string()),
            "architecture-beta" => Some("architecture".to_string()),
            "architecture" | "block" | "c4" | "gantt" | "info" | "kanban" | "mindmap"
            | "packet" | "pie" | "radar" | "sankey" | "sequence" | "timeline" | "treemap" => {
                Some(detected.to_string())
            }
            _ => None,
        }
    }

    fn complexity_score(body: &str, diagram_dir: &str) -> i64 {
        let line_count = body.lines().count() as i64;
        let mut score = line_count * 1_000 + (body.len() as i64);
        let lower = body.to_ascii_lowercase();

        fn bump(score: &mut i64, lower: &str, needle: &str, weight: i64) {
            if lower.contains(needle) {
                *score += weight;
            }
        }

        bump(&mut score, &lower, "%%{init", 5_000);
        bump(&mut score, &lower, "accdescr", 2_000);
        bump(&mut score, &lower, "acctitle", 2_000);
        bump(&mut score, &lower, "classdef", 2_000);
        bump(&mut score, &lower, "direction", 1_000);
        bump(&mut score, &lower, "<br", 1_000);

        if diagram_dir == "state" {
            bump(&mut score, &lower, "note ", 2_000);
            bump(&mut score, &lower, "state ", 1_000);
            bump(&mut score, &lower, "{", 1_000);
        }

        score
    }

    #[derive(Debug, Clone)]
    struct HtmlBlock {
        source_html: PathBuf,
        source_stem: String,
        idx_in_file: usize,
        heading: Option<String>,
        body: String,
    }

    fn strip_tags(s: &str) -> String {
        static TAG_RE: OnceLock<Regex> = OnceLock::new();
        let re = TAG_RE.get_or_init(|| Regex::new(r"(?is)<[^>]+>").expect("valid regex"));
        re.replace_all(s, "").to_string()
    }

    fn extract_html_blocks(html_path: &Path) -> Result<Vec<HtmlBlock>, XtaskError> {
        let text = fs::read_to_string(html_path).map_err(|source| XtaskError::ReadFile {
            path: html_path.display().to_string(),
            source,
        })?;

        static PRE_RE: OnceLock<Regex> = OnceLock::new();
        static H_RE: OnceLock<Regex> = OnceLock::new();
        let pre_re = PRE_RE.get_or_init(|| {
            Regex::new(r"(?is)<pre\b(?P<attrs>[^>]*)>(?P<body>.*?)</pre\s*>").expect("valid regex")
        });
        let h_re = H_RE.get_or_init(|| {
            Regex::new(r"(?is)<h[1-6]\b[^>]*>(?P<body>.*?)</h[1-6]>").expect("valid regex")
        });

        let source_stem = html_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("html")
            .to_string();

        let mut headings: Vec<(usize, String)> = Vec::new();
        for cap in h_re.captures_iter(&text) {
            if let (Some(m), Some(b)) = (cap.get(0), cap.name("body")) {
                let clean = strip_tags(b.as_str());
                let clean = html_unescape_basic(clean.trim());
                if !clean.trim().is_empty() {
                    headings.push((m.start(), clean.trim().to_string()));
                }
            }
        }
        headings.sort_by_key(|(pos, _)| *pos);

        let mut out: Vec<HtmlBlock> = Vec::new();
        let mut idx_in_file = 0usize;
        for cap in pre_re.captures_iter(&text) {
            let m = cap.get(0).expect("match");
            let attrs = cap.name("attrs").map(|m| m.as_str()).unwrap_or_default();
            if !attrs.to_ascii_lowercase().contains("mermaid") {
                continue;
            }
            let raw_body = cap.name("body").map(|m| m.as_str()).unwrap_or_default();

            let mut heading: Option<String> = None;
            for (pos, h) in headings.iter().rev() {
                if *pos < m.start() {
                    heading = Some(h.clone());
                    break;
                }
            }

            out.push(HtmlBlock {
                source_html: html_path.to_path_buf(),
                source_stem: source_stem.clone(),
                idx_in_file,
                heading,
                body: raw_body.to_string(),
            });
            idx_in_file += 1;
        }

        Ok(out)
    }

    fn load_existing_fixtures(fixtures_dir: &Path) -> std::collections::HashMap<String, PathBuf> {
        let mut map = std::collections::HashMap::new();
        let Ok(entries) = fs::read_dir(fixtures_dir) else {
            return map;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "mmd") {
                if let Ok(text) = fs::read_to_string(&path) {
                    let canon = canonical_fixture_text(&text);
                    map.insert(canon, path);
                }
            }
        }
        map
    }

    #[derive(Debug, Clone)]
    struct Candidate {
        block: HtmlBlock,
        diagram_dir: String,
        fixtures_dir: PathBuf,
        stem: String,
        body: String,
        score: i64,
    }

    let reg = merman::detect::DetectorRegistry::default_mermaid_11_12_2_full();
    let mut html_files: Vec<PathBuf> = Vec::new();
    collect_html_files_recursively(&html_root, &mut html_files)?;
    html_files.sort();

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    let mut existing_by_diagram: std::collections::HashMap<
        String,
        std::collections::HashMap<String, PathBuf>,
    > = std::collections::HashMap::new();

    for html_path in html_files {
        if let Some(f) = filter.as_deref() {
            let hay = html_path.to_string_lossy();
            if !hay.contains(f) {
                // Still allow filtering by heading later; don't early-skip the file here.
            }
        }

        let blocks = extract_html_blocks(&html_path)?;
        for b in blocks {
            let body = canonical_fixture_text(&normalize_html_mermaid_block(&b.body));
            if body.trim().is_empty() {
                continue;
            }
            if let Some(min) = min_lines {
                if body.lines().count() < min {
                    continue;
                }
            }

            if let Some(f) = filter.as_deref() {
                let mut hay = html_path.to_string_lossy().to_string();
                if let Some(h) = b.heading.as_deref() {
                    hay.push(' ');
                    hay.push_str(h);
                }
                if !hay.contains(f) {
                    continue;
                }
            }

            let mut cfg = merman::MermaidConfig::default();
            let detected = match reg.detect_type(body.as_str(), &mut cfg) {
                Ok(t) => t,
                Err(_) => {
                    skipped.push(format!(
                        "skip (type not detected): {} (idx={})",
                        b.source_html.display(),
                        b.idx_in_file
                    ));
                    continue;
                }
            };
            let Some(diagram_dir) = normalize_diagram_dir(detected) else {
                skipped.push(format!(
                    "skip (unsupported detected type '{detected}'): {}",
                    b.source_html.display()
                ));
                continue;
            };

            if diagram_dir == "zenuml" {
                continue;
            }
            if diagram != "all" && diagram_dir != diagram {
                continue;
            }

            let fixtures_dir = workspace_root.join("fixtures").join(&diagram_dir);
            if !fixtures_dir.is_dir() {
                skipped.push(format!(
                    "skip (fixtures dir missing): {}",
                    fixtures_dir.display()
                ));
                continue;
            }

            let source_slug = clamp_slug(slugify(&format!("demos_{}", b.source_stem)), 48);
            let heading_slug = clamp_slug(slugify(b.heading.as_deref().unwrap_or("example")), 64);
            let stem = format!(
                "upstream_html_{source_slug}_{heading_slug}_{idx:03}",
                idx = b.idx_in_file + 1
            );

            let score = complexity_score(&body, &diagram_dir);
            candidates.push(Candidate {
                block: b,
                diagram_dir,
                fixtures_dir,
                stem,
                body,
                score,
            });
        }
    }

    if prefer_complex {
        candidates.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.stem.cmp(&b.stem)));
    }

    #[derive(Debug, Clone)]
    struct CreatedFixture {
        diagram_dir: String,
        stem: String,
        path: PathBuf,
    }

    let mut created: Vec<CreatedFixture> = Vec::new();
    let mut imported = 0usize;

    for c in candidates {
        let existing = existing_by_diagram
            .entry(c.diagram_dir.clone())
            .or_insert_with(|| load_existing_fixtures(&c.fixtures_dir));
        if let Some(existing_path) = existing.get(&c.body) {
            skipped.push(format!(
                "skip (duplicate content): {} -> {}",
                c.block.source_html.display(),
                existing_path.display()
            ));
            continue;
        }

        let out_path = c.fixtures_dir.join(format!("{}.mmd", c.stem));
        if out_path.exists() && !overwrite {
            skipped.push(format!("skip (already exists): {}", out_path.display()));
            continue;
        }
        let deferred_out_path = workspace_root
            .join("fixtures")
            .join("_deferred")
            .join(&c.diagram_dir)
            .join(format!("{}.mmd", c.stem));
        if deferred_out_path.exists() && !overwrite {
            skipped.push(format!(
                "skip (already deferred): {}",
                deferred_out_path.display()
            ));
            continue;
        }

        fs::write(&out_path, c.body.as_bytes()).map_err(|source| XtaskError::WriteFile {
            path: out_path.display().to_string(),
            source,
        })?;
        existing.insert(c.body.clone(), out_path.clone());

        created.push(CreatedFixture {
            diagram_dir: c.diagram_dir,
            stem: c.stem,
            path: out_path,
        });

        imported += 1;
        if let Some(max) = limit {
            if imported >= max {
                break;
            }
        }
    }

    if created.is_empty() {
        return Err(XtaskError::SnapshotUpdateFailed(
            "no fixtures were imported (use --diagram <name> and optionally --filter/--limit)"
                .to_string(),
        ));
    }

    if install && !with_baselines {
        return Err(XtaskError::SnapshotUpdateFailed(
            "`--install` only applies when `--with-baselines` is set".to_string(),
        ));
    }

    if with_baselines {
        fn is_suspicious_blank_svg(svg_path: &Path) -> bool {
            let Ok(head) = fs::read_to_string(svg_path) else {
                return false;
            };
            let first = head.lines().next().unwrap_or_default();
            first.contains(r#"viewBox="-8 -8 16 16""#)
                || first.contains(r#"viewBox="0 0 16 16""#)
                || first.contains(r#"style="max-width: 16px"#)
        }

        fn should_defer_fixture(diagram_dir: &str, fixture_text: &str) -> Option<&'static str> {
            match diagram_dir {
                "flowchart" => {
                    if fixture_text.contains("\n  layout: elk")
                        || fixture_text.contains("\nlayout: elk")
                    {
                        return Some("flowchart frontmatter config.layout=elk (deferred)");
                    }
                    if fixture_text
                        .lines()
                        .any(|l| l.trim_start().starts_with("flowchart-elk"))
                    {
                        return Some("flowchart diagram type flowchart-elk (deferred)");
                    }
                }
                "sequence" => {
                    if fixture_text.contains("$$") {
                        return Some(
                            "sequence math rendering uses <foreignObject> upstream (deferred)",
                        );
                    }
                }
                _ => {}
            }
            None
        }

        fn defer_fixture_files_keep_baselines(workspace_root: &Path, f: &CreatedFixture) {
            let deferred_dir = workspace_root
                .join("fixtures")
                .join("_deferred")
                .join(&f.diagram_dir);
            let deferred_svg_dir = workspace_root
                .join("fixtures")
                .join("_deferred")
                .join("upstream-svgs")
                .join(&f.diagram_dir);
            let _ = fs::create_dir_all(&deferred_dir);
            let _ = fs::create_dir_all(&deferred_svg_dir);

            let deferred_mmd_path = deferred_dir.join(format!("{}.mmd", f.stem));
            let _ = fs::remove_file(&deferred_mmd_path);
            let _ = fs::rename(&f.path, &deferred_mmd_path);

            let svg_path = workspace_root
                .join("fixtures")
                .join("upstream-svgs")
                .join(&f.diagram_dir)
                .join(format!("{}.svg", f.stem));
            if svg_path.exists() {
                let deferred_svg_path = deferred_svg_dir.join(format!("{}.svg", f.stem));
                let _ = fs::remove_file(&deferred_svg_path);
                let _ = fs::rename(&svg_path, &deferred_svg_path);
            }

            let _ = fs::remove_file(
                workspace_root
                    .join("fixtures")
                    .join(&f.diagram_dir)
                    .join(format!("{}.golden.json", f.stem)),
            );
            let _ = fs::remove_file(
                workspace_root
                    .join("fixtures")
                    .join(&f.diagram_dir)
                    .join(format!("{}.layout.golden.json", f.stem)),
            );
        }

        let mut kept: Vec<CreatedFixture> = Vec::with_capacity(created.len());
        for f in &created {
            let mut svg_args = vec![
                "--diagram".to_string(),
                f.diagram_dir.clone(),
                "--filter".to_string(),
                f.stem.clone(),
            ];
            if install {
                svg_args.push("--install".to_string());
            }
            if let Err(err) = gen_upstream_svgs(svg_args) {
                skipped.push(format!(
                    "defer (upstream svg generation failed): {} ({err})",
                    f.path.display()
                ));
                defer_fixture_files_keep_baselines(&workspace_root, f);
                continue;
            }

            let fixture_text = match fs::read_to_string(&f.path) {
                Ok(v) => v,
                Err(err) => {
                    skipped.push(format!(
                        "defer (failed to read fixture after import): {} ({err})",
                        f.path.display()
                    ));
                    defer_fixture_files_keep_baselines(&workspace_root, f);
                    continue;
                }
            };
            if let Some(reason) = should_defer_fixture(&f.diagram_dir, &fixture_text) {
                skipped.push(format!("defer ({reason}): {}", f.path.display()));
                defer_fixture_files_keep_baselines(&workspace_root, f);
                continue;
            }

            let svg_path = workspace_root
                .join("fixtures")
                .join("upstream-svgs")
                .join(&f.diagram_dir)
                .join(format!("{}.svg", f.stem));
            if is_suspicious_blank_svg(&svg_path) {
                skipped.push(format!(
                    "defer (suspicious upstream blank svg): {}",
                    f.path.display()
                ));
                defer_fixture_files_keep_baselines(&workspace_root, f);
                continue;
            }

            if let Err(err) = update_snapshots(vec![
                "--diagram".to_string(),
                f.diagram_dir.clone(),
                "--filter".to_string(),
                f.stem.clone(),
            ]) {
                skipped.push(format!(
                    "defer (snapshot update failed): {} ({err})",
                    f.path.display()
                ));
                defer_fixture_files_keep_baselines(&workspace_root, f);
                continue;
            }

            if let Err(err) = update_layout_snapshots(vec![
                "--diagram".to_string(),
                f.diagram_dir.clone(),
                "--filter".to_string(),
                f.stem.clone(),
            ]) {
                skipped.push(format!(
                    "defer (layout snapshot update failed): {} ({err})",
                    f.path.display()
                ));
                defer_fixture_files_keep_baselines(&workspace_root, f);
                continue;
            }

            kept.push(f.clone());
        }
        created = kept;
        if created.is_empty() {
            return Err(XtaskError::SnapshotUpdateFailed(
                "no fixtures were imported (all candidates were deferred due to baseline/snapshot failures)".to_string(),
            ));
        }
    }

    eprintln!("Imported {} fixtures:", created.len());
    for f in &created {
        eprintln!("  {}", f.path.display());
    }
    if !skipped.is_empty() {
        eprintln!("Skipped {} blocks:", skipped.len());
        for s in skipped.iter().take(50) {
            eprintln!("  {s}");
        }
        if skipped.len() > 50 {
            eprintln!("  ... ({} more)", skipped.len() - 50);
        }
    }

    Ok(())
}

fn import_upstream_cypress(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;
    let mut limit: Option<usize> = None;
    let mut min_lines: Option<usize> = None;
    let mut prefer_complex: bool = false;
    let mut overwrite: bool = false;
    let mut with_baselines: bool = false;
    let mut install: bool = false;
    let mut spec_root: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--limit" => {
                i += 1;
                let raw = args.get(i).ok_or(XtaskError::Usage)?;
                limit = Some(raw.parse::<usize>().map_err(|_| XtaskError::Usage)?);
            }
            "--min-lines" => {
                i += 1;
                let raw = args.get(i).ok_or(XtaskError::Usage)?;
                min_lines = Some(raw.parse::<usize>().map_err(|_| XtaskError::Usage)?);
            }
            "--complex" => prefer_complex = true,
            "--overwrite" => overwrite = true,
            "--with-baselines" => with_baselines = true,
            "--install" => install = true,
            "--spec-root" => {
                i += 1;
                let raw = args.get(i).ok_or(XtaskError::Usage)?;
                spec_root = Some(PathBuf::from(raw));
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let spec_root = spec_root
        .map(|p| {
            if p.is_absolute() {
                p
            } else {
                workspace_root.join(p)
            }
        })
        .unwrap_or_else(|| {
            workspace_root
                .join("repo-ref")
                .join("mermaid")
                .join("cypress")
                .join("integration")
                .join("rendering")
        });
    if !spec_root.exists() {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "upstream cypress spec root not found: {} (expected repo-ref checkout of mermaid@11.12.2)",
            spec_root.display()
        )));
    }

    fn slugify(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut prev_us = false;
        for ch in s.chars() {
            let ch = ch.to_ascii_lowercase();
            if ch.is_ascii_alphanumeric() {
                out.push(ch);
                prev_us = false;
            } else if !prev_us {
                out.push('_');
                prev_us = true;
            }
        }
        while out.starts_with('_') {
            out.remove(0);
        }
        while out.ends_with('_') {
            out.pop();
        }
        if out.is_empty() {
            "untitled".to_string()
        } else {
            out
        }
    }

    fn clamp_slug(mut s: String, max_len: usize) -> String {
        if s.len() <= max_len {
            return s;
        }
        s.truncate(max_len);
        while s.ends_with('_') {
            s.pop();
        }
        if s.is_empty() {
            "untitled".to_string()
        } else {
            s
        }
    }

    fn canonical_fixture_text(s: &str) -> String {
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        let s = s.trim_matches('\n');
        format!("{s}\n")
    }

    fn html_unescape_basic(s: &str) -> String {
        // Cypress rendering specs sometimes embed Mermaid code through HTML, so `<`/`>` sequences
        // can be entity-escaped in the source even though Mermaid receives the decoded text.
        //
        // Keep this intentionally minimal: only decode the entity forms we've observed in
        // upstream fixtures.
        let s = s.replace("&amp;", "&");
        let s = s.replace("&lt;", "<").replace("&gt;", ">");
        let s = s.replace("&quot;", "\"").replace("&#39;", "'");
        let s = s.replace("&nbsp;", " ");
        let s = s.replace("&#160;", " ").replace("&#xA0;", " ");
        s
    }

    fn dedent(s: &str) -> String {
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        let lines: Vec<&str> = s.lines().collect();
        let min_indent = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                l.as_bytes()
                    .iter()
                    .take_while(|b| **b == b' ' || **b == b'\t')
                    .count()
            })
            .min()
            .unwrap_or(0);
        let mut out = String::with_capacity(s.len());
        for (idx, line) in lines.iter().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            if line.len() >= min_indent {
                out.push_str(&line[min_indent..]);
            } else {
                out.push_str(line);
            }
        }
        out
    }

    fn normalize_yaml_frontmatter_indentation(s: &str) -> String {
        fn trim_front_ws(line: &str, n: usize) -> &str {
            let mut removed = 0usize;
            for (idx, ch) in line.char_indices() {
                if removed >= n {
                    return &line[idx..];
                }
                if ch == ' ' || ch == '\t' {
                    removed += 1;
                    continue;
                }
                return &line[idx..];
            }
            if removed >= n { "" } else { line }
        }

        let lines: Vec<&str> = s.lines().collect();
        let mut first_non_empty = 0usize;
        while first_non_empty < lines.len() && lines[first_non_empty].trim().is_empty() {
            first_non_empty += 1;
        }
        if first_non_empty >= lines.len() {
            return s.to_string();
        }
        if lines[first_non_empty].trim() != "---" {
            return s.to_string();
        }

        let mut close_idx: Option<usize> = None;
        for i in (first_non_empty + 1)..lines.len() {
            if lines[i].trim() == "---" {
                close_idx = Some(i);
                break;
            }
        }
        let Some(close_idx) = close_idx else {
            return s.to_string();
        };

        let mut min_indent = None::<usize>;
        for l in &lines[(first_non_empty + 1)..close_idx] {
            if l.trim().is_empty() {
                continue;
            }
            let indent = l
                .as_bytes()
                .iter()
                .take_while(|b| **b == b' ' || **b == b'\t')
                .count();
            min_indent = Some(min_indent.map(|m| m.min(indent)).unwrap_or(indent));
        }
        let min_indent = min_indent.unwrap_or(0);

        let mut out = String::with_capacity(s.len());
        for (idx, line) in lines.iter().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            if idx == first_non_empty || idx == close_idx {
                out.push_str("---");
                continue;
            }
            if idx > first_non_empty && idx < close_idx {
                out.push_str(trim_front_ws(line, min_indent));
                continue;
            }
            out.push_str(line);
        }
        out
    }

    fn normalize_cypress_fixture_text(raw: &str) -> String {
        let s = dedent(&html_unescape_basic(raw));
        normalize_yaml_frontmatter_indentation(&s)
    }

    fn normalize_architecture_beta_legacy_edges(s: &str) -> String {
        // Cypress architecture fixtures (`repo-ref/mermaid/cypress/integration/rendering/architecture.spec.ts`)
        // use a legacy shorthand that is not accepted by Mermaid@11.12.2 CLI (Langium grammar):
        //
        // - `a L--R b`
        // - `a (L--R) b`
        // - `a L-[Label]-R b`
        // - split parens across lines, e.g. `a (B--T b` / `a R--L) b`
        //
        // Normalize into CLI-compatible form:
        //
        // - `a:L -- R:b`
        // - `a:L -[Label]- R:b`
        static EDGE_DIR_RE: OnceLock<Regex> = OnceLock::new();
        static EDGE_LABEL_RE: OnceLock<Regex> = OnceLock::new();
        let edge_dir_re = EDGE_DIR_RE.get_or_init(|| {
            Regex::new(
                r"^(?P<indent>\s*)(?P<src>\S+)\s+\(?(?P<d1>[LTRB])--(?P<d2>[LTRB])\)?\s+(?P<dst>\S+)\s*$",
            )
            .expect("valid regex")
        });
        let edge_label_re = EDGE_LABEL_RE.get_or_init(|| {
            Regex::new(
                r"^(?P<indent>\s*)(?P<src>\S+)\s+(?P<d1>[LTRB])-\[(?P<label>[^\]]*)\]-(?P<d2>[LTRB])\s+(?P<dst>\S+)\s*$",
            )
            .expect("valid regex")
        });

        let mut out = String::with_capacity(s.len());
        for (idx, raw_line) in s.lines().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            let line = raw_line.trim_end_matches(|c| c == ' ' || c == '\t');

            if let Some(caps) = edge_label_re.captures(line) {
                let indent = caps.name("indent").map(|m| m.as_str()).unwrap_or_default();
                let src = caps.name("src").map(|m| m.as_str()).unwrap_or_default();
                let d1 = caps.name("d1").map(|m| m.as_str()).unwrap_or_default();
                let label = caps.name("label").map(|m| m.as_str()).unwrap_or_default();
                let d2 = caps.name("d2").map(|m| m.as_str()).unwrap_or_default();
                let dst = caps.name("dst").map(|m| m.as_str()).unwrap_or_default();

                out.push_str(indent);
                out.push_str(src);
                out.push(':');
                out.push_str(d1);
                out.push_str(" -[");
                out.push_str(label);
                out.push_str("]- ");
                out.push_str(d2);
                out.push(':');
                out.push_str(dst);
                continue;
            }

            if let Some(caps) = edge_dir_re.captures(line) {
                let indent = caps.name("indent").map(|m| m.as_str()).unwrap_or_default();
                let src = caps.name("src").map(|m| m.as_str()).unwrap_or_default();
                let d1 = caps.name("d1").map(|m| m.as_str()).unwrap_or_default();
                let d2 = caps.name("d2").map(|m| m.as_str()).unwrap_or_default();
                let dst = caps.name("dst").map(|m| m.as_str()).unwrap_or_default();

                out.push_str(indent);
                out.push_str(src);
                out.push(':');
                out.push_str(d1);
                out.push_str(" -- ");
                out.push_str(d2);
                out.push(':');
                out.push_str(dst);
                continue;
            }

            out.push_str(line);
        }

        out
    }

    fn collect_spec_files_recursively(
        root: &Path,
        out: &mut Vec<PathBuf>,
    ) -> Result<(), XtaskError> {
        if root.is_file() {
            if root.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                (n.ends_with(".spec.js") || n.ends_with(".spec.ts")) && !n.contains("node_modules")
            }) {
                out.push(root.to_path_buf());
            }
            return Ok(());
        }
        let entries = fs::read_dir(root).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to list cypress directory {}: {err}",
                root.display()
            ))
        })?;
        for entry in entries {
            let path = entry
                .map_err(|err| {
                    XtaskError::SnapshotUpdateFailed(format!(
                        "failed to read cypress directory entry under {}: {err}",
                        root.display()
                    ))
                })?
                .path();
            if path.is_dir() {
                collect_spec_files_recursively(&path, out)?;
            } else if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                (n.ends_with(".spec.js") || n.ends_with(".spec.ts")) && !n.contains("node_modules")
            }) {
                out.push(path);
            }
        }
        Ok(())
    }

    fn extract_first_template_literal(s: &str, start: usize) -> Option<(String, usize)> {
        let bytes = s.as_bytes();
        let mut i = start;
        while i < bytes.len() && bytes[i] != b'`' {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        // i points at opening backtick
        i += 1;
        let mut out = String::new();
        let mut escaped = false;
        while i < bytes.len() {
            let b = bytes[i];
            if escaped {
                match b {
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'\\' => out.push('\\'),
                    b'`' => out.push('`'),
                    _ => out.push(b as char),
                }
                escaped = false;
                i += 1;
                continue;
            }
            if b == b'\\' {
                escaped = true;
                i += 1;
                continue;
            }
            if b == b'`' {
                return Some((out, i + 1));
            }
            // Reject template interpolation blocks; those aren't static Mermaid fixtures.
            if b == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                return None;
            }
            out.push(b as char);
            i += 1;
        }
        None
    }

    fn normalize_diagram_dir(detected: &str) -> Option<String> {
        match detected {
            "flowchart" | "flowchart-v2" | "flowchart-elk" => Some("flowchart".to_string()),
            "state" | "stateDiagram" => Some("state".to_string()),
            "class" | "classDiagram" => Some("class".to_string()),
            "gitGraph" => Some("gitgraph".to_string()),
            "quadrantChart" => Some("quadrantchart".to_string()),
            "er" => Some("er".to_string()),
            "journey" => Some("journey".to_string()),
            "xychart" => Some("xychart".to_string()),
            "requirement" => Some("requirement".to_string()),
            "architecture-beta" => Some("architecture".to_string()),
            "architecture" | "block" | "c4" | "gantt" | "info" | "kanban" | "mindmap"
            | "packet" | "pie" | "radar" | "sankey" | "sequence" | "timeline" | "treemap" => {
                Some(detected.to_string())
            }
            _ => None,
        }
    }

    fn complexity_score(body: &str, diagram_dir: &str) -> i64 {
        let line_count = body.lines().count() as i64;
        let mut score = line_count * 1_000 + (body.len() as i64);
        let lower = body.to_ascii_lowercase();

        fn bump(score: &mut i64, lower: &str, needle: &str, weight: i64) {
            if lower.contains(needle) {
                *score += weight;
            }
        }

        bump(&mut score, &lower, "%%{init", 5_000);
        bump(&mut score, &lower, "accdescr", 2_000);
        bump(&mut score, &lower, "acctitle", 2_000);
        bump(&mut score, &lower, "linkstyle", 2_000);
        bump(&mut score, &lower, "classdef", 2_000);
        bump(&mut score, &lower, "direction", 1_000);
        bump(&mut score, &lower, "click ", 1_500);
        bump(&mut score, &lower, "<img", 1_000);
        bump(&mut score, &lower, "<strong>", 1_000);
        bump(&mut score, &lower, "<em>", 1_000);

        match diagram_dir {
            "flowchart" => {
                bump(&mut score, &lower, "subgraph", 2_000);
                bump(&mut score, &lower, ":::", 1_000);
                bump(&mut score, &lower, "@{", 1_500);
            }
            "sequence" => {
                bump(&mut score, &lower, "alt", 1_500);
                bump(&mut score, &lower, "loop", 1_500);
                bump(&mut score, &lower, "par", 1_500);
                bump(&mut score, &lower, "opt", 1_000);
                bump(&mut score, &lower, "critical", 1_500);
                bump(&mut score, &lower, "rect", 1_000);
                bump(&mut score, &lower, "activate", 1_000);
                bump(&mut score, &lower, "deactivate", 1_000);
            }
            "class" => {
                bump(&mut score, &lower, "namespace", 1_000);
                bump(&mut score, &lower, "interface", 1_000);
                bump(&mut score, &lower, "enum", 1_000);
                bump(&mut score, &lower, "<<", 1_000);
            }
            "state" => {
                bump(&mut score, &lower, "fork", 1_000);
                bump(&mut score, &lower, "join", 1_000);
                bump(&mut score, &lower, "[*]", 1_000);
                bump(&mut score, &lower, "note", 1_000);
            }
            _ => {}
        }

        score
    }

    fn load_existing_fixtures(fixtures_dir: &Path) -> std::collections::HashMap<String, PathBuf> {
        let mut map = std::collections::HashMap::new();
        let Ok(entries) = fs::read_dir(fixtures_dir) else {
            return map;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "mmd") {
                if let Ok(text) = fs::read_to_string(&path) {
                    let canon = canonical_fixture_text(&text);
                    map.insert(canon, path);
                }
            }
        }
        map
    }

    #[derive(Debug, Clone)]
    struct CypressBlock {
        source_spec: PathBuf,
        source_stem: String,
        idx_in_file: usize,
        test_name: Option<String>,
        call: String,
        body: String,
    }

    fn extract_cypress_blocks(spec_path: &Path) -> Result<Vec<CypressBlock>, XtaskError> {
        let text = fs::read_to_string(spec_path).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to read cypress spec file {}: {err}",
                spec_path.display()
            ))
        })?;

        fn find_matching_paren_close(text: &str, open_paren: usize) -> Option<usize> {
            // Best-effort JS scanning to find the matching `)` for a call starting at `open_paren`.
            //
            // This intentionally ignores nested template literal `${...}` parsing; for our fixture
            // sources this is sufficient and prevents accidentally capturing backticks from later
            // tests when the call argument is not a template literal (e.g. `imgSnapshotTest(diagramCode, ...)`).
            let bytes = text.as_bytes();
            if bytes.get(open_paren) != Some(&b'(') {
                return None;
            }

            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            enum Mode {
                Normal,
                SingleQuote,
                DoubleQuote,
                Template,
                LineComment,
                BlockComment,
            }

            let mut mode = Mode::Normal;
            let mut depth: i32 = 1;
            let mut escaped = false;

            let mut i = open_paren + 1;
            while i < bytes.len() {
                let b = bytes[i];
                match mode {
                    Mode::Normal => {
                        if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
                            mode = Mode::LineComment;
                            i += 2;
                            continue;
                        }
                        if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
                            mode = Mode::BlockComment;
                            i += 2;
                            continue;
                        }
                        if b == b'\'' {
                            mode = Mode::SingleQuote;
                            escaped = false;
                            i += 1;
                            continue;
                        }
                        if b == b'"' {
                            mode = Mode::DoubleQuote;
                            escaped = false;
                            i += 1;
                            continue;
                        }
                        if b == b'`' {
                            mode = Mode::Template;
                            escaped = false;
                            i += 1;
                            continue;
                        }

                        if b == b'(' {
                            depth += 1;
                        } else if b == b')' {
                            depth -= 1;
                            if depth == 0 {
                                return Some(i);
                            }
                        }

                        i += 1;
                    }
                    Mode::SingleQuote => {
                        if escaped {
                            escaped = false;
                            i += 1;
                            continue;
                        }
                        if b == b'\\' {
                            escaped = true;
                            i += 1;
                            continue;
                        }
                        if b == b'\'' {
                            mode = Mode::Normal;
                        }
                        i += 1;
                    }
                    Mode::DoubleQuote => {
                        if escaped {
                            escaped = false;
                            i += 1;
                            continue;
                        }
                        if b == b'\\' {
                            escaped = true;
                            i += 1;
                            continue;
                        }
                        if b == b'"' {
                            mode = Mode::Normal;
                        }
                        i += 1;
                    }
                    Mode::Template => {
                        if escaped {
                            escaped = false;
                            i += 1;
                            continue;
                        }
                        if b == b'\\' {
                            escaped = true;
                            i += 1;
                            continue;
                        }
                        if b == b'`' {
                            mode = Mode::Normal;
                        }
                        i += 1;
                    }
                    Mode::LineComment => {
                        if b == b'\n' {
                            mode = Mode::Normal;
                        }
                        i += 1;
                    }
                    Mode::BlockComment => {
                        if b == b'*' && bytes.get(i + 1) == Some(&b'/') {
                            mode = Mode::Normal;
                            i += 2;
                            continue;
                        }
                        i += 1;
                    }
                }
            }
            None
        }

        let source_stem = spec_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // `regex` crate does not support backreferences; capture single-quoted and double-quoted
        // variants separately.
        let re_it_sq = Regex::new(r#"(?m)\bit\s*\(\s*'([^']*)'"#).map_err(|e| {
            XtaskError::SnapshotUpdateFailed(format!("invalid it() single-quote regex: {e}"))
        })?;
        let re_it_dq = Regex::new(r#"(?m)\bit\s*\(\s*"([^"]*)""#).map_err(|e| {
            XtaskError::SnapshotUpdateFailed(format!("invalid it() double-quote regex: {e}"))
        })?;
        let mut test_name: Option<String> = None;
        let mut it_positions: Vec<(usize, String)> = Vec::new();
        for cap in re_it_sq.captures_iter(&text) {
            if let (Some(m), Some(t)) = (cap.get(0), cap.get(1)) {
                it_positions.push((m.start(), t.as_str().to_string()));
            }
        }
        for cap in re_it_dq.captures_iter(&text) {
            if let (Some(m), Some(t)) = (cap.get(0), cap.get(1)) {
                it_positions.push((m.start(), t.as_str().to_string()));
            }
        }
        it_positions.sort_by_key(|(pos, _)| *pos);
        let mut next_it_idx = 0usize;

        let mut out: Vec<CypressBlock> = Vec::new();
        let mut idx_in_file = 0usize;
        for (call, needle) in [
            ("imgSnapshotTest", "imgSnapshotTest"),
            ("renderGraph", "renderGraph"),
        ] {
            let mut search_from = 0usize;
            while let Some(found) = text[search_from..].find(needle) {
                let abs = search_from + found;
                while next_it_idx + 1 < it_positions.len() && it_positions[next_it_idx + 1].0 < abs
                {
                    next_it_idx += 1;
                }
                if let Some((it_pos, name)) = it_positions.get(next_it_idx) {
                    if *it_pos < abs {
                        test_name = Some(name.clone());
                    }
                }

                // Find the opening paren and extract the first template literal after it.
                let after_call = abs + needle.len();
                let Some(open_paren) = text[after_call..].find('(').map(|o| after_call + o) else {
                    search_from = after_call;
                    continue;
                };
                let start = open_paren + 1;

                let Some(close_paren) = find_matching_paren_close(&text, open_paren) else {
                    search_from = start;
                    continue;
                };

                // Only scan within the call arguments; otherwise we can accidentally capture a
                // backtick string from a later `it()` block when the call argument itself isn't
                // a template literal.
                let args_slice = &text[start..close_paren];
                if let Some((raw, end_rel)) = extract_first_template_literal(args_slice, 0) {
                    out.push(CypressBlock {
                        source_spec: spec_path.to_path_buf(),
                        source_stem: source_stem.clone(),
                        idx_in_file,
                        test_name: test_name.clone(),
                        call: call.to_string(),
                        body: raw,
                    });
                    idx_in_file += 1;
                    search_from = start + end_rel;
                    continue;
                }

                search_from = close_paren + 1;
            }
        }

        Ok(out)
    }

    #[derive(Debug, Clone)]
    struct Candidate {
        block: CypressBlock,
        diagram_dir: String,
        fixtures_dir: PathBuf,
        stem: String,
        body: String,
        score: i64,
    }

    let reg = merman::detect::DetectorRegistry::default_mermaid_11_12_2_full();
    let mut spec_files: Vec<PathBuf> = Vec::new();
    collect_spec_files_recursively(&spec_root, &mut spec_files)?;
    spec_files.sort();

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    let mut existing_by_diagram: std::collections::HashMap<
        String,
        std::collections::HashMap<String, PathBuf>,
    > = std::collections::HashMap::new();

    for spec_path in spec_files {
        if let Some(f) = filter.as_deref() {
            let hay = spec_path.to_string_lossy();
            if !hay.contains(f) {
                // Still allow filtering by test name later; don't early-skip the file here.
            }
        }

        let blocks = extract_cypress_blocks(&spec_path)?;
        for b in blocks {
            let mut body = canonical_fixture_text(&normalize_cypress_fixture_text(&b.body));
            if body.trim().is_empty() {
                continue;
            }
            if let Some(min) = min_lines {
                if body.lines().count() < min {
                    continue;
                }
            }

            if let Some(f) = filter.as_deref() {
                let mut hay = spec_path.to_string_lossy().to_string();
                if let Some(t) = b.test_name.as_deref() {
                    hay.push(' ');
                    hay.push_str(t);
                }
                if !hay.contains(f) {
                    continue;
                }
            }

            let mut cfg = merman::MermaidConfig::default();
            let detected = match reg.detect_type(body.as_str(), &mut cfg) {
                Ok(t) => t,
                Err(_) => {
                    skipped.push(format!(
                        "skip (type not detected): {} (call={}, idx={})",
                        b.source_spec.display(),
                        b.call,
                        b.idx_in_file
                    ));
                    continue;
                }
            };
            let Some(diagram_dir) = normalize_diagram_dir(detected) else {
                skipped.push(format!(
                    "skip (unsupported detected type '{detected}'): {}",
                    b.source_spec.display()
                ));
                continue;
            };

            if diagram_dir == "zenuml" {
                continue;
            }
            if diagram != "all" && diagram_dir != diagram {
                continue;
            }

            if diagram_dir == "architecture" {
                body = canonical_fixture_text(&normalize_architecture_beta_legacy_edges(&body));
            }

            let fixtures_dir = workspace_root.join("fixtures").join(&diagram_dir);
            if !fixtures_dir.is_dir() {
                skipped.push(format!(
                    "skip (fixtures dir missing): {}",
                    fixtures_dir.display()
                ));
                continue;
            }

            let source_slug = clamp_slug(slugify(&b.source_stem), 48);
            let test_slug = clamp_slug(slugify(b.test_name.as_deref().unwrap_or("example")), 64);
            let stem = format!(
                "upstream_cypress_{source_slug}_{test_slug}_{idx:03}",
                idx = b.idx_in_file + 1
            );

            let score = complexity_score(&body, &diagram_dir);
            candidates.push(Candidate {
                block: b,
                diagram_dir,
                fixtures_dir,
                stem,
                body,
                score,
            });
        }
    }

    if prefer_complex {
        candidates.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.stem.cmp(&b.stem)));
    }

    // Create `.mmd` fixtures (deduped by canonical body text).
    #[derive(Debug, Clone)]
    struct CreatedFixture {
        diagram_dir: String,
        stem: String,
        path: PathBuf,
        source_spec: PathBuf,
        source_idx_in_file: usize,
        source_call: String,
        source_test_name: Option<String>,
    }

    let mut created: Vec<CreatedFixture> = Vec::new();
    let mut imported = 0usize;

    for c in candidates {
        let existing = existing_by_diagram
            .entry(c.diagram_dir.clone())
            .or_insert_with(|| load_existing_fixtures(&c.fixtures_dir));
        if let Some(existing_path) = existing.get(&c.body) {
            skipped.push(format!(
                "skip (duplicate content): {} -> {}",
                c.block.source_spec.display(),
                existing_path.display()
            ));
            continue;
        }

        let out_path = c.fixtures_dir.join(format!("{}.mmd", c.stem));
        if out_path.exists() && !overwrite {
            skipped.push(format!("skip (already exists): {}", out_path.display()));
            continue;
        }
        let deferred_out_path = workspace_root
            .join("fixtures")
            .join("_deferred")
            .join(&c.diagram_dir)
            .join(format!("{}.mmd", c.stem));
        if deferred_out_path.exists() && !overwrite {
            skipped.push(format!(
                "skip (already deferred): {}",
                deferred_out_path.display()
            ));
            continue;
        }

        fs::write(&out_path, c.body.as_bytes()).map_err(|source| XtaskError::WriteFile {
            path: out_path.display().to_string(),
            source,
        })?;
        existing.insert(c.body.clone(), out_path.clone());

        created.push(CreatedFixture {
            diagram_dir: c.diagram_dir,
            stem: c.stem,
            path: out_path,
            source_spec: c.block.source_spec,
            source_idx_in_file: c.block.idx_in_file,
            source_call: c.block.call,
            source_test_name: c.block.test_name,
        });

        imported += 1;
        if let Some(max) = limit {
            if imported >= max {
                break;
            }
        }
    }

    if created.is_empty() {
        return Err(XtaskError::SnapshotUpdateFailed(
            "no fixtures were imported (use --diagram <name> and optionally --filter/--limit)"
                .to_string(),
        ));
    }

    if install && !with_baselines {
        return Err(XtaskError::SnapshotUpdateFailed(
            "`--install` only applies when `--with-baselines` is set".to_string(),
        ));
    }

    if with_baselines {
        let report_path = workspace_root
            .join("target")
            .join("import-upstream-cypress.report.txt");
        let mut report_lines: Vec<String> = Vec::new();

        fn deferred_with_baselines_reason(
            diagram_dir: &str,
            fixture_text: &str,
        ) -> Option<&'static str> {
            match diagram_dir {
                "flowchart" => {
                    if fixture_text.contains("\n  look:") || fixture_text.contains("\nlook:") {
                        if !fixture_text.contains("\n  look: classic")
                            && !fixture_text.contains("\nlook: classic")
                        {
                            return Some("flowchart frontmatter config.look!=classic (deferred)");
                        }
                    }
                    if fixture_text.contains("$$") {
                        return Some("flowchart math (deferred)");
                    }
                }
                "sequence" => {
                    if fixture_text.contains("$$") {
                        return Some("sequence math (deferred)");
                    }
                    if fixture_text.contains("%%{init:")
                        && (fixture_text.contains("\"wrap\": true")
                            || fixture_text.contains("\"width\""))
                    {
                        return Some("sequence wrap/width directive (deferred)");
                    }
                }
                _ => {}
            }
            None
        }

        fn deferred_keep_baselines_reason(
            diagram_dir: &str,
            fixture_text: &str,
        ) -> Option<&'static str> {
            match diagram_dir {
                "flowchart" => {
                    // ELK layout is currently out of scope for the headless layout engine, but we
                    // still keep the upstream SVG baseline so the case remains traceable.
                    if fixture_text.contains("\n  layout: elk")
                        || fixture_text.contains("\nlayout: elk")
                    {
                        return Some("flowchart frontmatter config.layout=elk (deferred)");
                    }

                    // Mermaid also has a dedicated `flowchart-elk` diagram type.
                    // Keep these fixtures in `_deferred` until we implement ELK layout parity.
                    if fixture_text
                        .lines()
                        .any(|l| l.trim_start().starts_with("flowchart-elk"))
                    {
                        return Some("flowchart diagram type flowchart-elk (deferred)");
                    }
                }
                _ => {}
            }
            None
        }

        fn is_suspicious_blank_svg(svg_path: &Path) -> bool {
            let Ok(head) = fs::read_to_string(svg_path) else {
                return false;
            };
            let first = head.lines().next().unwrap_or_default();
            first.contains(r#"viewBox="-8 -8 16 16""#)
                || first.contains(r#"viewBox="0 0 16 16""#)
                || first.contains(r#"style="max-width: 16px"#)
        }

        fn cleanup_fixture_files(workspace_root: &Path, f: &CreatedFixture) {
            let _ = fs::remove_file(&f.path);
            let _ = fs::remove_file(
                workspace_root
                    .join("fixtures")
                    .join("upstream-svgs")
                    .join(&f.diagram_dir)
                    .join(format!("{}.svg", f.stem)),
            );
            let _ = fs::remove_file(
                workspace_root
                    .join("fixtures")
                    .join(&f.diagram_dir)
                    .join(format!("{}.golden.json", f.stem)),
            );
            let _ = fs::remove_file(
                workspace_root
                    .join("fixtures")
                    .join(&f.diagram_dir)
                    .join(format!("{}.layout.golden.json", f.stem)),
            );
        }

        fn defer_fixture_files_keep_baselines(workspace_root: &Path, f: &CreatedFixture) {
            let deferred_dir = workspace_root
                .join("fixtures")
                .join("_deferred")
                .join(&f.diagram_dir);
            let deferred_svg_dir = workspace_root
                .join("fixtures")
                .join("_deferred")
                .join("upstream-svgs")
                .join(&f.diagram_dir);
            let _ = fs::create_dir_all(&deferred_dir);
            let _ = fs::create_dir_all(&deferred_svg_dir);

            let deferred_mmd_path = deferred_dir.join(format!("{}.mmd", f.stem));
            let _ = fs::remove_file(&deferred_mmd_path);
            let _ = fs::rename(&f.path, &deferred_mmd_path);

            let svg_path = workspace_root
                .join("fixtures")
                .join("upstream-svgs")
                .join(&f.diagram_dir)
                .join(format!("{}.svg", f.stem));
            let deferred_svg_path = deferred_svg_dir.join(format!("{}.svg", f.stem));
            let _ = fs::remove_file(&deferred_svg_path);
            let _ = fs::rename(&svg_path, &deferred_svg_path);

            // We do not keep snapshots for deferred fixtures in the main fixture corpus.
            let _ = fs::remove_file(
                workspace_root
                    .join("fixtures")
                    .join(&f.diagram_dir)
                    .join(format!("{}.golden.json", f.stem)),
            );
            let _ = fs::remove_file(
                workspace_root
                    .join("fixtures")
                    .join(&f.diagram_dir)
                    .join(format!("{}.layout.golden.json", f.stem)),
            );
        }

        let mut kept: Vec<CreatedFixture> = Vec::with_capacity(created.len());
        for f in &created {
            let fixture_text = match fs::read_to_string(&f.path) {
                Ok(v) => v,
                Err(err) => {
                    report_lines.push(format!(
                        "READ_FIXTURE_FAILED\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\terr={err}",
                        f.diagram_dir,
                        f.stem,
                        f.source_spec.display(),
                        f.source_idx_in_file,
                        f.source_call,
                        f.source_test_name.clone().unwrap_or_default(),
                    ));
                    skipped.push(format!(
                        "skip (failed to read imported fixture): {} ({err})",
                        f.path.display(),
                    ));
                    cleanup_fixture_files(&workspace_root, f);
                    continue;
                }
            };
            if let Some(reason) = deferred_with_baselines_reason(&f.diagram_dir, &fixture_text) {
                report_lines.push(format!(
                    "DEFERRED_WITH_BASELINES\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\treason={reason}",
                    f.diagram_dir,
                    f.stem,
                    f.source_spec.display(),
                    f.source_idx_in_file,
                    f.source_call,
                    f.source_test_name.clone().unwrap_or_default(),
                ));
                skipped.push(format!(
                    "skip (deferred for --with-baselines): {} ({reason})",
                    f.path.display(),
                ));
                cleanup_fixture_files(&workspace_root, f);
                continue;
            }

            let mut svg_args = vec![
                "--diagram".to_string(),
                f.diagram_dir.clone(),
                "--filter".to_string(),
                f.stem.clone(),
            ];
            if install {
                svg_args.push("--install".to_string());
            }
            match gen_upstream_svgs(svg_args) {
                Ok(()) => {}
                Err(XtaskError::UpstreamSvgFailed(msg)) => {
                    report_lines.push(format!(
                        "UPSTREAM_SVG_FAILED\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\tmsg={}",
                        f.diagram_dir,
                        f.stem,
                        f.source_spec.display(),
                        f.source_idx_in_file,
                        f.source_call,
                        f.source_test_name.clone().unwrap_or_default(),
                        msg.lines().next().unwrap_or("unknown upstream error"),
                    ));
                    skipped.push(format!(
                        "skip (upstream svg failed): {} ({})",
                        f.path.display(),
                        msg.lines().next().unwrap_or("unknown upstream error")
                    ));
                    cleanup_fixture_files(&workspace_root, f);
                    continue;
                }
                Err(other) => return Err(other),
            }

            let svg_path = workspace_root
                .join("fixtures")
                .join("upstream-svgs")
                .join(&f.diagram_dir)
                .join(format!("{}.svg", f.stem));
            if is_suspicious_blank_svg(&svg_path) {
                report_lines.push(format!(
                    "UPSTREAM_SVG_SUSPICIOUS_BLANK\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}",
                    f.diagram_dir,
                    f.stem,
                    f.source_spec.display(),
                    f.source_idx_in_file,
                    f.source_call,
                    f.source_test_name.clone().unwrap_or_default(),
                ));
                skipped.push(format!(
                    "skip (suspicious upstream svg output): {} (blank 16x16-like svg)",
                    f.path.display(),
                ));
                cleanup_fixture_files(&workspace_root, f);
                continue;
            }

            if let Some(reason) = deferred_keep_baselines_reason(&f.diagram_dir, &fixture_text) {
                report_lines.push(format!(
                    "DEFERRED_WITH_BASELINES\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\treason={reason}",
                    f.diagram_dir,
                    f.stem,
                    f.source_spec.display(),
                    f.source_idx_in_file,
                    f.source_call,
                    f.source_test_name.clone().unwrap_or_default(),
                ));
                skipped.push(format!(
                    "skip (deferred for --with-baselines): {} ({reason})",
                    f.path.display(),
                ));
                defer_fixture_files_keep_baselines(&workspace_root, f);
                continue;
            }

            if let Err(err) = update_snapshots(vec![
                "--diagram".to_string(),
                f.diagram_dir.clone(),
                "--filter".to_string(),
                f.stem.clone(),
            ]) {
                report_lines.push(format!(
                    "SNAPSHOT_UPDATE_FAILED\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\terr={err}",
                    f.diagram_dir,
                    f.stem,
                    f.source_spec.display(),
                    f.source_idx_in_file,
                    f.source_call,
                    f.source_test_name.clone().unwrap_or_default(),
                ));
                skipped.push(format!(
                    "skip (snapshot update failed): {} ({err})",
                    f.path.display(),
                ));
                cleanup_fixture_files(&workspace_root, f);
                continue;
            }
            if let Err(err) = update_layout_snapshots(vec![
                "--diagram".to_string(),
                f.diagram_dir.clone(),
                "--filter".to_string(),
                f.stem.clone(),
            ]) {
                report_lines.push(format!(
                    "LAYOUT_SNAPSHOT_UPDATE_FAILED\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\terr={err}",
                    f.diagram_dir,
                    f.stem,
                    f.source_spec.display(),
                    f.source_idx_in_file,
                    f.source_call,
                    f.source_test_name.clone().unwrap_or_default(),
                ));
                skipped.push(format!(
                    "skip (layout snapshot update failed): {} ({err})",
                    f.path.display(),
                ));
                cleanup_fixture_files(&workspace_root, f);
                continue;
            }

            kept.push(f.clone());
        }
        created = kept;

        if !report_lines.is_empty() {
            if let Some(parent) = report_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let header = format!(
                "# import-upstream-cypress report (Mermaid@11.12.2)\n# generated_at={}\n",
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%z")
            );
            let mut out = String::new();
            out.push_str(&header);
            out.push_str(&report_lines.join("\n"));
            out.push('\n');
            let _ = fs::write(&report_path, out);
            eprintln!("Wrote import report: {}", report_path.display());
        }

        if created.is_empty() {
            return Err(XtaskError::SnapshotUpdateFailed(
                "no fixtures were imported (all candidates failed upstream rendering)".to_string(),
            ));
        }
    }

    eprintln!("Imported {} fixtures:", created.len());
    for f in &created {
        eprintln!("  {}", f.path.display());
    }
    if !skipped.is_empty() {
        eprintln!("Skipped {} candidates:", skipped.len());
        for s in skipped.iter().take(50) {
            eprintln!("  {s}");
        }
        if skipped.len() > 50 {
            eprintln!("  ... ({} more)", skipped.len() - 50);
        }
    }

    Ok(())
}

fn import_mmdr_fixtures(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;
    let mut limit: Option<usize> = None;
    let mut prefer_complex: bool = false;
    let mut overwrite: bool = false;
    let mut with_baselines: bool = false;
    let mut install: bool = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--limit" => {
                i += 1;
                let raw = args.get(i).ok_or(XtaskError::Usage)?;
                limit = Some(raw.parse::<usize>().map_err(|_| XtaskError::Usage)?);
            }
            "--complex" => prefer_complex = true,
            "--overwrite" => overwrite = true,
            "--with-baselines" => with_baselines = true,
            "--install" => install = true,
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let mmdr_root = workspace_root
        .join("repo-ref")
        .join("mermaid-rs-renderer")
        .join("tests")
        .join("fixtures");
    if !mmdr_root.is_dir() {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "mmdr fixtures folder not found: {} (expected repo-ref checkout of mermaid-rs-renderer)",
            mmdr_root.display()
        )));
    }

    fn canonical_fixture_text(s: &str) -> String {
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        let s = s.trim_matches('\n');
        format!("{s}\n")
    }

    fn sanitize_stem(raw: &str) -> String {
        let mut out = String::with_capacity(raw.len());
        let mut prev_us = false;
        for ch in raw.chars() {
            let ch = ch.to_ascii_lowercase();
            if ch.is_ascii_alphanumeric() {
                out.push(ch);
                prev_us = false;
            } else if !prev_us {
                out.push('_');
                prev_us = true;
            }
        }
        while out.starts_with('_') {
            out.remove(0);
        }
        while out.ends_with('_') {
            out.pop();
        }
        if out.is_empty() {
            "untitled".to_string()
        } else {
            out
        }
    }

    fn load_existing_fixtures(fixtures_dir: &Path) -> std::collections::HashMap<String, PathBuf> {
        let mut map = std::collections::HashMap::new();
        let Ok(entries) = fs::read_dir(fixtures_dir) else {
            return map;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "mmd") {
                if let Ok(text) = fs::read_to_string(&path) {
                    let canon = canonical_fixture_text(&text);
                    map.insert(canon, path);
                }
            }
        }
        map
    }

    fn normalize_diagram_dir(detected: &str) -> Option<String> {
        match detected {
            "flowchart" | "flowchart-v2" | "flowchart-elk" => Some("flowchart".to_string()),
            "state" | "stateDiagram" => Some("state".to_string()),
            "class" | "classDiagram" => Some("class".to_string()),
            "gitGraph" => Some("gitgraph".to_string()),
            "quadrantChart" => Some("quadrantchart".to_string()),
            "er" => Some("er".to_string()),
            "journey" => Some("journey".to_string()),
            "xychart" => Some("xychart".to_string()),
            "requirement" => Some("requirement".to_string()),
            "architecture" | "block" | "c4" | "gantt" | "info" | "kanban" | "mindmap"
            | "packet" | "pie" | "radar" | "sankey" | "sequence" | "timeline" | "treemap" => {
                Some(detected.to_string())
            }
            _ => None,
        }
    }

    #[derive(Debug, Clone)]
    struct Candidate {
        source_path: PathBuf,
        diagram_dir: String,
        stem: String,
        body: String,
        score: i64,
    }

    fn score_for_body(body: &str) -> i64 {
        let line_count = body.lines().count() as i64;
        (line_count * 1_000) + (body.len() as i64)
    }

    let reg = merman::detect::DetectorRegistry::default_mermaid_11_12_2_full();

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    let Ok(top_entries) = fs::read_dir(&mmdr_root) else {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "failed to list mmdr fixtures directory {}",
            mmdr_root.display()
        )));
    };
    for top_entry in top_entries.flatten() {
        let dir_path = top_entry.path();
        if !dir_path.is_dir() {
            continue;
        }
        let dir_name = dir_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        if dir_name == "node_modules" || dir_name == "target" {
            continue;
        }

        let Ok(entries) = fs::read_dir(&dir_path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !is_file_with_extension(&path, "mmd") {
                continue;
            }
            let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };

            if let Some(f) = filter.as_deref() {
                let hay = format!(
                    "{} {}",
                    dir_name,
                    path.file_name().and_then(|n| n.to_str()).unwrap_or("")
                );
                if !hay.contains(f) {
                    continue;
                }
            }

            let text = match fs::read_to_string(&path) {
                Ok(v) => v,
                Err(err) => {
                    skipped.push(format!("skip (read failed): {} ({err})", path.display()));
                    continue;
                }
            };
            let body = canonical_fixture_text(&text);
            if body.trim().is_empty() {
                continue;
            }

            let mut cfg = merman::MermaidConfig::default();
            let detected = match reg.detect_type(body.as_str(), &mut cfg) {
                Ok(t) => t,
                Err(_) => {
                    skipped.push(format!("skip (type not detected): {}", path.display()));
                    continue;
                }
            };
            let Some(diagram_dir) = normalize_diagram_dir(detected) else {
                skipped.push(format!(
                    "skip (unsupported detected type '{detected}'): {}",
                    path.display()
                ));
                continue;
            };

            if diagram_dir == "zenuml" {
                continue;
            }
            if diagram != "all" && diagram_dir != diagram {
                continue;
            }

            let stem = format!(
                "mmdr_tests_{diagram_dir}_{}_{}",
                sanitize_stem(&dir_name),
                sanitize_stem(file_stem)
            );

            candidates.push(Candidate {
                source_path: path,
                diagram_dir,
                stem,
                score: score_for_body(&body),
                body,
            });
        }
    }

    if prefer_complex {
        candidates.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.stem.cmp(&b.stem)));
    } else {
        candidates.sort_by(|a, b| a.stem.cmp(&b.stem));
    }

    let mut existing_by_diagram: std::collections::HashMap<
        String,
        std::collections::HashMap<String, PathBuf>,
    > = std::collections::HashMap::new();
    let mut created: Vec<(String, String, PathBuf)> = Vec::new();

    let mut imported = 0usize;
    for c in candidates {
        let fixtures_dir = workspace_root.join("fixtures").join(&c.diagram_dir);
        if !fixtures_dir.is_dir() {
            skipped.push(format!(
                "skip (fixtures dir missing): {}",
                fixtures_dir.display()
            ));
            continue;
        }

        let existing = existing_by_diagram
            .entry(c.diagram_dir.clone())
            .or_insert_with(|| load_existing_fixtures(&fixtures_dir));
        if let Some(existing_path) = existing.get(&c.body) {
            skipped.push(format!(
                "skip (duplicate content): {} -> {}",
                c.source_path.display(),
                existing_path.display()
            ));
            continue;
        }

        let out_path = fixtures_dir.join(format!("{}.mmd", c.stem));
        if out_path.exists() && !overwrite {
            skipped.push(format!("skip (exists): {}", out_path.display()));
            continue;
        }

        fs::write(&out_path, &c.body).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to write fixture {}: {err}",
                out_path.display()
            ))
        })?;
        existing.insert(c.body.clone(), out_path.clone());
        created.push((c.diagram_dir.clone(), c.stem.clone(), out_path));

        imported += 1;
        if let Some(max) = limit {
            if imported >= max {
                break;
            }
        }
    }

    if created.is_empty() {
        return Err(XtaskError::SnapshotUpdateFailed(
            "no fixtures were imported (use --diagram <name> and optionally --filter/--limit)"
                .to_string(),
        ));
    }

    if install && !with_baselines {
        return Err(XtaskError::SnapshotUpdateFailed(
            "`--install` only applies when `--with-baselines` is set".to_string(),
        ));
    }

    if with_baselines {
        for (diagram_dir, stem, _) in &created {
            let mut svg_args = vec![
                "--diagram".to_string(),
                diagram_dir.clone(),
                "--filter".to_string(),
                stem.clone(),
            ];
            if install {
                svg_args.push("--install".to_string());
            }
            gen_upstream_svgs(svg_args)?;
            update_snapshots(vec![
                "--diagram".to_string(),
                diagram_dir.clone(),
                "--filter".to_string(),
                stem.clone(),
            ])?;
            update_layout_snapshots(vec![
                "--diagram".to_string(),
                diagram_dir.clone(),
                "--filter".to_string(),
                stem.clone(),
            ])?;
        }
    }

    eprintln!("Imported {} fixtures:", created.len());
    for (_, _, path) in &created {
        eprintln!("  {}", path.display());
    }
    if !skipped.is_empty() {
        eprintln!("Skipped {} fixtures:", skipped.len());
        for s in skipped.iter().take(50) {
            eprintln!("  {s}");
        }
        if skipped.len() > 50 {
            eprintln!("  ... ({} more)", skipped.len() - 50);
        }
    }

    Ok(())
}

fn report_overrides(args: Vec<String>) -> Result<(), XtaskError> {
    if args.iter().any(|a| matches!(a.as_str(), "--help" | "-h")) {
        println!("usage: xtask report-overrides");
        println!();
        println!("Prints a lightweight inventory of parity override footprint.");
        println!("This is intended for CI logs and drift reviews.");
        return Ok(());
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let generated_dir = workspace_root
        .join("crates")
        .join("merman-render")
        .join("src")
        .join("generated");

    fn read_text(path: &Path) -> Result<String, XtaskError> {
        fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })
    }

    fn count_matches(re: &Regex, text: &str) -> usize {
        re.find_iter(text).count()
    }

    static ROOT_VIEWPORT_ENTRY_RE: OnceLock<Regex> = OnceLock::new();
    static STATE_TEXT_ENTRY_RE: OnceLock<Regex> = OnceLock::new();
    let root_viewport_entry_re = ROOT_VIEWPORT_ENTRY_RE
        .get_or_init(|| Regex::new(r#""[^"]+"\s*=>\s*(?:\{\s*)?Some\("#).expect("valid regex"));
    let state_text_entry_re =
        STATE_TEXT_ENTRY_RE.get_or_init(|| Regex::new(r#"=>\s*Some\("#).expect("valid regex"));

    let architecture = generated_dir.join("architecture_root_overrides_11_12_2.rs");
    let flowchart = generated_dir.join("flowchart_root_overrides_11_12_2.rs");
    let class = generated_dir.join("class_root_overrides_11_12_2.rs");
    let mindmap = generated_dir.join("mindmap_root_overrides_11_12_2.rs");
    let gitgraph = generated_dir.join("gitgraph_root_overrides_11_12_2.rs");
    let pie = generated_dir.join("pie_root_overrides_11_12_2.rs");
    let sankey = generated_dir.join("sankey_root_overrides_11_12_2.rs");
    let sequence = generated_dir.join("sequence_root_overrides_11_12_2.rs");
    let state_root = generated_dir.join("state_root_overrides_11_12_2.rs");
    let state_text = generated_dir.join("state_text_overrides_11_12_2.rs");
    let timeline = generated_dir.join("timeline_root_overrides_11_12_2.rs");

    let architecture_txt = read_text(&architecture)?;
    let flowchart_txt = read_text(&flowchart)?;
    let class_txt = read_text(&class)?;
    let mindmap_txt = read_text(&mindmap)?;
    let gitgraph_txt = read_text(&gitgraph)?;
    let pie_txt = read_text(&pie)?;
    let sankey_txt = read_text(&sankey)?;
    let sequence_txt = read_text(&sequence)?;
    let state_root_txt = read_text(&state_root)?;
    let state_text_txt = read_text(&state_text)?;
    let timeline_txt = read_text(&timeline)?;

    let architecture_n = count_matches(root_viewport_entry_re, &architecture_txt);
    let flowchart_n = count_matches(root_viewport_entry_re, &flowchart_txt);
    let class_n = count_matches(root_viewport_entry_re, &class_txt);
    let mindmap_n = count_matches(root_viewport_entry_re, &mindmap_txt);
    let gitgraph_n = count_matches(root_viewport_entry_re, &gitgraph_txt);
    let pie_n = count_matches(root_viewport_entry_re, &pie_txt);
    let sankey_n = count_matches(root_viewport_entry_re, &sankey_txt);
    let sequence_n = count_matches(root_viewport_entry_re, &sequence_txt);
    let state_root_n = count_matches(root_viewport_entry_re, &state_root_txt);
    let state_text_n = count_matches(state_text_entry_re, &state_text_txt);
    let timeline_n = count_matches(root_viewport_entry_re, &timeline_txt);

    println!("Mermaid baseline: @11.12.2");
    println!();
    println!("Root viewport overrides:");
    println!("- architecture_root_overrides_11_12_2.rs: {architecture_n} entries");
    println!("- flowchart_root_overrides_11_12_2.rs: {flowchart_n} entries");
    println!("- class_root_overrides_11_12_2.rs: {class_n} entries");
    println!("- mindmap_root_overrides_11_12_2.rs: {mindmap_n} entries");
    println!("- gitgraph_root_overrides_11_12_2.rs: {gitgraph_n} entries");
    println!("- pie_root_overrides_11_12_2.rs: {pie_n} entries");
    println!("- sankey_root_overrides_11_12_2.rs: {sankey_n} entries");
    println!("- sequence_root_overrides_11_12_2.rs: {sequence_n} entries");
    println!("- state_root_overrides_11_12_2.rs: {state_root_n} entries");
    println!("- timeline_root_overrides_11_12_2.rs: {timeline_n} entries");
    println!();
    println!("State text/bbox overrides:");
    println!(
        "- state_text_overrides_11_12_2.rs: {state_text_n} entries (\"=> Some(...)\" match arms)"
    );

    Ok(())
}

fn gen_svg_overrides(args: Vec<String>) -> Result<(), XtaskError> {
    let mut in_dir: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut base_font_size_px: f64 = 16.0;
    let mut mode: String = "sequence".to_string();
    let mut browser_exe: Option<PathBuf> = None;
    let mut text_anchor: String = "start".to_string();
    let mut preserve_spaces: bool = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--in" => {
                i += 1;
                in_dir = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--font-size" => {
                i += 1;
                base_font_size_px = args
                    .get(i)
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(16.0);
            }
            "--mode" => {
                i += 1;
                mode = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "sequence".to_string());
            }
            "--browser-exe" => {
                i += 1;
                browser_exe = args.get(i).map(PathBuf::from);
            }
            "--text-anchor" => {
                i += 1;
                text_anchor = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "start".to_string());
            }
            "--preserve-spaces" => preserve_spaces = true,
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let in_dir = in_dir.ok_or(XtaskError::Usage)?;
    let out_path = out_path.ok_or(XtaskError::Usage)?;

    fn normalize_font_key(s: &str) -> String {
        s.chars()
            .filter_map(|ch| {
                if ch.is_whitespace() || ch == '"' || ch == '\'' || ch == ';' {
                    None
                } else {
                    Some(ch.to_ascii_lowercase())
                }
            })
            .collect()
    }

    fn extract_base_font_family(svg: &str) -> String {
        let Ok(doc) = roxmltree::Document::parse(svg) else {
            return String::new();
        };
        let Some(root) = doc.descendants().find(|n| n.has_tag_name("svg")) else {
            return String::new();
        };
        let id = root.attribute("id").unwrap_or_default();
        let Some(style_node) = doc.descendants().find(|n| n.has_tag_name("style")) else {
            return String::new();
        };
        let style_text = style_node.text().unwrap_or_default();
        if id.is_empty() || style_text.is_empty() {
            return String::new();
        }
        let pat = format!(
            r#"#{id}\{{[^}}]*font-family:([^;}}]+)"#,
            id = regex::escape(id)
        );
        let Ok(re) = Regex::new(&pat) else {
            return String::new();
        };
        let Some(caps) = re.captures(style_text) else {
            return String::new();
        };
        caps.get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    }

    fn parse_style_font_size_px(style: &str) -> Option<f64> {
        // Very small parser for `font-size: 16px;` patterns.
        let s = style.to_ascii_lowercase();
        let idx = s.find("font-size")?;
        let rest = &s[idx + "font-size".len()..];
        let rest = rest.trim_start_matches(|c: char| c == ':' || c.is_whitespace());
        let mut num = String::new();
        for ch in rest.chars() {
            if ch.is_ascii_digit() || ch == '.' {
                num.push(ch);
            } else {
                break;
            }
        }
        if num.is_empty() {
            return None;
        }
        num.parse::<f64>().ok()
    }

    fn node_is_inside_defs(n: roxmltree::Node<'_, '_>) -> bool {
        n.ancestors()
            .filter(|a| a.is_element())
            .any(|a| a.has_tag_name("defs"))
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone)]
    struct SampleKey {
        font_key: String,
        font_family_raw: String,
        size_key: usize,
    }

    let Ok(entries) = fs::read_dir(&in_dir) else {
        return Err(XtaskError::ReadFile {
            path: in_dir.display().to_string(),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        });
    };

    // font_key + size_key => strings
    let mut strings_by_key: BTreeMap<(String, usize), Vec<String>> = BTreeMap::new();
    let mut family_by_font_key: BTreeMap<String, String> = BTreeMap::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_file_with_extension(&path, "svg") {
            continue;
        }
        let svg = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;

        let base_family_raw = extract_base_font_family(&svg);
        let font_key = normalize_font_key(&base_family_raw);
        if font_key.is_empty() {
            continue;
        }
        family_by_font_key
            .entry(font_key.clone())
            .or_insert_with(|| base_family_raw.clone());

        let Ok(doc) = roxmltree::Document::parse(&svg) else {
            continue;
        };

        for text_node in doc.descendants().filter(|n| n.has_tag_name("text")) {
            if node_is_inside_defs(text_node) {
                continue;
            }
            let class = text_node.attribute("class").unwrap_or_default();
            let tokens: Vec<&str> = class.split_whitespace().collect();

            let include = match mode.as_str() {
                "all" => true,
                // For strict SVG XML parity, sequence layout is extremely sensitive to message
                // text width (it drives `actor.margin` and thus all x coordinates). We start by
                // generating overrides from Mermaid's own text measurement. In practice, actor
                // box sizing is also driven by `calculateTextDimensions(...)`, so include actor
                // labels as well to avoid drift on long participant ids.
                "sequence" => tokens.iter().any(|t| matches!(*t, "messageText" | "actor")),
                _ => false,
            };
            if !include {
                continue;
            }

            let size_px = text_node
                .attribute("font-size")
                .and_then(|v| v.parse::<f64>().ok())
                .or_else(|| {
                    text_node
                        .attribute("style")
                        .and_then(parse_style_font_size_px)
                })
                .unwrap_or(base_font_size_px)
                .max(1.0);
            let size_key = (size_px * 1000.0).round().max(1.0) as usize;

            let mut pushed = false;
            for tspan in text_node.children().filter(|n| n.has_tag_name("tspan")) {
                if node_is_inside_defs(tspan) {
                    continue;
                }
                let raw = tspan.text().unwrap_or_default().to_string();
                if raw.trim().is_empty() {
                    continue;
                }
                pushed = true;
                strings_by_key
                    .entry((font_key.clone(), size_key))
                    .or_default()
                    .push(raw);
            }
            if pushed {
                continue;
            }
            let raw = text_node.text().unwrap_or_default().to_string();
            if raw.trim().is_empty() {
                continue;
            }
            strings_by_key
                .entry((font_key.clone(), size_key))
                .or_default()
                .push(raw);
        }
    }

    // For Mermaid `sequenceDiagram`, text widths are computed from the *encoded* Mermaid source
    // (after `encodeEntities(...)`), not from the final decoded SVG glyphs. To match upstream,
    // include raw strings extracted from our pinned fixture corpus as additional override seeds.
    if mode == "sequence" {
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");
        let fixtures_dir = workspace_root.join("fixtures").join("sequence");

        let engine = merman::Engine::new();
        let parse_opts = merman::ParseOptions {
            suppress_errors: true,
        };

        let mut extra: Vec<String> = Vec::new();
        if let Ok(entries) = fs::read_dir(&fixtures_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !is_file_with_extension(&path, "mmd") {
                    continue;
                }
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                let parsed =
                    match futures::executor::block_on(engine.parse_diagram(&text, parse_opts)) {
                        Ok(Some(v)) => v,
                        _ => continue,
                    };

                let m = &parsed.model;
                if let Some(actors) = m.get("actors").and_then(|v| v.as_object()) {
                    for a in actors.values() {
                        if let Some(s) = a.get("description").and_then(|v| v.as_str()) {
                            extra.push(s.to_string());
                        }
                    }
                }
                if let Some(msgs) = m.get("messages").and_then(|v| v.as_array()) {
                    for msg in msgs {
                        if let Some(s) = msg.get("message").and_then(|v| v.as_str()) {
                            extra.push(s.to_string());
                        }
                    }
                }
                if let Some(notes) = m.get("notes").and_then(|v| v.as_array()) {
                    for note in notes {
                        if let Some(s) = note.get("message").and_then(|v| v.as_str()) {
                            extra.push(s.to_string());
                        }
                    }
                }
                if let Some(boxes) = m.get("boxes").and_then(|v| v.as_array()) {
                    for b in boxes {
                        if let Some(s) = b.get("name").and_then(|v| v.as_str()) {
                            extra.push(s.to_string());
                        }
                    }
                }
                if let Some(title) = m.get("title").and_then(|v| v.as_str()) {
                    extra.push(title.to_string());
                }
            }
        }

        if !extra.is_empty() {
            for v in strings_by_key.values_mut() {
                v.extend(extra.iter().cloned());
            }
        }
    }

    if strings_by_key.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no svg text samples found under {}",
            in_dir.display()
        )));
    }

    #[derive(Debug, Clone, Copy, serde::Deserialize)]
    struct SvgTextBBoxMetrics {
        bbox_x: f64,
        bbox_w: f64,
    }

    #[derive(Debug, Clone, serde::Deserialize)]
    struct SequenceMessageWidth {
        // `utils.calculateTextDimensions(...).width` (NOT including wrapPadding).
        width_px: Option<f64>,
        #[serde(default)]
        center_diff: Option<f64>,
        #[serde(default)]
        margin_px: Option<f64>,
        #[serde(default)]
        debug_line_ids: Option<Vec<String>>,
        #[serde(default)]
        debug_svg_start: Option<String>,
        #[serde(default)]
        debug_actor_x1: Option<Vec<f64>>,
        #[serde(default)]
        debug_actor_rect_w: Option<Vec<f64>>,
        #[serde(default)]
        debug_cfg_message_font_family: Option<String>,
        #[serde(default)]
        debug_cfg_actor_margin: Option<f64>,
        #[serde(default)]
        debug_cfg_wrap_padding: Option<f64>,
        #[serde(default)]
        debug_cfg_width: Option<f64>,
    }

    fn measure_svg_text_bbox_metrics_via_browser(
        node_cwd: &Path,
        browser_exe: &Path,
        font_family: &str,
        font_size_px: f64,
        text_anchor: &str,
        preserve_spaces: bool,
        strings: &[String],
    ) -> Result<Vec<SvgTextBBoxMetrics>, XtaskError> {
        use std::process::Stdio;
        if strings.is_empty() {
            return Ok(Vec::new());
        }
        // Mermaid's default config ships `fontFamily` with a trailing `;` (see `getConfig()`),
        // and `sequenceRenderer.setConf(...)` copies that verbatim into `messageFontFamily`.
        //
        // When applying font families via CSSOM (as `calculateTextDimensions()` does), that
        // trailing `;` can change fallback font selection under Puppeteer headless shell. Our
        // upstream SVG baselines are generated via `mmdc` (headless shell), so preserve that
        // behavior by measuring with a trailing `;` here.
        let font_family = {
            // IMPORTANT: `calculateTextDimensions()` applies `fontFamily` via CSSOM:
            // `selection.style('font-family', fontFamily)`, i.e. `CSSStyleDeclaration::setProperty`.
            //
            // Mermaid's default `fontFamily` string includes a trailing `;` (see Mermaid config).
            // In Chromium (esp. Puppeteer headless shell), passing that exact value to CSSOM can
            // cause the declaration to be rejected and the UA fallback font to be used instead.
            //
            // Our upstream SVG baselines are generated via `mmdc` (headless shell), so we must
            // preserve this behavior here (do not strip quotes; only ensure a trailing `;`).
            let trimmed = font_family.trim_end();
            if trimmed.ends_with(';') {
                trimmed.to_string()
            } else {
                format!("{trimmed};")
            }
        };
        let input_json = serde_json::json!({
            "browser_exe": browser_exe.display().to_string(),
            "font_family": font_family,
            "font_size_px": font_size_px,
            "text_anchor": text_anchor,
            "preserve_spaces": preserve_spaces,
            "strings": strings,
        })
        .to_string();
        const JS: &str = r#"
const fs = require('fs');
const puppeteer = require('puppeteer-core');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const browserExe = input.browser_exe;
const fontFamily = input.font_family;
const fontSizePx = input.font_size_px;
const textAnchor = input.text_anchor;
const preserveSpaces = !!input.preserve_spaces;
const strings = input.strings;

(async () => {
  const browser = await puppeteer.launch({
    headless: 'shell',
    executablePath: browserExe,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  const page = await browser.newPage();
  await page.setContent(`<!doctype html><html><head><style>body{margin:0;padding:0;}</style></head><body></body></html>`);

  const out = await page.evaluate(({ strings, fontFamily, fontSizePx, textAnchor, preserveSpaces }) => {
    const SVG_NS = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('width', '2000');
    svg.setAttribute('height', '200');
    document.body.appendChild(svg);

    // `mermaid/utils.calculateTextDimensions()` measures both `'sans-serif'` and the supplied
    // font-family, then selects a result based on a heuristic (to handle missing user fonts).
    // For strict parity with `mmdc` baselines (which run under Puppeteer headless shell), we
    // replicate that logic here and store the chosen width as our override.
    const ff = String(fontFamily || '');
    const res = [];
    for (const s of strings) {
      const raw = String(s);
      const normalized = raw
        .replace(/<br\s*\/?\s*>/gi, ' ')
        .replace(/[\r\n]+/g, ' ');

      function measureWithFont(fontFamily) {
        const t = document.createElementNS(SVG_NS, 'text');
        t.setAttribute('x', '0');
        t.setAttribute('y', '0');
        const tspan = document.createElementNS(SVG_NS, 'tspan');
        tspan.setAttribute('x', '0');
        t.appendChild(tspan);

        // Mirror Mermaid `drawSimpleText(...).style(...)` behavior: apply presentation attributes
        // via CSSOM (not by string-building a `style="..."` attribute), because `fontFamily`
        // can contain a trailing `;` which must be parsed the same way as upstream baselines.
        t.style.setProperty('text-anchor', String(textAnchor || 'start'));
        t.style.setProperty('font-size', `${fontSizePx}px`);
        t.style.setProperty('font-weight', '400');
        t.style.setProperty('font-family', String(fontFamily || ''));
        if (preserveSpaces) {
          t.setAttribute('xml:space', 'preserve');
          t.style.setProperty('white-space', 'pre');
        }

        tspan.textContent = normalized || '\u200b';
        svg.appendChild(t);
        const bb = t.getBBox();
        svg.removeChild(t);
        const w = Math.round(bb.width);
        const h = Math.round(bb.height);
        return { w, h, lineHeight: h };
      }

      const dims0 = measureWithFont('sans-serif');
      const dims1 = measureWithFont(ff);
      const use0 = Number.isNaN(dims1.h) ||
        Number.isNaN(dims1.w) ||
        Number.isNaN(dims1.lineHeight) ||
        (dims0.h > dims1.h && dims0.w > dims1.w && dims0.lineHeight > dims1.lineHeight);
      const chosen = use0 ? dims0 : dims1;

      res.push({ bbox_x: 0, bbox_w: chosen.w });
    }
    return res;
  }, { strings, fontFamily, fontSizePx, textAnchor, preserveSpaces });

  console.log(JSON.stringify(out));
  await browser.close();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
"#;

        let mut cmd = Command::new("node");
        cmd.current_dir(node_cwd)
            .arg("-e")
            .arg(JS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to spawn node: {source}"))
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            std::io::Write::write_all(&mut stdin, input_json.as_bytes()).map_err(|source| {
                XtaskError::SvgCompareFailed(format!("failed to write node stdin: {source}"))
            })?;
        }
        let output = child.wait_with_output().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to run node: {source}"))
        })?;
        if !output.status.success() {
            return Err(XtaskError::SvgCompareFailed(
                "browser svg measurement failed".to_string(),
            ));
        }
        let raw: Vec<SvgTextBBoxMetrics> =
            serde_json::from_slice(&output.stdout).map_err(XtaskError::Json)?;
        Ok(raw)
    }

    fn infer_sequence_message_dimensions_width_px_via_mermaid_layout(
        node_cwd: &Path,
        browser_exe: Option<&Path>,
        strings: &[String],
    ) -> Result<Vec<SequenceMessageWidth>, XtaskError> {
        use std::process::Stdio;
        if strings.is_empty() {
            return Ok(Vec::new());
        }

        let debug = std::env::var_os("MERMAN_XTASK_DEBUG_SEQUENCE").is_some();
        let input_json = serde_json::json!({
            "browser_exe": browser_exe.map(|p| p.display().to_string()),
            "strings": strings,
            "debug": debug,
        })
        .to_string();

        // IMPORTANT: we infer Mermaid's internal `calculateTextDimensions(...).width` by
        // rendering a minimal 2-actor sequence diagram and inverting Mermaid's margin formula.
        //
        // Mermaid computes an actor-to-next margin using:
        //
        //   actor.margin = max(conf.actorMargin, messageWidth + conf.actorMargin - actor.width/2 - next.width/2)
        //
        // where:
        //
        //   messageWidth = calculateTextDimensions.width + 2*conf.wrapPadding
        //
        // If the margin saturates to `conf.actorMargin`, the exact width can't be recovered from
        // layout. To avoid that, we intentionally render with a very small `sequence.width`,
        // making actor widths small enough that typical message labels are in the non-saturated
        // regime.
        const JS: &str = r#"
const fs = require('fs');
const path = require('path');
const url = require('url');
const { createRequire } = require('module');
const requireFromCwd = createRequire(path.join(process.cwd(), 'package.json'));
const puppeteer = requireFromCwd('puppeteer');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const browserExe = input.browser_exe || null;
 const strings = input.strings || [];
 const debug = !!input.debug;

const cliRoot = process.cwd();
const mermaidHtmlPath = path.join(cliRoot, 'node_modules', '@mermaid-js', 'mermaid-cli', 'dist', 'index.html');
const mermaidIifePath = path.join(cliRoot, 'node_modules', 'mermaid', 'dist', 'mermaid.js');
const zenumlIifePath = path.join(cliRoot, 'node_modules', '@mermaid-js', 'mermaid-zenuml', 'dist', 'mermaid-zenuml.js');

(async () => {
  const launchOpts = { headless: 'shell', args: ['--no-sandbox', '--disable-setuid-sandbox'] };
  // NOTE: mmdc does NOT set `executablePath`, letting Puppeteer pick the best
  // headless-shell binary. Only use an explicit path if provided.
  if (browserExe) {
    launchOpts.executablePath = browserExe;
  }
  const browser = await puppeteer.launch(launchOpts);

  const page = await browser.newPage();
  await page.goto(url.pathToFileURL(mermaidHtmlPath).href);
  await page.addScriptTag({ path: mermaidIifePath });

   const out = await page.evaluate(async ({ strings, debug }) => {
    const mermaid = globalThis.mermaid;
    if (!mermaid) {
      throw new Error('mermaid global not found');
    }
    // Match upstream fixture generation: deterministic handDrawn seed, default theme, and
    // explicit sequence defaults to avoid any drift from build-time or environment defaults.
     mermaid.initialize({
       startOnLoad: false,
       theme: 'default',
       handDrawnSeed: 1,
       sequence: {
         actorMargin: 50,
         // Use a tiny min actor width to avoid margin saturation at `actorMargin`, so we can
         // invert from actor center distance to the internal text width deterministically.
         width: 1,
         wrapPadding: 10,
         messageFontSize: 16,
         messageFontFamily: '\"trebuchet ms\", verdana, arial, sans-serif',
       },
     });
     const cfg = mermaid.mermaidAPI && mermaid.mermaidAPI.getConfig ? mermaid.mermaidAPI.getConfig() : null;
     const cfgSeq = cfg && cfg.sequence ? cfg.sequence : {};

     const results = [];
     const container = document.getElementById('container') || document.body;
     const ACTOR_MARGIN = 50; // conf.actorMargin default
     const WRAP_PADDING = 10; // conf.wrapPadding default

    for (let i = 0; i < strings.length; i++) {
      const raw = String(strings[i] ?? '');
      // Keep the label as-is; Mermaid will normalize `<br/>` for width calculations internally.
      const def = [
        'sequenceDiagram',
        'participant A',
        'participant B',
        `A->>B: ${raw}`,
       ].join('\n');

      // Use a stable SVG id to mirror `mmdc` defaults (unless the user passes `--svgId`).
      // This reduces the risk of accidental id-scoped CSS differences affecting measurement.
      container.innerHTML = '';
      const { svg } = await mermaid.render('my-svg', def, container);

       const doc = new DOMParser().parseFromString(svg, 'image/svg+xml');
       const parseNumber = (v) => {
         const n = Number(v);
         return Number.isFinite(n) ? n : null;
       };

       // Mermaid increments actor line ids across renders (`actor0/actor1`, then `actor2/actor3`,
       // ...). Use the `actor{N}` id pattern and infer left/right ordering by x coordinate.
       const actorLines = Array.from(doc.querySelectorAll('line'))
         .filter((n) => /^actor\d+$/.test(String(n.getAttribute('id') || '')));
       if (actorLines.length < 2) {
         const lineIds = Array.from(doc.querySelectorAll('line'))
           .map((n) => n.getAttribute('id'))
           .filter((s) => !!s)
           .slice(0, 8);
         results.push({
           width_px: null,
           center_diff: null,
           margin_px: null,
           debug_line_ids: lineIds,
           debug_svg_start: svg.slice(0, 160),
         });
         continue;
       }
       const xs = actorLines
         .map((n) => parseNumber(n.getAttribute('x1')))
         .filter((n) => n !== null)
         .sort((a, b) => a - b);
       if (xs.length < 2) {
         const lineIds = Array.from(doc.querySelectorAll('line'))
           .map((n) => n.getAttribute('id'))
           .filter((s) => !!s)
           .slice(0, 8);
         results.push({
           width_px: null,
           center_diff: null,
           margin_px: null,
           debug_line_ids: lineIds,
           debug_svg_start: svg.slice(0, 160),
         });
         continue;
       }
       const centerDiff = xs[xs.length - 1] - xs[0];
       const rectWs = Array.from(doc.querySelectorAll('rect'))
         .filter((n) => String(n.getAttribute('class') || '').split(/\\s+/g).includes('actor-top'))
         .map((n) => parseNumber(n.getAttribute('width')))
         .filter((n) => n !== null)
         .slice(0, 4);
       const w0 = rectWs.length >= 1 ? rectWs[0] : null;
       const w1 = rectWs.length >= 2 ? rectWs[1] : null;
       const margin = (w0 !== null && w1 !== null) ? (centerDiff - (w0 / 2) - (w1 / 2)) : null;

       // With non-saturated margins (ensured by `sequence.width: 1`), we have:
       //   centerDiff = messageWidth + ACTOR_MARGIN
       //   messageWidth = calculateTextDimensions.width + 2*WRAP_PADDING
       const inferredWidthPx = Math.round(centerDiff - ACTOR_MARGIN - 2 * WRAP_PADDING);
       const meta = {
         width_px: Number.isFinite(inferredWidthPx) ? inferredWidthPx : null,
         center_diff: centerDiff,
         margin_px: margin,
       };
       if (debug) {
         meta.debug_actor_x1 = xs;
         meta.debug_actor_rect_w = rectWs;
         if (i === 0 && cfgSeq) {
           meta.debug_cfg_message_font_family = String(cfgSeq.messageFontFamily ?? '');
           meta.debug_cfg_actor_margin = Number(cfgSeq.actorMargin ?? NaN);
           meta.debug_cfg_wrap_padding = Number(cfgSeq.wrapPadding ?? NaN);
           meta.debug_cfg_width = Number(cfgSeq.width ?? NaN);
         }
       }
       results.push(meta);
     }
     return results;
   }, { strings, debug });

  console.log(JSON.stringify(out));
  await browser.close();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
"#;

        let mut cmd = Command::new("node");
        cmd.current_dir(node_cwd)
            .arg("-e")
            .arg(JS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = cmd.spawn().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to spawn node: {source}"))
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            std::io::Write::write_all(&mut stdin, input_json.as_bytes()).map_err(|source| {
                XtaskError::SvgCompareFailed(format!("failed to write node stdin: {source}"))
            })?;
        }
        let output = child.wait_with_output().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to run node: {source}"))
        })?;
        if !output.status.success() {
            return Err(XtaskError::SvgCompareFailed(
                "sequence layout inference failed".to_string(),
            ));
        }
        let raw: Vec<SequenceMessageWidth> =
            serde_json::from_slice(&output.stdout).map_err(XtaskError::Json)?;
        Ok(raw)
    }

    fn detect_windows_browser_exe() -> Option<PathBuf> {
        let candidates = [
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ];
        for p in candidates {
            let path = PathBuf::from(p);
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    let browser_exe = if let Some(p) = browser_exe.as_deref() {
        p.to_path_buf()
    } else if cfg!(windows) {
        detect_windows_browser_exe().ok_or_else(|| {
            XtaskError::SvgCompareFailed("no supported browser found for svg measurement".into())
        })?
    } else {
        return Err(XtaskError::SvgCompareFailed(
            "browser measurement requires --browser-exe on this platform".into(),
        ));
    };

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let node_cwd = workspace_root.join("tools").join("mermaid-cli");

    // font_key => (text => (size_key, left_em, right_em))
    let mut best_by_font: BTreeMap<String, BTreeMap<String, (usize, f64, f64)>> = BTreeMap::new();
    let base_size_key = (base_font_size_px * 1000.0).round().max(1.0) as usize;

    for ((font_key, size_key), mut strings) in strings_by_key {
        strings.sort();
        strings.dedup();
        if strings.is_empty() {
            continue;
        }
        let Some(font_family_raw) = family_by_font_key.get(&font_key).cloned() else {
            continue;
        };
        let font_size_px = (size_key as f64) / 1000.0;
        let denom = font_size_px.max(1.0);
        let by_text = best_by_font.entry(font_key.clone()).or_default();

        if mode == "sequence" {
            // For sequence message text, infer widths from Mermaid layout itself (see helper).
            let debug = std::env::var_os("MERMAN_XTASK_DEBUG_SEQUENCE").is_some();
            if debug {
                eprintln!(
                    "[gen-svg-overrides] sequence: font_key={font_key} size_px={font_size_px} unique_strings={}",
                    strings.len()
                );
                for s in strings.iter().take(8) {
                    eprintln!("  sample: {:?}", s);
                }
            }
            let raw = infer_sequence_message_dimensions_width_px_via_mermaid_layout(
                &node_cwd, None, &strings,
            )?;
            let widths = raw.iter().map(|m| m.width_px).collect::<Vec<_>>();
            if debug {
                let inferred = widths.iter().filter(|w| w.is_some()).count();
                eprintln!("  inferred_widths={inferred}");
                for ((s, w), meta) in strings.iter().zip(widths.iter()).zip(raw.iter()).take(8) {
                    eprintln!(
                        "  out: {:?} => width={:?} (center_diff={:?}, margin_px={:?}, debug_actor_x1={:?}, debug_actor_rect_w={:?}, cfg={:?}/{:?}/{:?}/{:?}, debug_line_ids={:?})",
                        s,
                        w,
                        meta.center_diff,
                        meta.margin_px,
                        meta.debug_actor_x1,
                        meta.debug_actor_rect_w,
                        meta.debug_cfg_message_font_family,
                        meta.debug_cfg_actor_margin,
                        meta.debug_cfg_wrap_padding,
                        meta.debug_cfg_width,
                        meta.debug_line_ids
                    );
                    if meta.center_diff.is_none() {
                        if let Some(s) = meta.debug_svg_start.as_deref() {
                            eprintln!("    debug_svg_start: {}", s);
                        }
                    }
                }
            }
            for (text, w_px_opt) in strings.into_iter().zip(widths.into_iter()) {
                let Some(w_px) = w_px_opt else {
                    continue;
                };
                if !w_px.is_finite() || w_px <= 0.0 {
                    continue;
                }
                let left_em = 0.0;
                let right_em = w_px / denom;
                match by_text.get(&text) {
                    None => {
                        by_text.insert(text, (size_key, left_em, right_em));
                    }
                    Some((existing_size, _, _)) if *existing_size == base_size_key => {}
                    Some((existing_size, _, _)) if size_key == base_size_key => {
                        by_text.insert(text, (size_key, left_em, right_em));
                    }
                    Some(_) => {}
                }
            }
            continue;
        }

        let metrics = measure_svg_text_bbox_metrics_via_browser(
            &node_cwd,
            &browser_exe,
            &font_family_raw,
            font_size_px,
            &text_anchor,
            preserve_spaces,
            &strings,
        )?;

        for (text, m) in strings.into_iter().zip(metrics.into_iter()) {
            let bbox_x = m.bbox_x;
            let bbox_w = m.bbox_w;
            if !(bbox_x.is_finite() && bbox_w.is_finite()) {
                continue;
            }
            let left_px = (-bbox_x).max(0.0);
            let right_px = (bbox_x + bbox_w).max(0.0);
            let left_em = left_px / denom;
            let right_em = right_px / denom;
            if !(left_em.is_finite() && right_em.is_finite() && (left_em + right_em) > 0.0) {
                continue;
            }

            match by_text.get(&text) {
                None => {
                    by_text.insert(text, (size_key, left_em, right_em));
                }
                Some((existing_size, _, _)) if *existing_size == base_size_key => {}
                Some((existing_size, _, _)) if size_key == base_size_key => {
                    by_text.insert(text, (size_key, left_em, right_em));
                }
                Some(_) => {}
            }
        }
    }

    // Render as a deterministic Rust module.
    let mut out = String::new();
    fn rust_f64(v: f64) -> String {
        let mut s = format!("{v}");
        if !s.contains('.') && !s.contains('e') && !s.contains('E') {
            s.push_str(".0");
        }
        s
    }
    let _ = writeln!(&mut out, "// This file is generated by `xtask`.\n");
    let _ = writeln!(
        &mut out,
        "pub fn lookup_svg_override_em(font_key: &str, text: &str) -> Option<(f64, f64)> {{"
    );
    let _ = writeln!(&mut out, "    match font_key {{");
    for font_key in best_by_font.keys() {
        let _ = writeln!(
            &mut out,
            "        {:?} => lookup_in_{}(),",
            font_key,
            font_key.replace(['-', ','], "_")
        );
    }
    let _ = writeln!(&mut out, "        _ => None,");
    let _ = writeln!(&mut out, "    }}");
    let _ = writeln!(&mut out, "    .and_then(|tbl| lookup_in(tbl, text))");
    let _ = writeln!(&mut out, "}}\n");

    let _ = writeln!(
        &mut out,
        "fn lookup_in(tbl: &'static [(&'static str, f64, f64)], text: &str) -> Option<(f64, f64)> {{"
    );
    let _ = writeln!(&mut out, "    let mut lo = 0usize;");
    let _ = writeln!(&mut out, "    let mut hi = tbl.len();");
    let _ = writeln!(&mut out, "    while lo < hi {{");
    let _ = writeln!(&mut out, "        let mid = (lo + hi) / 2;");
    let _ = writeln!(&mut out, "        let (k, l, r) = tbl[mid];");
    let _ = writeln!(&mut out, "        match k.cmp(text) {{");
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Equal => return Some((l, r)),"
    );
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Less => lo = mid + 1,"
    );
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Greater => hi = mid,"
    );
    let _ = writeln!(&mut out, "        }}");
    let _ = writeln!(&mut out, "    }}");
    let _ = writeln!(&mut out, "    None");
    let _ = writeln!(&mut out, "}}\n");

    for (font_key, by_text) in &best_by_font {
        let mut list: Vec<(&str, f64, f64)> = by_text
            .iter()
            .map(|(k, (_size, l, r))| (k.as_str(), *l, *r))
            .collect();
        list.sort_by(|a, b| a.0.cmp(b.0));

        let fn_name = format!("lookup_in_{}", font_key.replace(['-', ','], "_"));
        let _ = writeln!(
            &mut out,
            "fn {fn_name}() -> Option<&'static [(&'static str, f64, f64)]> {{ Some(SVG_OVERRIDES_{key}) }}",
            fn_name = fn_name,
            key = font_key.replace(['-', ','], "_").to_ascii_uppercase()
        );
        let _ = writeln!(
            &mut out,
            "static SVG_OVERRIDES_{key}: &[(&str, f64, f64)] = &[",
            key = font_key.replace(['-', ','], "_").to_ascii_uppercase()
        );
        for (text, l, r) in &list {
            let _ = writeln!(
                &mut out,
                "    ({:?}, {}, {}),",
                text,
                rust_f64(*l),
                rust_f64(*r)
            );
        }
        let _ = writeln!(&mut out, "];\n");
    }

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    std::fs::write(&out_path, out).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;
    Ok(())
}

fn gen_er_text_overrides(args: Vec<String>) -> Result<(), XtaskError> {
    use std::collections::{BTreeMap, BTreeSet};

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    let mut in_dir: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--in" => {
                i += 1;
                in_dir = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let in_dir = in_dir.unwrap_or_else(|| {
        workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("er")
    });
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("crates")
            .join("merman-render")
            .join("src")
            .join("generated")
            .join("er_text_overrides_11_12_2.rs")
    });

    fn font_size_key(font_size: f64) -> u16 {
        if !(font_size.is_finite() && font_size > 0.0) {
            return 0;
        }
        let k = (font_size * 100.0).round();
        if !(k.is_finite() && k >= 0.0 && k <= (u16::MAX as f64)) {
            return 0;
        }
        k as u16
    }

    fn node_has_class_token(node: roxmltree::Node<'_, '_>, token: &str) -> bool {
        node.attribute("class").is_some_and(|c| {
            c.split_whitespace()
                .any(|t| !t.is_empty() && t.trim() == token)
        })
    }

    fn has_ancestor_class_token(node: roxmltree::Node<'_, '_>, token: &str) -> bool {
        let mut cur = Some(node);
        while let Some(n) = cur {
            if n.is_element() && node_has_class_token(n, token) {
                return true;
            }
            cur = n.parent();
        }
        false
    }

    fn parse_max_width_px(style: &str) -> Option<i64> {
        // Keep it strict: we only want the integer `max-width: Npx` that Mermaid emits.
        let s = style;
        let key = "max-width:";
        let idx = s.find(key)?;
        let rest = s[idx + key.len()..].trim_start();
        let mut num = String::new();
        for ch in rest.chars() {
            if ch.is_ascii_digit() {
                num.push(ch);
            } else {
                break;
            }
        }
        if num.is_empty() {
            return None;
        }
        let rest = &rest[num.len()..];
        if !rest.trim_start().starts_with("px") {
            return None;
        }
        num.parse::<i64>().ok()
    }

    // `((font_size_key, text) -> width_px)` and `((font_size_key, text) -> calc_text_width_px)`.
    let mut html_widths: BTreeMap<(u16, String), f64> = BTreeMap::new();
    let mut calc_text_widths: BTreeMap<(u16, String), i64> = BTreeMap::new();

    let mut svg_paths: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&in_dir).map_err(|e| {
        XtaskError::SvgCompareFailed(format!("failed to read dir {}: {}", in_dir.display(), e))
    })? {
        let entry = entry.map_err(|e| {
            XtaskError::SvgCompareFailed(format!(
                "failed to read dir entry {}: {}",
                in_dir.display(),
                e
            ))
        })?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|e| e.to_string_lossy().to_ascii_lowercase() == "svg")
        {
            svg_paths.push(path);
        }
    }
    svg_paths.sort();

    let mut conflicts: BTreeSet<String> = BTreeSet::new();
    for path in svg_paths {
        let svg = std::fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;
        let doc = roxmltree::Document::parse(&svg).map_err(|e| {
            XtaskError::SvgCompareFailed(format!(
                "failed to parse upstream ER SVG {}: {}",
                path.display(),
                e
            ))
        })?;

        for fo in doc
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "foreignObject")
        {
            let Some(w_str) = fo.attribute("width") else {
                continue;
            };
            let Ok(width_px) = w_str.parse::<f64>() else {
                continue;
            };
            if !(width_px.is_finite() && width_px >= 0.0) {
                continue;
            }

            // Mermaid ER labels are single-line in the fixtures we care about, but the HTML
            // structure varies:
            // - Normal labels: `<span class="nodeLabel"><p>TEXT</p></span>`
            // - Generic labels: raw text nodes (e.g. `type&lt;T&gt;`) without nested tags
            //
            // Extract the user-visible string by concatenating text nodes under the inner `<div>`.
            let div = fo
                .descendants()
                .find(|n| n.is_element() && n.tag_name().name() == "div");
            let Some(div) = div else {
                continue;
            };
            let mut text_decoded = String::new();
            for t in div.descendants().filter(|n| n.is_text()) {
                if let Some(s) = t.text() {
                    text_decoded.push_str(s);
                }
            }
            let text_decoded = text_decoded.trim().to_string();
            if text_decoded.is_empty() {
                continue;
            }

            // Mermaid erBox.ts passes a pre-workaround string into `calculateTextWidth()`:
            // generics get replaced from `<`/`>` to `&lt;`/`&gt;` before the call.
            let text_calc_input = if text_decoded.contains('<') || text_decoded.contains('>') {
                text_decoded.replace('<', "&lt;").replace('>', "&gt;")
            } else {
                text_decoded.clone()
            };

            let font_size = if has_ancestor_class_token(fo, "edgeLabel") {
                14.0
            } else {
                16.0
            };
            let fs_key = font_size_key(font_size);
            if fs_key == 0 {
                continue;
            }

            let html_key = (fs_key, text_decoded.clone());
            if let Some(prev) = html_widths.get(&html_key).copied() {
                if (prev - width_px).abs() > 1e-9 {
                    conflicts.insert(format!(
                        "html width conflict for font_size={} text={:?}: {} vs {} (file {})",
                        font_size,
                        text_decoded,
                        prev,
                        width_px,
                        path.display()
                    ));
                }
            } else {
                html_widths.insert(html_key, width_px);
            }

            // Try to derive `calculateTextWidth()` from Mermaid's `createText(..., width=calc+100)`.
            // This shows up as `max-width: <n>px` in the inner div style.
            let max_width_px = div.attribute("style").and_then(parse_max_width_px);

            if let Some(mw) = max_width_px {
                // Edge labels use the flowchart wrapping width (200px) and are not driven by
                // `calculateTextWidth()+100`.
                if mw != 200 && mw >= 100 {
                    let calc_w = mw - 100;
                    let calc_key = (fs_key, text_calc_input);
                    if let Some(prev) = calc_text_widths.get(&calc_key).copied() {
                        if prev != calc_w {
                            conflicts.insert(format!(
                                "calcTextWidth conflict for font_size={} text={:?}: {} vs {} (file {})",
                                font_size,
                                calc_key.1,
                                prev,
                                calc_w,
                                path.display()
                            ));
                        }
                    } else {
                        calc_text_widths.insert(calc_key, calc_w);
                    }
                }
            }
        }
    }

    if !conflicts.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "conflicts while generating ER text overrides:\n{}",
            conflicts.into_iter().collect::<Vec<_>>().join("\n")
        )));
    }

    fn rust_f64(v: f64) -> String {
        // Preserve `1/64` widths exactly when possible (e.g. `78.984375`).
        let mut s = format!("{v}");
        if !s.contains('.') && !s.contains('e') && !s.contains('E') {
            s.push_str(".0");
        }
        s
    }

    let mut out = String::new();
    let _ = writeln!(
        &mut out,
        "// This file is generated by `xtask gen-er-text-overrides`.\n//\n// Mermaid baseline: 11.12.2\n// Source: fixtures/upstream-svgs/er/*.svg\n"
    );
    let _ = writeln!(&mut out, "#[allow(dead_code)]");
    let _ = writeln!(&mut out, "fn font_size_key(font_size: f64) -> u16 {{");
    let _ = writeln!(
        &mut out,
        "    if !(font_size.is_finite() && font_size > 0.0) {{ return 0; }}"
    );
    let _ = writeln!(&mut out, "    let k = (font_size * 100.0).round();");
    let _ = writeln!(
        &mut out,
        "    if !(k.is_finite() && k >= 0.0 && k <= (u16::MAX as f64)) {{ return 0; }}"
    );
    let _ = writeln!(&mut out, "    k as u16");
    let _ = writeln!(&mut out, "}}");
    let _ = writeln!(&mut out);

    let html_entries: Vec<(u16, String, f64)> = html_widths
        .into_iter()
        .map(|((fs, t), w)| (fs, t, w))
        .collect();
    let calc_entries: Vec<(u16, String, i64)> = calc_text_widths
        .into_iter()
        .map(|((fs, t), w)| (fs, t, w))
        .collect();

    let _ = writeln!(
        &mut out,
        "static HTML_WIDTH_OVERRIDES_PX: &[(u16, &str, f64)] = &["
    );
    for (fs, t, w) in &html_entries {
        let _ = writeln!(&mut out, "    ({fs}, {:?}, {}),", t, rust_f64(*w));
    }
    let _ = writeln!(&mut out, "];\n");

    let _ = writeln!(
        &mut out,
        "static CALC_TEXT_WIDTH_OVERRIDES_PX: &[(u16, &str, i64)] = &["
    );
    for (fs, t, w) in &calc_entries {
        let _ = writeln!(&mut out, "    ({fs}, {:?}, {w}),", t);
    }
    let _ = writeln!(&mut out, "];\n");

    let _ = writeln!(
        &mut out,
        "pub fn lookup_html_width_px(font_size: f64, text: &str) -> Option<f64> {{"
    );
    let _ = writeln!(&mut out, "    let fs = font_size_key(font_size);");
    let _ = writeln!(
        &mut out,
        "    if fs == 0 || text.is_empty() {{ return None; }}"
    );
    let _ = writeln!(&mut out, "    let mut lo = 0usize;");
    let _ = writeln!(&mut out, "    let mut hi = HTML_WIDTH_OVERRIDES_PX.len();");
    let _ = writeln!(&mut out, "    while lo < hi {{");
    let _ = writeln!(&mut out, "        let mid = (lo + hi) / 2;");
    let _ = writeln!(
        &mut out,
        "        let (k_fs, k_text, w) = HTML_WIDTH_OVERRIDES_PX[mid];"
    );
    let _ = writeln!(&mut out, "        match k_fs.cmp(&fs) {{");
    let _ = writeln!(&mut out, "            std::cmp::Ordering::Equal => {{");
    let _ = writeln!(&mut out, "                match k_text.cmp(text) {{");
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Equal => return Some(w),"
    );
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Less => lo = mid + 1,"
    );
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Greater => hi = mid,"
    );
    let _ = writeln!(&mut out, "                }}");
    let _ = writeln!(&mut out, "            }}");
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Less => lo = mid + 1,"
    );
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Greater => hi = mid,"
    );
    let _ = writeln!(&mut out, "        }}");
    let _ = writeln!(&mut out, "    }}");
    let _ = writeln!(&mut out, "    None");
    let _ = writeln!(&mut out, "}}\n");

    let _ = writeln!(
        &mut out,
        "pub fn lookup_calc_text_width_px(font_size: f64, text: &str) -> Option<i64> {{"
    );
    let _ = writeln!(&mut out, "    let fs = font_size_key(font_size);");
    let _ = writeln!(
        &mut out,
        "    if fs == 0 || text.is_empty() {{ return None; }}"
    );
    let _ = writeln!(&mut out, "    let mut lo = 0usize;");
    let _ = writeln!(
        &mut out,
        "    let mut hi = CALC_TEXT_WIDTH_OVERRIDES_PX.len();"
    );
    let _ = writeln!(&mut out, "    while lo < hi {{");
    let _ = writeln!(&mut out, "        let mid = (lo + hi) / 2;");
    let _ = writeln!(
        &mut out,
        "        let (k_fs, k_text, w) = CALC_TEXT_WIDTH_OVERRIDES_PX[mid];"
    );
    let _ = writeln!(&mut out, "        match k_fs.cmp(&fs) {{");
    let _ = writeln!(&mut out, "            std::cmp::Ordering::Equal => {{");
    let _ = writeln!(&mut out, "                match k_text.cmp(text) {{");
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Equal => return Some(w),"
    );
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Less => lo = mid + 1,"
    );
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Greater => hi = mid,"
    );
    let _ = writeln!(&mut out, "                }}");
    let _ = writeln!(&mut out, "            }}");
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Less => lo = mid + 1,"
    );
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Greater => hi = mid,"
    );
    let _ = writeln!(&mut out, "        }}");
    let _ = writeln!(&mut out, "    }}");
    let _ = writeln!(&mut out, "    None");
    let _ = writeln!(&mut out, "}}");

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    std::fs::write(&out_path, out).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}

fn compare_svg_xml(args: Vec<String>) -> Result<(), XtaskError> {
    let mut check: bool = false;
    let mut dom_mode: Option<String> = None;
    let mut dom_decimals: Option<u32> = None;
    let mut filter: Option<String> = None;
    let mut text_measurer: Option<String> = None;
    let mut only_diagrams: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--check" | "--check-xml" => check = true,
            "--dom-mode" => {
                i += 1;
                dom_mode = args.get(i).map(|s| s.trim().to_string());
            }
            "--dom-decimals" | "--xml-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.trim().parse::<u32>().ok());
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--text-measurer" => {
                i += 1;
                text_measurer = args.get(i).map(|s| s.trim().to_ascii_lowercase());
            }
            "--diagram" => {
                i += 1;
                let d = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
                if !d.is_empty() {
                    only_diagrams.push(d);
                }
            }
            "--help" | "-h" => {
                return Err(XtaskError::Usage);
            }
            other => {
                return Err(XtaskError::UnknownCommand(format!(
                    "compare-svg-xml: unknown arg `{other}`"
                )));
            }
        }
        i += 1;
    }

    let dom_mode = dom_mode.unwrap_or_else(|| "strict".to_string());
    let dom_decimals = dom_decimals.unwrap_or(3);
    let mode = svgdom::DomMode::parse(&dom_mode);

    let measurer: std::sync::Arc<dyn merman_render::text::TextMeasurer + Send + Sync> =
        match text_measurer.as_deref().unwrap_or("vendored") {
            "deterministic" => {
                std::sync::Arc::new(merman_render::text::DeterministicTextMeasurer::default())
            }
            _ => {
                std::sync::Arc::new(merman_render::text::VendoredFontMetricsTextMeasurer::default())
            }
        };

    // Mermaid gitGraph auto-generates commit ids using `Math.random()`. Upstream gitGraph SVGs in
    // this repo are generated with a seeded upstream renderer, so keep the local side seeded too
    // for meaningful strict XML comparisons.
    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "handDrawnSeed": 1, "gitGraph": { "seed": 1 } }),
    ));
    let layout_opts = merman_render::LayoutOptions {
        text_measurer: std::sync::Arc::clone(&measurer),
        ..Default::default()
    };

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let upstream_root = workspace_root.join("fixtures").join("upstream-svgs");
    let fixtures_root = workspace_root.join("fixtures");
    let out_root = workspace_root.join("target").join("compare").join("xml");

    fn sanitize_svg_id(raw: &str) -> String {
        let mut out = String::with_capacity(raw.len());
        for ch in raw.chars() {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                out.push(ch);
            } else {
                out.push('_');
            }
        }
        out
    }

    fn gantt_upstream_today_x1(svg: &str) -> Option<f64> {
        let doc = roxmltree::Document::parse(svg).ok()?;
        for n in doc.descendants().filter(|n| n.has_tag_name("line")) {
            if !n
                .attribute("class")
                .unwrap_or_default()
                .split_whitespace()
                .any(|t| t == "today")
            {
                continue;
            }
            let x1 = n.attribute("x1")?.parse::<f64>().ok()?;
            if x1.is_finite() {
                return Some(x1);
            }
        }
        None
    }

    fn gantt_derive_now_ms_from_upstream_today(
        upstream_svg: &str,
        layout: &merman_render::model::GanttDiagramLayout,
    ) -> Option<i64> {
        let x1 = gantt_upstream_today_x1(upstream_svg)?;
        let min_ms = layout.tasks.iter().map(|t| t.start_ms).min()?;
        let max_ms = layout.tasks.iter().map(|t| t.end_ms).max()?;
        if max_ms <= min_ms {
            return None;
        }
        let range = (layout.width - layout.left_padding - layout.right_padding).max(1.0);
        let target_x = x1;

        fn gantt_today_x(
            now_ms: i64,
            min_ms: i64,
            max_ms: i64,
            range: f64,
            left_padding: f64,
        ) -> f64 {
            if max_ms <= min_ms {
                return left_padding + (range / 2.0).round();
            }
            let t = (now_ms - min_ms) as f64 / (max_ms - min_ms) as f64;
            left_padding + (t * range).round()
        }

        // Start from a linear estimate, then bracket + binary-search to find a `now_ms` that
        // reproduces the exact upstream `x1` under our `round(t * range)` implementation.
        let span = (max_ms - min_ms) as f64;
        let scaled = target_x - layout.left_padding;
        if !(span.is_finite() && scaled.is_finite() && range.is_finite()) {
            return None;
        }
        let est = (min_ms as f64) + span * (scaled / range);
        if !est.is_finite() {
            return None;
        }
        let mut lo = est.round() as i64;
        let mut hi = lo;
        let mut step: i64 = 1;

        let mut guard = 0;
        while guard < 80 {
            guard += 1;
            let x_lo = gantt_today_x(lo, min_ms, max_ms, range, layout.left_padding);
            if x_lo.is_nan() {
                return None;
            }
            if x_lo <= target_x {
                break;
            }
            hi = lo;
            lo = lo.saturating_sub(step);
            step = step.saturating_mul(2);
        }
        guard = 0;
        step = 1;
        while guard < 80 {
            guard += 1;
            let x_hi = gantt_today_x(hi, min_ms, max_ms, range, layout.left_padding);
            if x_hi.is_nan() {
                return None;
            }
            if x_hi >= target_x {
                break;
            }
            lo = hi;
            hi = hi.saturating_add(step);
            step = step.saturating_mul(2);
        }

        // If we failed to bracket, bail.
        let x_lo = gantt_today_x(lo, min_ms, max_ms, range, layout.left_padding);
        let x_hi = gantt_today_x(hi, min_ms, max_ms, range, layout.left_padding);
        if !(x_lo <= target_x && target_x <= x_hi) {
            return None;
        }

        // Lower-bound search: first `now_ms` where x >= target.
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let x_mid = gantt_today_x(mid, min_ms, max_ms, range, layout.left_padding);
            if x_mid < target_x {
                lo = mid.saturating_add(1);
            } else {
                hi = mid;
            }
        }
        let x = gantt_today_x(lo, min_ms, max_ms, range, layout.left_padding);
        if x == target_x { Some(lo) } else { None }
    }

    let mut diagrams: Vec<String> = Vec::new();
    let Ok(entries) = fs::read_dir(&upstream_root) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list upstream svg directory {}",
            upstream_root.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !only_diagrams.is_empty() && !only_diagrams.iter().any(|d| d == name) {
            continue;
        }
        diagrams.push(name.to_string());
    }
    diagrams.sort();

    if diagrams.is_empty() {
        return Err(XtaskError::SvgCompareFailed(
            "no diagram directories matched under fixtures/upstream-svgs".to_string(),
        ));
    }

    let mut mismatches: Vec<(String, String, PathBuf, PathBuf)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    for diagram in diagrams {
        let upstream_dir = upstream_root.join(&diagram);
        let fixtures_dir = fixtures_root.join(&diagram);
        let out_dir = out_root.join(&diagram);
        fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
            path: out_dir.display().to_string(),
            source,
        })?;

        let Ok(svg_entries) = fs::read_dir(&upstream_dir) else {
            missing.push(format!(
                "{diagram}: failed to list {}",
                upstream_dir.display()
            ));
            continue;
        };

        let mut upstream_svgs: Vec<PathBuf> = Vec::new();
        for entry in svg_entries.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            if !has_extension(&p, "svg") {
                continue;
            }
            if let Some(ref f) = filter {
                if !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains(f))
                {
                    continue;
                }
            }
            upstream_svgs.push(p);
        }
        upstream_svgs.sort();

        for upstream_path in upstream_svgs {
            let Some(stem) = upstream_path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let fixture_path = fixtures_dir.join(format!("{stem}.mmd"));
            let text = match fs::read_to_string(&fixture_path) {
                Ok(v) => v,
                Err(err) => {
                    missing.push(format!(
                        "{diagram}/{stem}: missing fixture {} ({err})",
                        fixture_path.display()
                    ));
                    continue;
                }
            };
            let upstream_svg = match fs::read_to_string(&upstream_path) {
                Ok(v) => v,
                Err(err) => {
                    missing.push(format!(
                        "{diagram}/{stem}: failed to read upstream svg {} ({err})",
                        upstream_path.display()
                    ));
                    continue;
                }
            };

            let parsed = match futures::executor::block_on(engine.parse_diagram(
                &text,
                merman::ParseOptions {
                    suppress_errors: true,
                },
            )) {
                Ok(Some(v)) => v,
                Ok(None) => {
                    missing.push(format!("{diagram}/{stem}: no diagram detected"));
                    continue;
                }
                Err(err) => {
                    missing.push(format!("{diagram}/{stem}: parse failed: {err}"));
                    continue;
                }
            };

            let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
                Ok(v) => v,
                Err(err) => {
                    missing.push(format!("{diagram}/{stem}: layout failed: {err}"));
                    continue;
                }
            };

            let mut svg_opts = merman_render::svg::SvgRenderOptions {
                diagram_id: Some(sanitize_svg_id(stem)),
                ..Default::default()
            };
            if diagram == "gantt" {
                if let merman_render::model::LayoutDiagram::GanttDiagram(layout) = &layouted.layout
                {
                    svg_opts.now_ms_override =
                        gantt_derive_now_ms_from_upstream_today(&upstream_svg, layout);
                }
            }
            let local_svg = match merman_render::svg::render_layouted_svg(
                &layouted,
                layout_opts.text_measurer.as_ref(),
                &svg_opts,
            ) {
                Ok(v) => v,
                Err(err) => {
                    missing.push(format!("{diagram}/{stem}: render failed: {err}"));
                    continue;
                }
            };

            let upstream_xml = match svgdom::canonical_xml(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    missing.push(format!(
                        "{diagram}/{stem}: upstream xml parse failed: {err}"
                    ));
                    continue;
                }
            };
            let local_xml = match svgdom::canonical_xml(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    missing.push(format!("{diagram}/{stem}: local xml parse failed: {err}"));
                    continue;
                }
            };

            if upstream_xml != local_xml {
                let upstream_out = out_dir.join(format!("{stem}.upstream.xml"));
                let local_out = out_dir.join(format!("{stem}.local.xml"));
                fs::write(&upstream_out, upstream_xml).map_err(|source| XtaskError::WriteFile {
                    path: upstream_out.display().to_string(),
                    source,
                })?;
                fs::write(&local_out, local_xml).map_err(|source| XtaskError::WriteFile {
                    path: local_out.display().to_string(),
                    source,
                })?;

                mismatches.push((diagram.clone(), stem.to_string(), upstream_out, local_out));
            }
        }
    }

    let report_path = out_root.join("xml_report.md");
    let mut report = String::new();
    let _ = writeln!(&mut report, "# SVG Canonical XML Compare Report");
    let _ = writeln!(&mut report);
    let _ = writeln!(
        &mut report,
        "- Mode: `{}`",
        match mode {
            svgdom::DomMode::Strict => "strict",
            svgdom::DomMode::Structure => "structure",
            svgdom::DomMode::Parity => "parity",
            svgdom::DomMode::ParityRoot => "parity-root",
        }
    );
    let _ = writeln!(&mut report, "- Decimals: `{dom_decimals}`");
    let _ = writeln!(
        &mut report,
        "- Output: `target/compare/xml/<diagram>/<fixture>.(upstream|local).xml`"
    );
    let _ = writeln!(&mut report);
    let _ = writeln!(&mut report, "## Mismatches ({})", mismatches.len());
    let _ = writeln!(&mut report);
    for (diagram, stem, upstream_out, local_out) in &mismatches {
        let _ = writeln!(
            &mut report,
            "- `{diagram}/{stem}`: `{}` vs `{}`",
            upstream_out.display(),
            local_out.display()
        );
    }
    if !missing.is_empty() {
        let _ = writeln!(&mut report);
        let _ = writeln!(&mut report, "## Missing / Failed ({})", missing.len());
        let _ = writeln!(&mut report);
        for m in &missing {
            let _ = writeln!(&mut report, "- {m}");
        }
    }

    fs::write(&report_path, report).map_err(|source| XtaskError::WriteFile {
        path: report_path.display().to_string(),
        source,
    })?;

    if check && (!mismatches.is_empty() || !missing.is_empty()) {
        let mut msg = String::new();
        let _ = writeln!(
            &mut msg,
            "canonical XML mismatches: {} (mode={dom_mode}, decimals={dom_decimals})",
            mismatches.len()
        );
        if !mismatches.is_empty() {
            for (diagram, stem, upstream_out, local_out) in mismatches.iter().take(20) {
                let _ = writeln!(
                    &mut msg,
                    "- {diagram}/{stem}: {} vs {}",
                    upstream_out.display(),
                    local_out.display()
                );
            }
            if mismatches.len() > 20 {
                let _ = writeln!(&mut msg, "- … ({} more)", mismatches.len() - 20);
            }
        }
        if !missing.is_empty() {
            let _ = writeln!(&mut msg, "missing/failed cases: {}", missing.len());
            for m in missing.iter().take(20) {
                let _ = writeln!(&mut msg, "- {m}");
            }
            if missing.len() > 20 {
                let _ = writeln!(&mut msg, "- … ({} more)", missing.len() - 20);
            }
        }
        let _ = writeln!(&mut msg, "report: {}", report_path.display());
        return Err(XtaskError::SvgCompareFailed(msg));
    }

    println!("wrote report: {}", report_path.display());
    Ok(())
}

fn canon_svg_xml(args: Vec<String>) -> Result<(), XtaskError> {
    let mut in_path: Option<PathBuf> = None;
    let mut dom_mode: Option<String> = None;
    let mut dom_decimals: Option<u32> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--in" => {
                i += 1;
                in_path = args.get(i).map(PathBuf::from);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args.get(i).map(|s| s.trim().to_string());
            }
            "--dom-decimals" | "--xml-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.trim().parse::<u32>().ok());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let in_path = in_path.ok_or(XtaskError::Usage)?;
    let svg = fs::read_to_string(&in_path).map_err(|source| XtaskError::ReadFile {
        path: in_path.display().to_string(),
        source,
    })?;
    let mode = svgdom::DomMode::parse(dom_mode.as_deref().unwrap_or("strict"));
    let decimals = dom_decimals.unwrap_or(3);

    let xml = svgdom::canonical_xml(&svg, mode, decimals).map_err(XtaskError::SvgCompareFailed)?;
    print!("{xml}");
    Ok(())
}

fn svg_compare_layout_opts() -> merman_render::LayoutOptions {
    merman_render::LayoutOptions {
        text_measurer: std::sync::Arc::new(
            merman_render::text::VendoredFontMetricsTextMeasurer::default(),
        ),
        use_manatee_layout: true,
        ..Default::default()
    }
}

fn compare_all_svgs(args: Vec<String>) -> Result<(), XtaskError> {
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

fn debug_svg_bbox(args: Vec<String>) -> Result<(), XtaskError> {
    let mut svg_path: Option<PathBuf> = None;
    let mut padding: f64 = 8.0;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--svg" => {
                i += 1;
                svg_path = args.get(i).map(PathBuf::from);
            }
            "--padding" => {
                i += 1;
                padding = args
                    .get(i)
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(8.0);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let svg_path = svg_path.ok_or(XtaskError::Usage)?;
    let svg = fs::read_to_string(&svg_path).map_err(|source| XtaskError::ReadFile {
        path: svg_path.display().to_string(),
        source,
    })?;

    let dbg = merman_render::svg::debug_svg_emitted_bounds(&svg).ok_or_else(|| {
        XtaskError::DebugSvgFailed(format!(
            "failed to compute emitted bounds for {}",
            svg_path.display()
        ))
    })?;

    let b = dbg.bounds;
    let vb_min_x = b.min_x - padding;
    let vb_min_y = b.min_y - padding;
    let vb_w = (b.max_x - b.min_x) + 2.0 * padding;
    let vb_h = (b.max_y - b.min_y) + 2.0 * padding;

    println!("svg: {}", svg_path.display());
    println!(
        "bounds: min=({:.6},{:.6}) max=({:.6},{:.6})",
        b.min_x, b.min_y, b.max_x, b.max_y
    );
    println!(
        "viewBox (padding={:.3}): {:.6} {:.6} {:.6} {:.6}",
        padding, vb_min_x, vb_min_y, vb_w, vb_h
    );
    println!("style max-width: {:.6}px", vb_w);

    fn print_contrib(label: &str, c: &Option<merman_render::svg::SvgEmittedBoundsContributor>) {
        let Some(c) = c else {
            println!("{label}: <none>");
            return;
        };
        fn clip_attr(s: &str) -> String {
            const MAX: usize = 140;
            if s.len() <= MAX {
                return s.to_string();
            }
            let mut out = s.chars().take(MAX).collect::<String>();
            out.push('…');
            out
        }

        println!(
            "{label}: <{} id={:?} class={:?}> bbox=({:.6},{:.6})-({:.6},{:.6})",
            c.tag, c.id, c.class, c.bounds.min_x, c.bounds.min_y, c.bounds.max_x, c.bounds.max_y
        );
        if let Some(d) = c.d.as_deref() {
            println!("  d={}", clip_attr(d));
        }
        if let Some(points) = c.points.as_deref() {
            println!("  points={}", clip_attr(points));
        }
        if let Some(tf) = c.transform.as_deref() {
            println!("  transform={}", clip_attr(tf));
        }
    }

    print_contrib("min_x", &dbg.min_x);
    print_contrib("min_y", &dbg.min_y);
    print_contrib("max_x", &dbg.max_x);
    print_contrib("max_y", &dbg.max_y);

    Ok(())
}

fn debug_svg_data_points(args: Vec<String>) -> Result<(), XtaskError> {
    #[derive(Debug, Clone, Copy, serde::Deserialize)]
    struct Point {
        x: f64,
        y: f64,
    }

    use base64::Engine as _;

    fn decode_points(svg: &str, element_id: &str) -> Result<Vec<Point>, XtaskError> {
        let doc = roxmltree::Document::parse(svg)
            .map_err(|e| XtaskError::SvgCompareFailed(format!("failed to parse svg xml: {e}")))?;
        let node = doc
            .descendants()
            .find(|n| n.is_element() && n.attribute("id") == Some(element_id))
            .ok_or_else(|| {
                XtaskError::DebugSvgFailed(format!("missing element with id={element_id:?}"))
            })?;
        let b64 = node.attribute("data-points").ok_or_else(|| {
            XtaskError::DebugSvgFailed(format!(
                "element id={element_id:?} has no `data-points` attribute"
            ))
        })?;

        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64.as_bytes())
            .map_err(|e| XtaskError::DebugSvgFailed(format!("invalid base64 data-points: {e}")))?;
        let pts: Vec<Point> = serde_json::from_slice(&bytes).map_err(|e| {
            XtaskError::DebugSvgFailed(format!("invalid JSON data-points payload: {e}"))
        })?;
        Ok(pts)
    }

    let mut svg_path: Option<PathBuf> = None;
    let mut other_svg_path: Option<PathBuf> = None;
    let mut element_id: Option<String> = None;
    let mut decimals: usize = 3;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--svg" => {
                i += 1;
                svg_path = args.get(i).map(PathBuf::from);
            }
            "--other" => {
                i += 1;
                other_svg_path = args.get(i).map(PathBuf::from);
            }
            "--id" => {
                i += 1;
                element_id = args.get(i).map(|s| s.to_string());
            }
            "--decimals" => {
                i += 1;
                decimals = args
                    .get(i)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(3);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let svg_path = svg_path.ok_or(XtaskError::Usage)?;
    let element_id = element_id.ok_or(XtaskError::Usage)?;

    let svg = fs::read_to_string(&svg_path).map_err(|source| XtaskError::ReadFile {
        path: svg_path.display().to_string(),
        source,
    })?;
    let points = decode_points(&svg, &element_id)?;

    println!("svg: {}", svg_path.display());
    println!("id: {element_id}");
    println!("points: {}", points.len());
    for (idx, p) in points.iter().enumerate() {
        println!(
            "  {idx:>3}: {x:.d$}, {y:.d$}",
            x = p.x,
            y = p.y,
            d = decimals
        );
    }

    let Some(other_svg_path) = other_svg_path else {
        return Ok(());
    };

    let other_svg = fs::read_to_string(&other_svg_path).map_err(|source| XtaskError::ReadFile {
        path: other_svg_path.display().to_string(),
        source,
    })?;
    let other_points = decode_points(&other_svg, &element_id)?;

    println!("\nother: {}", other_svg_path.display());
    println!("points: {}", other_points.len());
    if points.len() != other_points.len() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "point count mismatch: {} vs {}",
            points.len(),
            other_points.len()
        )));
    }

    println!("\ndelta (other - svg):");
    for (idx, (a, b)) in points.iter().zip(other_points.iter()).enumerate() {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        println!(
            "  {idx:>3}: dx={dx:.d$} dy={dy:.d$}",
            dx = dx,
            dy = dy,
            d = decimals
        );
    }

    Ok(())
}

fn debug_architecture_delta(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<String> = None;
    let mut out_dir: Option<PathBuf> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.trim().to_string());
            }
            "--out" => {
                i += 1;
                out_dir = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let fixture = fixture.ok_or(XtaskError::Usage)?;
    if fixture.trim().is_empty() {
        return Err(XtaskError::Usage);
    }

    fn parse_viewbox(v: &str) -> Option<(f64, f64, f64, f64)> {
        let nums: Vec<f64> = v
            .split_whitespace()
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect();
        if nums.len() != 4 {
            return None;
        }
        Some((nums[0], nums[1], nums[2], nums[3]))
    }

    fn parse_translate(transform: &str) -> Option<(f64, f64)> {
        // Mermaid emits `translate(x,y)` or `translate(x, y)` in Architecture outputs.
        let s = transform.trim();
        let s = s.strip_prefix("translate(")?;
        let s = s.strip_suffix(')')?;
        let parts: Vec<&str> = s
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|t: &&str| !t.trim().is_empty())
            .collect();
        let x = parts.first()?.trim().parse::<f64>().ok()?;
        let y = parts
            .get(1)
            .copied()
            .and_then(|v| v.trim().parse::<f64>().ok())?;
        Some((x, y))
    }

    fn parse_max_width_px(style: &str) -> Option<f64> {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| Regex::new(r#"max-width:\s*([0-9.]+)px"#).unwrap());
        let cap = re.captures(style)?;
        cap.get(1)?.as_str().trim().parse::<f64>().ok()
    }

    fn has_class_token(class: &str, token: &str) -> bool {
        class.split_whitespace().any(|t| t == token)
    }

    fn sanitize_svg_id(stem: &str) -> String {
        stem.chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect()
    }

    #[derive(Debug, Clone, Copy)]
    struct Pt {
        x: f64,
        y: f64,
    }

    #[derive(Debug, Clone, Copy)]
    struct Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }

    type ArchPositions = (
        Option<(f64, f64, f64, f64)>,
        Option<f64>,
        BTreeMap<String, Pt>,
        BTreeMap<String, Pt>,
        BTreeMap<String, Rect>,
    );

    fn extract_arch_positions(svg: &str) -> Result<ArchPositions, XtaskError> {
        let doc = roxmltree::Document::parse(svg)
            .map_err(|e| XtaskError::SvgCompareFailed(format!("failed to parse svg xml: {e}")))?;
        let root = doc.root_element();
        let viewbox = root.attribute("viewBox").and_then(parse_viewbox);
        let max_width = root.attribute("style").and_then(parse_max_width_px);

        let mut services: BTreeMap<String, Pt> = BTreeMap::new();
        let mut junctions: BTreeMap<String, Pt> = BTreeMap::new();
        let mut groups: BTreeMap<String, Rect> = BTreeMap::new();

        for n in doc.descendants().filter(|n| n.is_element()) {
            let tag = n.tag_name().name();
            let Some(id) = n.attribute("id") else {
                continue;
            };

            if tag == "g"
                && id.starts_with("service-")
                && n.attribute("class")
                    .is_some_and(|c| has_class_token(c, "architecture-service"))
            {
                if let Some((x, y)) = n.attribute("transform").and_then(parse_translate) {
                    services.insert(id.to_string(), Pt { x, y });
                }
            }

            if tag == "g"
                && id.starts_with("junction-")
                && n.attribute("class")
                    .is_some_and(|c| has_class_token(c, "architecture-junction"))
            {
                if let Some((x, y)) = n.attribute("transform").and_then(parse_translate) {
                    junctions.insert(id.to_string(), Pt { x, y });
                }
            }

            if tag == "rect" && id.starts_with("group-") {
                let x = n
                    .attribute("x")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let y = n
                    .attribute("y")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let w = n
                    .attribute("width")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let h = n
                    .attribute("height")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                groups.insert(id.to_string(), Rect { x, y, w, h });
            }
        }

        Ok((viewbox, max_width, services, junctions, groups))
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("architecture");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("architecture");
    let out_dir = out_dir.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("debug")
            .join("architecture-delta")
    });

    let mut candidates: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_file_with_extension(&path, "mmd") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.contains("_parser_only_") || name.contains("_parser_only_spec") {
            continue;
        }
        if name.contains(&fixture) {
            candidates.push(path);
        }
    }
    candidates.sort();

    let mmd_path = match candidates.len() {
        0 => {
            return Err(XtaskError::SvgCompareFailed(format!(
                "no Architecture fixture matched {fixture:?} under {}",
                fixtures_dir.display()
            )));
        }
        1 => candidates[0].clone(),
        _ => {
            let list = candidates
                .iter()
                .take(20)
                .map(|p| format!("- {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(XtaskError::SvgCompareFailed(format!(
                "multiple Architecture fixtures matched {fixture:?}; please be more specific:\n{list}"
            )));
        }
    };

    let stem = mmd_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| {
            XtaskError::SvgCompareFailed(format!("invalid fixture filename {}", mmd_path.display()))
        })?
        .to_string();

    let diagram_id = sanitize_svg_id(&stem);

    let upstream_path = upstream_dir.join(format!("{stem}.svg"));
    let upstream_svg =
        fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
            path: upstream_path.display().to_string(),
            source,
        })?;

    let text = fs::read_to_string(&mmd_path).map_err(|source| XtaskError::ReadFile {
        path: mmd_path.display().to_string(),
        source,
    })?;

    let engine = merman::Engine::new();
    let parsed =
        futures::executor::block_on(engine.parse_diagram(&text, merman::ParseOptions::default()))
            .map_err(|e| {
                XtaskError::SvgCompareFailed(format!(
                    "parse failed for {}: {e}",
                    mmd_path.display()
                ))
            })?
            .ok_or_else(|| {
                XtaskError::SvgCompareFailed(format!(
                    "no diagram detected in {}",
                    mmd_path.display()
                ))
            })?;

    let layout_opts = svg_compare_layout_opts();
    let layouted = merman_render::layout_parsed(&parsed, &layout_opts).map_err(|e| {
        XtaskError::SvgCompareFailed(format!("layout failed for {}: {e}", mmd_path.display()))
    })?;

    let merman_render::model::LayoutDiagram::ArchitectureDiagram(layout) = &layouted.layout else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "unexpected layout type for {}: {}",
            mmd_path.display(),
            layouted.meta.diagram_type
        )));
    };

    let svg_opts = merman_render::svg::SvgRenderOptions {
        diagram_id: Some(diagram_id),
        ..Default::default()
    };
    let local_svg = merman_render::svg::render_architecture_diagram_svg(
        layout,
        &layouted.semantic,
        &layouted.meta.effective_config,
        &svg_opts,
    )
    .map_err(|e| {
        XtaskError::SvgCompareFailed(format!("render failed for {}: {e}", mmd_path.display()))
    })?;

    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let out_upstream_svg = out_dir.join(format!("{stem}.upstream.svg"));
    let out_local_svg = out_dir.join(format!("{stem}.local.svg"));
    let out_report = out_dir.join(format!("{stem}.md"));
    fs::write(&out_upstream_svg, &upstream_svg).map_err(|source| XtaskError::WriteFile {
        path: out_upstream_svg.display().to_string(),
        source,
    })?;
    fs::write(&out_local_svg, &local_svg).map_err(|source| XtaskError::WriteFile {
        path: out_local_svg.display().to_string(),
        source,
    })?;

    let (up_vb, up_mw, up_services, up_junctions, up_groups) =
        extract_arch_positions(&upstream_svg)?;
    let (lo_vb, lo_mw, lo_services, lo_junctions, lo_groups) = extract_arch_positions(&local_svg)?;

    #[derive(Debug, Clone)]
    struct DeltaRow {
        id: String,
        kind: &'static str,
        up: String,
        lo: String,
        dx: f64,
        dy: f64,
        score: f64,
    }

    let mut deltas: Vec<DeltaRow> = Vec::new();

    fn split_missing<T>(
        upstream: &BTreeMap<String, T>,
        local: &BTreeMap<String, T>,
    ) -> (Vec<String>, Vec<String>) {
        let mut only_up: Vec<String> = upstream
            .keys()
            .filter(|id| !local.contains_key(*id))
            .cloned()
            .collect();
        let mut only_lo: Vec<String> = local
            .keys()
            .filter(|id| !upstream.contains_key(*id))
            .cloned()
            .collect();
        only_up.sort();
        only_lo.sort();
        (only_up, only_lo)
    }

    let (missing_services_in_local, missing_services_in_upstream) =
        split_missing(&up_services, &lo_services);
    let (missing_junctions_in_local, missing_junctions_in_upstream) =
        split_missing(&up_junctions, &lo_junctions);
    let (missing_groups_in_local, missing_groups_in_upstream) =
        split_missing(&up_groups, &lo_groups);

    for (id, up) in &up_services {
        let Some(lo) = lo_services.get(id).copied() else {
            continue;
        };
        let dx = lo.x - up.x;
        let dy = lo.y - up.y;
        deltas.push(DeltaRow {
            id: id.to_string(),
            kind: "service",
            up: format!("translate({:.6},{:.6})", up.x, up.y),
            lo: format!("translate({:.6},{:.6})", lo.x, lo.y),
            dx,
            dy,
            score: dx.abs().max(dy.abs()),
        });
    }

    for (id, up) in &up_junctions {
        let Some(lo) = lo_junctions.get(id).copied() else {
            continue;
        };
        let dx = lo.x - up.x;
        let dy = lo.y - up.y;
        deltas.push(DeltaRow {
            id: id.to_string(),
            kind: "junction",
            up: format!("translate({:.6},{:.6})", up.x, up.y),
            lo: format!("translate({:.6},{:.6})", lo.x, lo.y),
            dx,
            dy,
            score: dx.abs().max(dy.abs()),
        });
    }

    for (id, up) in &up_groups {
        let Some(lo) = lo_groups.get(id).copied() else {
            continue;
        };
        let dx = lo.x - up.x;
        let dy = lo.y - up.y;
        deltas.push(DeltaRow {
            id: id.to_string(),
            kind: "group-rect",
            up: format!("x={:.6} y={:.6} w={:.6} h={:.6}", up.x, up.y, up.w, up.h),
            lo: format!("x={:.6} y={:.6} w={:.6} h={:.6}", lo.x, lo.y, lo.w, lo.h),
            dx,
            dy,
            score: dx.abs().max(dy.abs()),
        });
    }

    deltas.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut report = String::new();
    let _ = writeln!(&mut report, "# Architecture Delta Report\n");
    let _ = writeln!(
        &mut report,
        "- Fixture: `{}`\n- Upstream SVG: `{}`\n- Local SVG: `{}`\n",
        stem,
        out_upstream_svg.display(),
        out_local_svg.display()
    );

    let _ = writeln!(&mut report, "## Root viewport\n");
    let _ = writeln!(
        &mut report,
        "- upstream viewBox: `{}`",
        up_vb
            .map(|v| format!("{:.6} {:.6} {:.6} {:.6}", v.0, v.1, v.2, v.3))
            .unwrap_or_else(|| "<missing>".to_string())
    );
    let _ = writeln!(
        &mut report,
        "- local viewBox: `{}`",
        lo_vb
            .map(|v| format!("{:.6} {:.6} {:.6} {:.6}", v.0, v.1, v.2, v.3))
            .unwrap_or_else(|| "<missing>".to_string())
    );
    let _ = writeln!(
        &mut report,
        "- upstream max-width(px): `{}`",
        up_mw
            .map(|v| format!("{:.6}", v))
            .unwrap_or_else(|| "<missing>".to_string())
    );
    let _ = writeln!(
        &mut report,
        "- local max-width(px): `{}`\n",
        lo_mw
            .map(|v| format!("{:.6}", v))
            .unwrap_or_else(|| "<missing>".to_string())
    );

    let _ = writeln!(&mut report, "## Missing elements\n");
    let _ = writeln!(
        &mut report,
        "- services missing in local: `{}`",
        if missing_services_in_local.is_empty() {
            "<none>".to_string()
        } else {
            missing_services_in_local.join(", ")
        }
    );
    let _ = writeln!(
        &mut report,
        "- services missing in upstream: `{}`",
        if missing_services_in_upstream.is_empty() {
            "<none>".to_string()
        } else {
            missing_services_in_upstream.join(", ")
        }
    );
    let _ = writeln!(
        &mut report,
        "- junctions missing in local: `{}`",
        if missing_junctions_in_local.is_empty() {
            "<none>".to_string()
        } else {
            missing_junctions_in_local.join(", ")
        }
    );
    let _ = writeln!(
        &mut report,
        "- junctions missing in upstream: `{}`",
        if missing_junctions_in_upstream.is_empty() {
            "<none>".to_string()
        } else {
            missing_junctions_in_upstream.join(", ")
        }
    );
    let _ = writeln!(
        &mut report,
        "- group rects missing in local: `{}`",
        if missing_groups_in_local.is_empty() {
            "<none>".to_string()
        } else {
            missing_groups_in_local.join(", ")
        }
    );
    let _ = writeln!(
        &mut report,
        "- group rects missing in upstream: `{}`\n",
        if missing_groups_in_upstream.is_empty() {
            "<none>".to_string()
        } else {
            missing_groups_in_upstream.join(", ")
        }
    );

    let _ = writeln!(
        &mut report,
        "## Element deltas (top 50 by max(abs(dx), abs(dy)))\n"
    );
    let _ = writeln!(
        &mut report,
        "| kind | id | upstream | local | dx | dy | score |\n|---|---|---|---|---:|---:|---:|"
    );
    for row in deltas.iter().take(50) {
        let _ = writeln!(
            &mut report,
            "| {} | `{}` | `{}` | `{}` | {:.6} | {:.6} | {:.6} |",
            row.kind, row.id, row.up, row.lo, row.dx, row.dy, row.score
        );
    }

    fs::write(&out_report, &report).map_err(|source| XtaskError::WriteFile {
        path: out_report.display().to_string(),
        source,
    })?;

    println!("fixture: {stem}");
    println!("upstream: {}", upstream_path.display());
    println!("local:    {}", out_local_svg.display());
    println!("report:   {}", out_report.display());
    if let (Some(up), Some(lo)) = (up_vb, lo_vb) {
        println!(
            "root viewBox: upstream=({:.6},{:.6},{:.6},{:.6}) local=({:.6},{:.6},{:.6},{:.6})",
            up.0, up.1, up.2, up.3, lo.0, lo.1, lo.2, lo.3
        );
    }
    if let (Some(up), Some(lo)) = (up_mw, lo_mw) {
        println!("max-width(px): upstream={:.6} local={:.6}", up, lo);
    }
    println!(
        "elements: services={} junctions={} group_rects={}",
        up_services.len().min(lo_services.len()),
        up_junctions.len().min(lo_junctions.len()),
        up_groups.len().min(lo_groups.len())
    );

    Ok(())
}

fn summarize_architecture_deltas(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_dir: Option<PathBuf> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_dir = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    fn parse_viewbox(v: &str) -> Option<(f64, f64, f64, f64)> {
        let nums: Vec<f64> = v
            .split_whitespace()
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect();
        if nums.len() != 4 {
            return None;
        }
        Some((nums[0], nums[1], nums[2], nums[3]))
    }

    fn parse_translate(transform: &str) -> Option<(f64, f64)> {
        let s = transform.trim();
        let s = s.strip_prefix("translate(")?;
        let s = s.strip_suffix(')')?;
        let parts: Vec<&str> = s
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|t: &&str| !t.trim().is_empty())
            .collect();
        let x = parts.first()?.trim().parse::<f64>().ok()?;
        let y = parts
            .get(1)
            .copied()
            .and_then(|v| v.trim().parse::<f64>().ok())?;
        Some((x, y))
    }

    fn parse_max_width_px(style: &str) -> Option<f64> {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| Regex::new(r#"max-width:\s*([0-9.]+)px"#).unwrap());
        let cap = re.captures(style)?;
        cap.get(1)?.as_str().trim().parse::<f64>().ok()
    }

    fn has_class_token(class: &str, token: &str) -> bool {
        class.split_whitespace().any(|t| t == token)
    }

    fn sanitize_svg_id(stem: &str) -> String {
        stem.chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect()
    }

    #[derive(Debug, Clone, Copy)]
    struct Pt {
        x: f64,
        y: f64,
    }

    type ArchSummary = (
        Option<(f64, f64, f64, f64)>,
        Option<f64>,
        BTreeMap<String, Pt>,
        BTreeMap<String, Pt>,
    );

    fn extract_arch_summary(svg: &str) -> Result<ArchSummary, XtaskError> {
        let doc = roxmltree::Document::parse(svg)
            .map_err(|e| XtaskError::SvgCompareFailed(format!("failed to parse svg xml: {e}")))?;
        let root = doc.root_element();
        let viewbox = root.attribute("viewBox").and_then(parse_viewbox);
        let max_width = root.attribute("style").and_then(parse_max_width_px);

        let mut services: BTreeMap<String, Pt> = BTreeMap::new();
        let mut junctions: BTreeMap<String, Pt> = BTreeMap::new();

        for n in doc.descendants().filter(|n| n.is_element()) {
            let tag = n.tag_name().name();
            let Some(id) = n.attribute("id") else {
                continue;
            };

            if tag == "g"
                && id.starts_with("service-")
                && n.attribute("class")
                    .is_some_and(|c| has_class_token(c, "architecture-service"))
            {
                if let Some((x, y)) = n.attribute("transform").and_then(parse_translate) {
                    services.insert(id.to_string(), Pt { x, y });
                }
            }

            if tag == "g"
                && id.starts_with("junction-")
                && n.attribute("class")
                    .is_some_and(|c| has_class_token(c, "architecture-junction"))
            {
                if let Some((x, y)) = n.attribute("transform").and_then(parse_translate) {
                    junctions.insert(id.to_string(), Pt { x, y });
                }
            }
        }

        Ok((viewbox, max_width, services, junctions))
    }

    fn bbox_center_from_top_left_pts(pts: impl Iterator<Item = Pt>, size: f64) -> Option<Pt> {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        let mut any = false;
        for p in pts {
            any = true;
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x + size);
            max_y = max_y.max(p.y + size);
        }
        if !any {
            return None;
        }
        Some(Pt {
            x: (min_x + max_x) / 2.0,
            y: (min_y + max_y) / 2.0,
        })
    }

    fn mean_delta_by_id(up: &BTreeMap<String, Pt>, lo: &BTreeMap<String, Pt>) -> Option<Pt> {
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut n = 0usize;
        for (id, up_p) in up {
            let Some(lo_p) = lo.get(id) else {
                continue;
            };
            sum_x += lo_p.x - up_p.x;
            sum_y += lo_p.y - up_p.y;
            n += 1;
        }
        if n == 0 {
            return None;
        }
        Some(Pt {
            x: sum_x / (n as f64),
            y: sum_y / (n as f64),
        })
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("architecture");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("architecture");
    let out_dir = out_dir.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("debug")
            .join("architecture-delta")
    });

    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let mut fixtures: Vec<PathBuf> = Vec::new();
    let entries = fs::read_dir(&fixtures_dir).map_err(|e| {
        XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}: {e}",
            fixtures_dir.display()
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| {
            XtaskError::SvgCompareFailed(format!(
                "failed to read fixtures directory {}: {e}",
                fixtures_dir.display()
            ))
        })?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "mmd") {
            fixtures.push(path);
        }
    }
    fixtures.sort();

    let engine = merman::Engine::new();
    let layout_opts = svg_compare_layout_opts();

    #[derive(Debug, Clone)]
    struct Row {
        stem: String,
        up_vb: Option<(f64, f64, f64, f64)>,
        lo_vb: Option<(f64, f64, f64, f64)>,
        up_mw: Option<f64>,
        lo_mw: Option<f64>,
        service_center_dx: Option<f64>,
        service_center_dy: Option<f64>,
        service_mean_dx: Option<f64>,
        service_mean_dy: Option<f64>,
        junction_mean_dx: Option<f64>,
        junction_mean_dy: Option<f64>,
    }

    let mut rows: Vec<Row> = Vec::new();

    for mmd_path in fixtures {
        let Some(stem) = mmd_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
        else {
            continue;
        };

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        if !upstream_path.is_file() {
            continue;
        }

        let upstream_svg =
            fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
                path: upstream_path.display().to_string(),
                source,
            })?;

        let text = fs::read_to_string(&mmd_path).map_err(|source| XtaskError::ReadFile {
            path: mmd_path.display().to_string(),
            source,
        })?;

        let parsed = futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        )
        .map_err(|e| {
            XtaskError::SvgCompareFailed(format!("parse failed for {}: {e}", mmd_path.display()))
        })?
        .ok_or_else(|| {
            XtaskError::SvgCompareFailed(format!("no diagram detected in {}", mmd_path.display()))
        })?;

        let layouted = merman_render::layout_parsed(&parsed, &layout_opts).map_err(|e| {
            XtaskError::SvgCompareFailed(format!("layout failed for {}: {e}", mmd_path.display()))
        })?;

        let merman_render::model::LayoutDiagram::ArchitectureDiagram(layout) = &layouted.layout
        else {
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(sanitize_svg_id(&stem)),
            ..Default::default()
        };
        let local_svg = merman_render::svg::render_architecture_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            &svg_opts,
        )
        .map_err(|e| {
            XtaskError::SvgCompareFailed(format!("render failed for {}: {e}", mmd_path.display()))
        })?;

        let (up_vb, up_mw, up_services, up_junctions) = extract_arch_summary(&upstream_svg)?;
        let (lo_vb, lo_mw, lo_services, lo_junctions) = extract_arch_summary(&local_svg)?;

        let icon_size = 80.0;
        let up_center = bbox_center_from_top_left_pts(up_services.values().copied(), icon_size);
        let lo_center = bbox_center_from_top_left_pts(lo_services.values().copied(), icon_size);
        let (service_center_dx, service_center_dy) = match (up_center, lo_center) {
            (Some(up), Some(lo)) => (Some(lo.x - up.x), Some(lo.y - up.y)),
            _ => (None, None),
        };

        let svc_mean = mean_delta_by_id(&up_services, &lo_services);
        let junc_mean = mean_delta_by_id(&up_junctions, &lo_junctions);

        rows.push(Row {
            stem,
            up_vb,
            lo_vb,
            up_mw,
            lo_mw,
            service_center_dx,
            service_center_dy,
            service_mean_dx: svc_mean.map(|p| p.x),
            service_mean_dy: svc_mean.map(|p| p.y),
            junction_mean_dx: junc_mean.map(|p| p.x),
            junction_mean_dy: junc_mean.map(|p| p.y),
        });
    }

    rows.sort_by(|a, b| a.stem.cmp(&b.stem));

    let out_report = out_dir.join("architecture-delta-summary.md");
    let mut md = String::new();
    let _ = writeln!(&mut md, "# Architecture Delta Summary\n");
    let _ = writeln!(
        &mut md,
        "Generated by `xtask summarize-architecture-deltas`.\n"
    );
    let _ = writeln!(
        &mut md,
        "| fixture | up viewBox | lo viewBox | up max-width | lo max-width | svc bbox center dx | svc bbox center dy | svc mean dx | svc mean dy | junc mean dx | junc mean dy |"
    );
    let _ = writeln!(
        &mut md,
        "|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|"
    );

    for r in rows {
        let vb_up = r
            .up_vb
            .map(|v| format!("{:.3} {:.3} {:.3} {:.3}", v.0, v.1, v.2, v.3))
            .unwrap_or_else(|| "<missing>".to_string());
        let vb_lo = r
            .lo_vb
            .map(|v| format!("{:.3} {:.3} {:.3} {:.3}", v.0, v.1, v.2, v.3))
            .unwrap_or_else(|| "<missing>".to_string());

        let _ = writeln!(
            &mut md,
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |",
            r.stem,
            vb_up,
            vb_lo,
            r.up_mw
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<missing>".to_string()),
            r.lo_mw
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<missing>".to_string()),
            r.service_center_dx
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<n/a>".to_string()),
            r.service_center_dy
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<n/a>".to_string()),
            r.service_mean_dx
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<n/a>".to_string()),
            r.service_mean_dy
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<n/a>".to_string()),
            r.junction_mean_dx
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<n/a>".to_string()),
            r.junction_mean_dy
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<n/a>".to_string()),
        );
    }

    fs::write(&out_report, &md).map_err(|source| XtaskError::WriteFile {
        path: out_report.display().to_string(),
        source,
    })?;

    println!("report: {}", out_report.display());
    Ok(())
}

fn compare_dagre_layout(args: Vec<String>) -> Result<(), XtaskError> {
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

fn gen_mindmap_text_overrides(args: Vec<String>) -> Result<(), XtaskError> {
    use std::collections::{BTreeMap, BTreeSet};

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    let mut in_dir: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--in" => {
                i += 1;
                in_dir = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let in_dir = in_dir.unwrap_or_else(|| {
        workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("mindmap")
    });
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("crates")
            .join("merman-render")
            .join("src")
            .join("generated")
            .join("mindmap_text_overrides_11_12_2.rs")
    });

    fn font_size_key(font_size: f64) -> u16 {
        if !(font_size.is_finite() && font_size > 0.0) {
            return 0;
        }
        let k = (font_size * 100.0).round();
        if !(k.is_finite() && k >= 0.0 && k <= (u16::MAX as f64)) {
            return 0;
        }
        k as u16
    }

    fn collapse_ws(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut prev_space = true;
        for ch in s.chars() {
            if ch.is_whitespace() {
                if !prev_space {
                    out.push(' ');
                    prev_space = true;
                }
            } else {
                out.push(ch);
                prev_space = false;
            }
        }
        out.trim().to_string()
    }

    fn has_ancestor_class_token(node: roxmltree::Node<'_, '_>, token: &str) -> bool {
        let mut cur = Some(node);
        while let Some(n) = cur {
            if n.is_element()
                && n.attribute("class")
                    .is_some_and(|c| c.split_whitespace().any(|t| t == token))
            {
                return true;
            }
            cur = n.parent();
        }
        false
    }

    fn parse_font_size_px_from_style(svg_text: &str) -> Option<f64> {
        // Mermaid emits `font-size:16px` in the diagram-scoped stylesheet. Keep the parser small and
        // conservative: pick the first `font-size:` occurrence and parse a number ending with `px`.
        let key = "font-size:";
        let idx = svg_text.find(key)?;
        let rest = svg_text[idx + key.len()..].trim_start();
        let mut num = String::new();
        for ch in rest.chars() {
            if ch.is_ascii_digit() || ch == '.' {
                num.push(ch);
            } else {
                break;
            }
        }
        if num.is_empty() {
            return None;
        }
        let rest = &rest[num.len()..];
        if !rest.trim_start().starts_with("px") {
            return None;
        }
        num.parse::<f64>().ok()
    }

    let mut entries: BTreeMap<(u16, String), f64> = BTreeMap::new();
    let mut seen_files: BTreeSet<String> = BTreeSet::new();

    for dir_ent in std::fs::read_dir(&in_dir).map_err(|source| XtaskError::ReadFile {
        path: in_dir.display().to_string(),
        source,
    })? {
        let dir_ent = dir_ent.map_err(|source| XtaskError::ReadFile {
            path: in_dir.display().to_string(),
            source,
        })?;
        let path = dir_ent.path();
        if path.extension().and_then(|s| s.to_str()) != Some("svg") {
            continue;
        }
        let fname = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        seen_files.insert(fname);

        let svg = std::fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;
        let font_size = parse_font_size_px_from_style(&svg).unwrap_or(16.0);
        let fs_key = font_size_key(font_size);
        if fs_key == 0 {
            continue;
        }

        let doc = roxmltree::Document::parse(&svg)
            .map_err(|e| XtaskError::SvgCompareFailed(e.to_string()))?;

        for fo in doc
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "foreignObject")
        {
            // Only collect mindmap node labels, not edge labels (which are empty / width=0).
            if !has_ancestor_class_token(fo, "node") {
                continue;
            }

            let Some(width_attr) = fo.attribute("width") else {
                continue;
            };
            let Ok(width_px) = width_attr.parse::<f64>() else {
                continue;
            };
            if width_px <= 0.0 {
                continue;
            }

            // Text is nested under `<p>` in mindmap SVGs.
            let text = fo
                .descendants()
                .find(|n| n.is_element() && n.tag_name().name() == "p")
                .and_then(|p| p.text())
                .map(collapse_ws)
                .unwrap_or_default();
            if text.is_empty() {
                continue;
            }

            entries.entry((fs_key, text)).or_insert(width_px);
        }
    }

    let mut out = String::new();
    out.push_str("// This file is generated by `xtask gen-mindmap-text-overrides`.\n//\n");
    out.push_str("// Mermaid baseline: 11.12.2\n");
    out.push_str("// Source: fixtures/upstream-svgs/mindmap/*.svg\n\n");

    out.push_str("#[allow(dead_code)]\n");
    out.push_str("fn font_size_key(font_size: f64) -> u16 {\n");
    out.push_str(
        "    if !(font_size.is_finite() && font_size > 0.0) {\n        return 0;\n    }\n",
    );
    out.push_str("    let k = (font_size * 100.0).round();\n");
    out.push_str("    if !(k.is_finite() && k >= 0.0 && k <= (u16::MAX as f64)) {\n        return 0;\n    }\n");
    out.push_str("    k as u16\n}\n\n");

    out.push_str("static HTML_WIDTH_OVERRIDES_PX: &[(u16, &str, f64)] = &[\n");
    for ((fs, text), w) in &entries {
        let esc = text.replace('\\', "\\\\").replace('\"', "\\\"");
        out.push_str(&format!("    ({fs}, \"{esc}\", {w}),\n"));
    }
    out.push_str("];\n\n");

    out.push_str("pub fn lookup_html_width_px(font_size: f64, text: &str) -> Option<f64> {\n");
    out.push_str("    let fs = font_size_key(font_size);\n");
    out.push_str("    if fs == 0 || text.is_empty() {\n        return None;\n    }\n");
    out.push_str("    let mut lo = 0usize;\n    let mut hi = HTML_WIDTH_OVERRIDES_PX.len();\n");
    out.push_str("    while lo < hi {\n");
    out.push_str("        let mid = (lo + hi) / 2;\n");
    out.push_str("        let (k_fs, k_text, w) = HTML_WIDTH_OVERRIDES_PX[mid];\n");
    out.push_str("        match k_fs.cmp(&fs) {\n");
    out.push_str("            std::cmp::Ordering::Equal => match k_text.cmp(text) {\n");
    out.push_str("                std::cmp::Ordering::Equal => return Some(w),\n");
    out.push_str("                std::cmp::Ordering::Less => lo = mid + 1,\n");
    out.push_str("                std::cmp::Ordering::Greater => hi = mid,\n");
    out.push_str("            },\n");
    out.push_str("            std::cmp::Ordering::Less => lo = mid + 1,\n");
    out.push_str("            std::cmp::Ordering::Greater => hi = mid,\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    None\n}\n");

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    std::fs::write(&out_path, out).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;
    Ok(())
}

fn gen_gantt_text_overrides(args: Vec<String>) -> Result<(), XtaskError> {
    use std::collections::{BTreeMap, BTreeSet};

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    let mut in_dir: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--in" => {
                i += 1;
                in_dir = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let in_dir = in_dir.unwrap_or_else(|| {
        workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("gantt")
    });
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("crates")
            .join("merman-render")
            .join("src")
            .join("generated")
            .join("gantt_text_overrides_11_12_2.rs")
    });

    fn font_size_key(font_size: f64) -> u16 {
        if !(font_size.is_finite() && font_size > 0.0) {
            return 0;
        }
        let k = (font_size * 100.0).round();
        if !(k.is_finite() && k >= 0.0 && k <= (u16::MAX as f64)) {
            return 0;
        }
        k as u16
    }

    fn rust_f64(v: f64) -> String {
        let mut s = format!("{v}");
        if !s.contains('.') && !s.contains('e') && !s.contains('E') {
            s.push_str(".0");
        }
        s
    }

    let mut widths: BTreeMap<(u16, String), f64> = BTreeMap::new();
    let mut conflicts: BTreeSet<String> = BTreeSet::new();

    let mut svg_paths: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&in_dir).map_err(|e| {
        XtaskError::SvgCompareFailed(format!("failed to read dir {}: {}", in_dir.display(), e))
    })? {
        let entry = entry.map_err(|e| {
            XtaskError::SvgCompareFailed(format!(
                "failed to read dir entry {}: {}",
                in_dir.display(),
                e
            ))
        })?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|e| e.to_string_lossy().to_ascii_lowercase() == "svg")
        {
            svg_paths.push(path);
        }
    }
    svg_paths.sort();

    for path in svg_paths {
        let svg = std::fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;
        let doc = roxmltree::Document::parse(&svg).map_err(|e| {
            XtaskError::SvgCompareFailed(format!(
                "failed to parse upstream Gantt SVG {}: {}",
                path.display(),
                e
            ))
        })?;

        for node in doc.descendants().filter(|n| n.has_tag_name("text")) {
            let class = node.attribute("class").unwrap_or_default();
            if class.is_empty() {
                continue;
            }
            // Only capture the width hints that Mermaid emits on task labels:
            // `taskText ... width-<bboxWidth>`.
            if !class.split_whitespace().any(|t| t.starts_with("taskText")) {
                continue;
            }
            let Some(width_tok) = class.split_whitespace().find(|t| t.starts_with("width-")) else {
                continue;
            };
            let Some(width_str) = width_tok.strip_prefix("width-") else {
                continue;
            };
            let Ok(width_px) = width_str.parse::<f64>() else {
                continue;
            };
            if !(width_px.is_finite() && width_px >= 0.0) {
                continue;
            }

            let font_size = node
                .attribute("font-size")
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0);
            let fs_key = font_size_key(font_size);
            if fs_key == 0 {
                continue;
            }

            let text = node.text().unwrap_or_default().trim_end().to_string();
            if text.is_empty() {
                continue;
            }

            let key = (fs_key, text);
            if let Some(prev) = widths.get(&key).copied() {
                if (prev - width_px).abs() > 1e-6 {
                    conflicts.insert(format!(
                        "gantt width conflict for font_size={} text={:?}: {} vs {} (file {})",
                        font_size,
                        key.1,
                        rust_f64(prev),
                        rust_f64(width_px),
                        path.display()
                    ));
                }
            } else {
                widths.insert(key, width_px);
            }
        }
    }

    if !conflicts.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "conflicts while generating Gantt text overrides:\n{}",
            conflicts.into_iter().collect::<Vec<_>>().join("\n")
        )));
    }

    let entries: Vec<(u16, String, f64)> =
        widths.into_iter().map(|((fs, t), w)| (fs, t, w)).collect();

    let mut out = String::new();
    let _ = writeln!(
        &mut out,
        "// This file is generated by `xtask gen-gantt-text-overrides`.\n//\n// Mermaid baseline: 11.12.2\n// Source: fixtures/upstream-svgs/gantt/*.svg\n"
    );
    let _ = writeln!(&mut out, "#[allow(dead_code)]");
    let _ = writeln!(&mut out, "fn font_size_key(font_size: f64) -> u16 {{");
    let _ = writeln!(
        &mut out,
        "    if !(font_size.is_finite() && font_size > 0.0) {{ return 0; }}"
    );
    let _ = writeln!(&mut out, "    let k = (font_size * 100.0).round();");
    let _ = writeln!(
        &mut out,
        "    if !(k.is_finite() && k >= 0.0 && k <= (u16::MAX as f64)) {{ return 0; }}"
    );
    let _ = writeln!(&mut out, "    k as u16");
    let _ = writeln!(&mut out, "}}");
    let _ = writeln!(&mut out);

    let _ = writeln!(
        &mut out,
        "static TASK_TEXT_BBOX_WIDTH_OVERRIDES_PX: &[(u16, &str, f64)] = &["
    );
    for (fs, t, w) in &entries {
        let _ = writeln!(&mut out, "    ({fs}, {:?}, {}),", t, rust_f64(*w));
    }
    let _ = writeln!(&mut out, "];\n");

    let _ = writeln!(
        &mut out,
        "pub fn lookup_task_text_bbox_width_px(font_size: f64, text: &str) -> Option<f64> {{"
    );
    let _ = writeln!(&mut out, "    let fs = font_size_key(font_size);");
    let _ = writeln!(
        &mut out,
        "    if fs == 0 || text.is_empty() {{ return None; }}"
    );
    let _ = writeln!(&mut out, "    let mut lo = 0usize;");
    let _ = writeln!(
        &mut out,
        "    let mut hi = TASK_TEXT_BBOX_WIDTH_OVERRIDES_PX.len();"
    );
    let _ = writeln!(&mut out, "    while lo < hi {{");
    let _ = writeln!(&mut out, "        let mid = (lo + hi) / 2;");
    let _ = writeln!(
        &mut out,
        "        let (k_fs, k_text, w) = TASK_TEXT_BBOX_WIDTH_OVERRIDES_PX[mid];"
    );
    let _ = writeln!(&mut out, "        match k_fs.cmp(&fs) {{");
    let _ = writeln!(&mut out, "            std::cmp::Ordering::Equal => {{");
    let _ = writeln!(&mut out, "                match k_text.cmp(text) {{");
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Equal => return Some(w),"
    );
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Less => lo = mid + 1,"
    );
    let _ = writeln!(
        &mut out,
        "                    std::cmp::Ordering::Greater => hi = mid,"
    );
    let _ = writeln!(&mut out, "                }}");
    let _ = writeln!(&mut out, "            }}");
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Less => lo = mid + 1,"
    );
    let _ = writeln!(
        &mut out,
        "            std::cmp::Ordering::Greater => hi = mid,"
    );
    let _ = writeln!(&mut out, "        }}");
    let _ = writeln!(&mut out, "    }}");
    let _ = writeln!(&mut out, "    None");
    let _ = writeln!(&mut out, "}}");

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    std::fs::write(&out_path, out).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}

fn gen_font_metrics(args: Vec<String>) -> Result<(), XtaskError> {
    let mut in_dir: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut base_font_size_px: f64 = 16.0;
    let mut debug_text: Option<String> = None;
    let mut debug_dump: usize = 0;
    let mut backend: String = "browser".to_string();
    let mut browser_exe: Option<PathBuf> = None;
    let mut svg_sample_mode: String = "flowchart".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--in" => {
                i += 1;
                in_dir = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--font-size" => {
                i += 1;
                base_font_size_px = args
                    .get(i)
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(16.0);
            }
            "--debug-text" => {
                i += 1;
                debug_text = args.get(i).map(|s| s.to_string());
            }
            "--debug-dump" => {
                i += 1;
                debug_dump = args
                    .get(i)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);
            }
            "--backend" => {
                i += 1;
                backend = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "browser".to_string());
            }
            "--svg-sample-mode" => {
                i += 1;
                svg_sample_mode = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "flowchart".to_string());
            }
            "--browser-exe" => {
                i += 1;
                browser_exe = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let in_dir = in_dir.ok_or(XtaskError::Usage)?;
    let out_path = out_path.ok_or(XtaskError::Usage)?;

    #[derive(Debug, Clone)]
    struct Sample {
        font_key: String,
        text: String,
        width_px: f64,
        font_size_px: f64,
    }

    fn normalize_font_key(s: &str) -> String {
        s.chars()
            .filter_map(|ch| {
                if ch.is_whitespace() || ch == '"' || ch == '\'' || ch == ';' {
                    None
                } else {
                    Some(ch.to_ascii_lowercase())
                }
            })
            .collect()
    }

    fn extract_base_font_family(svg: &str) -> String {
        let Ok(doc) = roxmltree::Document::parse(svg) else {
            return String::new();
        };
        let Some(root) = doc.descendants().find(|n| n.has_tag_name("svg")) else {
            return String::new();
        };
        let id = root.attribute("id").unwrap_or_default();
        let Some(style_node) = doc.descendants().find(|n| n.has_tag_name("style")) else {
            return String::new();
        };
        let style_text = style_node.text().unwrap_or_default();
        if id.is_empty() || style_text.is_empty() {
            return String::new();
        }
        let pat = format!(
            r#"#{id}\{{[^}}]*font-family:([^;}}]+)"#,
            id = regex::escape(id)
        );
        let Ok(re) = Regex::new(&pat) else {
            return String::new();
        };
        let Some(caps) = re.captures(style_text) else {
            return String::new();
        };
        caps.get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    }

    fn foreignobject_text_lines(fo: roxmltree::Node<'_, '_>) -> Vec<String> {
        let mut raw = String::new();
        for n in fo.descendants() {
            if n.is_element() {
                match n.tag_name().name() {
                    "br" => raw.push('\n'),
                    "p" => {
                        if !raw.is_empty() && !raw.ends_with('\n') {
                            raw.push('\n');
                        }
                    }
                    _ => {}
                }
            }
            if n.is_text() {
                if let Some(t) = n.text() {
                    raw.push_str(t);
                }
            }
        }

        raw.split('\n')
            .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect()
    }

    use base64::Engine as _;

    #[allow(dead_code)]
    fn class_has_token(node: roxmltree::Node<'_, '_>, token: &str) -> bool {
        node.attribute("class")
            .unwrap_or_default()
            .split_whitespace()
            .any(|t| t == token)
    }

    #[allow(dead_code)]
    fn parse_translate_x(transform: &str) -> Option<f64> {
        let t = transform.trim();
        let start = t.find("translate(")? + "translate(".len();
        let rest = &t[start..];
        let end = rest.find([',', ' ', ')']).unwrap_or(rest.len());
        rest[..end].trim().parse::<f64>().ok()
    }

    #[allow(dead_code)]
    fn accumulated_translate_x(node: roxmltree::Node<'_, '_>) -> f64 {
        let mut sum = 0.0;
        for a in node.ancestors().filter(|n| n.is_element()) {
            if let Some(t) = a.attribute("transform") {
                if let Some(x) = parse_translate_x(t) {
                    sum += x;
                }
            }
        }
        sum
    }

    #[allow(dead_code)]
    fn parse_viewbox_w(root_svg: roxmltree::Node<'_, '_>) -> Option<f64> {
        let vb = root_svg.attribute("viewBox")?;
        let nums = vb
            .split_whitespace()
            .filter_map(|s| s.parse::<f64>().ok())
            .collect::<Vec<_>>();
        if nums.len() == 4 { Some(nums[2]) } else { None }
    }

    #[allow(dead_code)]
    fn parse_viewbox(root_svg: roxmltree::Node<'_, '_>) -> Option<(f64, f64, f64, f64)> {
        let vb = root_svg.attribute("viewBox")?;
        let nums = vb
            .split_whitespace()
            .filter_map(|s| s.parse::<f64>().ok())
            .collect::<Vec<_>>();
        if nums.len() == 4 {
            Some((nums[0], nums[1], nums[2], nums[3]))
        } else {
            None
        }
    }

    fn extract_flowchart_title_font_size_px(svg: &str, diagram_id: &str) -> Option<f64> {
        if diagram_id.is_empty() {
            return None;
        }
        let Ok(doc) = roxmltree::Document::parse(svg) else {
            return None;
        };
        let style_node = doc.descendants().find(|n| n.has_tag_name("style"))?;
        let style_text = style_node.text().unwrap_or_default();
        if style_text.is_empty() {
            return None;
        }
        let pat = format!(
            r#"#{id}\s+\.flowchartTitleText\{{[^}}]*font-size:([0-9.]+)px"#,
            id = regex::escape(diagram_id)
        );
        let Ok(re) = Regex::new(&pat) else {
            return None;
        };
        let caps = re.captures(style_text)?;
        caps.get(1)?.as_str().parse::<f64>().ok()
    }

    fn extract_base_font_size_px(svg: &str, diagram_id: &str) -> Option<f64> {
        if diagram_id.is_empty() {
            return None;
        }
        let Ok(doc) = roxmltree::Document::parse(svg) else {
            return None;
        };
        let style_node = doc.descendants().find(|n| n.has_tag_name("style"))?;
        let style_text = style_node.text().unwrap_or_default();
        if style_text.is_empty() {
            return None;
        }
        let pat = format!(
            r#"#{id}\{{[^}}]*font-size:([0-9.]+)px"#,
            id = regex::escape(diagram_id)
        );
        let Ok(re) = Regex::new(&pat) else {
            return None;
        };
        let caps = re.captures(style_text)?;
        caps.get(1)?.as_str().parse::<f64>().ok()
    }

    #[allow(dead_code)]
    fn parse_points_min_max_x(points: &str) -> Option<(f64, f64)> {
        let nums = points
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<f64>().ok())
            .collect::<Vec<_>>();
        if nums.len() < 2 {
            return None;
        }
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        for (idx, v) in nums.into_iter().enumerate() {
            if idx % 2 != 0 {
                continue;
            }
            min_x = min_x.min(v);
            max_x = max_x.max(v);
        }
        if min_x.is_finite() && max_x.is_finite() && min_x <= max_x {
            Some((min_x, max_x))
        } else {
            None
        }
    }

    #[allow(dead_code)]
    fn estimate_flowchart_content_width_px(doc: &roxmltree::Document<'_>) -> Option<f64> {
        let root_g = doc
            .descendants()
            .find(|n| n.has_tag_name("g") && n.is_element() && class_has_token(*n, "root"))?;

        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;

        for n in root_g.descendants().filter(|n| n.is_element()) {
            let tx = accumulated_translate_x(n);

            // Prefer edge extents from Mermaid's baked-in `data-points` (base64 JSON points),
            // which are in diagram coordinates and avoid having to parse SVG path `d` data.
            if n.has_tag_name("path") {
                if let Some(dp) = n.attribute("data-points") {
                    if let Ok(bytes) =
                        base64::engine::general_purpose::STANDARD.decode(dp.as_bytes())
                    {
                        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                            if let Some(arr) = v.as_array() {
                                for p in arr {
                                    let Some(x) = p.get("x").and_then(|v| v.as_f64()) else {
                                        continue;
                                    };
                                    if !x.is_finite() {
                                        continue;
                                    }
                                    min_x = min_x.min(tx + x);
                                    max_x = max_x.max(tx + x);
                                }
                            }
                        }
                    }
                }
                continue;
            }

            // Include label boxes that are rendered as `<foreignObject>` but do not live inside
            // nodes/clusters (e.g. edge labels). These participate in `getBBox()` and can dominate
            // the layout width, so excluding them would misclassify "title-dominant" samples.
            if n.has_tag_name("foreignObject") {
                let width_px = n
                    .attribute("width")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                if !(width_px.is_finite() && width_px > 0.0) {
                    continue;
                }
                let x = n
                    .attribute("x")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                if !x.is_finite() {
                    continue;
                }
                min_x = min_x.min(tx + x);
                max_x = max_x.max(tx + x + width_px);
                continue;
            }

            // Otherwise restrict to shapes inside nodes/clusters to avoid markers and other
            // decorations that live outside the main layout bounds.
            let in_node_or_cluster = n.ancestors().any(|a| {
                a.is_element() && (class_has_token(a, "node") || class_has_token(a, "cluster"))
            });
            if !in_node_or_cluster {
                continue;
            }

            match n.tag_name().name() {
                "rect" => {
                    let x = n.attribute("x").and_then(|v| v.parse::<f64>().ok());
                    let w = n.attribute("width").and_then(|v| v.parse::<f64>().ok());
                    let (Some(x), Some(w)) = (x, w) else {
                        continue;
                    };
                    if !(x.is_finite() && w.is_finite() && w > 0.0) {
                        continue;
                    }
                    min_x = min_x.min(tx + x);
                    max_x = max_x.max(tx + x + w);
                }
                "circle" => {
                    let cx = n.attribute("cx").and_then(|v| v.parse::<f64>().ok());
                    let r = n.attribute("r").and_then(|v| v.parse::<f64>().ok());
                    let (Some(cx), Some(r)) = (cx, r) else {
                        continue;
                    };
                    if !(cx.is_finite() && r.is_finite() && r > 0.0) {
                        continue;
                    }
                    min_x = min_x.min(tx + cx - r);
                    max_x = max_x.max(tx + cx + r);
                }
                "ellipse" => {
                    let cx = n.attribute("cx").and_then(|v| v.parse::<f64>().ok());
                    let rx = n.attribute("rx").and_then(|v| v.parse::<f64>().ok());
                    let (Some(cx), Some(rx)) = (cx, rx) else {
                        continue;
                    };
                    if !(cx.is_finite() && rx.is_finite() && rx > 0.0) {
                        continue;
                    }
                    min_x = min_x.min(tx + cx - rx);
                    max_x = max_x.max(tx + cx + rx);
                }
                "polygon" => {
                    let Some(points) = n.attribute("points") else {
                        continue;
                    };
                    let Some((pmin, pmax)) = parse_points_min_max_x(points) else {
                        continue;
                    };
                    min_x = min_x.min(tx + pmin);
                    max_x = max_x.max(tx + pmax);
                }
                _ => {}
            }
        }

        if !(min_x.is_finite() && max_x.is_finite() && min_x <= max_x) {
            return None;
        }
        Some(max_x - min_x)
    }

    let mut html_samples: Vec<Sample> = Vec::new();
    let mut html_seed_samples: Vec<Sample> = Vec::new();
    let mut svg_samples: Vec<Sample> = Vec::new();
    let mut font_family_by_key: BTreeMap<String, String> = BTreeMap::new();
    let Ok(entries) = fs::read_dir(&in_dir) else {
        return Err(XtaskError::ReadFile {
            path: in_dir.display().to_string(),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        });
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_file_with_extension(&path, "svg") {
            continue;
        }
        let svg = match fs::read_to_string(&path) {
            Ok(v) => v,
            Err(err) => {
                return Err(XtaskError::ReadFile {
                    path: path.display().to_string(),
                    source: err,
                });
            }
        };

        let base_family_raw = extract_base_font_family(&svg);
        let font_key = normalize_font_key(&base_family_raw);
        if font_key.is_empty() {
            continue;
        }
        // Mermaid's `calculateTextDimensions` probes both `sans-serif` and the configured
        // `fontFamily`. Generate a dedicated `sans-serif` table so headless `calculateTextWidth`
        // call sites can follow upstream behavior.
        let sans_key = "sans-serif".to_string();
        font_family_by_key
            .entry(sans_key.clone())
            .or_insert_with(|| "sans-serif".to_string());
        font_family_by_key
            .entry(font_key.clone())
            .or_insert_with(|| base_family_raw.clone());

        let Ok(doc) = roxmltree::Document::parse(&svg) else {
            continue;
        };

        let Some(root_svg) = doc.descendants().find(|n| n.has_tag_name("svg")) else {
            continue;
        };
        let diagram_id = root_svg.attribute("id").unwrap_or_default();
        let diagram_font_size_px = extract_base_font_size_px(&svg, diagram_id)
            .unwrap_or(base_font_size_px)
            .max(1.0);

        for fo in doc
            .descendants()
            .filter(|n| n.has_tag_name("foreignObject"))
        {
            let lines = foreignobject_text_lines(fo);
            for text in &lines {
                if text.is_empty() {
                    continue;
                }
                // Seed samples are used to build the per-font character set (including unicode
                // characters from long labels). Width is intentionally zero so these do not affect
                // `html_overrides` regression.
                html_seed_samples.push(Sample {
                    font_key: font_key.clone(),
                    text: text.clone(),
                    width_px: 0.0,
                    font_size_px: diagram_font_size_px,
                });
                html_seed_samples.push(Sample {
                    font_key: sans_key.clone(),
                    text: text.clone(),
                    width_px: 0.0,
                    font_size_px: diagram_font_size_px,
                });
            }

            let width_px = fo
                .attribute("width")
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0);
            if !(width_px.is_finite() && width_px > 0.0) {
                continue;
            }
            // Mermaid HTML labels are rendered with `max-width: 200px`. When a label hits that
            // constraint, the measured width is no longer a linear function of per-character
            // advances. Filter those samples out to keep the regression stable.
            if width_px >= 190.0 {
                continue;
            }
            if lines.len() != 1 {
                continue;
            }
            let text = lines[0].clone();
            if text.is_empty() {
                continue;
            }
            html_samples.push(Sample {
                font_key: font_key.clone(),
                text,
                width_px,
                font_size_px: diagram_font_size_px,
            });
        }

        // Collect SVG `<text>` samples to later derive a `svg_scale` factor that approximates
        // Mermaid's SVG text advance measurement behavior (`getComputedTextLength()` in practice).

        // Prefer collecting the inner tspans used by Mermaid's `createText(...)` output. These
        // correspond to individual wrapped lines and are abundant across fixtures, which makes the
        // derived scale significantly more stable than the older "title-dominant viewBox" heuristic.
        for tspan in doc.descendants().filter(|n| n.has_tag_name("tspan")) {
            let class = tspan.attribute("class").unwrap_or_default();
            if !class.split_whitespace().any(|t| t == "text-inner-tspan") {
                continue;
            }
            let line = tspan.text().unwrap_or_default().trim().to_string();
            if line.is_empty() {
                continue;
            }
            svg_samples.push(Sample {
                font_key: font_key.clone(),
                text: line.clone(),
                width_px: 0.0,
                font_size_px: diagram_font_size_px,
            });
            svg_samples.push(Sample {
                font_key: sans_key.clone(),
                text: line,
                width_px: 0.0,
                font_size_px: diagram_font_size_px,
            });
        }

        // Keep flowchart diagram title samples as a fallback (they are usually unwrapped).
        if let Some(title_node) = doc.descendants().find(|n| {
            n.has_tag_name("text")
                && n.attribute("class")
                    .unwrap_or_default()
                    .split_whitespace()
                    .any(|t| t == "flowchartTitleText")
        }) {
            let title_text = title_node.text().unwrap_or_default().trim().to_string();
            if !title_text.is_empty() {
                let title_font_size_px = extract_flowchart_title_font_size_px(&svg, diagram_id)
                    .unwrap_or(diagram_font_size_px)
                    .max(1.0);
                svg_samples.push(Sample {
                    font_key: font_key.clone(),
                    text: title_text.clone(),
                    width_px: 0.0,
                    font_size_px: title_font_size_px,
                });
                svg_samples.push(Sample {
                    font_key: sans_key.clone(),
                    text: title_text,
                    width_px: 0.0,
                    font_size_px: title_font_size_px,
                });
            }
        }

        // Mermaid sequence diagrams render many labels as plain SVG `<text>` (or single `<tspan>`
        // runs) without the `text-inner-tspan` helper class. When generating metrics for those
        // diagrams, include the relevant label strings so we can derive stable `svg_overrides`
        // from upstream fixtures.
        if svg_sample_mode == "sequence" {
            for text_node in doc.descendants().filter(|n| n.has_tag_name("text")) {
                let class = text_node.attribute("class").unwrap_or_default();
                let tokens: Vec<&str> = class.split_whitespace().collect();
                if tokens.is_empty() {
                    continue;
                }
                let is_sequence_label = tokens.iter().any(|t| {
                    matches!(
                        *t,
                        "messageText"
                            | "noteText"
                            | "labelText"
                            | "loopText"
                            | "actor"
                            | "actor-man"
                    )
                });
                if !is_sequence_label {
                    continue;
                }

                // Prefer per-line `<tspan>` runs when present.
                let mut pushed_any = false;
                for tspan in text_node.children().filter(|n| n.has_tag_name("tspan")) {
                    let line = tspan.text().unwrap_or_default().trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    pushed_any = true;
                    svg_samples.push(Sample {
                        font_key: font_key.clone(),
                        text: line.clone(),
                        width_px: 0.0,
                        font_size_px: diagram_font_size_px,
                    });
                    svg_samples.push(Sample {
                        font_key: sans_key.clone(),
                        text: line,
                        width_px: 0.0,
                        font_size_px: diagram_font_size_px,
                    });
                }
                if pushed_any {
                    continue;
                }

                let line = text_node.text().unwrap_or_default().trim().to_string();
                if line.is_empty() {
                    continue;
                }
                svg_samples.push(Sample {
                    font_key: font_key.clone(),
                    text: line.clone(),
                    width_px: 0.0,
                    font_size_px: diagram_font_size_px,
                });
                svg_samples.push(Sample {
                    font_key: sans_key.clone(),
                    text: line,
                    width_px: 0.0,
                    font_size_px: diagram_font_size_px,
                });
            }
        }
    }

    // Add a small set of extra seed strings that are known to appear across non-flowchart
    // diagrams (notably ER) and that are sensitive to uppercase kerning/hinting in Chromium.
    // These samples improve `calculateTextWidth` parity without requiring per-diagram tables.
    const EXTRA_SEED_TEXTS: &[&str] = &["DRIVER", "PERSON"];
    for font_key in font_family_by_key.keys().cloned().collect::<Vec<_>>() {
        for &text in EXTRA_SEED_TEXTS {
            html_seed_samples.push(Sample {
                font_key: font_key.clone(),
                text: text.to_string(),
                width_px: 0.0,
                font_size_px: base_font_size_px.max(1.0),
            });
            svg_samples.push(Sample {
                font_key: font_key.clone(),
                text: text.to_string(),
                width_px: 0.0,
                font_size_px: base_font_size_px.max(1.0),
            });
        }
    }

    if matches!(backend.as_str(), "browser" | "puppeteer") && !svg_samples.is_empty() {
        let browser_exe = if let Some(p) = browser_exe.as_deref() {
            p.to_path_buf()
        } else if cfg!(windows) {
            detect_windows_browser_exe().ok_or_else(|| {
                XtaskError::SvgCompareFailed(
                    "no supported browser found for font measurement".into(),
                )
            })?
        } else {
            return Err(XtaskError::SvgCompareFailed(
                "browser measurement requires --browser-exe on this platform".into(),
            ));
        };

        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");
        let node_cwd = workspace_root.join("tools").join("mermaid-cli");

        // Group by `(font_key, font_size_px)` to minimize browser round-trips.
        let mut groups: BTreeMap<(String, i64), Vec<usize>> = BTreeMap::new();
        for (idx, s) in svg_samples.iter().enumerate() {
            let size_key = (s.font_size_px * 1000.0).round() as i64;
            groups
                .entry((s.font_key.clone(), size_key))
                .or_default()
                .push(idx);
        }

        for ((font_key, size_key), idxs) in groups {
            let Some(font_family) = font_family_by_key.get(&font_key) else {
                continue;
            };
            let font_size_px = (size_key as f64) / 1000.0;
            let strings = idxs
                .iter()
                .map(|&i| svg_samples[i].text.clone())
                .collect::<Vec<_>>();
            let widths_px = measure_svg_text_bbox_widths_via_browser(
                &node_cwd,
                &browser_exe,
                font_family,
                font_size_px,
                &strings,
            )?;
            for (&i, w) in idxs.iter().zip(widths_px.into_iter()) {
                svg_samples[i].width_px = w;
            }
        }

        svg_samples.retain(|s| s.width_px.is_finite() && s.width_px > 0.0);
    }

    if html_samples.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no foreignObject samples found under {}",
            in_dir.display()
        )));
    }

    if let Some(dt) = debug_text.as_deref() {
        eprintln!("debug-text={dt:?}");
        for (label, list) in [("html", &html_samples), ("svg", &svg_samples)] {
            let mut by_font: BTreeMap<String, Vec<f64>> = BTreeMap::new();
            for s in list.iter() {
                if s.text == dt {
                    by_font
                        .entry(s.font_key.clone())
                        .or_default()
                        .push(s.width_px / s.font_size_px.max(1.0));
                }
            }
            if by_font.is_empty() {
                continue;
            }
            eprintln!("  source={label}");
            for (k, mut ws) in by_font {
                ws.sort_by(|a, b| a.total_cmp(b));
                let min = ws.first().copied().unwrap_or(0.0);
                let max = ws.last().copied().unwrap_or(0.0);
                let mean = if ws.is_empty() {
                    0.0
                } else {
                    ws.iter().sum::<f64>() / ws.len() as f64
                };
                eprintln!(
                    "    font_key={:?} samples={} em[min/mean/max]=[{:.4}/{:.4}/{:.4}]",
                    k,
                    ws.len(),
                    min,
                    mean,
                    max
                );
            }
        }
    }

    if debug_dump > 0 {
        let mut by_font: BTreeMap<String, Vec<&Sample>> = BTreeMap::new();
        for s in &html_samples {
            by_font.entry(s.font_key.clone()).or_default().push(s);
        }
        eprintln!("debug-dump: showing up to {debug_dump} samples per font_key");
        for (k, mut ss) in by_font {
            ss.sort_by(|a, b| {
                a.text
                    .cmp(&b.text)
                    .then_with(|| a.width_px.total_cmp(&b.width_px))
            });
            eprintln!("  font_key={k:?} total={}", ss.len());
            for s in ss.into_iter().take(debug_dump) {
                eprintln!("    text={:?} width_px={}", s.text, s.width_px);
            }
        }
    }

    #[allow(clippy::needless_range_loop)]
    fn solve_ridge(at_a: &mut [Vec<f64>], at_b: &mut [f64]) -> Vec<f64> {
        let n = at_b.len();
        for i in 0..n {
            // Pivot.
            let mut pivot = i;
            let mut best = at_a[i][i].abs();
            for r in (i + 1)..n {
                let v = at_a[r][i].abs();
                if v > best {
                    best = v;
                    pivot = r;
                }
            }
            if pivot != i {
                at_a.swap(i, pivot);
                at_b.swap(i, pivot);
            }

            let diag = at_a[i][i];
            if diag.abs() < 1e-12 {
                continue;
            }
            let inv = 1.0 / diag;
            for c in i..n {
                at_a[i][c] *= inv;
            }
            at_b[i] *= inv;

            for r in 0..n {
                if r == i {
                    continue;
                }
                let factor = at_a[r][i];
                if factor.abs() < 1e-12 {
                    continue;
                }
                for c in i..n {
                    at_a[r][c] -= factor * at_a[i][c];
                }
                at_b[r] -= factor * at_b[i];
            }
        }
        at_b.to_vec()
    }

    // Group by font key and fit widths in `em`, separately for:
    // - HTML `<foreignObject>` labels (used when `htmlLabels=true`), and
    // - SVG `<text>` titles (used for the flowchart title).
    let mut html_samples_by_font: BTreeMap<String, Vec<Sample>> = BTreeMap::new();
    for s in html_samples {
        html_samples_by_font
            .entry(s.font_key.clone())
            .or_default()
            .push(s);
    }
    let mut html_seed_samples_by_font: BTreeMap<String, Vec<Sample>> = BTreeMap::new();
    for s in html_seed_samples {
        html_seed_samples_by_font
            .entry(s.font_key.clone())
            .or_default()
            .push(s);
    }
    let mut svg_samples_by_font: BTreeMap<String, Vec<Sample>> = BTreeMap::new();
    for s in svg_samples {
        svg_samples_by_font
            .entry(s.font_key.clone())
            .or_default()
            .push(s);
    }

    fn heuristic_char_width_em(ch: char) -> f64 {
        if ch == ' ' {
            return 0.33;
        }
        if ch == '\t' {
            return 0.66;
        }
        if ch == '_' || ch == '-' {
            return 0.33;
        }
        if matches!(ch, '.' | ',' | ':' | ';') {
            return 0.28;
        }
        if matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '/') {
            return 0.33;
        }
        if matches!(ch, '+' | '*' | '=' | '\\' | '^' | '|' | '~') {
            return 0.45;
        }
        if ch.is_ascii_digit() {
            return 0.56;
        }
        if ch.is_ascii_uppercase() {
            return match ch {
                'I' => 0.30,
                'W' => 0.85,
                _ => 0.60,
            };
        }
        if ch.is_ascii_lowercase() {
            return match ch {
                'i' | 'l' => 0.28,
                'm' | 'w' => 0.78,
                'k' | 'y' => 0.55,
                _ => 0.43,
            };
        }
        0.60
    }

    #[derive(Debug, Clone)]
    struct FontTable {
        font_key: String,
        default_em: f64,
        entries: Vec<(char, f64)>,
        kern_pairs: Vec<(u32, u32, f64)>,
        /// Extra width adjustment (in `em`) for the pattern `a + ' ' + b`.
        ///
        /// In Chromium layout, the width contributed by a normal space can depend on surrounding
        /// glyphs (GPOS kerning around spaces, etc.). Measuring 2-char strings like `"e "` / `" T"`
        /// is unreliable because HTML collapses leading/trailing spaces. Instead, we capture the
        /// combined adjustment for internal spaces via these trigrams.
        space_trigrams: Vec<(u32, u32, f64)>,
        /// Extra width adjustment (in `em`) for the trigram pattern `a + b + c` (with no
        /// whitespace).
        ///
        /// In Chromium layout, text advances are not perfectly decomposable into
        /// `single-char widths + pair kerning`: subpixel positioning and hinting can make glyph
        /// contributions depend on immediate neighbors. We learn the residual for 3-char samples
        /// and apply it as a local correction while measuring longer strings.
        trigrams: Vec<(u32, u32, u32, f64)>,
        /// Exact (already-quantized) widths for single-line HTML `<foreignObject>` labels, stored
        /// in `em` units (width_px / font_size_px).
        ///
        /// This is used as an override for DOM parity: Chromium's layout uses fixed-point
        /// arithmetic and hinting that can make widths non-additive even with kerning pairs and
        /// local trigram residuals.
        html_overrides: Vec<(String, f64)>,
        /// Exact SVG `<text>` extents (in `em`) for `text-anchor: middle`, stored as `(text, left_em, right_em)`.
        ///
        /// SVG `getBBox()` and `getComputedTextLength()` do not match HTML layout advances, and
        /// approximations (scale factors / per-glyph overhang) can drift for long titles. These
        /// overrides are measured via a real browser and used to match upstream viewBox parity.
        svg_overrides: Vec<(String, f64, f64)>,
    }

    fn median(v: &mut [f64]) -> Option<f64> {
        if v.is_empty() {
            return None;
        }
        v.sort_by(|a, b| a.total_cmp(b));
        let mid = v.len() / 2;
        if v.len() % 2 == 1 {
            Some(v[mid])
        } else {
            Some((v[mid - 1] + v[mid]) / 2.0)
        }
    }

    fn fit_tables(
        samples_by_font: BTreeMap<String, Vec<Sample>>,
        prior_by_font: Option<&BTreeMap<String, BTreeMap<char, f64>>>,
    ) -> BTreeMap<String, FontTable> {
        let mut out: BTreeMap<String, FontTable> = BTreeMap::new();

        for (font_key, mut ss) in samples_by_font {
            ss.sort_by(|a, b| {
                a.text
                    .cmp(&b.text)
                    .then_with(|| a.width_px.total_cmp(&b.width_px))
            });

            // Stage 1: lock in direct per-character widths from single-character samples (if any).
            let mut direct: BTreeMap<char, Vec<f64>> = BTreeMap::new();
            for s in &ss {
                let mut it = s.text.chars();
                let Some(ch) = it.next() else {
                    continue;
                };
                if it.next().is_some() {
                    continue;
                }
                let w_em = s.width_px / s.font_size_px.max(1.0);
                if !(w_em.is_finite() && w_em > 0.0) {
                    continue;
                }
                direct.entry(ch).or_default().push(w_em);
            }

            let mut fixed: BTreeMap<char, f64> = BTreeMap::new();
            for (ch, mut ws) in direct {
                if let Some(m) = median(&mut ws) {
                    fixed.insert(ch, m);
                }
            }

            // Stage 2: fit remaining characters via ridge regression around priors.
            let mut unknown_chars: Vec<char> = ss
                .iter()
                .flat_map(|s| s.text.chars())
                .filter(|ch| !fixed.contains_key(ch))
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            unknown_chars.sort_by_key(|a| *a as u32);

            let mut unknown_index: BTreeMap<char, usize> = BTreeMap::new();
            for (idx, ch) in unknown_chars.iter().enumerate() {
                unknown_index.insert(*ch, idx);
            }

            let n_vars = unknown_chars.len();
            let mut sol: Vec<f64> = vec![0.0; n_vars];
            if n_vars > 0 {
                let mut at_a = vec![vec![0.0_f64; n_vars]; n_vars];
                let mut at_b = vec![0.0_f64; n_vars];
                let mut prior = vec![0.0_f64; n_vars];

                let priors_for_font = prior_by_font.and_then(|m| m.get(&font_key));
                for (idx, ch) in unknown_chars.iter().enumerate() {
                    prior[idx] = priors_for_font
                        .and_then(|m| m.get(ch))
                        .copied()
                        .unwrap_or_else(|| heuristic_char_width_em(*ch));
                }

                for s in &ss {
                    let mut counts = vec![0.0_f64; n_vars];
                    let mut fixed_sum_em: f64 = 0.0;
                    for ch in s.text.chars() {
                        if let Some(w) = fixed.get(&ch) {
                            fixed_sum_em += *w;
                            continue;
                        }
                        let Some(&idx) = unknown_index.get(&ch) else {
                            continue;
                        };
                        counts[idx] += 1.0;
                    }

                    let mut b_em = s.width_px / s.font_size_px.max(1.0) - fixed_sum_em;
                    if !b_em.is_finite() {
                        continue;
                    }
                    // For samples dominated by rounding noise, skip to avoid destabilizing the fit.
                    if b_em.abs() < 1e-6 {
                        continue;
                    }
                    // Clamp residuals to avoid pathological negative values caused by kerning or
                    // DOM rounding on very short strings.
                    if b_em < 0.0 {
                        b_em = 0.0;
                    }

                    for i in 0..n_vars {
                        let ci = counts[i];
                        if ci == 0.0 {
                            continue;
                        }
                        at_b[i] += ci * b_em;
                        for j in 0..n_vars {
                            at_a[i][j] += ci * counts[j];
                        }
                    }
                }

                let lambda = 0.05;
                for i in 0..n_vars {
                    at_a[i][i] += lambda;
                    at_b[i] += lambda * prior[i];
                }

                let mut rhs = at_b;
                let mut mat = at_a;
                sol = solve_ridge(&mut mat, &mut rhs)
                    .into_iter()
                    .map(|v| v.max(0.0))
                    .collect();
            }

            let mut entries: Vec<(char, f64)> = Vec::new();
            for (ch, w) in fixed {
                entries.push((ch, w));
            }
            for (idx, ch) in unknown_chars.iter().enumerate() {
                entries.push((*ch, sol[idx]));
            }
            entries.sort_by(|a, b| (a.0 as u32).cmp(&(b.0 as u32)));

            let avg_em = if entries.is_empty() {
                0.6
            } else {
                entries.iter().map(|(_, v)| *v).sum::<f64>() / entries.len() as f64
            };

            out.insert(
                font_key.clone(),
                FontTable {
                    font_key,
                    default_em: avg_em.max(0.1),
                    entries,
                    kern_pairs: Vec::new(),
                    space_trigrams: Vec::new(),
                    trigrams: Vec::new(),
                    html_overrides: Vec::new(),
                    svg_overrides: Vec::new(),
                },
            );
        }

        out
    }

    fn detect_windows_browser_exe() -> Option<PathBuf> {
        let candidates = [
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ];
        for p in candidates {
            let path = PathBuf::from(p);
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    #[allow(dead_code)]
    fn measure_char_widths_via_browser(
        node_cwd: &Path,
        browser_exe: &Path,
        font_family: &str,
        font_size_px: f64,
        chars: &[char],
    ) -> Result<BTreeMap<char, f64>, XtaskError> {
        use std::process::Stdio;
        let input_json = serde_json::json!({
            "browser_exe": browser_exe.display().to_string(),
            "font_family": font_family,
            "font_size_px": font_size_px,
            "chars": chars.iter().map(|ch| ch.to_string()).collect::<Vec<_>>(),
        })
        .to_string();

        // NOTE: This requires `puppeteer-core` to be resolvable from `node_cwd` (we vendor it via
        // `tools/mermaid-cli`).
        // NOTE: Mermaid's HTML label sizing is based on DOM layout (`getBoundingClientRect()` on
        // the foreignObject content). Canvas `measureText()` is close, but not identical for all
        // strings/fonts, and these small drifts bubble up into `viewBox`/`max-width` parity. We
        // intentionally measure via DOM here to match upstream SVG baselines.
        const JS: &str = r#"
 const fs = require('fs');
 const puppeteer = require('puppeteer-core');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const browserExe = input.browser_exe;
const fontFamily = input.font_family;
const fontSizePx = input.font_size_px;
const chars = input.chars;

(async () => {
  const browser = await puppeteer.launch({
    headless: 'shell',
    executablePath: browserExe,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  const page = await browser.newPage();
  await page.setContent(`<!doctype html><html><head><style>body{margin:0;padding:0;}</style></head><body></body></html>`);

  const widths = await page.evaluate(({ chars, fontFamily, fontSizePx }) => {
    const canvas = document.createElement('canvas');
    const ctx = canvas.getContext('2d');
    const ff = String(fontFamily || '').replace(/;\\s*$/, '');
    ctx.font = `${fontSizePx}px ${ff}`;

    const out = {};
    for (const ch of chars) {
      out[ch] = ctx.measureText(String(ch)).width;
    }
    return out;
  }, { chars, fontFamily, fontSizePx });

  console.log(JSON.stringify(widths));
  await browser.close();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
"#;

        let mut cmd = Command::new("node");
        cmd.current_dir(node_cwd)
            .arg("-e")
            .arg(JS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to spawn node: {source}"))
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            std::io::Write::write_all(&mut stdin, input_json.as_bytes()).map_err(|source| {
                XtaskError::SvgCompareFailed(format!("failed to write node stdin: {source}"))
            })?;
        }
        let output = child.wait_with_output().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to run node: {source}"))
        })?;
        if !output.status.success() {
            return Err(XtaskError::SvgCompareFailed(
                "browser measurement failed".to_string(),
            ));
        }

        let map: BTreeMap<String, f64> =
            serde_json::from_slice(&output.stdout).map_err(XtaskError::Json)?;
        let mut out: BTreeMap<char, f64> = BTreeMap::new();
        for (k, v) in map {
            let mut it = k.chars();
            let Some(ch) = it.next() else {
                continue;
            };
            if it.next().is_some() {
                continue;
            }
            if v.is_finite() && v >= 0.0 {
                out.insert(ch, v / font_size_px.max(1.0));
            }
        }
        Ok(out)
    }

    fn measure_text_widths_via_browser(
        node_cwd: &Path,
        browser_exe: &Path,
        font_family: &str,
        font_size_px: f64,
        strings: &[String],
    ) -> Result<Vec<f64>, XtaskError> {
        use std::process::Stdio;

        if strings.is_empty() {
            return Ok(Vec::new());
        }

        let input_json = serde_json::json!({
            "browser_exe": browser_exe.display().to_string(),
            "font_family": font_family,
            "font_size_px": font_size_px,
            "strings": strings,
        })
        .to_string();

        const JS: &str = r#"
const fs = require('fs');
const puppeteer = require('puppeteer-core');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const browserExe = input.browser_exe;
const fontFamily = input.font_family;
const fontSizePx = input.font_size_px;
const strings = input.strings;

 (async () => {
   const browser = await puppeteer.launch({
     headless: 'shell',
     executablePath: browserExe,
     args: [
       '--no-sandbox',
       '--disable-setuid-sandbox',
       // Match Mermaid CLI (Chromium) layout units more deterministically.
       '--force-device-scale-factor=1',
     ],
   });
 
   const page = await browser.newPage();
   await page.setViewport({ width: 800, height: 600, deviceScaleFactor: 1 });
   await page.setContent(`<!doctype html><html><head><style>body{margin:0;padding:0;} p{margin:0;}</style></head><body></body></html>`);
 
   const widths = await page.evaluate(({ strings, fontFamily, fontSizePx }) => {
     const ff = String(fontFamily || '').replace(/;\s*$/, '');
 
     // Mimic Mermaid's flowchart foreignObject label container (single-line).
     const div = document.createElement('div');
     div.style.display = 'table-cell';
     div.style.whiteSpace = 'nowrap';
     div.style.lineHeight = '1.5';
     div.style.maxWidth = '200px';
     div.style.textAlign = 'center';
     div.style.fontFamily = ff;
     div.style.fontSize = `${fontSizePx}px`;
 
     const span = document.createElement('span');
     span.className = 'nodeLabel';
     const p = document.createElement('p');
     span.appendChild(p);
     div.appendChild(span);
     document.body.appendChild(div);
 
     const out = [];
     for (const s of strings) {
       const ss = String(s);
       // A lone U+0020 would collapse away in HTML and measure as 0px. Use NBSP for that one
       // special case so we can still derive correct space advances for in-line spaces.
       p.textContent = ss === ' ' ? '\u00A0' : ss;
       out.push(div.getBoundingClientRect().width);
     }
     return out;
   }, { strings, fontFamily, fontSizePx });

  console.log(JSON.stringify(widths));
  await browser.close();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
"#;

        let mut cmd = Command::new("node");
        cmd.current_dir(node_cwd)
            .arg("-e")
            .arg(JS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to spawn node: {source}"))
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            std::io::Write::write_all(&mut stdin, input_json.as_bytes()).map_err(|source| {
                XtaskError::SvgCompareFailed(format!("failed to write node stdin: {source}"))
            })?;
        }
        let output = child.wait_with_output().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to run node: {source}"))
        })?;
        if !output.status.success() {
            return Err(XtaskError::SvgCompareFailed(
                "browser measurement failed".to_string(),
            ));
        }

        let widths_px: Vec<f64> =
            serde_json::from_slice(&output.stdout).map_err(XtaskError::Json)?;
        let mut out = Vec::with_capacity(widths_px.len());
        for w in widths_px {
            if w.is_finite() && w >= 0.0 {
                out.push(w);
            } else {
                out.push(0.0);
            }
        }
        Ok(out)
    }

    fn measure_svg_text_bbox_widths_via_browser(
        node_cwd: &Path,
        browser_exe: &Path,
        font_family: &str,
        font_size_px: f64,
        strings: &[String],
    ) -> Result<Vec<f64>, XtaskError> {
        use std::process::Stdio;
        if strings.is_empty() {
            return Ok(Vec::new());
        }
        let input_json = serde_json::json!({
            "browser_exe": browser_exe.display().to_string(),
            "font_family": font_family,
            "font_size_px": font_size_px,
            "strings": strings,
        })
        .to_string();
        const JS: &str = r#"
const fs = require('fs');
const puppeteer = require('puppeteer-core');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const browserExe = input.browser_exe;
const fontFamily = input.font_family;
const fontSizePx = input.font_size_px;
const strings = input.strings;

(async () => {
  const browser = await puppeteer.launch({
    headless: 'shell',
    executablePath: browserExe,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  const page = await browser.newPage();
  await page.setContent(`<!doctype html><html><head><style>body{margin:0;padding:0;}</style></head><body></body></html>`);

  const widths = await page.evaluate(({ strings, fontFamily, fontSizePx }) => {
    const out = [];
    const SVG_NS = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('width', '1000');
    svg.setAttribute('height', '200');
    document.body.appendChild(svg);

    const ff = String(fontFamily || '').replace(/;\\s*$/, '');
    for (const s of strings) {
      const t = document.createElementNS(SVG_NS, 'text');
      t.setAttribute('x', '0');
      t.setAttribute('y', '0');
      // Preserve spaces so `getComputedTextLength()` matches Mermaid's layout inputs.
      t.setAttribute('xml:space', 'preserve');
      t.setAttribute('style', `font-family:${ff};font-size:${fontSizePx}px;white-space:pre;`);
      t.textContent = String(s);
      svg.appendChild(t);
      out.push(t.getComputedTextLength());
      svg.removeChild(t);
    }
    return out;
  }, { strings, fontFamily, fontSizePx });

  console.log(JSON.stringify(widths));
  await browser.close();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
"#;
        let mut cmd = Command::new("node");
        cmd.current_dir(node_cwd)
            .arg("-e")
            .arg(JS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to spawn node: {source}"))
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            std::io::Write::write_all(&mut stdin, input_json.as_bytes()).map_err(|source| {
                XtaskError::SvgCompareFailed(format!("failed to write node stdin: {source}"))
            })?;
        }
        let output = child.wait_with_output().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to run node: {source}"))
        })?;
        if !output.status.success() {
            return Err(XtaskError::SvgCompareFailed(
                "browser svg measurement failed".to_string(),
            ));
        }
        let widths_px: Vec<f64> =
            serde_json::from_slice(&output.stdout).map_err(XtaskError::Json)?;
        let mut out = Vec::with_capacity(widths_px.len());
        for w in widths_px {
            if w.is_finite() && w >= 0.0 {
                out.push(w);
            } else {
                out.push(0.0);
            }
        }
        Ok(out)
    }

    #[derive(Debug, Clone, Copy, serde::Deserialize)]
    struct SvgTextBBoxMetrics {
        adv_px: f64,
        bbox_x: f64,
        bbox_w: f64,
    }

    fn measure_svg_text_bbox_metrics_via_browser(
        node_cwd: &Path,
        browser_exe: &Path,
        font_family: &str,
        font_size_px: f64,
        strings: &[String],
    ) -> Result<Vec<SvgTextBBoxMetrics>, XtaskError> {
        use std::process::Stdio;
        if strings.is_empty() {
            return Ok(Vec::new());
        }
        let input_json = serde_json::json!({
            "browser_exe": browser_exe.display().to_string(),
            "font_family": font_family,
            "font_size_px": font_size_px,
            "strings": strings,
        })
        .to_string();
        const JS: &str = r#"
const fs = require('fs');
const puppeteer = require('puppeteer-core');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const browserExe = input.browser_exe;
const fontFamily = input.font_family;
const fontSizePx = input.font_size_px;
const strings = input.strings;

(async () => {
  const browser = await puppeteer.launch({
    headless: 'shell',
    executablePath: browserExe,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  const page = await browser.newPage();
  await page.setContent(`<!doctype html><html><head><style>body{margin:0;padding:0;}</style></head><body></body></html>`);

  const out = await page.evaluate(({ strings, fontFamily, fontSizePx }) => {
    const SVG_NS = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('width', '1000');
    svg.setAttribute('height', '200');
    document.body.appendChild(svg);

    const ff = String(fontFamily || '').replace(/;\\s*$/, '');
    const res = [];
    for (const s of strings) {
      const t = document.createElementNS(SVG_NS, 'text');
      t.setAttribute('x', '0');
      t.setAttribute('y', '0');
      t.setAttribute('text-anchor', 'middle');
      // Preserve spaces so bbox/advance measurements match Mermaid's `<text>` output.
      t.setAttribute('xml:space', 'preserve');
      t.setAttribute('style', `font-family:${ff};font-size:${fontSizePx}px;white-space:pre;`);
      t.textContent = String(s);
      svg.appendChild(t);

      const adv = t.getComputedTextLength();
      const bb = t.getBBox();
      res.push({ adv_px: adv, bbox_x: bb.x, bbox_w: bb.width });
      svg.removeChild(t);
    }
    return res;
  }, { strings, fontFamily, fontSizePx });

  console.log(JSON.stringify(out));
  await browser.close();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
"#;

        let mut cmd = Command::new("node");
        cmd.current_dir(node_cwd)
            .arg("-e")
            .arg(JS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to spawn node: {source}"))
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            std::io::Write::write_all(&mut stdin, input_json.as_bytes()).map_err(|source| {
                XtaskError::SvgCompareFailed(format!("failed to write node stdin: {source}"))
            })?;
        }
        let output = child.wait_with_output().map_err(|source| {
            XtaskError::SvgCompareFailed(format!("failed to run node: {source}"))
        })?;
        if !output.status.success() {
            return Err(XtaskError::SvgCompareFailed(
                "browser svg measurement failed".to_string(),
            ));
        }
        let raw: Vec<SvgTextBBoxMetrics> =
            serde_json::from_slice(&output.stdout).map_err(XtaskError::Json)?;
        let mut out = Vec::with_capacity(raw.len());
        for m in raw {
            if m.adv_px.is_finite()
                && m.adv_px >= 0.0
                && m.bbox_x.is_finite()
                && m.bbox_w.is_finite()
            {
                out.push(m);
            } else {
                out.push(SvgTextBBoxMetrics {
                    adv_px: 0.0,
                    bbox_x: 0.0,
                    bbox_w: 0.0,
                });
            }
        }
        Ok(out)
    }

    fn build_tables_via_browser(
        samples_by_font: BTreeMap<String, Vec<Sample>>,
        font_family_by_key: &BTreeMap<String, String>,
        base_font_size_px: f64,
        browser_exe: Option<&Path>,
    ) -> Result<BTreeMap<String, FontTable>, XtaskError> {
        let browser_exe = if let Some(p) = browser_exe {
            p.to_path_buf()
        } else if cfg!(windows) {
            detect_windows_browser_exe().ok_or_else(|| {
                XtaskError::SvgCompareFailed(
                    "no supported browser found for font measurement".into(),
                )
            })?
        } else {
            return Err(XtaskError::SvgCompareFailed(
                "browser measurement requires --browser-exe on this platform".into(),
            ));
        };

        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");
        let node_cwd = workspace_root.join("tools").join("mermaid-cli");

        let mut out: BTreeMap<String, FontTable> = BTreeMap::new();
        for (font_key, ss) in samples_by_font {
            let Some(font_family) = font_family_by_key.get(&font_key) else {
                continue;
            };

            let mut charset: std::collections::BTreeSet<char> = std::collections::BTreeSet::new();
            let mut pairset: std::collections::BTreeSet<(char, char)> =
                std::collections::BTreeSet::new();
            for s in &ss {
                let mut prev: Option<char> = None;
                for ch in s.text.chars() {
                    charset.insert(ch);
                    if let Some(p) = prev {
                        // Avoid pairs involving whitespace. HTML collapses leading/trailing spaces,
                        // which makes 2-char samples like `"e "` / `" T"` produce bogus negative
                        // "kerning" that effectively cancels the space width. Real Mermaid labels
                        // do not apply kerning to spaces, so skipping them keeps the model stable.
                        if !p.is_whitespace() && !ch.is_whitespace() {
                            pairset.insert((p, ch));
                        }
                    }
                    prev = Some(ch);
                }
            }
            if charset.is_empty() {
                continue;
            }
            let chars = charset.into_iter().collect::<Vec<_>>();
            let char_strings = chars.iter().map(|ch| ch.to_string()).collect::<Vec<_>>();
            let widths_px = measure_text_widths_via_browser(
                &node_cwd,
                &browser_exe,
                font_family,
                base_font_size_px,
                &char_strings,
            )?;
            let mut measured: BTreeMap<char, f64> = BTreeMap::new();
            for (ch, w_px) in chars.iter().copied().zip(widths_px.into_iter()) {
                let em = w_px / base_font_size_px.max(1.0);
                if em.is_finite() && em >= 0.0 {
                    measured.insert(ch, em);
                }
            }

            let char_em: BTreeMap<char, f64> = measured.clone();
            let mut entries = measured.into_iter().collect::<Vec<_>>();
            entries.sort_by(|a, b| (a.0 as u32).cmp(&(b.0 as u32)));

            let mut for_default = entries
                .iter()
                .filter(|(ch, _)| !ch.is_whitespace())
                .map(|(_, v)| *v)
                .collect::<Vec<_>>();
            let default_em = median(&mut for_default).unwrap_or_else(|| {
                if entries.is_empty() {
                    0.6
                } else {
                    entries.iter().map(|(_, v)| *v).sum::<f64>() / entries.len() as f64
                }
            });

            let mut kern_pairs: Vec<(u32, u32, f64)> = Vec::new();
            if !pairset.is_empty() {
                let pairs = pairset.into_iter().collect::<Vec<_>>();
                let pair_strings = pairs
                    .iter()
                    .map(|(a, b)| format!("{a}{b}"))
                    .collect::<Vec<_>>();
                let widths_px = measure_text_widths_via_browser(
                    &node_cwd,
                    &browser_exe,
                    font_family,
                    base_font_size_px,
                    &pair_strings,
                )?;
                for ((a, b), w_px) in pairs.into_iter().zip(widths_px.into_iter()) {
                    let a_em = char_em.get(&a).copied().unwrap_or(default_em);
                    let b_em = char_em.get(&b).copied().unwrap_or(default_em);
                    let pair_em = w_px / base_font_size_px.max(1.0);
                    if !(pair_em.is_finite() && a_em.is_finite() && b_em.is_finite()) {
                        continue;
                    }
                    let adj = pair_em - a_em - b_em;
                    if adj.abs() > 1e-9 && adj.is_finite() {
                        kern_pairs.push((a as u32, b as u32, adj));
                    }
                }
                kern_pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            }

            // Measure internal-space adjustments for `a + ' ' + b`.
            //
            // In Chromium, normal spaces can have context-dependent spacing due to kerning around
            // spaces and because U+0020 and U+00A0 are not guaranteed to share the same advance.
            // We cannot learn this from 2-char strings like `"e "` / `" T"` because HTML collapses
            // leading/trailing spaces, so we measure 3-char strings with the space in the middle.
            let mut space_trigrams: Vec<(u32, u32, f64)> = Vec::new();
            {
                let mut trigram_set: std::collections::BTreeSet<(char, char)> =
                    std::collections::BTreeSet::new();
                for s in &ss {
                    let chars = s.text.chars().collect::<Vec<_>>();
                    if chars.len() < 3 {
                        continue;
                    }
                    for i in 1..(chars.len() - 1) {
                        if chars[i] != ' ' {
                            continue;
                        }
                        let a = chars[i - 1];
                        let b = chars[i + 1];
                        if a.is_whitespace() || b.is_whitespace() {
                            continue;
                        }
                        trigram_set.insert((a, b));
                    }
                }
                if !trigram_set.is_empty() {
                    let trigrams = trigram_set.into_iter().collect::<Vec<_>>();
                    let trigram_strings = trigrams
                        .iter()
                        .map(|(a, b)| format!("{a} {b}"))
                        .collect::<Vec<_>>();
                    let widths_px = measure_text_widths_via_browser(
                        &node_cwd,
                        &browser_exe,
                        font_family,
                        base_font_size_px,
                        &trigram_strings,
                    )?;
                    let space_em = char_em.get(&' ').copied().unwrap_or(default_em);
                    for ((a, b), w_px) in trigrams.into_iter().zip(widths_px.into_iter()) {
                        let a_em = char_em.get(&a).copied().unwrap_or(default_em);
                        let b_em = char_em.get(&b).copied().unwrap_or(default_em);
                        let trigram_em = w_px / base_font_size_px.max(1.0);
                        if !(trigram_em.is_finite()
                            && a_em.is_finite()
                            && space_em.is_finite()
                            && b_em.is_finite())
                        {
                            continue;
                        }
                        let adj = trigram_em - a_em - space_em - b_em;
                        if adj.abs() > 1e-9 && adj.is_finite() {
                            space_trigrams.push((a as u32, b as u32, adj));
                        }
                    }
                    space_trigrams.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
                }
            }

            // Measure residuals for 3-char (non-whitespace) trigrams `a + b + c`.
            //
            // Even after learning `kern_pairs`, Chromium's DOM width is not perfectly additive due
            // to subpixel positioning/hinting. Capturing the 3-char residual and applying it as a
            // local correction greatly improves parity for longer words.
            let mut trigrams: Vec<(u32, u32, u32, f64)> = Vec::new();
            {
                let mut trigram_set: std::collections::BTreeSet<(char, char, char)> =
                    std::collections::BTreeSet::new();
                for s in &ss {
                    let chars = s.text.chars().collect::<Vec<_>>();
                    if chars.len() < 3 {
                        continue;
                    }
                    for i in 1..(chars.len() - 1) {
                        let a = chars[i - 1];
                        let b = chars[i];
                        let c = chars[i + 1];
                        if a.is_whitespace() || b.is_whitespace() || c.is_whitespace() {
                            continue;
                        }
                        trigram_set.insert((a, b, c));
                    }
                }

                if !trigram_set.is_empty() {
                    let trigrams_keys = trigram_set.into_iter().collect::<Vec<_>>();
                    let trigram_strings = trigrams_keys
                        .iter()
                        .map(|(a, b, c)| format!("{a}{b}{c}"))
                        .collect::<Vec<_>>();
                    let widths_px = measure_text_widths_via_browser(
                        &node_cwd,
                        &browser_exe,
                        font_family,
                        base_font_size_px,
                        &trigram_strings,
                    )?;

                    let mut kern_map: std::collections::BTreeMap<(u32, u32), f64> =
                        std::collections::BTreeMap::new();
                    for (a, b, adj) in &kern_pairs {
                        kern_map.insert((*a, *b), *adj);
                    }

                    for ((a, b, c), w_px) in trigrams_keys.into_iter().zip(widths_px.into_iter()) {
                        let a_em = char_em.get(&a).copied().unwrap_or(default_em);
                        let b_em = char_em.get(&b).copied().unwrap_or(default_em);
                        let c_em = char_em.get(&c).copied().unwrap_or(default_em);
                        let trigram_em = w_px / base_font_size_px.max(1.0);
                        if !(trigram_em.is_finite()
                            && a_em.is_finite()
                            && b_em.is_finite()
                            && c_em.is_finite())
                        {
                            continue;
                        }
                        let ab_adj = kern_map.get(&(a as u32, b as u32)).copied().unwrap_or(0.0);
                        let bc_adj = kern_map.get(&(b as u32, c as u32)).copied().unwrap_or(0.0);

                        let adj = trigram_em - a_em - b_em - c_em - ab_adj - bc_adj;
                        if adj.abs() > 1e-9 && adj.is_finite() {
                            trigrams.push((a as u32, b as u32, c as u32, adj));
                        }
                    }
                    trigrams.sort_by(|a, b| {
                        a.0.cmp(&b.0)
                            .then_with(|| a.1.cmp(&b.1))
                            .then_with(|| a.2.cmp(&b.2))
                    });
                }
            }

            let mut html_overrides: Vec<(String, f64)> = Vec::new();
            {
                let mut by_text: BTreeMap<String, Vec<f64>> = BTreeMap::new();
                for s in &ss {
                    if !(s.width_px.is_finite() && s.width_px > 0.0) {
                        continue;
                    }
                    let em = s.width_px / s.font_size_px.max(1.0);
                    if em.is_finite() && em > 0.0 {
                        by_text.entry(s.text.clone()).or_default().push(em);
                    }
                }
                for (text, mut ems) in by_text {
                    let Some(m) = median(&mut ems) else {
                        continue;
                    };
                    if m.is_finite() && m > 0.0 {
                        html_overrides.push((text, m));
                    }
                }
                html_overrides.sort_by(|a, b| a.0.cmp(&b.0));
            }

            out.insert(
                font_key.clone(),
                FontTable {
                    font_key,
                    default_em: default_em.max(0.1),
                    entries,
                    kern_pairs,
                    space_trigrams,
                    trigrams,
                    html_overrides,
                    svg_overrides: Vec::new(),
                },
            );
        }
        Ok(out)
    }

    let html_tables = if matches!(backend.as_str(), "browser" | "puppeteer") {
        // Use both HTML and SVG title samples to build the kerning pair set. Titles dominate the
        // flowchart viewport width in many upstream fixtures, so missing title-specific kerning
        // pairs can skew `viewBox`/`max-width` parity.
        let mut canvas_samples_by_font = html_samples_by_font.clone();
        for (k, mut ss) in html_seed_samples_by_font.clone() {
            canvas_samples_by_font.entry(k).or_default().append(&mut ss);
        }
        for (k, mut ss) in svg_samples_by_font.clone() {
            canvas_samples_by_font.entry(k).or_default().append(&mut ss);
        }
        build_tables_via_browser(
            canvas_samples_by_font,
            &font_family_by_key,
            base_font_size_px,
            browser_exe.as_deref(),
        )?
    } else {
        fit_tables(html_samples_by_font, None)
    };

    fn lookup_char_em(entries: &[(char, f64)], default_em: f64, ch: char) -> f64 {
        let mut lo = 0usize;
        let mut hi = entries.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            match entries[mid].0.cmp(&ch) {
                std::cmp::Ordering::Equal => return entries[mid].1,
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        default_em
    }

    fn lookup_kern_em(kern_pairs: &[(u32, u32, f64)], a: char, b: char) -> f64 {
        let key_a = a as u32;
        let key_b = b as u32;
        let mut lo = 0usize;
        let mut hi = kern_pairs.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            let (ma, mb, v) = kern_pairs[mid];
            match (ma.cmp(&key_a), mb.cmp(&key_b)) {
                (std::cmp::Ordering::Equal, std::cmp::Ordering::Equal) => return v,
                (std::cmp::Ordering::Less, _) => lo = mid + 1,
                (std::cmp::Ordering::Equal, std::cmp::Ordering::Less) => lo = mid + 1,
                _ => hi = mid,
            }
        }
        0.0
    }

    fn lookup_space_trigram_em(space_trigrams: &[(u32, u32, f64)], a: char, b: char) -> f64 {
        let key_a = a as u32;
        let key_b = b as u32;
        let mut lo = 0usize;
        let mut hi = space_trigrams.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            let (ma, mb, v) = space_trigrams[mid];
            match (ma.cmp(&key_a), mb.cmp(&key_b)) {
                (std::cmp::Ordering::Equal, std::cmp::Ordering::Equal) => return v,
                (std::cmp::Ordering::Less, _) => lo = mid + 1,
                (std::cmp::Ordering::Equal, std::cmp::Ordering::Less) => lo = mid + 1,
                _ => hi = mid,
            }
        }
        0.0
    }

    fn line_width_px(
        entries: &[(char, f64)],
        default_em: f64,
        kern_pairs: &[(u32, u32, f64)],
        space_trigrams: &[(u32, u32, f64)],
        text: &str,
        font_size: f64,
    ) -> f64 {
        let mut em = 0.0;
        let mut prev: Option<char> = None;
        let mut it = text.chars().peekable();
        while let Some(ch) = it.next() {
            em += lookup_char_em(entries, default_em, ch);
            if let Some(p) = prev {
                em += lookup_kern_em(kern_pairs, p, ch);
            }
            if ch == ' ' {
                if let (Some(a), Some(&b)) = (prev, it.peek()) {
                    em += lookup_space_trigram_em(space_trigrams, a, b);
                }
            }
            prev = Some(ch);
        }
        em * font_size
    }

    // Derive a simple SVG text scaling factor from SVG text samples:
    // `svg_scale ≈ computedTextLength(svg_text) / width(canvas_measureText_model)`.
    //
    // Mermaid uses SVG text measurement heavily (wrapping, label layout). We keep this as a single
    // scale factor (instead of per-glyph corrections) to remain deterministic while still
    // converging on upstream DOM output.
    let mut svg_scales_by_font: BTreeMap<String, f64> = BTreeMap::new();
    for (font_key, ss) in &svg_samples_by_font {
        let Some(html) = html_tables.get(font_key) else {
            continue;
        };
        let mut scales: Vec<f64> = Vec::new();
        for s in ss {
            let pred_px = line_width_px(
                &html.entries,
                html.default_em.max(0.1),
                &html.kern_pairs,
                &html.space_trigrams,
                &s.text,
                s.font_size_px.max(1.0),
            );
            if !(pred_px.is_finite() && pred_px > 0.0) {
                continue;
            }
            let scale = s.width_px / pred_px;
            if scale.is_finite() && (0.5..=2.0).contains(&scale) {
                scales.push(scale);
            }
        }
        if let Some(m) = median(&mut scales) {
            svg_scales_by_font.insert(font_key.clone(), m.clamp(0.5, 2.0));
        }
    }

    // Derive first/last-character bbox overhangs (relative to the `text-anchor=middle` position)
    // from browser SVG metrics. This models the fact that SVG `getBBox()` can be asymmetric due to
    // glyph overhangs. Overhangs are stored in `em` and applied on top of scaled advances.
    type SvgBBoxOverhangs = (f64, f64, Vec<(char, f64)>, Vec<(char, f64)>);
    let mut svg_bbox_overhangs_by_font: BTreeMap<String, SvgBBoxOverhangs> = BTreeMap::new();
    let mut svg_overrides_by_font: BTreeMap<String, Vec<(String, f64, f64)>> = BTreeMap::new();
    if matches!(backend.as_str(), "browser" | "puppeteer") {
        let browser_exe = if let Some(p) = browser_exe.as_deref() {
            p.to_path_buf()
        } else if cfg!(windows) {
            detect_windows_browser_exe().ok_or_else(|| {
                XtaskError::SvgCompareFailed(
                    "no supported browser found for font measurement".into(),
                )
            })?
        } else {
            return Err(XtaskError::SvgCompareFailed(
                "browser measurement requires --browser-exe on this platform".into(),
            ));
        };
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");
        let node_cwd = workspace_root.join("tools").join("mermaid-cli");

        for (font_key, html) in &html_tables {
            let Some(font_family) = font_family_by_key.get(font_key) else {
                continue;
            };

            let mut chars = html.entries.iter().map(|(ch, _)| *ch).collect::<Vec<_>>();
            chars.sort_by_key(|ch| *ch as u32);
            chars.dedup();

            let strings = chars.iter().map(|ch| ch.to_string()).collect::<Vec<_>>();
            let metrics = measure_svg_text_bbox_metrics_via_browser(
                &node_cwd,
                &browser_exe,
                font_family,
                base_font_size_px.max(1.0),
                &strings,
            )?;

            let mut left_all: Vec<f64> = Vec::new();
            let mut right_all: Vec<f64> = Vec::new();
            let mut left_by_char: BTreeMap<char, f64> = BTreeMap::new();
            let mut right_by_char: BTreeMap<char, f64> = BTreeMap::new();
            for (ch, m) in chars.iter().copied().zip(metrics.into_iter()) {
                let adv_px = m.adv_px;
                let bbox_x = m.bbox_x;
                let bbox_w = m.bbox_w;
                if !(adv_px.is_finite()
                    && adv_px >= 0.0
                    && bbox_x.is_finite()
                    && bbox_w.is_finite())
                {
                    continue;
                }
                let left_extent = (-bbox_x).max(0.0);
                let right_extent = (bbox_x + bbox_w).max(0.0);
                let half = (adv_px / 2.0).max(0.0);
                let denom = base_font_size_px.max(1.0);
                let left_em = ((left_extent - half) / denom).clamp(-0.2, 0.2);
                let right_em = ((right_extent - half) / denom).clamp(-0.2, 0.2);
                left_all.push(left_em);
                right_all.push(right_em);
                left_by_char.insert(ch, left_em);
                right_by_char.insert(ch, right_em);
            }

            let default_left = median(&mut left_all).unwrap_or(0.0).clamp(-0.2, 0.2);
            let default_right = median(&mut right_all).unwrap_or(0.0).clamp(-0.2, 0.2);

            let mut left_entries: Vec<(char, f64)> = Vec::new();
            let mut right_entries: Vec<(char, f64)> = Vec::new();
            for (ch, v) in left_by_char {
                if (v - default_left).abs() > 1e-6 {
                    left_entries.push((ch, v));
                }
            }
            for (ch, v) in right_by_char {
                if (v - default_right).abs() > 1e-6 {
                    right_entries.push((ch, v));
                }
            }
            left_entries.sort_by_key(|(ch, _)| *ch as u32);
            right_entries.sort_by_key(|(ch, _)| *ch as u32);

            svg_bbox_overhangs_by_font.insert(
                font_key.clone(),
                (default_left, default_right, left_entries, right_entries),
            );
        }

        for (font_key, ss) in &svg_samples_by_font {
            let Some(font_family) = font_family_by_key.get(font_key) else {
                continue;
            };

            // Titles use a different font size (18px by default). SVG `getBBox()` can be
            // non-linear due to hinting, so measure overrides at the actual observed font size
            // and store them in `em` relative to that size.
            let base_size_key = (base_font_size_px.max(1.0) * 1000.0).round() as i64;
            let mut groups: BTreeMap<i64, Vec<String>> = BTreeMap::new();
            for s in ss {
                let size_key = (s.font_size_px.max(1.0) * 1000.0).round() as i64;
                groups.entry(size_key).or_default().push(s.text.clone());
            }

            let mut best_by_text: BTreeMap<String, (i64, f64, f64)> = BTreeMap::new();
            for (size_key, mut strings) in groups {
                strings.sort();
                strings.dedup();
                if strings.is_empty() {
                    continue;
                }

                let font_size_px = (size_key as f64) / 1000.0;
                let metrics = measure_svg_text_bbox_metrics_via_browser(
                    &node_cwd,
                    &browser_exe,
                    font_family,
                    font_size_px,
                    &strings,
                )?;
                let denom = font_size_px.max(1.0);

                for (text, m) in strings.into_iter().zip(metrics.into_iter()) {
                    let bbox_x = m.bbox_x;
                    let bbox_w = m.bbox_w;
                    if !(bbox_x.is_finite() && bbox_w.is_finite()) {
                        continue;
                    }
                    let left_px = (-bbox_x).max(0.0);
                    let right_px = (bbox_x + bbox_w).max(0.0);
                    let left_em = left_px / denom;
                    let right_em = right_px / denom;
                    if !(left_em.is_finite() && right_em.is_finite() && (left_em + right_em) > 0.0)
                    {
                        continue;
                    }

                    // If the same string appears at multiple sizes, prefer base size (16px)
                    // measurements since most SVG text in Mermaid flowcharts is at the diagram
                    // font size.
                    match best_by_text.get(&text) {
                        None => {
                            best_by_text.insert(text, (size_key, left_em, right_em));
                        }
                        Some((existing_size, _, _)) if *existing_size == base_size_key => {}
                        Some((existing_size, _, _)) if size_key == base_size_key => {
                            best_by_text.insert(text, (size_key, left_em, right_em));
                        }
                        Some(_) => {}
                    }
                }
            }

            let overrides = best_by_text
                .into_iter()
                .map(|(text, (_size, left_em, right_em))| (text, left_em, right_em))
                .collect::<Vec<_>>();
            svg_overrides_by_font.insert(font_key.clone(), overrides);
        }
    }

    type FontTableWithScaleAndOverhangs = (FontTable, f64, SvgBBoxOverhangs);
    let mut tables: Vec<FontTableWithScaleAndOverhangs> = Vec::new();
    for (font_key, mut t) in html_tables {
        if let Some(ov) = svg_overrides_by_font.get(&font_key).cloned() {
            t.svg_overrides = ov;
        }
        let scale = svg_scales_by_font.get(&font_key).copied().unwrap_or(1.0);
        let overhangs = svg_bbox_overhangs_by_font
            .get(&font_key)
            .cloned()
            .unwrap_or((0.0, 0.0, Vec::new(), Vec::new()));
        tables.push((t, scale, overhangs));
    }

    // Render as a deterministic Rust module.
    let mut out = String::new();
    fn rust_f64(v: f64) -> String {
        let mut s = format!("{v}");
        if !s.contains('.') && !s.contains('e') && !s.contains('E') {
            s.push_str(".0");
        }
        s
    }
    let _ = writeln!(&mut out, "// This file is generated by `xtask`.\n");
    let _ = writeln!(&mut out, "#[derive(Debug, Clone, Copy)]");
    let _ = writeln!(&mut out, "pub struct FontMetricsTables {{");
    let _ = writeln!(&mut out, "    pub font_key: &'static str,");
    let _ = writeln!(&mut out, "    pub base_font_size_px: f64,");
    let _ = writeln!(&mut out, "    pub default_em: f64,");
    let _ = writeln!(&mut out, "    pub entries: &'static [(char, f64)],");
    let _ = writeln!(&mut out, "    pub kern_pairs: &'static [(u32, u32, f64)],");
    let _ = writeln!(
        &mut out,
        "    pub space_trigrams: &'static [(u32, u32, f64)],"
    );
    let _ = writeln!(
        &mut out,
        "    pub trigrams: &'static [(u32, u32, u32, f64)],"
    );
    let _ = writeln!(
        &mut out,
        "    pub html_overrides: &'static [(&'static str, f64)],"
    );
    let _ = writeln!(
        &mut out,
        "    pub svg_overrides: &'static [(&'static str, f64, f64)],"
    );
    let _ = writeln!(&mut out, "    pub svg_scale: f64,");
    let _ = writeln!(&mut out, "    pub svg_bbox_overhang_left_default_em: f64,");
    let _ = writeln!(&mut out, "    pub svg_bbox_overhang_right_default_em: f64,");
    let _ = writeln!(
        &mut out,
        "    pub svg_bbox_overhang_left: &'static [(char, f64)],"
    );
    let _ = writeln!(
        &mut out,
        "    pub svg_bbox_overhang_right: &'static [(char, f64)],"
    );
    let _ = writeln!(&mut out, "}}\n");

    let _ = writeln!(
        &mut out,
        "pub const FONT_METRICS_TABLES: &[FontMetricsTables] = &["
    );
    for (t, svg_scale, (left_default, right_default, left_oh, right_oh)) in &tables {
        let _ = writeln!(
            &mut out,
            "    FontMetricsTables {{ font_key: {:?}, base_font_size_px: {}, default_em: {}, entries: &[",
            t.font_key,
            rust_f64(base_font_size_px),
            rust_f64(t.default_em)
        );
        for (ch, w) in &t.entries {
            let _ = writeln!(&mut out, "        ({:?}, {}),", ch, rust_f64(*w));
        }
        let _ = writeln!(&mut out, "    ], kern_pairs: &[");
        for (a, b, adj) in &t.kern_pairs {
            let _ = writeln!(&mut out, "        ({}, {}, {}),", a, b, rust_f64(*adj));
        }
        let _ = writeln!(&mut out, "    ], space_trigrams: &[");
        for (a, b, adj) in &t.space_trigrams {
            let _ = writeln!(&mut out, "        ({}, {}, {}),", a, b, rust_f64(*adj));
        }
        let _ = writeln!(&mut out, "    ], trigrams: &[");
        for (a, b, c, adj) in &t.trigrams {
            let _ = writeln!(
                &mut out,
                "        ({}, {}, {}, {}),",
                a,
                b,
                c,
                rust_f64(*adj)
            );
        }
        let _ = writeln!(&mut out, "    ], html_overrides: &[");
        for (text, em) in &t.html_overrides {
            let _ = writeln!(&mut out, "        ({:?}, {}),", text, rust_f64(*em));
        }
        let _ = writeln!(&mut out, "    ], svg_overrides: &[");
        for (text, left_em, right_em) in &t.svg_overrides {
            let _ = writeln!(
                &mut out,
                "        ({:?}, {}, {}),",
                text,
                rust_f64(*left_em),
                rust_f64(*right_em)
            );
        }
        let _ = writeln!(
            &mut out,
            "    ], svg_scale: {}, svg_bbox_overhang_left_default_em: {}, svg_bbox_overhang_right_default_em: {}, svg_bbox_overhang_left: &{:?}, svg_bbox_overhang_right: &{:?} }},\n",
            rust_f64(*svg_scale),
            rust_f64(*left_default),
            rust_f64(*right_default),
            left_oh,
            right_oh
        );
    }
    let _ = writeln!(&mut out, "];\n");

    let _ = writeln!(
        &mut out,
        "pub fn lookup_font_metrics(font_key: &str) -> Option<&'static FontMetricsTables> {{"
    );
    let _ = writeln!(&mut out, "    for t in FONT_METRICS_TABLES {{");
    let _ = writeln!(&mut out, "        if t.font_key == font_key {{");
    let _ = writeln!(&mut out, "            return Some(t);");
    let _ = writeln!(&mut out, "        }}");
    let _ = writeln!(&mut out, "    }}");
    let _ = writeln!(&mut out, "    None");
    let _ = writeln!(&mut out, "}}\n");

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, out).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}

fn debug_flowchart_svg_roots(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<String> = None;
    let mut upstream: Option<PathBuf> = None;
    let mut local: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.to_string());
            }
            "--upstream" => {
                i += 1;
                upstream = args.get(i).map(PathBuf::from);
            }
            "--local" => {
                i += 1;
                local = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    if let Some(f) = fixture.as_deref() {
        let upstream_default = workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("flowchart")
            .join(format!("{f}.svg"));
        let local_default = workspace_root
            .join("target")
            .join("compare")
            .join("flowchart")
            .join(format!("{f}.svg"));
        upstream = upstream.or(Some(upstream_default));
        local = local.or(Some(local_default));
    }

    let Some(upstream_path) = upstream else {
        return Err(XtaskError::Usage);
    };
    let Some(local_path) = local else {
        return Err(XtaskError::Usage);
    };

    let upstream_svg =
        fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
            path: upstream_path.display().to_string(),
            source,
        })?;
    let local_svg = fs::read_to_string(&local_path).map_err(|source| XtaskError::ReadFile {
        path: local_path.display().to_string(),
        source,
    })?;

    #[derive(Debug, Clone)]
    struct ClusterInfo {
        id: String,
        root_translate: Option<String>,
        rect_x: Option<String>,
        rect_y: Option<String>,
        rect_w: Option<String>,
        rect_h: Option<String>,
    }

    #[derive(Debug, Clone)]
    struct Summary {
        view_box: Option<String>,
        max_width: Option<String>,
        root_transforms: Vec<String>,
        clusters: Vec<ClusterInfo>,
    }

    fn parse_translate(transform: &str) -> Option<String> {
        // Keep the exact token payload inside `translate(...)` for readability.
        let t = transform.trim();
        let t = t.strip_prefix("translate(")?;
        let t = t.strip_suffix(')')?;
        Some(t.trim().to_string())
    }

    fn parse_summary(svg: &str) -> Result<Summary, String> {
        let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;
        let root = doc.root_element();
        let view_box = root.attribute("viewBox").map(|s| s.to_string());
        let max_width = root.attribute("style").and_then(|s| {
            // Extract `max-width: <n>px` when present.
            static RE: OnceLock<Regex> = OnceLock::new();
            let re = RE.get_or_init(|| Regex::new(r#"max-width:\s*([0-9.]+)px"#).unwrap());
            re.captures(s)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        });

        let mut root_transforms: Vec<String> = Vec::new();
        let mut clusters: Vec<ClusterInfo> = Vec::new();

        for n in doc.descendants().filter(|n| n.is_element()) {
            if n.tag_name().name() == "g" {
                if let Some(class) = n.attribute("class") {
                    if class.split_whitespace().any(|t| t == "root") {
                        if let Some(transform) = n.attribute("transform") {
                            if let Some(t) = parse_translate(transform) {
                                root_transforms.push(t);
                            }
                        }
                    }
                    if class.split_whitespace().any(|t| t == "cluster") {
                        if let Some(id) = n.attribute("id") {
                            let mut root_translate: Option<String> = None;
                            for a in n.ancestors() {
                                if !a.is_element() || a.tag_name().name() != "g" {
                                    continue;
                                }
                                let Some(class) = a.attribute("class") else {
                                    continue;
                                };
                                if !class.split_whitespace().any(|t| t == "root") {
                                    continue;
                                }
                                let Some(transform) = a.attribute("transform") else {
                                    continue;
                                };
                                root_translate = parse_translate(transform);
                                break;
                            }

                            let rect = n
                                .children()
                                .find(|c| c.is_element() && c.tag_name().name() == "rect");
                            let rect_x = rect.and_then(|r| r.attribute("x")).map(|s| s.to_string());
                            let rect_y = rect.and_then(|r| r.attribute("y")).map(|s| s.to_string());
                            let rect_w = rect
                                .and_then(|r| r.attribute("width"))
                                .map(|s| s.to_string());
                            let rect_h = rect
                                .and_then(|r| r.attribute("height"))
                                .map(|s| s.to_string());

                            clusters.push(ClusterInfo {
                                id: id.to_string(),
                                root_translate,
                                rect_x,
                                rect_y,
                                rect_w,
                                rect_h,
                            });
                        }
                    }
                }
            }
        }

        root_transforms.sort();
        root_transforms.dedup();
        clusters.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(Summary {
            view_box,
            max_width,
            root_transforms,
            clusters,
        })
    }

    let upstream_summary = parse_summary(&upstream_svg).map_err(XtaskError::DebugSvgFailed)?;
    let local_summary = parse_summary(&local_svg).map_err(XtaskError::DebugSvgFailed)?;

    println!("upstream: {}", upstream_path.display());
    println!("local:    {}", local_path.display());
    println!();

    println!("== Root SVG ==");
    println!(
        "upstream viewBox: {:?}",
        upstream_summary.view_box.as_deref().unwrap_or("<missing>")
    );
    println!(
        "local    viewBox: {:?}",
        local_summary.view_box.as_deref().unwrap_or("<missing>")
    );
    println!(
        "upstream max-width(px): {:?}",
        upstream_summary.max_width.as_deref().unwrap_or("<missing>")
    );
    println!(
        "local    max-width(px): {:?}",
        local_summary.max_width.as_deref().unwrap_or("<missing>")
    );
    println!();

    println!("== <g class=\"root\" transform=\"translate(...)\"> ==");
    println!("upstream:");
    for t in &upstream_summary.root_transforms {
        println!("- {t}");
    }
    println!("local:");
    for t in &local_summary.root_transforms {
        println!("- {t}");
    }
    println!();

    println!("== Clusters ==");
    fn fmt_cluster(c: &ClusterInfo) -> String {
        format!(
            "id={} root={:?} rect=({:?}, {:?}, {:?}, {:?})",
            c.id, c.root_translate, c.rect_x, c.rect_y, c.rect_w, c.rect_h
        )
    }
    let mut upstream_by_id: std::collections::HashMap<&str, &ClusterInfo> =
        std::collections::HashMap::new();
    for c in &upstream_summary.clusters {
        upstream_by_id.insert(c.id.as_str(), c);
    }
    for c in &local_summary.clusters {
        let up = upstream_by_id.get(c.id.as_str()).copied();
        if let Some(up) = up {
            if up.root_translate != c.root_translate
                || up.rect_w != c.rect_w
                || up.rect_h != c.rect_h
                || up.rect_x != c.rect_x
                || up.rect_y != c.rect_y
            {
                println!("upstream: {}", fmt_cluster(up));
                println!("local:    {}", fmt_cluster(c));
            }
        } else {
            println!("local-only: {}", fmt_cluster(c));
        }
    }
    for c in &upstream_summary.clusters {
        if !local_summary.clusters.iter().any(|l| l.id == c.id) {
            println!("upstream-only: {}", fmt_cluster(c));
        }
    }

    Ok(())
}

fn debug_flowchart_svg_positions(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<String> = None;
    let mut upstream: Option<PathBuf> = None;
    let mut local: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.to_string());
            }
            "--upstream" => {
                i += 1;
                upstream = args.get(i).map(PathBuf::from);
            }
            "--local" => {
                i += 1;
                local = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    if let Some(f) = fixture.as_deref() {
        let upstream_default = workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("flowchart")
            .join(format!("{f}.svg"));
        let local_default = workspace_root
            .join("target")
            .join("compare")
            .join("flowchart")
            .join(format!("{f}.svg"));
        upstream = upstream.or(Some(upstream_default));
        local = local.or(Some(local_default));
    }

    let Some(upstream_path) = upstream else {
        return Err(XtaskError::Usage);
    };
    let Some(local_path) = local else {
        return Err(XtaskError::Usage);
    };

    let upstream_svg =
        fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
            path: upstream_path.display().to_string(),
            source,
        })?;
    let local_svg = fs::read_to_string(&local_path).map_err(|source| XtaskError::ReadFile {
        path: local_path.display().to_string(),
        source,
    })?;

    #[derive(Debug, Clone, Copy)]
    struct Translate {
        x: f64,
        y: f64,
    }

    fn parse_translate(transform: &str) -> Option<Translate> {
        let t = transform.trim();
        let t = t.strip_prefix("translate(")?;
        let t = t.strip_suffix(')')?;
        let parts = t
            .split(|ch: char| ch == ',' || ch.is_whitespace())
            .filter(|s| !s.trim().is_empty())
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect::<Vec<_>>();
        match parts.as_slice() {
            [x, y] => Some(Translate { x: *x, y: *y }),
            [x] => Some(Translate { x: *x, y: 0.0 }),
            _ => None,
        }
    }

    fn accumulated_translate(node: roxmltree::Node<'_, '_>) -> Translate {
        let mut x = 0.0;
        let mut y = 0.0;
        // `ancestors()` includes the node itself; we want the sum of parent transforms only.
        for n in node.ancestors().filter(|n| n.is_element()).skip(1) {
            if let Some(transform) = n.attribute("transform") {
                if let Some(t) = parse_translate(transform) {
                    x += t.x;
                    y += t.y;
                }
            }
        }
        Translate { x, y }
    }

    #[derive(Debug, Clone)]
    struct NodePos {
        kind: &'static str,
        x: f64,
        y: f64,
    }

    #[derive(Debug, Clone)]
    struct ClusterRect {
        left: f64,
        top: f64,
        w: f64,
        h: f64,
    }

    type PositionsAndClusters = (BTreeMap<String, NodePos>, BTreeMap<String, ClusterRect>);

    fn parse_positions(svg: &str) -> Result<PositionsAndClusters, String> {
        let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;

        let mut nodes: BTreeMap<String, NodePos> = BTreeMap::new();
        let mut clusters: BTreeMap<String, ClusterRect> = BTreeMap::new();

        for n in doc.descendants().filter(|n| n.is_element()) {
            if n.tag_name().name() != "g" {
                continue;
            }
            let Some(id) = n.attribute("id") else {
                continue;
            };
            let class = n.attribute("class").unwrap_or_default();
            let class_tokens = class.split_whitespace().collect::<Vec<_>>();

            if class_tokens.contains(&"node") {
                let Some(transform) = n.attribute("transform") else {
                    continue;
                };
                let Some(local) = parse_translate(transform) else {
                    continue;
                };
                let abs = accumulated_translate(n);
                nodes.insert(
                    id.to_string(),
                    NodePos {
                        kind: "node",
                        x: local.x + abs.x,
                        y: local.y + abs.y,
                    },
                );
                continue;
            }

            // Mermaid self-loop helper nodes use `<g class="label edgeLabel" id="X---X---1" transform="translate(...)">`.
            if class_tokens.contains(&"edgeLabel") && class_tokens.contains(&"label") {
                let Some(transform) = n.attribute("transform") else {
                    continue;
                };
                let Some(local) = parse_translate(transform) else {
                    continue;
                };
                let abs = accumulated_translate(n);
                nodes.insert(
                    id.to_string(),
                    NodePos {
                        kind: "labelRect",
                        x: local.x + abs.x,
                        y: local.y + abs.y,
                    },
                );
                continue;
            }

            if class_tokens.contains(&"cluster") {
                let abs = accumulated_translate(n);
                let rect = n
                    .children()
                    .find(|c| c.is_element() && c.tag_name().name() == "rect");
                let Some(rect) = rect else {
                    continue;
                };
                let x = rect
                    .attribute("x")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let y = rect
                    .attribute("y")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let w = rect
                    .attribute("width")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let h = rect
                    .attribute("height")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                clusters.insert(
                    id.to_string(),
                    ClusterRect {
                        left: abs.x + x,
                        top: abs.y + y,
                        w,
                        h,
                    },
                );
            }
        }

        Ok((nodes, clusters))
    }

    let (up_nodes, up_clusters) =
        parse_positions(&upstream_svg).map_err(XtaskError::DebugSvgFailed)?;
    let (lo_nodes, lo_clusters) =
        parse_positions(&local_svg).map_err(XtaskError::DebugSvgFailed)?;

    println!("upstream: {}", upstream_path.display());
    println!("local:    {}", local_path.display());
    println!();

    println!("== Nodes / LabelRects (abs translate) ==");
    let mut node_ids: Vec<&String> = up_nodes.keys().collect();
    node_ids.sort();
    for id in node_ids {
        let Some(a) = up_nodes.get(id) else { continue };
        let Some(b) = lo_nodes.get(id) else { continue };
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        if dx.abs() < 1e-6 && dy.abs() < 1e-6 {
            continue;
        }
        println!(
            "{id} kind={} upstream=({:.6},{:.6}) local=({:.6},{:.6}) Δ=({:.6},{:.6})",
            a.kind, a.x, a.y, b.x, b.y, dx, dy
        );
    }
    for (id, b) in &lo_nodes {
        if !up_nodes.contains_key(id) {
            println!("{id} local-only kind={} ({:.6},{:.6})", b.kind, b.x, b.y);
        }
    }
    for (id, a) in &up_nodes {
        if !lo_nodes.contains_key(id) {
            println!("{id} upstream-only kind={} ({:.6},{:.6})", a.kind, a.x, a.y);
        }
    }
    println!();

    println!("== Clusters (abs rect) ==");
    let mut cluster_ids: Vec<&String> = up_clusters.keys().collect();
    cluster_ids.sort();
    for id in cluster_ids {
        let Some(a) = up_clusters.get(id) else {
            continue;
        };
        let Some(b) = lo_clusters.get(id) else {
            continue;
        };
        let dx = b.left - a.left;
        let dy = b.top - a.top;
        let dw = b.w - a.w;
        let dh = b.h - a.h;
        if dx.abs() < 1e-6 && dy.abs() < 1e-6 && dw.abs() < 1e-6 && dh.abs() < 1e-6 {
            continue;
        }
        println!(
            "{id} upstream=({:.6},{:.6},{:.6},{:.6}) local=({:.6},{:.6},{:.6},{:.6}) Δ=({:.6},{:.6},{:.6},{:.6})",
            a.left, a.top, a.w, a.h, b.left, b.top, b.w, b.h, dx, dy, dw, dh
        );
    }
    for (id, b) in &lo_clusters {
        if !up_clusters.contains_key(id) {
            println!(
                "{id} local-only ({:.6},{:.6},{:.6},{:.6})",
                b.left, b.top, b.w, b.h
            );
        }
    }
    for (id, a) in &up_clusters {
        if !lo_clusters.contains_key(id) {
            println!(
                "{id} upstream-only ({:.6},{:.6},{:.6},{:.6})",
                a.left, a.top, a.w, a.h
            );
        }
    }

    Ok(())
}

fn debug_mindmap_svg_positions(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<String> = None;
    let mut upstream: Option<PathBuf> = None;
    let mut local: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.to_string());
            }
            "--upstream" => {
                i += 1;
                upstream = args.get(i).map(PathBuf::from);
            }
            "--local" => {
                i += 1;
                local = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    if let Some(f) = fixture.as_deref() {
        let upstream_default = workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("mindmap")
            .join(format!("{f}.svg"));
        let local_default = workspace_root
            .join("target")
            .join("compare")
            .join("mindmap")
            .join(format!("{f}.svg"));
        upstream = upstream.or(Some(upstream_default));
        local = local.or(Some(local_default));
    }

    let Some(upstream_path) = upstream else {
        return Err(XtaskError::Usage);
    };
    let Some(local_path) = local else {
        return Err(XtaskError::Usage);
    };

    let upstream_svg =
        fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
            path: upstream_path.display().to_string(),
            source,
        })?;
    let local_svg = fs::read_to_string(&local_path).map_err(|source| XtaskError::ReadFile {
        path: local_path.display().to_string(),
        source,
    })?;

    #[derive(Debug, Clone, Copy)]
    struct Translate {
        x: f64,
        y: f64,
    }

    fn parse_translate(transform: &str) -> Option<Translate> {
        let t = transform.trim();
        let t = t.strip_prefix("translate(")?;
        let t = t.strip_suffix(')')?;
        let parts = t
            .split(|ch: char| ch == ',' || ch.is_whitespace())
            .filter(|s| !s.trim().is_empty())
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect::<Vec<_>>();
        match parts.as_slice() {
            [x, y] => Some(Translate { x: *x, y: *y }),
            [x] => Some(Translate { x: *x, y: 0.0 }),
            _ => None,
        }
    }

    fn accumulated_translate(node: roxmltree::Node<'_, '_>) -> Translate {
        let mut x = 0.0;
        let mut y = 0.0;
        for n in node.ancestors().filter(|n| n.is_element()).skip(1) {
            if let Some(transform) = n.attribute("transform") {
                if let Some(t) = parse_translate(transform) {
                    x += t.x;
                    y += t.y;
                }
            }
        }
        Translate { x, y }
    }

    #[derive(Debug, Clone)]
    struct RootInfo {
        view_box: Option<String>,
        max_width: Option<String>,
    }

    #[derive(Debug, Clone)]
    struct NodePos {
        id: String,
        class: String,
        x: f64,
        y: f64,
    }

    fn parse_root_info(svg: &str) -> Result<RootInfo, String> {
        let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;
        let root = doc.root_element();
        let view_box = root.attribute("viewBox").map(|s| s.to_string());
        let max_width = root.attribute("style").and_then(|s| {
            static RE: OnceLock<Regex> = OnceLock::new();
            let re = RE.get_or_init(|| Regex::new(r#"max-width:\s*([0-9.]+)px"#).unwrap());
            re.captures(s)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        });
        Ok(RootInfo {
            view_box,
            max_width,
        })
    }

    fn parse_node_positions(svg: &str) -> Result<Vec<NodePos>, String> {
        let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;
        let mut out: Vec<NodePos> = Vec::new();

        for n in doc.descendants().filter(|n| n.is_element()) {
            if n.tag_name().name() != "g" {
                continue;
            }
            let Some(id) = n.attribute("id") else {
                continue;
            };
            if !id.starts_with("node_") {
                continue;
            }
            let Some(class) = n.attribute("class") else {
                continue;
            };
            if !class.split_whitespace().any(|t| t == "node") {
                continue;
            }
            let Some(transform) = n.attribute("transform") else {
                continue;
            };
            let Some(local) = parse_translate(transform) else {
                continue;
            };
            let abs = accumulated_translate(n);
            out.push(NodePos {
                id: id.to_string(),
                class: class.to_string(),
                x: local.x + abs.x,
                y: local.y + abs.y,
            });
        }

        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    let up_root = parse_root_info(&upstream_svg).map_err(XtaskError::DebugSvgFailed)?;
    let lo_root = parse_root_info(&local_svg).map_err(XtaskError::DebugSvgFailed)?;
    let up_nodes = parse_node_positions(&upstream_svg).map_err(XtaskError::DebugSvgFailed)?;
    let lo_nodes = parse_node_positions(&local_svg).map_err(XtaskError::DebugSvgFailed)?;

    println!("upstream: {}", upstream_path.display());
    println!("local:    {}", local_path.display());
    println!();

    println!("== Root SVG ==");
    println!(
        "upstream viewBox: {:?}",
        up_root.view_box.as_deref().unwrap_or("<missing>")
    );
    println!(
        "local    viewBox: {:?}",
        lo_root.view_box.as_deref().unwrap_or("<missing>")
    );
    println!(
        "upstream max-width(px): {:?}",
        up_root.max_width.as_deref().unwrap_or("<missing>")
    );
    println!(
        "local    max-width(px): {:?}",
        lo_root.max_width.as_deref().unwrap_or("<missing>")
    );
    println!();

    println!("== Nodes ==");
    println!("upstream nodes: {}", up_nodes.len());
    println!("local nodes:    {}", lo_nodes.len());
    println!();

    let mut up_by_id: std::collections::BTreeMap<&str, &NodePos> =
        std::collections::BTreeMap::new();
    for n in &up_nodes {
        up_by_id.insert(n.id.as_str(), n);
    }
    let mut lo_by_id: std::collections::BTreeMap<&str, &NodePos> =
        std::collections::BTreeMap::new();
    for n in &lo_nodes {
        lo_by_id.insert(n.id.as_str(), n);
    }

    for (id, up) in &up_by_id {
        let lo = lo_by_id.get(id).copied();
        match lo {
            Some(lo) => {
                if up.x != lo.x || up.y != lo.y || up.class != lo.class {
                    println!("id={id}");
                    println!("  upstream: ({:.6}, {:.6}) class={}", up.x, up.y, up.class);
                    println!("  local:    ({:.6}, {:.6}) class={}", lo.x, lo.y, lo.class);
                }
            }
            None => println!("upstream-only: {id}"),
        }
    }
    for id in lo_by_id.keys() {
        if !up_by_id.contains_key(id) {
            println!("local-only: {id}");
        }
    }

    Ok(())
}

fn debug_flowchart_svg_diff(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<String> = None;
    let mut upstream: Option<PathBuf> = None;
    let mut local: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut min_abs_delta: f64 = 0.5;
    let mut max_rows: usize = 50;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.to_string());
            }
            "--upstream" => {
                i += 1;
                upstream = args.get(i).map(PathBuf::from);
            }
            "--local" => {
                i += 1;
                local = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--min-abs-delta" => {
                i += 1;
                min_abs_delta = args
                    .get(i)
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.5);
            }
            "--max" => {
                i += 1;
                max_rows = args
                    .get(i)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(50);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    if let Some(f) = fixture.as_deref() {
        let upstream_default = workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("flowchart")
            .join(format!("{f}.svg"));
        let local_default = workspace_root
            .join("target")
            .join("compare")
            .join("flowchart")
            .join(format!("{f}.svg"));
        upstream = upstream.or(Some(upstream_default));
        local = local.or(Some(local_default));
    }

    let Some(upstream_path) = upstream else {
        return Err(XtaskError::Usage);
    };
    let Some(local_path) = local else {
        return Err(XtaskError::Usage);
    };

    let upstream_svg =
        fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
            path: upstream_path.display().to_string(),
            source,
        })?;
    let local_svg = fs::read_to_string(&local_path).map_err(|source| XtaskError::ReadFile {
        path: local_path.display().to_string(),
        source,
    })?;

    #[derive(Debug, Clone, Copy)]
    struct Translate {
        x: f64,
        y: f64,
    }

    fn parse_translate(transform: &str) -> Option<Translate> {
        let t = transform.trim();
        let t = t.strip_prefix("translate(")?;
        let t = t.strip_suffix(')')?;
        let parts = t
            .split(|ch: char| ch == ',' || ch.is_whitespace())
            .filter(|s| !s.trim().is_empty())
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect::<Vec<_>>();
        match parts.as_slice() {
            [x, y] => Some(Translate { x: *x, y: *y }),
            [x] => Some(Translate { x: *x, y: 0.0 }),
            _ => None,
        }
    }

    fn accumulated_translate_including_self(node: roxmltree::Node<'_, '_>) -> Translate {
        let mut x = 0.0;
        let mut y = 0.0;
        for n in node.ancestors().filter(|n| n.is_element()) {
            if let Some(transform) = n.attribute("transform") {
                if let Some(t) = parse_translate(transform) {
                    x += t.x;
                    y += t.y;
                }
            }
        }
        Translate { x, y }
    }

    #[derive(Debug, Clone)]
    struct NodePos {
        kind: &'static str,
        x: f64,
        y: f64,
    }

    #[derive(Debug, Clone)]
    struct ClusterRect {
        left: f64,
        top: f64,
        w: f64,
        h: f64,
    }

    #[derive(Debug, Clone, Copy)]
    struct BBox {
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
    }

    impl BBox {
        fn width(&self) -> f64 {
            self.max_x - self.min_x
        }
        fn height(&self) -> f64 {
            self.max_y - self.min_y
        }
    }

    #[derive(Debug, Clone)]
    struct EdgePoints {
        tx: f64,
        ty: f64,
        points: Vec<(f64, f64)>,
        bbox: Option<BBox>,
        abs_bbox: Option<BBox>,
    }

    fn decode_data_points(dp: &str) -> Option<Vec<(f64, f64)>> {
        use base64::Engine as _;
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(dp.as_bytes()) else {
            return None;
        };
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            return None;
        };
        let arr = v.as_array()?;
        let mut out: Vec<(f64, f64)> = Vec::with_capacity(arr.len());
        for p in arr {
            let (Some(x), Some(y)) = (
                p.get("x").and_then(|v| v.as_f64()),
                p.get("y").and_then(|v| v.as_f64()),
            ) else {
                continue;
            };
            if !(x.is_finite() && y.is_finite()) {
                continue;
            }
            out.push((x, y));
        }
        Some(out)
    }

    fn bbox_of_points(points: &[(f64, f64)]) -> Option<BBox> {
        if points.is_empty() {
            return None;
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for (x, y) in points {
            min_x = min_x.min(*x);
            min_y = min_y.min(*y);
            max_x = max_x.max(*x);
            max_y = max_y.max(*y);
        }
        if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
            Some(BBox {
                min_x,
                min_y,
                max_x,
                max_y,
            })
        } else {
            None
        }
    }

    fn parse_root_viewport(svg: &str) -> Result<(Option<String>, Option<String>), String> {
        let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;
        let root = doc.root_element();
        let view_box = root.attribute("viewBox").map(|s| s.to_string());
        let max_width = root.attribute("style").and_then(|s| {
            static RE: OnceLock<Regex> = OnceLock::new();
            let re = RE.get_or_init(|| Regex::new(r#"max-width:\s*([0-9.]+)px"#).unwrap());
            re.captures(s)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        });
        Ok((view_box, max_width))
    }

    type PositionsAndEdges = (
        BTreeMap<String, NodePos>,
        BTreeMap<String, ClusterRect>,
        BTreeMap<String, EdgePoints>,
        Vec<String>,
    );

    fn parse_positions_and_edges(svg: &str) -> Result<PositionsAndEdges, String> {
        let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;

        let mut nodes: BTreeMap<String, NodePos> = BTreeMap::new();
        let mut clusters: BTreeMap<String, ClusterRect> = BTreeMap::new();
        let mut edges: BTreeMap<String, EdgePoints> = BTreeMap::new();
        let mut root_transforms: Vec<String> = Vec::new();

        for n in doc.descendants().filter(|n| n.is_element()) {
            if n.tag_name().name() == "g" {
                if let Some(class) = n.attribute("class") {
                    if class.split_whitespace().any(|t| t == "root") {
                        if let Some(transform) = n.attribute("transform") {
                            if let Some(t) = transform
                                .trim()
                                .strip_prefix("translate(")
                                .and_then(|s| s.strip_suffix(')'))
                            {
                                root_transforms.push(t.trim().to_string());
                            }
                        }
                    }
                }
            }

            if n.tag_name().name() == "g" {
                let Some(id) = n.attribute("id") else {
                    continue;
                };
                let class = n.attribute("class").unwrap_or_default();
                let class_tokens = class.split_whitespace().collect::<Vec<_>>();

                if class_tokens.contains(&"node") {
                    let abs = accumulated_translate_including_self(n);
                    nodes.insert(
                        id.to_string(),
                        NodePos {
                            kind: "node",
                            x: abs.x,
                            y: abs.y,
                        },
                    );
                    continue;
                }

                // Mermaid self-loop helper nodes use `<g class="label edgeLabel" id="X---X---1" transform="translate(...)">`.
                if class_tokens.contains(&"edgeLabel") && class_tokens.contains(&"label") {
                    let abs = accumulated_translate_including_self(n);
                    nodes.insert(
                        id.to_string(),
                        NodePos {
                            kind: "labelRect",
                            x: abs.x,
                            y: abs.y,
                        },
                    );
                    continue;
                }

                if class_tokens.contains(&"cluster") {
                    let abs = accumulated_translate_including_self(n);
                    let rect = n
                        .children()
                        .find(|c| c.is_element() && c.tag_name().name() == "rect");
                    let Some(rect) = rect else {
                        continue;
                    };
                    let x = rect
                        .attribute("x")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    let y = rect
                        .attribute("y")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    let w = rect
                        .attribute("width")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    let h = rect
                        .attribute("height")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    clusters.insert(
                        id.to_string(),
                        ClusterRect {
                            left: abs.x + x,
                            top: abs.y + y,
                            w,
                            h,
                        },
                    );
                }
            }

            if n.tag_name().name() == "path" {
                if n.attribute("data-edge").is_none_or(|v| v != "true") {
                    continue;
                }
                let Some(edge_id) = n.attribute("data-id") else {
                    continue;
                };
                let Some(dp) = n.attribute("data-points") else {
                    continue;
                };
                let Some(points) = decode_data_points(dp) else {
                    continue;
                };
                let abs = accumulated_translate_including_self(n);
                let bbox = bbox_of_points(&points);
                let abs_bbox = bbox.map(|b| BBox {
                    min_x: b.min_x + abs.x,
                    max_x: b.max_x + abs.x,
                    min_y: b.min_y + abs.y,
                    max_y: b.max_y + abs.y,
                });
                edges.insert(
                    edge_id.to_string(),
                    EdgePoints {
                        tx: abs.x,
                        ty: abs.y,
                        points,
                        bbox,
                        abs_bbox,
                    },
                );
            }
        }

        root_transforms.sort();
        root_transforms.dedup();
        Ok((nodes, clusters, edges, root_transforms))
    }

    let (up_viewbox, up_maxw) =
        parse_root_viewport(&upstream_svg).map_err(XtaskError::DebugSvgFailed)?;
    let (lo_viewbox, lo_maxw) =
        parse_root_viewport(&local_svg).map_err(XtaskError::DebugSvgFailed)?;

    let (up_nodes, up_clusters, up_edges, up_roots) =
        parse_positions_and_edges(&upstream_svg).map_err(XtaskError::DebugSvgFailed)?;
    let (lo_nodes, lo_clusters, lo_edges, lo_roots) =
        parse_positions_and_edges(&local_svg).map_err(XtaskError::DebugSvgFailed)?;

    println!("upstream: {}", upstream_path.display());
    println!("local:    {}", local_path.display());
    println!();

    println!("== Root SVG ==");
    println!(
        "upstream viewBox: {:?}",
        up_viewbox.as_deref().unwrap_or("<missing>")
    );
    println!(
        "local    viewBox: {:?}",
        lo_viewbox.as_deref().unwrap_or("<missing>")
    );
    println!(
        "upstream max-width(px): {:?}",
        up_maxw.as_deref().unwrap_or("<missing>")
    );
    println!(
        "local    max-width(px): {:?}",
        lo_maxw.as_deref().unwrap_or("<missing>")
    );
    println!(
        "counts: nodes={} clusters={} edges={}",
        up_nodes.len().min(lo_nodes.len()),
        up_clusters.len().min(lo_clusters.len()),
        up_edges.len().min(lo_edges.len())
    );
    println!();

    println!("== Root group transforms ==");
    println!("upstream:");
    for t in &up_roots {
        println!("- {t}");
    }
    println!("local:");
    for t in &lo_roots {
        println!("- {t}");
    }
    println!();

    fn keep_id(id: &str, filter: &Option<String>) -> bool {
        filter.as_deref().map(|f| id.contains(f)).unwrap_or(true)
    }

    println!("== Nodes / LabelRects (abs translate) ==");
    let mut node_rows: Vec<(f64, String)> = Vec::new();
    for (id, up) in &up_nodes {
        if !keep_id(id, &filter) {
            continue;
        }
        let Some(lo) = lo_nodes.get(id) else {
            continue;
        };
        let dx = lo.x - up.x;
        let dy = lo.y - up.y;
        let score = (dx * dx + dy * dy).sqrt();
        if score >= min_abs_delta {
            node_rows.push((
                score,
                format!(
                    "{id} kind={} upstream=({:.3},{:.3}) local=({:.3},{:.3}) Δ=({:.3},{:.3})",
                    up.kind, up.x, up.y, lo.x, lo.y, dx, dy
                ),
            ));
        }
    }
    node_rows.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .reverse()
    });
    for (_, line) in node_rows.into_iter().take(max_rows) {
        println!("{line}");
    }
    println!();

    println!("== Clusters (abs rect) ==");
    let mut cluster_rows: Vec<(f64, String)> = Vec::new();
    for (id, up) in &up_clusters {
        if !keep_id(id, &filter) {
            continue;
        }
        let Some(lo) = lo_clusters.get(id) else {
            continue;
        };
        let dl = lo.left - up.left;
        let dt = lo.top - up.top;
        let dw = lo.w - up.w;
        let dh = lo.h - up.h;
        let score = dl.abs().max(dt.abs()).max(dw.abs()).max(dh.abs());
        if score >= min_abs_delta {
            cluster_rows.push((
                score,
                format!(
                    "{id} upstream=({:.3},{:.3},{:.3},{:.3}) local=({:.3},{:.3},{:.3},{:.3}) Δ=({:.3},{:.3},{:.3},{:.3})",
                    up.left, up.top, up.w, up.h,
                    lo.left, lo.top, lo.w, lo.h,
                    dl, dt, dw, dh
                ),
            ));
        }
    }
    cluster_rows.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .reverse()
    });
    for (_, line) in cluster_rows.into_iter().take(max_rows) {
        println!("{line}");
    }
    println!();

    println!("== Edges (data-points bbox/translate) ==");
    let mut edge_rows: Vec<(f64, String)> = Vec::new();
    for (id, up) in &up_edges {
        if !keep_id(id, &filter) {
            continue;
        }
        let Some(lo) = lo_edges.get(id) else {
            continue;
        };
        let dtx = lo.tx - up.tx;
        let dty = lo.ty - up.ty;
        let mut score = dtx.abs().max(dty.abs());

        let mut detail = String::new();
        if up.points.len() != lo.points.len() {
            detail.push_str(&format!(
                " points_len upstream={} local={}",
                up.points.len(),
                lo.points.len()
            ));
        }

        if let (Some(ub), Some(lb), Some(uab), Some(lab)) =
            (up.bbox, lo.bbox, up.abs_bbox, lo.abs_bbox)
        {
            let dw = lb.width() - ub.width();
            let dh = lb.height() - ub.height();
            let dminx = lab.min_x - uab.min_x;
            let dmaxx = lab.max_x - uab.max_x;
            let dminy = lab.min_y - uab.min_y;
            let dmaxy = lab.max_y - uab.max_y;
            score = score
                .max(dw.abs())
                .max(dh.abs())
                .max(dminx.abs())
                .max(dmaxx.abs())
                .max(dminy.abs())
                .max(dmaxy.abs());
            detail.push_str(&format!(
                " abs_bbox upstream=({:.3},{:.3},{:.3},{:.3}) local=({:.3},{:.3},{:.3},{:.3}) Δ=({:.3},{:.3},{:.3},{:.3}) sizeΔ=({:.3},{:.3})",
                uab.min_x, uab.min_y, uab.max_x, uab.max_y,
                lab.min_x, lab.min_y, lab.max_x, lab.max_y,
                dminx, dminy, dmaxx, dmaxy,
                dw, dh
            ));
        }

        if score < min_abs_delta {
            continue;
        }

        edge_rows.push((score, format!("{id} Δt=({:.3},{:.3}){detail}", dtx, dty)));
    }
    edge_rows.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .reverse()
    });
    for (_, line) in edge_rows.into_iter().take(max_rows) {
        println!("{line}");
    }

    Ok(())
}

fn debug_flowchart_data_points(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<String> = None;
    let mut upstream: Option<PathBuf> = None;
    let mut local: Option<PathBuf> = None;
    let mut edge_id: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.to_string());
            }
            "--upstream" => {
                i += 1;
                upstream = args.get(i).map(PathBuf::from);
            }
            "--local" => {
                i += 1;
                local = args.get(i).map(PathBuf::from);
            }
            "--edge" => {
                i += 1;
                edge_id = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let Some(edge_id) = edge_id.as_deref() else {
        return Err(XtaskError::Usage);
    };

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    if let Some(f) = fixture.as_deref() {
        let upstream_default = workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("flowchart")
            .join(format!("{f}.svg"));
        let local_default = workspace_root
            .join("target")
            .join("compare")
            .join("flowchart")
            .join(format!("{f}.svg"));
        upstream = upstream.or(Some(upstream_default));
        local = local.or(Some(local_default));
    }

    let Some(upstream_path) = upstream else {
        return Err(XtaskError::Usage);
    };
    let Some(local_path) = local else {
        return Err(XtaskError::Usage);
    };

    let upstream_svg =
        fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
            path: upstream_path.display().to_string(),
            source,
        })?;
    let local_svg = fs::read_to_string(&local_path).map_err(|source| XtaskError::ReadFile {
        path: local_path.display().to_string(),
        source,
    })?;

    fn find_data_points(doc: &roxmltree::Document<'_>, edge_id: &str) -> Option<String> {
        for n in doc.descendants().filter(|n| n.is_element()) {
            if n.tag_name().name() != "path" {
                continue;
            }
            let Some(id) = n.attribute("data-id") else {
                continue;
            };
            if id != edge_id {
                continue;
            }
            let Some(dp) = n.attribute("data-points") else {
                continue;
            };
            return Some(dp.to_string());
        }
        None
    }

    fn decode_data_points_json(dp: &str) -> Option<serde_json::Value> {
        use base64::Engine as _;
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(dp.as_bytes()) else {
            return None;
        };
        serde_json::from_slice::<serde_json::Value>(&bytes).ok()
    }

    fn to_points(v: &serde_json::Value) -> Vec<(f64, f64)> {
        let Some(arr) = v.as_array() else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(arr.len());
        for p in arr {
            let (Some(x), Some(y)) = (
                p.get("x").and_then(|v| v.as_f64()),
                p.get("y").and_then(|v| v.as_f64()),
            ) else {
                continue;
            };
            if x.is_finite() && y.is_finite() {
                out.push((x, y));
            }
        }
        out
    }

    let upstream_doc = roxmltree::Document::parse(&upstream_svg)
        .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?;
    let local_doc = roxmltree::Document::parse(&local_svg)
        .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?;

    let Some(up_dp) = find_data_points(&upstream_doc, edge_id) else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "missing data-points for edge {edge_id:?} in {}",
            upstream_path.display()
        )));
    };
    let Some(lo_dp) = find_data_points(&local_doc, edge_id) else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "missing data-points for edge {edge_id:?} in {}",
            local_path.display()
        )));
    };

    let up_json = decode_data_points_json(&up_dp).ok_or_else(|| {
        XtaskError::DebugSvgFailed("failed to decode upstream data-points".into())
    })?;
    let lo_json = decode_data_points_json(&lo_dp)
        .ok_or_else(|| XtaskError::DebugSvgFailed("failed to decode local data-points".into()))?;

    println!("upstream: {}", upstream_path.display());
    println!("local:    {}", local_path.display());
    println!("edge:     {edge_id}");
    println!();

    println!("== Upstream decoded JSON ==");
    println!(
        "{}",
        serde_json::to_string_pretty(&up_json).unwrap_or_else(|_| "<unprintable>".to_string())
    );
    println!();

    println!("== Local decoded JSON ==");
    println!(
        "{}",
        serde_json::to_string_pretty(&lo_json).unwrap_or_else(|_| "<unprintable>".to_string())
    );
    println!();

    let up_pts = to_points(&up_json);
    let lo_pts = to_points(&lo_json);
    if up_pts.is_empty() || lo_pts.is_empty() {
        return Ok(());
    }

    println!("== Point deltas (upstream -> local) ==");
    let n = up_pts.len().min(lo_pts.len());
    let mut max_abs = 0.0f64;
    for idx in 0..n {
        let (ux, uy) = up_pts[idx];
        let (lx, ly) = lo_pts[idx];
        let dx = lx - ux;
        let dy = ly - uy;
        max_abs = max_abs.max(dx.abs()).max(dy.abs());
        println!(
            "#{idx}: upstream=({ux:.17},{uy:.17}) local=({lx:.17},{ly:.17}) Δ=({dx:.17},{dy:.17})"
        );
    }
    if up_pts.len() != lo_pts.len() {
        println!(
            "length mismatch: upstream={} local={}",
            up_pts.len(),
            lo_pts.len()
        );
    }
    println!("max |Δ| = {max_abs:.17}");

    Ok(())
}

fn debug_flowchart_edge_trace(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<String> = None;
    let mut edge_id: Option<String> = None;
    let mut out: Option<PathBuf> = None;
    let mut upstream: Option<PathBuf> = None;
    let mut local: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.to_string());
            }
            "--edge" => {
                i += 1;
                edge_id = args.get(i).map(|s| s.to_string());
            }
            "--out" => {
                i += 1;
                out = args.get(i).map(PathBuf::from);
            }
            "--upstream" => {
                i += 1;
                upstream = args.get(i).map(PathBuf::from);
            }
            "--local" => {
                i += 1;
                local = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let Some(edge_id) = edge_id.as_deref() else {
        return Err(XtaskError::Usage);
    };

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    let fixture_name = fixture
        .as_deref()
        .unwrap_or("upstream_flowchart_v2_self_loops_spec");
    let mmd_path = workspace_root
        .join("fixtures")
        .join("flowchart")
        .join(format!("{fixture_name}.mmd"));

    let text = fs::read_to_string(&mmd_path).map_err(|source| XtaskError::ReadFile {
        path: mmd_path.display().to_string(),
        source,
    })?;

    // Match compare-svg-xml defaults (handDrawnSeed ensures deterministic output).
    // Keep layout snapshots consistent with the in-repo `layout_snapshots_test` harness, which
    // uses the default engine configuration.
    let engine = merman::Engine::new();
    let measurer: std::sync::Arc<dyn merman_render::text::TextMeasurer + Send + Sync> =
        std::sync::Arc::new(merman_render::text::VendoredFontMetricsTextMeasurer::default());
    let layout_opts = merman_render::LayoutOptions {
        text_measurer: std::sync::Arc::clone(&measurer),
        ..Default::default()
    };

    let parsed =
        futures::executor::block_on(engine.parse_diagram(&text, merman::ParseOptions::default()))
            .map_err(|e| XtaskError::DebugSvgFailed(format!("parse failed: {e}")))?
            .ok_or_else(|| XtaskError::DebugSvgFailed("no diagram detected".to_string()))?;

    let layouted = merman_render::layout_parsed(&parsed, &layout_opts)
        .map_err(|e| XtaskError::DebugSvgFailed(format!("layout failed: {e}")))?;

    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = &layouted.layout else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "expected flowchart-v2 layout, got {}",
            layouted.meta.diagram_type
        )));
    };

    let out = out.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("trace")
            .join("flowchart")
            .join(fixture_name)
            .join(format!("{edge_id}.json"))
    });
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }

    // Rust 1.85+ marks environment mutation as `unsafe` due to potential UB when other
    // threads concurrently read/modify the process environment. `xtask` sets these vars
    // up-front before invoking rendering code, so this is safe in our usage.
    unsafe {
        std::env::set_var("MERMAN_TRACE_FLOWCHART_EDGE", edge_id);
        std::env::set_var("MERMAN_TRACE_FLOWCHART_OUT", &out);
    }

    let svg_opts = merman_render::svg::SvgRenderOptions {
        diagram_id: Some(fixture_name.to_string()),
        ..Default::default()
    };

    // Render once to trigger the trace emission inside `merman-render`.
    let svg = merman_render::svg::render_flowchart_v2_svg(
        layout,
        &layouted.semantic,
        &layouted.meta.effective_config,
        layouted.meta.title.as_deref(),
        layout_opts.text_measurer.as_ref(),
        &svg_opts,
    )
    .map_err(|e| XtaskError::DebugSvgFailed(format!("render failed: {e}")))?;

    if let Ok(doc) = roxmltree::Document::parse(&svg) {
        if let Some(dp) = find_data_points(&doc, edge_id) {
            if let Some(json) = decode_data_points_json(&dp) {
                println!("== Rendered SVG data-points (decoded) ==");
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json)
                        .unwrap_or_else(|_| "<unprintable>".to_string())
                );
                println!();
            }
        }
    }

    let trace_json = fs::read_to_string(&out).map_err(|source| XtaskError::ReadFile {
        path: out.display().to_string(),
        source,
    })?;

    println!("trace:   {}", out.display());
    println!("fixture: {fixture_name}");
    println!("edge:    {edge_id}");
    println!();
    println!("== Local edge trace (JSON) ==");
    println!("{trace_json}");

    // Optional: also print upstream/local decoded `data-points` from the XML compare output if available.
    if upstream.is_none() && local.is_none() {
        let upstream_default = workspace_root
            .join("target")
            .join("compare")
            .join("xml")
            .join("flowchart")
            .join(format!("{fixture_name}.upstream.xml"));
        let local_default = workspace_root
            .join("target")
            .join("compare")
            .join("xml")
            .join("flowchart")
            .join(format!("{fixture_name}.local.xml"));
        upstream = Some(upstream_default);
        local = Some(local_default);
    }

    let (Some(upstream_path), Some(local_path)) = (upstream, local) else {
        return Ok(());
    };
    let upstream_svg =
        fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
            path: upstream_path.display().to_string(),
            source,
        })?;
    let local_svg = fs::read_to_string(&local_path).map_err(|source| XtaskError::ReadFile {
        path: local_path.display().to_string(),
        source,
    })?;

    fn find_data_points(doc: &roxmltree::Document<'_>, edge_id: &str) -> Option<String> {
        for n in doc.descendants().filter(|n| n.is_element()) {
            if n.tag_name().name() != "path" {
                continue;
            }
            let Some(id) = n.attribute("data-id") else {
                continue;
            };
            if id != edge_id {
                continue;
            }
            let Some(dp) = n.attribute("data-points") else {
                continue;
            };
            return Some(dp.to_string());
        }
        None
    }

    fn decode_data_points_json(dp: &str) -> Option<serde_json::Value> {
        use base64::Engine as _;
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(dp.as_bytes()) else {
            return None;
        };
        serde_json::from_slice::<serde_json::Value>(&bytes).ok()
    }

    let upstream_doc = roxmltree::Document::parse(&upstream_svg)
        .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?;
    let local_doc = roxmltree::Document::parse(&local_svg)
        .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?;
    let Some(up_dp) = find_data_points(&upstream_doc, edge_id) else {
        println!();
        println!(
            "(no upstream data-points found for edge {edge_id} in {})",
            upstream_path.display()
        );
        return Ok(());
    };
    let Some(lo_dp) = find_data_points(&local_doc, edge_id) else {
        println!();
        println!(
            "(no local data-points found for edge {edge_id} in {})",
            local_path.display()
        );
        return Ok(());
    };

    let up_json = decode_data_points_json(&up_dp).ok_or_else(|| {
        XtaskError::DebugSvgFailed("failed to decode upstream data-points".into())
    })?;
    let lo_json = decode_data_points_json(&lo_dp)
        .ok_or_else(|| XtaskError::DebugSvgFailed("failed to decode local data-points".into()))?;

    println!();
    println!("== XML data-points (decoded) ==");
    println!("upstream: {}", upstream_path.display());
    println!("local:    {}", local_path.display());
    println!();
    println!("-- Upstream --");
    println!(
        "{}",
        serde_json::to_string_pretty(&up_json).unwrap_or_else(|_| "<unprintable>".to_string())
    );
    println!();
    println!("-- Local --");
    println!(
        "{}",
        serde_json::to_string_pretty(&lo_json).unwrap_or_else(|_| "<unprintable>".to_string())
    );

    Ok(())
}

fn measure_text(args: Vec<String>) -> Result<(), XtaskError> {
    use merman_render::text::TextMeasurer as _;

    let mut text: Option<String> = None;
    let mut font_family: Option<String> = None;
    let mut font_size: f64 = 16.0;
    let mut wrap_mode: String = "svg".to_string();
    let mut max_width: Option<f64> = None;
    let mut measurer: String = "vendored".to_string();
    let mut svg_bbox_x: bool = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--text" => {
                i += 1;
                text = args.get(i).map(|s| s.to_string());
            }
            "--font-family" => {
                i += 1;
                font_family = args.get(i).map(|s| s.to_string());
            }
            "--font-size" => {
                i += 1;
                font_size = args
                    .get(i)
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(16.0);
            }
            "--wrap-mode" => {
                i += 1;
                wrap_mode = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "svg".to_string());
            }
            "--max-width" => {
                i += 1;
                max_width = args.get(i).and_then(|s| s.parse::<f64>().ok());
            }
            "--measurer" => {
                i += 1;
                measurer = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "vendored".to_string());
            }
            "--svg-bbox-x" => svg_bbox_x = true,
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let Some(text) = text else {
        return Err(XtaskError::Usage);
    };

    let wrap_mode = match wrap_mode.as_str() {
        "html" | "htmllike" => merman_render::text::WrapMode::HtmlLike,
        "svg-single-run" | "svg-singlerun" | "svglikesinglerun" => {
            merman_render::text::WrapMode::SvgLikeSingleRun
        }
        _ => merman_render::text::WrapMode::SvgLike,
    };

    let style = merman_render::text::TextStyle {
        font_family,
        font_size,
        font_weight: None,
    };

    let metrics = if matches!(
        measurer.as_str(),
        "deterministic" | "deterministic-text" | "deterministic-text-measurer"
    ) {
        let m = merman_render::text::DeterministicTextMeasurer::default();
        m.measure_wrapped(&text, &style, max_width, wrap_mode)
    } else {
        let m = merman_render::text::VendoredFontMetricsTextMeasurer::default();
        m.measure_wrapped(&text, &style, max_width, wrap_mode)
    };

    println!("text: {:?}", text);
    println!("font_family: {:?}", style.font_family);
    println!("font_size: {}", style.font_size);
    println!("wrap_mode: {:?}", wrap_mode);
    println!("max_width: {:?}", max_width);
    println!("width: {}", metrics.width);
    println!("height: {}", metrics.height);
    println!("line_count: {}", metrics.line_count);
    if svg_bbox_x {
        let (left, right) = if matches!(
            measurer.as_str(),
            "deterministic" | "deterministic-text" | "deterministic-text-measurer"
        ) {
            let m = merman_render::text::DeterministicTextMeasurer::default();
            m.measure_svg_text_bbox_x(&text, &style)
        } else {
            let m = merman_render::text::VendoredFontMetricsTextMeasurer::default();
            m.measure_svg_text_bbox_x(&text, &style)
        };
        println!("svg_bbox_x_left: {}", left);
        println!("svg_bbox_x_right: {}", right);
        println!("svg_bbox_x_width: {}", left + right);
    }

    Ok(())
}

fn debug_flowchart_layout(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<PathBuf> = None;
    let mut edge_id: Option<String> = None;
    let mut text_measurer: String = "deterministic".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(PathBuf::from);
            }
            "--edge" => {
                i += 1;
                edge_id = args.get(i).map(|s| s.to_string());
            }
            "--text-measurer" => {
                i += 1;
                text_measurer = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "deterministic".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let Some(fixture_path) = fixture else {
        return Err(XtaskError::Usage);
    };
    let text = std::fs::read_to_string(&fixture_path).map_err(|source| XtaskError::ReadFile {
        path: fixture_path.display().to_string(),
        source,
    })?;

    let engine = merman::Engine::new()
        .with_site_config(merman::MermaidConfig::from_value(
            serde_json::json!({ "handDrawnSeed": 1 }),
        ))
        .with_fixed_today(Some(
            chrono::NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid date"),
        ));
    let parsed =
        futures::executor::block_on(engine.parse_diagram(&text, merman::ParseOptions::default()))
            .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?
            .ok_or_else(|| {
                XtaskError::DebugSvgFailed(format!(
                    "no diagram detected in {}",
                    fixture_path.display()
                ))
            })?;

    let mut layout_opts = merman_render::LayoutOptions::default();
    if matches!(
        text_measurer.as_str(),
        "vendored" | "vendored-font" | "vendored-font-metrics"
    ) {
        layout_opts.text_measurer =
            std::sync::Arc::new(merman_render::text::VendoredFontMetricsTextMeasurer::default());
    }
    let layouted = merman_render::layout_parsed(&parsed, &layout_opts)
        .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?;

    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = &layouted.layout else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "unexpected layout type: {}",
            layouted.meta.diagram_type
        )));
    };

    println!("fixture: {}", fixture_path.display());
    if let Some(title) = layouted.meta.title.as_deref() {
        println!("title: {}", title);
    }
    println!("diagram_type: {}", layouted.meta.diagram_type);
    println!("text_measurer: {}", text_measurer);
    println!();

    // Mirror `compute_layout_bounds` (private to `merman-render`) for debugging.
    #[derive(Debug, Clone, Copy)]
    struct Bounds {
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
    }

    fn compute_bounds(
        clusters: &[merman_render::model::LayoutCluster],
        nodes: &[merman_render::model::LayoutNode],
        edges: &[merman_render::model::LayoutEdge],
    ) -> Option<Bounds> {
        let mut b: Option<Bounds> = None;
        let mut include_rect = |min_x: f64, min_y: f64, max_x: f64, max_y: f64| {
            if let Some(ref mut cur) = b {
                cur.min_x = cur.min_x.min(min_x);
                cur.min_y = cur.min_y.min(min_y);
                cur.max_x = cur.max_x.max(max_x);
                cur.max_y = cur.max_y.max(max_y);
            } else {
                b = Some(Bounds {
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                });
            }
        };

        for c in clusters {
            let hw = c.width / 2.0;
            let hh = c.height / 2.0;
            include_rect(c.x - hw, c.y - hh, c.x + hw, c.y + hh);
            let lhw = c.title_label.width / 2.0;
            let lhh = c.title_label.height / 2.0;
            include_rect(
                c.title_label.x - lhw,
                c.title_label.y - lhh,
                c.title_label.x + lhw,
                c.title_label.y + lhh,
            );
        }

        for n in nodes {
            let hw = n.width / 2.0;
            let hh = n.height / 2.0;
            include_rect(n.x - hw, n.y - hh, n.x + hw, n.y + hh);
        }

        for e in edges {
            for p in &e.points {
                include_rect(p.x, p.y, p.x, p.y);
            }
            for lbl in [
                e.label.as_ref(),
                e.start_label_left.as_ref(),
                e.start_label_right.as_ref(),
                e.end_label_left.as_ref(),
                e.end_label_right.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                let hw = lbl.width / 2.0;
                let hh = lbl.height / 2.0;
                include_rect(lbl.x - hw, lbl.y - hh, lbl.x + hw, lbl.y + hh);
            }
        }

        b
    }

    if let Some(b) = compute_bounds(&layout.clusters, &layout.nodes, &layout.edges) {
        println!(
            "bounds: min=({}, {}) max=({}, {}) size=({}, {})",
            b.min_x,
            b.min_y,
            b.max_x,
            b.max_y,
            b.max_x - b.min_x,
            b.max_y - b.min_y
        );
        println!();
    }

    println!("clusters: {}", layout.clusters.len());
    for c in &layout.clusters {
        println!(
            "- {} x={} y={} w={} h={} dir={}",
            c.id, c.x, c.y, c.width, c.height, c.effective_dir
        );
    }
    println!();

    println!("nodes: {}", layout.nodes.len());
    for n in &layout.nodes {
        println!(
            "- {} x={} y={} w={} h={}",
            n.id, n.x, n.y, n.width, n.height
        );
    }
    println!();

    println!("edges: {}", layout.edges.len());
    for e in &layout.edges {
        if edge_id.as_ref().is_some_and(|id| id != &e.id) {
            continue;
        }
        println!(
            "- {} {} -> {} from_cluster={:?} to_cluster={:?} points={}",
            e.id,
            e.from,
            e.to,
            e.from_cluster,
            e.to_cluster,
            e.points.len()
        );
        if let Some(lbl) = e.label.as_ref() {
            println!(
                "  label: x={} y={} w={} h={}",
                lbl.x, lbl.y, lbl.width, lbl.height
            );
        }
        for (idx, p) in e.points.iter().enumerate() {
            if idx >= 16 {
                println!("  ...");
                break;
            }
            println!("  - p{idx}: x={} y={}", p.x, p.y);
        }
    }

    Ok(())
}

fn gen_c4_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root = out_root.unwrap_or_else(|| workspace_root.join("target").join("svgs"));

    let fixtures_dir = workspace_root.join("fixtures").join("c4");
    let out_dir = out_root.join("c4");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    // Keep this aligned with `crates/merman-render/tests/layout_snapshots_test.rs` so the
    // `update-layout-snapshots` output matches the test's computed layouts.
    let engine = merman_core::Engine::new();
    let layout_opts = merman_render::LayoutOptions::default();
    let mut failures: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(engine.parse_diagram(
            &text,
            merman_core::ParseOptions {
                suppress_errors: true,
            },
        )) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::C4Diagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let svg = match merman_render::svg::render_c4_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let out_path = out_dir.join(format!("{stem}.svg"));
        if let Err(err) = fs::write(&out_path, svg) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::DebugSvgFailed(failures.join("\n")))
}

fn gen_c4_textlength(args: Vec<String>) -> Result<(), XtaskError> {
    let mut in_dir: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--in" => {
                i += 1;
                in_dir = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    let in_dir = in_dir.unwrap_or_else(|| {
        workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("c4")
    });
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("crates")
            .join("merman-render")
            .join("src")
            .join("generated")
            .join("c4_type_textlength_11_12_2.rs")
    });

    let mut svg_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&in_dir) else {
        return Err(XtaskError::VerifyFailed(format!(
            "failed to list C4 upstream svg directory {}",
            in_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "svg") {
            continue;
        }
        svg_files.push(path);
    }
    svg_files.sort();
    if svg_files.is_empty() {
        return Err(XtaskError::VerifyFailed(format!(
            "no C4 upstream SVG files found under {}",
            in_dir.display()
        )));
    }

    let re =
        Regex::new(r#"textLength="(?P<len>\d+(?:\.\d+)?)"[^>]*>&lt;&lt;(?P<ty>[^&]+)&gt;&gt;"#)
            .map_err(|e| XtaskError::VerifyFailed(format!("failed to build regex: {e}")))?;

    let mut map: BTreeMap<String, f64> = BTreeMap::new();
    let mut conflicts: Vec<String> = Vec::new();

    for path in svg_files {
        let svg = read_text(&path)?;
        for cap in re.captures_iter(&svg) {
            let ty = cap.name("ty").map(|m| m.as_str()).unwrap_or("").to_string();
            let len = cap
                .name("len")
                .and_then(|m| m.as_str().parse::<f64>().ok())
                .unwrap_or(0.0);
            if ty.is_empty() || len <= 0.0 {
                continue;
            }
            if let Some(prev) = map.get(&ty) {
                if (*prev - len).abs() > 0.001 {
                    conflicts.push(format!("{ty}: {prev} vs {len}"));
                }
            } else {
                map.insert(ty, len);
            }
        }
    }

    if !conflicts.is_empty() {
        conflicts.sort();
        conflicts.dedup();
        return Err(XtaskError::VerifyFailed(format!(
            "conflicting C4 type textLength values found:\n{}",
            conflicts.join("\n")
        )));
    }

    if map.is_empty() {
        return Err(XtaskError::VerifyFailed(format!(
            "no C4 type textLength values were extracted from {}",
            in_dir.display()
        )));
    }

    let mut out = String::new();
    out.push_str("// This file is @generated by `cargo run -p xtask -- gen-c4-textlength`.\n");
    out.push_str("//\n");
    out.push_str(
        "// Mermaid derives these values via DOM-backed text measurement (`getBBox`) and emits them as the\n",
    );
    out.push_str(
        "// `textLength` attribute for the C4 type line (`<<person>>`, etc). To make DOM parity reproducible\n",
    );
    out.push_str(
        "// in a headless Rust context, we vendor the observed values from the pinned Mermaid CLI baselines.\n\n",
    );
    out.push_str("pub fn c4_type_text_length_px_11_12_2(type_c4_shape: &str) -> Option<f64> {\n");
    out.push_str("    match type_c4_shape {\n");
    for (ty, len) in &map {
        let _ = writeln!(&mut out, r#"        "{}" => Some({}),"#, ty, fmt_f64(*len));
    }
    out.push_str("        _ => None,\n");
    out.push_str("    }\n");
    out.push_str("}\n");

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, out).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}

fn fmt_f64(v: f64) -> String {
    // Keep output stable and human-readable:
    // - round to 3 decimals
    // - trim trailing zeros
    // - keep at least 1 decimal place (e.g. `73.0`, not `73`)
    let rounded = (v * 1000.0).round() / 1000.0;
    let mut s = format!("{rounded:.3}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.push('0');
    }
    if s == "-0.0" { "0.0".to_string() } else { s }
}

fn update_layout_snapshots(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;
    let mut decimals: u32 = 3;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args
                    .get(i)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "all".to_string());
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--decimals" => {
                i += 1;
                decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    fn round_f64(v: f64, decimals: u32) -> f64 {
        let p = 10_f64.powi(decimals as i32);
        (v * p).round() / p
    }

    fn round_json_numbers(v: &mut JsonValue, decimals: u32) {
        match v {
            JsonValue::Number(n) => {
                let Some(f) = n.as_f64() else {
                    return;
                };
                let r = round_f64(f, decimals);
                if let Some(nn) = serde_json::Number::from_f64(r) {
                    *v = JsonValue::Number(nn);
                }
            }
            JsonValue::Array(arr) => {
                for item in arr {
                    round_json_numbers(item, decimals);
                }
            }
            JsonValue::Object(map) => {
                for (_k, val) in map.iter_mut() {
                    round_json_numbers(val, decimals);
                }
            }
            _ => {}
        }
    }

    fn normalize_layout_snapshot(diagram_type: &str, v: &mut JsonValue) {
        // Mermaid gitGraph auto-generates commit ids using random hex suffixes.
        // Normalize these ids so snapshots are stable across runs.
        if diagram_type == "gitGraph" {
            let re = Regex::new(r"\b(\d+)-[0-9a-f]{7}\b").expect("gitGraph id regex must compile");

            fn walk(re: &Regex, v: &mut JsonValue) {
                match v {
                    JsonValue::String(s) => {
                        if re.is_match(s) {
                            *s = re.replace_all(s, "$1-<dynamic>").to_string();
                        }
                    }
                    JsonValue::Array(arr) => {
                        for item in arr {
                            walk(re, item);
                        }
                    }
                    JsonValue::Object(map) => {
                        for (_k, val) in map.iter_mut() {
                            walk(re, val);
                        }
                    }
                    _ => {}
                }
            }

            walk(&re, v);
            return;
        }

        // Mermaid block diagram auto-generates internal ids using random base36 suffixes.
        if diagram_type == "block" {
            let re = Regex::new(r"id-[a-z0-9]+-(\d+)").expect("block id regex must compile");

            fn walk(re: &Regex, v: &mut JsonValue) {
                match v {
                    JsonValue::String(s) => {
                        if re.is_match(s) {
                            *s = re.replace_all(s, "id-<id>-$1").to_string();
                        }
                    }
                    JsonValue::Array(arr) => {
                        for item in arr {
                            walk(re, item);
                        }
                    }
                    JsonValue::Object(map) => {
                        for (_k, val) in map.iter_mut() {
                            walk(re, val);
                        }
                    }
                    _ => {}
                }
            }

            walk(&re, v);
        }
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_root = if diagram == "all" {
        workspace_root.join("fixtures")
    } else {
        workspace_root.join("fixtures").join(&diagram)
    };

    let mut mmd_files = Vec::new();
    let mut stack = vec![fixtures_root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "upstream-svgs") {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|e| e == "mmd") {
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
                {
                    continue;
                }
                if let Some(ref f) = filter {
                    if !path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.contains(f))
                    {
                        continue;
                    }
                }
                mmd_files.push(path);
            }
        }
    }
    mmd_files.sort();
    if mmd_files.is_empty() {
        return Err(XtaskError::LayoutSnapshotUpdateFailed(format!(
            "no .mmd fixtures found under {}",
            fixtures_root.display()
        )));
    }

    let engine = merman::Engine::new()
        .with_site_config(merman::MermaidConfig::from_value(
            serde_json::json!({ "handDrawnSeed": 1 }),
        ))
        .with_fixed_today(Some(
            chrono::NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid date"),
        ));
    let layout_opts = merman_render::LayoutOptions::default();
    let mut failures = Vec::new();

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(engine.parse_diagram(
            &text,
            merman::ParseOptions {
                suppress_errors: true,
            },
        )) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        if diagram != "all" {
            let dt = parsed.meta.diagram_type.as_str();
            let matches = dt == diagram
                || (diagram == "er" && matches!(dt, "er" | "erDiagram"))
                || (diagram == "flowchart" && dt == "flowchart-v2")
                || (diagram == "state" && dt == "stateDiagram")
                || (diagram == "class" && matches!(dt, "class" | "classDiagram"))
                || (diagram == "gitgraph" && dt == "gitGraph")
                || (diagram == "quadrantchart" && dt == "quadrantChart");
            if !matches {
                continue;
            }
        }

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(merman_render::Error::UnsupportedDiagram { .. }) => {
                // Layout snapshots are only defined for diagram types currently supported by
                // `merman-render::layout_parsed`. Skip unsupported diagrams so `--diagram all`
                // can be used for "all supported layout diagrams".
                continue;
            }
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let mut layout_json = match serde_json::to_value(&layouted.layout) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "failed to serialize layout JSON for {}: {err}",
                    mmd_path.display()
                ));
                continue;
            }
        };
        round_json_numbers(&mut layout_json, decimals);

        let mut out = serde_json::json!({
            "diagramType": parsed.meta.diagram_type,
            "layout": layout_json,
        });
        normalize_layout_snapshot(&parsed.meta.diagram_type, &mut out);

        let pretty = match serde_json::to_string_pretty(&out) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "failed to pretty-print JSON for {}: {err}",
                    mmd_path.display()
                ));
                continue;
            }
        };

        let out_path = mmd_path.with_extension("layout.golden.json");
        if let Some(parent) = out_path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                failures.push(format!("failed to create dir {}: {err}", parent.display()));
                continue;
            }
        }
        if let Err(err) = fs::write(&out_path, format!("{pretty}\n")) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::LayoutSnapshotUpdateFailed(failures.join("\n")))
    }
}

fn compare_er_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_markers: bool = false;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-markers" => check_markers = true,
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("er");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("er");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("er_report.md")
    });

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    let re_viewbox = Regex::new(r#"viewBox="([^"]+)""#).unwrap();
    let re_max_width = Regex::new(r#"max-width:\s*([0-9.]+)px"#).unwrap();
    let re_marker_id = Regex::new(r#"<marker[^>]*\bid="([^"]+)""#).unwrap();
    let re_marker_ref = Regex::new(r#"marker-(?:start|end)="url\(#([^)]+)\)""#).unwrap();

    let mode = svgdom::DomMode::parse(&dom_mode);

    #[derive(Default)]
    struct SvgSig {
        view_box: Option<String>,
        max_width_px: Option<String>,
        marker_ids: std::collections::BTreeSet<String>,
        marker_refs: std::collections::BTreeSet<String>,
    }

    fn sig_for_svg(
        svg: &str,
        re_viewbox: &Regex,
        re_max_width: &Regex,
        re_marker_id: &Regex,
        re_marker_ref: &Regex,
    ) -> SvgSig {
        let view_box = re_viewbox
            .captures(svg)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string());
        let max_width_px = re_max_width
            .captures(svg)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string());
        let mut marker_ids = std::collections::BTreeSet::new();
        for cap in re_marker_id.captures_iter(svg) {
            if let Some(m) = cap.get(1) {
                marker_ids.insert(m.as_str().to_string());
            }
        }
        let mut marker_refs = std::collections::BTreeSet::new();
        for cap in re_marker_ref.captures_iter(svg) {
            if let Some(m) = cap.get(1) {
                marker_refs.insert(m.as_str().to_string());
            }
        }
        SvgSig {
            view_box,
            max_width_px,
            marker_ids,
            marker_refs,
        }
    }

    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "handDrawnSeed": 1 }),
    ));
    let layout_opts = svg_compare_layout_opts();

    let mut report = String::new();
    let _ = writeln!(&mut report, "# ER SVG Compare Report");
    let _ = writeln!(&mut report);
    let _ = writeln!(
        &mut report,
        "- Upstream: `fixtures/upstream-svgs/er/*.svg` (Mermaid CLI pinned to Mermaid 11.12.2)"
    );
    let _ = writeln!(&mut report, "- Local: `render_er_diagram_svg` (Stage B)");
    let _ = writeln!(&mut report);
    let _ = writeln!(
        &mut report,
        "| fixture | markers ok | dom ok | viewBox (upstream) | viewBox (local) | max-width (upstream) | max-width (local) |"
    );
    let _ = writeln!(&mut report, "|---|---:|---:|---|---|---:|---:|");

    let mut failures: Vec<String> = Vec::new();
    let mut dom_failures: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {}: {} ({err})",
                    mmd_path.display(),
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(engine.parse_diagram(
            &text,
            merman::ParseOptions {
                suppress_errors: true,
            },
        )) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::ErDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_er_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let upstream_sig = sig_for_svg(
            &upstream_svg,
            &re_viewbox,
            &re_max_width,
            &re_marker_id,
            &re_marker_ref,
        );
        let local_sig = sig_for_svg(
            &local_svg,
            &re_viewbox,
            &re_max_width,
            &re_marker_id,
            &re_marker_ref,
        );

        let mut marker_ok = true;
        let mut missing: Vec<String> = Vec::new();
        let mut extra: Vec<String> = Vec::new();
        for m in &upstream_sig.marker_ids {
            if !local_sig.marker_ids.contains(m) {
                marker_ok = false;
                missing.push(m.clone());
            }
        }
        for m in &local_sig.marker_ids {
            if !upstream_sig.marker_ids.contains(m) {
                marker_ok = false;
                extra.push(m.clone());
            }
        }
        for r in &local_sig.marker_refs {
            if !local_sig.marker_ids.contains(r) {
                marker_ok = false;
                extra.push(format!("ref-missing-def:{r}"));
            }
        }

        if check_markers && !marker_ok {
            failures.push(format!(
                "marker mismatch for {stem}: missing={:?} extra={:?}",
                missing, extra
            ));
        }

        let mut dom_ok = true;
        let dom_ok_str = if check_dom {
            let upstream_dom = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => Some(v),
                Err(err) => {
                    dom_ok = false;
                    dom_failures.push(format!("dom parse failed (upstream) for {stem}: {err}"));
                    None
                }
            };
            let local_dom = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => Some(v),
                Err(err) => {
                    dom_ok = false;
                    dom_failures.push(format!("dom parse failed (local) for {stem}: {err}"));
                    None
                }
            };

            if dom_ok {
                if let (Some(upstream_dom), Some(local_dom)) =
                    (upstream_dom.as_ref(), local_dom.as_ref())
                {
                    if let Some(diff) = svgdom::dom_diff(upstream_dom, local_dom) {
                        dom_ok = false;
                        dom_failures.push(format!("{stem}: {diff}"));
                    }
                }
            }

            if !dom_ok {
                failures.push(format!(
                    "dom mismatch for {stem} (mode={dom_mode}, decimals={dom_decimals})"
                ));
            }

            if dom_ok { "yes" } else { "no" }
        } else {
            "-"
        };

        let _ = writeln!(
            &mut report,
            "| `{}` | {} | {} | `{}` | `{}` | `{}` | `{}` |",
            stem,
            if marker_ok { "yes" } else { "no" },
            dom_ok_str,
            upstream_sig
                .view_box
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            local_sig
                .view_box
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            upstream_sig
                .max_width_px
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            local_sig
                .max_width_px
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        );
    }

    if check_dom && !dom_failures.is_empty() {
        let _ = writeln!(&mut report);
        let _ = writeln!(&mut report, "## DOM Mismatch Details");
        let _ = writeln!(&mut report);
        for f in &dom_failures {
            let _ = writeln!(&mut report, "- {f}");
        }
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::SvgCompareFailed(failures.join("\n")))
}

fn gen_upstream_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "er".to_string();
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut install: bool = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
            }
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--install" => install = true,
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root =
        out_root.unwrap_or_else(|| workspace_root.join("fixtures").join("upstream-svgs"));

    let tools_root = workspace_root.join("tools").join("mermaid-cli");
    let node_modules = tools_root.join("node_modules");
    if install || !node_modules.exists() {
        let npm_cmd = if tools_root.join("package-lock.json").is_file() {
            "ci"
        } else {
            "install"
        };
        let mut cmd = if cfg!(windows) {
            let mut cmd = Command::new("cmd.exe");
            cmd.arg("/c").arg("npm").arg(npm_cmd);
            cmd
        } else {
            let mut cmd = Command::new("npm");
            cmd.arg(npm_cmd);
            cmd
        };
        let status = cmd.current_dir(&tools_root).status().map_err(|err| {
            XtaskError::UpstreamSvgFailed(format!(
                "failed to run `npm {npm_cmd}` in {}: {err}",
                tools_root.display()
            ))
        })?;
        if !status.success() {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "npm {npm_cmd} failed in {}",
                tools_root.display()
            )));
        }
    }

    let mmdc = find_mmdc(&tools_root).ok_or_else(|| {
        XtaskError::UpstreamSvgFailed(format!(
            "mmdc not found under {} (run: npm install)",
            tools_root.display()
        ))
    })?;

    fn run_one(
        workspace_root: &Path,
        out_root: &Path,
        mmdc: &Path,
        diagram: &str,
        filter: Option<&str>,
    ) -> Result<(), XtaskError> {
        let fixtures_dir = workspace_root.join("fixtures").join(diagram);
        let out_dir = out_root.join(diagram);
        let node_cwd = workspace_root.join("tools").join("mermaid-cli");
        let use_seeded_renderer = diagram == "architecture" || diagram == "gitgraph";
        let seeded_script = if use_seeded_renderer {
            Some(ensure_seeded_upstream_svg_renderer_script(workspace_root)?)
        } else {
            None
        };

        fn sanitize_svg_id(raw: &str) -> String {
            let mut out = String::with_capacity(raw.len());
            for ch in raw.chars() {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    out.push(ch);
                } else {
                    out.push('_');
                }
            }
            if out.is_empty() {
                "diagram".to_string()
            } else {
                out
            }
        }

        let mut mmd_files: Vec<PathBuf> = Vec::new();
        let Ok(entries) = fs::read_dir(&fixtures_dir) else {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "failed to list fixtures directory {}",
                fixtures_dir.display()
            )));
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().is_none_or(|e| e != "mmd") {
                continue;
            }
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
            {
                continue;
            }
            if diagram == "gantt"
                && path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    matches!(
                        n,
                        "click_loose.mmd"
                            | "click_strict.mmd"
                            | "dateformat_hash_comment_truncates.mmd"
                            | "excludes_hash_comment_truncates.mmd"
                            | "today_marker_and_axis.mmd"
                    )
                })
            {
                continue;
            }
            if diagram == "state"
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("_parser_") || n.contains("_parser_spec"))
            {
                continue;
            }
            if diagram == "class"
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("upstream_text_label_variants_spec"))
            {
                continue;
            }
            if diagram == "c4" {
                // Mermaid C4 has known render-time type assumptions that make some valid parser
                // fixtures non-renderable (e.g. kv-objects stored in `label.text` or
                // `UpdateElementStyle(..., techn="Rust")` storing `techn` as a raw string).
                //
                // Keep these fixtures for parser parity, but skip them for upstream SVG baselines.
                if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    matches!(
                        n,
                        "nesting_updates.mmd"
                            | "upstream_boundary_spec.mmd"
                            | "upstream_c4container_header_and_direction_spec.mmd"
                            | "upstream_container_spec.mmd"
                            | "upstream_person_ext_spec.mmd"
                            | "upstream_person_spec.mmd"
                            | "upstream_system_spec.mmd"
                            | "upstream_update_element_style_all_fields_spec.mmd"
                    )
                }) {
                    continue;
                }
            }
            if let Some(f) = filter {
                if !path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains(f))
                {
                    continue;
                }
            }
            mmd_files.push(path);
        }
        mmd_files.sort();

        if mmd_files.is_empty() {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "no .mmd fixtures matched under {}",
                fixtures_dir.display()
            )));
        }

        fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
            path: out_dir.display().to_string(),
            source,
        })?;

        let failures_path = out_dir.join("_failures.txt");
        let _ = fs::remove_file(&failures_path);

        let mut failures: Vec<String> = Vec::new();

        for mmd_path in mmd_files {
            let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
                failures.push(format!("invalid fixture filename {}", mmd_path.display()));
                continue;
            };
            let out_path = out_dir.join(format!("{stem}.svg"));
            let svg_id = sanitize_svg_id(stem);

            let status = if use_seeded_renderer {
                use std::io::Write;
                use std::process::Stdio;

                // Architecture layout relies on cytoscape-fcose, which uses `Math.random()` for
                // spectral initialization. To keep upstream baselines reproducible, we render via
                // a small puppeteer wrapper that seeds `Math.random()` deterministically.
                let pinned_config = node_cwd.join("mermaid-config.json");
                let seed: u64 = 1;
                let output_abs = if out_path.is_absolute() {
                    out_path.clone()
                } else {
                    workspace_root.join(&out_path)
                };

                let input_json = serde_json::json!({
                    "input_path": mmd_path.display().to_string(),
                    "output_path": output_abs.display().to_string(),
                    "config_path": pinned_config.display().to_string(),
                    "theme": "default",
                    "svg_id": svg_id,
                    "seed": seed,
                    "width": 800,
                    "height": 600,
                    "background_color": "white",
                })
                .to_string();

                let Some(script_path) = seeded_script.as_ref() else {
                    return Err(XtaskError::UpstreamSvgFailed(
                        "seeded renderer script not available".to_string(),
                    ));
                };

                let mut cmd = Command::new("node");
                cmd.arg(script_path)
                    .current_dir(&node_cwd)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit());
                let mut child = cmd.spawn().map_err(|err| {
                    XtaskError::UpstreamSvgFailed(format!(
                        "failed to spawn seeded upstream svg renderer: {err}"
                    ))
                })?;
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(input_json.as_bytes());
                }
                child.wait()
            } else {
                let mut cmd = if cfg!(windows) {
                    match mmdc.extension().and_then(|s| s.to_str()) {
                        Some(ext)
                            if ext.eq_ignore_ascii_case("cmd")
                                || ext.eq_ignore_ascii_case("bat") =>
                        {
                            let mut cmd = Command::new("cmd.exe");
                            cmd.arg("/c").arg(mmdc);
                            cmd
                        }
                        Some(ext) if ext.eq_ignore_ascii_case("ps1") => {
                            let mut cmd = Command::new("powershell.exe");
                            cmd.arg("-NoProfile")
                                .arg("-ExecutionPolicy")
                                .arg("Bypass")
                                .arg("-File")
                                .arg(mmdc);
                            cmd
                        }
                        _ => Command::new(mmdc),
                    }
                } else {
                    Command::new(mmdc)
                };
                cmd.arg("-i")
                    .arg(&mmd_path)
                    .arg("-o")
                    .arg(&out_path)
                    .arg("-t")
                    .arg("default");

                // Stabilize Rough.js output across runs. Mermaid uses Rough.js for many "classic look"
                // shapes too (often with `roughness: 0`), but the stroke control points still depend on
                // `random()` via `divergePoint`. Pin `handDrawnSeed` for reproducible upstream SVG
                // baselines.
                let pinned_config = workspace_root
                    .join("tools")
                    .join("mermaid-cli")
                    .join("mermaid-config.json");
                cmd.arg("-c").arg(pinned_config);

                // Gantt rendering depends on the page width (`parentElement.offsetWidth`). In a
                // headless Rust context we default to the Mermaid fallback width (1200) when no DOM
                // width is available. Use the same page width for upstream baselines so parity diffs
                // remain meaningful.
                if diagram == "gantt" {
                    cmd.arg("-w").arg("1200");
                }

                cmd.arg("--svgId").arg(svg_id);
                cmd.status()
            };

            match status {
                Ok(s) if s.success() => {
                    // Some upstream renderer failures surface only as console errors while still
                    // returning a successful exit code. Treat missing/empty outputs as failures so
                    // we don't silently accept a broken baseline corpus.
                    match fs::metadata(&out_path) {
                        Ok(meta) if meta.is_file() && meta.len() > 0 => {}
                        Ok(meta) => failures.push(format!(
                            "mmdc succeeded but output is empty for {} (out={}, bytes={})",
                            mmd_path.display(),
                            out_path.display(),
                            meta.len()
                        )),
                        Err(err) => failures.push(format!(
                            "mmdc succeeded but output is missing for {} (out={}, err={})",
                            mmd_path.display(),
                            out_path.display(),
                            err
                        )),
                    }
                }
                Ok(s) => failures.push(format!(
                    "mmdc failed for {} (exit={})",
                    mmd_path.display(),
                    s.code().unwrap_or(-1)
                )),
                Err(err) => failures.push(format!("mmdc failed for {}: {err}", mmd_path.display())),
            }
        }

        if failures.is_empty() {
            return Ok(());
        }

        let _ = fs::write(&failures_path, failures.join("\n"));

        Err(XtaskError::UpstreamSvgFailed(failures.join("\n")))
    }

    let filter = filter.as_deref();
    match diagram.as_str() {
        "all" => {
            let mut failures: Vec<String> = Vec::new();
            for d in [
                "er",
                "flowchart",
                "gantt",
                "architecture",
                "mindmap",
                "state",
                "class",
                "sequence",
                "info",
                "pie",
                "sankey",
                "requirement",
                "packet",
                "timeline",
                "journey",
                "kanban",
                "gitgraph",
                "quadrantchart",
                "c4",
                "block",
                "radar",
                "treemap",
                "xychart",
            ] {
                if let Err(err) = run_one(&workspace_root, &out_root, &mmdc, d, filter) {
                    failures.push(format!("{d}: {err}"));
                }
            }
            if failures.is_empty() {
                Ok(())
            } else {
                Err(XtaskError::UpstreamSvgFailed(failures.join("\n")))
            }
        }
        "er" | "flowchart" | "state" | "class" | "sequence" | "info" | "pie" | "requirement"
        | "sankey" | "packet" | "timeline" | "journey" | "kanban" | "gitgraph" | "gantt" | "c4"
        | "block" | "radar" | "quadrantchart" | "treemap" | "xychart" | "mindmap"
        | "architecture" => run_one(&workspace_root, &out_root, &mmdc, &diagram, filter),
        other => Err(XtaskError::UpstreamSvgFailed(format!(
            "unsupported diagram for upstream svg export: {other} (supported: er, flowchart, gantt, architecture, mindmap, state, class, sequence, info, pie, sankey, requirement, packet, timeline, journey, kanban, gitgraph, quadrantchart, c4, block, radar, treemap, xychart, all)"
        ))),
    }
}

fn check_upstream_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "er".to_string();
    let mut filter: Option<String> = None;
    let mut install: bool = false;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "strict".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--install" => install = true,
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "strict".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let baseline_root = workspace_root.join("fixtures").join("upstream-svgs");
    let out_root = workspace_root.join("target").join("upstream-svgs-check");

    let mut gen_args: Vec<String> = vec![
        "--diagram".to_string(),
        diagram.clone(),
        "--out".to_string(),
        out_root.to_string_lossy().to_string(),
    ];
    if let Some(f) = &filter {
        gen_args.push("--filter".to_string());
        gen_args.push(f.clone());
    }
    if install {
        gen_args.push("--install".to_string());
    }

    gen_upstream_svgs(gen_args)?;

    #[allow(clippy::too_many_arguments)]
    fn check_one(
        workspace_root: &Path,
        baseline_root: &Path,
        out_root: &Path,
        diagram: &str,
        filter: Option<&str>,
        check_dom: bool,
        dom_mode: svgdom::DomMode,
        dom_decimals: u32,
    ) -> Result<(), XtaskError> {
        let fixtures_dir = workspace_root.join("fixtures").join(diagram);
        let baseline_dir = baseline_root.join(diagram);
        let out_dir = out_root.join(diagram);

        let mut mmd_files: Vec<PathBuf> = Vec::new();
        let Ok(entries) = fs::read_dir(&fixtures_dir) else {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "failed to list fixtures directory {}",
                fixtures_dir.display()
            )));
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().is_none_or(|e| e != "mmd") {
                continue;
            }
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
            {
                continue;
            }
            if diagram == "gantt"
                && path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    matches!(
                        n,
                        "click_loose.mmd"
                            | "click_strict.mmd"
                            | "dateformat_hash_comment_truncates.mmd"
                            | "excludes_hash_comment_truncates.mmd"
                            | "today_marker_and_axis.mmd"
                    )
                })
            {
                continue;
            }
            if diagram == "state"
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("_parser_") || n.contains("_parser_spec"))
            {
                continue;
            }
            if diagram == "class"
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("upstream_text_label_variants_spec"))
            {
                continue;
            }
            if diagram == "c4"
                && path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    matches!(
                        n,
                        "nesting_updates.mmd"
                            | "upstream_boundary_spec.mmd"
                            | "upstream_c4container_header_and_direction_spec.mmd"
                            | "upstream_container_spec.mmd"
                            | "upstream_person_ext_spec.mmd"
                            | "upstream_person_spec.mmd"
                            | "upstream_system_spec.mmd"
                            | "upstream_update_element_style_all_fields_spec.mmd"
                    )
                })
            {
                continue;
            }
            if let Some(f) = filter {
                if !path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains(f))
                {
                    continue;
                }
            }
            mmd_files.push(path);
        }
        mmd_files.sort();

        if mmd_files.is_empty() {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "no .mmd fixtures matched under {}",
                fixtures_dir.display()
            )));
        }

        let mut mismatches: Vec<String> = Vec::new();
        for mmd_path in mmd_files {
            let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
                mismatches.push(format!("invalid fixture filename {}", mmd_path.display()));
                continue;
            };

            let baseline_path = baseline_dir.join(format!("{stem}.svg"));
            let out_path = out_dir.join(format!("{stem}.svg"));

            let baseline_svg = match fs::read_to_string(&baseline_path) {
                Ok(v) => v,
                Err(err) => {
                    mismatches.push(format!(
                        "missing baseline svg: {} ({err})",
                        baseline_path.display()
                    ));
                    continue;
                }
            };
            let out_svg = match fs::read_to_string(&out_path) {
                Ok(v) => v,
                Err(err) => {
                    mismatches.push(format!(
                        "missing generated svg: {} ({err})",
                        out_path.display()
                    ));
                    continue;
                }
            };

            let (use_dom, mode) = if check_dom {
                (true, dom_mode)
            } else if diagram == "state"
                || diagram == "gitgraph"
                || diagram == "gantt"
                || diagram == "er"
                || diagram == "class"
                || diagram == "requirement"
                || diagram == "block"
                || diagram == "mindmap"
                || diagram == "architecture"
            {
                (true, svgdom::DomMode::Structure)
            } else {
                (false, dom_mode)
            };

            if use_dom {
                let a = match svgdom::dom_signature(&baseline_svg, mode, dom_decimals) {
                    Ok(v) => v,
                    Err(err) => {
                        mismatches.push(format!(
                            "{diagram}/{stem}: baseline dom parse failed: {err}"
                        ));
                        continue;
                    }
                };
                let b = match svgdom::dom_signature(&out_svg, mode, dom_decimals) {
                    Ok(v) => v,
                    Err(err) => {
                        mismatches.push(format!(
                            "{diagram}/{stem}: generated dom parse failed: {err}"
                        ));
                        continue;
                    }
                };
                if a != b {
                    mismatches.push(format!("{diagram}/{stem}: dom differs from baseline"));
                }
            } else if baseline_svg != out_svg {
                mismatches.push(format!("{diagram}/{stem}: output differs from baseline"));
            }
        }

        if mismatches.is_empty() {
            Ok(())
        } else {
            Err(XtaskError::UpstreamSvgFailed(mismatches.join("\n")))
        }
    }

    let filter = filter.as_deref();
    let parsed_dom_mode = svgdom::DomMode::parse(&dom_mode);
    match diagram.as_str() {
        "all" => {
            let mut failures: Vec<String> = Vec::new();
            for d in [
                "er",
                "flowchart",
                "gantt",
                "architecture",
                "mindmap",
                "state",
                "class",
                "sequence",
                "info",
                "pie",
                "sankey",
                "requirement",
                "packet",
                "timeline",
                "journey",
                "kanban",
                "gitgraph",
                "quadrantchart",
                "c4",
                "block",
                "radar",
                "treemap",
            ] {
                if let Err(err) = check_one(
                    &workspace_root,
                    &baseline_root,
                    &out_root,
                    d,
                    filter,
                    check_dom,
                    parsed_dom_mode,
                    dom_decimals,
                ) {
                    failures.push(format!("{d}: {err}"));
                }
            }
            if failures.is_empty() {
                Ok(())
            } else {
                Err(XtaskError::UpstreamSvgFailed(failures.join("\n")))
            }
        }
        "er" | "flowchart" | "state" | "class" | "sequence" | "info" | "pie" | "requirement"
        | "sankey" | "packet" | "timeline" | "journey" | "kanban" | "gitgraph" | "gantt" | "c4"
        | "block" | "radar" | "quadrantchart" | "treemap" | "mindmap" | "architecture" => {
            check_one(
                &workspace_root,
                &baseline_root,
                &out_root,
                diagram.as_str(),
                filter,
                check_dom,
                parsed_dom_mode,
                dom_decimals,
            )
        }
        other => Err(XtaskError::UpstreamSvgFailed(format!(
            "unsupported diagram for upstream svg check: {other} (supported: er, flowchart, gantt, architecture, mindmap, state, class, sequence, info, pie, sankey, requirement, packet, timeline, journey, kanban, gitgraph, quadrantchart, c4, block, radar, treemap, all)"
        ))),
    }
}

fn find_mmdc(tools_root: &Path) -> Option<PathBuf> {
    let bin_root = tools_root.join("node_modules").join(".bin");
    for name in ["mmdc.cmd", "mmdc.ps1", "mmdc"] {
        let p = bin_root.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn ensure_seeded_upstream_svg_renderer_script(
    workspace_root: &Path,
) -> Result<PathBuf, XtaskError> {
    const JS: &str = r#"
const fs = require('fs');
const path = require('path');
const url = require('url');
const { createRequire } = require('module');
const requireFromCwd = createRequire(path.join(process.cwd(), 'package.json'));
const puppeteer = requireFromCwd('puppeteer');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const inputPath = String(input.input_path || '');
const outputPath = String(input.output_path || '');
const configPath = String(input.config_path || '');
const theme = String(input.theme || 'default');
const svgId = String(input.svg_id || 'diagram');
const seedStr = String((input.seed ?? 1));
const width = Number(input.width || 800);
const height = Number(input.height || 600);
const backgroundColor = String(input.background_color || 'white');

if (!inputPath || !outputPath || !configPath) {
  console.error('missing required input/output/config path');
  process.exit(2);
}

const cliRoot = process.cwd();
const mermaidHtmlPath = path.join(cliRoot, 'node_modules', '@mermaid-js', 'mermaid-cli', 'dist', 'index.html');
const mermaidIifePath = path.join(cliRoot, 'node_modules', 'mermaid', 'dist', 'mermaid.js');
const zenumlIifePath = path.join(cliRoot, 'node_modules', '@mermaid-js', 'mermaid-zenuml', 'dist', 'mermaid-zenuml.js');

(async () => {
  const code = fs.readFileSync(inputPath, 'utf8');
  const cfg = JSON.parse(fs.readFileSync(configPath, 'utf8'));

  const launchOpts = { headless: 'shell', args: ['--no-sandbox', '--disable-setuid-sandbox'] };
  const browser = await puppeteer.launch(launchOpts);
  const page = await browser.newPage();

  await page.evaluateOnNewDocument((seedStr) => {
    const mask64 = (1n << 64n) - 1n;
    let state = (BigInt(seedStr) & mask64);
    if (state === 0n) state = 1n;

    function nextU64() {
      let x = state;
      x ^= (x >> 12n);
      x ^= (x << 25n) & mask64;
      x ^= (x >> 27n);
      state = x;
      return (x * 0x2545F4914F6CDD1Dn) & mask64;
    }

    function nextF64() {
      const u = nextU64() >> 11n;
      return Number(u) / 9007199254740992; // 2^53
    }

    Math.random = nextF64;

    if (globalThis.crypto && typeof globalThis.crypto.getRandomValues === 'function') {
      const orig = globalThis.crypto.getRandomValues.bind(globalThis.crypto);
      globalThis.crypto.getRandomValues = (arr) => {
        if (!arr || typeof arr.length !== 'number') {
          return orig(arr);
        }
        for (let i = 0; i < arr.length; i++) {
          arr[i] = Math.floor(nextF64() * 256);
        }
        return arr;
      };
    }
  }, seedStr);

  await page.setViewport({ width: Math.max(1, width), height: Math.max(1, height), deviceScaleFactor: 1 });
  await page.goto(url.pathToFileURL(mermaidHtmlPath).href);
  await Promise.all([
    page.addScriptTag({ path: mermaidIifePath }),
    page.addScriptTag({ path: zenumlIifePath }),
  ]);

  const svg = await page.evaluate(async ({ code, cfg, theme, svgId, width }) => {
    const mermaid = globalThis.mermaid;
    if (!mermaid) throw new Error('mermaid global not found');

    if (document.fonts && typeof document.fonts[Symbol.iterator] === 'function') {
      await Promise.all(Array.from(document.fonts, (font) => font.load()));
    }

    // Match mermaid-cli behavior: register external diagrams and layout loaders.
    const zenuml = globalThis['mermaid-zenuml'];
    if (zenuml && typeof mermaid.registerExternalDiagrams === 'function') {
      await mermaid.registerExternalDiagrams([zenuml]);
    }
    const elkLayouts = globalThis.elkLayouts;
    if (elkLayouts && typeof mermaid.registerLayoutLoaders === 'function') {
      mermaid.registerLayoutLoaders(elkLayouts);
    }

    mermaid.initialize(Object.assign({ startOnLoad: false, theme }, cfg));

    const container = document.getElementById('container') || document.body;
    container.innerHTML = '';
    container.style.width = `${Math.max(1, Number(width) || 1)}px`;

    const { svg } = await mermaid.render(svgId, code, container);
    return svg;
  }, { code, cfg, theme, svgId, width });

  function ensureSvgBackgroundColor(svgText, bg) {
    if (typeof svgText !== 'string') {
      throw new Error(`expected svg string from mermaid.render, got ${typeof svgText}`);
    }
    if (!bg) return svgText;
    if (svgText.includes('background-color:')) return svgText;
    const m = svgText.match(/<svg\b[^>]*\bstyle="([^"]*)"/);
    if (m) {
      const raw = m[1] || '';
      let next = raw.trim();
      if (next.length > 0 && !next.trim().endsWith(';')) {
        next += ';';
      }
      next += ` background-color: ${bg};`;
      return svgText.replace(m[0], m[0].replace(raw, next));
    }
    // Fallback: inject a style attr into the root <svg>.
    return svgText.replace(/<svg\b/, `<svg style="background-color: ${bg};"`);
  }

  const svgWithBg = ensureSvgBackgroundColor(svg, backgroundColor);
  fs.writeFileSync(outputPath, svgWithBg, 'utf8');
  await browser.close();
})().catch((err) => {
  console.error(err && err.stack ? err.stack : String(err));
  process.exit(1);
});
"#;

    let dir = workspace_root.join("target").join("xtask-js");
    fs::create_dir_all(&dir).map_err(|source| XtaskError::WriteFile {
        path: dir.display().to_string(),
        source,
    })?;
    let script_path = dir.join("seeded-upstream-svg-render.js");
    fs::write(&script_path, JS).map_err(|source| XtaskError::WriteFile {
        path: script_path.display().to_string(),
        source,
    })?;
    Ok(script_path)
}

fn gen_er_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root = out_root.unwrap_or_else(|| workspace_root.join("target").join("svgs"));

    let fixtures_dir = workspace_root.join("fixtures").join("er");
    let out_dir = out_root.join("er");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "handDrawnSeed": 1 }),
    ));
    let layout_opts = merman_render::LayoutOptions::default();
    let mut failures: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(engine.parse_diagram(
            &text,
            merman::ParseOptions {
                suppress_errors: true,
            },
        )) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::ErDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let svg = match merman_render::svg::render_er_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let out_path = out_dir.join(format!("{stem}.svg"));
        if let Err(err) = fs::write(&out_path, svg) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::DebugSvgFailed(failures.join("\n")))
}

fn gen_debug_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "class".to_string();
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
            }
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root = out_root.unwrap_or_else(|| workspace_root.join("target").join("debug-svgs"));

    fn gen_one(
        workspace_root: &Path,
        out_root: &Path,
        diagram: &str,
        filter: Option<&str>,
    ) -> Result<(), XtaskError> {
        let (fixtures_dir, out_dir) = match diagram {
            "flowchart" | "flowchart-v2" | "flowchartV2" => (
                workspace_root.join("fixtures").join("flowchart"),
                out_root.join("flowchart"),
            ),
            "state" | "stateDiagram" | "stateDiagram-v2" | "stateDiagramV2" => (
                workspace_root.join("fixtures").join("state"),
                out_root.join("state"),
            ),
            "class" | "classDiagram" => (
                workspace_root.join("fixtures").join("class"),
                out_root.join("class"),
            ),
            "er" | "erDiagram" => (
                workspace_root.join("fixtures").join("er"),
                out_root.join("er"),
            ),
            "sequence" => (
                workspace_root.join("fixtures").join("sequence"),
                out_root.join("sequence"),
            ),
            "info" => (
                workspace_root.join("fixtures").join("info"),
                out_root.join("info"),
            ),
            "pie" => (
                workspace_root.join("fixtures").join("pie"),
                out_root.join("pie"),
            ),
            "packet" => (
                workspace_root.join("fixtures").join("packet"),
                out_root.join("packet"),
            ),
            other => {
                return Err(XtaskError::DebugSvgFailed(format!(
                    "unsupported diagram for debug svg export: {other} (supported: flowchart, state, class, er, sequence, info, pie, packet)"
                )));
            }
        };

        let mut mmd_files: Vec<PathBuf> = Vec::new();
        let Ok(entries) = fs::read_dir(&fixtures_dir) else {
            return Err(XtaskError::DebugSvgFailed(format!(
                "failed to list fixtures directory {}",
                fixtures_dir.display()
            )));
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().is_none_or(|e| e != "mmd") {
                continue;
            }
            if let Some(f) = filter {
                if !path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains(f))
                {
                    continue;
                }
            }
            mmd_files.push(path);
        }
        mmd_files.sort();

        if mmd_files.is_empty() {
            return Err(XtaskError::DebugSvgFailed(format!(
                "no .mmd fixtures matched under {}",
                fixtures_dir.display()
            )));
        }

        fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
            path: out_dir.display().to_string(),
            source,
        })?;

        let engine = merman::Engine::new();
        let mut failures: Vec<String> = Vec::new();

        for mmd_path in mmd_files {
            let text = match fs::read_to_string(&mmd_path) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                    continue;
                }
            };

            let parsed = match futures::executor::block_on(
                engine.parse_diagram(&text, merman::ParseOptions::default()),
            ) {
                Ok(Some(v)) => v,
                Ok(None) => {
                    failures.push(format!("no diagram detected in {}", mmd_path.display()));
                    continue;
                }
                Err(err) => {
                    failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                    continue;
                }
            };

            let layout_opts = merman_render::LayoutOptions::default();
            let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                    continue;
                }
            };

            let svg = match &layouted.layout {
                merman_render::model::LayoutDiagram::FlowchartV2(layout) => {
                    Ok(merman_render::svg::render_flowchart_v2_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    ))
                }
                merman_render::model::LayoutDiagram::StateDiagramV2(layout) => {
                    Ok(merman_render::svg::render_state_diagram_v2_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    ))
                }
                merman_render::model::LayoutDiagram::ClassDiagramV2(layout) => {
                    Ok(merman_render::svg::render_class_diagram_v2_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    ))
                }
                merman_render::model::LayoutDiagram::ErDiagram(layout) => {
                    Ok(merman_render::svg::render_er_diagram_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    ))
                }
                merman_render::model::LayoutDiagram::SequenceDiagram(layout) => {
                    Ok(merman_render::svg::render_sequence_diagram_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    ))
                }
                merman_render::model::LayoutDiagram::InfoDiagram(layout) => {
                    merman_render::svg::render_info_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "info svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                merman_render::model::LayoutDiagram::PieDiagram(layout) => {
                    merman_render::svg::render_pie_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "pie svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                merman_render::model::LayoutDiagram::PacketDiagram(layout) => {
                    merman_render::svg::render_packet_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        layouted.meta.title.as_deref(),
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "packet svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                merman_render::model::LayoutDiagram::TimelineDiagram(layout) => {
                    merman_render::svg::render_timeline_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        layouted.meta.title.as_deref(),
                        layout_opts.text_measurer.as_ref(),
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "timeline svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                merman_render::model::LayoutDiagram::JourneyDiagram(layout) => {
                    merman_render::svg::render_journey_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        layouted.meta.title.as_deref(),
                        layout_opts.text_measurer.as_ref(),
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "journey svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                merman_render::model::LayoutDiagram::KanbanDiagram(layout) => {
                    merman_render::svg::render_kanban_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "kanban svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                _ => Err(XtaskError::DebugSvgFailed(format!(
                    "unsupported layout for debug svg export: {} ({})",
                    mmd_path.display(),
                    layouted.meta.diagram_type
                ))),
            };

            let svg = match svg {
                Ok(v) => v,
                Err(err) => {
                    failures.push(err.to_string());
                    continue;
                }
            };

            let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
                failures.push(format!("invalid fixture filename {}", mmd_path.display()));
                continue;
            };
            let out_path = out_dir.join(format!("{stem}.svg"));
            if let Err(err) = fs::write(&out_path, svg) {
                failures.push(format!("failed to write {}: {err}", out_path.display()));
                continue;
            }
        }

        if failures.is_empty() {
            return Ok(());
        }

        Err(XtaskError::DebugSvgFailed(failures.join("\n")))
    }

    let filter = filter.as_deref();
    let diagrams: Vec<&str> = match diagram.as_str() {
        "all" => vec!["flowchart", "state", "class", "er"],
        other => vec![other],
    };

    let mut failures: Vec<String> = Vec::new();
    for d in diagrams {
        if let Err(err) = gen_one(&workspace_root, &out_root, d, filter) {
            failures.push(format!("{d}: {err}"));
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::DebugSvgFailed(failures.join("\n")))
}

fn check_alignment(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() && !(args.len() == 1 && (args[0] == "--help" || args[0] == "-h")) {
        return Err(XtaskError::Usage);
    }
    if args.len() == 1 {
        return Err(XtaskError::Usage);
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let alignment_dir = workspace_root.join("docs").join("alignment");
    let fixtures_root = workspace_root.join("fixtures");

    let mut failures: Vec<String> = Vec::new();

    // 1) Every *_MINIMUM.md should have a *_UPSTREAM_TEST_COVERAGE.md sibling.
    let mut minimum_docs: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&alignment_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.ends_with("_MINIMUM.md") {
                minimum_docs.push(path);
            }
        }
    }
    minimum_docs.sort();
    for min_path in &minimum_docs {
        let Some(stem) = min_path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix("_MINIMUM.md"))
        else {
            continue;
        };
        let cov = alignment_dir.join(format!("{stem}_UPSTREAM_TEST_COVERAGE.md"));
        if !cov.exists() {
            failures.push(format!(
                "missing upstream coverage doc for {stem}: expected {}",
                cov.display()
            ));
        }
    }

    fn strip_reference_suffix(s: &str) -> &str {
        // Normalize "path:line" and "path#Lline" forms to just "path" for existence checks.
        if let Some((left, right)) = s.rsplit_once(':') {
            if right.chars().all(|c| c.is_ascii_digit()) {
                return left;
            }
        }
        if let Some((left, right)) = s.rsplit_once("#L") {
            if right.chars().all(|c| c.is_ascii_digit()) {
                return left;
            }
        }
        s
    }

    fn is_probably_relative_path(s: &str) -> bool {
        s.starts_with("fixtures/")
            || s.starts_with("docs/")
            || s.starts_with("crates/")
            || s.starts_with("repo-ref/")
    }

    fn contains_glob(s: &str) -> bool {
        s.contains('*') || s.contains('?') || s.contains('[') || s.contains(']')
    }

    // 2) Every `fixtures/**/*.mmd` must have a sibling `.golden.json`.
    let mut mmd_files = Vec::new();
    let mut stack = vec![fixtures_root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|e| e == "mmd") {
                mmd_files.push(path);
            }
        }
    }
    mmd_files.sort();
    for mmd in &mmd_files {
        let golden = mmd.with_extension("golden.json");
        if !golden.exists() {
            failures.push(format!(
                "missing golden snapshot for fixture {} (expected {})",
                mmd.display(),
                golden.display()
            ));
        }
    }

    // 3) Coverage docs should not reference non-existent local files.
    let backtick_re = Regex::new(r"`([^`]+)`")
        .map_err(|e| XtaskError::AlignmentCheckFailed(format!("invalid regex: {e}")))?;

    let mut coverage_docs: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&alignment_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.ends_with("_UPSTREAM_TEST_COVERAGE.md") {
                coverage_docs.push(path);
            }
        }
    }
    coverage_docs.sort();

    for cov_path in &coverage_docs {
        let text = read_text(cov_path)?;
        for caps in backtick_re.captures_iter(&text) {
            let raw = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let raw = strip_reference_suffix(raw.trim());
            if raw.is_empty() {
                continue;
            }
            if !is_probably_relative_path(raw) {
                continue;
            }
            if contains_glob(raw) {
                continue;
            }
            let path = workspace_root.join(raw);
            // `repo-ref/*` repositories are optional workspace checkouts (not committed).
            // We only require `fixtures/`, `docs/`, and `crates/` references to exist.
            if raw.starts_with("repo-ref/") && !path.exists() {
                continue;
            }
            if !path.exists() {
                failures.push(format!(
                    "broken reference in {}: `{}` does not exist",
                    cov_path.display(),
                    raw
                ));
                continue;
            }
            if raw.starts_with("fixtures/") && raw.ends_with(".mmd") {
                let golden = path.with_extension("golden.json");
                if !golden.exists() {
                    failures.push(format!(
                        "broken reference in {}: missing golden for `{}` (expected {})",
                        cov_path.display(),
                        raw,
                        golden.display()
                    ));
                }
            }
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::AlignmentCheckFailed(failures.join("\n")))
}

fn gen_default_config(args: Vec<String>) -> Result<(), XtaskError> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Err(XtaskError::Usage);
    }

    let mut schema_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--schema" => {
                i += 1;
                schema_path = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let schema_path = schema_path.unwrap_or_else(|| {
        PathBuf::from("repo-ref/mermaid/packages/mermaid/src/schemas/config.schema.yaml")
    });
    let out_path = out_path
        .unwrap_or_else(|| PathBuf::from("crates/merman-core/src/generated/default_config.json"));

    let schema_text = fs::read_to_string(&schema_path).map_err(|source| XtaskError::ReadFile {
        path: schema_path.display().to_string(),
        source,
    })?;
    let schema_yaml: YamlValue = serde_yaml::from_str(&schema_text)?;

    let Some(root_defaults) = extract_defaults(&schema_yaml, &schema_yaml) else {
        return Err(XtaskError::InvalidRef(
            "schema produced no defaults (unexpected)".to_string(),
        ));
    };

    let pretty = serde_json::to_string_pretty(&root_defaults)?;
    let out_dir = out_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    fs::write(&out_path, pretty).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}

fn verify_generated(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() && !(args.len() == 1 && (args[0] == "--help" || args[0] == "-h")) {
        return Err(XtaskError::Usage);
    }
    if args.len() == 1 {
        return Err(XtaskError::Usage);
    }

    let tmp_dir = PathBuf::from("target/xtask");
    fs::create_dir_all(&tmp_dir).map_err(|source| XtaskError::WriteFile {
        path: tmp_dir.display().to_string(),
        source,
    })?;

    let mut failures = Vec::new();

    // Verify default config JSON.
    let expected_config = PathBuf::from("crates/merman-core/src/generated/default_config.json");
    let actual_config = tmp_dir.join("default_config.actual.json");
    gen_default_config(vec![
        "--schema".to_string(),
        "repo-ref/mermaid/packages/mermaid/src/schemas/config.schema.yaml".to_string(),
        "--out".to_string(),
        actual_config.display().to_string(),
    ])?;
    let expected_config_json: JsonValue = serde_json::from_str(&read_text(&expected_config)?)?;
    let actual_config_json: JsonValue = serde_json::from_str(&read_text(&actual_config)?)?;
    if expected_config_json != actual_config_json {
        failures.push(format!(
            "default config mismatch: regenerate with `cargo run -p xtask -- gen-default-config` ({})",
            expected_config.display()
        ));
    }

    // Verify DOMPurify allowlists.
    let expected_purify = PathBuf::from("crates/merman-core/src/generated/dompurify_defaults.rs");
    let actual_purify = tmp_dir.join("dompurify_defaults.actual.rs");
    gen_dompurify_defaults(vec![
        "--src".to_string(),
        "repo-ref/dompurify/dist/purify.cjs.js".to_string(),
        "--out".to_string(),
        actual_purify.display().to_string(),
    ])?;
    if read_text_normalized(&expected_purify)? != read_text_normalized(&actual_purify)? {
        failures.push(format!(
            "dompurify defaults mismatch: regenerate with `cargo run -p xtask -- gen-dompurify-defaults` ({})",
            expected_purify.display()
        ));
    }

    // Verify generated C4 type textLength table.
    let expected_c4_textlength =
        PathBuf::from("crates/merman-render/src/generated/c4_type_textlength_11_12_2.rs");
    let actual_c4_textlength = tmp_dir.join("c4_type_textlength_11_12_2.actual.rs");
    gen_c4_textlength(vec![
        "--in".to_string(),
        "fixtures/upstream-svgs/c4".to_string(),
        "--out".to_string(),
        actual_c4_textlength.display().to_string(),
    ])?;
    if read_text_normalized(&expected_c4_textlength)?
        != read_text_normalized(&actual_c4_textlength)?
    {
        failures.push(format!(
            "c4 textLength table mismatch: regenerate with `cargo run -p xtask -- gen-c4-textlength` ({})",
            expected_c4_textlength.display()
        ));
    }

    // Verify generated Flowchart font metrics table.
    let expected_flowchart_font_metrics =
        PathBuf::from("crates/merman-render/src/generated/font_metrics_flowchart_11_12_2.rs");
    let actual_flowchart_font_metrics = tmp_dir.join("font_metrics_flowchart_11_12_2.actual.rs");
    gen_font_metrics(vec![
        "--in".to_string(),
        "fixtures/upstream-svgs/flowchart".to_string(),
        "--out".to_string(),
        actual_flowchart_font_metrics.display().to_string(),
        "--font-size".to_string(),
        "16".to_string(),
    ])?;
    if read_text_normalized(&expected_flowchart_font_metrics)?
        != read_text_normalized(&actual_flowchart_font_metrics)?
    {
        failures.push(format!(
            "flowchart font metrics mismatch: regenerate with `cargo run -p xtask -- gen-font-metrics --in fixtures/upstream-svgs/flowchart --out crates/merman-render/src/generated/font_metrics_flowchart_11_12_2.rs --font-size 16` ({})",
            expected_flowchart_font_metrics.display()
        ));
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::VerifyFailed(failures.join("\n")))
}

fn update_snapshots(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_root = if diagram == "all" {
        workspace_root.join("fixtures")
    } else {
        workspace_root.join("fixtures").join(&diagram)
    };

    let mut mmd_files = Vec::new();
    let mut stack = vec![fixtures_root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|e| e == "mmd") {
                mmd_files.push(path);
            }
        }
    }
    mmd_files.sort();
    if let Some(f) = filter.as_deref() {
        mmd_files.retain(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
        });
    }
    if mmd_files.is_empty() {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "no .mmd fixtures found under {}",
            fixtures_root.display()
        )));
    }

    // Pin `handDrawnSeed` so Rough.js-dependent output is deterministic and comparable to
    // `fixtures/upstream-svgs/**` (generated with Mermaid config `handDrawnSeed: 1`).
    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "handDrawnSeed": 1 }),
    ));
    let mut failures = Vec::new();

    fn ms_to_local_iso(ms: i64) -> Option<String> {
        let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)?;
        Some(
            dt.with_timezone(&chrono::Local)
                .format("%Y-%m-%dT%H:%M:%S%.3f")
                .to_string(),
        )
    }

    let re_gitgraph_id = Regex::new(r"\b(\d+)-[0-9a-f]{7}\b")
        .map_err(|e| XtaskError::SnapshotUpdateFailed(format!("invalid gitGraph id regex: {e}")))?;
    let re_block_id = Regex::new(r"id-[a-z0-9]+-(\d+)")
        .map_err(|e| XtaskError::SnapshotUpdateFailed(format!("invalid block id regex: {e}")))?;

    fn walk_replace(re: &Regex, replacement: &str, v: &mut JsonValue) {
        match v {
            JsonValue::String(s) => {
                if re.is_match(s) {
                    *s = re.replace_all(s, replacement).to_string();
                }
            }
            JsonValue::Array(arr) => {
                for item in arr {
                    walk_replace(re, replacement, item);
                }
            }
            JsonValue::Object(map) => {
                for (_k, val) in map.iter_mut() {
                    walk_replace(re, replacement, val);
                }
            }
            _ => {}
        }
    }

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(engine.parse_diagram(
            &text,
            merman::ParseOptions {
                suppress_errors: true,
            },
        )) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        if diagram != "all" {
            let dt = parsed.meta.diagram_type.as_str();
            let matches = dt == diagram
                || (diagram == "er" && matches!(dt, "er" | "erDiagram"))
                || (diagram == "flowchart" && dt == "flowchart-v2")
                || (diagram == "state" && dt == "stateDiagram")
                || (diagram == "class" && matches!(dt, "class" | "classDiagram"))
                || (diagram == "gitgraph" && dt == "gitGraph")
                || (diagram == "quadrantchart" && dt == "quadrantChart");
            if !matches {
                continue;
            }
        }

        let mut model = parsed.model;
        if let JsonValue::Object(obj) = &mut model {
            obj.remove("config");
            if parsed.meta.diagram_type == "mindmap" && obj.get("diagramId").is_some() {
                obj.insert(
                    "diagramId".to_string(),
                    JsonValue::String("<dynamic>".to_string()),
                );
            }

            if parsed.meta.diagram_type == "gantt" {
                let date_format = obj
                    .get("dateFormat")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("")
                    .trim();
                if !matches!(date_format, "x" | "X") {
                    if let Some(tasks) = obj.get_mut("tasks").and_then(JsonValue::as_array_mut) {
                        for task in tasks {
                            let JsonValue::Object(task_obj) = task else {
                                continue;
                            };
                            for key in ["startTime", "endTime", "renderEndTime"] {
                                let Some(v) = task_obj.get_mut(key) else {
                                    continue;
                                };
                                let Some(ms) = v
                                    .as_i64()
                                    .or_else(|| v.as_u64().and_then(|n| i64::try_from(n).ok()))
                                else {
                                    continue;
                                };
                                if let Some(s) = ms_to_local_iso(ms) {
                                    *v = JsonValue::String(s);
                                }
                            }
                        }
                    }
                }
            }
        }

        if parsed.meta.diagram_type == "gitGraph" {
            walk_replace(&re_gitgraph_id, "$1-<dynamic>", &mut model);
        }

        if parsed.meta.diagram_type == "block" {
            walk_replace(&re_block_id, "id-<id>-$1", &mut model);
        }

        let out = serde_json::json!({
            "diagramType": parsed.meta.diagram_type,
            "model": model,
        });

        let pretty = match serde_json::to_string_pretty(&out) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "failed to serialize JSON for {}: {err}",
                    mmd_path.display()
                ));
                continue;
            }
        };

        let out_path = mmd_path.with_extension("golden.json");
        if let Some(parent) = out_path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                failures.push(format!("failed to create dir {}: {err}", parent.display()));
                continue;
            }
        }
        if let Err(err) = fs::write(&out_path, format!("{pretty}\n")) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::SnapshotUpdateFailed(failures.join("\n")))
}

fn read_text(path: &Path) -> Result<String, XtaskError> {
    fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })
}

fn read_text_normalized(path: &Path) -> Result<String, XtaskError> {
    let text = read_text(path)?;
    let normalized_line_endings = text.replace("\r\n", "\n");
    Ok(normalized_line_endings.trim_end().to_string())
}

fn gen_dompurify_defaults(args: Vec<String>) -> Result<(), XtaskError> {
    let mut src_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--src" => {
                i += 1;
                src_path = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let src_path =
        src_path.unwrap_or_else(|| PathBuf::from("repo-ref/dompurify/dist/purify.cjs.js"));
    let out_path = out_path
        .unwrap_or_else(|| PathBuf::from("crates/merman-core/src/generated/dompurify_defaults.rs"));

    let src_text = fs::read_to_string(&src_path).map_err(|source| XtaskError::ReadFile {
        path: src_path.display().to_string(),
        source,
    })?;

    let html_tags = extract_frozen_string_array(&src_text, "html$1")?;
    let svg_tags = extract_frozen_string_array(&src_text, "svg$1")?;
    let svg_filters = extract_frozen_string_array(&src_text, "svgFilters")?;
    let mathml_tags = extract_frozen_string_array(&src_text, "mathMl$1")?;

    let html_attrs = extract_frozen_string_array(&src_text, "html")?;
    let svg_attrs = extract_frozen_string_array(&src_text, "svg")?;
    let mathml_attrs = extract_frozen_string_array(&src_text, "mathMl")?;
    let xml_attrs = extract_frozen_string_array(&src_text, "xml")?;

    let default_data_uri_tags =
        extract_add_to_set_string_array(&src_text, "DEFAULT_DATA_URI_TAGS")?;
    let default_uri_safe_attrs =
        extract_add_to_set_string_array(&src_text, "DEFAULT_URI_SAFE_ATTRIBUTES")?;

    let allowed_tags = unique_sorted_lowercase(
        html_tags
            .into_iter()
            .chain(svg_tags)
            .chain(svg_filters)
            .chain(mathml_tags),
    );

    let allowed_attrs = unique_sorted_lowercase(
        html_attrs
            .into_iter()
            .chain(svg_attrs)
            .chain(mathml_attrs)
            .chain(xml_attrs),
    );

    let data_uri_tags = unique_sorted_lowercase(default_data_uri_tags);
    let uri_safe_attrs = unique_sorted_lowercase(default_uri_safe_attrs);

    let out_dir = out_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let rust = render_dompurify_defaults_rs(
        &allowed_tags,
        &allowed_attrs,
        &uri_safe_attrs,
        &data_uri_tags,
    );
    fs::write(&out_path, rust).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}

fn render_dompurify_defaults_rs(
    allowed_tags: &[String],
    allowed_attrs: &[String],
    uri_safe_attrs: &[String],
    data_uri_tags: &[String],
) -> String {
    fn render_slice(name: &str, values: &[String]) -> String {
        let mut out = String::new();
        // Keep small slices compact for readability and stable diffs.
        if values.len() <= 8 {
            out.push_str(&format!("pub const {name}: &[&str] = &["));
            for (i, v) in values.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&format!("{v:?}"));
            }
            out.push_str("];\n\n");
            return out;
        }
        out.push_str(&format!("pub const {name}: &[&str] = &[\n"));
        for v in values {
            out.push_str(&format!("    {v:?},\n"));
        }
        out.push_str("];\n\n");
        out
    }

    let mut out = String::new();
    out.push_str("// This file is @generated by `cargo run -p xtask -- gen-dompurify-defaults`.\n");
    out.push_str("// Source: `repo-ref/dompurify/dist/purify.cjs.js` (DOMPurify 3.2.5)\n\n");
    out.push_str(&render_slice("DEFAULT_ALLOWED_TAGS", allowed_tags));
    out.push_str(&render_slice("DEFAULT_ALLOWED_ATTR", allowed_attrs));
    out.push_str(&render_slice("DEFAULT_URI_SAFE_ATTRIBUTES", uri_safe_attrs));
    out.push_str(&render_slice("DEFAULT_DATA_URI_TAGS", data_uri_tags));
    out
}

fn unique_sorted_lowercase<I>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut set = std::collections::BTreeSet::new();
    for v in values {
        set.insert(v.to_ascii_lowercase());
    }
    set.into_iter().collect()
}

fn extract_add_to_set_string_array(src: &str, ident: &str) -> Result<Vec<String>, XtaskError> {
    let needle = format!("const {ident} = addToSet({{}}, [");
    let start = src
        .find(&needle)
        .ok_or_else(|| XtaskError::ParseDompurify(format!("missing {ident} definition")))?;
    let bracket_start = start + needle.len() - 1; // points at '['
    extract_string_array_at(src, bracket_start)
}

fn extract_frozen_string_array(src: &str, ident: &str) -> Result<Vec<String>, XtaskError> {
    let needle = format!("const {ident} = freeze([");
    let start = src
        .find(&needle)
        .ok_or_else(|| XtaskError::ParseDompurify(format!("missing {ident} definition")))?;
    let bracket_start = start + needle.len() - 1; // points at '['
    extract_string_array_at(src, bracket_start)
}

fn extract_string_array_at(src: &str, bracket_start: usize) -> Result<Vec<String>, XtaskError> {
    let bytes = src.as_bytes();
    if *bytes.get(bracket_start).unwrap_or(&0) != b'[' {
        return Err(XtaskError::ParseDompurify("expected array '['".to_string()));
    }

    let mut out: Vec<String> = Vec::new();
    let mut i = bracket_start + 1;
    let mut in_string = false;
    let mut cur = String::new();

    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            match b {
                b'\\' => {
                    // Minimal escape handling: keep the escaped character verbatim.
                    if i + 1 >= bytes.len() {
                        return Err(XtaskError::ParseDompurify(
                            "unterminated escape".to_string(),
                        ));
                    }
                    let next = bytes[i + 1] as char;
                    cur.push(next);
                    i += 2;
                    continue;
                }
                b'\'' => {
                    out.push(cur.clone());
                    cur.clear();
                    in_string = false;
                    i += 1;
                    continue;
                }
                _ => {
                    cur.push(b as char);
                    i += 1;
                    continue;
                }
            }
        }

        match b {
            b'\'' => {
                in_string = true;
                i += 1;
            }
            b']' => return Ok(out),
            _ => i += 1,
        }
    }

    Err(XtaskError::ParseDompurify("unterminated array".to_string()))
}

fn extract_defaults(schema: &YamlValue, root: &YamlValue) -> Option<JsonValue> {
    let schema = expand_schema(schema, root);

    if let Some(default) = schema
        .as_mapping()
        .and_then(|m| m.get(YamlValue::String("default".to_string())))
    {
        return yaml_to_json(default).ok();
    }

    if let Some(any_of) = schema
        .as_mapping()
        .and_then(|m| m.get(YamlValue::String("anyOf".to_string())))
        .and_then(|v| v.as_sequence())
    {
        for s in any_of {
            if let Some(d) = extract_defaults(s, root) {
                return Some(d);
            }
        }
    }

    if let Some(one_of) = schema
        .as_mapping()
        .and_then(|m| m.get(YamlValue::String("oneOf".to_string())))
        .and_then(|v| v.as_sequence())
    {
        for s in one_of {
            if let Some(d) = extract_defaults(s, root) {
                return Some(d);
            }
        }
    }

    let is_object_type = schema
        .as_mapping()
        .and_then(|m| m.get(YamlValue::String("type".to_string())))
        .and_then(|v| v.as_str())
        == Some("object");

    let props = schema
        .as_mapping()
        .and_then(|m| m.get(YamlValue::String("properties".to_string())))
        .and_then(|v| v.as_mapping());

    if is_object_type || props.is_some() {
        let mut out: BTreeMap<String, JsonValue> = BTreeMap::new();
        if let Some(props) = props {
            for (k, v) in props {
                let Some(k) = k.as_str() else { continue };
                if let Some(d) = extract_defaults(v, root) {
                    out.insert(k.to_string(), d);
                }
            }
        }
        if out.is_empty() {
            return None;
        }
        return Some(JsonValue::Object(out.into_iter().collect()));
    }

    None
}

fn expand_schema(schema: &YamlValue, root: &YamlValue) -> YamlValue {
    let mut schema = schema.clone();
    schema = resolve_ref(&schema, root).unwrap_or(schema);

    let all_of = schema
        .as_mapping()
        .and_then(|m| m.get(YamlValue::String("allOf".to_string())))
        .and_then(|v| v.as_sequence())
        .cloned();

    if let Some(all_of) = all_of {
        let mut merged = schema.clone();
        if let Some(m) = merged.as_mapping_mut() {
            m.remove(YamlValue::String("allOf".to_string()));
        }
        for s in all_of {
            let s = expand_schema(&s, root);
            merged = merge_yaml(merged, s);
        }
        merged
    } else {
        schema
    }
}

fn resolve_ref(schema: &YamlValue, root: &YamlValue) -> Result<YamlValue, XtaskError> {
    let Some(map) = schema.as_mapping() else {
        return Ok(schema.clone());
    };
    let Some(ref_str) = map
        .get(YamlValue::String("$ref".to_string()))
        .and_then(|v| v.as_str())
    else {
        return Ok(schema.clone());
    };
    let target = resolve_ref_target(ref_str, root)?;
    let mut base = expand_schema(target, root);

    // Overlay other keys on top of the resolved target.
    let mut overlay = YamlValue::Mapping(map.clone());
    if let Some(m) = overlay.as_mapping_mut() {
        m.remove(YamlValue::String("$ref".to_string()));
    }
    base = merge_yaml(base, overlay);
    Ok(base)
}

fn resolve_ref_target<'a>(r: &str, root: &'a YamlValue) -> Result<&'a YamlValue, XtaskError> {
    if !r.starts_with("#/") {
        return Err(XtaskError::InvalidRef(r.to_string()));
    }
    let mut cur = root;
    for seg in r.trim_start_matches("#/").split('/') {
        let Some(map) = cur.as_mapping() else {
            return Err(XtaskError::UnresolvedRef(r.to_string()));
        };
        let key = YamlValue::String(seg.to_string());
        cur = map
            .get(&key)
            .ok_or_else(|| XtaskError::UnresolvedRef(r.to_string()))?;
    }
    Ok(cur)
}

fn merge_yaml(mut base: YamlValue, overlay: YamlValue) -> YamlValue {
    match (&mut base, overlay) {
        (YamlValue::Mapping(dst), YamlValue::Mapping(src)) => {
            for (k, v) in src {
                match dst.get_mut(&k) {
                    Some(existing) => {
                        let merged = merge_yaml(existing.clone(), v);
                        *existing = merged;
                    }
                    None => {
                        dst.insert(k, v);
                    }
                }
            }
            base
        }
        (_, v) => v,
    }
}

fn yaml_to_json(v: &YamlValue) -> Result<JsonValue, serde_json::Error> {
    serde_json::to_value(v)
}

fn gen_flowchart_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root = out_root.unwrap_or_else(|| workspace_root.join("target").join("svgs"));

    let fixtures_dir = workspace_root.join("fixtures").join("flowchart");
    let out_dir = out_root.join("flowchart");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let engine = merman::Engine::new();
    let layout_opts = merman_render::LayoutOptions::default();
    let mut failures: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::FlowchartV2(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let svg = match merman_render::svg::render_flowchart_v2_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let out_path = out_dir.join(format!("{stem}.svg"));
        if let Err(err) = fs::write(&out_path, svg) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::DebugSvgFailed(failures.join("\n")))
}

fn gen_state_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root = out_root.unwrap_or_else(|| workspace_root.join("target").join("svgs"));

    let fixtures_dir = workspace_root.join("fixtures").join("state");
    let out_dir = out_root.join("state");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let engine = merman::Engine::new();
    let layout_opts = merman_render::LayoutOptions::default();
    let mut failures: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::StateDiagramV2(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let svg = match merman_render::svg::render_state_diagram_v2_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let out_path = out_dir.join(format!("{stem}.svg"));
        if let Err(err) = fs::write(&out_path, svg) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::DebugSvgFailed(failures.join("\n")))
}

fn gen_class_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root = out_root.unwrap_or_else(|| workspace_root.join("target").join("svgs"));

    let fixtures_dir = workspace_root.join("fixtures").join("class");
    let out_dir = out_root.join("class");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let engine = merman::Engine::new();
    let layout_opts = merman_render::LayoutOptions::default();
    let mut failures: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let is_classdiagram_v2_header = merman::preprocess_diagram(&text, engine.registry())
            .ok()
            .map(|p| p.code.trim_start().starts_with("classDiagram-v2"))
            .unwrap_or(false);

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::ClassDiagramV2(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            aria_roledescription: is_classdiagram_v2_header.then(|| "classDiagram".to_string()),
            ..Default::default()
        };

        let svg = match merman_render::svg::render_class_diagram_v2_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let out_path = out_dir.join(format!("{stem}.svg"));
        if let Err(err) = fs::write(&out_path, svg) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::DebugSvgFailed(failures.join("\n")))
}

fn compare_flowchart_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut report_root: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();
    let mut text_measurer: String = "vendored".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--report-root" => report_root = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "structure".to_string());
            }
            "--text-measurer" => {
                i += 1;
                text_measurer = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "deterministic".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("flowchart");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("flowchart");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("flowchart_report.md")
    });
    let out_svg_dir = out_path
        .parent()
        .unwrap_or(&workspace_root)
        .join("flowchart");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);
    let should_report_root = report_root || mode == svgdom::DomMode::ParityRoot;

    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "handDrawnSeed": 1 }),
    ));
    let mut layout_opts = merman_render::LayoutOptions::default();
    if matches!(
        text_measurer.as_str(),
        "vendored" | "vendored-font" | "vendored-font-metrics"
    ) {
        layout_opts.text_measurer =
            std::sync::Arc::new(merman_render::text::VendoredFontMetricsTextMeasurer::default());
    }
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Flowchart SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/flowchart/*.svg` (Mermaid 11.12.2)\n- Local: `render_flowchart_v2_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n- Text measurer: `{}`\n",
        dom_mode, dom_decimals, text_measurer
    );

    #[derive(Debug, Clone)]
    struct RootAttrs {
        viewbox: Option<(f64, f64, f64, f64)>,
        max_width_px: Option<f64>,
    }

    fn parse_viewbox(v: &str) -> Option<(f64, f64, f64, f64)> {
        let parts = v
            .split_whitespace()
            .filter_map(|t| t.parse::<f64>().ok())
            .collect::<Vec<_>>();
        if parts.len() == 4 {
            Some((parts[0], parts[1], parts[2], parts[3]))
        } else {
            None
        }
    }

    fn parse_style_max_width_px(style: &str) -> Option<f64> {
        let style = style.to_ascii_lowercase();
        let key = "max-width:";
        let i = style.find(key)?;
        let rest = &style[i + key.len()..];
        let rest = rest.trim_start();
        let mut num = String::new();
        for ch in rest.chars() {
            if ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+' | 'e' | 'E') {
                num.push(ch);
            } else {
                break;
            }
        }
        num.trim().parse::<f64>().ok()
    }

    fn parse_root_attrs(svg: &str) -> Result<RootAttrs, String> {
        let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;
        let root = doc
            .descendants()
            .find(|n| n.has_tag_name("svg"))
            .ok_or_else(|| "missing <svg> root".to_string())?;
        let viewbox = root.attribute("viewBox").and_then(parse_viewbox);
        let max_width_px = root
            .attribute("style")
            .and_then(parse_style_max_width_px)
            .filter(|v| v.is_finite() && *v > 0.0);
        Ok(RootAttrs {
            viewbox,
            max_width_px,
        })
    }

    #[derive(Debug, Clone)]
    struct RootDelta {
        stem: String,
        upstream: RootAttrs,
        local: RootAttrs,
        max_width_delta: Option<f64>,
    }

    let mut root_deltas: Vec<RootDelta> = Vec::new();

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let diagram_id: String = stem
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg {}: {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::FlowchartV2(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_flowchart_v2_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if should_report_root {
            match (
                parse_root_attrs(&upstream_svg),
                parse_root_attrs(&local_svg),
            ) {
                (Ok(up), Ok(lo)) => {
                    let max_width_delta = match (up.max_width_px, lo.max_width_px) {
                        (Some(a), Some(b)) => Some(b - a),
                        _ => None,
                    };
                    root_deltas.push(RootDelta {
                        stem: stem.to_string(),
                        upstream: up,
                        local: lo,
                        max_width_delta,
                    });
                }
                (Err(e), _) => failures.push(format!("root parse failed for upstream {stem}: {e}")),
                (_, Err(e)) => failures.push(format!("root parse failed for local {stem}: {e}")),
            }
        }

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("upstream dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("local dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if should_report_root && !root_deltas.is_empty() {
        let _ = writeln!(
            &mut report,
            "\n## Root Viewport Deltas (max-width/viewBox)\n\nThis section is mainly useful when `--dom-mode parity-root` is enabled.\n"
        );

        root_deltas.sort_by(|a, b| {
            a.max_width_delta
                .unwrap_or(0.0)
                .abs()
                .partial_cmp(&b.max_width_delta.unwrap_or(0.0).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
                .reverse()
        });

        let take = root_deltas.len().min(25);
        let _ = writeln!(
            &mut report,
            "| Fixture | upstream max-width(px) | local max-width(px) | Δ | upstream viewBox(w×h) | local viewBox(w×h) |\n|---|---:|---:|---:|---:|---:|"
        );
        for d in root_deltas.iter().take(take) {
            let (up_mw, lo_mw, mw_delta) = match (d.upstream.max_width_px, d.local.max_width_px) {
                (Some(a), Some(b)) => (
                    format!("{a:.3}"),
                    format!("{b:.3}"),
                    format!("{:+.3}", b - a),
                ),
                _ => ("".to_string(), "".to_string(), "".to_string()),
            };
            let (up_vb, lo_vb) = match (d.upstream.viewbox, d.local.viewbox) {
                (Some((_, _, w, h)), Some((_, _, w2, h2))) => {
                    (format!("{w:.3}×{h:.3}"), format!("{w2:.3}×{h2:.3}"))
                }
                (Some((_, _, w, h)), None) => (format!("{w:.3}×{h:.3}"), "".to_string()),
                (None, Some((_, _, w, h))) => ("".to_string(), format!("{w:.3}×{h:.3}")),
                _ => ("".to_string(), "".to_string()),
            };
            let _ = writeln!(
                &mut report,
                "| `{}` | {} | {} | {} | {} | {} |",
                d.stem, up_mw, lo_mw, mw_delta, up_vb, lo_vb
            );
        }
        let _ = writeln!(
            &mut report,
            "\nNote: These deltas are a symptom of numeric layout/text-metrics drift; matching them requires moving closer to upstream measurement behavior.\n"
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_sequence_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "structure".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("sequence");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("sequence");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("sequence_report.md")
    });
    let out_svg_dir = out_path
        .parent()
        .unwrap_or(&workspace_root)
        .join("sequence");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);

    let engine = merman::Engine::new();
    let parse_opts = merman::ParseOptions {
        suppress_errors: true,
    };
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Sequence SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/sequence/*.svg` (Mermaid 11.12.2)\n- Local: `render_sequence_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let diagram_id: String = stem
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg {}: {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(engine.parse_diagram(&text, parse_opts)) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = merman_render::LayoutOptions {
            text_measurer: std::sync::Arc::new(
                merman_render::text::VendoredFontMetricsTextMeasurer::default(),
            ),
            ..Default::default()
        };
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::SequenceDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_sequence_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("upstream dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("local dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_info_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "structure".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("info");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("info");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("info_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("info");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);

    let engine = merman::Engine::new();

    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Info SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/info/*.svg` (Mermaid 11.12.2)\n- Local: `render_info_diagram_svg`\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };
        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg {}: {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::InfoDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_info_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("upstream dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("local dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_pie_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "structure".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("pie");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("pie");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("pie_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("pie");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);
    let engine = merman::Engine::new();

    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Pie SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/pie/*.svg` (Mermaid 11.12.2)\n- Local: `render_pie_diagram_svg`\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };
        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg {}: {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::PieDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_pie_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("upstream dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("local dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_packet_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "structure".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("packet");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("packet");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("packet_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("packet");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);
    let engine = merman::Engine::new();

    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Packet SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/packet/*.svg` (Mermaid 11.12.2)\n- Local: `render_packet_diagram_svg`\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };
        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg {}: {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::PacketDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_packet_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("upstream dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("local dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_timeline_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "structure".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("timeline");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("timeline");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("timeline_report.md")
    });
    let out_svg_dir = out_path
        .parent()
        .unwrap_or(&workspace_root)
        .join("timeline");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);
    let engine = merman::Engine::new();

    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Timeline SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/timeline/*.svg` (Mermaid 11.12.2)\n- Local: `render_timeline_diagram_svg`\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };
        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg {}: {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::TimelineDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_timeline_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("upstream dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("local dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_journey_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "structure".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("journey");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("journey");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("journey_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("journey");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);
    let engine = merman::Engine::new();

    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Journey SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/journey/*.svg` (Mermaid 11.12.2)\n- Local: `render_journey_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };
        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg {}: {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::JourneyDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_journey_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("upstream dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("local dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_class_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();
    let mut check_dom: bool = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "structure".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let mode = svgdom::DomMode::parse(&dom_mode);
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("class");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("class");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("class_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("class");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
            n.contains("upstream_text_label_variants_spec")
                || n.contains("upstream_parser_class_spec")
        }) {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let engine = merman::Engine::new();
    let layout_opts = svg_compare_layout_opts();

    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# ClassDiagram SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/class/*.svg` (Mermaid 11.12.2)\n- Local: `render_class_diagram_v2_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };
        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {stem}: {} ({err})",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::ClassDiagramV2(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let is_classdiagram_v2_header = merman::preprocess_diagram(&text, engine.registry())
            .ok()
            .map(|p| p.code.trim_start().starts_with("classDiagram-v2"))
            .unwrap_or(false);

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            aria_roledescription: is_classdiagram_v2_header.then(|| "classDiagram".to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_class_diagram_v2_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("upstream dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("local dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_kanban_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "structure".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("kanban");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("kanban");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("kanban_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("kanban");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);
    let engine = merman::Engine::new();

    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Kanban SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/kanban/*.svg` (Mermaid 11.12.2)\n- Local: `render_kanban_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };
        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg {}: {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::KanbanDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_kanban_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("upstream dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("local dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_gitgraph_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "structure".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("gitgraph");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("gitgraph");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("gitgraph_report.md")
    });
    let out_svg_dir = out_path
        .parent()
        .unwrap_or(&workspace_root)
        .join("gitgraph");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);

    // Mermaid gitGraph auto-generates commit ids via `Math.random()`. Upstream gitGraph SVG
    // baselines in this repo are generated with a seeded renderer, so keep the local side seeded
    // too for meaningful parity-root comparisons (root viewBox/max-width depend on label widths).
    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "gitGraph": { "seed": 1 } }),
    ));

    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# GitGraph SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/gitgraph/*.svg` (Mermaid 11.12.2)\n- Local: `render_gitgraph_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };
        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg {}: {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::GitGraphDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_gitgraph_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("upstream dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("local dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_gantt_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "structure".to_string();
    let mut check_dom: bool = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "structure".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("gantt");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("gantt");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("gantt_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("gantt");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }

        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if matches!(
            name,
            "today_marker_and_axis.mmd"
                | "click_loose.mmd"
                | "click_strict.mmd"
                | "dateformat_hash_comment_truncates.mmd"
                | "excludes_hash_comment_truncates.mmd"
        ) {
            continue;
        }

        if let Some(ref f) = filter {
            if !name.contains(f) {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);
    let engine = merman::Engine::new();

    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Gantt SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/gantt/*.svg` (Mermaid 11.12.2)\n- Local: `render_gantt_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };
        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg {}: {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::GanttDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let now_ms_override = (|| {
            let doc = roxmltree::Document::parse(&upstream_svg).ok()?;
            let x1 = doc
                .descendants()
                .filter(|n| n.has_tag_name("line"))
                .find(|n| {
                    n.attribute("class")
                        .unwrap_or_default()
                        .split_whitespace()
                        .any(|t| t == "today")
                })
                .and_then(|n| n.attribute("x1"))
                .and_then(|v| v.parse::<f64>().ok())?;
            if !x1.is_finite() {
                return None;
            }

            let min_ms = layout.tasks.iter().map(|t| t.start_ms).min()?;
            let max_ms = layout.tasks.iter().map(|t| t.end_ms).max()?;
            if max_ms <= min_ms {
                return None;
            }
            let range = (layout.width - layout.left_padding - layout.right_padding).max(1.0);

            fn gantt_today_x(
                now_ms: i64,
                min_ms: i64,
                max_ms: i64,
                range: f64,
                left_padding: f64,
            ) -> f64 {
                if max_ms <= min_ms {
                    return left_padding + (range / 2.0).round();
                }
                let t = (now_ms - min_ms) as f64 / (max_ms - min_ms) as f64;
                left_padding + (t * range).round()
            }

            let target_x = x1;
            let span = (max_ms - min_ms) as f64;
            let scaled = target_x - layout.left_padding;
            if !(span.is_finite() && scaled.is_finite() && range.is_finite()) {
                return None;
            }
            let est = (min_ms as f64) + span * (scaled / range);
            if !est.is_finite() {
                return None;
            }
            let mut lo = est.round() as i64;
            let mut hi = lo;
            let mut step: i64 = 1;

            let mut guard = 0;
            while guard < 80 {
                guard += 1;
                let x_lo = gantt_today_x(lo, min_ms, max_ms, range, layout.left_padding);
                if x_lo.is_nan() {
                    return None;
                }
                if x_lo <= target_x {
                    break;
                }
                hi = lo;
                lo = lo.saturating_sub(step);
                step = step.saturating_mul(2);
            }

            guard = 0;
            step = 1;
            while guard < 80 {
                guard += 1;
                let x_hi = gantt_today_x(hi, min_ms, max_ms, range, layout.left_padding);
                if x_hi.is_nan() {
                    return None;
                }
                if x_hi >= target_x {
                    break;
                }
                lo = hi;
                hi = hi.saturating_add(step);
                step = step.saturating_mul(2);
            }

            let x_lo = gantt_today_x(lo, min_ms, max_ms, range, layout.left_padding);
            let x_hi = gantt_today_x(hi, min_ms, max_ms, range, layout.left_padding);
            if !(x_lo <= target_x && target_x <= x_hi) {
                return None;
            }

            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                let x_mid = gantt_today_x(mid, min_ms, max_ms, range, layout.left_padding);
                if x_mid < target_x {
                    lo = mid.saturating_add(1);
                } else {
                    hi = mid;
                }
            }
            let x = gantt_today_x(lo, min_ms, max_ms, range, layout.left_padding);
            if x == target_x { Some(lo) } else { None }
        })();

        let diagram_id: String = stem
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            now_ms_override,
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_gantt_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("upstream dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("local dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_c4_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "structure".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("c4");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("c4");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("c4_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("c4");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        // Keep C4 fixtures that Mermaid CLI can render (baselines exist).
        if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
            matches!(
                n,
                "nesting_updates.mmd"
                    | "upstream_boundary_spec.mmd"
                    | "upstream_c4container_header_and_direction_spec.mmd"
                    | "upstream_container_spec.mmd"
                    | "upstream_person_ext_spec.mmd"
                    | "upstream_person_spec.mmd"
                    | "upstream_system_spec.mmd"
                    | "upstream_update_element_style_all_fields_spec.mmd"
            )
        }) {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);

    let engine = merman::Engine::new();
    let parse_opts = merman::ParseOptions {
        suppress_errors: true,
    };
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# C4 SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/c4/*.svg` (Mermaid 11.12.2)\n- Local: `render_c4_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg {}: {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(engine.parse_diagram(&text, parse_opts)) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::C4Diagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_c4_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("upstream dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("local dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_block_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "structure".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("block");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("block");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("block_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("block");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);

    let engine = merman::Engine::new();
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Block SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/block/*.svg` (Mermaid 11.12.2)\n- Local: `render_block_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let diagram_id: String = stem
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {stem}: {} ({err})",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::BlockDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_block_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse upstream svg dom for {stem} ({}): {err}",
                        upstream_path.display()
                    ));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse local svg dom for {stem} ({}): {err}",
                        local_out_path.display()
                    ));
                    continue;
                }
            };

            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_radar_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("radar");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("radar");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("radar_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("radar");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);

    let engine = merman::Engine::new();
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Radar SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/radar/*.svg` (Mermaid 11.12.2)\n- Local: `render_radar_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let diagram_id: String = stem
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg {}: {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(engine.parse_diagram(
            &text,
            merman::ParseOptions {
                suppress_errors: true,
            },
        )) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_layouted_svg(
            &layouted,
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("upstream dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("local dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_treemap_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("treemap");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("treemap");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("treemap_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("treemap");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);

    let engine = merman::Engine::new();
    let parse_opts = merman::ParseOptions {
        suppress_errors: true,
    };
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Treemap SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/treemap/*.svg` (Mermaid 11.12.2)\n- Local: `render_treemap_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let diagram_id: String = stem
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {stem}: {} ({err})",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(engine.parse_diagram(&text, parse_opts)) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_layouted_svg(
            &layouted,
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse upstream svg dom for {stem} ({}): {err}",
                        upstream_path.display()
                    ));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse local svg dom for {stem} ({}): {err}",
                        local_out_path.display()
                    ));
                    continue;
                }
            };

            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_requirement_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("requirement");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("requirement");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("requirement_report.md")
    });
    let out_svg_dir = out_path
        .parent()
        .unwrap_or(&workspace_root)
        .join("requirement");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);
    let mut report = String::new();
    let _ = write!(
        &mut report,
        "# Requirement SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/requirement/*.svg` (Mermaid 11.12.2)\n- Local: `render_requirement_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n\n",
        dom_mode, dom_decimals
    );
    let mut failures: Vec<String> = Vec::new();

    let engine = merman::Engine::new();

    for mmd_path in &mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        if !upstream_path.is_file() {
            failures.push(format!(
                "missing upstream svg baseline for {stem}: {}",
                upstream_path.display()
            ));
            continue;
        }
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "failed to read upstream svg for {stem} ({}): {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let diagram_id: String = stem.to_string();

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::RequirementDiagram(layout) = &layouted.layout
        else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_requirement_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse upstream svg dom for {stem} ({}): {err}",
                        upstream_path.display()
                    ));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse local svg dom for {stem} ({}): {err}",
                        local_out_path.display()
                    ));
                    continue;
                }
            };

            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        } else if upstream_svg != local_svg {
            failures.push(format!("svg mismatch for {stem}"));
        }

        let status = if failures.iter().any(|f| f.contains(stem)) {
            "FAIL"
        } else {
            "PASS"
        };
        let _ = writeln!(
            &mut report,
            "- {status} `{stem}`\n  - fixture: `{}`\n  - upstream: `{}`\n  - local: `{}`",
            mmd_path.display(),
            upstream_path.display(),
            out_svg_dir.join(format!("{stem}.svg")).display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_quadrantchart_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("quadrantchart");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("quadrantchart");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("quadrantchart_report.md")
    });
    let out_svg_dir = out_path
        .parent()
        .unwrap_or(&workspace_root)
        .join("quadrantchart");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);
    let mut report = String::new();
    let _ = write!(
        &mut report,
        "# QuadrantChart SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/quadrantchart/*.svg` (Mermaid 11.12.2)\n- Local: `render_quadrantchart_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n\n",
        dom_mode, dom_decimals
    );
    let mut failures: Vec<String> = Vec::new();

    let engine = merman::Engine::new();

    for mmd_path in &mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        if !upstream_path.is_file() {
            failures.push(format!(
                "missing upstream svg baseline for {stem}: {}",
                upstream_path.display()
            ));
            continue;
        }
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "failed to read upstream svg for {stem} ({}): {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let diagram_id: String = stem.to_string();

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::QuadrantChartDiagram(layout) = &layouted.layout
        else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_quadrantchart_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse upstream svg dom for {stem} ({}): {err}",
                        upstream_path.display()
                    ));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse local svg dom for {stem} ({}): {err}",
                        local_out_path.display()
                    ));
                    continue;
                }
            };

            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        } else if upstream_svg != local_svg {
            failures.push(format!("svg mismatch for {stem}"));
        }

        let status = if failures.iter().any(|f| f.contains(stem)) {
            "FAIL"
        } else {
            "PASS"
        };
        let _ = writeln!(
            &mut report,
            "- {status} `{stem}`\n  - fixture: `{}`\n  - upstream: `{}`\n  - local: `{}`",
            mmd_path.display(),
            upstream_path.display(),
            out_svg_dir.join(format!("{stem}.svg")).display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_xychart_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("xychart");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("xychart");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("xychart_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("xychart");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);

    let engine = merman::Engine::new();
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# XYChart SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/xychart/*.svg` (Mermaid 11.12.2)\n- Local: `render_xychart_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let diagram_id: String = stem
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {stem}: {} ({err})",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::XyChartDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_xychart_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse upstream svg dom for {stem} ({}): {err}",
                        upstream_path.display()
                    ));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse local svg dom for {stem} ({}): {err}",
                        local_out_path.display()
                    ));
                    continue;
                }
            };

            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_mindmap_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("mindmap");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("mindmap");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("mindmap_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("mindmap");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);

    let engine = merman::Engine::new();
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Mindmap SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/mindmap/*.svg` (Mermaid 11.12.2)\n- Local: `render_mindmap_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let diagram_id: String = stem
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {stem}: {} ({err})",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::MindmapDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_mindmap_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse upstream svg dom for {stem} ({}): {err}",
                        upstream_path.display()
                    ));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse local svg dom for {stem} ({}): {err}",
                        local_out_path.display()
                    ));
                    continue;
                }
            };

            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_sankey_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity-root".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("sankey");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("sankey");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("sankey_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("sankey");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);

    let engine = merman::Engine::new();
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Sankey SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/sankey/*.svg` (Mermaid 11.12.2)\n- Local: `render_sankey_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let diagram_id: String = stem
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {stem}: {} ({err})",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::SankeyDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_sankey_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse upstream svg dom for {stem} ({}): {err}",
                        upstream_path.display()
                    ));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse local svg dom for {stem} ({}): {err}",
                        local_out_path.display()
                    ));
                    continue;
                }
            };

            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_architecture_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("architecture");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("architecture");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("architecture_report.md")
    });
    let out_svg_dir = out_path
        .parent()
        .unwrap_or(&workspace_root)
        .join("architecture");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);

    let engine = merman::Engine::new();
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Architecture SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/architecture/*.svg` (Mermaid 11.12.2)\n- Local: `render_architecture_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let diagram_id: String = stem
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {stem}: {} ({err})",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::ArchitectureDiagram(layout) = &layouted.layout
        else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_architecture_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse upstream svg dom for {stem} ({}): {err}",
                        upstream_path.display()
                    ));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse local svg dom for {stem} ({}): {err}",
                        local_out_path.display()
                    ));
                    continue;
                }
            };

            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn compare_state_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "structure".to_string();
    let mut check_dom: bool = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "structure".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("state");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("state");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("state_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("state");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_") || n.contains("_parser_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);

    let engine = merman::Engine::new();
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# StateDiagram SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/state/*.svg` (Mermaid 11.12.2)\n- Local: `render_state_diagram_v2_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };
        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {stem}: {} ({err})",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = merman_render::LayoutOptions {
            text_measurer: std::sync::Arc::new(
                merman_render::text::VendoredFontMetricsTextMeasurer::default(),
            ),
            ..Default::default()
        };
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::StateDiagramV2(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_state_diagram_v2_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("upstream dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("local dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

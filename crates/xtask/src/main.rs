use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use regex::Regex;

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
    println!("  update-snapshots");
    println!("  update-layout-snapshots   (alias: gen-layout-goldens)");
    println!("  gen-upstream-svgs");
    println!("  check-upstream-svgs");
    println!("  compare-all-svgs");
    println!("  compare-svg-xml");
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
        "measure-text" => measure_text(args.collect()),
        "gen-upstream-svgs" => gen_upstream_svgs(args.collect()),
        "check-upstream-svgs" => check_upstream_svgs(args.collect()),
        "compare-er-svgs" => compare_er_svgs(args.collect()),
        "compare-flowchart-svgs" => compare_flowchart_svgs(args.collect()),
        "debug-flowchart-layout" => debug_flowchart_layout(args.collect()),
        "debug-flowchart-svg-roots" => debug_flowchart_svg_roots(args.collect()),
        "debug-flowchart-svg-positions" => debug_flowchart_svg_positions(args.collect()),
        "debug-flowchart-svg-diff" => debug_flowchart_svg_diff(args.collect()),
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
        other => Err(XtaskError::UnknownCommand(other.to_string())),
    }
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

    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "handDrawnSeed": 1 }),
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
            if !p.extension().is_some_and(|e| e == "svg") {
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

            let parsed = match futures::executor::block_on(
                engine.parse_diagram(&text, merman::ParseOptions::default()),
            ) {
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

            let svg_opts = merman_render::svg::SvgRenderOptions {
                diagram_id: Some(stem.to_string()),
                ..Default::default()
            };
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
    let _ = writeln!(&mut report, "");
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
    let _ = writeln!(&mut report, "");
    let _ = writeln!(&mut report, "## Mismatches ({})", mismatches.len());
    let _ = writeln!(&mut report, "");
    for (diagram, stem, upstream_out, local_out) in &mismatches {
        let _ = writeln!(
            &mut report,
            "- `{diagram}/{stem}`: `{}` vs `{}`",
            upstream_out.display(),
            local_out.display()
        );
    }
    if !missing.is_empty() {
        let _ = writeln!(&mut report, "");
        let _ = writeln!(&mut report, "## Missing / Failed ({})", missing.len());
        let _ = writeln!(&mut report, "");
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

        if let Err(err) = res {
            failures.push(format!("{diagram}: {err}"));
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

fn gen_font_metrics(args: Vec<String>) -> Result<(), XtaskError> {
    let mut in_dir: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut base_font_size_px: f64 = 16.0;
    let mut debug_text: Option<String> = None;
    let mut debug_dump: usize = 0;
    let mut backend: String = "browser".to_string();
    let mut browser_exe: Option<PathBuf> = None;

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

    fn class_has_token(node: roxmltree::Node<'_, '_>, token: &str) -> bool {
        node.attribute("class")
            .unwrap_or_default()
            .split_whitespace()
            .any(|t| t == token)
    }

    fn parse_translate_x(transform: &str) -> Option<f64> {
        let t = transform.trim();
        let start = t.find("translate(")? + "translate(".len();
        let rest = &t[start..];
        let end = rest
            .find(|c: char| c == ',' || c == ' ' || c == ')')
            .unwrap_or(rest.len());
        rest[..end].trim().parse::<f64>().ok()
    }

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

    fn parse_viewbox_w(root_svg: roxmltree::Node<'_, '_>) -> Option<f64> {
        let vb = root_svg.attribute("viewBox")?;
        let nums = vb
            .split_whitespace()
            .filter_map(|s| s.parse::<f64>().ok())
            .collect::<Vec<_>>();
        if nums.len() == 4 { Some(nums[2]) } else { None }
    }

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
        let Some(style_node) = doc.descendants().find(|n| n.has_tag_name("style")) else {
            return None;
        };
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
        let Some(style_node) = doc.descendants().find(|n| n.has_tag_name("style")) else {
            return None;
        };
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

    fn estimate_flowchart_content_width_px(doc: &roxmltree::Document<'_>) -> Option<f64> {
        let Some(root_g) = doc
            .descendants()
            .find(|n| n.has_tag_name("g") && n.is_element() && class_has_token(*n, "root"))
        else {
            return None;
        };

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
        if !path.is_file() || !path.extension().is_some_and(|e| e == "svg") {
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
                    text: title_text,
                    width_px: 0.0,
                    font_size_px: title_font_size_px,
                });
            }
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

    fn median(v: &mut Vec<f64>) -> Option<f64> {
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
            unknown_chars.sort_by(|a, b| (*a as u32).cmp(&(*b as u32)));

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
    headless: 'new',
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
     headless: 'new',
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
    headless: 'new',
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
    headless: 'new',
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
    let mut svg_bbox_overhangs_by_font: BTreeMap<
        String,
        (f64, f64, Vec<(char, f64)>, Vec<(char, f64)>),
    > = BTreeMap::new();
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

    let mut tables: Vec<(
        FontTable,
        f64,
        (f64, f64, Vec<(char, f64)>, Vec<(char, f64)>),
    )> = Vec::new();
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

    let upstream_summary =
        parse_summary(&upstream_svg).map_err(|e| XtaskError::DebugSvgFailed(e))?;
    let local_summary = parse_summary(&local_svg).map_err(|e| XtaskError::DebugSvgFailed(e))?;

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

    fn parse_positions(
        svg: &str,
    ) -> Result<(BTreeMap<String, NodePos>, BTreeMap<String, ClusterRect>), String> {
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

            if class_tokens.iter().any(|t| *t == "node") {
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
            if class_tokens.iter().any(|t| *t == "edgeLabel")
                && class_tokens.iter().any(|t| *t == "label")
            {
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

            if class_tokens.iter().any(|t| *t == "cluster") {
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
        parse_positions(&upstream_svg).map_err(|e| XtaskError::DebugSvgFailed(e))?;
    let (lo_nodes, lo_clusters) =
        parse_positions(&local_svg).map_err(|e| XtaskError::DebugSvgFailed(e))?;

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
        let Some(arr) = v.as_array() else {
            return None;
        };
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

    fn parse_positions_and_edges(
        svg: &str,
    ) -> Result<
        (
            BTreeMap<String, NodePos>,
            BTreeMap<String, ClusterRect>,
            BTreeMap<String, EdgePoints>,
            Vec<String>,
        ),
        String,
    > {
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

                if class_tokens.iter().any(|t| *t == "node") {
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
                if class_tokens.iter().any(|t| *t == "edgeLabel")
                    && class_tokens.iter().any(|t| *t == "label")
                {
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

                if class_tokens.iter().any(|t| *t == "cluster") {
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
                if !n.attribute("data-edge").is_some_and(|v| v == "true") {
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

        match (up.bbox, lo.bbox, up.abs_bbox, lo.abs_bbox) {
            (Some(ub), Some(lb), Some(uab), Some(lab)) => {
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
            _ => {}
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

    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "handDrawnSeed": 1 }),
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
            ] {
                if let Some(lbl) = lbl {
                    let hw = lbl.width / 2.0;
                    let hh = lbl.height / 2.0;
                    include_rect(lbl.x - hw, lbl.y - hh, lbl.x + hw, lbl.y + hh);
                }
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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
        if !path.extension().is_some_and(|e| e == "svg") {
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
    let fixtures_root = workspace_root.join("fixtures");

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

    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "handDrawnSeed": 1 }),
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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
        let mut sig = SvgSig::default();
        sig.view_box = re_viewbox
            .captures(svg)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string());
        sig.max_width_px = re_max_width
            .captures(svg)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string());
        for cap in re_marker_id.captures_iter(svg) {
            if let Some(m) = cap.get(1) {
                sig.marker_ids.insert(m.as_str().to_string());
            }
        }
        for cap in re_marker_ref.captures_iter(svg) {
            if let Some(m) = cap.get(1) {
                sig.marker_refs.insert(m.as_str().to_string());
            }
        }
        sig
    }

    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "handDrawnSeed": 1 }),
    ));
    let layout_opts = merman_render::LayoutOptions::default();

    let mut report = String::new();
    let _ = writeln!(&mut report, "# ER SVG Compare Report");
    let _ = writeln!(&mut report, "");
    let _ = writeln!(
        &mut report,
        "- Upstream: `fixtures/upstream-svgs/er/*.svg` (Mermaid CLI pinned to Mermaid 11.12.2)"
    );
    let _ = writeln!(&mut report, "- Local: `render_er_diagram_svg` (Stage B)");
    let _ = writeln!(&mut report, "");
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
        let _ = writeln!(&mut report, "");
        let _ = writeln!(&mut report, "## DOM Mismatch Details");
        let _ = writeln!(&mut report, "");
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
        let status = Command::new("npm")
            .arg(npm_cmd)
            .current_dir(&tools_root)
            .status()
            .map_err(|err| {
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
            if !path.extension().is_some_and(|e| e == "mmd") {
                continue;
            }
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
            {
                continue;
            }
            if diagram == "gantt" {
                if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    matches!(
                        n,
                        "click_loose.mmd"
                            | "click_strict.mmd"
                            | "dateformat_hash_comment_truncates.mmd"
                            | "excludes_hash_comment_truncates.mmd"
                            | "today_marker_and_axis.mmd"
                    )
                }) {
                    continue;
                }
            }
            if diagram == "state" {
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("_parser_") || n.contains("_parser_spec"))
                {
                    continue;
                }
            }
            if diagram == "class" {
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("upstream_text_label_variants_spec"))
                {
                    continue;
                }
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

            let mut cmd = Command::new(mmdc);
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

            let status = cmd.status();

            match status {
                Ok(s) if s.success() => {}
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

    let mut gen_args: Vec<String> = Vec::new();
    gen_args.push("--diagram".to_string());
    gen_args.push(diagram.clone());
    gen_args.push("--out".to_string());
    gen_args.push(out_root.to_string_lossy().to_string());
    if let Some(f) = &filter {
        gen_args.push("--filter".to_string());
        gen_args.push(f.clone());
    }
    if install {
        gen_args.push("--install".to_string());
    }

    gen_upstream_svgs(gen_args)?;

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
            if !path.extension().is_some_and(|e| e == "mmd") {
                continue;
            }
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
            {
                continue;
            }
            if diagram == "gantt" {
                if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    matches!(
                        n,
                        "click_loose.mmd"
                            | "click_strict.mmd"
                            | "dateformat_hash_comment_truncates.mmd"
                            | "excludes_hash_comment_truncates.mmd"
                            | "today_marker_and_axis.mmd"
                    )
                }) {
                    continue;
                }
            }
            if diagram == "state" {
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("_parser_") || n.contains("_parser_spec"))
                {
                    continue;
                }
            }
            if diagram == "class" {
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("upstream_text_label_variants_spec"))
                {
                    continue;
                }
            }
            if diagram == "c4" {
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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
            if !path.extension().is_some_and(|e| e == "mmd") {
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
    let fixtures_root = workspace_root.join("fixtures");

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

    let engine = merman::Engine::new();
    let mut failures = Vec::new();

    fn ms_to_local_iso(ms: i64) -> Option<String> {
        let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)?;
        Some(
            dt.with_timezone(&chrono::Local)
                .format("%Y-%m-%dT%H:%M:%S%.3f")
                .to_string(),
        )
    }

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

        if diagram != "all" {
            let dt = parsed.meta.diagram_type.as_str();
            let matches = dt == diagram
                || (diagram == "er" && matches!(dt, "er" | "erDiagram"))
                || (diagram == "flowchart" && dt == "flowchart-v2")
                || (diagram == "state" && dt == "stateDiagram")
                || (diagram == "class" && matches!(dt, "class" | "classDiagram"))
                || (diagram == "gitgraph" && dt == "gitGraph");
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
            let re = Regex::new(r"\b(\d+)-[0-9a-f]{7}\b").map_err(|e| {
                XtaskError::SnapshotUpdateFailed(format!("invalid gitGraph id regex: {e}"))
            })?;

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

            walk(&re, &mut model);
        }

        if parsed.meta.diagram_type == "block" {
            let re = Regex::new(r"id-[a-z0-9]+-(\d+)").map_err(|e| {
                XtaskError::SnapshotUpdateFailed(format!("invalid block id regex: {e}"))
            })?;

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

            walk(&re, &mut model);
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
        .and_then(|m| m.get(&YamlValue::String("default".to_string())))
    {
        return yaml_to_json(default).ok();
    }

    if let Some(any_of) = schema
        .as_mapping()
        .and_then(|m| m.get(&YamlValue::String("anyOf".to_string())))
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
        .and_then(|m| m.get(&YamlValue::String("oneOf".to_string())))
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
        .and_then(|m| m.get(&YamlValue::String("type".to_string())))
        .and_then(|v| v.as_str())
        == Some("object");

    let props = schema
        .as_mapping()
        .and_then(|m| m.get(&YamlValue::String("properties".to_string())))
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
        .and_then(|m| m.get(&YamlValue::String("allOf".to_string())))
        .and_then(|v| v.as_sequence())
        .cloned();

    if let Some(all_of) = all_of {
        let mut merged = schema.clone();
        if let Some(m) = merged.as_mapping_mut() {
            m.remove(&YamlValue::String("allOf".to_string()));
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
        .get(&YamlValue::String("$ref".to_string()))
        .and_then(|v| v.as_str())
    else {
        return Ok(schema.clone());
    };
    let target = resolve_ref_target(ref_str, root)?;
    let mut base = expand_schema(target, root);

    // Overlay other keys on top of the resolved target.
    let mut overlay = YamlValue::Mapping(map.clone());
    if let Some(m) = overlay.as_mapping_mut() {
        m.remove(&YamlValue::String("$ref".to_string()));
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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
    let mut text_measurer: String = "deterministic".to_string();

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
        if !path.extension().is_some_and(|e| e == "mmd") {
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let mut layout_opts = merman_render::LayoutOptions::default();
        layout_opts.text_measurer =
            std::sync::Arc::new(merman_render::text::VendoredFontMetricsTextMeasurer::default());
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let layout_opts = merman_render::LayoutOptions::default();
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let layout_opts = merman_render::LayoutOptions::default();
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let layout_opts = merman_render::LayoutOptions::default();
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let layout_opts = merman_render::LayoutOptions::default();
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let layout_opts = merman_render::LayoutOptions::default();
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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
    let layout_opts = merman_render::LayoutOptions::default();

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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let layout_opts = merman_render::LayoutOptions::default();
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let layout_opts = merman_render::LayoutOptions::default();
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let layout_opts = merman_render::LayoutOptions::default();
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let layout_opts = merman_render::LayoutOptions::default();
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let merman_render::model::LayoutDiagram::RadarDiagram(layout) = &layouted.layout else {
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

        let local_svg = match merman_render::svg::render_radar_diagram_svg(
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let merman_render::model::LayoutDiagram::TreemapDiagram(layout) = &layouted.layout else {
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

        let local_svg = match merman_render::svg::render_treemap_diagram_svg(
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let layout_opts = merman_render::LayoutOptions::default();
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let layout_opts = merman_render::LayoutOptions::default();
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let layout_opts = merman_render::LayoutOptions::default();
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let layout_opts = merman_render::LayoutOptions::default();
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let layout_opts = merman_render::LayoutOptions::default();
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let layout_opts = merman_render::LayoutOptions::default();
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
        if !path.extension().is_some_and(|e| e == "mmd") {
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

        let layout_opts = merman_render::LayoutOptions::default();
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

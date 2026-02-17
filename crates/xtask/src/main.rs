mod cmd;
mod state_svgdump;
mod svgdom;
mod util;

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
    let Some(cmd_name) = args.next() else {
        return Err(XtaskError::Usage);
    };

    if matches!(cmd_name.as_str(), "--help" | "-h") {
        print_help(None);
        return Ok(());
    }
    if cmd_name == "help" {
        print_help(args.next().as_deref());
        return Ok(());
    }

    match cmd_name.as_str() {
        "gen-default-config" => cmd::gen_default_config(args.collect()),
        "gen-dompurify-defaults" => cmd::gen_dompurify_defaults(args.collect()),
        "verify-generated" => cmd::verify_generated(args.collect()),
        "import-upstream-docs" => cmd::import_upstream_docs(args.collect()),
        "import-upstream-html" => cmd::import_upstream_html(args.collect()),
        "import-upstream-cypress" => cmd::import_upstream_cypress(args.collect()),
        "import-mmdr-fixtures" => cmd::import_mmdr_fixtures(args.collect()),
        "update-snapshots" => cmd::update_snapshots(args.collect()),
        "update-layout-snapshots" | "gen-layout-goldens" => {
            cmd::update_layout_snapshots(args.collect())
        }
        "check-alignment" => cmd::check_alignment(args.collect()),
        "gen-debug-svgs" => cmd::gen_debug_svgs(args.collect()),
        "gen-er-svgs" => cmd::gen_er_svgs(args.collect()),
        "gen-flowchart-svgs" => cmd::gen_flowchart_svgs(args.collect()),
        "gen-state-svgs" => cmd::gen_state_svgs(args.collect()),
        "gen-class-svgs" => cmd::gen_class_svgs(args.collect()),
        "gen-c4-svgs" => cmd::gen_c4_svgs(args.collect()),
        "gen-c4-textlength" => cmd::gen_c4_textlength(args.collect()),
        "gen-font-metrics" => cmd::gen_font_metrics(args.collect()),
        "gen-svg-overrides" => cmd::gen_svg_overrides(args.collect()),
        "gen-er-text-overrides" => cmd::gen_er_text_overrides(args.collect()),
        "gen-mindmap-text-overrides" => cmd::gen_mindmap_text_overrides(args.collect()),
        "gen-gantt-text-overrides" => cmd::gen_gantt_text_overrides(args.collect()),
        "measure-text" => cmd::measure_text(args.collect()),
        "gen-upstream-svgs" => cmd::gen_upstream_svgs(args.collect()),
        "check-upstream-svgs" => cmd::check_upstream_svgs(args.collect()),
        "compare-er-svgs" => cmd::compare_er_svgs(args.collect()),
        "compare-flowchart-svgs" => cmd::compare_flowchart_svgs(args.collect()),
        "debug-flowchart-layout" => cmd::debug_flowchart_layout(args.collect()),
        "debug-flowchart-svg-roots" => cmd::debug_flowchart_svg_roots(args.collect()),
        "debug-flowchart-svg-positions" => cmd::debug_flowchart_svg_positions(args.collect()),
        "debug-flowchart-svg-diff" => cmd::debug_flowchart_svg_diff(args.collect()),
        "debug-flowchart-data-points" => cmd::debug_flowchart_data_points(args.collect()),
        "debug-flowchart-edge-trace" => cmd::debug_flowchart_edge_trace(args.collect()),
        "debug-mindmap-svg-positions" => cmd::debug_mindmap_svg_positions(args.collect()),
        "debug-svg-bbox" => cmd::debug_svg_bbox(args.collect()),
        "debug-svg-data-points" => cmd::debug_svg_data_points(args.collect()),
        "debug-architecture-delta" => cmd::debug_architecture_delta(args.collect()),
        "summarize-architecture-deltas" => cmd::summarize_architecture_deltas(args.collect()),
        "compare-dagre-layout" => cmd::compare_dagre_layout(args.collect()),
        "analyze-state-fixture" => state_svgdump::analyze_state_fixture(args.collect()),
        "compare-sequence-svgs" => cmd::compare_sequence_svgs(args.collect()),
        "compare-class-svgs" => cmd::compare_class_svgs(args.collect()),
        "compare-state-svgs" => cmd::compare_state_svgs(args.collect()),
        "compare-info-svgs" => cmd::compare_info_svgs(args.collect()),
        "compare-pie-svgs" => cmd::compare_pie_svgs(args.collect()),
        "compare-sankey-svgs" => cmd::compare_sankey_svgs(args.collect()),
        "compare-packet-svgs" => cmd::compare_packet_svgs(args.collect()),
        "compare-timeline-svgs" => cmd::compare_timeline_svgs(args.collect()),
        "compare-journey-svgs" => cmd::compare_journey_svgs(args.collect()),
        "compare-kanban-svgs" => cmd::compare_kanban_svgs(args.collect()),
        "compare-gitgraph-svgs" => cmd::compare_gitgraph_svgs(args.collect()),
        "compare-gantt-svgs" => cmd::compare_gantt_svgs(args.collect()),
        "compare-c4-svgs" => cmd::compare_c4_svgs(args.collect()),
        "compare-block-svgs" => cmd::compare_block_svgs(args.collect()),
        "compare-radar-svgs" => cmd::compare_radar_svgs(args.collect()),
        "compare-requirement-svgs" => cmd::compare_requirement_svgs(args.collect()),
        "compare-mindmap-svgs" => cmd::compare_mindmap_svgs(args.collect()),
        "compare-architecture-svgs" => cmd::compare_architecture_svgs(args.collect()),
        "compare-quadrantchart-svgs" => cmd::compare_quadrantchart_svgs(args.collect()),
        "compare-treemap-svgs" => cmd::compare_treemap_svgs(args.collect()),
        "compare-xychart-svgs" => cmd::compare_xychart_svgs(args.collect()),
        "compare-all-svgs" => cmd::compare_all_svgs(args.collect()),
        "compare-svg-xml" => cmd::compare_svg_xml(args.collect()),
        "canon-svg-xml" => cmd::canon_svg_xml(args.collect()),
        "report-overrides" => cmd::report_overrides(args.collect()),
        other => Err(XtaskError::UnknownCommand(other.to_string())),
    }
}

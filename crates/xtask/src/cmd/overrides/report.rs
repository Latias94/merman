//! Inventory and reporting for parity overrides.

use crate::XtaskError;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub(crate) fn report_overrides(args: Vec<String>) -> Result<(), XtaskError> {
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
    let block = generated_dir.join("block_root_overrides_11_12_2.rs");
    let flowchart = generated_dir.join("flowchart_root_overrides_11_12_2.rs");
    let class = generated_dir.join("class_root_overrides_11_12_2.rs");
    let mindmap = generated_dir.join("mindmap_root_overrides_11_12_2.rs");
    let gitgraph = generated_dir.join("gitgraph_root_overrides_11_12_2.rs");
    let journey = generated_dir.join("journey_root_overrides_11_12_2.rs");
    let er = generated_dir.join("er_root_overrides_11_12_2.rs");
    let kanban = generated_dir.join("kanban_root_overrides_11_12_2.rs");
    let pie = generated_dir.join("pie_root_overrides_11_12_2.rs");
    let requirement = generated_dir.join("requirement_root_overrides_11_12_2.rs");
    let sankey = generated_dir.join("sankey_root_overrides_11_12_2.rs");
    let sequence = generated_dir.join("sequence_root_overrides_11_12_2.rs");
    let state_root = generated_dir.join("state_root_overrides_11_12_2.rs");
    let state_text = generated_dir.join("state_text_overrides_11_12_2.rs");
    let timeline = generated_dir.join("timeline_root_overrides_11_12_2.rs");

    let architecture_txt = read_text(&architecture)?;
    let block_txt = read_text(&block)?;
    let flowchart_txt = read_text(&flowchart)?;
    let class_txt = read_text(&class)?;
    let mindmap_txt = read_text(&mindmap)?;
    let gitgraph_txt = read_text(&gitgraph)?;
    let journey_txt = read_text(&journey)?;
    let er_txt = read_text(&er)?;
    let kanban_txt = read_text(&kanban)?;
    let pie_txt = read_text(&pie)?;
    let requirement_txt = read_text(&requirement)?;
    let sankey_txt = read_text(&sankey)?;
    let sequence_txt = read_text(&sequence)?;
    let state_root_txt = read_text(&state_root)?;
    let state_text_txt = read_text(&state_text)?;
    let timeline_txt = read_text(&timeline)?;

    let architecture_n = count_matches(root_viewport_entry_re, &architecture_txt);
    let block_n = count_matches(root_viewport_entry_re, &block_txt);
    let flowchart_n = count_matches(root_viewport_entry_re, &flowchart_txt);
    let class_n = count_matches(root_viewport_entry_re, &class_txt);
    let mindmap_n = count_matches(root_viewport_entry_re, &mindmap_txt);
    let gitgraph_n = count_matches(root_viewport_entry_re, &gitgraph_txt);
    let journey_n = count_matches(root_viewport_entry_re, &journey_txt);
    let er_n = count_matches(root_viewport_entry_re, &er_txt);
    let kanban_n = count_matches(root_viewport_entry_re, &kanban_txt);
    let pie_n = count_matches(root_viewport_entry_re, &pie_txt);
    let requirement_n = count_matches(root_viewport_entry_re, &requirement_txt);
    let sankey_n = count_matches(root_viewport_entry_re, &sankey_txt);
    let sequence_n = count_matches(root_viewport_entry_re, &sequence_txt);
    let state_root_n = count_matches(root_viewport_entry_re, &state_root_txt);
    let state_text_n = count_matches(state_text_entry_re, &state_text_txt);
    let timeline_n = count_matches(root_viewport_entry_re, &timeline_txt);

    println!("Mermaid baseline: @11.12.2");
    println!();
    println!("Root viewport overrides:");
    println!("- architecture_root_overrides_11_12_2.rs: {architecture_n} entries");
    println!("- block_root_overrides_11_12_2.rs: {block_n} entries");
    println!("- flowchart_root_overrides_11_12_2.rs: {flowchart_n} entries");
    println!("- class_root_overrides_11_12_2.rs: {class_n} entries");
    println!("- mindmap_root_overrides_11_12_2.rs: {mindmap_n} entries");
    println!("- gitgraph_root_overrides_11_12_2.rs: {gitgraph_n} entries");
    println!("- journey_root_overrides_11_12_2.rs: {journey_n} entries");
    println!("- er_root_overrides_11_12_2.rs: {er_n} entries");
    println!("- kanban_root_overrides_11_12_2.rs: {kanban_n} entries");
    println!("- pie_root_overrides_11_12_2.rs: {pie_n} entries");
    println!("- requirement_root_overrides_11_12_2.rs: {requirement_n} entries");
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

//! Inventory and reporting for parity overrides.

use crate::XtaskError;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum OverrideCategory {
    RootViewport,
    TextLookup,
    SvgTextMetrics,
    FontMetrics,
    TypeTextLength,
    HandCuratedHelpers,
    RawPathBridge,
}

impl OverrideCategory {
    const ALL: [OverrideCategory; 7] = [
        OverrideCategory::RootViewport,
        OverrideCategory::TextLookup,
        OverrideCategory::SvgTextMetrics,
        OverrideCategory::FontMetrics,
        OverrideCategory::TypeTextLength,
        OverrideCategory::HandCuratedHelpers,
        OverrideCategory::RawPathBridge,
    ];

    fn heading(self) -> &'static str {
        match self {
            OverrideCategory::RootViewport => "Root viewport overrides",
            OverrideCategory::TextLookup => "Text metric lookup overrides",
            OverrideCategory::SvgTextMetrics => "SVG text metric tables",
            OverrideCategory::FontMetrics => "Font metric tables",
            OverrideCategory::TypeTextLength => "Typed textLength lookups",
            OverrideCategory::HandCuratedHelpers => "Hand-curated helper overrides",
            OverrideCategory::RawPathBridge => "Manual raw SVG/path bridges",
        }
    }

    fn total_unit(self) -> &'static str {
        match self {
            OverrideCategory::RootViewport => "entries",
            OverrideCategory::TextLookup => "lookup arms",
            OverrideCategory::SvgTextMetrics => "table rows",
            OverrideCategory::FontMetrics => "table rows",
            OverrideCategory::TypeTextLength => "lookup arms",
            OverrideCategory::HandCuratedHelpers => "helper functions",
            OverrideCategory::RawPathBridge => "bridge functions",
        }
    }
}

#[derive(Debug, Clone)]
struct OverrideFootprintEntry {
    file_name: String,
    category: OverrideCategory,
    count: usize,
    unit: &'static str,
}

pub(crate) fn report_overrides(args: Vec<String>) -> Result<(), XtaskError> {
    if args.iter().any(|a| matches!(a.as_str(), "--help" | "-h")) {
        println!("usage: xtask report-overrides");
        println!();
        println!("Prints a parity override footprint inventory.");
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
    let parity_dir = workspace_root
        .join("crates")
        .join("merman-render")
        .join("src")
        .join("svg")
        .join("parity");
    let source_root = workspace_root
        .join("crates")
        .join("merman-render")
        .join("src");

    let generated_entries = collect_generated_override_footprint_entries(&generated_dir)?;
    let manual_entries = collect_manual_bridge_footprint_entries(&parity_dir, &source_root)?;

    println!("Mermaid baseline: @11.12.3");
    println!();
    println!(
        "Generated override modules scanned: {}",
        generated_entries.len()
    );
    println!(
        "Manual raw SVG/path bridge files scanned: {}",
        manual_entries.len()
    );
    println!();

    let mut entries = generated_entries;
    entries.extend(manual_entries);

    for category in OverrideCategory::ALL {
        print_category(&entries, category);
    }

    println!("Notes:");
    println!("- Counts are inventory units and are not directly comparable across categories.");
    println!(
        "- Generated module counts cover `crates/merman-render/src/generated`, while manual bridge counts cover hand-authored path-bridge helpers under `crates/merman-render/src/svg/parity`."
    );
    println!("- Root viewport entries count match arms returning `Some((viewBox, max_width))`.");
    println!("- Text lookup arms count generated or hand-curated `=> Some(...)` parity branches.");
    println!("- Table rows count tuple rows in generated font/SVG metric arrays.");

    Ok(())
}

fn collect_generated_override_footprint_entries(
    generated_dir: &Path,
) -> Result<Vec<OverrideFootprintEntry>, XtaskError> {
    let mut files = collect_generated_rs_files(generated_dir)?;
    files.sort();

    let mut entries = Vec::new();
    for path in files {
        let Some(file_name) = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        if file_name == "mod.rs" {
            continue;
        }

        let text = read_text(&path)?;
        if let Some(entry) = classify_generated_override_file(file_name, &text) {
            entries.push(entry);
        }
    }

    Ok(entries)
}

fn collect_manual_bridge_footprint_entries(
    parity_dir: &Path,
    source_root: &Path,
) -> Result<Vec<OverrideFootprintEntry>, XtaskError> {
    let mut files = collect_parity_rs_files(parity_dir)?;
    files.sort();

    let mut entries = Vec::new();
    for path in files {
        let Some(file_name) = path.strip_prefix(source_root).ok().map(report_path_name) else {
            continue;
        };
        let text = read_text(&path)?;
        let count = count_manual_bridge_functions(text.as_str());
        if count == 0 {
            continue;
        }
        entries.push(OverrideFootprintEntry {
            file_name,
            category: OverrideCategory::RawPathBridge,
            count,
            unit: "bridge functions",
        });
    }

    Ok(entries)
}

fn collect_generated_rs_files(generated_dir: &Path) -> Result<Vec<PathBuf>, XtaskError> {
    let read_dir = fs::read_dir(generated_dir).map_err(|source| XtaskError::ReadFile {
        path: generated_dir.display().to_string(),
        source,
    })?;

    let mut files = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|source| XtaskError::ReadFile {
            path: generated_dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path);
        }
    }

    Ok(files)
}

fn collect_parity_rs_files(parity_dir: &Path) -> Result<Vec<PathBuf>, XtaskError> {
    let mut stack = vec![parity_dir.to_path_buf()];
    let mut files = Vec::new();

    while let Some(dir) = stack.pop() {
        let read_dir = fs::read_dir(&dir).map_err(|source| XtaskError::ReadFile {
            path: dir.display().to_string(),
            source,
        })?;
        for entry in read_dir {
            let entry = entry.map_err(|source| XtaskError::ReadFile {
                path: dir.display().to_string(),
                source,
            })?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }

    Ok(files)
}

fn classify_generated_override_file(
    file_name: String,
    text: &str,
) -> Option<OverrideFootprintEntry> {
    if file_name.contains("_root_overrides_") {
        return Some(OverrideFootprintEntry {
            file_name,
            category: OverrideCategory::RootViewport,
            count: count_root_viewport_entries(text),
            unit: "entries",
        });
    }

    if file_name.starts_with("font_metrics_") {
        return Some(OverrideFootprintEntry {
            file_name,
            category: OverrideCategory::FontMetrics,
            count: count_tuple_rows(text),
            unit: "table rows",
        });
    }

    if file_name.starts_with("svg_overrides_") {
        return Some(OverrideFootprintEntry {
            file_name,
            category: OverrideCategory::SvgTextMetrics,
            count: count_tuple_rows(text),
            unit: "table rows",
        });
    }

    if file_name.contains("_type_textlength_") {
        return Some(OverrideFootprintEntry {
            file_name,
            category: OverrideCategory::TypeTextLength,
            count: count_some_match_arms(text),
            unit: "lookup arms",
        });
    }

    if file_name.contains("_text_overrides_") {
        let lookup_arms = count_some_match_arms(text);
        if lookup_arms > 0 {
            return Some(OverrideFootprintEntry {
                file_name,
                category: OverrideCategory::TextLookup,
                count: lookup_arms,
                unit: "lookup arms",
            });
        }

        return Some(OverrideFootprintEntry {
            file_name,
            category: OverrideCategory::HandCuratedHelpers,
            count: count_public_functions(text),
            unit: "helper functions",
        });
    }

    None
}

fn print_category(entries: &[OverrideFootprintEntry], category: OverrideCategory) {
    let category_entries: Vec<_> = entries
        .iter()
        .filter(|entry| entry.category == category)
        .collect();
    if category_entries.is_empty() {
        return;
    }

    let total: usize = category_entries.iter().map(|entry| entry.count).sum();
    println!("{}:", category.heading());
    println!("- total: {total} {}", category.total_unit());
    for entry in category_entries {
        println!("- {}: {} {}", entry.file_name, entry.count, entry.unit);
    }
    println!();
}

fn read_text(path: &Path) -> Result<String, XtaskError> {
    fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })
}

fn report_path_name(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn count_root_viewport_entries(text: &str) -> usize {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| Regex::new(r#""[^"]+"\s*=>\s*(?:\{\s*)?Some\("#).expect("valid regex"));
    count_matches(re, text)
}

fn count_some_match_arms(text: &str) -> usize {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"=>\s*Some\("#).expect("valid regex"));
    count_matches(re, text)
}

fn count_tuple_rows(text: &str) -> usize {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"(?m)^\s*\("#).expect("valid regex"));
    count_matches(re, text)
}

fn count_public_functions(text: &str) -> usize {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| Regex::new(r#"(?m)^pub fn\s+[A-Za-z0-9_]+\s*\("#).expect("valid regex"));
    count_matches(re, text)
}

fn count_manual_bridge_functions(text: &str) -> usize {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"(?m)^(?:pub(?:\([^)]+\))?\s+)?fn\s+maybe_override_[A-Za-z0-9_]+\s*\("#)
            .expect("valid regex")
    });
    count_matches(re, text)
}

fn count_matches(re: &Regex, text: &str) -> usize {
    re.find_iter(text).count()
}

#[cfg(test)]
mod tests {
    use super::{count_manual_bridge_functions, count_public_functions, report_path_name};
    use std::path::Path;

    #[test]
    fn counts_manual_bridge_functions_in_flowchart_path_override() {
        let text = r#"
//! Flowchart edge path overrides.
pub(in crate::svg::parity::flowchart) fn maybe_override_degenerate_subgraph_edge_path_d(
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::flowchart::FlowEdge,
    data_points: &[crate::model::LayoutPoint],
) -> Option<String> {
    None
}
"#;

        assert_eq!(count_manual_bridge_functions(text), 1);
    }

    #[test]
    fn ignores_non_bridge_functions() {
        let text = r#"
pub fn not_a_bridge() {}
fn definitely_not_a_bridge() {}
"#;

        assert_eq!(count_manual_bridge_functions(text), 0);
    }

    #[test]
    fn counts_public_helper_functions() {
        let text = r#"
pub fn helper_one() {}
pub fn helper_two(
) {}
fn private_helper() {}
"#;

        assert_eq!(count_public_functions(text), 2);
    }

    #[test]
    fn report_paths_are_stable_across_platforms() {
        assert_eq!(
            report_path_name(Path::new(
                r"svg\parity\flowchart\edge_geom\degenerate_path.rs"
            )),
            "svg/parity/flowchart/edge_geom/degenerate_path.rs"
        );
    }
}

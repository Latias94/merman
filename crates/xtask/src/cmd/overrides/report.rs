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
    HandCuratedHelpers,
    RawPathBridge,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct OverrideCategoryMetadata {
    owner: &'static str,
    source: &'static str,
    allowed_use: &'static str,
    expected_removal: &'static str,
}

impl OverrideCategory {
    const ALL: [OverrideCategory; 6] = [
        OverrideCategory::RootViewport,
        OverrideCategory::TextLookup,
        OverrideCategory::SvgTextMetrics,
        OverrideCategory::FontMetrics,
        OverrideCategory::HandCuratedHelpers,
        OverrideCategory::RawPathBridge,
    ];

    fn heading(self) -> &'static str {
        match self {
            OverrideCategory::RootViewport => "Root viewport overrides",
            OverrideCategory::TextLookup => "Text metric lookup overrides",
            OverrideCategory::SvgTextMetrics => "SVG text metric tables",
            OverrideCategory::FontMetrics => "Font metric tables",
            OverrideCategory::HandCuratedHelpers => "Hand-curated helper overrides",
            OverrideCategory::RawPathBridge => "Manual raw SVG/path bridges",
        }
    }

    fn total_unit(self) -> &'static str {
        match self {
            OverrideCategory::RootViewport => "entries",
            OverrideCategory::TextLookup => "lookup entries",
            OverrideCategory::SvgTextMetrics => "table rows",
            OverrideCategory::FontMetrics => "table rows",
            OverrideCategory::HandCuratedHelpers => "helper functions",
            OverrideCategory::RawPathBridge => "bridge functions",
        }
    }

    fn no_growth_budget(self) -> usize {
        match self {
            OverrideCategory::RootViewport => 750,
            OverrideCategory::TextLookup => 477,
            OverrideCategory::SvgTextMetrics => 184,
            OverrideCategory::FontMetrics => 3774,
            OverrideCategory::HandCuratedHelpers => 0,
            OverrideCategory::RawPathBridge => 0,
        }
    }

    fn metadata(self) -> OverrideCategoryMetadata {
        match self {
            OverrideCategory::RootViewport => OverrideCategoryMetadata {
                owner: "render parity workstream",
                source: "fixture-derived upstream SVG root viewBox/max-width baselines for Mermaid @11.12.3",
                allowed_use: "narrow export-bound pins when browser insertion or emitted bounds differ from deterministic Rust layout",
                expected_removal: "delete entries once typed layout/emitted bounds can derive the same root viewport or a baseline upgrade removes the pinned behavior",
            },
            OverrideCategory::TextLookup => OverrideCategoryMetadata {
                owner: "render parity workstream",
                source: "fixture or browser-probe HTML/SVG text measurements for exact diagram text contexts",
                allowed_use: "exact diagram/text/font-size lookups for browser/font measurement facts that shared metrics cannot derive yet",
                expected_removal: "delete entries once vendored/shared text measurement returns the upstream dimensions without fixture-specific lookup arms",
            },
            OverrideCategory::SvgTextMetrics => OverrideCategoryMetadata {
                owner: "render parity workstream",
                source: "browser getBBox/getComputedTextLength measurements extracted from upstream SVG text nodes",
                allowed_use: "font-keyed SVG text overhang and scale correction for Mermaid baseline parity",
                expected_removal: "replace with shared font metrics or browser-probe imports, then delete stale rows",
            },
            OverrideCategory::FontMetrics => OverrideCategoryMetadata {
                owner: "shared text measurement owner",
                source: "browser-measured glyph, kerning, trigram, HTML, and SVG correction tables",
                allowed_use: "deterministic text measurement support when runtime browser measurement is unavailable",
                expected_removal: "regenerate or trim when better vendored font/probe data covers the drift; remove only if a real measurement backend becomes the default",
            },
            OverrideCategory::HandCuratedHelpers => OverrideCategoryMetadata {
                owner: "diagram renderer owner",
                source: "small hand-curated constants for known Mermaid browser/layout quirks",
                allowed_use: "narrow constants that are stable, tested, and cheaper than broad generated tables",
                expected_removal: "replace with repeatable generated data or typed model/layout computations as soon as a reliable source exists",
            },
            OverrideCategory::RawPathBridge => OverrideCategoryMetadata {
                owner: "diagram-specific svg/parity module owner",
                source: "hand-authored maybe_override_* functions under svg/parity",
                allowed_use: "temporary exact raw SVG/path bridges for literal upstream behavior that the generic emitter cannot reproduce yet",
                expected_removal: "delete once typed layout/path emission reproduces the upstream literal behavior; keep local owner/removal notes beside each bridge",
            },
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
    let mut check_no_growth = false;

    for arg in args {
        match arg.as_str() {
            "--check-no-growth" => check_no_growth = true,
            "--help" | "-h" => {
                println!("usage: xtask report-overrides [--check-no-growth]");
                println!();
                println!("Prints a parity override footprint inventory.");
                println!("This is intended for CI logs and drift reviews.");
                println!();
                println!("Options:");
                println!(
                    "  --check-no-growth  fail if any category grows beyond the explicit budget or root viewport lookups bypass the shared helper"
                );
                return Ok(());
            }
            _ => return Err(XtaskError::Usage),
        }
    }

    if check_no_growth {
        println!("Override growth budget: enabled");
        println!();
    }

    let workspace_root = crate::cmd::workspace_root();

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

    if check_no_growth {
        check_override_no_growth(&entries)?;
        println!("Override growth check: ok");
        check_root_viewport_lookup_usage(&parity_dir, &source_root)?;
        println!("Root viewport override usage check: ok");
        println!();
    }

    println!("Notes:");
    println!("- Counts are inventory units and are not directly comparable across categories.");
    println!(
        "- Generated module counts cover `crates/merman-render/src/generated`, while manual bridge counts cover hand-authored path-bridge helpers under `crates/merman-render/src/svg/parity`."
    );
    println!("- Root viewport entries count match arms returning `Some((viewBox, max_width))`.");
    println!(
        "- Root viewport lookups in parity renderers must be applied through `apply_root_viewport_override`."
    );
    println!(
        "- Text lookup entries count generated or hand-curated `=> Some(...)` parity branches and rows in `*_OVERRIDES_*` lookup tables."
    );
    println!("- Table rows count tuple rows in generated font/SVG metric arrays.");

    Ok(())
}

fn check_override_no_growth(entries: &[OverrideFootprintEntry]) -> Result<(), XtaskError> {
    let mut failures = Vec::new();
    for category in OverrideCategory::ALL {
        let total = category_total(entries, category);
        let budget = category.no_growth_budget();
        if total > budget {
            failures.push(format!(
                "{} grew to {total} {}, budget {budget}",
                category.heading(),
                category.total_unit()
            ));
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::VerifyFailed(format!(
        "override footprint grew beyond the explicit no-growth budget:\n{}",
        failures.join("\n")
    )))
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RootViewportLookupViolation {
    file_name: String,
    line_number: usize,
    line: String,
}

fn check_root_viewport_lookup_usage(
    parity_dir: &Path,
    source_root: &Path,
) -> Result<(), XtaskError> {
    let mut files = collect_parity_rs_files(parity_dir)?;
    files.sort();

    let mut violations = Vec::new();
    for path in files {
        let Some(file_name) = path.strip_prefix(source_root).ok().map(report_path_name) else {
            continue;
        };
        let text = read_text(&path)?;
        violations.extend(find_root_viewport_lookup_violations(&file_name, &text));
    }

    if violations.is_empty() {
        return Ok(());
    }

    let details = violations
        .iter()
        .map(|violation| {
            format!(
                "{}:{}: {}",
                violation.file_name,
                violation.line_number,
                violation.line.trim()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Err(XtaskError::VerifyFailed(format!(
        "root viewport override lookup bypassed shared helper:\n{details}"
    )))
}

fn find_root_viewport_lookup_violations(
    file_name: &str,
    text: &str,
) -> Vec<RootViewportLookupViolation> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"\blookup_[A-Za-z0-9_]+_root_viewport_override\b"#).expect("valid regex")
    });

    let mut violations = Vec::new();
    let mut apply_call_depth: Option<i32> = None;
    for (line_index, line) in text.lines().enumerate() {
        let code = strip_line_comment(line);
        let mut delta_input = None;
        if apply_call_depth.is_none() {
            if let Some(start) = code.find("apply_root_viewport_override(") {
                apply_call_depth = Some(0);
                delta_input = Some(&code[start..]);
            }
        } else {
            delta_input = Some(code);
        }

        if re.is_match(code) && apply_call_depth.is_none() {
            violations.push(RootViewportLookupViolation {
                file_name: file_name.to_string(),
                line_number: line_index + 1,
                line: line.to_string(),
            });
        }

        if let (Some(depth), Some(delta_input)) = (apply_call_depth.as_mut(), delta_input) {
            *depth += paren_delta(delta_input);
            if *depth <= 0 {
                apply_call_depth = None;
            }
        }
    }

    violations
}

fn strip_line_comment(line: &str) -> &str {
    line.split_once("//").map_or(line, |(code, _)| code)
}

fn paren_delta(text: &str) -> i32 {
    let mut delta = 0;
    for ch in text.chars() {
        match ch {
            '(' => delta += 1,
            ')' => delta -= 1,
            _ => {}
        }
    }
    delta
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
        entries.extend(classify_generated_override_file(file_name, &text));
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

fn classify_generated_override_file(file_name: String, text: &str) -> Vec<OverrideFootprintEntry> {
    if file_name.contains("_root_overrides_") {
        return vec![OverrideFootprintEntry {
            file_name,
            category: OverrideCategory::RootViewport,
            count: count_root_viewport_entries(text),
            unit: "entries",
        }];
    }

    if file_name.starts_with("font_metrics_") {
        return vec![OverrideFootprintEntry {
            file_name,
            category: OverrideCategory::FontMetrics,
            count: count_tuple_rows(text),
            unit: "table rows",
        }];
    }

    if file_name.starts_with("svg_overrides_") {
        return vec![OverrideFootprintEntry {
            file_name,
            category: OverrideCategory::SvgTextMetrics,
            count: count_tuple_rows(text),
            unit: "table rows",
        }];
    }

    if file_name.contains("_text_overrides_") {
        let lookup_entries = count_some_match_arms(text) + count_static_override_table_rows(text);
        if lookup_entries > 0 {
            return vec![OverrideFootprintEntry {
                file_name,
                category: OverrideCategory::TextLookup,
                count: lookup_entries,
                unit: "lookup entries",
            }];
        }

        return vec![OverrideFootprintEntry {
            file_name,
            category: OverrideCategory::HandCuratedHelpers,
            count: count_visible_functions(text),
            unit: "helper functions",
        }];
    }

    Vec::new()
}

fn print_category(entries: &[OverrideFootprintEntry], category: OverrideCategory) {
    let category_entries: Vec<_> = entries
        .iter()
        .filter(|entry| entry.category == category)
        .collect();

    let total: usize = category_entries.iter().map(|entry| entry.count).sum();
    let metadata = category.metadata();
    println!("{}:", category.heading());
    println!("- owner: {}", metadata.owner);
    println!("- source: {}", metadata.source);
    println!("- allowed use: {}", metadata.allowed_use);
    println!("- expected removal: {}", metadata.expected_removal);
    println!("- total: {total} {}", category.total_unit());
    if category_entries.is_empty() {
        println!("- no entries");
    } else {
        for entry in category_entries {
            println!("- {}: {} {}", entry.file_name, entry.count, entry.unit);
        }
    }
    println!();
}

fn category_total(entries: &[OverrideFootprintEntry], category: OverrideCategory) -> usize {
    entries
        .iter()
        .filter(|entry| entry.category == category)
        .map(|entry| entry.count)
        .sum()
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
    let re = RE.get_or_init(|| Regex::new(r#"=>\s*(?:\{\s*)?Some\("#).expect("valid regex"));
    count_matches(re, text)
}

fn count_tuple_rows(text: &str) -> usize {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"(?m)^\s*\("#).expect("valid regex"));
    count_matches(re, text)
}

fn count_static_override_table_rows(text: &str) -> usize {
    let mut in_override_table = false;
    let mut rows = 0usize;

    for line in text.lines() {
        let trimmed = line.trim_start();
        if !in_override_table {
            in_override_table = trimmed.starts_with("static ")
                && trimmed.contains("_OVERRIDES")
                && trimmed.contains("&[");
            continue;
        }

        if trimmed.starts_with("];") {
            in_override_table = false;
            continue;
        }

        if trimmed.starts_with('(') {
            rows += 1;
        }
    }

    rows
}

fn count_visible_functions(text: &str) -> usize {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"(?m)^pub(?:\([^)]+\))?\s+fn\s+[A-Za-z0-9_]+\s*\("#).expect("valid regex")
    });
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
    use super::{
        OverrideCategory, OverrideFootprintEntry, check_override_no_growth,
        classify_generated_override_file, count_manual_bridge_functions, count_some_match_arms,
        count_static_override_table_rows, count_visible_functions,
        find_root_viewport_lookup_violations, report_path_name,
    };
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
    fn counts_visible_helper_functions() {
        let text = r#"
pub fn helper_one() {}
pub(crate) fn helper_two(
) {}
pub(in crate::cmd) fn helper_three(
) {}
fn private_helper() {}
"#;

        assert_eq!(count_visible_functions(text), 3);
    }

    #[test]
    fn counts_block_wrapped_some_match_arms() {
        let text = r#"
match (font_key, text) {
    ("trebuchetms,verdana,arial,sans-serif", "wide label") => {
        Some((84.1328125, 84.1328125))
    }
    ("trebuchetms,verdana,arial,sans-serif", "short label") => Some(42.0),
    _ => None,
}
"#;

        assert_eq!(count_some_match_arms(text), 2);
    }

    #[test]
    fn counts_static_override_lookup_rows() {
        let text = r#"
static HTML_WIDTH_OVERRIDES_PX: &[(u16, &str, f64)] = &[
    (1600, "A", 9.4375),
    (
        2400,
        "Font size precedence should widen this block",
        487.890625,
    ),
];

static OTHER_ROWS: &[(u16, &str, f64)] = &[
    (1600, "ignored", 1.0),
];
"#;

        assert_eq!(count_static_override_table_rows(text), 2);
    }

    #[test]
    fn classifies_static_text_tables_as_lookup_entries() {
        let text = r#"
static TASK_TEXT_BBOX_WIDTH_OVERRIDES_PX: &[(u16, &str, f64)] = &[
    (1100, "Task", 22.24853515625),
    (1100, "Task2", 27.796875),
];

pub fn lookup_task_text_bbox_width_px(font_size: f64, text: &str) -> Option<f64> {
    let _ = (font_size, text);
    None
}
"#;

        let entries =
            classify_generated_override_file("er_text_overrides_11_12_2.rs".to_string(), text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].category, OverrideCategory::TextLookup);
        assert_eq!(entries[0].count, 2);
        assert_eq!(entries[0].unit, "lookup entries");
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

    #[test]
    fn generated_categories_report_removal_metadata() {
        for category in [
            OverrideCategory::RootViewport,
            OverrideCategory::TextLookup,
            OverrideCategory::SvgTextMetrics,
            OverrideCategory::FontMetrics,
            OverrideCategory::HandCuratedHelpers,
        ] {
            let metadata = category.metadata();
            assert!(!metadata.source.is_empty());
            assert!(!metadata.allowed_use.is_empty());
            assert!(!metadata.expected_removal.is_empty());
        }
    }

    #[test]
    fn override_growth_check_allows_current_budget() {
        let entries: Vec<_> = OverrideCategory::ALL
            .into_iter()
            .map(|category| OverrideFootprintEntry {
                file_name: category.heading().to_string(),
                category,
                count: category.no_growth_budget(),
                unit: category.total_unit(),
            })
            .collect();

        assert!(check_override_no_growth(&entries).is_ok());
    }

    #[test]
    fn override_growth_check_rejects_category_growth() {
        let entries = [OverrideFootprintEntry {
            file_name: "flowchart_root_overrides_11_12_2.rs".to_string(),
            category: OverrideCategory::RootViewport,
            count: OverrideCategory::RootViewport.no_growth_budget() + 1,
            unit: "entries",
        }];

        let err = check_override_no_growth(&entries).expect_err("growth should fail");
        let msg = err.to_string();
        assert!(msg.contains("Root viewport overrides grew"));
        assert!(msg.contains("budget 750"));
    }

    #[test]
    fn override_growth_check_rejects_manual_bridge_growth() {
        let entries = [OverrideFootprintEntry {
            file_name: "svg/parity/flowchart/edge_geom/degenerate_path.rs".to_string(),
            category: OverrideCategory::RawPathBridge,
            count: 1,
            unit: "bridge functions",
        }];

        let err = check_override_no_growth(&entries).expect_err("bridge growth should fail");
        let msg = err.to_string();
        assert!(msg.contains("Manual raw SVG/path bridges grew"));
        assert!(msg.contains("budget 0"));
    }

    #[test]
    fn root_viewport_lookup_usage_accepts_shared_helper_call() {
        let text = r#"
fn apply(diagram_id: &str) {
    apply_root_viewport_override(
        diagram_id,
        &mut viewbox_attr,
        &mut width_attr,
        &mut height_attr,
        &mut max_width_attr,
        crate::generated::state_root_overrides_11_12_2::lookup_state_root_viewport_override,
    );
}
"#;

        assert!(find_root_viewport_lookup_violations("state/render.rs", text).is_empty());
    }

    #[test]
    fn root_viewport_lookup_usage_rejects_direct_lookup_call() {
        let text = r#"
fn apply(diagram_id: &str) {
    if let Some((viewbox, max_w)) =
        crate::generated::state_root_overrides_11_12_2::lookup_state_root_viewport_override(
            diagram_id,
        )
    {
        viewbox_attr = viewbox.to_string();
        max_w_attr = max_w.to_string();
    }
}
"#;

        let violations = find_root_viewport_lookup_violations("state/render.rs", text);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].file_name, "state/render.rs");
        assert_eq!(violations[0].line_number, 4);
    }

    #[test]
    fn root_viewport_lookup_usage_ignores_line_comments() {
        let text = r#"
fn apply() {
    // crate::generated::state_root_overrides_11_12_2::lookup_state_root_viewport_override
}
"#;

        assert!(find_root_viewport_lookup_violations("state/render.rs", text).is_empty());
    }
}

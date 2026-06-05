//! Global root viewport override governance audit.

use crate::cmd::compare::diagram_supports_root_delta_report;
use crate::{XtaskError, cmd};
use merman_core::baseline::{
    LEGACY_GENERATED_BASELINE_SUFFIX, PINNED_MERMAID_BASELINE_VERSION_SUFFIX,
};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
struct RootOverrideTable {
    family: String,
    file_name: String,
    inventory_entries: usize,
    fixture_keys: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct FamilyAudit {
    table: RootOverrideTable,
    report_path: Option<PathBuf>,
    exit_code: Option<i32>,
    dom_mismatch_keys: BTreeSet<String>,
    root_delta_keys: BTreeSet<String>,
    stale_keys: BTreeSet<String>,
    retained_keys: BTreeSet<String>,
    missing_keys: BTreeSet<String>,
    runner_issues: Vec<String>,
}

pub(crate) fn audit_root_overrides(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path = cmd::target_root()
        .join("compare")
        .join("root_override_global_audit_current.md");
    let mut report_dir = cmd::target_root()
        .join("compare")
        .join("root_override_global_audit_current_reports");
    let mut only_families: BTreeSet<String> = BTreeSet::new();
    let mut inventory_only = false;
    let mut fail_on_stale = false;
    let mut dom_decimals = 3u32;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from).ok_or(XtaskError::Usage)?;
            }
            "--report-dir" => {
                i += 1;
                report_dir = args.get(i).map(PathBuf::from).ok_or(XtaskError::Usage)?;
            }
            "--diagram" => {
                i += 1;
                let family = args.get(i).ok_or(XtaskError::Usage)?.trim();
                if !family.is_empty() {
                    only_families.insert(family.to_ascii_lowercase());
                }
            }
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args
                    .get(i)
                    .and_then(|s| s.parse::<u32>().ok())
                    .ok_or(XtaskError::Usage)?;
            }
            "--inventory-only" => inventory_only = true,
            "--fail-on-stale" => fail_on_stale = true,
            "--help" | "-h" => {
                println!(
                    "usage: xtask audit-root-overrides [--diagram <name>] [--out <path>] [--report-dir <path>] [--dom-decimals <n>] [--inventory-only] [--fail-on-stale]"
                );
                println!();
                println!(
                    "Cross-checks generated root viewport override keys against disabled-root parity-root DOM mismatches."
                );
                println!(
                    "The compare runs execute in child xtask processes with MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1."
                );
                return Ok(());
            }
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let generated_dir = cmd::workspace_root()
        .join("crates")
        .join("merman-render")
        .join("src")
        .join("generated");
    let mut tables = collect_root_override_tables(&generated_dir)?;
    if !only_families.is_empty() {
        tables.retain(|table| only_families.contains(&table.family));
    }
    if tables.is_empty() {
        return Err(XtaskError::Usage);
    }

    if !inventory_only {
        fs::create_dir_all(&report_dir).map_err(|source| XtaskError::WriteFile {
            path: report_dir.display().to_string(),
            source,
        })?;
    }

    let mut audits = Vec::new();
    for table in tables {
        if inventory_only {
            audits.push(FamilyAudit::from_inventory(table));
            continue;
        }

        let family_report_path = report_dir.join(format!("{}_disabled_root.md", table.family));
        let run = run_disabled_root_compare(
            &table.family,
            &family_report_path,
            &table.fixture_keys,
            dom_decimals,
        )?;
        audits.push(FamilyAudit::from_run(table, family_report_path, run));
    }

    let report = render_global_root_override_audit(&audits, inventory_only, dom_decimals);
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

    println!("wrote report: {}", out_path.display());

    if fail_on_stale {
        let failures = audits
            .iter()
            .filter(|audit| !audit.stale_keys.is_empty() || !audit.runner_issues.is_empty())
            .map(|audit| {
                format!(
                    "{}: stale={} runner_issues={}",
                    audit.table.family,
                    audit.stale_keys.len(),
                    audit.runner_issues.len()
                )
            })
            .collect::<Vec<_>>();
        if !failures.is_empty() {
            return Err(XtaskError::VerifyFailed(format!(
                "root override audit found stale root overrides or inconclusive runs:\n{}",
                failures.join("\n")
            )));
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct CompareRun {
    exit_code: Option<i32>,
    dom_mismatch_keys: BTreeSet<String>,
    root_delta_keys: BTreeSet<String>,
    runner_issues: Vec<String>,
}

impl FamilyAudit {
    fn from_inventory(table: RootOverrideTable) -> Self {
        Self {
            table,
            report_path: None,
            exit_code: None,
            dom_mismatch_keys: BTreeSet::new(),
            root_delta_keys: BTreeSet::new(),
            stale_keys: BTreeSet::new(),
            retained_keys: BTreeSet::new(),
            missing_keys: BTreeSet::new(),
            runner_issues: Vec::new(),
        }
    }

    fn from_run(table: RootOverrideTable, report_path: PathBuf, run: CompareRun) -> Self {
        let stale_keys = table
            .fixture_keys
            .difference(&run.root_delta_keys)
            .cloned()
            .collect();
        let retained_keys = table
            .fixture_keys
            .intersection(&run.root_delta_keys)
            .cloned()
            .collect();
        let missing_keys = run
            .dom_mismatch_keys
            .difference(&table.fixture_keys)
            .cloned()
            .collect();

        Self {
            table,
            report_path: Some(report_path),
            exit_code: run.exit_code,
            dom_mismatch_keys: run.dom_mismatch_keys,
            root_delta_keys: run.root_delta_keys,
            stale_keys,
            retained_keys,
            missing_keys,
            runner_issues: run.runner_issues,
        }
    }
}

fn collect_root_override_tables(
    generated_dir: &Path,
) -> Result<Vec<RootOverrideTable>, XtaskError> {
    let read_dir = fs::read_dir(generated_dir).map_err(|source| XtaskError::ReadFile {
        path: generated_dir.display().to_string(),
        source,
    })?;

    let mut tables = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|source| XtaskError::ReadFile {
            path: generated_dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        let Some(file_name) = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        let Some(family) = root_override_family_from_file_name(&file_name) else {
            continue;
        };

        let text = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;
        let inventory_entries = count_root_viewport_entries(&text);
        let fixture_keys = collect_root_override_fixture_keys(&text);
        tables.push(RootOverrideTable {
            family,
            file_name,
            inventory_entries,
            fixture_keys,
        });
    }

    tables.sort_by(|a, b| a.family.cmp(&b.family));
    Ok(tables)
}

fn root_override_family_from_file_name(file_name: &str) -> Option<String> {
    for suffix in [
        PINNED_MERMAID_BASELINE_VERSION_SUFFIX,
        LEGACY_GENERATED_BASELINE_SUFFIX,
    ] {
        if let Some(family) = file_name
            .strip_suffix(&format!("_root_overrides_{suffix}.rs"))
            .map(str::to_owned)
        {
            return Some(family);
        }
    }
    None
}

fn run_disabled_root_compare(
    family: &str,
    report_path: &Path,
    fixture_keys: &BTreeSet<String>,
    dom_decimals: u32,
) -> Result<CompareRun, XtaskError> {
    let exe = std::env::current_exe().map_err(|source| XtaskError::ReadFile {
        path: "current executable".to_string(),
        source,
    })?;
    let mut command = Command::new(exe);
    command.current_dir(cmd::workspace_root());
    command.env("MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES", "1");
    command.arg(format!("compare-{family}-svgs"));
    command.arg("--check-dom");
    command.arg("--dom-mode");
    command.arg("parity-root");
    command.arg("--dom-decimals");
    command.arg(dom_decimals.to_string());
    command.arg("--out");
    command.arg(report_path);
    if family == "flowchart" {
        command.arg("--no-root-overrides");
    }
    if diagram_supports_root_delta_report(family) {
        command.arg("--report-root-all");
    }

    let output = command.output().map_err(|source| XtaskError::ReadFile {
        path: format!("spawn compare-{family}-svgs"),
        source,
    })?;
    let report_text = fs::read_to_string(report_path).unwrap_or_default();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}\n{report_text}");
    let dom_mismatch_keys = collect_dom_mismatch_keys(&combined);
    let (root_delta_keys, root_delta_issues) =
        collect_table_root_delta_keys(family, report_path, fixture_keys);
    let mut runner_issues = collect_runner_issues(&combined);
    runner_issues.extend(root_delta_issues);
    if !output.status.success() && dom_mismatch_keys.is_empty() {
        runner_issues.push(format!(
            "compare-{family}-svgs exited {:?} without parseable DOM mismatch rows",
            output.status.code()
        ));
    }

    Ok(CompareRun {
        exit_code: output.status.code(),
        dom_mismatch_keys,
        root_delta_keys,
        runner_issues,
    })
}

fn collect_table_root_delta_keys(
    family: &str,
    report_path: &Path,
    fixture_keys: &BTreeSet<String>,
) -> (BTreeSet<String>, Vec<String>) {
    let mut keys = BTreeSet::new();
    let mut issues = Vec::new();
    let local_dir = report_path
        .parent()
        .map(|parent| parent.join(family))
        .unwrap_or_else(|| cmd::target_root().join("compare").join(family));
    let upstream_dir = cmd::fixtures_root().join("upstream-svgs").join(family);

    for fixture in fixture_keys {
        let upstream_path = upstream_dir.join(format!("{fixture}.svg"));
        let local_path = local_dir.join(format!("{fixture}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(svg) => svg,
            Err(err) => {
                issues.push(format!(
                    "failed to read upstream root attrs for {family}/{fixture}: {} ({err})",
                    upstream_path.display()
                ));
                continue;
            }
        };
        let local_svg = match fs::read_to_string(&local_path) {
            Ok(svg) => svg,
            Err(err) => {
                issues.push(format!(
                    "failed to read local root attrs for {family}/{fixture}: {} ({err})",
                    local_path.display()
                ));
                continue;
            }
        };
        let upstream = match crate::cmd::compare::parse_root_attrs(&upstream_svg) {
            Ok(attrs) => attrs,
            Err(err) => {
                issues.push(format!(
                    "failed to parse upstream root attrs for {family}/{fixture}: {err}"
                ));
                continue;
            }
        };
        let local = match crate::cmd::compare::parse_root_attrs(&local_svg) {
            Ok(attrs) => attrs,
            Err(err) => {
                issues.push(format!(
                    "failed to parse local root attrs for {family}/{fixture}: {err}"
                ));
                continue;
            }
        };
        if root_attrs_differ(&upstream, &local) {
            keys.insert(fixture.clone());
        }
    }

    (keys, issues)
}

fn root_attrs_differ(
    upstream: &crate::cmd::compare::RootAttrs,
    local: &crate::cmd::compare::RootAttrs,
) -> bool {
    const EPS: f64 = 1e-9;
    match (upstream.max_width_px, local.max_width_px) {
        (Some(a), Some(b)) if (a - b).abs() > EPS => return true,
        (Some(_), None) | (None, Some(_)) => return true,
        _ => {}
    }
    match (upstream.viewbox, local.viewbox) {
        (Some(a), Some(b)) => {
            let upstream = [a.0, a.1, a.2, a.3];
            let local = [b.0, b.1, b.2, b.3];
            upstream
                .iter()
                .zip(local.iter())
                .any(|(a, b)| (*a - *b).abs() > EPS)
        }
        (Some(_), None) | (None, Some(_)) => true,
        _ => false,
    }
}

fn render_global_root_override_audit(
    audits: &[FamilyAudit],
    inventory_only: bool,
    dom_decimals: u32,
) -> String {
    let mut out = String::new();
    let _ = writeln!(&mut out, "# Global Root Override Audit\n");
    let _ = writeln!(
        &mut out,
        "- Generated at: `{}`",
        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    );
    let _ = writeln!(
        &mut out,
        "- Mermaid baseline: `{}`",
        crate::cmd::pinned_mermaid_baseline_label(&cmd::workspace_root())
    );
    let _ = writeln!(&mut out, "- DOM mode: `parity-root`");
    let _ = writeln!(&mut out, "- DOM decimals: `{dom_decimals}`");
    let _ = writeln!(
        &mut out,
        "- Root overrides during compare: `{}`",
        if inventory_only {
            "not-run"
        } else {
            "disabled via MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1"
        }
    );
    let _ = writeln!(
        &mut out,
        "- Policy: stale entries are deletion candidates only after disabled-root SVG outputs prove the fixture root attrs now match upstream; outside-table DOM mismatches are parity regressions or new root-only candidates that normal gates should explain.\n"
    );

    let total_inventory_entries: usize = audits
        .iter()
        .map(|audit| audit.table.inventory_entries)
        .sum();
    let total_fixture_keys: usize = audits
        .iter()
        .map(|audit| audit.table.fixture_keys.len())
        .sum();
    let total_retained: usize = audits.iter().map(|audit| audit.retained_keys.len()).sum();
    let total_stale: usize = audits.iter().map(|audit| audit.stale_keys.len()).sum();
    let total_missing: usize = audits.iter().map(|audit| audit.missing_keys.len()).sum();
    let total_runner_issues: usize = audits.iter().map(|audit| audit.runner_issues.len()).sum();

    let _ = writeln!(&mut out, "## Summary\n");
    let _ = writeln!(
        &mut out,
        "| family | module | inventory entries | fixture keys | root delta keys | DOM mismatches | retained | stale | outside-table DOM | exit | report |"
    );
    let _ = writeln!(
        &mut out,
        "|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|"
    );
    for audit in audits {
        let report = audit
            .report_path
            .as_ref()
            .map(|path| format!("`{}`", path.display()))
            .unwrap_or_else(|| "-".to_string());
        let exit = audit
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "-".to_string());
        let _ = writeln!(
            &mut out,
            "| `{}` | `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            audit.table.family,
            audit.table.file_name,
            audit.table.inventory_entries,
            audit.table.fixture_keys.len(),
            audit.root_delta_keys.len(),
            audit.dom_mismatch_keys.len(),
            audit.retained_keys.len(),
            audit.stale_keys.len(),
            audit.missing_keys.len(),
            exit,
            report
        );
    }
    let _ = writeln!(
        &mut out,
        "| **total** |  | **{}** | **{}** | **{}** | **{}** | **{}** | **{}** | **{}** |  |  |\n",
        total_inventory_entries,
        total_fixture_keys,
        audits
            .iter()
            .map(|audit| audit.root_delta_keys.len())
            .sum::<usize>(),
        audits
            .iter()
            .map(|audit| audit.dom_mismatch_keys.len())
            .sum::<usize>(),
        total_retained,
        total_stale,
        total_missing
    );

    let _ = writeln!(&mut out, "## Findings\n");
    if inventory_only {
        let _ = writeln!(
            &mut out,
            "- Inventory-only run; disabled-root compare was not executed.\n"
        );
    } else if total_stale == 0 && total_missing == 0 && total_runner_issues == 0 {
        let _ = writeln!(
            &mut out,
            "- No stale retained root override keys found across the generated tables."
        );
        let _ = writeln!(
            &mut out,
            "- No outside-table disabled-root DOM mismatches found."
        );
        let _ = writeln!(
            &mut out,
            "- Current result: all retained root pins still guard visible `parity-root` drift; there are no table-only deletion candidates in this pass.\n"
        );
    } else {
        if total_stale > 0 {
            let _ = writeln!(
                &mut out,
                "- Stale candidates found: `{total_stale}`. These are the first deletion targets, but remove them only with focused normal and disabled-root `parity-root` checks."
            );
        }
        if total_missing > 0 {
            let _ = writeln!(
                &mut out,
                "- Outside-table DOM mismatches found: `{total_missing}`. These are disabled-root DOM failures outside the current generated tables and need a normal-gate/regression explanation."
            );
        }
        if total_runner_issues > 0 {
            let _ = writeln!(
                &mut out,
                "- Runner issues found: `{total_runner_issues}`. Treat affected family rows as inconclusive."
            );
        }
        out.push('\n');
    }

    for audit in audits {
        if audit.stale_keys.is_empty()
            && audit.missing_keys.is_empty()
            && audit.runner_issues.is_empty()
        {
            continue;
        }
        let _ = writeln!(&mut out, "## `{}` Details\n", audit.table.family);
        if !audit.stale_keys.is_empty() {
            push_key_list(&mut out, "Stale deletion candidates", &audit.stale_keys);
        }
        if !audit.missing_keys.is_empty() {
            push_key_list(
                &mut out,
                "Outside-table DOM mismatches",
                &audit.missing_keys,
            );
        }
        if !audit.runner_issues.is_empty() {
            let _ = writeln!(&mut out, "### Runner Issues\n");
            for issue in &audit.runner_issues {
                let _ = writeln!(&mut out, "- {}", markdown_cell(issue));
            }
            out.push('\n');
        }
    }

    let mut coverage = BTreeMap::new();
    for audit in audits {
        coverage.insert(
            audit.table.family.as_str(),
            if diagram_supports_root_delta_report(audit.table.family.as_str()) {
                "compare report includes root delta table"
            } else {
                "mismatch cross-check only; per-family root delta table not yet wired"
            },
        );
    }
    let _ = writeln!(&mut out, "## Coverage Notes\n");
    for (family, note) in coverage {
        let _ = writeln!(&mut out, "- `{family}`: {note}");
    }
    out.push('\n');

    out
}

fn push_key_list(out: &mut String, title: &str, keys: &BTreeSet<String>) {
    let _ = writeln!(out, "### {title} ({})\n", keys.len());
    for key in keys {
        let _ = writeln!(out, "- `{}`", markdown_cell(key));
    }
    out.push('\n');
}

fn collect_root_override_fixture_keys(text: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for arm in root_override_arm_re().captures_iter(text) {
        let Some(patterns) = arm.get(1).map(|m| m.as_str()) else {
            continue;
        };
        for cap in quoted_string_re().captures_iter(patterns) {
            if let Some(key) = cap.get(1) {
                keys.insert(key.as_str().to_string());
            }
        }
    }

    keys
}

fn count_root_viewport_entries(text: &str) -> usize {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| Regex::new(r#""[^"]+"\s*=>\s*(?:\{\s*)?Some\("#).expect("valid regex"));
    re.find_iter(text).count()
}

fn collect_dom_mismatch_keys(text: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for re in [
        dom_mismatch_for_re(),
        er_dom_mismatch_detail_re(),
        fail_status_re(),
    ] {
        for cap in re.captures_iter(text) {
            if let Some(key) = cap.get(1).map(|m| m.as_str()) {
                if is_fixture_like_key(key) {
                    keys.insert(key.to_string());
                }
            }
        }
    }
    keys
}

fn collect_runner_issues(text: &str) -> Vec<String> {
    let issue_markers = [
        "missing upstream svg",
        "parse failed",
        "layout failed",
        "render failed",
        "dom parse failed",
        "failed to parse upstream svg dom",
        "failed to parse local svg dom",
        "root parse failed",
        "unexpected layout type",
        "no diagram detected",
        "no .mmd fixtures matched",
    ];

    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let lower = trimmed.to_ascii_lowercase();
            if issue_markers.iter().any(|marker| lower.contains(marker)) {
                Some(trimmed.trim_start_matches("- ").to_string())
            } else {
                None
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn is_fixture_like_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

fn markdown_cell(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\r', "")
        .replace('\n', "\\n")
}

fn quoted_string_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#""([^"]+)""#).expect("valid regex"))
}

fn root_override_arm_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?ms)((?:"[^"]+"\s*(?:\|\s*)?|\|\s*"[^"]+"\s*)+)=>\s*(?:\{\s*)?Some\s*\("#)
            .expect("valid regex")
    })
}

fn dom_mismatch_for_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"dom mismatch for ([A-Za-z0-9_.-]+)"#).expect("valid regex"))
}

fn er_dom_mismatch_detail_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?m)^- ([A-Za-z0-9_.-]+): "#).expect("valid regex"))
}

fn fail_status_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?m)^- FAIL `([^`]+)`"#).expect("valid regex"))
}

#[cfg(test)]
mod tests {
    use super::{
        FamilyAudit, RootOverrideTable, collect_dom_mismatch_keys,
        collect_root_override_fixture_keys, collect_runner_issues, count_root_viewport_entries,
        render_global_root_override_audit, root_override_family_from_file_name,
    };
    use std::collections::BTreeSet;

    #[test]
    fn collects_all_fixture_keys_from_or_pattern_root_arms() {
        let text = r#"
match diagram_id {
    "alpha_stress_case" => Some(("0 0 10 10", "10")),
    "upstream_one"
    | "upstream_two" => {
        Some(("0 0 20 20", "20"))
    }
    _ => None,
}
"#;

        let keys = collect_root_override_fixture_keys(text);

        assert!(keys.contains("alpha_stress_case"));
        assert!(keys.contains("upstream_one"));
        assert!(keys.contains("upstream_two"));
        assert_eq!(keys.len(), 3);
        assert_eq!(count_root_viewport_entries(text), 2);
    }

    #[test]
    fn collects_mismatches_from_compare_reports_and_errors() {
        let text = r#"
- dom mismatch for upstream_docs_one: upstream=a local=b
## DOM Mismatch Details
- stress_er_two: child-count mismatch
- FAIL `requirement_three`
"#;

        let keys = collect_dom_mismatch_keys(text);

        assert!(keys.contains("upstream_docs_one"));
        assert!(keys.contains("stress_er_two"));
        assert!(keys.contains("requirement_three"));
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn collects_runner_issues_without_dom_mismatch_noise() {
        let text = r#"
- dom mismatch for upstream_docs_one: upstream=a local=b
- parse failed for fixtures/x.mmd: invalid syntax
- missing upstream svg for stress_missing: fixtures/upstream-svgs/x.svg
"#;

        let issues = collect_runner_issues(text);

        assert_eq!(issues.len(), 2);
        assert!(issues.iter().any(|issue| issue.contains("parse failed")));
        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("missing upstream svg"))
        );
    }

    #[test]
    fn root_override_inventory_accepts_current_and_legacy_suffixes() {
        assert_eq!(
            root_override_family_from_file_name("eventmodeling_root_overrides_11_15_0.rs"),
            Some("eventmodeling".to_string())
        );
        assert_eq!(
            root_override_family_from_file_name("timeline_root_overrides_11_12_2.rs"),
            Some("timeline".to_string())
        );
        assert_eq!(root_override_family_from_file_name("not_root.rs"), None);
    }

    #[test]
    fn coverage_notes_use_shared_root_delta_support_for_timeline() {
        let audit = FamilyAudit::from_inventory(RootOverrideTable {
            family: "timeline".to_string(),
            file_name: "timeline_root_overrides_11_12_2.rs".to_string(),
            inventory_entries: 1,
            fixture_keys: BTreeSet::from(["timeline_fixture".to_string()]),
        });

        let report = render_global_root_override_audit(&[audit], true, 3);

        assert!(report.contains("`timeline`: compare report includes root delta table"));
        assert!(!report.contains(
            "`timeline`: mismatch cross-check only; per-family root delta table not yet wired"
        ));
    }
}

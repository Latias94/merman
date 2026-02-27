//! Gap audits and corpus health checks.

use crate::XtaskError;
use crate::util::*;
use regex::Regex;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Clone)]
struct DeferredParseOk {
    path: PathBuf,
    expected_group: String,
    diagram_type: String,
    out_of_scope: Vec<String>,
}

#[derive(Debug, Clone)]
struct DeferredParseErr {
    path: PathBuf,
    expected_group: String,
    message_key: String,
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn collect_mmd_files_recursive(root: &Path) -> Result<Vec<PathBuf>, XtaskError> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|source| XtaskError::ReadFile {
            path: dir.display().to_string(),
            source,
        })?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if is_file_with_extension(&path, "mmd") {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn top_level_dir_under(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    rel.components()
        .next()?
        .as_os_str()
        .to_str()
        .map(|s| s.to_string())
}

fn normalize_error_key(message: &str) -> String {
    // Keep this stable and reasonably grouping-friendly:
    // - first line only
    // - collapse file/line/col suffixes if present
    // - collapse repeated whitespace
    static WS: OnceLock<Regex> = OnceLock::new();
    static LOC: OnceLock<Regex> = OnceLock::new();
    let ws = WS.get_or_init(|| Regex::new(r#"\s+"#).unwrap());
    let loc = LOC.get_or_init(|| Regex::new(r#"\s*\(line\s*\d+,\s*col\s*\d+\)\s*$"#).unwrap());

    let first = message.lines().next().unwrap_or(message).trim();
    let first = loc.replace(first, "");
    ws.replace_all(first.trim(), " ").to_string()
}

fn detect_out_of_scope(meta: &merman::ParseMetadata) -> Vec<String> {
    let mut out = Vec::new();

    if let Some(layout) = meta.config.get_str("layout") {
        out.push(format!("layout={layout}"));
    }
    if let Some(look) = meta.config.get_str("look") {
        out.push(format!("look={look}"));
    }

    out
}

pub(crate) fn audit_gaps(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut limit: usize = 60;

    let mut i = 0usize;
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
            "--limit" => {
                i += 1;
                limit = args
                    .get(i)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(60);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = workspace_root();
    let fixtures_root = workspace_root.join("fixtures");
    let deferred_root = fixtures_root.join("_deferred");

    let out_path =
        out_path.unwrap_or_else(|| workspace_root.join("target").join("audit").join("gaps.md"));

    let engine = merman::Engine::new()
        .with_fixed_today(Some(
            chrono::NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid date"),
        ))
        .with_fixed_local_offset_minutes(Some(0));

    // 1) Parser-only fixtures (not part of parity gates).
    let mut parser_only_by_diagram: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    let all_fixture_mmds = collect_mmd_files_recursive(&fixtures_root)?;
    for p in all_fixture_mmds {
        let Some(top) = top_level_dir_under(&fixtures_root, &p) else {
            continue;
        };
        if top == "_deferred" || top == "upstream-svgs" {
            continue;
        }
        let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !(name.contains("_parser_only_") || name.contains("_parser_only_spec")) {
            continue;
        }
        if let Some(ref f) = filter {
            if !p.to_string_lossy().contains(f) {
                continue;
            }
        }
        parser_only_by_diagram.entry(top).or_default().push(p);
    }
    for v in parser_only_by_diagram.values_mut() {
        v.sort();
    }

    // 2) Deferred fixtures (mostly expected errors / out-of-scope configs).
    let mut deferred_ok: Vec<DeferredParseOk> = Vec::new();
    let mut deferred_err: Vec<DeferredParseErr> = Vec::new();
    if deferred_root.exists() {
        let deferred_files = collect_mmd_files_recursive(&deferred_root)?;
        for p in deferred_files {
            let Some(expected_group) = top_level_dir_under(&deferred_root, &p) else {
                continue;
            };
            if let Some(ref f) = filter {
                if !p.to_string_lossy().contains(f) {
                    continue;
                }
            }

            let text = read_text_normalized(&p)?;
            match futures::executor::block_on(
                engine.parse_diagram(&text, merman::ParseOptions::default()),
            ) {
                Ok(Some(parsed)) => {
                    deferred_ok.push(DeferredParseOk {
                        path: p,
                        expected_group,
                        diagram_type: parsed.meta.diagram_type.clone(),
                        out_of_scope: detect_out_of_scope(&parsed.meta),
                    });
                }
                Ok(None) => {
                    deferred_err.push(DeferredParseErr {
                        path: p,
                        expected_group,
                        message_key: "no diagram detected".to_string(),
                    });
                }
                Err(err) => {
                    let msg = err.to_string();
                    let key = normalize_error_key(&msg);
                    deferred_err.push(DeferredParseErr {
                        path: p,
                        expected_group,
                        message_key: key,
                    });
                }
            }
        }
    }

    deferred_ok.sort_by(|a, b| a.path.cmp(&b.path));
    deferred_err.sort_by(|a, b| a.path.cmp(&b.path));

    // Render report.
    let mut report = String::new();
    let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let _ = writeln!(&mut report, "# Gap Audit (Mermaid@11.12.3)\n");
    let _ = writeln!(&mut report, "- Generated: `{ts}`");
    let out_rel = out_path.strip_prefix(&workspace_root).unwrap_or(&out_path);
    let _ = write!(
        &mut report,
        "- Command: `cargo run -p xtask -- audit-gaps --out {}",
        out_rel.display()
    );
    if let Some(ref f) = filter {
        let _ = write!(&mut report, " --filter {f}");
    }
    if limit != 60 {
        let _ = write!(&mut report, " --limit {limit}");
    }
    let _ = writeln!(&mut report, "`\n");

    let parser_only_total: usize = parser_only_by_diagram.values().map(|v| v.len()).sum();
    let _ = writeln!(
        &mut report,
        "## Parser-only fixtures\n\nTotal: **{parser_only_total}**\n"
    );
    if parser_only_total == 0 {
        let _ = writeln!(&mut report, "_None found._\n");
    } else {
        for (diagram, files) in &parser_only_by_diagram {
            let _ = writeln!(&mut report, "### `{diagram}` ({})\n", files.len());
            for p in files.iter().take(limit) {
                let rel = p.strip_prefix(&workspace_root).unwrap_or(p);
                let _ = writeln!(&mut report, "- `{}`", rel.display());
            }
            if files.len() > limit {
                let _ = writeln!(
                    &mut report,
                    "- _... {} more omitted (use `--limit` or `--filter`)_",
                    files.len() - limit
                );
            }
            let _ = writeln!(&mut report);
        }
    }

    let _ = writeln!(
        &mut report,
        "## Deferred fixtures\n\n- Root: `fixtures/_deferred`\n- Parse OK: **{}**\n- Parse ERR: **{}**\n",
        deferred_ok.len(),
        deferred_err.len()
    );

    if !deferred_err.is_empty() {
        let mut err_by_group: BTreeMap<String, Vec<&DeferredParseErr>> = BTreeMap::new();
        for e in &deferred_err {
            err_by_group
                .entry(e.expected_group.clone())
                .or_default()
                .push(e);
        }

        let _ = writeln!(&mut report, "### Deferred parse errors (by group)\n");
        for (group, errs) in &err_by_group {
            let _ = writeln!(&mut report, "#### `{group}` ({})\n", errs.len());

            // Cluster by message_key to avoid a wall of text.
            let mut by_key: BTreeMap<String, Vec<&DeferredParseErr>> = BTreeMap::new();
            for e in errs {
                by_key.entry(e.message_key.clone()).or_default().push(*e);
            }
            let mut clusters: Vec<(String, usize)> =
                by_key.iter().map(|(k, v)| (k.clone(), v.len())).collect();
            clusters.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

            for (key, count) in clusters.iter().take(limit.min(20)) {
                let _ = writeln!(&mut report, "- **{count}×** `{key}`");
            }
            if clusters.len() > limit.min(20) {
                let _ = writeln!(
                    &mut report,
                    "- _... {} more clusters omitted_",
                    clusters.len() - limit.min(20)
                );
            }
            let _ = writeln!(&mut report);
        }
    }

    if !deferred_ok.is_empty() {
        let _ = writeln!(&mut report, "### Deferred parse OK (by group)\n");
        let mut ok_by_group: BTreeMap<String, Vec<&DeferredParseOk>> = BTreeMap::new();
        for ok in &deferred_ok {
            ok_by_group
                .entry(ok.expected_group.clone())
                .or_default()
                .push(ok);
        }
        for (group, oks) in &ok_by_group {
            let _ = writeln!(&mut report, "#### `{group}` ({})\n", oks.len());

            let mut out_of_scope_counts: BTreeMap<String, usize> = BTreeMap::new();
            let mut diag_type_counts: BTreeMap<String, usize> = BTreeMap::new();
            for ok in oks {
                *diag_type_counts.entry(ok.diagram_type.clone()).or_default() += 1;
                for flag in &ok.out_of_scope {
                    *out_of_scope_counts.entry(flag.clone()).or_default() += 1;
                }
            }

            if !out_of_scope_counts.is_empty() {
                let _ = writeln!(&mut report, "- Out-of-scope signals:");
                for (k, v) in out_of_scope_counts {
                    let _ = writeln!(&mut report, "  - {v}× `{k}`");
                }
            }

            let _ = writeln!(&mut report, "- Detected diagram types:");
            for (k, v) in diag_type_counts {
                let _ = writeln!(&mut report, "  - {v}× `{k}`");
            }

            let _ = writeln!(&mut report, "\nSample (first {}):\n", limit.min(20));
            for ok in oks.iter().take(limit.min(20)) {
                let rel = ok.path.strip_prefix(&workspace_root).unwrap_or(&ok.path);
                if ok.out_of_scope.is_empty() {
                    let _ = writeln!(
                        &mut report,
                        "- `{}` -> `{}`",
                        rel.display(),
                        ok.diagram_type
                    );
                } else {
                    let _ = writeln!(
                        &mut report,
                        "- `{}` -> `{}` ({})",
                        rel.display(),
                        ok.diagram_type,
                        ok.out_of_scope.join(", ")
                    );
                }
            }
            let _ = writeln!(&mut report);
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

    println!("Wrote audit report: {}", out_path.display());
    Ok(())
}

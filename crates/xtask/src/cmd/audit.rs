//! Gap audits and corpus health checks.

use crate::XtaskError;
use crate::cmd::{
    MmdFixtureScan, collect_mmd_fixtures, ensure_upstream_svg_puppeteer_config,
    spawn_timeout_managed_child, wait_with_timeout,
};
use crate::util::*;
use regex::Regex;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Debug, Clone)]
struct DeferredParseOk {
    path: PathBuf,
    expected_group: String,
    diagram_type: String,
    out_of_scope: Vec<String>,
}

#[derive(Debug, Clone)]
struct AbsorbedDeferredDuplicate {
    path: PathBuf,
    expected_group: String,
    active_path: PathBuf,
    reason: &'static str,
}

#[derive(Debug, Clone)]
struct DeferredParseErr {
    path: PathBuf,
    expected_group: String,
    message_key: String,
}

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
    if meta.diagram_type.ends_with("-elk") {
        out.push("layout=elk".to_string());
    }

    out
}

fn fixture_body_without_frontmatter(path: &Path) -> Option<String> {
    let mut text = fs::read_to_string(path).ok()?;
    if text.contains("\r\n") {
        text = text.replace("\r\n", "\n");
    }

    let body = if let Some(rest) = text.strip_prefix("---\n") {
        match rest.find("\n---\n") {
            Some(end) => &rest[end + "\n---\n".len()..],
            None => text.as_str(),
        }
    } else {
        text.as_str()
    };

    Some(body.trim().to_string())
}

fn body_equivalent_active_fixture(
    fixtures_root: &Path,
    expected_group: &str,
    deferred_path: &Path,
) -> Option<PathBuf> {
    let deferred_body = fixture_body_without_frontmatter(deferred_path)?;
    let active_dir = fixtures_root.join(expected_group);
    if !active_dir.is_dir() {
        return None;
    }

    let mut candidates = collect_mmd_fixtures(
        &active_dir,
        MmdFixtureScan {
            recursive: true,
            skip_private_dirs: true,
            skip_upstream_svgs: true,
            ..MmdFixtureScan::default()
        },
    );
    candidates.sort();

    candidates.into_iter().find(|candidate| {
        fixture_body_without_frontmatter(candidate).is_some_and(|body| body == deferred_body)
    })
}

fn absorbed_deferred_duplicate(
    fixtures_root: &Path,
    deferred_root: &Path,
    path: &Path,
    expected_group: &str,
    meta: &merman::ParseMetadata,
) -> Option<AbsorbedDeferredDuplicate> {
    let rel = path.strip_prefix(deferred_root).ok()?;
    let active_path = fixtures_root.join(rel);

    if expected_group == "flowchart"
        && (meta.diagram_type == "flowchart-elk"
            || meta.config.get_str("layout") == Some("elk")
            || meta.config.get_str("flowchart.defaultRenderer") == Some("elk"))
        && active_path.exists()
    {
        return Some(AbsorbedDeferredDuplicate {
            path: path.to_path_buf(),
            expected_group: expected_group.to_string(),
            active_path,
            reason: "active source-backed Flowchart ELK parity fixture already exists",
        });
    }

    if expected_group == "class"
        && (meta.config.get_str("layout") == Some("elk")
            || meta.config.get_str("class.defaultRenderer") == Some("elk"))
        && let Some(active_path) =
            body_equivalent_active_fixture(fixtures_root, expected_group, path)
    {
        return Some(AbsorbedDeferredDuplicate {
            path: path.to_path_buf(),
            expected_group: expected_group.to_string(),
            active_path,
            reason: "active Class ELK source fixture has the same diagram body",
        });
    }

    None
}

#[derive(Debug, Clone)]
struct UpstreamRenderCheck {
    fixture: String,
    ok: bool,
    error_key: Option<String>,
}

fn extract_upstream_error_key(stderr_text: &str) -> Option<String> {
    // Mermaid CLI logs tend to be noisy (stack traces, repeated "Generating ..." lines).
    // Try to pull out a stable "first meaningful" line.
    for raw in stderr_text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.eq_ignore_ascii_case("Generating single mermaid chart")
            || line.eq_ignore_ascii_case("Generating single mermaid chart.")
        {
            continue;
        }
        return Some(normalize_error_key(line));
    }
    None
}

fn upstream_svg_is_error(svg_text: &str) -> bool {
    // Mermaid error SVGs are still valid SVGs and may be produced with exit status 0.
    // They are distinguished by `aria-roledescription="error"`.
    svg_text.contains(r#"aria-roledescription="error""#)
}

fn extract_upstream_error_key_from_error_svg(svg_text: &str) -> Option<String> {
    if !upstream_svg_is_error(svg_text) {
        return None;
    }

    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"<text[^>]*class="error-text"[^>]*>([^<]+)</text>"#)
            .expect("error svg regex must compile")
    });

    for cap in re.captures_iter(svg_text) {
        let line = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.to_ascii_lowercase().starts_with("mermaid version") {
            continue;
        }
        return Some(normalize_error_key(line));
    }

    Some("rendered error svg".to_string())
}

fn upstream_mmdc_command(
    mmdc: &Path,
    node_cwd: &Path,
    mmd_path: &Path,
    out_path: &Path,
    pinned_config: &Path,
    puppeteer_config: &Path,
    svg_id: &str,
) -> Command {
    let mut command = Command::new("node");
    command
        .arg(mmdc)
        .current_dir(node_cwd)
        .arg("-i")
        .arg(mmd_path)
        .arg("-o")
        .arg(out_path)
        .arg("-t")
        .arg("default")
        .arg("-c")
        .arg(pinned_config)
        .arg("-p")
        .arg(puppeteer_config)
        .arg("--svgId")
        .arg(svg_id);
    command
}

fn check_upstream_renderability_for_parser_only(
    workspace_root: &Path,
    diagram: &str,
    mmd_path: &Path,
    out_root: &Path,
    timeout: Duration,
) -> Result<UpstreamRenderCheck, XtaskError> {
    let fixture_rel = mmd_path
        .strip_prefix(workspace_root)
        .unwrap_or(mmd_path)
        .display()
        .to_string();

    let tools_root = crate::cmd::mermaid_cli_root();
    let mmdc = crate::cmd::validate_mermaid_cli_install(&tools_root)?;

    let node_cwd = tools_root.clone();
    let pinned_config = node_cwd.join("mermaid-config.json");
    let puppeteer_config = ensure_upstream_svg_puppeteer_config()?;

    let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
        return Ok(UpstreamRenderCheck {
            fixture: fixture_rel,
            ok: false,
            error_key: Some("invalid fixture filename".to_string()),
        });
    };
    let svg_id = sanitize_svg_id(stem);

    let out_dir = out_root.join(diagram);
    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;
    let out_path = out_dir.join(format!("{stem}.svg"));
    let log_path = out_dir.join(format!("{stem}.stderr.txt"));

    let mut cmd = upstream_mmdc_command(
        &mmdc,
        &node_cwd,
        mmd_path,
        &out_path,
        &pinned_config,
        &puppeteer_config,
        &svg_id,
    );

    let log_file = fs::File::create(&log_path).map_err(|source| XtaskError::WriteFile {
        path: log_path.display().to_string(),
        source,
    })?;

    cmd.stdout(Stdio::null()).stderr(Stdio::from(log_file));

    let mut child = spawn_timeout_managed_child(&mut cmd)
        .map_err(|err| XtaskError::UpstreamSvgFailed(format!("failed to spawn mmdc: {err}")))?;
    let status = match wait_with_timeout(&mut child, timeout) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
            return Ok(UpstreamRenderCheck {
                fixture: fixture_rel,
                ok: false,
                error_key: Some("timeout".to_string()),
            });
        }
        Err(e) => {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "mmdc execution failed: {e}"
            )));
        }
    };

    if status.success() {
        let mut svg_text = String::new();
        if let Ok(mut f) = fs::File::open(&out_path) {
            let _ = f.read_to_string(&mut svg_text);
        }

        if let Some(key) = extract_upstream_error_key_from_error_svg(&svg_text) {
            Ok(UpstreamRenderCheck {
                fixture: fixture_rel,
                ok: false,
                error_key: Some(key),
            })
        } else {
            let _ = fs::remove_file(&log_path);
            Ok(UpstreamRenderCheck {
                fixture: fixture_rel,
                ok: true,
                error_key: None,
            })
        }
    } else {
        let mut stderr_text = String::new();
        if let Ok(mut f) = fs::File::open(&log_path) {
            let _ = f.read_to_string(&mut stderr_text);
        }
        let key =
            extract_upstream_error_key(&stderr_text).or_else(|| Some(format!("exit={status}")));
        Ok(UpstreamRenderCheck {
            fixture: fixture_rel,
            ok: false,
            error_key: key,
        })
    }
}

pub(crate) fn audit_gaps(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut limit: usize = 60;
    let mut check_upstream_render: bool = false;
    let mut check_upstream_render_deferred_ok: bool = false;
    let mut upstream_timeout_secs: u64 = 60;

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
            "--check-upstream-render" => check_upstream_render = true,
            "--check-upstream-render-deferred-ok" => check_upstream_render_deferred_ok = true,
            "--upstream-timeout-secs" => {
                i += 1;
                upstream_timeout_secs = args
                    .get(i)
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(60);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = crate::cmd::workspace_root();
    let fixtures_root = crate::cmd::fixtures_root();
    let deferred_root = fixtures_root.join("_deferred");
    let baseline_label = crate::cmd::pinned_mermaid_baseline_label(&workspace_root);

    let out_path =
        out_path.unwrap_or_else(|| crate::cmd::target_root().join("audit").join("gaps.md"));

    let engine = merman::Engine::new()
        .with_fixed_today(Some(
            chrono::NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid date"),
        ))
        .with_fixed_local_offset_minutes(Some(0));

    // 1) Parser-only fixtures (not part of parity gates).
    let mut parser_only_by_diagram: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    let all_fixture_mmds = collect_mmd_fixtures(
        &fixtures_root,
        MmdFixtureScan {
            recursive: true,
            ..MmdFixtureScan::default()
        },
    );
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
        if let Some(ref f) = filter
            && !p.to_string_lossy().contains(f)
        {
            continue;
        }
        parser_only_by_diagram.entry(top).or_default().push(p);
    }
    for v in parser_only_by_diagram.values_mut() {
        v.sort();
    }

    // 2) Deferred fixtures (mostly expected errors / out-of-scope configs).
    let mut absorbed_deferred_duplicates: Vec<AbsorbedDeferredDuplicate> = Vec::new();
    let mut deferred_ok: Vec<DeferredParseOk> = Vec::new();
    let mut deferred_err: Vec<DeferredParseErr> = Vec::new();
    if deferred_root.exists() {
        let deferred_files = collect_mmd_fixtures(
            &deferred_root,
            MmdFixtureScan {
                recursive: true,
                ..MmdFixtureScan::default()
            },
        );
        for p in deferred_files {
            let Some(expected_group) = top_level_dir_under(&deferred_root, &p) else {
                continue;
            };
            if let Some(ref f) = filter
                && !p.to_string_lossy().contains(f)
            {
                continue;
            }

            // Do NOT `trim_end()` here: some upstream grammars treat trailing whitespace-only
            // lines as a syntax error (e.g. Treemap at Mermaid CLI @11.12.3).
            let mut text = read_text(&p)?;
            if text.contains("\r\n") {
                text = text.replace("\r\n", "\n");
            }
            match futures::executor::block_on(
                engine.parse_diagram(&text, merman::ParseOptions::default()),
            ) {
                Ok(Some(parsed)) => {
                    if let Some(absorbed) = absorbed_deferred_duplicate(
                        &fixtures_root,
                        &deferred_root,
                        &p,
                        &expected_group,
                        &parsed.meta,
                    ) {
                        absorbed_deferred_duplicates.push(absorbed);
                        continue;
                    }
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

    absorbed_deferred_duplicates.sort_by(|a, b| a.path.cmp(&b.path));
    deferred_ok.sort_by(|a, b| a.path.cmp(&b.path));
    deferred_err.sort_by(|a, b| a.path.cmp(&b.path));

    // Render report.
    let mut report = String::new();
    let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let _ = writeln!(&mut report, "# Gap Audit (Mermaid{baseline_label})\n");
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
    if check_upstream_render {
        let _ = write!(&mut report, " --check-upstream-render");
    }
    if check_upstream_render_deferred_ok {
        let _ = write!(&mut report, " --check-upstream-render-deferred-ok");
    }
    if upstream_timeout_secs != 60 {
        let _ = write!(
            &mut report,
            " --upstream-timeout-secs {upstream_timeout_secs}"
        );
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

    if check_upstream_render && parser_only_total > 0 {
        let timeout = Duration::from_secs(upstream_timeout_secs.max(1));
        let out_root = crate::cmd::target_root()
            .join("audit")
            .join("upstream-render");
        let out_root_rel = out_root.strip_prefix(&workspace_root).unwrap_or(&out_root);

        let _ = writeln!(
            &mut report,
            "## Upstream renderability (parser-only)\n\n- Tool: Mermaid CLI (`tools/mermaid-cli`)\n- Timeout: `{}` seconds per chart\n- Output: `{}`\n",
            upstream_timeout_secs,
            out_root_rel.display()
        );

        let mut results_by_diagram: BTreeMap<String, Vec<UpstreamRenderCheck>> = BTreeMap::new();
        let mut failures_by_diagram: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();

        for (diagram, files) in &parser_only_by_diagram {
            for p in files {
                let res = check_upstream_renderability_for_parser_only(
                    &workspace_root,
                    diagram,
                    p,
                    &out_root,
                    timeout,
                )?;
                results_by_diagram
                    .entry(diagram.clone())
                    .or_default()
                    .push(res.clone());
                if !res.ok {
                    let key = res
                        .error_key
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());
                    *failures_by_diagram
                        .entry(diagram.clone())
                        .or_default()
                        .entry(key)
                        .or_default() += 1;
                }
            }
        }

        let mut actionable: Vec<(String, String)> = Vec::new();
        for (diagram, results) in &results_by_diagram {
            for r in results.iter().filter(|r| r.ok) {
                actionable.push((diagram.clone(), r.fixture.clone()));
            }
        }
        actionable.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

        let _ = writeln!(
            &mut report,
            "### Actionable gaps\n\nThese are parser-only fixtures that upstream Mermaid CLI can render successfully.\nThey indicate missing or intentionally-deferred renderer parity on the merman side.\n\nTotal: **{}**\n",
            actionable.len()
        );
        if actionable.is_empty() {
            let _ = writeln!(&mut report, "_None._\n");
        } else {
            for (diagram, fixture) in actionable.iter().take(limit) {
                let _ = writeln!(&mut report, "- `{diagram}`: `{fixture}`");
            }
            if actionable.len() > limit {
                let _ = writeln!(
                    &mut report,
                    "- _... {} more omitted (use `--limit` or `--filter`)_\n",
                    actionable.len() - limit
                );
            } else {
                let _ = writeln!(&mut report);
            }
        }

        for (diagram, results) in &results_by_diagram {
            let ok_count = results.iter().filter(|r| r.ok).count();
            let fail_count = results.len() - ok_count;
            let _ = writeln!(
                &mut report,
                "### `{diagram}`\n\n- Render OK: **{ok_count}**\n- Render FAIL: **{fail_count}**\n"
            );

            if ok_count > 0 {
                let _ = writeln!(&mut report, "Render OK fixtures:\n");
                for r in results.iter().filter(|r| r.ok).take(limit) {
                    let _ = writeln!(&mut report, "- `{}`", r.fixture);
                }
                if ok_count > limit {
                    let _ = writeln!(&mut report, "- _... {} more omitted_", ok_count - limit);
                }
                let _ = writeln!(&mut report);
            }

            if let Some(fails) = failures_by_diagram.get(diagram) {
                let mut clusters: Vec<(String, usize)> =
                    fails.iter().map(|(k, v)| (k.clone(), *v)).collect();
                clusters.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                let _ = writeln!(&mut report, "Failures (by key):\n");
                for (k, n) in clusters.iter().take(limit.min(20)) {
                    let _ = writeln!(&mut report, "- **{n}×** `{k}`");
                }
                if clusters.len() > limit.min(20) {
                    let _ = writeln!(
                        &mut report,
                        "- _... {} more clusters omitted_",
                        clusters.len() - limit.min(20)
                    );
                }
                let _ = writeln!(&mut report, "\nFailure fixtures:\n");
                let mut fail_rows: Vec<(String, String)> = results
                    .iter()
                    .filter(|r| !r.ok)
                    .map(|r| {
                        (
                            r.fixture.clone(),
                            r.error_key.clone().unwrap_or_else(|| "unknown".to_string()),
                        )
                    })
                    .collect();
                fail_rows.sort_by(|a, b| a.0.cmp(&b.0));
                for (fixture, key) in fail_rows.iter().take(limit) {
                    let _ = writeln!(&mut report, "- `{fixture}`: `{key}`");
                }
                if fail_rows.len() > limit {
                    let _ = writeln!(
                        &mut report,
                        "- _... {} more omitted_",
                        fail_rows.len() - limit
                    );
                }
                let _ = writeln!(&mut report);
            }
        }
    }

    let _ = writeln!(
        &mut report,
        "## Deferred fixtures\n\n- Root: `fixtures/_deferred`\n- Absorbed duplicate fixtures: **{}**\n- Parse OK: **{}**\n- Parse ERR: **{}**\n",
        absorbed_deferred_duplicates.len(),
        deferred_ok.len(),
        deferred_err.len()
    );

    if !absorbed_deferred_duplicates.is_empty() {
        let mut absorbed_by_group: BTreeMap<String, Vec<&AbsorbedDeferredDuplicate>> =
            BTreeMap::new();
        for absorbed in &absorbed_deferred_duplicates {
            absorbed_by_group
                .entry(absorbed.expected_group.clone())
                .or_default()
                .push(absorbed);
        }

        let _ = writeln!(
            &mut report,
            "### Absorbed deferred duplicates\n\nThese deferred files have matching active fixtures and are not counted as current gaps.\n"
        );
        for (group, absorbed) in &absorbed_by_group {
            let _ = writeln!(&mut report, "#### `{group}` ({})\n", absorbed.len());
            for row in absorbed.iter().take(limit.min(20)) {
                let rel = row.path.strip_prefix(&workspace_root).unwrap_or(&row.path);
                let active_rel = row
                    .active_path
                    .strip_prefix(&workspace_root)
                    .unwrap_or(&row.active_path);
                let _ = writeln!(
                    &mut report,
                    "- `{}` -> `{}` ({})",
                    rel.display(),
                    active_rel.display(),
                    row.reason
                );
            }
            if absorbed.len() > limit.min(20) {
                let _ = writeln!(
                    &mut report,
                    "- _... {} more omitted (use `--limit` or `--filter`)_",
                    absorbed.len() - limit.min(20)
                );
            }
            let _ = writeln!(&mut report);
        }
    }

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

            let in_scope: Vec<&DeferredParseOk> = oks
                .iter()
                .copied()
                .filter(|ok| ok.out_of_scope.is_empty())
                .collect();

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

            let _ = writeln!(
                &mut report,
                "- In-scope fixtures (no `layout`/`look`): **{}**\n",
                in_scope.len()
            );

            if !in_scope.is_empty() {
                let _ = writeln!(&mut report, "Sample in-scope (first {}):\n", limit.min(20));
                for ok in in_scope.iter().take(limit.min(20)) {
                    let rel = ok.path.strip_prefix(&workspace_root).unwrap_or(&ok.path);
                    let _ = writeln!(
                        &mut report,
                        "- `{}` -> `{}`",
                        rel.display(),
                        ok.diagram_type
                    );
                }
            } else {
                let _ = writeln!(&mut report, "Sample (first {}):\n", limit.min(20));
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
            }
            let _ = writeln!(&mut report);
        }
    }

    if check_upstream_render_deferred_ok && !deferred_ok.is_empty() {
        let timeout = Duration::from_secs(upstream_timeout_secs.max(1));
        let out_root = crate::cmd::target_root()
            .join("audit")
            .join("upstream-render-deferred-ok");
        let out_root_rel = out_root.strip_prefix(&workspace_root).unwrap_or(&out_root);

        let _ = writeln!(
            &mut report,
            "### Upstream renderability (deferred parse OK)\n\n- Tool: Mermaid CLI (`tools/mermaid-cli`)\n- Timeout: `{}` seconds per chart\n- Output: `{}`\n",
            upstream_timeout_secs,
            out_root_rel.display()
        );

        let mut results_by_group: BTreeMap<String, Vec<UpstreamRenderCheck>> = BTreeMap::new();
        let mut failures_by_group: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
        let mut promotable: Vec<String> = Vec::new();

        for ok in &deferred_ok {
            let res = check_upstream_renderability_for_parser_only(
                &workspace_root,
                &ok.expected_group,
                &ok.path,
                &out_root,
                timeout,
            )?;
            results_by_group
                .entry(ok.expected_group.clone())
                .or_default()
                .push(res.clone());

            if res.ok && ok.out_of_scope.is_empty() {
                promotable.push(res.fixture.clone());
            }

            if !res.ok {
                let key = res
                    .error_key
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string());
                *failures_by_group
                    .entry(ok.expected_group.clone())
                    .or_default()
                    .entry(key)
                    .or_default() += 1;
            }
        }

        promotable.sort();

        let _ = writeln!(
            &mut report,
            "Promotable candidates (in-scope + upstream renders OK): **{}**\n",
            promotable.len()
        );
        if promotable.is_empty() {
            let _ = writeln!(&mut report, "_None._\n");
        } else {
            for f in promotable.iter().take(limit) {
                let _ = writeln!(&mut report, "- `{f}`");
            }
            if promotable.len() > limit {
                let _ = writeln!(
                    &mut report,
                    "- _... {} more omitted (use `--limit`)_",
                    promotable.len() - limit
                );
            }
            let _ = writeln!(&mut report);
        }

        for (group, results) in &results_by_group {
            let ok_count = results.iter().filter(|r| r.ok).count();
            let fail_count = results.len() - ok_count;
            let _ = writeln!(
                &mut report,
                "#### `{group}`\n\n- Render OK: **{ok_count}**\n- Render FAIL: **{fail_count}**\n"
            );

            if let Some(fails) = failures_by_group.get(group) {
                let mut clusters: Vec<(String, usize)> =
                    fails.iter().map(|(k, v)| (k.clone(), *v)).collect();
                clusters.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                let _ = writeln!(&mut report, "Failures (by key):\n");
                for (k, n) in clusters.iter().take(limit.min(20)) {
                    let _ = writeln!(&mut report, "- **{n}×** `{k}`");
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn parse_meta(text: &str) -> merman::ParseMetadata {
        let parsed = futures::executor::block_on(
            merman::Engine::new().parse_diagram(text, merman::ParseOptions::default()),
        )
        .expect("parse should succeed")
        .expect("diagram should be detected");
        parsed.meta
    }

    #[test]
    fn upstream_audit_mmdc_command_passes_the_managed_puppeteer_config() {
        let mmdc = Path::new("tools/mermaid-cli/mmdc.js");
        let node_cwd = Path::new("tools/mermaid-cli");
        let input = Path::new("fixtures/flowchart/basic.mmd");
        let output = Path::new("target/audit/basic.svg");
        let mermaid_config = Path::new("tools/mermaid-cli/mermaid-config.json");
        let puppeteer_config = Path::new("target/xtask-js/puppeteer.json");
        let command = upstream_mmdc_command(
            mmdc,
            node_cwd,
            input,
            output,
            mermaid_config,
            puppeteer_config,
            "basic",
        );
        let args: Vec<_> = command.get_args().collect();
        let puppeteer_arg = args
            .iter()
            .position(|arg| *arg == std::ffi::OsStr::new("-p"))
            .expect("mmdc command must pass a Puppeteer config");

        assert_eq!(command.get_program(), std::ffi::OsStr::new("node"));
        assert_eq!(args.first(), Some(&mmdc.as_os_str()));
        assert_eq!(
            args.get(puppeteer_arg + 1),
            Some(&puppeteer_config.as_os_str())
        );
        assert_eq!(command.get_current_dir(), Some(node_cwd));
    }

    #[test]
    fn absorbed_deferred_duplicate_recognizes_admitted_flowchart_elk_copy() {
        let root = crate::cmd::target_root()
            .join("xtask-tests")
            .join("audit_absorbed_deferred_duplicate");
        let fixtures_root = root.join("fixtures");
        let deferred_root = fixtures_root.join("_deferred");
        let stem = "upstream_html_demos_flowchart_elk_flowchart_elk_001";
        let active_path = fixtures_root.join("flowchart").join(format!("{stem}.mmd"));
        let deferred_path = deferred_root.join("flowchart").join(format!("{stem}.mmd"));

        fs::create_dir_all(active_path.parent().expect("active parent")).expect("active dir");
        fs::create_dir_all(deferred_path.parent().expect("deferred parent")).expect("deferred dir");
        fs::write(&active_path, "flowchart-elk\n  A-->B\n").expect("active fixture");
        fs::write(&deferred_path, "flowchart-elk\n  A-->B\n").expect("deferred fixture");

        let meta = parse_meta("flowchart-elk\n  A-->B\n");

        let absorbed = absorbed_deferred_duplicate(
            &fixtures_root,
            &deferred_root,
            &deferred_path,
            "flowchart",
            &meta,
        )
        .expect("admitted Flowchart ELK duplicate should be absorbed");

        assert_eq!(absorbed.active_path, active_path);
        assert_eq!(
            absorbed.reason,
            "active source-backed Flowchart ELK parity fixture already exists"
        );
    }

    #[test]
    fn absorbed_deferred_duplicate_rejects_unknown_flowchart_elk_copy() {
        let fixtures_root = Path::new("fixtures");
        let deferred_root = fixtures_root.join("_deferred");
        let path = deferred_root
            .join("flowchart")
            .join("not_admitted_flowchart_elk.mmd");
        let meta = parse_meta("flowchart-elk\n  A-->B\n");

        assert!(
            absorbed_deferred_duplicate(fixtures_root, &deferred_root, &path, "flowchart", &meta)
                .is_none()
        );
    }

    #[test]
    fn absorbed_deferred_duplicate_recognizes_class_elk_body_equivalent_copy() {
        let root = crate::cmd::target_root()
            .join("xtask-tests")
            .join("audit_absorbed_class_elk_body_duplicate");
        let fixtures_root = root.join("fixtures");
        let deferred_root = fixtures_root.join("_deferred");
        let active_path = fixtures_root.join("class").join("active_class_elk.mmd");
        let deferred_path = deferred_root.join("class").join("deferred_class_elk.mmd");

        fs::create_dir_all(active_path.parent().expect("active parent")).expect("active dir");
        fs::create_dir_all(deferred_path.parent().expect("deferred parent")).expect("deferred dir");
        fs::write(&active_path, "classDiagram\nA <|-- B\n").expect("active fixture");
        fs::write(
            &deferred_path,
            "---\nconfig:\n  layout: elk\n---\nclassDiagram\nA <|-- B\n",
        )
        .expect("deferred fixture");

        let meta = parse_meta("---\nconfig:\n  layout: elk\n---\nclassDiagram\nA <|-- B\n");

        let absorbed = absorbed_deferred_duplicate(
            &fixtures_root,
            &deferred_root,
            &deferred_path,
            "class",
            &meta,
        )
        .expect("body-equivalent Class ELK duplicate should be absorbed");

        assert_eq!(absorbed.active_path, active_path);
        assert_eq!(
            absorbed.reason,
            "active Class ELK source fixture has the same diagram body"
        );
    }
}

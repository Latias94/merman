use super::*;

fn xychart_has_renderable_plot(body: &str) -> bool {
    fn plot_data_is_renderable(rest: &str) -> bool {
        let Some(open) = rest.find('[') else {
            return false;
        };
        let after_open = &rest[open + 1..];
        if after_open.contains('[') {
            return false;
        }

        let Some(close_rel) = after_open.find(']') else {
            return false;
        };
        let data = after_open[..close_rel].trim();
        let trailing = after_open[close_rel + 1..].trim();
        if data.is_empty() || !(trailing.is_empty() || trailing.starts_with(';')) {
            return false;
        }

        data.split(',')
            .map(str::trim)
            .all(|token| !token.is_empty() && token.parse::<f64>().is_ok())
    }

    body.lines().any(|raw| {
        let line = raw.trim_start();
        let lower = line.to_ascii_lowercase();
        let Some(rest) = lower
            .strip_prefix("line")
            .or_else(|| lower.strip_prefix("bar"))
        else {
            return false;
        };

        (rest.starts_with('[') || rest.starts_with(char::is_whitespace) || rest.starts_with('"'))
            && plot_data_is_renderable(rest)
    })
}

fn class_has_whitespace_only_text_label(body: &str) -> bool {
    body.lines().any(|raw| {
        let line = raw.trim_start();
        if !line.starts_with("class ") {
            return false;
        }

        let Some(open) = line.find("[\"") else {
            return false;
        };
        let label_start = open + 2;
        let Some(close_rel) = line[label_start..].find("\"]") else {
            return false;
        };

        let label = &line[label_start..label_start + close_rel];
        !label.is_empty() && label.trim().is_empty()
    })
}

fn strip_yaml_frontmatter_for_header(body: &str) -> &str {
    let mut lines = body.lines();
    let Some(first) = lines.next() else {
        return body;
    };
    if first.trim() != "---" {
        return body;
    }

    let mut consumed = first.len() + 1;
    for line in lines {
        consumed += line.len() + 1;
        if line.trim() == "---" {
            break;
        }
    }
    body.get(consumed..).unwrap_or("")
}

fn has_valid_gitgraph_header(body: &str) -> bool {
    for raw in strip_yaml_frontmatter_for_header(body).lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }

        let lower = line.to_ascii_lowercase();
        let Some(rest) = lower.strip_prefix("gitgraph") else {
            return false;
        };
        let rest = rest.trim();
        if rest.is_empty() || rest == ":" {
            return true;
        }

        let direction = rest.strip_suffix(':').unwrap_or(rest).trim();
        return matches!(direction, "tb" | "bt" | "lr");
    }
    false
}

fn extract_javascript_fixture_literals(
    source: &str,
) -> Result<Vec<crate::cmd::javascript_source::StaticStringExpression>, &'static str> {
    crate::cmd::javascript_source::extract_package_test_strings(source)
}

pub(crate) fn import_upstream_pkg_tests(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;
    let mut limit: Option<usize> = None;
    let mut min_lines: Option<usize> = None;
    let mut prefer_complex: bool = false;
    let mut overwrite: bool = false;
    let mut with_baselines: bool = false;
    let mut install: bool = false;
    let mut src_root: Option<PathBuf> = None;

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
            "--src-root" => {
                i += 1;
                let raw = args.get(i).ok_or(XtaskError::Usage)?;
                src_root = Some(PathBuf::from(raw));
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = crate::cmd::workspace_root();

    let default_src_root = crate::cmd::mermaid_repo_root()
        .join("packages")
        .join("mermaid")
        .join("src");
    let src_root = src_root
        .map(|p| {
            if p.is_absolute() {
                p
            } else {
                workspace_root.join(p)
            }
        })
        .unwrap_or(default_src_root);
    if !src_root.is_dir() {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "upstream package src root not found: {} (expected repo-ref checkout of the pinned Mermaid baseline)",
            src_root.display()
        )));
    }

    if install && !with_baselines {
        return Err(XtaskError::SnapshotUpdateFailed(
            "`--install` only applies when `--with-baselines` is set".to_string(),
        ));
    }

    fn canonical_fixture_text(s: &str) -> String {
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        let s = s.trim_matches('\n');
        format!("{s}\n")
    }

    fn strip_yaml_frontmatter(body: &str) -> &str {
        // Keep parity with other importers: Mermaid fixtures occasionally start with YAML
        // frontmatter, so directive detection must ignore it.
        let mut lines = body.lines();
        let Some(first) = lines.next() else {
            return body;
        };
        if first.trim() != "---" {
            return body;
        }
        let mut consumed = first.len() + 1;
        for l in lines {
            consumed += l.len() + 1;
            if l.trim() == "---" {
                break;
            }
        }
        body.get(consumed..).unwrap_or("")
    }

    fn has_any_directive(body: &str, directives: &[&str]) -> bool {
        let body = strip_yaml_frontmatter(body);
        let mut seen = 0usize;
        for raw in body.lines() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            let lower = line.to_ascii_lowercase();
            if lower.starts_with("%%{init") || lower.starts_with("%%") {
                // Allow a small amount of init/comment prelude.
                seen += 1;
                if seen > 25 {
                    break;
                }
                continue;
            }
            for d in directives {
                if lower.starts_with(&d.to_ascii_lowercase()) {
                    return true;
                }
            }
            seen += 1;
            if seen > 25 {
                break;
            }
        }
        false
    }

    fn looks_like_mermaid_diagram(diagram_dir: &str, body: &str) -> bool {
        // The detector registry can be permissive (e.g. "architecture ..." matches the
        // Architecture detector even though Mermaid requires `architecture-beta`).
        //
        // Filter false positives early so `--with-baselines` doesn't churn `_deferred/` or spam
        // upstream baseline generation with invalid inputs.
        match diagram_dir {
            "flowchart" => has_any_directive(body, &["flowchart", "graph"]),
            "sequence" => has_any_directive(body, &["sequencediagram"]),
            "class" => has_any_directive(body, &["classdiagram"]),
            "state" => has_any_directive(body, &["statediagram"]),
            "er" => has_any_directive(body, &["erdiagram"]),
            "gantt" => has_any_directive(body, &["gantt"]),
            "journey" => has_any_directive(body, &["journey"]),
            "pie" => has_any_directive(body, &["pie"]),
            "mindmap" => has_any_directive(body, &["mindmap"]),
            "timeline" => has_any_directive(body, &["timeline"]),
            "gitgraph" => has_valid_gitgraph_header(body),
            "sankey" => has_any_directive(body, &["sankey"]),
            "packet" => has_any_directive(body, &["packet"]),
            "treemap" => has_any_directive(body, &["treemap"]),
            "radar" => has_any_directive(body, &["radar"]),
            "xychart" => has_any_directive(body, &["xychart"]),
            "quadrantchart" => has_any_directive(body, &["quadrantchart"]),
            "requirement" => has_any_directive(body, &["requirementdiagram"]),
            "architecture" => has_any_directive(body, &["architecture-beta"]),
            "block" => has_any_directive(body, &["block"]),
            "c4" => has_any_directive(
                body,
                &[
                    "c4context",
                    "c4container",
                    "c4component",
                    "c4deployment",
                    "c4dynamic",
                ],
            ),
            "info" => has_any_directive(body, &["info"]),
            _ => true,
        }
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

    fn collect_test_files_recursively(
        root: &Path,
        out: &mut Vec<PathBuf>,
    ) -> Result<(), XtaskError> {
        if root.is_file() {
            let name = root
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if (name.ends_with(".spec.ts")
                || name.ends_with(".spec.js")
                || name.ends_with(".test.ts")
                || name.ends_with(".test.js"))
                && !name.contains(".d.ts")
            {
                out.push(root.to_path_buf());
            }
            return Ok(());
        }

        let entries = fs::read_dir(root).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to list upstream src directory {}: {err}",
                root.display()
            ))
        })?;
        for entry in entries {
            let path = entry
                .map_err(|err| {
                    XtaskError::SnapshotUpdateFailed(format!(
                        "failed to read upstream src directory entry under {}: {err}",
                        root.display()
                    ))
                })?
                .path();
            if path.is_dir() {
                let dir_name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                if dir_name == "node_modules" || dir_name == "dist" || dir_name == "target" {
                    continue;
                }
                collect_test_files_recursively(&path, out)?;
            } else {
                collect_test_files_recursively(&path, out)?;
            }
        }
        Ok(())
    }

    fn complexity_score(body: &str) -> i64 {
        let line_count = body.lines().count() as i64;
        (line_count * 1_000) + (body.len() as i64)
    }

    #[derive(Debug, Clone)]
    struct Candidate {
        source_path: PathBuf,
        idx_in_file: usize,
        diagram_dir: String,
        stem: String,
        body: String,
        score: i64,
    }

    let reg = merman::detect::DetectorRegistry::pinned_mermaid_baseline();

    let mut spec_files: Vec<PathBuf> = Vec::new();
    collect_test_files_recursively(&src_root, &mut spec_files)?;
    spec_files.sort();

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for spec_path in spec_files {
        let hay = spec_path.to_string_lossy();
        if let Some(f) = filter.as_deref()
            && !hay.contains(f)
        {
            // Still allow matching by diagram heading later; template strings have no heading here.
            continue;
        }

        let text = match fs::read_to_string(&spec_path) {
            Ok(v) => v,
            Err(err) => {
                skipped.push(format!(
                    "skip (read failed): {} ({err})",
                    spec_path.display()
                ));
                continue;
            }
        };
        let blocks = match extract_javascript_fixture_literals(&text) {
            Ok(blocks) => blocks,
            Err(reason) => {
                skipped.push(format!(
                    "skip (TypeScript parse failed): {} ({reason})",
                    spec_path.display()
                ));
                continue;
            }
        };
        if blocks.is_empty() {
            continue;
        }

        let source_stem = spec_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let source_slug = clamp_slug(slugify(&source_stem), 48);

        for expression in blocks {
            let idx = expression.source_ordinal;
            let body = canonical_fixture_text(&expression.value);
            if body.trim().is_empty() {
                continue;
            }
            if let Some(min) = min_lines
                && body.lines().count() < min
            {
                continue;
            }

            let mut cfg = merman::MermaidConfig::default();
            let detected = match reg.detect_type(body.as_str(), &mut cfg) {
                Ok(t) => t,
                Err(_) => continue,
            };
            let Some(diagram_dir) = normalize_imported_diagram_dir(detected).map(str::to_string)
            else {
                continue;
            };
            if !looks_like_mermaid_diagram(diagram_dir.as_str(), body.as_str()) {
                continue;
            }
            if diagram_dir == "zenuml" {
                continue;
            }
            if diagram != "all" && diagram_dir != diagram {
                continue;
            }
            if with_baselines && diagram_dir == "xychart" && !xychart_has_renderable_plot(&body) {
                skipped.push(format!(
                    "skip (xychart parser-only without renderable plot): {} (idx={})",
                    spec_path.display(),
                    idx + 1
                ));
                continue;
            }
            if with_baselines
                && diagram_dir == "class"
                && class_has_whitespace_only_text_label(&body)
            {
                skipped.push(format!(
                    "skip (class parser-only whitespace text label): {} (idx={})",
                    spec_path.display(),
                    idx + 1
                ));
                continue;
            }

            let content_id = imported_fixture_content_id(&body);
            let stem = format!("upstream_pkgtests_{source_slug}_{content_id}");
            candidates.push(Candidate {
                source_path: spec_path.clone(),
                idx_in_file: idx,
                diagram_dir: diagram_dir.clone(),
                stem,
                score: complexity_score(&body),
                body,
            });
        }
    }

    if prefer_complex {
        candidates.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.stem.cmp(&b.stem)));
    } else {
        candidates.sort_by(|a, b| a.stem.cmp(&b.stem));
    }

    if candidates.is_empty() {
        return Err(XtaskError::SnapshotUpdateFailed(
            "no candidate template/string literals were detected (use --filter, or check repo-ref/mermaid checkout)"
                .to_string(),
        ));
    }

    let mut existing_by_diagram: std::collections::HashMap<
        String,
        std::collections::HashMap<String, PathBuf>,
    > = std::collections::HashMap::new();

    #[derive(Debug)]
    struct CreatedFixture {
        diagram_dir: String,
        stem: String,
        path: PathBuf,
        rollback: Option<ImportedFixtureSnapshot>,
    }

    fn is_upstream_error_svg(svg_path: &Path) -> Result<bool, XtaskError> {
        let svg = fs::read_to_string(svg_path).map_err(|source| XtaskError::ReadFile {
            path: svg_path.display().to_string(),
            source,
        })?;
        Ok(svg.contains("aria-roledescription=\"error\""))
    }

    fn reject_fixture(f: &CreatedFixture) -> Result<(), XtaskError> {
        reject_imported_fixture_transaction(&f.diagram_dir, &f.stem, &f.path, f.rollback.as_ref())
    }

    fn defer_fixture(
        f: &CreatedFixture,
        keep_upstream_svg: bool,
        replace_existing: bool,
    ) -> Result<PathBuf, XtaskError> {
        defer_imported_fixture_transaction(
            &f.diagram_dir,
            &f.stem,
            &f.path,
            f.rollback.as_ref(),
            keep_upstream_svg,
            replace_existing,
        )
    }

    let mut created: Vec<CreatedFixture> = Vec::new();

    let mut imported = 0usize;
    let workspace_lock = acquire_imported_fixture_workspace_lock()?;
    let _non_baseline_family_locks = if with_baselines {
        None
    } else {
        Some(acquire_imported_fixture_family_locks(
            &workspace_lock,
            candidates
                .iter()
                .map(|candidate| candidate.diagram_dir.as_str()),
        )?)
    };
    for c in candidates {
        if imported > 0 && limit.is_some_and(|max| imported >= max) {
            break;
        }

        let fixtures_dir = crate::cmd::fixtures_root().join(&c.diagram_dir);
        if !fixtures_dir.is_dir() {
            skipped.push(format!(
                "skip (fixtures dir missing): {}",
                fixtures_dir.display()
            ));
            continue;
        }

        let existing = existing_by_diagram
            .entry(c.diagram_dir.clone())
            .or_insert_with(|| {
                load_existing_imported_fixtures(
                    &workspace_lock,
                    &fixtures_dir,
                    &c.diagram_dir,
                    canonical_fixture_text,
                )
            });
        let existing_path = existing.get(&c.body).cloned();
        let out_path = fixtures_dir.join(format!("{}.mmd", c.stem));
        let deferred_out_path = crate::cmd::fixtures_root()
            .join("_deferred")
            .join(&c.diagram_dir)
            .join(format!("{}.mmd", c.stem));
        // An explicit overwrite belongs to the source-addressed normal fixture. Deferred
        // fixtures still require baseline revalidation before being promoted.
        let source_addressed_overwrite = source_addressed_active_overwrite(overwrite, &out_path);
        if let Some(existing_path) = existing_path.as_deref()
            && !source_addressed_overwrite
            && !should_revalidate_deferred_fixture(
                existing_path,
                &deferred_out_path,
                with_baselines,
                overwrite,
            )
        {
            skipped.push(format!(
                "skip (duplicate content): {} (idx={}) -> {}",
                c.source_path.display(),
                c.idx_in_file + 1,
                existing_path.display()
            ));
            continue;
        }

        if out_path.exists() && !overwrite {
            skipped.push(format!("skip (exists): {}", out_path.display()));
            continue;
        }
        if deferred_out_path.exists() && !overwrite {
            skipped.push(format!(
                "skip (already deferred): {}",
                deferred_out_path.display()
            ));
            continue;
        }

        let transaction_locks = if with_baselines {
            Some(acquire_imported_fixture_transaction_locks(
                &workspace_lock,
                &c.diagram_dir,
            )?)
        } else {
            None
        };
        if with_baselines && !overwrite && (out_path.exists() || deferred_out_path.exists()) {
            skipped.push(format!(
                "skip (candidate appeared while waiting for import lock): {}",
                if out_path.exists() {
                    out_path.display()
                } else {
                    deferred_out_path.display()
                }
            ));
            continue;
        }
        let rollback = if with_baselines {
            Some(ImportedFixtureSnapshot::capture(
                &c.diagram_dir,
                &c.stem,
                &out_path,
            )?)
        } else {
            None
        };
        if let Err(error) = write_imported_fixture(&c.diagram_dir, &c.stem, &out_path, &c.body) {
            return Err(rollback_imported_fixture_snapshots(error, rollback.iter()));
        }
        if with_baselines
            && let Err(error) =
                validate_exact_import_candidate_filter(&c.diagram_dir, &c.stem, &out_path)
        {
            return Err(rollback_imported_fixture_snapshots(error, rollback.iter()));
        }

        let mut f = CreatedFixture {
            diagram_dir: c.diagram_dir,
            stem: c.stem,
            path: out_path,
            rollback,
        };

        imported += 1;
        if !with_baselines {
            record_imported_fixture_content(
                existing,
                c.body,
                f.path.clone(),
                &[f.path.as_path(), deferred_out_path.as_path()],
            );
            created.push(f);
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
        if let Err(error) = super::super::gen_upstream_svgs_with_transaction_locks(
            svg_args,
            transaction_locks
                .as_ref()
                .expect("baseline import transaction must hold a family lock")
                .family_lock(),
            transaction_locks
                .as_ref()
                .expect("baseline import transaction must hold a toolchain lock")
                .toolchain_lock(),
        ) {
            let message = match candidate_upstream_svg_failure(error, &f.path) {
                Ok(message) => message,
                Err(error) => {
                    return Err(rollback_imported_fixture_snapshots(
                        error,
                        f.rollback.iter(),
                    ));
                }
            };
            skipped.push(format!(
                "defer (upstream svg generation failed): {} ({message})",
                f.path.display()
            ));
            let deferred_path = defer_fixture(&f, false, overwrite)?;
            record_imported_fixture_content(
                existing,
                c.body,
                deferred_path,
                &[f.path.as_path(), deferred_out_path.as_path()],
            );
            continue;
        }

        let svg_path = crate::cmd::fixtures_root()
            .join("upstream-svgs")
            .join(&f.diagram_dir)
            .join(format!("{}.svg", f.stem));
        if is_upstream_error_svg(&svg_path)
            .map_err(|error| rollback_imported_fixture_snapshots(error, f.rollback.iter()))?
        {
            skipped.push(format!(
                "defer (upstream rendered error diagram): {}",
                f.path.display()
            ));
            let deferred_path = defer_fixture(&f, true, overwrite)?;
            record_imported_fixture_content(
                existing,
                c.body,
                deferred_path,
                &[f.path.as_path(), deferred_out_path.as_path()],
            );
            continue;
        }

        if let Err(error) = super::super::update_snapshots(vec![
            "--diagram".to_string(),
            f.diagram_dir.clone(),
            "--filter".to_string(),
            f.stem.clone(),
        ]) {
            let message = match candidate_snapshot_failure(error, &f.path) {
                Ok(message) => message,
                Err(error) => {
                    return Err(rollback_imported_fixture_snapshots(
                        error,
                        f.rollback.iter(),
                    ));
                }
            };
            skipped.push(format!(
                "skip (snapshot update failed): {} ({message})",
                f.path.display()
            ));
            reject_fixture(&f)?;
            continue;
        }

        if let Err(error) = super::super::update_layout_snapshots(vec![
            "--diagram".to_string(),
            f.diagram_dir.clone(),
            "--filter".to_string(),
            f.stem.clone(),
        ]) {
            let message = match candidate_snapshot_failure(error, &f.path) {
                Ok(message) => message,
                Err(error) => {
                    return Err(rollback_imported_fixture_snapshots(
                        error,
                        f.rollback.iter(),
                    ));
                }
            };
            skipped.push(format!(
                "skip (layout snapshot update failed): {} ({message})",
                f.path.display()
            ));
            reject_fixture(&f)?;
            continue;
        }

        // Parity gate (matches `xtask verify`): keep only fixtures that pass SVG DOM parity.
        if let Err(error) = super::super::compare_all_svgs_with_transaction_locks(
            vec![
                "--check-dom".to_string(),
                "--dom-mode".to_string(),
                "parity".to_string(),
                "--dom-decimals".to_string(),
                "3".to_string(),
                "--diagram".to_string(),
                f.diagram_dir.clone(),
                "--filter".to_string(),
                f.stem.clone(),
            ],
            transaction_locks
                .as_ref()
                .expect("baseline import transaction must hold a family lock")
                .family_lock(),
            transaction_locks
                .as_ref()
                .expect("baseline import transaction must hold a toolchain lock")
                .toolchain_lock(),
        ) {
            let message = match candidate_svg_compare_failure(error, &f.path, &f.stem) {
                Ok(message) => message,
                Err(error) => {
                    return Err(rollback_imported_fixture_snapshots(
                        error,
                        f.rollback.iter(),
                    ));
                }
            };
            skipped.push(format!(
                "skip (svg dom parity mismatch): {} ({message})",
                f.path.display()
            ));
            reject_fixture(&f)?;
            continue;
        }

        if let Err(error) = cleanup_deferred_fixture_files(&f.diagram_dir, &f.stem) {
            return Err(rollback_imported_fixture_snapshots(
                error,
                f.rollback.iter(),
            ));
        }
        record_imported_fixture_content(
            existing,
            c.body,
            f.path.clone(),
            &[f.path.as_path(), deferred_out_path.as_path()],
        );
        f.rollback = None;
        created.push(f);
    }

    if created.is_empty() {
        eprintln!("Imported 0 fixtures (all candidates were skipped).");
    } else {
        eprintln!("Imported {} fixtures:", created.len());
        for f in &created {
            eprintln!("  {}", f.path.display());
        }
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

#[cfg(test)]
mod tests {
    use super::{
        class_has_whitespace_only_text_label, extract_javascript_fixture_literals,
        has_valid_gitgraph_header, xychart_has_renderable_plot,
    };

    #[test]
    fn package_test_literal_extraction_preserves_unicode_and_js_escape_semantics() {
        let source = r#"
const diagram = `stateDiagram-v2
  [*] --> 完成🚀: \u007D \uD83D\uDE0E`;
const other = "flowchart LR\n  开始 --> 完成\xFF";
"#;

        assert_eq!(
            extract_javascript_fixture_literals(source)
                .expect("TypeScript source should parse")
                .into_iter()
                .map(|expression| expression.value)
                .collect::<Vec<_>>(),
            [
                "stateDiagram-v2\n  [*] --> 完成🚀: } 😎",
                "flowchart LR\n  开始 --> 完成ÿ",
            ]
        );
    }

    #[test]
    fn package_test_literal_extraction_folds_static_concatenation_without_regex_false_positives() {
        let source = r#"
const graph = 'gitGraph TB:\n' + 'commit\n';
const dynamic = "flowchart LR" + suffix;
const helpers = {
  check() {
    if (ready) /"stateDiagram-v2"/.test(input);
  },
};
"#;

        assert_eq!(
            extract_javascript_fixture_literals(source)
                .expect("TypeScript source should parse")
                .into_iter()
                .map(|expression| (expression.source_ordinal, expression.value))
                .collect::<Vec<_>>(),
            [(0, "gitGraph TB:\ncommit\n".to_string())]
        );
    }

    #[test]
    fn package_test_literal_extraction_rejects_identifier_composition_without_emitting_fragments() {
        let source = r#"
const header = 'gitGraph:\n';
const graph = header + 'commit\n';
parser.parse(graph);
"#;

        assert!(
            extract_javascript_fixture_literals(source)
                .expect("TypeScript source should parse")
                .is_empty()
        );
    }

    #[test]
    fn package_test_literal_extraction_uses_source_order_ordinals() {
        let source = r#"
const graph = 'gitGraph TB:\n' + 'commit\n';
const note = `not a diagram`;
"#;

        assert_eq!(
            extract_javascript_fixture_literals(source)
                .expect("TypeScript source should parse")
                .into_iter()
                .map(|expression| (expression.source_ordinal, expression.value))
                .collect::<Vec<_>>(),
            [
                (0, "gitGraph TB:\ncommit\n".to_string()),
                (1, "not a diagram".to_string()),
            ]
        );
    }

    #[test]
    fn gitgraph_header_validation_rejects_prefix_only_messages() {
        assert!(has_valid_gitgraph_header("gitGraph:\ncommit\n"));
        assert!(has_valid_gitgraph_header("gitGraph TB:\ncommit\n"));
        assert!(has_valid_gitgraph_header(
            "%%{init: {}}%%\ngitGraph\ncommit\n"
        ));
        assert!(!has_valid_gitgraph_header("gitGraph config directives"));
        assert!(!has_valid_gitgraph_header("gitGraph TBD:\ncommit\n"));
        assert!(!has_valid_gitgraph_header("gitGraph RL:\ncommit\n"));
    }

    #[test]
    fn class_render_baseline_import_skips_whitespace_only_text_label() {
        assert!(!class_has_whitespace_only_text_label(
            "classDiagram\nclass C1[\"OneWord\"]\n"
        ));
        assert!(!class_has_whitespace_only_text_label(
            "classDiagram\nclass C1[\" With spaces around words \"]\n"
        ));

        assert!(class_has_whitespace_only_text_label(
            "classDiagram\nclass C6[\" \"]\n"
        ));
        assert!(class_has_whitespace_only_text_label(
            "classDiagram\nclass C6[\"   \"]\n"
        ));
    }

    #[test]
    fn xychart_render_baseline_import_requires_valid_plot_data() {
        assert!(!xychart_has_renderable_plot("xychart\nx-axis xAxisName\n"));
        assert!(!xychart_has_renderable_plot("xychart\nline \"t\"\n"));
        assert!(!xychart_has_renderable_plot("xychart\nline \"t\" [ ]\n"));
        assert!(!xychart_has_renderable_plot(
            "xychart\nline \"t\" [  +23 [ -45  , 56.6 ]\n"
        ));
        assert!(!xychart_has_renderable_plot(
            "xychart\nbar \"t\" [  +23 , -4aa5  , 56.6 ]\n"
        ));

        assert!(xychart_has_renderable_plot("xychart\nline[1,2,.33]\n"));
        assert!(xychart_has_renderable_plot(
            "xychart\nbar \"barTitle with space\" [ +23 , -45 , 56.6 ]\n"
        ));
    }
}

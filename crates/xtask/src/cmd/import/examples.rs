use super::*;

pub(crate) fn import_upstream_examples(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;
    let mut limit: Option<usize> = None;
    let mut overwrite: bool = false;
    let mut with_baselines: bool = false;
    let mut install: bool = false;
    let mut examples_root: Option<PathBuf> = None;

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
            "--overwrite" => overwrite = true,
            "--with-baselines" => with_baselines = true,
            "--install" => install = true,
            "--examples-root" => {
                i += 1;
                let raw = args.get(i).ok_or(XtaskError::Usage)?;
                examples_root = Some(PathBuf::from(raw));
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

    let examples_root = examples_root
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
                .join("packages")
                .join("examples")
                .join("src")
                .join("examples")
        });
    if !examples_root.exists() {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "upstream examples root not found: {} (expected repo-ref checkout of mermaid@11.12.3)",
            examples_root.display()
        )));
    }

    #[derive(Debug, Clone)]
    struct ExampleBlock {
        source_ts: PathBuf,
        idx_in_file: usize,
        title: Option<String>,
    }

    #[derive(Debug, Clone)]
    struct CreatedFixture {
        diagram_dir: String,
        stem: String,
        path: PathBuf,
        source_ts: PathBuf,
        source_idx_in_file: usize,
        source_title: Option<String>,
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

    fn dedent_template_literal(s: &str) -> String {
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        let s = s.strip_prefix('\n').unwrap_or(&s);
        let lines: Vec<&str> = s.lines().collect();
        if lines.is_empty() {
            return String::new();
        }
        let mut min_indent: Option<usize> = None;
        for (idx, &line) in lines.iter().enumerate() {
            if idx == 0 {
                continue;
            }
            if line.trim().is_empty() {
                continue;
            }
            let indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
            min_indent = Some(match min_indent {
                Some(prev) => prev.min(indent),
                None => indent,
            });
        }
        let Some(min_indent) = min_indent.filter(|v| *v > 0) else {
            return s.to_string();
        };

        let mut out = String::with_capacity(s.len());
        for (idx, &line) in lines.iter().enumerate() {
            if idx == 0 {
                out.push_str(line);
            } else if line.trim().is_empty() {
                out.push_str(line);
            } else {
                let mut removed = 0usize;
                let mut it = line.chars();
                while removed < min_indent {
                    match it.next() {
                        Some(' ') | Some('\t') => removed += 1,
                        Some(other) => {
                            out.push(other);
                            break;
                        }
                        None => break,
                    }
                }
                out.extend(it);
            }
            out.push('\n');
        }
        out
    }

    fn normalize_diagram_dir(detected: &str) -> Option<String> {
        match detected {
            "flowchart" | "flowchart-v2" | "flowchart-elk" => Some("flowchart".to_string()),
            "state" | "stateDiagram" | "stateDiagram-v2" | "stateDiagramV2" => {
                Some("state".to_string())
            }
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

    if install && !with_baselines {
        return Err(XtaskError::SnapshotUpdateFailed(
            "`--install` only applies when `--with-baselines` is set".to_string(),
        ));
    }

    fn deferred_with_baselines_reason(
        diagram_dir: &str,
        fixture_text: &str,
    ) -> Option<&'static str> {
        match diagram_dir {
            "flowchart" => {
                if fixture_text.trim_start().starts_with("flowchart-elk") {
                    return Some("flowchart-elk directive (deferred)");
                }
                if fixture_text.contains("\n  layout: elk")
                    || fixture_text.contains("\nlayout: elk")
                {
                    return Some("flowchart frontmatter config.layout=elk (deferred)");
                }
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
            "gantt" => {
                if fixture_text.starts_with("---\n")
                    && fixture_text.contains("\n---\n")
                    && fixture_text.contains("\ngantt:")
                {
                    return Some("gantt frontmatter config (deferred)");
                }
            }
            "sequence" => {
                if fixture_text.contains("$$") {
                    return Some("sequence math (deferred)");
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

    fn defer_fixture_files(workspace_root: &Path, f: &CreatedFixture, keep_upstream_svg: bool) {
        let deferred_fixture_dir = workspace_root
            .join("fixtures")
            .join("_deferred")
            .join(&f.diagram_dir);
        let _ = fs::create_dir_all(&deferred_fixture_dir);

        let deferred_fixture_path = deferred_fixture_dir.join(format!("{}.mmd", f.stem));
        if deferred_fixture_path.exists() {
            let _ = fs::remove_file(&f.path);
        } else {
            let _ = fs::rename(&f.path, &deferred_fixture_path)
                .or_else(|_| fs::copy(&f.path, &deferred_fixture_path).map(|_| ()))
                .and_then(|_| fs::remove_file(&f.path));
        }

        if keep_upstream_svg {
            let upstream_svg_path = workspace_root
                .join("fixtures")
                .join("upstream-svgs")
                .join(&f.diagram_dir)
                .join(format!("{}.svg", f.stem));
            if upstream_svg_path.exists() {
                let deferred_svg_dir = workspace_root
                    .join("fixtures")
                    .join("_deferred")
                    .join("upstream-svgs")
                    .join(&f.diagram_dir);
                let _ = fs::create_dir_all(&deferred_svg_dir);

                let deferred_svg_path = deferred_svg_dir.join(format!("{}.svg", f.stem));
                if deferred_svg_path.exists() {
                    let _ = fs::remove_file(&upstream_svg_path);
                } else {
                    let _ = fs::rename(&upstream_svg_path, &deferred_svg_path)
                        .or_else(|_| fs::copy(&upstream_svg_path, &deferred_svg_path).map(|_| ()))
                        .and_then(|_| fs::remove_file(&upstream_svg_path));
                }
            }
        } else {
            let _ = fs::remove_file(
                workspace_root
                    .join("fixtures")
                    .join("upstream-svgs")
                    .join(&f.diagram_dir)
                    .join(format!("{}.svg", f.stem)),
            );
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

    let mut ts_files: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(&examples_root).map_err(|err| {
        XtaskError::SnapshotUpdateFailed(format!(
            "failed to list examples directory {}: {err}",
            examples_root.display()
        ))
    })? {
        let path = entry
            .map_err(|err| {
                XtaskError::SnapshotUpdateFailed(format!(
                    "failed to read examples directory entry under {}: {err}",
                    examples_root.display()
                ))
            })?
            .path();
        if path.extension().is_some_and(|e| e == "ts") {
            ts_files.push(path);
        }
    }
    ts_files.sort();

    let example_re = Regex::new(r#"(?s)\{\s*title:\s*(?:'([^']*)'|"([^"]*)").*?code:\s*`([^`]*)`"#)
        .map_err(|err| XtaskError::SnapshotUpdateFailed(format!("bad regex: {err}")))?;

    let reg = merman::detect::DetectorRegistry::default_mermaid_11_12_2_full();

    let mut existing_by_diagram: std::collections::HashMap<
        String,
        std::collections::HashMap<String, PathBuf>,
    > = std::collections::HashMap::new();

    let mut created: Vec<CreatedFixture> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    struct Candidate {
        block: ExampleBlock,
        diagram_dir: String,
        fixtures_dir: PathBuf,
        stem: String,
        body: String,
    }

    let mut candidates: Vec<Candidate> = Vec::new();

    for ts_path in ts_files {
        let text = fs::read_to_string(&ts_path).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to read ts file {}: {err}",
                ts_path.display()
            ))
        })?;

        let source_stem = ts_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let source_slug = clamp_slug(slugify(&source_stem), 48);

        for (idx_in_file, cap) in example_re.captures_iter(&text).enumerate() {
            let title = cap
                .get(1)
                .or_else(|| cap.get(2))
                .map(|m| m.as_str().trim().to_string())
                .filter(|s| !s.is_empty());
            let raw_body = cap.get(3).map(|m| m.as_str()).unwrap_or_default();
            let body = canonical_fixture_text(&dedent_template_literal(raw_body));
            if body.trim().is_empty() {
                continue;
            }

            if let Some(f) = filter.as_deref() {
                let t = title.clone().unwrap_or_default();
                if !source_stem.contains(f) && !t.contains(f) {
                    continue;
                }
            }

            let mut cfg = merman::MermaidConfig::default();
            let detected = match reg.detect_type(body.as_str(), &mut cfg) {
                Ok(t) => t,
                Err(_) => {
                    skipped.push(format!(
                        "skip (type not detected): {} (idx={})",
                        ts_path.display(),
                        idx_in_file
                    ));
                    continue;
                }
            };
            let Some(diagram_dir) = normalize_diagram_dir(detected) else {
                skipped.push(format!(
                    "skip (unsupported detected type '{detected}'): {}",
                    ts_path.display()
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

            let title_slug = clamp_slug(slugify(title.as_deref().unwrap_or("example")), 64);
            let stem = format!(
                "upstream_examples_{source_slug}_{title_slug}_{idx:03}",
                idx = idx_in_file + 1
            );

            candidates.push(Candidate {
                block: ExampleBlock {
                    source_ts: ts_path.clone(),
                    idx_in_file,
                    title: title.clone(),
                },
                diagram_dir,
                fixtures_dir,
                stem,
                body,
            });
        }
    }

    let report_path = workspace_root
        .join("target")
        .join("import-upstream-examples.report.txt");
    let mut report_lines: Vec<String> = Vec::new();
    let mut report_total_candidates: usize = 0;
    let mut report_skip_duplicate_content: usize = 0;
    let mut report_skip_exists: usize = 0;

    let mut imported = 0usize;
    for c in candidates {
        report_total_candidates += 1;
        let existing = existing_by_diagram
            .entry(c.diagram_dir.clone())
            .or_insert_with(|| load_existing_fixtures(&c.fixtures_dir));
        if let Some(existing_path) = existing.get(&c.body) {
            if with_baselines {
                report_skip_duplicate_content += 1;
                report_lines.push(format!(
                    "SKIP_DUPLICATE_CONTENT\t{}\t{}\t{}\texample_idx={}\ttitle={}\texisting={}",
                    c.diagram_dir,
                    c.stem,
                    c.block.source_ts.display(),
                    c.block.idx_in_file,
                    c.block.title.clone().unwrap_or_default(),
                    existing_path.display(),
                ));
            }
            skipped.push(format!(
                "skip (duplicate content): {} -> {}",
                c.block.source_ts.display(),
                existing_path.display()
            ));
            continue;
        }

        let out_path = c.fixtures_dir.join(format!("{}.mmd", c.stem));
        if out_path.exists() && !overwrite {
            if with_baselines {
                report_skip_exists += 1;
                report_lines.push(format!(
                    "SKIP_EXISTS\t{}\t{}\t{}\texample_idx={}\ttitle={}\tpath={}",
                    c.diagram_dir,
                    c.stem,
                    c.block.source_ts.display(),
                    c.block.idx_in_file,
                    c.block.title.clone().unwrap_or_default(),
                    out_path.display(),
                ));
            }
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
            source_ts: c.block.source_ts.clone(),
            source_idx_in_file: c.block.idx_in_file,
            source_title: c.block.title.clone(),
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

        if let Some(reason) = deferred_with_baselines_reason(&f.diagram_dir, &c.body) {
            report_lines.push(format!(
                "DEFERRED_WITH_BASELINES\t{}\t{}\t{}\texample_idx={}\ttitle={}\treason={reason}",
                f.diagram_dir,
                f.stem,
                f.source_ts.display(),
                f.source_idx_in_file,
                f.source_title.clone().unwrap_or_default(),
            ));
            skipped.push(format!(
                "skip (deferred for --with-baselines): {} ({reason})",
                f.path.display(),
            ));
            defer_fixture_files(&workspace_root, &f, false);
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
        match super::super::gen_upstream_svgs(svg_args) {
            Ok(()) => {}
            Err(XtaskError::UpstreamSvgFailed(msg)) => {
                report_lines.push(format!(
                    "UPSTREAM_SVG_FAILED\t{}\t{}\t{}\texample_idx={}\ttitle={}\terr={}",
                    f.diagram_dir,
                    f.stem,
                    f.source_ts.display(),
                    f.source_idx_in_file,
                    f.source_title.clone().unwrap_or_default(),
                    msg.lines().next().unwrap_or("unknown upstream error"),
                ));
                skipped.push(format!(
                    "skip (upstream svg failed): {} ({})",
                    f.path.display(),
                    msg.lines().next().unwrap_or("unknown upstream error")
                ));
                defer_fixture_files(&workspace_root, &f, false);
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
                "UPSTREAM_SVG_SUSPICIOUS_BLANK\t{}\t{}\t{}\texample_idx={}\ttitle={}",
                f.diagram_dir,
                f.stem,
                f.source_ts.display(),
                f.source_idx_in_file,
                f.source_title.clone().unwrap_or_default(),
            ));
            skipped.push(format!(
                "skip (suspicious upstream svg output): {} (blank 16x16-like svg)",
                f.path.display(),
            ));
            defer_fixture_files(&workspace_root, &f, true);
            continue;
        }

        if let Err(err) = super::super::update_snapshots(vec![
            "--diagram".to_string(),
            f.diagram_dir.clone(),
            "--filter".to_string(),
            f.stem.clone(),
        ]) {
            report_lines.push(format!(
                "SNAPSHOT_UPDATE_FAILED\t{}\t{}\t{}\texample_idx={}\ttitle={}\terr={err}",
                f.diagram_dir,
                f.stem,
                f.source_ts.display(),
                f.source_idx_in_file,
                f.source_title.clone().unwrap_or_default(),
            ));
            skipped.push(format!(
                "skip (snapshot update failed): {} ({err})",
                f.path.display(),
            ));
            cleanup_fixture_files(&workspace_root, &f);
            continue;
        }
        if let Err(err) = super::super::update_layout_snapshots(vec![
            "--diagram".to_string(),
            f.diagram_dir.clone(),
            "--filter".to_string(),
            f.stem.clone(),
        ]) {
            report_lines.push(format!(
                "LAYOUT_SNAPSHOT_UPDATE_FAILED\t{}\t{}\t{}\texample_idx={}\ttitle={}\terr={err}",
                f.diagram_dir,
                f.stem,
                f.source_ts.display(),
                f.source_idx_in_file,
                f.source_title.clone().unwrap_or_default(),
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

    if with_baselines {
        if let Some(parent) = report_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let header = format!(
            "# import-upstream-examples report (Mermaid@11.12.3)\n# generated_at={}\n# total_candidates={}\n# skip_duplicate_content={}\n# skip_exists={}\n",
            chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%z"),
            report_total_candidates,
            report_skip_duplicate_content,
            report_skip_exists,
        );
        let mut out = String::new();
        out.push_str(&header);
        if report_lines.is_empty() {
            out.push_str("# (no per-candidate report lines were produced)\n");
        } else {
            out.push_str(&report_lines.join("\n"));
            out.push('\n');
        }
        out.push('\n');
        let _ = fs::write(&report_path, out);
        eprintln!("Wrote import report: {}", report_path.display());
    }

    if created.is_empty() {
        if with_baselines {
            let mut msg = String::from("no fixtures were imported");
            if report_total_candidates == 0 {
                msg.push_str(" (no examples were detected)");
            } else if report_skip_duplicate_content == report_total_candidates {
                msg.push_str(" (all candidates were duplicates of existing fixtures)");
            } else if report_skip_duplicate_content + report_skip_exists == report_total_candidates
            {
                msg.push_str(" (all candidates were duplicates or already existed)");
            } else {
                msg.push_str(" (no candidates passed upstream baseline/snapshot gating)");
            }
            msg.push_str(&format!("; report: {}", report_path.display()));
            eprintln!("{msg}");
            return Ok(());
        }
        return Err(XtaskError::SnapshotUpdateFailed(
            "no fixtures were imported (use --diagram <name> and optionally --filter/--limit)"
                .to_string(),
        ));
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

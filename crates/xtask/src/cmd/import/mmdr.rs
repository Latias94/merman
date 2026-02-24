use super::*;

pub(crate) fn import_mmdr_fixtures(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;
    let mut limit: Option<usize> = None;
    let mut prefer_complex: bool = false;
    let mut overwrite: bool = false;
    let mut with_baselines: bool = false;
    let mut install: bool = false;
    let mut mmd_root: Option<PathBuf> = None;

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
            "--complex" => prefer_complex = true,
            "--overwrite" => overwrite = true,
            "--with-baselines" => with_baselines = true,
            "--install" => install = true,
            "--mmd-root" => {
                i += 1;
                let raw = args.get(i).ok_or(XtaskError::Usage)?;
                mmd_root = Some(PathBuf::from(raw));
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

    let default_mmdr_root = workspace_root
        .join("repo-ref")
        .join("mermaid-rs-renderer")
        .join("tests")
        .join("fixtures");
    let mmd_root = mmd_root
        .map(|p| {
            if p.is_absolute() {
                p
            } else {
                workspace_root.join(p)
            }
        })
        .unwrap_or_else(|| default_mmdr_root.clone());
    let is_default_mmdr_root = mmd_root == default_mmdr_root;

    if !mmd_root.is_dir() {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "mmd fixtures folder not found: {}",
            mmd_root.display()
        )));
    }

    fn strip_yaml_frontmatter(s: &str) -> String {
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        let lines: Vec<&str> = s.lines().collect();
        let mut first_non_empty = 0usize;
        while first_non_empty < lines.len() && lines[first_non_empty].trim().is_empty() {
            first_non_empty += 1;
        }
        if first_non_empty >= lines.len() || lines[first_non_empty].trim() != "---" {
            return s;
        }
        let mut close_idx: Option<usize> = None;
        for i in (first_non_empty + 1)..lines.len() {
            if lines[i].trim() == "---" {
                close_idx = Some(i);
                break;
            }
        }
        let Some(close_idx) = close_idx else {
            return s;
        };
        let body = lines[(close_idx + 1)..].join("\n");
        if body.trim().is_empty() { s } else { body }
    }

    fn canonical_fixture_text(s: &str) -> String {
        let s = strip_yaml_frontmatter(s);
        let s = s.trim_matches('\n');
        format!("{s}\n")
    }

    fn sanitize_stem(raw: &str) -> String {
        let mut out = String::with_capacity(raw.len());
        let mut prev_us = false;
        for ch in raw.chars() {
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
            "architecture" | "block" | "c4" | "gantt" | "info" | "kanban" | "mindmap"
            | "packet" | "pie" | "radar" | "sankey" | "sequence" | "timeline" | "treemap" => {
                Some(detected.to_string())
            }
            _ => None,
        }
    }

    #[derive(Debug, Clone)]
    struct Candidate {
        source_path: PathBuf,
        diagram_dir: String,
        stem: String,
        body: String,
        score: i64,
    }

    fn score_for_body(body: &str) -> i64 {
        let line_count = body.lines().count() as i64;
        (line_count * 1_000) + (body.len() as i64)
    }

    let reg = merman::detect::DetectorRegistry::default_mermaid_11_12_2_full();

    fn collect_mmd_files_recursively(
        root: &Path,
        out: &mut Vec<PathBuf>,
    ) -> Result<(), XtaskError> {
        if root.is_file() {
            if is_file_with_extension(root, "mmd") {
                out.push(root.to_path_buf());
            }
            return Ok(());
        }
        let entries = fs::read_dir(root).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to list mmd fixtures directory {}: {err}",
                root.display()
            ))
        })?;
        for entry in entries {
            let path = entry
                .map_err(|err| {
                    XtaskError::SnapshotUpdateFailed(format!(
                        "failed to read directory entry under {}: {err}",
                        root.display()
                    ))
                })?
                .path();
            if path.is_dir() {
                let dir_name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                if dir_name == "node_modules" || dir_name == "target" {
                    continue;
                }
                collect_mmd_files_recursively(&path, out)?;
            } else if is_file_with_extension(&path, "mmd") {
                out.push(path);
            }
        }
        Ok(())
    }

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    collect_mmd_files_recursively(&mmd_root, &mut mmd_files)?;
    mmd_files.sort();

    let root_tag = if is_default_mmdr_root {
        "mmdr_tests".to_string()
    } else {
        let name = mmd_root
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("mmd");
        format!("upstream_mmd_{}", sanitize_stem(name))
    };

    for path in mmd_files {
        let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };

        if let Some(f) = filter.as_deref() {
            let hay = path.to_string_lossy();
            if !hay.contains(f) {
                continue;
            }
        }

        let text = match fs::read_to_string(&path) {
            Ok(v) => v,
            Err(err) => {
                skipped.push(format!("skip (read failed): {} ({err})", path.display()));
                continue;
            }
        };
        let body = canonical_fixture_text(&text);
        if body.trim().is_empty() {
            continue;
        }

        let mut cfg = merman::MermaidConfig::default();
        let detected = match reg.detect_type(body.as_str(), &mut cfg) {
            Ok(t) => t,
            Err(_) => {
                skipped.push(format!("skip (type not detected): {}", path.display()));
                continue;
            }
        };
        let Some(diagram_dir) = normalize_diagram_dir(detected) else {
            skipped.push(format!(
                "skip (unsupported detected type '{detected}'): {}",
                path.display()
            ));
            continue;
        };

        if diagram_dir == "zenuml" {
            continue;
        }
        if diagram != "all" && diagram_dir != diagram {
            continue;
        }

        let rel_slug = path
            .strip_prefix(&mmd_root)
            .ok()
            .and_then(|p| p.parent())
            .and_then(|p| p.to_str())
            .unwrap_or_default();
        let rel_slug = format!("{rel_slug}_{file_stem}");
        let rel_slug = sanitize_stem(&rel_slug);

        let stem = if is_default_mmdr_root {
            // Preserve the existing naming scheme for mermaid-rs-renderer fixtures.
            let dir_name = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            format!(
                "{root_tag}_{diagram_dir}_{}_{}",
                sanitize_stem(dir_name),
                sanitize_stem(file_stem)
            )
        } else {
            format!("{root_tag}_{diagram_dir}_{rel_slug}")
        };

        candidates.push(Candidate {
            source_path: path,
            diagram_dir,
            stem,
            score: score_for_body(&body),
            body,
        });
    }

    if prefer_complex {
        candidates.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.stem.cmp(&b.stem)));
    } else {
        candidates.sort_by(|a, b| a.stem.cmp(&b.stem));
    }

    let mut existing_by_diagram: std::collections::HashMap<
        String,
        std::collections::HashMap<String, PathBuf>,
    > = std::collections::HashMap::new();
    let mut created: Vec<(String, String, PathBuf)> = Vec::new();

    let mut imported = 0usize;
    for c in candidates {
        let fixtures_dir = workspace_root.join("fixtures").join(&c.diagram_dir);
        if !fixtures_dir.is_dir() {
            skipped.push(format!(
                "skip (fixtures dir missing): {}",
                fixtures_dir.display()
            ));
            continue;
        }

        let existing = existing_by_diagram
            .entry(c.diagram_dir.clone())
            .or_insert_with(|| load_existing_fixtures(&fixtures_dir));
        if let Some(existing_path) = existing.get(&c.body) {
            skipped.push(format!(
                "skip (duplicate content): {} -> {}",
                c.source_path.display(),
                existing_path.display()
            ));
            continue;
        }

        let out_path = fixtures_dir.join(format!("{}.mmd", c.stem));
        if out_path.exists() && !overwrite {
            skipped.push(format!("skip (exists): {}", out_path.display()));
            continue;
        }

        fs::write(&out_path, &c.body).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to write fixture {}: {err}",
                out_path.display()
            ))
        })?;
        existing.insert(c.body.clone(), out_path.clone());
        created.push((c.diagram_dir.clone(), c.stem.clone(), out_path));

        imported += 1;
        if let Some(max) = limit {
            if imported >= max {
                break;
            }
        }
    }

    if created.is_empty() {
        return Err(XtaskError::SnapshotUpdateFailed(
            "no fixtures were imported (use --diagram <name> and optionally --filter/--limit)"
                .to_string(),
        ));
    }

    if install && !with_baselines {
        return Err(XtaskError::SnapshotUpdateFailed(
            "`--install` only applies when `--with-baselines` is set".to_string(),
        ));
    }

    if with_baselines {
        let mut kept: Vec<(String, String, PathBuf)> = Vec::with_capacity(created.len());

        fn is_upstream_error_svg(svg_path: &Path) -> bool {
            let Ok(svg) = fs::read_to_string(svg_path) else {
                return false;
            };
            svg.contains("aria-roledescription=\"error\"")
        }

        fn cleanup_fixture_and_svg(
            workspace_root: &Path,
            diagram_dir: &str,
            stem: &str,
            path: &Path,
        ) {
            let _ = fs::remove_file(path);
            let _ = fs::remove_file(
                workspace_root
                    .join("fixtures")
                    .join("upstream-svgs")
                    .join(diagram_dir)
                    .join(format!("{stem}.svg")),
            );
        }

        for (diagram_dir, stem, path) in &created {
            let mut svg_args = vec![
                "--diagram".to_string(),
                diagram_dir.clone(),
                "--filter".to_string(),
                stem.clone(),
            ];
            if install {
                svg_args.push("--install".to_string());
            }
            super::super::gen_upstream_svgs(svg_args)?;

            let svg_path = workspace_root
                .join("fixtures")
                .join("upstream-svgs")
                .join(diagram_dir)
                .join(format!("{stem}.svg"));
            if is_upstream_error_svg(&svg_path) {
                skipped.push(format!(
                    "skip (upstream rendered error diagram): {}",
                    path.display()
                ));
                cleanup_fixture_and_svg(&workspace_root, diagram_dir, stem, path);
                continue;
            }

            super::super::update_snapshots(vec![
                "--diagram".to_string(),
                diagram_dir.clone(),
                "--filter".to_string(),
                stem.clone(),
            ])?;
            super::super::update_layout_snapshots(vec![
                "--diagram".to_string(),
                diagram_dir.clone(),
                "--filter".to_string(),
                stem.clone(),
            ])?;

            kept.push((diagram_dir.clone(), stem.clone(), path.clone()));
        }

        created = kept;
    }

    eprintln!("Imported {} fixtures:", created.len());
    for (_, _, path) in &created {
        eprintln!("  {}", path.display());
    }
    if !skipped.is_empty() {
        eprintln!("Skipped {} fixtures:", skipped.len());
        for s in skipped.iter().take(50) {
            eprintln!("  {s}");
        }
        if skipped.len() > 50 {
            eprintln!("  ... ({} more)", skipped.len() - 50);
        }
    }

    Ok(())
}

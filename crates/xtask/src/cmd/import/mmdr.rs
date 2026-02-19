use super::*;

pub(crate) fn import_mmdr_fixtures(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;
    let mut limit: Option<usize> = None;
    let mut prefer_complex: bool = false;
    let mut overwrite: bool = false;
    let mut with_baselines: bool = false;
    let mut install: bool = false;

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

    let mmdr_root = workspace_root
        .join("repo-ref")
        .join("mermaid-rs-renderer")
        .join("tests")
        .join("fixtures");
    if !mmdr_root.is_dir() {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "mmdr fixtures folder not found: {} (expected repo-ref checkout of mermaid-rs-renderer)",
            mmdr_root.display()
        )));
    }

    fn canonical_fixture_text(s: &str) -> String {
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
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

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    let Ok(top_entries) = fs::read_dir(&mmdr_root) else {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "failed to list mmdr fixtures directory {}",
            mmdr_root.display()
        )));
    };
    for top_entry in top_entries.flatten() {
        let dir_path = top_entry.path();
        if !dir_path.is_dir() {
            continue;
        }
        let dir_name = dir_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        if dir_name == "node_modules" || dir_name == "target" {
            continue;
        }

        let Ok(entries) = fs::read_dir(&dir_path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !is_file_with_extension(&path, "mmd") {
                continue;
            }
            let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };

            if let Some(f) = filter.as_deref() {
                let hay = format!(
                    "{} {}",
                    dir_name,
                    path.file_name().and_then(|n| n.to_str()).unwrap_or("")
                );
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

            let stem = format!(
                "mmdr_tests_{diagram_dir}_{}_{}",
                sanitize_stem(&dir_name),
                sanitize_stem(file_stem)
            );

            candidates.push(Candidate {
                source_path: path,
                diagram_dir,
                stem,
                score: score_for_body(&body),
                body,
            });
        }
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
        for (diagram_dir, stem, _) in &created {
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
        }
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

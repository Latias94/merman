use super::*;

pub(crate) fn import_upstream_docs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;
    let mut limit: Option<usize> = None;
    let mut min_lines: Option<usize> = None;
    let mut prefer_complex: bool = false;
    let mut overwrite: bool = false;
    let mut with_baselines: bool = false;
    let mut install: bool = false;
    let mut docs_root: Option<PathBuf> = None;

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
            "--docs-root" => {
                i += 1;
                let raw = args.get(i).ok_or(XtaskError::Usage)?;
                docs_root = Some(PathBuf::from(raw));
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

    let docs_root = docs_root
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
                .join("docs")
                .join("syntax")
        });
    if !docs_root.exists() {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "upstream docs root not found: {} (expected repo-ref checkout of mermaid@11.12.2)",
            docs_root.display()
        )));
    }

    #[derive(Debug, Clone)]
    struct MdBlock {
        source_md: PathBuf,
        source_stem: String,
        idx_in_file: usize,
        heading: Option<String>,
        info: String,
        body: String,
    }

    #[derive(Debug, Clone)]
    struct CreatedFixture {
        diagram_dir: String,
        stem: String,
        path: PathBuf,
        source_md: PathBuf,
        source_idx_in_file: usize,
        source_info: String,
        source_heading: Option<String>,
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

    fn extract_md_blocks(md_path: &Path) -> Result<Vec<MdBlock>, XtaskError> {
        let text = fs::read_to_string(md_path).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to read markdown file {}: {err}",
                md_path.display()
            ))
        })?;

        let source_stem = md_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut out = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0usize;
        let mut current_heading: Option<String> = None;
        let mut idx_in_file = 0usize;
        while i < lines.len() {
            let line = lines[i];
            if let Some(h) = line.strip_prefix('#') {
                current_heading = Some(h.trim().trim_start_matches('#').trim().to_string());
            }

            let trimmed = line.trim_start();
            if trimmed.starts_with("```") {
                let ticks = trimmed.chars().take_while(|c| *c == '`').count();
                let info = trimmed[ticks..].trim().to_string();
                i += 1;
                let mut body_lines: Vec<&str> = Vec::new();
                while i < lines.len() {
                    let l = lines[i];
                    if l.trim_start().starts_with(&"`".repeat(ticks)) {
                        break;
                    }
                    body_lines.push(l);
                    i += 1;
                }

                let body = body_lines.join("\n");
                out.push(MdBlock {
                    source_md: md_path.to_path_buf(),
                    source_stem: source_stem.clone(),
                    idx_in_file,
                    heading: current_heading.clone(),
                    info,
                    body,
                });
                idx_in_file += 1;
            }

            i += 1;
        }

        Ok(out)
    }

    fn docs_md_for_diagram(diagram: &str) -> Option<&'static str> {
        match diagram {
            "all" => None,
            "architecture" => Some("architecture.md"),
            "block" => Some("block.md"),
            "c4" => Some("c4.md"),
            "class" => Some("classDiagram.md"),
            "er" => Some("entityRelationshipDiagram.md"),
            "flowchart" => Some("flowchart.md"),
            "gantt" => Some("gantt.md"),
            "gitgraph" => Some("gitgraph.md"),
            "kanban" => Some("kanban.md"),
            "mindmap" => Some("mindmap.md"),
            "packet" => Some("packet.md"),
            "pie" => Some("pie.md"),
            "quadrantchart" => Some("quadrantChart.md"),
            "radar" => Some("radar.md"),
            "requirement" => Some("requirementDiagram.md"),
            "sankey" => Some("sankey.md"),
            "sequence" => Some("sequenceDiagram.md"),
            "state" => Some("stateDiagram.md"),
            "timeline" => Some("timeline.md"),
            "treemap" => Some("treemap.md"),
            "journey" => Some("userJourney.md"),
            "xychart" => Some("xyChart.md"),
            _ => None,
        }
    }

    fn collect_markdown_files_recursively(
        root: &Path,
        out: &mut Vec<PathBuf>,
    ) -> Result<(), XtaskError> {
        if root.is_file() {
            if root.extension().is_some_and(|e| e == "md") {
                out.push(root.to_path_buf());
            }
            return Ok(());
        }
        let entries = fs::read_dir(root).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to list docs directory {}: {err}",
                root.display()
            ))
        })?;
        for entry in entries {
            let path = entry
                .map_err(|err| {
                    XtaskError::SnapshotUpdateFailed(format!(
                        "failed to read docs directory entry under {}: {err}",
                        root.display()
                    ))
                })?
                .path();
            if path.is_dir() {
                collect_markdown_files_recursively(&path, out)?;
            } else if path.extension().is_some_and(|e| e == "md") {
                out.push(path);
            }
        }
        Ok(())
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

    let mut md_files: Vec<PathBuf> = Vec::new();
    if diagram == "all" {
        collect_markdown_files_recursively(&docs_root, &mut md_files)?;
    } else if docs_root.ends_with(PathBuf::from("docs").join("syntax")) {
        let Some(name) = docs_md_for_diagram(&diagram) else {
            return Err(XtaskError::SnapshotUpdateFailed(format!(
                "unknown diagram: {diagram} (expected one of the fixtures/ subfolders, or 'all')"
            )));
        };
        md_files.push(docs_root.join(name));
    } else {
        // When a custom docs root is provided, scan all markdown files under it and rely on diagram detection.
        collect_markdown_files_recursively(&docs_root, &mut md_files)?;
    }
    md_files.sort();

    let allowed_infos = [
        "",
        "mermaid",
        "mermaid-example",
        "mermaid-nocode",
        "architecture",
        "block",
        "c4",
        "classDiagram",
        "erDiagram",
        "flowchart",
        "gantt",
        "gitGraph",
        "kanban",
        "mindmap",
        "packet",
        "pie",
        "quadrantChart",
        "radar",
        "requirementDiagram",
        "sankey",
        "sequenceDiagram",
        "stateDiagram",
        "timeline",
        "treemap",
        "userJourney",
        "xyChart",
        "xychart",
    ];

    let reg = merman::detect::DetectorRegistry::default_mermaid_11_12_2_full();
    let mut created: Vec<CreatedFixture> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    let mut existing_by_diagram: std::collections::HashMap<
        String,
        std::collections::HashMap<String, PathBuf>,
    > = std::collections::HashMap::new();

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

    #[derive(Debug, Clone)]
    struct Candidate {
        md_block: MdBlock,
        diagram_dir: String,
        fixtures_dir: PathBuf,
        stem: String,
        body: String,
        score: i64,
    }

    fn complexity_score(body: &str, diagram_dir: &str) -> i64 {
        let line_count = body.lines().count() as i64;
        let mut score = line_count * 1_000 + (body.len() as i64);
        let lower = body.to_ascii_lowercase();

        fn bump(score: &mut i64, lower: &str, needle: &str, weight: i64) {
            if lower.contains(needle) {
                *score += weight;
            }
        }

        // Global "complexity" markers across diagrams.
        bump(&mut score, &lower, "%%{init", 5_000);
        bump(&mut score, &lower, "accdescr", 2_000);
        bump(&mut score, &lower, "acctitle", 2_000);
        bump(&mut score, &lower, "linkstyle", 2_000);
        bump(&mut score, &lower, "classdef", 2_000);
        bump(&mut score, &lower, "direction", 1_000);
        bump(&mut score, &lower, "click ", 1_500);
        bump(&mut score, &lower, "<img", 1_000);
        bump(&mut score, &lower, "<strong>", 1_000);
        bump(&mut score, &lower, "<em>", 1_000);

        match diagram_dir {
            "flowchart" => {
                bump(&mut score, &lower, "subgraph", 2_000);
                bump(&mut score, &lower, ":::", 1_000);
                bump(&mut score, &lower, "@{", 1_500);
            }
            "sequence" => {
                bump(&mut score, &lower, "alt", 1_500);
                bump(&mut score, &lower, "loop", 1_500);
                bump(&mut score, &lower, "par", 1_500);
                bump(&mut score, &lower, "opt", 1_000);
                bump(&mut score, &lower, "critical", 1_500);
                bump(&mut score, &lower, "rect", 1_000);
                bump(&mut score, &lower, "activate", 1_000);
                bump(&mut score, &lower, "deactivate", 1_000);
            }
            "class" => {
                bump(&mut score, &lower, "namespace", 1_000);
                bump(&mut score, &lower, "interface", 1_000);
                bump(&mut score, &lower, "enum", 1_000);
                bump(&mut score, &lower, "<<", 1_000);
            }
            "state" => {
                bump(&mut score, &lower, "fork", 1_000);
                bump(&mut score, &lower, "join", 1_000);
                bump(&mut score, &lower, "[*]", 1_000);
                bump(&mut score, &lower, "note", 1_000);
            }
            "gantt" => {
                bump(&mut score, &lower, "section", 1_000);
                bump(&mut score, &lower, "crit", 1_000);
                bump(&mut score, &lower, "milestone", 1_000);
                bump(&mut score, &lower, "after", 1_000);
            }
            _ => {}
        }

        score
    }

    let mut candidates: Vec<Candidate> = Vec::new();

    for md_path in md_files {
        if !md_path.is_file() {
            skipped.push(format!("missing markdown source: {}", md_path.display()));
            continue;
        }

        let blocks = extract_md_blocks(&md_path)?;
        let source_stem = md_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let source_slug = clamp_slug(slugify(source_stem), 48);

        for b in blocks {
            if !allowed_infos.iter().any(|v| *v == b.info) {
                continue;
            }
            if let Some(f) = filter.as_deref() {
                let h = b.heading.clone().unwrap_or_default();
                if !b.source_stem.contains(f) && !h.contains(f) {
                    continue;
                }
            }

            let body = canonical_fixture_text(&b.body);
            if body.trim().is_empty() {
                continue;
            }
            if let Some(min) = min_lines {
                if body.lines().count() < min {
                    continue;
                }
            }

            let mut cfg = merman::MermaidConfig::default();
            let detected = match reg.detect_type(body.as_str(), &mut cfg) {
                Ok(t) => t,
                Err(_) => {
                    skipped.push(format!(
                        "skip (type not detected): {} (info='{}', idx={})",
                        b.source_md.display(),
                        b.info,
                        b.idx_in_file
                    ));
                    continue;
                }
            };
            let Some(diagram_dir) = normalize_diagram_dir(detected) else {
                skipped.push(format!(
                    "skip (unsupported detected type '{detected}'): {}",
                    b.source_md.display()
                ));
                continue;
            };

            // External plugin diagrams (like zenuml) are out of scope for now.
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

            let heading_slug = clamp_slug(slugify(b.heading.as_deref().unwrap_or("example")), 64);
            let stem = format!(
                "upstream_docs_{source_slug}_{heading_slug}_{idx:03}",
                idx = b.idx_in_file + 1
            );

            let score = complexity_score(&body, &diagram_dir);
            candidates.push(Candidate {
                md_block: b,
                diagram_dir,
                fixtures_dir,
                stem,
                body,
                score,
            });
        }
    }

    if prefer_complex {
        candidates.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.stem.cmp(&b.stem)));
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
        // Keep `--with-baselines` aligned with the current parity hardening scope.
        //
        // Some examples require upstream (browser) features we have not yet replicated in the
        // headless pipeline. Import them later in dedicated parity work items (tracked in
        // `docs/alignment/FIXTURE_EXPANSION_TODO.md`).
        match diagram_dir {
            "flowchart" => {
                // ELK layout is currently out of scope for the headless layout engine.
                if fixture_text.contains("\n  layout: elk")
                    || fixture_text.contains("\nlayout: elk")
                {
                    return Some("flowchart frontmatter config.layout=elk (deferred)");
                }
                // Flowchart "look" variants change DOM structure and markers; only classic is in scope.
                if fixture_text.contains("\n  look:") || fixture_text.contains("\nlook:") {
                    if !fixture_text.contains("\n  look: classic")
                        && !fixture_text.contains("\nlook: classic")
                    {
                        return Some("flowchart frontmatter config.look!=classic (deferred)");
                    }
                }
                // Math rendering depends on browser KaTeX + foreignObject details.
                if fixture_text.contains("$$") {
                    return Some("flowchart math (deferred)");
                }
            }
            "sequence" => {
                // Math rendering depends on browser KaTeX + font metrics.
                if fixture_text.contains("$$") {
                    return Some("sequence math (deferred)");
                }
            }
            _ => {}
        }
        None
    }

    fn is_suspicious_blank_svg(svg_path: &Path) -> bool {
        // Mermaid CLI often emits a tiny 16x16 SVG for "empty" diagrams (e.g. `graph LR` with
        // no nodes/edges). These are usually unhelpful as parity fixtures and tend to create
        // noisy root viewport diffs.
        //
        // Treat them as "output anomalies" for fixture import purposes: keep the candidate
        // traceable via the report and skip importing it for now.
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

    let report_path = workspace_root
        .join("target")
        .join("import-upstream-docs.report.txt");
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
                    "SKIP_DUPLICATE_CONTENT\t{}\t{}\t{}\tblock_idx={}\tinfo={}\theading={}\texisting={}",
                    c.diagram_dir,
                    c.stem,
                    c.md_block.source_md.display(),
                    c.md_block.idx_in_file,
                    c.md_block.info,
                    c.md_block.heading.clone().unwrap_or_default(),
                    existing_path.display(),
                ));
            }
            skipped.push(format!(
                "skip (duplicate content): {} -> {}",
                c.md_block.source_md.display(),
                existing_path.display()
            ));
            continue;
        }

        let out_path = c.fixtures_dir.join(format!("{}.mmd", c.stem));
        if out_path.exists() && !overwrite {
            if with_baselines {
                report_skip_exists += 1;
                report_lines.push(format!(
                    "SKIP_EXISTS\t{}\t{}\t{}\tblock_idx={}\tinfo={}\theading={}\tpath={}",
                    c.diagram_dir,
                    c.stem,
                    c.md_block.source_md.display(),
                    c.md_block.idx_in_file,
                    c.md_block.info,
                    c.md_block.heading.clone().unwrap_or_default(),
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
            source_md: c.md_block.source_md.clone(),
            source_idx_in_file: c.md_block.idx_in_file,
            source_info: c.md_block.info.clone(),
            source_heading: c.md_block.heading.clone(),
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

        // `--with-baselines`: treat `--limit` as the number of fixtures that survive upstream
        // rendering + snapshot updates (instead of the number of files written).
        if let Some(reason) = deferred_with_baselines_reason(&f.diagram_dir, &c.body) {
            report_lines.push(format!(
                "DEFERRED_WITH_BASELINES\t{}\t{}\t{}\tblock_idx={}\tinfo={}\theading={}\treason={reason}",
                f.diagram_dir,
                f.stem,
                f.source_md.display(),
                f.source_idx_in_file,
                f.source_info,
                f.source_heading.clone().unwrap_or_default(),
            ));
            skipped.push(format!(
                "skip (deferred for --with-baselines): {} ({reason})",
                f.path.display(),
            ));
            cleanup_fixture_files(&workspace_root, &f);
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
                    "UPSTREAM_SVG_FAILED\t{}\t{}\t{}\tblock_idx={}\tinfo={}\theading={}\tmsg={}",
                    f.diagram_dir,
                    f.stem,
                    f.source_md.display(),
                    f.source_idx_in_file,
                    f.source_info,
                    f.source_heading.clone().unwrap_or_default(),
                    msg.lines().next().unwrap_or("unknown upstream error"),
                ));
                skipped.push(format!(
                    "skip (upstream svg failed): {} ({})",
                    f.path.display(),
                    msg.lines().next().unwrap_or("unknown upstream error")
                ));
                cleanup_fixture_files(&workspace_root, &f);
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
                "UPSTREAM_SVG_SUSPICIOUS_BLANK\t{}\t{}\t{}\tblock_idx={}\tinfo={}\theading={}",
                f.diagram_dir,
                f.stem,
                f.source_md.display(),
                f.source_idx_in_file,
                f.source_info,
                f.source_heading.clone().unwrap_or_default(),
            ));
            skipped.push(format!(
                "skip (suspicious upstream svg output): {} (blank 16x16-like svg)",
                f.path.display(),
            ));
            cleanup_fixture_files(&workspace_root, &f);
            continue;
        }

        if let Err(err) = super::super::update_snapshots(vec![
            "--diagram".to_string(),
            f.diagram_dir.clone(),
            "--filter".to_string(),
            f.stem.clone(),
        ]) {
            report_lines.push(format!(
                "SNAPSHOT_UPDATE_FAILED\t{}\t{}\t{}\tblock_idx={}\tinfo={}\theading={}\terr={err}",
                f.diagram_dir,
                f.stem,
                f.source_md.display(),
                f.source_idx_in_file,
                f.source_info,
                f.source_heading.clone().unwrap_or_default(),
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
                "LAYOUT_SNAPSHOT_UPDATE_FAILED\t{}\t{}\t{}\tblock_idx={}\tinfo={}\theading={}\terr={err}",
                f.diagram_dir,
                f.stem,
                f.source_md.display(),
                f.source_idx_in_file,
                f.source_info,
                f.source_heading.clone().unwrap_or_default(),
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
            "# import-upstream-docs report (Mermaid@11.12.2)\n# generated_at={}\n# total_candidates={}\n# skip_duplicate_content={}\n# skip_exists={}\n",
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
                msg.push_str(" (no Mermaid code blocks were detected)");
            } else if report_skip_duplicate_content == report_total_candidates {
                msg.push_str(" (all candidates were duplicates of existing fixtures)");
            } else if report_skip_duplicate_content + report_skip_exists == report_total_candidates
            {
                msg.push_str(" (all candidates were duplicates or already existed)");
            } else {
                msg.push_str(" (no candidates passed upstream baseline/snapshot gating)");
            }
            msg.push_str(&format!("; report: {}", report_path.display()));
            return Err(XtaskError::SnapshotUpdateFailed(msg));
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

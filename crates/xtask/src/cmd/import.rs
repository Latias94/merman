use crate::XtaskError;
use crate::util::*;
use regex::Regex;
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

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
        match super::gen_upstream_svgs(svg_args) {
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

        if let Err(err) = super::update_snapshots(vec![
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
        if let Err(err) = super::update_layout_snapshots(vec![
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

pub(crate) fn import_upstream_html(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;
    let mut limit: Option<usize> = None;
    let mut min_lines: Option<usize> = None;
    let mut prefer_complex: bool = false;
    let mut overwrite: bool = false;
    let mut with_baselines: bool = false;
    let mut install: bool = false;
    let mut html_root: Option<PathBuf> = None;

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
            "--html-root" => {
                i += 1;
                let raw = args.get(i).ok_or(XtaskError::Usage)?;
                html_root = Some(PathBuf::from(raw));
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

    let html_root = html_root
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
                .join("demos")
        });
    if !html_root.exists() {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "upstream html root not found: {} (expected repo-ref checkout of mermaid@11.12.2)",
            html_root.display()
        )));
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

    fn html_unescape_basic(s: &str) -> String {
        let s = s.replace("&amp;", "&");
        let s = s.replace("&lt;", "<").replace("&gt;", ">");
        let s = s.replace("&quot;", "\"").replace("&#39;", "'");
        let s = s.replace("&nbsp;", " ");
        let s = s.replace("&#160;", " ").replace("&#xA0;", " ");
        s
    }

    fn dedent(s: &str) -> String {
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        let lines: Vec<&str> = s.lines().collect();
        let min_indent = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                l.as_bytes()
                    .iter()
                    .take_while(|b| **b == b' ' || **b == b'\t')
                    .count()
            })
            .min()
            .unwrap_or(0);
        let mut out = String::with_capacity(s.len());
        for (idx, line) in lines.iter().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            if line.len() >= min_indent {
                out.push_str(&line[min_indent..]);
            } else {
                out.push_str(line);
            }
        }
        out
    }

    fn normalize_yaml_frontmatter_indentation(s: &str) -> String {
        fn trim_front_ws(line: &str, n: usize) -> &str {
            let mut removed = 0usize;
            for (idx, ch) in line.char_indices() {
                if removed >= n {
                    return &line[idx..];
                }
                if ch == ' ' || ch == '\t' {
                    removed += 1;
                    continue;
                }
                return &line[idx..];
            }
            if removed >= n { "" } else { line }
        }

        let lines: Vec<&str> = s.lines().collect();
        let mut first_non_empty = 0usize;
        while first_non_empty < lines.len() && lines[first_non_empty].trim().is_empty() {
            first_non_empty += 1;
        }
        if first_non_empty >= lines.len() {
            return s.to_string();
        }
        if lines[first_non_empty].trim() != "---" {
            return s.to_string();
        }

        let mut close_idx: Option<usize> = None;
        for i in (first_non_empty + 1)..lines.len() {
            if lines[i].trim() == "---" {
                close_idx = Some(i);
                break;
            }
        }
        let Some(close_idx) = close_idx else {
            return s.to_string();
        };

        let mut min_indent = None::<usize>;
        for l in &lines[(first_non_empty + 1)..close_idx] {
            if l.trim().is_empty() {
                continue;
            }
            let indent = l
                .as_bytes()
                .iter()
                .take_while(|b| **b == b' ' || **b == b'\t')
                .count();
            min_indent = Some(min_indent.map(|m| m.min(indent)).unwrap_or(indent));
        }
        let min_indent = min_indent.unwrap_or(0);

        let mut out = String::with_capacity(s.len());
        for (idx, line) in lines.iter().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            if idx == first_non_empty || idx == close_idx {
                out.push_str("---");
                continue;
            }
            if idx > first_non_empty && idx < close_idx {
                out.push_str(trim_front_ws(line, min_indent));
                continue;
            }
            out.push_str(line);
        }
        out
    }

    fn normalize_html_mermaid_block(raw: &str) -> String {
        let s = dedent(&html_unescape_basic(raw));
        let s = normalize_yaml_frontmatter_indentation(&s);
        // Upstream HTML fixtures sometimes include HTML comment markers inside `<pre class="mermaid">`
        // blocks (e.g. `<!-- prettier-ignore -->`). These are not Mermaid syntax and would prevent
        // our diagram detector from recognizing the block, so strip comment-only lines.
        let mut out = String::with_capacity(s.len());
        let mut wrote_any = false;
        for line in s.lines() {
            let is_html_comment_line = {
                let t = line.trim();
                t.starts_with("<!--") && t.ends_with("-->")
            };
            if is_html_comment_line {
                continue;
            }
            if wrote_any {
                out.push('\n');
            }
            out.push_str(line);
            wrote_any = true;
        }
        out
    }

    fn collect_html_files_recursively(
        root: &Path,
        out: &mut Vec<PathBuf>,
    ) -> Result<(), XtaskError> {
        if root.is_file() {
            if root.extension().is_some_and(|e| e == "html") {
                out.push(root.to_path_buf());
            }
            return Ok(());
        }
        let entries = fs::read_dir(root).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to list html directory {}: {err}",
                root.display()
            ))
        })?;
        for entry in entries {
            let path = entry
                .map_err(|err| {
                    XtaskError::SnapshotUpdateFailed(format!(
                        "failed to read html directory entry under {}: {err}",
                        root.display()
                    ))
                })?
                .path();
            if path.is_dir() {
                collect_html_files_recursively(&path, out)?;
            } else if path.extension().is_some_and(|e| e == "html") {
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

    fn complexity_score(body: &str, diagram_dir: &str) -> i64 {
        let line_count = body.lines().count() as i64;
        let mut score = line_count * 1_000 + (body.len() as i64);
        let lower = body.to_ascii_lowercase();

        fn bump(score: &mut i64, lower: &str, needle: &str, weight: i64) {
            if lower.contains(needle) {
                *score += weight;
            }
        }

        bump(&mut score, &lower, "%%{init", 5_000);
        bump(&mut score, &lower, "accdescr", 2_000);
        bump(&mut score, &lower, "acctitle", 2_000);
        bump(&mut score, &lower, "classdef", 2_000);
        bump(&mut score, &lower, "direction", 1_000);
        bump(&mut score, &lower, "<br", 1_000);

        if diagram_dir == "state" {
            bump(&mut score, &lower, "note ", 2_000);
            bump(&mut score, &lower, "state ", 1_000);
            bump(&mut score, &lower, "{", 1_000);
        }

        score
    }

    #[derive(Debug, Clone)]
    struct HtmlBlock {
        source_html: PathBuf,
        source_stem: String,
        idx_in_file: usize,
        heading: Option<String>,
        body: String,
    }

    fn strip_tags(s: &str) -> String {
        static TAG_RE: OnceLock<Regex> = OnceLock::new();
        let re = TAG_RE.get_or_init(|| Regex::new(r"(?is)<[^>]+>").expect("valid regex"));
        re.replace_all(s, "").to_string()
    }

    fn extract_html_blocks(html_path: &Path) -> Result<Vec<HtmlBlock>, XtaskError> {
        let text = fs::read_to_string(html_path).map_err(|source| XtaskError::ReadFile {
            path: html_path.display().to_string(),
            source,
        })?;

        static PRE_RE: OnceLock<Regex> = OnceLock::new();
        static H_RE: OnceLock<Regex> = OnceLock::new();
        let pre_re = PRE_RE.get_or_init(|| {
            Regex::new(r"(?is)<pre\b(?P<attrs>[^>]*)>(?P<body>.*?)</pre\s*>").expect("valid regex")
        });
        let h_re = H_RE.get_or_init(|| {
            Regex::new(r"(?is)<h[1-6]\b[^>]*>(?P<body>.*?)</h[1-6]>").expect("valid regex")
        });

        let source_stem = html_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("html")
            .to_string();

        let mut headings: Vec<(usize, String)> = Vec::new();
        for cap in h_re.captures_iter(&text) {
            if let (Some(m), Some(b)) = (cap.get(0), cap.name("body")) {
                let clean = strip_tags(b.as_str());
                let clean = html_unescape_basic(clean.trim());
                if !clean.trim().is_empty() {
                    headings.push((m.start(), clean.trim().to_string()));
                }
            }
        }
        headings.sort_by_key(|(pos, _)| *pos);

        let mut out: Vec<HtmlBlock> = Vec::new();
        let mut idx_in_file = 0usize;
        for cap in pre_re.captures_iter(&text) {
            let m = cap.get(0).expect("match");
            let attrs = cap.name("attrs").map(|m| m.as_str()).unwrap_or_default();
            if !attrs.to_ascii_lowercase().contains("mermaid") {
                continue;
            }
            let raw_body = cap.name("body").map(|m| m.as_str()).unwrap_or_default();

            let mut heading: Option<String> = None;
            for (pos, h) in headings.iter().rev() {
                if *pos < m.start() {
                    heading = Some(h.clone());
                    break;
                }
            }

            out.push(HtmlBlock {
                source_html: html_path.to_path_buf(),
                source_stem: source_stem.clone(),
                idx_in_file,
                heading,
                body: raw_body.to_string(),
            });
            idx_in_file += 1;
        }

        Ok(out)
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

    #[derive(Debug, Clone)]
    struct Candidate {
        block: HtmlBlock,
        diagram_dir: String,
        fixtures_dir: PathBuf,
        stem: String,
        body: String,
        score: i64,
    }

    let reg = merman::detect::DetectorRegistry::default_mermaid_11_12_2_full();
    let mut html_files: Vec<PathBuf> = Vec::new();
    collect_html_files_recursively(&html_root, &mut html_files)?;
    html_files.sort();

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    let mut existing_by_diagram: std::collections::HashMap<
        String,
        std::collections::HashMap<String, PathBuf>,
    > = std::collections::HashMap::new();

    for html_path in html_files {
        if let Some(f) = filter.as_deref() {
            let hay = html_path.to_string_lossy();
            if !hay.contains(f) {
                // Still allow filtering by heading later; don't early-skip the file here.
            }
        }

        let blocks = extract_html_blocks(&html_path)?;
        for b in blocks {
            let body = canonical_fixture_text(&normalize_html_mermaid_block(&b.body));
            if body.trim().is_empty() {
                continue;
            }
            if let Some(min) = min_lines {
                if body.lines().count() < min {
                    continue;
                }
            }

            if let Some(f) = filter.as_deref() {
                let mut hay = html_path.to_string_lossy().to_string();
                if let Some(h) = b.heading.as_deref() {
                    hay.push(' ');
                    hay.push_str(h);
                }
                if !hay.contains(f) {
                    continue;
                }
            }

            let mut cfg = merman::MermaidConfig::default();
            let detected = match reg.detect_type(body.as_str(), &mut cfg) {
                Ok(t) => t,
                Err(_) => {
                    skipped.push(format!(
                        "skip (type not detected): {} (idx={})",
                        b.source_html.display(),
                        b.idx_in_file
                    ));
                    continue;
                }
            };
            let Some(diagram_dir) = normalize_diagram_dir(detected) else {
                skipped.push(format!(
                    "skip (unsupported detected type '{detected}'): {}",
                    b.source_html.display()
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

            let source_slug = clamp_slug(slugify(&format!("demos_{}", b.source_stem)), 48);
            let heading_slug = clamp_slug(slugify(b.heading.as_deref().unwrap_or("example")), 64);
            let stem = format!(
                "upstream_html_{source_slug}_{heading_slug}_{idx:03}",
                idx = b.idx_in_file + 1
            );

            let score = complexity_score(&body, &diagram_dir);
            candidates.push(Candidate {
                block: b,
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

    #[derive(Debug, Clone)]
    struct CreatedFixture {
        diagram_dir: String,
        stem: String,
        path: PathBuf,
    }

    let mut created: Vec<CreatedFixture> = Vec::new();
    let mut imported = 0usize;

    for c in candidates {
        let existing = existing_by_diagram
            .entry(c.diagram_dir.clone())
            .or_insert_with(|| load_existing_fixtures(&c.fixtures_dir));
        if let Some(existing_path) = existing.get(&c.body) {
            skipped.push(format!(
                "skip (duplicate content): {} -> {}",
                c.block.source_html.display(),
                existing_path.display()
            ));
            continue;
        }

        let out_path = c.fixtures_dir.join(format!("{}.mmd", c.stem));
        if out_path.exists() && !overwrite {
            skipped.push(format!("skip (already exists): {}", out_path.display()));
            continue;
        }
        let deferred_out_path = workspace_root
            .join("fixtures")
            .join("_deferred")
            .join(&c.diagram_dir)
            .join(format!("{}.mmd", c.stem));
        if deferred_out_path.exists() && !overwrite {
            skipped.push(format!(
                "skip (already deferred): {}",
                deferred_out_path.display()
            ));
            continue;
        }

        fs::write(&out_path, c.body.as_bytes()).map_err(|source| XtaskError::WriteFile {
            path: out_path.display().to_string(),
            source,
        })?;
        existing.insert(c.body.clone(), out_path.clone());

        created.push(CreatedFixture {
            diagram_dir: c.diagram_dir,
            stem: c.stem,
            path: out_path,
        });

        imported += 1;
        if let Some(max) = limit {
            if imported >= max {
                break;
            }
        }
    }

    if created.is_empty() {
        if !skipped.is_empty() {
            let mut dup = 0usize;
            let mut exists = 0usize;
            let mut deferred = 0usize;
            for s in &skipped {
                if s.starts_with("skip (duplicate content):") {
                    dup += 1;
                } else if s.starts_with("skip (already exists):") {
                    exists += 1;
                } else if s.starts_with("skip (already deferred):") {
                    deferred += 1;
                }
            }
            let mut msg = String::from("no fixtures were imported");
            if dup + exists + deferred > 0 {
                msg.push_str(&format!(
                    " (skipped: {dup} duplicate, {exists} exists, {deferred} deferred)"
                ));
            }
            msg.push_str(" (use --overwrite, or adjust --filter/--limit)");
            return Err(XtaskError::SnapshotUpdateFailed(msg));
        }
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
        fn is_suspicious_blank_svg(svg_path: &Path) -> bool {
            let Ok(head) = fs::read_to_string(svg_path) else {
                return false;
            };
            let first = head.lines().next().unwrap_or_default();
            first.contains(r#"viewBox="-8 -8 16 16""#)
                || first.contains(r#"viewBox="0 0 16 16""#)
                || first.contains(r#"style="max-width: 16px"#)
        }

        fn should_defer_fixture(diagram_dir: &str, fixture_text: &str) -> Option<&'static str> {
            match diagram_dir {
                "flowchart" => {
                    if fixture_text.contains("\n  layout: elk")
                        || fixture_text.contains("\nlayout: elk")
                    {
                        return Some("flowchart frontmatter config.layout=elk (deferred)");
                    }
                    if fixture_text
                        .lines()
                        .any(|l| l.trim_start().starts_with("flowchart-elk"))
                    {
                        return Some("flowchart diagram type flowchart-elk (deferred)");
                    }
                }
                "sequence" => {
                    if fixture_text.contains("$$") {
                        return Some(
                            "sequence math rendering uses <foreignObject> upstream (deferred)",
                        );
                    }
                }
                _ => {}
            }
            None
        }

        fn defer_fixture_files_keep_baselines(workspace_root: &Path, f: &CreatedFixture) {
            let deferred_dir = workspace_root
                .join("fixtures")
                .join("_deferred")
                .join(&f.diagram_dir);
            let deferred_svg_dir = workspace_root
                .join("fixtures")
                .join("_deferred")
                .join("upstream-svgs")
                .join(&f.diagram_dir);
            let _ = fs::create_dir_all(&deferred_dir);
            let _ = fs::create_dir_all(&deferred_svg_dir);

            let deferred_mmd_path = deferred_dir.join(format!("{}.mmd", f.stem));
            let _ = fs::remove_file(&deferred_mmd_path);
            let _ = fs::rename(&f.path, &deferred_mmd_path);

            let svg_path = workspace_root
                .join("fixtures")
                .join("upstream-svgs")
                .join(&f.diagram_dir)
                .join(format!("{}.svg", f.stem));
            if svg_path.exists() {
                let deferred_svg_path = deferred_svg_dir.join(format!("{}.svg", f.stem));
                let _ = fs::remove_file(&deferred_svg_path);
                let _ = fs::rename(&svg_path, &deferred_svg_path);
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

        let mut kept: Vec<CreatedFixture> = Vec::with_capacity(created.len());
        for f in &created {
            let mut svg_args = vec![
                "--diagram".to_string(),
                f.diagram_dir.clone(),
                "--filter".to_string(),
                f.stem.clone(),
            ];
            if install {
                svg_args.push("--install".to_string());
            }
            if let Err(err) = super::gen_upstream_svgs(svg_args) {
                skipped.push(format!(
                    "defer (upstream svg generation failed): {} ({err})",
                    f.path.display()
                ));
                defer_fixture_files_keep_baselines(&workspace_root, f);
                continue;
            }

            let fixture_text = match fs::read_to_string(&f.path) {
                Ok(v) => v,
                Err(err) => {
                    skipped.push(format!(
                        "defer (failed to read fixture after import): {} ({err})",
                        f.path.display()
                    ));
                    defer_fixture_files_keep_baselines(&workspace_root, f);
                    continue;
                }
            };
            if let Some(reason) = should_defer_fixture(&f.diagram_dir, &fixture_text) {
                skipped.push(format!("defer ({reason}): {}", f.path.display()));
                defer_fixture_files_keep_baselines(&workspace_root, f);
                continue;
            }

            let svg_path = workspace_root
                .join("fixtures")
                .join("upstream-svgs")
                .join(&f.diagram_dir)
                .join(format!("{}.svg", f.stem));
            if is_suspicious_blank_svg(&svg_path) {
                skipped.push(format!(
                    "defer (suspicious upstream blank svg): {}",
                    f.path.display()
                ));
                defer_fixture_files_keep_baselines(&workspace_root, f);
                continue;
            }

            if let Err(err) = super::update_snapshots(vec![
                "--diagram".to_string(),
                f.diagram_dir.clone(),
                "--filter".to_string(),
                f.stem.clone(),
            ]) {
                skipped.push(format!(
                    "defer (snapshot update failed): {} ({err})",
                    f.path.display()
                ));
                defer_fixture_files_keep_baselines(&workspace_root, f);
                continue;
            }

            if let Err(err) = super::update_layout_snapshots(vec![
                "--diagram".to_string(),
                f.diagram_dir.clone(),
                "--filter".to_string(),
                f.stem.clone(),
            ]) {
                skipped.push(format!(
                    "defer (layout snapshot update failed): {} ({err})",
                    f.path.display()
                ));
                defer_fixture_files_keep_baselines(&workspace_root, f);
                continue;
            }

            kept.push(f.clone());
        }
        created = kept;
        if created.is_empty() {
            return Err(XtaskError::SnapshotUpdateFailed(
                "no fixtures were imported (all created candidates were deferred due to baseline/snapshot failures)"
                    .to_string(),
            ));
        }
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

pub(crate) fn import_upstream_cypress(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;
    let mut limit: Option<usize> = None;
    let mut min_lines: Option<usize> = None;
    let mut prefer_complex: bool = false;
    let mut overwrite: bool = false;
    let mut with_baselines: bool = false;
    let mut install: bool = false;
    let mut spec_root: Option<PathBuf> = None;

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
            "--spec-root" => {
                i += 1;
                let raw = args.get(i).ok_or(XtaskError::Usage)?;
                spec_root = Some(PathBuf::from(raw));
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

    let spec_root = spec_root
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
                .join("cypress")
                .join("integration")
                .join("rendering")
        });
    if !spec_root.exists() {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "upstream cypress spec root not found: {} (expected repo-ref checkout of mermaid@11.12.2)",
            spec_root.display()
        )));
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

    fn html_unescape_basic(s: &str) -> String {
        // Cypress rendering specs sometimes embed Mermaid code through HTML, so `<`/`>` sequences
        // can be entity-escaped in the source even though Mermaid receives the decoded text.
        //
        // Keep this intentionally minimal: only decode the entity forms we've observed in
        // upstream fixtures.
        let s = s.replace("&amp;", "&");
        let s = s.replace("&lt;", "<").replace("&gt;", ">");
        let s = s.replace("&quot;", "\"").replace("&#39;", "'");
        let s = s.replace("&nbsp;", " ");
        let s = s.replace("&#160;", " ").replace("&#xA0;", " ");
        s
    }

    fn dedent(s: &str) -> String {
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        let lines: Vec<&str> = s.lines().collect();
        let min_indent = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                l.as_bytes()
                    .iter()
                    .take_while(|b| **b == b' ' || **b == b'\t')
                    .count()
            })
            .min()
            .unwrap_or(0);
        let mut out = String::with_capacity(s.len());
        for (idx, line) in lines.iter().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            if line.len() >= min_indent {
                out.push_str(&line[min_indent..]);
            } else {
                out.push_str(line);
            }
        }
        out
    }

    fn normalize_yaml_frontmatter_indentation(s: &str) -> String {
        fn trim_front_ws(line: &str, n: usize) -> &str {
            let mut removed = 0usize;
            for (idx, ch) in line.char_indices() {
                if removed >= n {
                    return &line[idx..];
                }
                if ch == ' ' || ch == '\t' {
                    removed += 1;
                    continue;
                }
                return &line[idx..];
            }
            if removed >= n { "" } else { line }
        }

        let lines: Vec<&str> = s.lines().collect();
        let mut first_non_empty = 0usize;
        while first_non_empty < lines.len() && lines[first_non_empty].trim().is_empty() {
            first_non_empty += 1;
        }
        if first_non_empty >= lines.len() {
            return s.to_string();
        }
        if lines[first_non_empty].trim() != "---" {
            return s.to_string();
        }

        let mut close_idx: Option<usize> = None;
        for i in (first_non_empty + 1)..lines.len() {
            if lines[i].trim() == "---" {
                close_idx = Some(i);
                break;
            }
        }
        let Some(close_idx) = close_idx else {
            return s.to_string();
        };

        let mut min_indent = None::<usize>;
        for l in &lines[(first_non_empty + 1)..close_idx] {
            if l.trim().is_empty() {
                continue;
            }
            let indent = l
                .as_bytes()
                .iter()
                .take_while(|b| **b == b' ' || **b == b'\t')
                .count();
            min_indent = Some(min_indent.map(|m| m.min(indent)).unwrap_or(indent));
        }
        let min_indent = min_indent.unwrap_or(0);

        let mut out = String::with_capacity(s.len());
        for (idx, line) in lines.iter().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            if idx == first_non_empty || idx == close_idx {
                out.push_str("---");
                continue;
            }
            if idx > first_non_empty && idx < close_idx {
                out.push_str(trim_front_ws(line, min_indent));
                continue;
            }
            out.push_str(line);
        }
        out
    }

    fn normalize_cypress_fixture_text(raw: &str) -> String {
        let s = dedent(&html_unescape_basic(raw));
        normalize_yaml_frontmatter_indentation(&s)
    }

    fn normalize_architecture_beta_legacy_edges(s: &str) -> String {
        // Cypress architecture fixtures (`repo-ref/mermaid/cypress/integration/rendering/architecture.spec.ts`)
        // use a legacy shorthand that is not accepted by Mermaid@11.12.2 CLI (Langium grammar):
        //
        // - `a L--R b`
        // - `a (L--R) b`
        // - `a L-[Label]-R b`
        // - split parens across lines, e.g. `a (B--T b` / `a R--L) b`
        //
        // Normalize into CLI-compatible form:
        //
        // - `a:L -- R:b`
        // - `a:L -[Label]- R:b`
        static EDGE_DIR_RE: OnceLock<Regex> = OnceLock::new();
        static EDGE_LABEL_RE: OnceLock<Regex> = OnceLock::new();
        let edge_dir_re = EDGE_DIR_RE.get_or_init(|| {
            Regex::new(
                r"^(?P<indent>\s*)(?P<src>\S+)\s+\(?(?P<d1>[LTRB])--(?P<d2>[LTRB])\)?\s+(?P<dst>\S+)\s*$",
            )
            .expect("valid regex")
        });
        let edge_label_re = EDGE_LABEL_RE.get_or_init(|| {
            Regex::new(
                r"^(?P<indent>\s*)(?P<src>\S+)\s+(?P<d1>[LTRB])-\[(?P<label>[^\]]*)\]-(?P<d2>[LTRB])\s+(?P<dst>\S+)\s*$",
            )
            .expect("valid regex")
        });

        let mut out = String::with_capacity(s.len());
        for (idx, raw_line) in s.lines().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            let line = raw_line.trim_end_matches(|c| c == ' ' || c == '\t');

            if let Some(caps) = edge_label_re.captures(line) {
                let indent = caps.name("indent").map(|m| m.as_str()).unwrap_or_default();
                let src = caps.name("src").map(|m| m.as_str()).unwrap_or_default();
                let d1 = caps.name("d1").map(|m| m.as_str()).unwrap_or_default();
                let label = caps.name("label").map(|m| m.as_str()).unwrap_or_default();
                let d2 = caps.name("d2").map(|m| m.as_str()).unwrap_or_default();
                let dst = caps.name("dst").map(|m| m.as_str()).unwrap_or_default();

                out.push_str(indent);
                out.push_str(src);
                out.push(':');
                out.push_str(d1);
                out.push_str(" -[");
                out.push_str(label);
                out.push_str("]- ");
                out.push_str(d2);
                out.push(':');
                out.push_str(dst);
                continue;
            }

            if let Some(caps) = edge_dir_re.captures(line) {
                let indent = caps.name("indent").map(|m| m.as_str()).unwrap_or_default();
                let src = caps.name("src").map(|m| m.as_str()).unwrap_or_default();
                let d1 = caps.name("d1").map(|m| m.as_str()).unwrap_or_default();
                let d2 = caps.name("d2").map(|m| m.as_str()).unwrap_or_default();
                let dst = caps.name("dst").map(|m| m.as_str()).unwrap_or_default();

                out.push_str(indent);
                out.push_str(src);
                out.push(':');
                out.push_str(d1);
                out.push_str(" -- ");
                out.push_str(d2);
                out.push(':');
                out.push_str(dst);
                continue;
            }

            out.push_str(line);
        }

        out
    }

    fn collect_spec_files_recursively(
        root: &Path,
        out: &mut Vec<PathBuf>,
    ) -> Result<(), XtaskError> {
        if root.is_file() {
            if root.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                (n.ends_with(".spec.js") || n.ends_with(".spec.ts")) && !n.contains("node_modules")
            }) {
                out.push(root.to_path_buf());
            }
            return Ok(());
        }
        let entries = fs::read_dir(root).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to list cypress directory {}: {err}",
                root.display()
            ))
        })?;
        for entry in entries {
            let path = entry
                .map_err(|err| {
                    XtaskError::SnapshotUpdateFailed(format!(
                        "failed to read cypress directory entry under {}: {err}",
                        root.display()
                    ))
                })?
                .path();
            if path.is_dir() {
                collect_spec_files_recursively(&path, out)?;
            } else if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                (n.ends_with(".spec.js") || n.ends_with(".spec.ts")) && !n.contains("node_modules")
            }) {
                out.push(path);
            }
        }
        Ok(())
    }

    fn extract_first_template_literal(s: &str, start: usize) -> Option<(String, usize)> {
        let bytes = s.as_bytes();
        let mut i = start;
        while i < bytes.len() && bytes[i] != b'`' {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        // i points at opening backtick
        i += 1;
        let mut out = String::new();
        let mut escaped = false;
        while i < bytes.len() {
            let b = bytes[i];
            if escaped {
                match b {
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'\\' => out.push('\\'),
                    b'`' => out.push('`'),
                    _ => out.push(b as char),
                }
                escaped = false;
                i += 1;
                continue;
            }
            if b == b'\\' {
                escaped = true;
                i += 1;
                continue;
            }
            if b == b'`' {
                return Some((out, i + 1));
            }
            // Reject template interpolation blocks; those aren't static Mermaid fixtures.
            if b == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                return None;
            }
            out.push(b as char);
            i += 1;
        }
        None
    }

    fn extract_last_template_literal(s: &str, start: usize) -> Option<(String, usize)> {
        let mut cursor = start;
        let mut last: Option<(String, usize)> = None;
        while let Some((raw, end_rel)) = extract_first_template_literal(s, cursor) {
            last = Some((raw, end_rel));
            cursor = end_rel;
        }
        last
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

    fn complexity_score(body: &str, diagram_dir: &str) -> i64 {
        let line_count = body.lines().count() as i64;
        let mut score = line_count * 1_000 + (body.len() as i64);
        let lower = body.to_ascii_lowercase();

        fn bump(score: &mut i64, lower: &str, needle: &str, weight: i64) {
            if lower.contains(needle) {
                *score += weight;
            }
        }

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
            _ => {}
        }

        score
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

    #[derive(Debug, Clone)]
    struct CypressBlock {
        source_spec: PathBuf,
        source_stem: String,
        idx_in_file: usize,
        test_name: Option<String>,
        call: String,
        body: String,
    }

    fn extract_cypress_blocks(spec_path: &Path) -> Result<Vec<CypressBlock>, XtaskError> {
        let text = fs::read_to_string(spec_path).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to read cypress spec file {}: {err}",
                spec_path.display()
            ))
        })?;

        fn find_matching_paren_close(text: &str, open_paren: usize) -> Option<usize> {
            // Best-effort JS scanning to find the matching `)` for a call starting at `open_paren`.
            //
            // This intentionally ignores nested template literal `${...}` parsing; for our fixture
            // sources this is sufficient and prevents accidentally capturing backticks from later
            // tests when the call argument is not a template literal (e.g. `imgSnapshotTest(diagramCode, ...)`).
            let bytes = text.as_bytes();
            if bytes.get(open_paren) != Some(&b'(') {
                return None;
            }

            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            enum Mode {
                Normal,
                SingleQuote,
                DoubleQuote,
                Template,
                LineComment,
                BlockComment,
            }

            let mut mode = Mode::Normal;
            let mut depth: i32 = 1;
            let mut escaped = false;

            let mut i = open_paren + 1;
            while i < bytes.len() {
                let b = bytes[i];
                match mode {
                    Mode::Normal => {
                        if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
                            mode = Mode::LineComment;
                            i += 2;
                            continue;
                        }
                        if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
                            mode = Mode::BlockComment;
                            i += 2;
                            continue;
                        }
                        if b == b'\'' {
                            mode = Mode::SingleQuote;
                            escaped = false;
                            i += 1;
                            continue;
                        }
                        if b == b'"' {
                            mode = Mode::DoubleQuote;
                            escaped = false;
                            i += 1;
                            continue;
                        }
                        if b == b'`' {
                            mode = Mode::Template;
                            escaped = false;
                            i += 1;
                            continue;
                        }

                        if b == b'(' {
                            depth += 1;
                        } else if b == b')' {
                            depth -= 1;
                            if depth == 0 {
                                return Some(i);
                            }
                        }

                        i += 1;
                    }
                    Mode::SingleQuote => {
                        if escaped {
                            escaped = false;
                            i += 1;
                            continue;
                        }
                        if b == b'\\' {
                            escaped = true;
                            i += 1;
                            continue;
                        }
                        if b == b'\'' {
                            mode = Mode::Normal;
                        }
                        i += 1;
                    }
                    Mode::DoubleQuote => {
                        if escaped {
                            escaped = false;
                            i += 1;
                            continue;
                        }
                        if b == b'\\' {
                            escaped = true;
                            i += 1;
                            continue;
                        }
                        if b == b'"' {
                            mode = Mode::Normal;
                        }
                        i += 1;
                    }
                    Mode::Template => {
                        if escaped {
                            escaped = false;
                            i += 1;
                            continue;
                        }
                        if b == b'\\' {
                            escaped = true;
                            i += 1;
                            continue;
                        }
                        if b == b'`' {
                            mode = Mode::Normal;
                        }
                        i += 1;
                    }
                    Mode::LineComment => {
                        if b == b'\n' {
                            mode = Mode::Normal;
                        }
                        i += 1;
                    }
                    Mode::BlockComment => {
                        if b == b'*' && bytes.get(i + 1) == Some(&b'/') {
                            mode = Mode::Normal;
                            i += 2;
                            continue;
                        }
                        i += 1;
                    }
                }
            }
            None
        }

        let source_stem = spec_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // `regex` crate does not support backreferences; capture single-quoted and double-quoted
        // variants separately.
        let re_it_sq = Regex::new(r#"(?m)\bit\s*\(\s*'([^']*)'"#).map_err(|e| {
            XtaskError::SnapshotUpdateFailed(format!("invalid it() single-quote regex: {e}"))
        })?;
        let re_it_dq = Regex::new(r#"(?m)\bit\s*\(\s*"([^"]*)""#).map_err(|e| {
            XtaskError::SnapshotUpdateFailed(format!("invalid it() double-quote regex: {e}"))
        })?;
        let mut test_name: Option<String> = None;
        let mut it_positions: Vec<(usize, String)> = Vec::new();
        for cap in re_it_sq.captures_iter(&text) {
            if let (Some(m), Some(t)) = (cap.get(0), cap.get(1)) {
                it_positions.push((m.start(), t.as_str().to_string()));
            }
        }
        for cap in re_it_dq.captures_iter(&text) {
            if let (Some(m), Some(t)) = (cap.get(0), cap.get(1)) {
                it_positions.push((m.start(), t.as_str().to_string()));
            }
        }
        it_positions.sort_by_key(|(pos, _)| *pos);
        let mut next_it_idx = 0usize;

        let mut out: Vec<CypressBlock> = Vec::new();
        let mut idx_in_file = 0usize;
        for (call, needle) in [
            ("imgSnapshotTest", "imgSnapshotTest"),
            ("renderGraph", "renderGraph"),
        ] {
            let mut search_from = 0usize;
            while let Some(found) = text[search_from..].find(needle) {
                let abs = search_from + found;
                while next_it_idx + 1 < it_positions.len() && it_positions[next_it_idx + 1].0 < abs
                {
                    next_it_idx += 1;
                }
                if let Some((it_pos, name)) = it_positions.get(next_it_idx) {
                    if *it_pos < abs {
                        test_name = Some(name.clone());
                    }
                }

                // Find the opening paren and extract the first template literal after it.
                let after_call = abs + needle.len();
                let Some(open_paren) = text[after_call..].find('(').map(|o| after_call + o) else {
                    search_from = after_call;
                    continue;
                };
                let start = open_paren + 1;

                let Some(close_paren) = find_matching_paren_close(&text, open_paren) else {
                    search_from = start;
                    continue;
                };

                // Only scan within the call arguments; otherwise we can accidentally capture a
                // backtick string from a later `it()` block when the call argument itself isn't
                // a template literal.
                let args_slice = &text[start..close_paren];
                let use_last_template =
                    call == "renderGraph" && args_slice.trim_start().starts_with('[');
                let extracted = if use_last_template {
                    extract_last_template_literal(args_slice, 0)
                } else {
                    extract_first_template_literal(args_slice, 0)
                };
                if let Some((raw, end_rel)) = extracted {
                    out.push(CypressBlock {
                        source_spec: spec_path.to_path_buf(),
                        source_stem: source_stem.clone(),
                        idx_in_file,
                        test_name: test_name.clone(),
                        call: call.to_string(),
                        body: raw,
                    });
                    idx_in_file += 1;
                    search_from = start + end_rel;
                    continue;
                }

                search_from = close_paren + 1;
            }
        }

        Ok(out)
    }

    #[derive(Debug, Clone)]
    struct Candidate {
        block: CypressBlock,
        diagram_dir: String,
        fixtures_dir: PathBuf,
        stem: String,
        body: String,
        score: i64,
    }

    let reg = merman::detect::DetectorRegistry::default_mermaid_11_12_2_full();
    let mut spec_files: Vec<PathBuf> = Vec::new();
    collect_spec_files_recursively(&spec_root, &mut spec_files)?;
    spec_files.sort();

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    let mut existing_by_diagram: std::collections::HashMap<
        String,
        std::collections::HashMap<String, PathBuf>,
    > = std::collections::HashMap::new();

    for spec_path in spec_files {
        if let Some(f) = filter.as_deref() {
            let hay = spec_path.to_string_lossy();
            if !hay.contains(f) {
                // Still allow filtering by test name later; don't early-skip the file here.
            }
        }

        let blocks = extract_cypress_blocks(&spec_path)?;
        for b in blocks {
            let mut body = canonical_fixture_text(&normalize_cypress_fixture_text(&b.body));
            if body.trim().is_empty() {
                continue;
            }
            if let Some(min) = min_lines {
                if body.lines().count() < min {
                    continue;
                }
            }

            if let Some(f) = filter.as_deref() {
                let mut hay = spec_path.to_string_lossy().to_string();
                if let Some(t) = b.test_name.as_deref() {
                    hay.push(' ');
                    hay.push_str(t);
                }
                if !hay.contains(f) {
                    continue;
                }
            }

            let mut cfg = merman::MermaidConfig::default();
            let detected = match reg.detect_type(body.as_str(), &mut cfg) {
                Ok(t) => t,
                Err(_) => {
                    skipped.push(format!(
                        "skip (type not detected): {} (call={}, idx={})",
                        b.source_spec.display(),
                        b.call,
                        b.idx_in_file
                    ));
                    continue;
                }
            };
            let Some(diagram_dir) = normalize_diagram_dir(detected) else {
                skipped.push(format!(
                    "skip (unsupported detected type '{detected}'): {}",
                    b.source_spec.display()
                ));
                continue;
            };

            if diagram_dir == "zenuml" {
                continue;
            }
            if diagram != "all" && diagram_dir != diagram {
                continue;
            }

            // Keep `--with-baselines` aligned with the current parity hardening scope.
            //
            // We explicitly defer/skip cases that:
            // - require the ELK layout engine (`flowchart-elk`), which is out of scope for the
            //   headless layout engine in this repo
            // - exercise browser-only math rendering (`$$...$$`)
            // - are sourced from the upstream `errorDiagram` spec (these are intentionally-invalid
            //   inputs that should render as Mermaid "error" diagrams, not as flowcharts)
            if with_baselines && diagram_dir == "flowchart" {
                let spec_name = spec_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                if spec_name.contains("flowchart-elk.spec.") {
                    skipped.push(format!(
                        "skip (deferred for --with-baselines): {} (flowchart-elk spec)",
                        spec_path.display()
                    ));
                    continue;
                }
                if spec_name.contains("katex.spec.") {
                    skipped.push(format!(
                        "skip (deferred for --with-baselines): {} (katex spec)",
                        spec_path.display()
                    ));
                    continue;
                }
                if spec_name.contains("errorDiagram.spec.") {
                    skipped.push(format!(
                        "skip (deferred for --with-baselines): {} (errorDiagram spec)",
                        spec_path.display()
                    ));
                    continue;
                }
                if body.contains("$$") {
                    skipped.push(format!(
                        "skip (deferred for --with-baselines): {} (flowchart math)",
                        spec_path.display()
                    ));
                    continue;
                }
                if body
                    .lines()
                    .any(|l| l.trim_start().starts_with("flowchart-elk"))
                {
                    skipped.push(format!(
                        "skip (deferred for --with-baselines): {} (flowchart-elk diagram type)",
                        spec_path.display()
                    ));
                    continue;
                }
            }

            if diagram_dir == "architecture" {
                body = canonical_fixture_text(&normalize_architecture_beta_legacy_edges(&body));
            }

            let fixtures_dir = workspace_root.join("fixtures").join(&diagram_dir);
            if !fixtures_dir.is_dir() {
                skipped.push(format!(
                    "skip (fixtures dir missing): {}",
                    fixtures_dir.display()
                ));
                continue;
            }

            let source_slug = clamp_slug(slugify(&b.source_stem), 48);
            let test_slug = clamp_slug(slugify(b.test_name.as_deref().unwrap_or("example")), 64);
            let stem = format!(
                "upstream_cypress_{source_slug}_{test_slug}_{idx:03}",
                idx = b.idx_in_file + 1
            );

            let score = complexity_score(&body, &diagram_dir);
            candidates.push(Candidate {
                block: b,
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

    // Create `.mmd` fixtures (deduped by canonical body text).
    #[derive(Debug, Clone)]
    struct CreatedFixture {
        diagram_dir: String,
        stem: String,
        path: PathBuf,
        source_spec: PathBuf,
        source_idx_in_file: usize,
        source_call: String,
        source_test_name: Option<String>,
    }

    let mut created: Vec<CreatedFixture> = Vec::new();
    let mut imported = 0usize;

    for c in candidates {
        let existing = existing_by_diagram
            .entry(c.diagram_dir.clone())
            .or_insert_with(|| load_existing_fixtures(&c.fixtures_dir));
        if let Some(existing_path) = existing.get(&c.body) {
            skipped.push(format!(
                "skip (duplicate content): {} -> {}",
                c.block.source_spec.display(),
                existing_path.display()
            ));
            continue;
        }

        let out_path = c.fixtures_dir.join(format!("{}.mmd", c.stem));
        if out_path.exists() && !overwrite {
            skipped.push(format!("skip (already exists): {}", out_path.display()));
            continue;
        }
        let deferred_out_path = workspace_root
            .join("fixtures")
            .join("_deferred")
            .join(&c.diagram_dir)
            .join(format!("{}.mmd", c.stem));
        if deferred_out_path.exists() && !overwrite {
            skipped.push(format!(
                "skip (already deferred): {}",
                deferred_out_path.display()
            ));
            continue;
        }

        fs::write(&out_path, c.body.as_bytes()).map_err(|source| XtaskError::WriteFile {
            path: out_path.display().to_string(),
            source,
        })?;
        existing.insert(c.body.clone(), out_path.clone());

        created.push(CreatedFixture {
            diagram_dir: c.diagram_dir,
            stem: c.stem,
            path: out_path,
            source_spec: c.block.source_spec,
            source_idx_in_file: c.block.idx_in_file,
            source_call: c.block.call,
            source_test_name: c.block.test_name,
        });

        imported += 1;
        if let Some(max) = limit {
            if imported >= max {
                break;
            }
        }
    }

    if created.is_empty() {
        if !skipped.is_empty() {
            let mut dup = 0usize;
            let mut exists = 0usize;
            let mut deferred = 0usize;
            for s in &skipped {
                if s.starts_with("skip (duplicate content):") {
                    dup += 1;
                } else if s.starts_with("skip (already exists):") {
                    exists += 1;
                } else if s.starts_with("skip (already deferred):") {
                    deferred += 1;
                }
            }
            let mut msg = String::from("no fixtures were imported");
            if dup + exists + deferred > 0 {
                msg.push_str(&format!(
                    " (skipped: {dup} duplicate, {exists} exists, {deferred} deferred)"
                ));
            }
            msg.push_str(" (use --overwrite, or adjust --filter/--limit)");
            return Err(XtaskError::SnapshotUpdateFailed(msg));
        }
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
        let report_path = workspace_root
            .join("target")
            .join("import-upstream-cypress.report.txt");
        let mut report_lines: Vec<String> = Vec::new();

        fn deferred_with_baselines_reason(
            diagram_dir: &str,
            fixture_text: &str,
        ) -> Option<&'static str> {
            match diagram_dir {
                "flowchart" => {
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
                "sequence" => {
                    if fixture_text.contains("$$") {
                        return Some("sequence math (deferred)");
                    }
                }
                _ => {}
            }
            None
        }

        fn deferred_keep_baselines_reason(
            diagram_dir: &str,
            fixture_text: &str,
        ) -> Option<&'static str> {
            match diagram_dir {
                "flowchart" => {
                    // ELK layout is currently out of scope for the headless layout engine, but we
                    // still keep the upstream SVG baseline so the case remains traceable.
                    if fixture_text.contains("\n  layout: elk")
                        || fixture_text.contains("\nlayout: elk")
                    {
                        return Some("flowchart frontmatter config.layout=elk (deferred)");
                    }

                    // Mermaid also has a dedicated `flowchart-elk` diagram type.
                    // Keep these fixtures in `_deferred` until we implement ELK layout parity.
                    if fixture_text
                        .lines()
                        .any(|l| l.trim_start().starts_with("flowchart-elk"))
                    {
                        return Some("flowchart diagram type flowchart-elk (deferred)");
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

        fn defer_fixture_files_keep_baselines(workspace_root: &Path, f: &CreatedFixture) {
            let deferred_dir = workspace_root
                .join("fixtures")
                .join("_deferred")
                .join(&f.diagram_dir);
            let deferred_svg_dir = workspace_root
                .join("fixtures")
                .join("_deferred")
                .join("upstream-svgs")
                .join(&f.diagram_dir);
            let _ = fs::create_dir_all(&deferred_dir);
            let _ = fs::create_dir_all(&deferred_svg_dir);

            let deferred_mmd_path = deferred_dir.join(format!("{}.mmd", f.stem));
            let _ = fs::remove_file(&deferred_mmd_path);
            let _ = fs::rename(&f.path, &deferred_mmd_path);

            let svg_path = workspace_root
                .join("fixtures")
                .join("upstream-svgs")
                .join(&f.diagram_dir)
                .join(format!("{}.svg", f.stem));
            let deferred_svg_path = deferred_svg_dir.join(format!("{}.svg", f.stem));
            let _ = fs::remove_file(&deferred_svg_path);
            let _ = fs::rename(&svg_path, &deferred_svg_path);

            // We do not keep snapshots for deferred fixtures in the main fixture corpus.
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

        let mut kept: Vec<CreatedFixture> = Vec::with_capacity(created.len());
        for f in &created {
            let fixture_text = match fs::read_to_string(&f.path) {
                Ok(v) => v,
                Err(err) => {
                    report_lines.push(format!(
                        "READ_FIXTURE_FAILED\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\terr={err}",
                        f.diagram_dir,
                        f.stem,
                        f.source_spec.display(),
                        f.source_idx_in_file,
                        f.source_call,
                        f.source_test_name.clone().unwrap_or_default(),
                    ));
                    skipped.push(format!(
                        "skip (failed to read imported fixture): {} ({err})",
                        f.path.display(),
                    ));
                    cleanup_fixture_files(&workspace_root, f);
                    continue;
                }
            };
            if let Some(reason) = deferred_with_baselines_reason(&f.diagram_dir, &fixture_text) {
                report_lines.push(format!(
                    "DEFERRED_WITH_BASELINES\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\treason={reason}",
                    f.diagram_dir,
                    f.stem,
                    f.source_spec.display(),
                    f.source_idx_in_file,
                    f.source_call,
                    f.source_test_name.clone().unwrap_or_default(),
                ));
                skipped.push(format!(
                    "skip (deferred for --with-baselines): {} ({reason})",
                    f.path.display(),
                ));
                cleanup_fixture_files(&workspace_root, f);
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
            match super::gen_upstream_svgs(svg_args) {
                Ok(()) => {}
                Err(XtaskError::UpstreamSvgFailed(msg)) => {
                    report_lines.push(format!(
                        "UPSTREAM_SVG_FAILED\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\tmsg={}",
                        f.diagram_dir,
                        f.stem,
                        f.source_spec.display(),
                        f.source_idx_in_file,
                        f.source_call,
                        f.source_test_name.clone().unwrap_or_default(),
                        msg.lines().next().unwrap_or("unknown upstream error"),
                    ));
                    skipped.push(format!(
                        "skip (upstream svg failed): {} ({})",
                        f.path.display(),
                        msg.lines().next().unwrap_or("unknown upstream error")
                    ));
                    cleanup_fixture_files(&workspace_root, f);
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
                    "UPSTREAM_SVG_SUSPICIOUS_BLANK\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}",
                    f.diagram_dir,
                    f.stem,
                    f.source_spec.display(),
                    f.source_idx_in_file,
                    f.source_call,
                    f.source_test_name.clone().unwrap_or_default(),
                ));
                skipped.push(format!(
                    "skip (suspicious upstream svg output): {} (blank 16x16-like svg)",
                    f.path.display(),
                ));
                cleanup_fixture_files(&workspace_root, f);
                continue;
            }

            if let Some(reason) = deferred_keep_baselines_reason(&f.diagram_dir, &fixture_text) {
                report_lines.push(format!(
                    "DEFERRED_WITH_BASELINES\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\treason={reason}",
                    f.diagram_dir,
                    f.stem,
                    f.source_spec.display(),
                    f.source_idx_in_file,
                    f.source_call,
                    f.source_test_name.clone().unwrap_or_default(),
                ));
                skipped.push(format!(
                    "skip (deferred for --with-baselines): {} ({reason})",
                    f.path.display(),
                ));
                defer_fixture_files_keep_baselines(&workspace_root, f);
                continue;
            }

            if let Err(err) = super::update_snapshots(vec![
                "--diagram".to_string(),
                f.diagram_dir.clone(),
                "--filter".to_string(),
                f.stem.clone(),
            ]) {
                report_lines.push(format!(
                    "SNAPSHOT_UPDATE_FAILED\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\terr={err}",
                    f.diagram_dir,
                    f.stem,
                    f.source_spec.display(),
                    f.source_idx_in_file,
                    f.source_call,
                    f.source_test_name.clone().unwrap_or_default(),
                ));
                skipped.push(format!(
                    "skip (snapshot update failed): {} ({err})",
                    f.path.display(),
                ));
                cleanup_fixture_files(&workspace_root, f);
                continue;
            }
            if let Err(err) = super::update_layout_snapshots(vec![
                "--diagram".to_string(),
                f.diagram_dir.clone(),
                "--filter".to_string(),
                f.stem.clone(),
            ]) {
                report_lines.push(format!(
                    "LAYOUT_SNAPSHOT_UPDATE_FAILED\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\terr={err}",
                    f.diagram_dir,
                    f.stem,
                    f.source_spec.display(),
                    f.source_idx_in_file,
                    f.source_call,
                    f.source_test_name.clone().unwrap_or_default(),
                ));
                skipped.push(format!(
                    "skip (layout snapshot update failed): {} ({err})",
                    f.path.display(),
                ));
                cleanup_fixture_files(&workspace_root, f);
                continue;
            }

            kept.push(f.clone());
        }
        created = kept;

        if !report_lines.is_empty() {
            if let Some(parent) = report_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let header = format!(
                "# import-upstream-cypress report (Mermaid@11.12.2)\n# generated_at={}\n",
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%z")
            );
            let mut out = String::new();
            out.push_str(&header);
            out.push_str(&report_lines.join("\n"));
            out.push('\n');
            let _ = fs::write(&report_path, out);
            eprintln!("Wrote import report: {}", report_path.display());
        }

        if created.is_empty() {
            return Err(XtaskError::SnapshotUpdateFailed(
                "no fixtures were imported (all candidates failed upstream rendering)".to_string(),
            ));
        }
    }

    eprintln!("Imported {} fixtures:", created.len());
    for f in &created {
        eprintln!("  {}", f.path.display());
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
            super::gen_upstream_svgs(svg_args)?;
            super::update_snapshots(vec![
                "--diagram".to_string(),
                diagram_dir.clone(),
                "--filter".to_string(),
                stem.clone(),
            ])?;
            super::update_layout_snapshots(vec![
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

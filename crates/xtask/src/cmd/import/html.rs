use super::*;

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
            if let Err(err) = super::super::gen_upstream_svgs(svg_args) {
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

            if let Err(err) = super::super::update_snapshots(vec![
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

            if let Err(err) = super::super::update_layout_snapshots(vec![
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

use super::*;

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

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let default_src_root = workspace_root
        .join("repo-ref")
        .join("mermaid")
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
            "upstream package src root not found: {} (expected repo-ref checkout of mermaid@11.12.2)",
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
            "architecture-beta" => Some("architecture".to_string()),
            "architecture" | "block" | "c4" | "gantt" | "info" | "kanban" | "mindmap"
            | "packet" | "pie" | "radar" | "sankey" | "sequence" | "timeline" | "treemap" => {
                Some(detected.to_string())
            }
            _ => None,
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

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum InterpMode {
        Normal,
        SingleQuote,
        DoubleQuote,
        LineComment,
        BlockComment,
    }

    fn scan_template_literal(text: &str, open_tick: usize) -> Option<(Option<String>, usize)> {
        let bytes = text.as_bytes();
        if bytes.get(open_tick) != Some(&b'`') {
            return None;
        }

        let mut i = open_tick + 1;
        let mut out = String::new();
        let mut escaped = false;
        let mut _has_interpolation = false;
        let mut interp_depth: i32 = 0;
        let mut mode = InterpMode::Normal;

        while i < bytes.len() {
            let b = bytes[i];

            if interp_depth == 0 {
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
                    let end = i + 1;
                    return Some((Some(out), end));
                }
                if b == b'$' && bytes.get(i + 1) == Some(&b'{') {
                    _has_interpolation = true;
                    // Replace `${...}` with a deterministic placeholder so we can still extract a
                    // stable Mermaid definition from tests that build diagrams with variables.
                    // Downstream diagram detection will discard non-Mermaid strings.
                    //
                    // Heuristic: if the interpolation is appended directly to an identifier-like
                    // token (e.g. `C4Context${suffix}`), inserting `X` would often break upstream
                    // parsing (`C4ContextX` is not a recognized macro). In that case, prefer
                    // eliding the interpolation entirely so the surrounding token stays valid.
                    let prev_is_ident = out
                        .chars()
                        .last()
                        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
                    if !prev_is_ident {
                        out.push('X');
                    }
                    interp_depth = 1;
                    mode = InterpMode::Normal;
                    i += 2;
                    continue;
                }

                out.push(b as char);
                i += 1;
                continue;
            }

            match mode {
                InterpMode::Normal => {
                    if b == b'\'' {
                        mode = InterpMode::SingleQuote;
                        escaped = false;
                        i += 1;
                        continue;
                    }
                    if b == b'"' {
                        mode = InterpMode::DoubleQuote;
                        escaped = false;
                        i += 1;
                        continue;
                    }
                    if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
                        mode = InterpMode::LineComment;
                        i += 2;
                        continue;
                    }
                    if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
                        mode = InterpMode::BlockComment;
                        i += 2;
                        continue;
                    }
                    if b == b'`' {
                        // Nested template literal inside interpolation. Skip it.
                        if let Some((_, end)) = scan_template_literal(text, i) {
                            i = end;
                            continue;
                        }
                    }

                    if b == b'{' {
                        interp_depth += 1;
                    } else if b == b'}' {
                        interp_depth -= 1;
                        if interp_depth == 0 {
                            i += 1;
                            continue;
                        }
                    }
                    i += 1;
                }
                InterpMode::SingleQuote => {
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
                        mode = InterpMode::Normal;
                    }
                    i += 1;
                }
                InterpMode::DoubleQuote => {
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
                        mode = InterpMode::Normal;
                    }
                    i += 1;
                }
                InterpMode::LineComment => {
                    if b == b'\n' {
                        mode = InterpMode::Normal;
                    }
                    i += 1;
                }
                InterpMode::BlockComment => {
                    if b == b'*' && bytes.get(i + 1) == Some(&b'/') {
                        mode = InterpMode::Normal;
                        i += 2;
                        continue;
                    }
                    i += 1;
                }
            }
        }

        None
    }

    fn extract_all_template_literals(text: &str) -> Vec<String> {
        let bytes = text.as_bytes();
        let mut out: Vec<String> = Vec::new();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'`' {
                if let Some((content, end)) = scan_template_literal(text, i) {
                    if let Some(content) = content {
                        out.push(content);
                    }
                    i = end;
                    continue;
                }
            }
            i += 1;
        }
        out
    }

    fn extract_all_string_literals(text: &str) -> Vec<String> {
        fn hex_val(b: u8) -> Option<u8> {
            match b {
                b'0'..=b'9' => Some(b - b'0'),
                b'a'..=b'f' => Some(10 + (b - b'a')),
                b'A'..=b'F' => Some(10 + (b - b'A')),
                _ => None,
            }
        }

        fn scan_string_literal(
            text: &str,
            open_quote: usize,
            quote: u8,
        ) -> Option<(String, usize)> {
            let bytes = text.as_bytes();
            if bytes.get(open_quote) != Some(&quote) {
                return None;
            }

            let mut out: Vec<u8> = Vec::new();
            let mut i = open_quote + 1;
            let mut escaped = false;

            while i < bytes.len() {
                let b = bytes[i];

                if escaped {
                    escaped = false;
                    match b {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'\\' => out.push(b'\\'),
                        b'\'' => out.push(b'\''),
                        b'"' => out.push(b'"'),
                        b'`' => out.push(b'`'),
                        b'0' => out.push(0),
                        b'\n' => {
                            // Line continuation: `"...\\\n..."` in JS.
                        }
                        b'\r' => {
                            // Handle `\\\r\n` continuation.
                            if bytes.get(i + 1) == Some(&b'\n') {
                                i += 1;
                            }
                        }
                        b'x' => {
                            let hi = bytes.get(i + 1).copied().and_then(hex_val);
                            let lo = bytes.get(i + 2).copied().and_then(hex_val);
                            if let (Some(hi), Some(lo)) = (hi, lo) {
                                out.push((hi << 4) | lo);
                                i += 2;
                            } else {
                                out.extend_from_slice(b"x");
                            }
                        }
                        b'u' => {
                            // `\uXXXX` or `\u{...}`.
                            if bytes.get(i + 1) == Some(&b'{') {
                                let mut j = i + 2;
                                let mut v: u32 = 0;
                                let mut saw = false;
                                while j < bytes.len() {
                                    if bytes[j] == b'}' {
                                        break;
                                    }
                                    let Some(h) = hex_val(bytes[j]) else {
                                        break;
                                    };
                                    saw = true;
                                    v = (v << 4) | (h as u32);
                                    j += 1;
                                }
                                if saw && j < bytes.len() && bytes[j] == b'}' {
                                    if let Some(ch) = char::from_u32(v) {
                                        let mut buf = [0u8; 4];
                                        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                                        i = j;
                                    } else {
                                        out.extend_from_slice(b"u");
                                    }
                                } else {
                                    out.extend_from_slice(b"u");
                                }
                            } else {
                                let d1 = bytes.get(i + 1).copied().and_then(hex_val);
                                let d2 = bytes.get(i + 2).copied().and_then(hex_val);
                                let d3 = bytes.get(i + 3).copied().and_then(hex_val);
                                let d4 = bytes.get(i + 4).copied().and_then(hex_val);
                                if let (Some(d1), Some(d2), Some(d3), Some(d4)) = (d1, d2, d3, d4) {
                                    let v: u32 = ((d1 as u32) << 12)
                                        | ((d2 as u32) << 8)
                                        | ((d3 as u32) << 4)
                                        | (d4 as u32);
                                    if let Some(ch) = char::from_u32(v) {
                                        let mut buf = [0u8; 4];
                                        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                                        i += 4;
                                    } else {
                                        out.extend_from_slice(b"u");
                                    }
                                } else {
                                    out.extend_from_slice(b"u");
                                }
                            }
                        }
                        _ => out.push(b),
                    }
                    i += 1;
                    continue;
                }

                if b == b'\\' {
                    escaped = true;
                    i += 1;
                    continue;
                }

                if b == quote {
                    let s = String::from_utf8_lossy(&out).to_string();
                    return Some((s, i + 1));
                }

                // Invalid JS string (raw newline). Bail out to avoid swallowing the rest of the file.
                if b == b'\n' || b == b'\r' {
                    return None;
                }

                out.push(b);
                i += 1;
            }

            None
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        enum Mode {
            Normal,
            LineComment,
            BlockComment,
        }

        let bytes = text.as_bytes();
        let mut out: Vec<String> = Vec::new();
        let mut i = 0usize;
        let mut mode = Mode::Normal;
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
                    if b == b'`' {
                        // Skip template literals entirely so we don't accidentally scan quotes within
                        // them here. Template literals are extracted separately.
                        if let Some((_, end)) = scan_template_literal(text, i) {
                            i = end;
                            continue;
                        }
                    }
                    if b == b'\'' {
                        if let Some((content, end)) = scan_string_literal(text, i, b'\'') {
                            out.push(content);
                            i = end;
                            continue;
                        }
                    }
                    if b == b'"' {
                        if let Some((content, end)) = scan_string_literal(text, i, b'"') {
                            out.push(content);
                            i = end;
                            continue;
                        }
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

        out
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

    let reg = merman::detect::DetectorRegistry::default_mermaid_11_12_2_full();

    let mut spec_files: Vec<PathBuf> = Vec::new();
    collect_test_files_recursively(&src_root, &mut spec_files)?;
    spec_files.sort();

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for spec_path in spec_files {
        let hay = spec_path.to_string_lossy();
        if let Some(f) = filter.as_deref() {
            if !hay.contains(f) {
                // Still allow matching by diagram heading later; template strings have no heading here.
                continue;
            }
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
        let mut blocks = extract_all_template_literals(&text);
        blocks.extend(extract_all_string_literals(&text));
        if blocks.is_empty() {
            continue;
        }

        let source_stem = spec_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let source_slug = clamp_slug(slugify(&source_stem), 48);

        for (idx, raw) in blocks.into_iter().enumerate() {
            let body = canonical_fixture_text(&raw);
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
                Err(_) => continue,
            };
            let Some(diagram_dir) = normalize_diagram_dir(detected) else {
                continue;
            };
            if diagram_dir == "zenuml" {
                continue;
            }
            if diagram != "all" && diagram_dir != diagram {
                continue;
            }

            let stem = format!("upstream_pkgtests_{source_slug}_{idx:03}", idx = idx + 1);
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

    #[derive(Debug, Clone)]
    struct CreatedFixture {
        diagram_dir: String,
        stem: String,
        path: PathBuf,
    }

    let mut created: Vec<CreatedFixture> = Vec::new();

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
                "skip (duplicate content): {} (idx={}) -> {}",
                c.source_path.display(),
                c.idx_in_file + 1,
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
        return Err(XtaskError::SnapshotUpdateFailed(
            "no fixtures were imported (use --diagram <name> and optionally --filter/--limit)"
                .to_string(),
        ));
    }

    if with_baselines {
        let mut kept: Vec<CreatedFixture> = Vec::with_capacity(created.len());

        fn is_upstream_error_svg(svg_path: &Path) -> bool {
            let Ok(svg) = fs::read_to_string(svg_path) else {
                return false;
            };
            svg.contains("aria-roledescription=\"error\"")
        }

        fn cleanup_fixture_and_svg(workspace_root: &Path, f: &CreatedFixture) {
            let _ = fs::remove_file(&f.path);
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
            let _ = fs::remove_file(
                workspace_root
                    .join("fixtures")
                    .join("upstream-svgs")
                    .join(&f.diagram_dir)
                    .join(format!("{}.svg", f.stem)),
            );
        }

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
                    "skip (upstream svg generation failed): {} ({err})",
                    f.path.display()
                ));
                cleanup_fixture_and_svg(&workspace_root, f);
                continue;
            }

            let svg_path = workspace_root
                .join("fixtures")
                .join("upstream-svgs")
                .join(&f.diagram_dir)
                .join(format!("{}.svg", f.stem));
            if is_upstream_error_svg(&svg_path) {
                skipped.push(format!(
                    "skip (upstream rendered error diagram): {}",
                    f.path.display()
                ));
                cleanup_fixture_and_svg(&workspace_root, f);
                continue;
            }

            if let Err(err) = super::super::update_snapshots(vec![
                "--diagram".to_string(),
                f.diagram_dir.clone(),
                "--filter".to_string(),
                f.stem.clone(),
            ]) {
                skipped.push(format!(
                    "skip (snapshot update failed): {} ({err})",
                    f.path.display()
                ));
                cleanup_fixture_and_svg(&workspace_root, f);
                continue;
            }

            if let Err(err) = super::super::update_layout_snapshots(vec![
                "--diagram".to_string(),
                f.diagram_dir.clone(),
                "--filter".to_string(),
                f.stem.clone(),
            ]) {
                skipped.push(format!(
                    "skip (layout snapshot update failed): {} ({err})",
                    f.path.display()
                ));
                cleanup_fixture_and_svg(&workspace_root, f);
                continue;
            }

            // Parity gate (matches `xtask verify`): keep only fixtures that pass SVG DOM parity.
            if let Err(err) = super::super::compare_all_svgs(vec![
                "--check-dom".to_string(),
                "--dom-mode".to_string(),
                "parity".to_string(),
                "--dom-decimals".to_string(),
                "3".to_string(),
                "--diagram".to_string(),
                f.diagram_dir.clone(),
                "--filter".to_string(),
                f.stem.clone(),
            ]) {
                skipped.push(format!(
                    "skip (svg dom parity mismatch): {} ({err})",
                    f.path.display()
                ));
                cleanup_fixture_and_svg(&workspace_root, f);
                continue;
            }

            kept.push(f.clone());
        }

        created = kept;
    }

    if created.is_empty() {
        return Err(XtaskError::SnapshotUpdateFailed(
            "no fixtures were kept after baseline/snapshot/parity checks".to_string(),
        ));
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

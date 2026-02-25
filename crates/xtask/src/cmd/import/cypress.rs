use super::*;

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
            "upstream cypress spec root not found: {} (expected repo-ref checkout of mermaid@11.12.3)",
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
        // Some Cypress specs end blocks with a line that is "blank" but indented (spaces only).
        // For indentation-sensitive grammars (notably treemap-beta), Mermaid treats this as a
        // parse error. Trim leading/trailing whitespace-only lines to keep fixtures stable.
        let mut lines: Vec<&str> = s.lines().collect();
        while matches!(lines.first(), Some(l) if l.trim().is_empty()) {
            lines.remove(0);
        }
        while matches!(lines.last(), Some(l) if l.trim().is_empty()) {
            lines.pop();
        }
        let s = lines.join("\n");
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

    fn is_ws_or_newline_byte(b: u8) -> bool {
        matches!(b, b' ' | b'\t' | b'\n' | b'\r')
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
        options_obj: Option<String>,
    }

    fn extract_cypress_blocks(spec_path: &Path) -> Result<Vec<CypressBlock>, XtaskError> {
        let text = fs::read_to_string(spec_path).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to read cypress spec file {}: {err}",
                spec_path.display()
            ))
        })?;
        let bytes = text.as_bytes();

        fn is_ident_byte(b: u8) -> bool {
            b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        enum ArrayToken {
            Str(String),
            Ident(String),
        }

        fn is_ws_byte(b: u8) -> bool {
            matches!(b, b' ' | b'\t' | b'\n' | b'\r')
        }

        fn parse_string_lit(bytes: &[u8], mut i: usize, quote: u8) -> Option<(String, usize)> {
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
                        b'\'' => out.push('\''),
                        b'"' => out.push('"'),
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
                if b == quote {
                    return Some((out, i + 1));
                }
                out.push(b as char);
                i += 1;
            }
            None
        }

        fn parse_ident(bytes: &[u8], mut i: usize) -> (String, usize) {
            let start = i;
            while i < bytes.len() && is_ident_byte(bytes[i]) {
                i += 1;
            }
            (String::from_utf8_lossy(&bytes[start..i]).to_string(), i)
        }

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

        fn find_matching_brace_close(text: &str, open_brace: usize) -> Option<usize> {
            let bytes = text.as_bytes();
            if bytes.get(open_brace) != Some(&b'{') {
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

            let mut i = open_brace + 1;
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
                        if b == b'{' {
                            depth += 1;
                        } else if b == b'}' {
                            depth -= 1;
                            if depth == 0 {
                                return Some(i + 1);
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

        fn extract_second_arg_object_literal(
            args_slice: &str,
            after_first_arg: usize,
        ) -> Option<String> {
            let bytes = args_slice.as_bytes();
            let mut i = after_first_arg;

            while i < bytes.len() && is_ws_or_newline_byte(bytes[i]) {
                i += 1;
            }
            if bytes.get(i) != Some(&b',') {
                return None;
            }
            i += 1;
            while i < bytes.len() && is_ws_or_newline_byte(bytes[i]) {
                i += 1;
            }
            if bytes.get(i) != Some(&b'{') {
                return None;
            }
            let Some(end) = find_matching_brace_close(args_slice, i) else {
                return None;
            };
            Some(args_slice[i..end].to_string())
        }

        fn collect_const_arrays(text: &str) -> std::collections::HashMap<String, Vec<ArrayToken>> {
            let bytes = text.as_bytes();

            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            enum Mode {
                Normal,
                SingleQuote,
                DoubleQuote,
                Template,
                LineComment,
                BlockComment,
            }

            let mut out: std::collections::HashMap<String, Vec<ArrayToken>> =
                std::collections::HashMap::new();
            let mut mode = Mode::Normal;
            let mut escaped = false;

            let mut i = 0usize;
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

                        if bytes.get(i..i + 5) == Some(b"const") {
                            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
                            let after_ok = !bytes.get(i + 5).is_some_and(|c| is_ident_byte(*c));
                            if !before_ok || !after_ok {
                                i += 1;
                                continue;
                            }

                            let mut j = i + 5;
                            while bytes.get(j).is_some_and(|c| is_ws_byte(*c)) {
                                j += 1;
                            }
                            if !bytes.get(j).is_some_and(|c| is_ident_byte(*c)) {
                                i += 1;
                                continue;
                            }
                            let (name, mut k) = parse_ident(bytes, j);

                            while k < bytes.len() {
                                if bytes[k] == b'/' && bytes.get(k + 1) == Some(&b'/') {
                                    while k < bytes.len() && bytes[k] != b'\n' {
                                        k += 1;
                                    }
                                    continue;
                                }
                                if bytes[k] == b'/' && bytes.get(k + 1) == Some(&b'*') {
                                    k += 2;
                                    while k + 1 < bytes.len() {
                                        if bytes[k] == b'*' && bytes[k + 1] == b'/' {
                                            k += 2;
                                            break;
                                        }
                                        k += 1;
                                    }
                                    continue;
                                }
                                if bytes[k] == b'=' {
                                    break;
                                }
                                if bytes[k] == b'\n' {
                                    break;
                                }
                                k += 1;
                            }
                            if bytes.get(k) != Some(&b'=') {
                                i += 1;
                                continue;
                            }
                            k += 1;
                            while bytes.get(k).is_some_and(|c| is_ws_byte(*c)) {
                                k += 1;
                            }
                            if bytes.get(k) != Some(&b'[') {
                                i += 1;
                                continue;
                            }

                            let mut depth = 1i32;
                            let mut tokens: Vec<ArrayToken> = Vec::new();
                            let mut m = k + 1;
                            let mut inner_mode = Mode::Normal;
                            let mut inner_escaped = false;
                            while m < bytes.len() {
                                let c = bytes[m];
                                match inner_mode {
                                    Mode::Normal => {
                                        if c == b'/' && bytes.get(m + 1) == Some(&b'/') {
                                            inner_mode = Mode::LineComment;
                                            m += 2;
                                            continue;
                                        }
                                        if c == b'/' && bytes.get(m + 1) == Some(&b'*') {
                                            inner_mode = Mode::BlockComment;
                                            m += 2;
                                            continue;
                                        }
                                        if c == b'\'' || c == b'"' {
                                            let quote = c;
                                            if let Some((s, next)) =
                                                parse_string_lit(bytes, m + 1, quote)
                                            {
                                                tokens.push(ArrayToken::Str(s));
                                                m = next;
                                                continue;
                                            }
                                        }
                                        if is_ident_byte(c) {
                                            let (id, next) = parse_ident(bytes, m);
                                            tokens.push(ArrayToken::Ident(id));
                                            m = next;
                                            continue;
                                        }
                                        if c == b'[' {
                                            depth += 1;
                                        } else if c == b']' {
                                            depth -= 1;
                                            if depth == 0 {
                                                break;
                                            }
                                        }
                                        m += 1;
                                    }
                                    Mode::SingleQuote | Mode::DoubleQuote | Mode::Template => {
                                        if inner_escaped {
                                            inner_escaped = false;
                                            m += 1;
                                            continue;
                                        }
                                        if c == b'\\' {
                                            inner_escaped = true;
                                            m += 1;
                                            continue;
                                        }
                                        if (inner_mode == Mode::SingleQuote && c == b'\'')
                                            || (inner_mode == Mode::DoubleQuote && c == b'"')
                                            || (inner_mode == Mode::Template && c == b'`')
                                        {
                                            inner_mode = Mode::Normal;
                                        }
                                        m += 1;
                                    }
                                    Mode::LineComment => {
                                        if c == b'\n' {
                                            inner_mode = Mode::Normal;
                                        }
                                        m += 1;
                                    }
                                    Mode::BlockComment => {
                                        if c == b'*' && bytes.get(m + 1) == Some(&b'/') {
                                            inner_mode = Mode::Normal;
                                            m += 2;
                                            continue;
                                        }
                                        m += 1;
                                    }
                                }
                            }

                            if depth == 0 && !tokens.is_empty() {
                                out.insert(name, tokens);
                            }
                            i = m;
                            continue;
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

            out
        }

        fn array_tokens_to_strings(tokens: &[ArrayToken]) -> Vec<String> {
            tokens
                .iter()
                .filter_map(|t| match t {
                    ArrayToken::Str(s) => Some(s.clone()),
                    _ => None,
                })
                .collect()
        }

        fn array_tokens_to_idents(tokens: &[ArrayToken]) -> Vec<String> {
            tokens
                .iter()
                .filter_map(|t| match t {
                    ArrayToken::Ident(s) => Some(s.clone()),
                    _ => None,
                })
                .collect()
        }

        fn synthesize_flowchart_shape_alias_blocks(
            spec_path: &Path,
            source_stem: &str,
            text: &str,
        ) -> Result<Vec<CypressBlock>, XtaskError> {
            let arrays = collect_const_arrays(text);
            let Some(alias_sets_tokens) = arrays.get("aliasSets") else {
                return Ok(Vec::new());
            };
            let alias_sets = array_tokens_to_idents(alias_sets_tokens);
            if alias_sets.is_empty() {
                return Ok(Vec::new());
            }

            let mut out: Vec<CypressBlock> = Vec::new();
            for (idx, set_name) in alias_sets.iter().enumerate() {
                let Some(set_tokens) = arrays.get(set_name) else {
                    return Err(XtaskError::SnapshotUpdateFailed(format!(
                        "failed to synthesize cypress blocks from {}: missing const array {set_name}",
                        spec_path.display()
                    )));
                };
                let aliases = array_tokens_to_strings(set_tokens);
                if aliases.is_empty() {
                    continue;
                }

                let mut body = String::from("flowchart\n");
                for (i, a) in aliases.iter().enumerate() {
                    body.push_str(&format!(" n{i}@{{ shape: {a}, label: \"{a}\" }}\n"));
                }

                out.push(CypressBlock {
                    source_spec: spec_path.to_path_buf(),
                    source_stem: source_stem.to_string(),
                    idx_in_file: idx,
                    test_name: Some(format!("shape-alias {set_name}")),
                    call: "imgSnapshotTest".to_string(),
                    body,
                    options_obj: None,
                });
            }
            Ok(out)
        }

        fn synthesize_flowchart_shapes_blocks(
            spec_path: &Path,
            source_stem: &str,
            text: &str,
            aggregate_name: &str,
        ) -> Result<Vec<CypressBlock>, XtaskError> {
            let arrays = collect_const_arrays(text);

            let looks = arrays
                .get("looks")
                .map(|t| array_tokens_to_strings(t))
                .unwrap_or_default();
            if !looks.iter().any(|l| l == "classic") {
                return Ok(Vec::new());
            }

            let directions = arrays
                .get("directions")
                .map(|t| array_tokens_to_strings(t))
                .unwrap_or_default();
            if directions.is_empty() {
                return Ok(Vec::new());
            }

            let Some(sets_tokens) = arrays.get(aggregate_name) else {
                return Ok(Vec::new());
            };
            let set_names = array_tokens_to_idents(sets_tokens);
            if set_names.is_empty() {
                return Ok(Vec::new());
            }

            let variants: [(&str, bool); 8] = [
                ("nolabel", false),
                ("label", false),
                ("allpairs", false),
                ("longlabel", false),
                ("md_html_true", false),
                ("md_html_false", true),
                ("styles", false),
                ("classdef", false),
            ];

            let mut out: Vec<CypressBlock> = Vec::new();
            let mut idx_in_file = 0usize;
            for dir in &directions {
                for set_name in &set_names {
                    let Some(set_tokens) = arrays.get(set_name) else {
                        return Err(XtaskError::SnapshotUpdateFailed(format!(
                            "failed to synthesize cypress blocks from {}: missing const array {set_name}",
                            spec_path.display()
                        )));
                    };
                    let shapes = array_tokens_to_strings(set_tokens);
                    if shapes.is_empty() {
                        continue;
                    }

                    for (variant, needs_html_labels_false) in variants {
                        let mut code = String::new();
                        if needs_html_labels_false {
                            code.push_str("---\n");
                            code.push_str("config:\n");
                            code.push_str("  htmlLabels: false\n");
                            code.push_str("  flowchart:\n");
                            code.push_str("    htmlLabels: false\n");
                            code.push_str("---\n");
                        }

                        code.push_str(&format!("flowchart {dir}\n"));

                        match variant {
                            "nolabel" => {
                                for (i, s) in shapes.iter().enumerate() {
                                    code.push_str(&format!(
                                        "  n{i} --> n{i}{i}@{{ shape: {s} }}\n"
                                    ));
                                }
                            }
                            "label" => {
                                for (i, s) in shapes.iter().enumerate() {
                                    code.push_str(&format!(
                                        "  n{i} --> n{i}{i}@{{ shape: {s}, label: 'This is a label for {s} shape' }}\n"
                                    ));
                                }
                            }
                            "allpairs" => {
                                for (i, s) in shapes.iter().enumerate() {
                                    code.push_str(&format!(
                                        "  n{i}{i}@{{ shape: {s}, label: 'This is a label for {s} shape' }}\n"
                                    ));
                                }
                                for i in 0..shapes.len() {
                                    for j in (i + 1)..shapes.len() {
                                        code.push_str(&format!("  n{i}{i} --> n{j}{j}\n"));
                                    }
                                }
                            }
                            "longlabel" => {
                                for (i, s) in shapes.iter().enumerate() {
                                    code.push_str(&format!(
                                        "  n{i} --> n{i}{i}@{{ shape: {s}, label: 'This is a very very very very very long long long label for {s} shape' }}\n"
                                    ));
                                }
                            }
                            "md_html_true" | "md_html_false" => {
                                for (i, s) in shapes.iter().enumerate() {
                                    code.push_str(&format!(
                                        "  n{i} --> n{i}{i}@{{ shape: {s}, label: 'This is **bold** </br>and <strong>strong</strong> for {s} shape' }}\n"
                                    ));
                                }
                            }
                            "styles" => {
                                for (i, s) in shapes.iter().enumerate() {
                                    code.push_str(&format!(
                                        "  n{i} --> n{i}{i}@{{ shape: {s}, label: 'new {s} shape' }}\n"
                                    ));
                                    code.push_str(&format!(
                                        "  style n{i}{i} fill:#f9f,stroke:#333,stroke-width:4px \n"
                                    ));
                                }
                            }
                            "classdef" => {
                                code.push_str("  classDef customClazz fill:#bbf,stroke:#f66,stroke-width:2px,color:#fff,stroke-dasharray: 5 5\n");
                                for (i, s) in shapes.iter().enumerate() {
                                    code.push_str(&format!(
                                        "  n{i} --> n{i}{i}@{{ shape: {s}, label: 'new {s} shape' }}\n"
                                    ));
                                    code.push_str(&format!("  n{i}{i}:::customClazz\n"));
                                }
                            }
                            _ => {}
                        }

                        out.push(CypressBlock {
                            source_spec: spec_path.to_path_buf(),
                            source_stem: source_stem.to_string(),
                            idx_in_file,
                            test_name: Some(format!("{aggregate_name} {set_name} {dir} {variant}")),
                            call: "imgSnapshotTest".to_string(),
                            body: code,
                            options_obj: None,
                        });
                        idx_in_file += 1;
                    }
                }
            }

            Ok(out)
        }

        let source_stem = spec_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        #[derive(Clone, Debug)]
        struct ItPos {
            pos: usize,
            name: String,
            skipped: bool,
        }

        fn collect_it_positions(text: &str) -> Vec<ItPos> {
            let bytes = text.as_bytes();

            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            enum Mode {
                Normal,
                SingleQuote,
                DoubleQuote,
                Template,
                LineComment,
                BlockComment,
            }

            fn parse_string(bytes: &[u8], mut i: usize, quote: u8) -> Option<(String, usize)> {
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
                            b'\'' => out.push('\''),
                            b'"' => out.push('"'),
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
                    if b == quote {
                        return Some((out, i + 1));
                    }
                    out.push(b as char);
                    i += 1;
                }
                None
            }

            let mut out: Vec<ItPos> = Vec::new();
            let mut mode = Mode::Normal;
            let mut escaped = false;

            let mut i = 0usize;
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

                        if bytes.get(i) == Some(&b'i') && bytes.get(i + 1) == Some(&b't') {
                            let prev = if i == 0 {
                                None
                            } else {
                                bytes.get(i - 1).copied()
                            };
                            if prev.is_some_and(is_ident_byte) {
                                i += 1;
                                continue;
                            }
                            let mut j = i + 2;
                            let mut skipped = false;
                            if bytes.get(j) == Some(&b'.') {
                                if bytes.get(j + 1..j + 5) == Some(b"skip") {
                                    skipped = true;
                                    j += 5;
                                } else if bytes.get(j + 1..j + 5) == Some(b"only") {
                                    j += 5;
                                } else {
                                    i += 1;
                                    continue;
                                }
                            }
                            if bytes.get(j).is_some_and(|b| is_ident_byte(*b)) {
                                i += 1;
                                continue;
                            }

                            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                                j += 1;
                            }
                            if bytes.get(j) != Some(&b'(') {
                                i += 1;
                                continue;
                            }
                            j += 1;
                            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                                j += 1;
                            }
                            let quote = match bytes.get(j).copied() {
                                Some(b'\'') => b'\'',
                                Some(b'"') => b'"',
                                _ => {
                                    i += 1;
                                    continue;
                                }
                            };
                            j += 1;
                            let Some((name, end)) = parse_string(bytes, j, quote) else {
                                i += 1;
                                continue;
                            };
                            out.push(ItPos {
                                pos: i,
                                name,
                                skipped,
                            });
                            i = end;
                            continue;
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

            out
        }

        fn find_next_call(text: &str, needle: &str, from: usize) -> Option<usize> {
            let bytes = text.as_bytes();
            let needle_bytes = needle.as_bytes();

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
            let mut escaped = false;

            let mut i = from;
            while i + needle_bytes.len() <= bytes.len() {
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

                        if bytes[i..].starts_with(needle_bytes) {
                            let prev = if i == 0 {
                                None
                            } else {
                                bytes.get(i - 1).copied()
                            };
                            let next = bytes.get(i + needle_bytes.len()).copied();
                            if !prev.is_some_and(is_ident_byte) && !next.is_some_and(is_ident_byte)
                            {
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

        let mut test_name: Option<String> = None;
        let it_positions = collect_it_positions(&text);
        let mut next_it_idx = 0usize;

        let mut out: Vec<CypressBlock> = Vec::new();
        let mut idx_in_file = 0usize;

        fn synthesize_sankey_render_graph_using_this_graph_blocks(
            spec_path: &Path,
            source_stem: &str,
            text: &str,
            it_positions: &[ItPos],
            idx_in_file: &mut usize,
        ) -> Vec<CypressBlock> {
            #[derive(Clone, Debug)]
            struct GraphAssign {
                pos: usize,
                body: String,
            }

            fn find_test_name(it_positions: &[ItPos], abs: usize) -> Option<String> {
                let mut best: Option<String> = None;
                for it in it_positions {
                    if it.pos > abs {
                        break;
                    }
                    if it.pos < abs && !it.skipped {
                        best = Some(it.name.clone());
                    }
                }
                best
            }

            let bytes = text.as_bytes();
            let mut assigns: Vec<GraphAssign> = Vec::new();

            // Capture `cy.wrap(`...`).as('graph')` blocks.
            let mut search_from = 0usize;
            while let Some(abs) = find_next_call(text, "cy.wrap", search_from) {
                let after_call = abs + "cy.wrap".len();
                let mut open_paren = after_call;
                while bytes.get(open_paren).is_some_and(|b| is_ws_byte(*b)) {
                    open_paren += 1;
                }
                if bytes.get(open_paren) != Some(&b'(') {
                    search_from = after_call;
                    continue;
                }
                let Some(close_paren) = find_matching_paren_close(text, open_paren) else {
                    search_from = open_paren + 1;
                    continue;
                };

                let args_slice = &text[open_paren + 1..close_paren];
                let Some((raw, _end_rel)) = extract_first_template_literal(args_slice, 0) else {
                    search_from = close_paren + 1;
                    continue;
                };

                // Only keep the assignment if it targets `this.graph` via `.as('graph')`.
                let mut j = close_paren + 1;
                while bytes.get(j).is_some_and(|b| is_ws_byte(*b)) {
                    j += 1;
                }
                let tail = &text[j..text.len().min(j + 128)];
                if !(tail.contains(".as('graph')") || tail.contains(".as(\"graph\")")) {
                    search_from = close_paren + 1;
                    continue;
                }

                assigns.push(GraphAssign {
                    pos: abs,
                    body: raw,
                });
                search_from = close_paren + 1;
            }

            if assigns.is_empty() {
                return Vec::new();
            }

            // Synthesize `renderGraph(this.graph, { ... })` fixtures using the nearest preceding
            // `cy.wrap(...).as('graph')` source.
            let mut out: Vec<CypressBlock> = Vec::new();
            search_from = 0usize;
            while let Some(abs) = find_next_call(text, "renderGraph", search_from) {
                let after_call = abs + "renderGraph".len();
                let mut open_paren = after_call;
                while bytes.get(open_paren).is_some_and(|b| is_ws_byte(*b)) {
                    open_paren += 1;
                }
                if bytes.get(open_paren) != Some(&b'(') {
                    search_from = after_call;
                    continue;
                }
                let Some(close_paren) = find_matching_paren_close(text, open_paren) else {
                    search_from = open_paren + 1;
                    continue;
                };

                let args_slice = &text[open_paren + 1..close_paren];
                let trimmed = args_slice.trim_start();
                if !trimmed.starts_with("this.graph") {
                    search_from = close_paren + 1;
                    continue;
                }

                let Some(graph) = assigns.iter().rev().find(|a| a.pos < abs).cloned() else {
                    search_from = close_paren + 1;
                    continue;
                };

                // Extract second arg object literal.
                let arg0_start = args_slice.len() - trimmed.len();
                let after_first_arg = arg0_start + "this.graph".len();
                let Some(options_obj) =
                    extract_second_arg_object_literal(args_slice, after_first_arg)
                else {
                    search_from = close_paren + 1;
                    continue;
                };

                out.push(CypressBlock {
                    source_spec: spec_path.to_path_buf(),
                    source_stem: source_stem.to_string(),
                    idx_in_file: *idx_in_file,
                    test_name: find_test_name(it_positions, abs),
                    call: "renderGraph".to_string(),
                    body: graph.body,
                    options_obj: Some(options_obj),
                });
                *idx_in_file += 1;

                search_from = close_paren + 1;
            }

            out
        }

        for (call, needle) in [
            ("imgSnapshotTest", "imgSnapshotTest"),
            ("renderGraph", "renderGraph"),
        ] {
            let mut search_from = 0usize;
            while let Some(abs) = find_next_call(&text, needle, search_from) {
                while next_it_idx + 1 < it_positions.len()
                    && it_positions[next_it_idx + 1].pos < abs
                {
                    next_it_idx += 1;
                }
                let skipped_it = it_positions
                    .get(next_it_idx)
                    .is_some_and(|it| it.pos < abs && it.skipped);
                if skipped_it {
                    search_from = abs + needle.len();
                    continue;
                }
                if let Some(it) = it_positions.get(next_it_idx) {
                    if it.pos < abs {
                        test_name = Some(it.name.clone());
                    }
                }

                // Find the opening paren and extract the first template literal after it.
                let after_call = abs + needle.len();
                let mut open_paren = after_call;
                while bytes.get(open_paren).is_some_and(|b| is_ws_byte(*b)) {
                    open_paren += 1;
                }
                if bytes.get(open_paren) != Some(&b'(') {
                    // Not a direct call; e.g. `import { imgSnapshotTest } ...` or destructuring.
                    search_from = after_call;
                    continue;
                }
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
                    let options_obj = if call == "imgSnapshotTest" {
                        extract_second_arg_object_literal(args_slice, end_rel)
                    } else {
                        None
                    };
                    out.push(CypressBlock {
                        source_spec: spec_path.to_path_buf(),
                        source_stem: source_stem.clone(),
                        idx_in_file,
                        test_name: test_name.clone(),
                        call: call.to_string(),
                        body: raw,
                        options_obj,
                    });
                    idx_in_file += 1;
                    search_from = start + end_rel;
                    continue;
                }

                search_from = close_paren + 1;
            }
        }

        let file_name = spec_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if file_name == "sankey.spec.ts" {
            out.extend(synthesize_sankey_render_graph_using_this_graph_blocks(
                spec_path,
                &source_stem,
                &text,
                &it_positions,
                &mut idx_in_file,
            ));
        }

        if out.is_empty() {
            let file = spec_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();

            if file == "flowchart-shape-alias.spec.ts" {
                let synthesized =
                    synthesize_flowchart_shape_alias_blocks(spec_path, &source_stem, &text)?;
                if !synthesized.is_empty() {
                    return Ok(synthesized);
                }
            }

            if file == "oldShapes.spec.ts" {
                let synthesized = synthesize_flowchart_shapes_blocks(
                    spec_path,
                    &source_stem,
                    &text,
                    "shapesSets",
                )?;
                if !synthesized.is_empty() {
                    return Ok(synthesized);
                }
            }

            if file == "newShapes.spec.ts" {
                let synthesized = synthesize_flowchart_shapes_blocks(
                    spec_path,
                    &source_stem,
                    &text,
                    "newShapesSets",
                )?;
                if !synthesized.is_empty() {
                    return Ok(synthesized);
                }
            }
        }

        Ok(out)
    }

    fn js_object_literal_to_yaml_config_map(obj: &str) -> Option<serde_yaml::Mapping> {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        enum Mode {
            Normal,
            SingleQuote,
            DoubleQuote,
            LineComment,
            BlockComment,
        }

        fn is_ident_start(b: u8) -> bool {
            b.is_ascii_alphabetic() || b == b'_' || b == b'$'
        }

        fn is_ident_continue(b: u8) -> bool {
            b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
        }

        fn skip_ws_and_comments(bytes: &[u8], mut i: usize) -> usize {
            let mut mode = Mode::Normal;
            while i < bytes.len() {
                let b = bytes[i];
                match mode {
                    Mode::Normal => {
                        if is_ws_or_newline_byte(b) {
                            i += 1;
                            continue;
                        }
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
                        break;
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
                    Mode::SingleQuote | Mode::DoubleQuote => unreachable!(),
                }
            }
            i
        }

        fn parse_string(bytes: &[u8], mut i: usize, quote: u8) -> Option<(String, usize)> {
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
                        b'\'' => out.push('\''),
                        b'"' => out.push('"'),
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
                if b == quote {
                    return Some((out, i + 1));
                }
                out.push(b as char);
                i += 1;
            }
            None
        }

        fn parse_ident(bytes: &[u8], mut i: usize) -> (String, usize) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            (String::from_utf8_lossy(&bytes[start..i]).to_string(), i)
        }

        fn parse_number(bytes: &[u8], mut i: usize) -> Option<(serde_yaml::Value, usize)> {
            let start = i;
            if bytes.get(i) == Some(&b'-') {
                i += 1;
            }
            while i < bytes.len() {
                let b = bytes[i];
                if b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-') {
                    i += 1;
                    continue;
                }
                break;
            }
            let s = String::from_utf8_lossy(&bytes[start..i]).to_string();
            if s.is_empty() || s == "-" {
                return None;
            }
            let n: f64 = s.parse().ok()?;
            Some((serde_yaml::Value::Number(serde_yaml::Number::from(n)), i))
        }

        fn parse_value(bytes: &[u8], mut i: usize) -> Option<(serde_yaml::Value, usize)> {
            i = skip_ws_and_comments(bytes, i);
            let b = *bytes.get(i)?;
            match b {
                b'{' => {
                    let (m, j) = parse_object(bytes, i)?;
                    Some((serde_yaml::Value::Mapping(m), j))
                }
                b'[' => {
                    let (seq, j) = parse_array(bytes, i)?;
                    Some((serde_yaml::Value::Sequence(seq), j))
                }
                b'\'' | b'"' => {
                    let (s, j) = parse_string(bytes, i + 1, b)?;
                    Some((serde_yaml::Value::String(s), j))
                }
                b'-' | b'0'..=b'9' => parse_number(bytes, i),
                _ => {
                    if !is_ident_start(b) {
                        return None;
                    }
                    let (id, j) = parse_ident(bytes, i);
                    match id.as_str() {
                        "true" => Some((serde_yaml::Value::Bool(true), j)),
                        "false" => Some((serde_yaml::Value::Bool(false), j)),
                        "null" => Some((serde_yaml::Value::Null, j)),
                        _ => None,
                    }
                }
            }
        }

        fn parse_key(bytes: &[u8], mut i: usize) -> Option<(String, usize)> {
            i = skip_ws_and_comments(bytes, i);
            let b = *bytes.get(i)?;
            match b {
                b'\'' | b'"' => parse_string(bytes, i + 1, b),
                _ => {
                    if !is_ident_start(b) {
                        return None;
                    }
                    let (id, j) = parse_ident(bytes, i);
                    Some((id, j))
                }
            }
        }

        fn parse_object(bytes: &[u8], mut i: usize) -> Option<(serde_yaml::Mapping, usize)> {
            if bytes.get(i) != Some(&b'{') {
                return None;
            }
            i += 1;
            let mut map = serde_yaml::Mapping::new();
            loop {
                i = skip_ws_and_comments(bytes, i);
                if bytes.get(i) == Some(&b'}') {
                    return Some((map, i + 1));
                }
                let (key, mut j) = parse_key(bytes, i)?;
                j = skip_ws_and_comments(bytes, j);
                if bytes.get(j) != Some(&b':') {
                    return None;
                }
                j += 1;
                let (val, mut k) = parse_value(bytes, j)?;
                map.insert(serde_yaml::Value::String(key), val);
                k = skip_ws_and_comments(bytes, k);
                match bytes.get(k) {
                    Some(b',') => {
                        i = k + 1;
                        continue;
                    }
                    Some(b'}') => return Some((map, k + 1)),
                    _ => return None,
                }
            }
        }

        fn parse_array(bytes: &[u8], mut i: usize) -> Option<(Vec<serde_yaml::Value>, usize)> {
            if bytes.get(i) != Some(&b'[') {
                return None;
            }
            i += 1;
            let mut seq = Vec::new();
            loop {
                i = skip_ws_and_comments(bytes, i);
                if bytes.get(i) == Some(&b']') {
                    return Some((seq, i + 1));
                }
                let (val, mut j) = parse_value(bytes, i)?;
                seq.push(val);
                j = skip_ws_and_comments(bytes, j);
                match bytes.get(j) {
                    Some(b',') => {
                        i = j + 1;
                        continue;
                    }
                    Some(b']') => return Some((seq, j + 1)),
                    _ => return None,
                }
            }
        }

        let bytes = obj.as_bytes();
        let i = skip_ws_and_comments(bytes, 0);
        let (map, j) = parse_object(bytes, i)?;
        let j = skip_ws_and_comments(bytes, j);
        if j != bytes.len() {
            return None;
        }
        Some(map)
    }

    fn strip_yaml_frontmatter_for_detect(s: &str) -> &str {
        let bytes = s.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() && is_ws_or_newline_byte(bytes[i]) {
            i += 1;
        }
        let s = &s[i..];
        if !s.starts_with("---") {
            return s;
        }

        let mut pieces = s.split_inclusive('\n');
        let Some(first_piece) = pieces.next() else {
            return s;
        };
        let first_line = first_piece.trim_end_matches('\n').trim_end_matches('\r');
        if first_line.trim_end() != "---" {
            return s;
        }

        let mut consumed = first_piece.len();
        for piece in pieces {
            let line = piece.trim_end_matches('\n').trim_end_matches('\r');
            consumed += piece.len();
            if line.trim_end() == "---" {
                return &s[consumed..];
            }
        }

        s
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

    fn split_yaml_frontmatter(s: &str) -> Option<(&str, &str)> {
        let bytes = s.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() && is_ws_or_newline_byte(bytes[i]) {
            i += 1;
        }
        let s = &s[i..];
        if !s.starts_with("---") {
            return None;
        }

        let mut pieces = s.split_inclusive('\n');
        let Some(first_piece) = pieces.next() else {
            return None;
        };
        let first_line = first_piece.trim_end_matches('\n').trim_end_matches('\r');
        if first_line.trim_end() != "---" {
            return None;
        }

        let mut yaml_end = first_piece.len();
        for piece in pieces {
            let line = piece.trim_end_matches('\n').trim_end_matches('\r');
            if line.trim_end() == "---" {
                let yaml = &s[first_piece.len()..yaml_end];
                let rest = &s[yaml_end + piece.len()..];
                return Some((yaml, rest));
            }
            yaml_end += piece.len();
        }

        None
    }

    fn merge_yaml_mappings(dst: &mut serde_yaml::Mapping, src: serde_yaml::Mapping) {
        for (k, v) in src {
            match (dst.get_mut(&k), v) {
                (
                    Some(serde_yaml::Value::Mapping(dst_map)),
                    serde_yaml::Value::Mapping(src_map),
                ) => {
                    merge_yaml_mappings(dst_map, src_map);
                }
                (Some(dst_v), src_v) => {
                    *dst_v = src_v;
                }
                (None, src_v) => {
                    dst.insert(k, src_v);
                }
            }
        }
    }

    fn with_options_frontmatter(fixture_text: &str, options_obj: &str) -> String {
        let Some(options_map) = js_object_literal_to_yaml_config_map(options_obj) else {
            return fixture_text.to_string();
        };
        if options_map.is_empty() {
            return fixture_text.to_string();
        }

        let cfg_key = serde_yaml::Value::String("config".to_string());
        if let Some((yaml_raw, rest)) = split_yaml_frontmatter(fixture_text) {
            let yaml_raw = yaml_raw.trim();
            let mut fm = if yaml_raw.is_empty() {
                serde_yaml::Mapping::new()
            } else {
                match serde_yaml::from_str::<serde_yaml::Value>(yaml_raw) {
                    Ok(serde_yaml::Value::Mapping(m)) => m,
                    Ok(serde_yaml::Value::Null) => serde_yaml::Mapping::new(),
                    Ok(_) | Err(_) => {
                        let mut fm = serde_yaml::Mapping::new();
                        fm.insert(cfg_key.clone(), serde_yaml::Value::Mapping(options_map));
                        if let Ok(yaml) = serde_yaml::to_string(&fm) {
                            let yaml = yaml.trim_end_matches('\n');
                            return format!("---\n{yaml}\n---\n{fixture_text}");
                        }
                        return fixture_text.to_string();
                    }
                }
            };

            match fm.get_mut(&cfg_key) {
                Some(serde_yaml::Value::Mapping(existing)) => {
                    merge_yaml_mappings(existing, options_map);
                }
                Some(v) => {
                    *v = serde_yaml::Value::Mapping(options_map);
                }
                None => {
                    fm.insert(cfg_key, serde_yaml::Value::Mapping(options_map));
                }
            }

            if let Ok(yaml) = serde_yaml::to_string(&fm) {
                let yaml = yaml.trim_end_matches('\n');
                return format!("---\n{yaml}\n---\n{rest}");
            }
            return fixture_text.to_string();
        }

        let mut fm = serde_yaml::Mapping::new();
        fm.insert(cfg_key, serde_yaml::Value::Mapping(options_map));
        if let Ok(yaml) = serde_yaml::to_string(&fm) {
            let yaml = yaml.trim_end_matches('\n');
            return format!("---\n{yaml}\n---\n{fixture_text}");
        }
        fixture_text.to_string()
    }

    fn find_existing_fixture_stem_by_prefix(fixtures_dir: &Path, prefix: &str) -> Option<String> {
        let Ok(entries) = fs::read_dir(fixtures_dir) else {
            return None;
        };

        let mut best: Option<String> = None;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "mmd") {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.starts_with(prefix) {
                continue;
            }
            let Some(stem) = name.strip_suffix(".mmd") else {
                continue;
            };

            match best.as_deref() {
                None => best = Some(stem.to_string()),
                Some(prev) => {
                    if stem < prev {
                        best = Some(stem.to_string());
                    }
                }
            }
        }

        best
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
            let body = normalize_cypress_fixture_text(&b.body);
            let mut fixture_text = body;
            if let Some(options_obj) = b.options_obj.as_deref() {
                fixture_text = with_options_frontmatter(&fixture_text, options_obj);
            }

            let mut body = canonical_fixture_text(&fixture_text);
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
            let detect_input = strip_yaml_frontmatter_for_detect(body.as_str());
            let detected = match reg.detect_type(detect_input, &mut cfg) {
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

    if install && !with_baselines {
        return Err(XtaskError::SnapshotUpdateFailed(
            "`--install` only applies when `--with-baselines` is set".to_string(),
        ));
    }

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

    fn looks_like_sequence_half_arrows(fixture_text: &str) -> bool {
        [
            "-|\\",   // -|\
            "--|\\",  // --|\
            "-|/",    // -|/
            "--|/",   // --|/
            "-\\\\",  // -\\
            "--\\\\", // --\\
            "-//",    // -//
            "--//",   // --//
            "/|-",    // /|-
            "/|--",   // /|--
            "\\|-",   // \|-
            "\\|--",  // \|--
            "//-",    // //-
            "//--",   // //--
            "\\\\-",  // \\-
            "\\\\--", // \\--
        ]
        .into_iter()
        .any(|n| fixture_text.contains(n))
    }

    fn deferred_keep_fixture_only_reason(
        diagram_dir: &str,
        fixture_text: &str,
    ) -> Option<&'static str> {
        match diagram_dir {
            "flowchart" => {
                // Mermaid's Cypress flowchart suite includes cases that Mermaid itself can render
                // in-browser, but that our pinned upstream baseline renderer (`mmdc`) currently
                // fails to parse (Langium grammar). One known example is setting a nested
                // `direction` inside a `subgraph` block.
                //
                // Keep these fixtures under `_deferred` without baselines so `verify` stays green.
                let mut in_subgraph = false;
                for raw in fixture_text.lines() {
                    let l = raw.trim_start();
                    if l.starts_with("subgraph ") {
                        in_subgraph = true;
                        continue;
                    }
                    if in_subgraph && l == "end" {
                        in_subgraph = false;
                        continue;
                    }
                    if in_subgraph && l.starts_with("direction ") {
                        return Some(
                            "flowchart subgraph direction (deferred; no upstream baselines yet)",
                        );
                    }
                }
            }
            "er" => {
                // Some upstream Cypress ER fixtures intentionally exercise syntax that Mermaid's
                // CLI renderer (`mmdc`) fails to baseline-render today (e.g. numeric-only entity
                // names like `1` / `2.5`, or the standalone entity name `u`).
                //
                // Keep these fixtures for traceability under `_deferred` without baselines so
                // `verify` remains green.
                let er_src = fixture_text
                    .lines()
                    .skip_while(|l| !l.trim_start().starts_with("erDiagram"))
                    .collect::<Vec<_>>()
                    .join("\n");
                for raw in er_src.lines().skip(1) {
                    let l = raw.trim();
                    if l.is_empty() {
                        continue;
                    }
                    if l.as_bytes().first().is_some_and(|b| b.is_ascii_digit()) {
                        return Some(
                            "er numeric entity names (deferred; no upstream baselines yet)",
                        );
                    }
                    if l == "u" || l.starts_with("u {") || l.starts_with("u{") {
                        return Some("er entity name `u` (deferred; no upstream baselines yet)");
                    }
                    if l.contains("||--|| u") || l.contains("||--o{ u") || l.contains(" u--") {
                        return Some(
                            "er `u` in entities/cardinalities (deferred; no upstream baselines yet)",
                        );
                    }
                }
            }
            "sequence" => {
                // Our pinned upstream baseline renderer (tools/mermaid-cli) currently fails to
                // render these "half-arrow" operators, so keep the fixture for traceability under
                // `_deferred` without baselines.
                if looks_like_sequence_half_arrows(fixture_text) {
                    return Some("sequence half-arrows (deferred; no upstream baselines yet)");
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
            "class" => {
                // Our current class diagram renderer differs from Mermaid's v2 "direction" output
                // (upstream emits `<text>`, we often emit `<foreignObject>`). Defer these cases so
                // `verify` stays green while we iterate on parity.
                let is_class_v2 = fixture_text
                    .lines()
                    .any(|l| l.trim_start().starts_with("classDiagram-v2"));
                if is_class_v2
                    && fixture_text
                        .lines()
                        .any(|l| l.trim_start().starts_with("direction "))
                {
                    return Some("classDiagram-v2 direction (deferred)");
                }

                // ELK layout and non-classic looks are currently out of scope for parity-gated
                // headless rendering. Keep upstream SVG baselines for traceability but move these
                // fixtures under `_deferred` so `verify` remains green.
                if fixture_text.contains("\n  flowchart:\n    htmlLabels: false")
                    || fixture_text.contains("\nflowchart:\n    htmlLabels: false")
                {
                    return Some("class frontmatter config.flowchart.htmlLabels=false (deferred)");
                }
                if fixture_text.contains("\n  htmlLabels: false")
                    || fixture_text.contains("\nhtmlLabels: false")
                {
                    return Some("class frontmatter config.htmlLabels=false (deferred)");
                }
                if fixture_text.contains("\n  layout: elk")
                    || fixture_text.contains("\nlayout: elk")
                {
                    return Some("class frontmatter config.layout=elk (deferred)");
                }
                if fixture_text.contains("\n  look:") || fixture_text.contains("\nlook:") {
                    if !fixture_text.contains("\n  look: classic")
                        && !fixture_text.contains("\nlook: classic")
                    {
                        return Some("class frontmatter config.look!=classic (deferred)");
                    }
                }
            }
            "flowchart" => {
                // ELK layout is currently out of scope for the headless layout engine, but we
                // still keep the upstream SVG baseline so the case remains traceable.
                if fixture_text.contains("\n  layout: elk")
                    || fixture_text.contains("\nlayout: elk")
                {
                    return Some("flowchart frontmatter config.layout=elk (deferred)");
                }

                // Non-classic looks (e.g. `handDrawn`) are currently out of scope for parity-gated
                // headless rendering. Keep upstream SVG baselines for traceability but move these
                // fixtures under `_deferred` so `verify` remains green.
                if fixture_text.contains("\n  look:") || fixture_text.contains("\nlook:") {
                    if !fixture_text.contains("\n  look: classic")
                        && !fixture_text.contains("\nlook: classic")
                    {
                        return Some("flowchart frontmatter config.look!=classic (deferred)");
                    }
                }

                // Mermaid also has a dedicated `flowchart-elk` diagram type.
                // Keep these fixtures in `_deferred` until we implement ELK layout parity.
                if fixture_text
                    .lines()
                    .any(|l| l.trim_start().starts_with("flowchart-elk"))
                {
                    return Some("flowchart diagram type flowchart-elk (deferred)");
                }

                // Mermaid supports flowchart nodes with an `@{ icon: ... }` modifier. merman does
                // not implement icon rendering yet, so keep the upstream SVG for traceability but
                // move the fixture under `_deferred` to keep `verify` green.
                if fixture_text.contains("@{") && fixture_text.contains("icon:") {
                    return Some("flowchart icon nodes (deferred)");
                }

                // Mermaid also supports icon shorthands inside node labels, e.g.
                // `A(\"fab:fa-twitter Twitter\")` / `B(\"fa:fa-coffee Coffee\")`.
                if fixture_text.contains("fa:fa-")
                    || fixture_text.contains("fab:fa-")
                    || fixture_text.contains("far:fa-")
                    || fixture_text.contains("fas:fa-")
                    || fixture_text.contains("fal:fa-")
                    || fixture_text.contains("fad:fa-")
                {
                    return Some("flowchart icon labels (deferred)");
                }
            }
            "sequence" => {
                // Mermaid's sequence diagram v2 supports "central connections" where the arrow
                // contains circles on the actor lifelines, e.g. `Alice ()->>() Bob`.
                // merman does not implement this rendering yet, so keep the upstream SVG for
                // traceability but move the fixture under `_deferred` to keep `verify` green.
                if fixture_text.contains(" ()-") || fixture_text.contains("()-") {
                    return Some("sequence central connections (deferred)");
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

    fn defer_fixture_files_no_baselines(workspace_root: &Path, f: &CreatedFixture) -> PathBuf {
        let deferred_dir = workspace_root
            .join("fixtures")
            .join("_deferred")
            .join(&f.diagram_dir);
        let _ = fs::create_dir_all(&deferred_dir);

        let deferred_mmd_path = deferred_dir.join(format!("{}.mmd", f.stem));
        let _ = fs::remove_file(&deferred_mmd_path);
        let _ = fs::rename(&f.path, &deferred_mmd_path);

        // Ensure we don't leave any partially-generated artifacts in the main corpus.
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

        deferred_mmd_path
    }

    let mut created: Vec<CreatedFixture> = Vec::new();
    let mut imported_kept = 0usize;
    let mut imported_deferred = 0usize;

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

        let stem = {
            let source_slug = clamp_slug(slugify(&c.block.source_stem), 48);
            let test_slug = clamp_slug(
                slugify(c.block.test_name.as_deref().unwrap_or("example")),
                64,
            );
            let prefix = format!("upstream_cypress_{source_slug}_{test_slug}_");
            find_existing_fixture_stem_by_prefix(&c.fixtures_dir, &prefix)
                .unwrap_or_else(|| c.stem.clone())
        };

        let out_path = c.fixtures_dir.join(format!("{stem}.mmd"));
        if out_path.exists() && !overwrite {
            skipped.push(format!("skip (already exists): {}", out_path.display()));
            continue;
        }
        let deferred_out_path = workspace_root
            .join("fixtures")
            .join("_deferred")
            .join(&c.diagram_dir)
            .join(format!("{stem}.mmd"));
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

        let f = CreatedFixture {
            diagram_dir: c.diagram_dir,
            stem,
            path: out_path,
            source_spec: c.block.source_spec,
            source_idx_in_file: c.block.idx_in_file,
            source_call: c.block.call,
            source_test_name: c.block.test_name,
        };

        if !with_baselines {
            existing.insert(c.body.clone(), f.path.clone());
            created.push(f);
            imported_kept += 1;
            if let Some(max) = limit {
                if imported_kept >= max {
                    break;
                }
            }
            continue;
        }

        let fixture_text = c.body;

        if let Some(reason) = deferred_with_baselines_reason(&f.diagram_dir, &fixture_text) {
            report_lines.push(format!(
                "DEFERRED_WITHOUT_BASELINES\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\treason={reason}",
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
            cleanup_fixture_files(&workspace_root, &f);
            continue;
        }

        if let Some(reason) = deferred_keep_fixture_only_reason(&f.diagram_dir, &fixture_text) {
            report_lines.push(format!(
                "DEFERRED_NO_BASELINES\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\treason={reason}",
                f.diagram_dir,
                f.stem,
                f.source_spec.display(),
                f.source_idx_in_file,
                f.source_call,
                f.source_test_name.clone().unwrap_or_default(),
            ));
            let deferred_path = defer_fixture_files_no_baselines(&workspace_root, &f);
            imported_deferred += 1;
            skipped.push(format!(
                "skip (deferred without baselines): {} ({reason})",
                deferred_path.display(),
            ));
            existing.insert(fixture_text.clone(), deferred_path);
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
                let is_error_diagram_spec = f
                    .source_spec
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n == "errorDiagram.spec.js");

                let fixture_only_reason =
                    deferred_keep_fixture_only_reason(&f.diagram_dir, &fixture_text);
                let is_half_arrow_parse_error = f.diagram_dir == "sequence"
                    && msg.contains("Parse error")
                    && looks_like_sequence_half_arrows(&fixture_text);

                let can_defer_without_baselines = fixture_only_reason.is_some()
                    || is_half_arrow_parse_error
                    || is_error_diagram_spec;

                if can_defer_without_baselines {
                    let reason = if let Some(r) = fixture_only_reason {
                        r
                    } else if is_half_arrow_parse_error {
                        "sequence half-arrows (upstream parse error; deferred)"
                    } else {
                        debug_assert!(is_error_diagram_spec);
                        "errorDiagram fixtures (upstream svg fails; deferred)"
                    };

                    report_lines.push(format!(
                        "DEFERRED_NO_BASELINES\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\treason={reason}\tmsg={}",
                        f.diagram_dir,
                        f.stem,
                        f.source_spec.display(),
                        f.source_idx_in_file,
                        f.source_call,
                        f.source_test_name.clone().unwrap_or_default(),
                        msg.lines().next().unwrap_or("unknown upstream error"),
                    ));

                    let deferred_path = defer_fixture_files_no_baselines(&workspace_root, &f);
                    imported_deferred += 1;
                    skipped.push(format!(
                        "skip (deferred without baselines): {} ({reason})",
                        deferred_path.display()
                    ));
                    existing.insert(fixture_text.clone(), deferred_path);
                } else {
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
                    cleanup_fixture_files(&workspace_root, &f);
                }
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
            cleanup_fixture_files(&workspace_root, &f);
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
            defer_fixture_files_keep_baselines(&workspace_root, &f);
            imported_deferred += 1;
            existing.insert(fixture_text.clone(), deferred_out_path);
            continue;
        }

        if let Err(err) = super::super::update_snapshots(vec![
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
            cleanup_fixture_files(&workspace_root, &f);
            continue;
        }

        existing.insert(fixture_text.clone(), f.path.clone());
        created.push(f);

        imported_kept += 1;
        if let Some(max) = limit {
            if imported_kept >= max {
                break;
            }
        }
    }

    if !report_lines.is_empty() {
        if let Some(parent) = report_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let header = format!(
            "# import-upstream-cypress report (Mermaid@11.12.3)\n# generated_at={}\n",
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
        if !skipped.is_empty() {
            let mut dup = 0usize;
            let mut exists = 0usize;
            let mut deferred = 0usize;
            let mut upstream_failed = 0usize;
            let mut blank_svg = 0usize;
            let mut snapshot_failed = 0usize;
            let mut layout_snapshot_failed = 0usize;
            let mut other = 0usize;

            for s in &skipped {
                if s.starts_with("skip (duplicate content):") {
                    dup += 1;
                } else if s.starts_with("skip (already exists):") {
                    exists += 1;
                } else if s.starts_with("skip (already deferred):") {
                    deferred += 1;
                } else if s.starts_with("skip (upstream svg failed):") {
                    upstream_failed += 1;
                } else if s.starts_with("skip (suspicious upstream svg output):") {
                    blank_svg += 1;
                } else if s.starts_with("skip (snapshot update failed):") {
                    snapshot_failed += 1;
                } else if s.starts_with("skip (layout snapshot update failed):") {
                    layout_snapshot_failed += 1;
                } else {
                    other += 1;
                }
            }

            let mut msg = String::from("no fixtures were imported");
            msg.push_str(&format!(
                " (skipped: {dup} duplicate, {exists} exists, {deferred} deferred, {upstream_failed} upstream_failed, {blank_svg} blank_svg, {snapshot_failed} snapshot_failed, {layout_snapshot_failed} layout_snapshot_failed, {other} other)"
            ));
            msg.push_str(" (use --overwrite, or adjust --filter/--limit)");
            if imported_deferred > 0
                || (upstream_failed == 0
                    && blank_svg == 0
                    && snapshot_failed == 0
                    && layout_snapshot_failed == 0)
            {
                eprintln!("{msg}");
                return Ok(());
            }
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

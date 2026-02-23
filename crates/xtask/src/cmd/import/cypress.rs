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
            match super::super::gen_upstream_svgs(svg_args) {
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
                cleanup_fixture_files(&workspace_root, f);
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

use super::*;
use crate::cmd::javascript_source::{self, CypressRenderHelper};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

fn should_apply_cypress_options(options: &Value) -> bool {
    match options {
        Value::Null => false,
        Value::Object(map) => !map.is_empty(),
        _ => true,
    }
}

fn canonical_cypress_fixture_text(source: &str) -> String {
    let source = source.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = source.lines().collect::<Vec<_>>();
    while matches!(lines.first(), Some(line) if line.trim().is_empty()) {
        lines.remove(0);
    }
    while matches!(lines.last(), Some(line) if line.trim().is_empty()) {
        lines.pop();
    }
    format!("{}\n", lines.join("\n"))
}

fn canonical_cypress_fixture_identity(source: &str) -> String {
    let source = canonical_cypress_fixture_text(source);
    let Some((yaml, body)) = split_cypress_yaml_frontmatter(&source) else {
        return format!("frontmatter:{{}}\nbody:{source}");
    };
    let mut frontmatter = match serde_saphyr::from_str::<Value>(yaml.trim()) {
        Ok(Value::Null) => Value::Object(serde_json::Map::new()),
        Ok(value) => value,
        Err(_) => return format!("frontmatter:raw:{yaml}\nbody:{body}"),
    };
    super::canonicalize_imported_config_value(&mut frontmatter);
    format!(
        "frontmatter:{}\nbody:{}",
        serde_json::to_string(&frontmatter).expect("canonical config must serialize"),
        canonical_cypress_fixture_text(body)
    )
}

fn html_unescape_cypress_fixture(source: &str) -> String {
    let source = source.replace("&amp;", "&");
    let source = source.replace("&lt;", "<").replace("&gt;", ">");
    let source = source.replace("&quot;", "\"").replace("&#39;", "'");
    let source = source.replace("&nbsp;", " ");
    source.replace("&#160;", " ").replace("&#xA0;", " ")
}

fn dedent_cypress_fixture(source: &str) -> String {
    let source = source.replace("\r\n", "\n").replace('\r', "\n");
    let lines = source.lines().collect::<Vec<_>>();
    let min_indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.as_bytes()
                .iter()
                .take_while(|byte| matches!(byte, b' ' | b'\t'))
                .count()
        })
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|line| line.get(min_indent..).unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_cypress_yaml_frontmatter_indentation(source: &str) -> String {
    fn trim_front_whitespace(line: &str, count: usize) -> &str {
        let mut removed = 0usize;
        for (index, character) in line.char_indices() {
            if removed >= count {
                return &line[index..];
            }
            if matches!(character, ' ' | '\t') {
                removed += 1;
            } else {
                return &line[index..];
            }
        }
        if removed >= count { "" } else { line }
    }

    let lines = source.lines().collect::<Vec<_>>();
    let Some(first_non_empty) = lines.iter().position(|line| !line.trim().is_empty()) else {
        return source.to_string();
    };
    if lines[first_non_empty].trim() != "---" {
        return source.to_string();
    }
    let Some(close_index) = lines
        .iter()
        .enumerate()
        .skip(first_non_empty + 1)
        .find_map(|(index, line)| (line.trim() == "---").then_some(index))
    else {
        return source.to_string();
    };
    let min_indent = lines[(first_non_empty + 1)..close_index]
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.as_bytes()
                .iter()
                .take_while(|byte| matches!(byte, b' ' | b'\t'))
                .count()
        })
        .min()
        .unwrap_or(0);

    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            if matches!(index, value if value == first_non_empty || value == close_index) {
                "---"
            } else if index > first_non_empty && index < close_index {
                trim_front_whitespace(line, min_indent)
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_cypress_fixture_source(source: &str) -> String {
    canonical_cypress_fixture_text(&normalize_cypress_yaml_frontmatter_indentation(
        &dedent_cypress_fixture(&html_unescape_cypress_fixture(source)),
    ))
}

fn split_cypress_yaml_frontmatter(source: &str) -> Option<(&str, &str)> {
    let source = source.trim_start_matches(char::is_whitespace);
    let mut pieces = source.split_inclusive('\n');
    let first_piece = pieces.next()?;
    if first_piece.trim_end_matches(['\n', '\r']).trim_end() != "---" {
        return None;
    }

    let mut yaml_end = first_piece.len();
    for piece in pieces {
        if piece.trim_end_matches(['\n', '\r']).trim_end() == "---" {
            return Some((
                &source[first_piece.len()..yaml_end],
                &source[yaml_end + piece.len()..],
            ));
        }
        yaml_end += piece.len();
    }
    None
}

fn merge_static_config(destination: &mut Value, source: Value) {
    match (destination, source) {
        (Value::Object(destination), Value::Object(source)) => {
            for (key, value) in source {
                match destination.get_mut(&key) {
                    Some(destination) => merge_static_config(destination, value),
                    None => {
                        destination.insert(key, value);
                    }
                }
            }
        }
        (destination, source) => *destination = source,
    }
}

fn cypress_object_spread(value: Option<&Value>) -> serde_json::Map<String, Value> {
    match value {
        Some(Value::Object(map)) => map.clone(),
        Some(Value::Array(values)) => values
            .iter()
            .enumerate()
            .map(|(index, value)| (index.to_string(), value.clone()))
            .collect(),
        Some(Value::String(value)) => value
            .chars()
            .enumerate()
            .map(|(index, character)| (index.to_string(), Value::String(character.to_string())))
            .collect(),
        Some(Value::Null | Value::Bool(_) | Value::Number(_)) | None => serde_json::Map::new(),
    }
}

fn cypress_nullish_or(options: &serde_json::Map<String, Value>, key: &str, default: &str) -> Value {
    options
        .get(key)
        .filter(|value| !value.is_null())
        .cloned()
        .unwrap_or_else(|| Value::String(default.to_string()))
}

fn cypress_seeded_section(value: Option<&Value>) -> Value {
    let mut section = serde_json::Map::new();
    section.insert("seed".to_string(), Value::from(1));
    section.extend(cypress_object_spread(value));
    Value::Object(section)
}

fn materialized_cypress_options(
    helper: CypressRenderHelper,
    options: &Value,
) -> Result<Value, String> {
    let Value::Object(options) = options else {
        return Err("statically extracted options are not an object".to_string());
    };
    let mut effective = options.clone();
    if helper == CypressRenderHelper::ImgSnapshotTest {
        effective.insert(
            "fontFamily".to_string(),
            cypress_nullish_or(options, "fontFamily", "courier"),
        );
        effective.insert(
            "fontSize".to_string(),
            cypress_nullish_or(options, "fontSize", "16px"),
        );

        let mut sequence = cypress_object_spread(options.get("sequence"));
        sequence.insert(
            "actorFontFamily".to_string(),
            Value::String("courier".to_string()),
        );
        sequence.insert(
            "noteFontFamily".to_string(),
            cypress_nullish_or(&sequence, "noteFontFamily", "courier"),
        );
        sequence.insert(
            "messageFontFamily".to_string(),
            Value::String("courier".to_string()),
        );
        effective.insert("sequence".to_string(), Value::Object(sequence));
    }

    // Keep this in lockstep with Mermaid's pinned Cypress helper
    // (`cypress/helpers/util.ts`): `mermaidUrl` applies these defaults after
    // `imgSnapshotTest` has built its option object.
    effective.insert("handDrawnSeed".to_string(), Value::from(1));
    effective.insert(
        "architecture".to_string(),
        cypress_seeded_section(effective.get("architecture")),
    );
    effective.insert(
        "cynefin".to_string(),
        cypress_seeded_section(effective.get("cynefin")),
    );

    for cypress_only in ["listUrl", "listId", "name", "screenshot"] {
        effective.remove(cypress_only);
    }
    Ok(Value::Object(effective))
}

fn apply_cypress_options(fixture_text: &str, options: &Value) -> Result<String, String> {
    let Value::Object(options) = options.clone() else {
        return Err("statically extracted options are not an object".to_string());
    };
    let options = Value::Object(options);
    if !should_apply_cypress_options(&options) {
        return Ok(fixture_text.to_string());
    }

    let config_key = "config".to_string();
    if let Some((yaml, rest)) = split_cypress_yaml_frontmatter(fixture_text) {
        let mut frontmatter = if yaml.trim().is_empty() {
            serde_json::Map::new()
        } else {
            match serde_saphyr::from_str::<Value>(yaml.trim()) {
                Ok(Value::Object(frontmatter)) => frontmatter,
                Ok(Value::Null) => serde_json::Map::new(),
                Ok(_) => return Err("existing YAML frontmatter is not an object".to_string()),
                Err(error) => {
                    return Err(format!("existing YAML frontmatter is invalid: {error}"));
                }
            }
        };
        let mut merged_config = options;
        if let Some(frontmatter_config) = frontmatter.remove(&config_key) {
            // Mermaid applies initialize options first; diagram frontmatter has final precedence.
            merge_static_config(&mut merged_config, frontmatter_config);
        }
        frontmatter.insert(config_key, merged_config);
        let yaml = serde_saphyr::to_string(&frontmatter)
            .map_err(|error| format!("failed to serialize merged options: {error}"))?;
        return Ok(format!("---\n{}\n---\n{rest}", yaml.trim_end_matches('\n')));
    }

    let mut frontmatter = serde_json::Map::new();
    frontmatter.insert(config_key, options);
    let yaml = serde_saphyr::to_string(&frontmatter)
        .map_err(|error| format!("failed to serialize Cypress options: {error}"))?;
    Ok(format!(
        "---\n{}\n---\n{fixture_text}",
        yaml.trim_end_matches('\n')
    ))
}

pub(crate) fn materialize_cypress_fixture_source(
    source: &str,
    helper: CypressRenderHelper,
    options: &Value,
) -> Result<String, String> {
    let source = normalize_cypress_fixture_source(source);
    let options = materialized_cypress_options(helper, options)?;
    apply_cypress_options(&source, &options).map(|source| canonical_cypress_fixture_text(&source))
}

fn resolve_existing_cypress_stem(
    diagram_dir: &str,
    candidate_stem: &str,
    exact_exists: bool,
    body_matched_stems: &[String],
    claimed: &HashSet<(String, String)>,
    reserved_exact: &HashSet<(String, String)>,
) -> Result<String, Vec<String>> {
    let identity = |stem: &str| (diagram_dir.to_string(), stem.to_string());
    let is_claimed = |stem: &str| claimed.contains(&identity(stem));
    let is_reserved_for_other =
        |stem: &str| stem != candidate_stem && reserved_exact.contains(&identity(stem));
    if exact_exists
        && !is_claimed(candidate_stem)
        && body_matched_stems.iter().any(|stem| stem == candidate_stem)
    {
        return Ok(candidate_stem.to_string());
    }
    let available_body_matches = body_matched_stems
        .iter()
        .filter(|stem| !is_claimed(stem) && !is_reserved_for_other(stem))
        .cloned()
        .collect::<Vec<_>>();
    match available_body_matches.as_slice() {
        [only] => return Ok(only.clone()),
        [] => {}
        _ => return Err(available_body_matches),
    }
    if exact_exists {
        return Err(vec![candidate_stem.to_string()]);
    }
    if is_claimed(candidate_stem) {
        Err(vec![candidate_stem.to_string()])
    } else {
        Ok(candidate_stem.to_string())
    }
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
    let mut flowchart_elk_parity_fixtures: bool = false;
    let mut check_corpus_manifest_source: bool = false;
    let mut refresh_corpus_manifest_source: bool = false;
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
            "--flowchart-elk-parity-fixtures" => flowchart_elk_parity_fixtures = true,
            "--check-11-16-corpus-manifest-source" => check_corpus_manifest_source = true,
            "--refresh-11-16-corpus-manifest-source" => refresh_corpus_manifest_source = true,
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

    if check_corpus_manifest_source && refresh_corpus_manifest_source {
        return Err(XtaskError::SnapshotUpdateFailed(
            "Cypress corpus source check and refresh modes are mutually exclusive".to_string(),
        ));
    }
    if refresh_corpus_manifest_source
        && (diagram != "all"
            || filter.is_some()
            || limit.is_some()
            || min_lines.is_some()
            || prefer_complex
            || overwrite
            || with_baselines
            || install
            || flowchart_elk_parity_fixtures)
    {
        return Err(XtaskError::SnapshotUpdateFailed(
            "Cypress corpus source refresh cannot be combined with fixture selection or baseline import options"
                .to_string(),
        ));
    }

    let workspace_root = crate::cmd::workspace_root();
    let baseline_label = crate::cmd::pinned_mermaid_baseline_label(&workspace_root);

    let spec_root = spec_root
        .map(|p| {
            if p.is_absolute() {
                p
            } else {
                workspace_root.join(p)
            }
        })
        .unwrap_or_else(|| {
            crate::cmd::mermaid_repo_root()
                .join("cypress")
                .join("integration")
                .join("rendering")
        });
    if !spec_root.exists() {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "upstream cypress spec root not found: {} (expected repo-ref checkout of mermaid{baseline_label})",
            spec_root.display()
        )));
    }
    let corpus_manifest = if check_corpus_manifest_source || refresh_corpus_manifest_source {
        let failures = crate::cmd::committed_cypress_corpus_alignment_failures(&workspace_root);
        if !failures.is_empty() {
            return Err(XtaskError::AlignmentCheckFailed(failures.join("\n")));
        }
        Some(
            crate::cmd::load_committed_cypress_corpus_manifest(&workspace_root)
                .map_err(XtaskError::AlignmentCheckFailed)?,
        )
    } else {
        None
    };

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
            let line = raw_line.trim_end_matches([' ', '\t']);

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

    fn is_ws_or_newline_byte(b: u8) -> bool {
        matches!(b, b' ' | b'\t' | b'\n' | b'\r')
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

    #[derive(Debug, Clone)]
    struct CypressBlock {
        source_spec: PathBuf,
        source_stem: String,
        idx_in_file: usize,
        test_name: Option<String>,
        helper: CypressRenderHelper,
        call: String,
        body: String,
        options: Value,
    }

    fn extract_literal_cypress_blocks(
        spec_path: &Path,
    ) -> Result<(Vec<CypressBlock>, Vec<String>), XtaskError> {
        let text = fs::read_to_string(spec_path).map_err(|err| {
            XtaskError::SnapshotUpdateFailed(format!(
                "failed to read cypress spec file {}: {err}",
                spec_path.display()
            ))
        })?;
        let extraction =
            javascript_source::extract_cypress_render_cases(&text).map_err(|reason| {
                XtaskError::SnapshotUpdateFailed(format!(
                    "failed to parse Cypress spec file {} as TypeScript: {reason}",
                    spec_path.display()
                ))
            })?;
        let source_stem = spec_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("unknown")
            .to_string();
        let diagnostics = extraction
            .unsupported
            .into_iter()
            .map(|diagnostic| {
                format!(
                    "skip (unsupported static Cypress call): {} (call={}, byte={}): {}",
                    spec_path.display(),
                    diagnostic.helper.as_str(),
                    diagnostic.start_byte,
                    diagnostic.reason
                )
            })
            .collect();
        let blocks = extraction
            .cases
            .into_iter()
            .enumerate()
            .map(|(idx_in_file, case)| CypressBlock {
                source_spec: spec_path.to_path_buf(),
                source_stem: source_stem.clone(),
                idx_in_file,
                test_name: case.test_name,
                helper: case.helper,
                call: case.helper.as_str().to_string(),
                body: case.diagram,
                options: case.options,
            })
            .collect();
        Ok((blocks, diagnostics))
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
        identity: String,
        score: i64,
    }

    fn existing_fixture_stems_by_body(
        dirs: &[&Path],
        prefix: &str,
        body: &str,
    ) -> Result<Vec<String>, XtaskError> {
        let mut stems = Vec::new();
        for dir in dirs {
            let entries = match fs::read_dir(dir) {
                Ok(entries) => entries,
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => continue,
                Err(source) => {
                    return Err(XtaskError::ReadFile {
                        path: dir.display().to_string(),
                        source,
                    });
                }
            };
            for entry in entries {
                let path = entry
                    .map_err(|source| XtaskError::ReadFile {
                        path: dir.display().to_string(),
                        source,
                    })?
                    .path();
                let Some(stem) = path
                    .extension()
                    .filter(|extension| *extension == "mmd")
                    .and_then(|_| path.file_stem())
                    .and_then(|stem| stem.to_str())
                    .filter(|stem| stem.starts_with(prefix))
                else {
                    continue;
                };
                let existing_body =
                    fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
                        path: path.display().to_string(),
                        source,
                    })?;
                if canonical_cypress_fixture_identity(&existing_body)
                    == canonical_cypress_fixture_identity(body)
                {
                    stems.push(stem.to_string());
                }
            }
        }
        stems.sort();
        stems.dedup();
        Ok(stems)
    }

    let reg = merman::detect::DetectorRegistry::pinned_mermaid_baseline();
    let spec_files: Vec<PathBuf> = if let Some(manifest) = corpus_manifest.as_ref() {
        let mermaid_root = crate::cmd::mermaid_repo_root();
        manifest
            .scope
            .source_specs
            .iter()
            .map(|source| {
                crate::cmd::resolve_cypress_source_spec_path(&mermaid_root, &source.path)
                    .map_err(XtaskError::AlignmentCheckFailed)
            })
            .collect::<Result<_, _>>()?
    } else {
        let mut files = Vec::new();
        collect_spec_files_recursively(&spec_root, &mut files)?;
        files.sort();
        files
    };

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    let mut existing_by_diagram: HashMap<String, HashMap<String, PathBuf>> = HashMap::new();

    for spec_path in spec_files {
        if let Some(f) = filter.as_deref() {
            let hay = spec_path.to_string_lossy();
            if !hay.contains(f) {
                // Still allow filtering by test name later; don't early-skip the file here.
            }
        }

        let (blocks, diagnostics) = extract_literal_cypress_blocks(&spec_path)?;
        skipped.extend(diagnostics);
        for b in blocks {
            let mut body = materialize_cypress_fixture_source(&b.body, b.helper, &b.options).map_err(
                |reason| {
                    XtaskError::SnapshotUpdateFailed(format!(
                        "failed to materialize static Cypress fixture from {} (call={}, idx={}): {reason}",
                        b.source_spec.display(),
                        b.call,
                        b.idx_in_file
                    ))
                },
            )?;
            if body.trim().is_empty() {
                continue;
            }
            if let Some(min) = min_lines
                && body.lines().count() < min
            {
                continue;
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
            let Some(diagram_dir) = normalize_imported_diagram_dir(detected).map(str::to_string)
            else {
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
            // We explicitly defer/skip cases that exercise browser-only math rendering
            // (`$$...$$`) or are sourced from the upstream `errorDiagram` spec. Flowchart ELK is
            // handled after fixture stem assignment so admitted ELK cases can enter the dedicated
            // layout lane while unknown ELK fixtures remain deferred.
            if with_baselines && diagram_dir == "flowchart" {
                let spec_name = spec_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
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
            }

            if diagram_dir == "architecture" {
                body = canonical_cypress_fixture_text(&normalize_architecture_beta_legacy_edges(
                    &body,
                ));
            }

            let fixtures_dir = crate::cmd::fixtures_root().join(&diagram_dir);
            if !fixtures_dir.is_dir() {
                skipped.push(format!(
                    "skip (fixtures dir missing): {}",
                    fixtures_dir.display()
                ));
                continue;
            }

            let source_slug = clamp_slug(slugify(&b.source_stem), 48);
            let test_slug = clamp_slug(slugify(b.test_name.as_deref().unwrap_or("example")), 64);
            let content_id = imported_fixture_content_id(&body);
            let stem = format!("upstream_cypress_{source_slug}_{test_slug}_{content_id}");
            let identity = canonical_cypress_fixture_identity(&body);

            let score = complexity_score(&body, &diagram_dir);
            candidates.push(Candidate {
                block: b,
                diagram_dir,
                fixtures_dir,
                stem,
                body,
                identity,
                score,
            });
        }
    }

    if let Some(manifest) = corpus_manifest.as_ref() {
        let mermaid_root = crate::cmd::mermaid_repo_root();
        let mut observations = Vec::with_capacity(candidates.len());
        for candidate in &candidates {
            let source_spec = candidate
                .block
                .source_spec
                .strip_prefix(&mermaid_root)
                .map_err(|_| {
                    XtaskError::AlignmentCheckFailed(format!(
                        "Cypress corpus source is outside pinned Mermaid checkout: {}",
                        candidate.block.source_spec.display()
                    ))
                })?
                .to_string_lossy()
                .replace('\\', "/");
            observations.push(crate::cmd::CypressSourceObservation {
                source_spec,
                call_ordinal: candidate.block.idx_in_file + 1,
                call: candidate.block.call.clone(),
                test_name: candidate.block.test_name.clone().unwrap_or_default(),
                family: candidate.diagram_dir.clone(),
                mmd_sha256: crate::cmd::cypress_corpus_mmd_sha256(candidate.body.as_bytes()),
            });
        }
        if refresh_corpus_manifest_source {
            let refreshed = crate::cmd::refreshed_cypress_corpus_manifest(manifest, &observations)
                .map_err(|failures| XtaskError::AlignmentCheckFailed(failures.join("\n")))?;
            let manifest_path = workspace_root.join(crate::cmd::MANIFEST_RELATIVE_PATH);
            let mut replacements = refreshed
                .entries
                .iter()
                .zip(candidates.iter())
                .map(|(entry, candidate)| {
                    (
                        workspace_root.join(entry.fixture.as_path()),
                        candidate.body.as_bytes().to_vec(),
                    )
                })
                .collect::<Vec<_>>();
            let manifest_json = serde_json::to_string_pretty(&refreshed).map_err(|error| {
                XtaskError::SnapshotUpdateFailed(format!(
                    "failed to serialize refreshed Cypress corpus manifest: {error}"
                ))
            })?;
            replacements.push((manifest_path, format!("{manifest_json}\n").into_bytes()));

            let originals = replacements
                .iter()
                .map(|(path, _)| {
                    fs::read(path)
                        .map(|bytes| (path.clone(), bytes))
                        .map_err(|source| XtaskError::ReadFile {
                            path: path.display().to_string(),
                            source,
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            for (path, contents) in &replacements {
                if let Err(source) = fs::write(path, contents) {
                    let mut rollback_failures = Vec::new();
                    for (original_path, original_contents) in &originals {
                        if let Err(error) = fs::write(original_path, original_contents) {
                            rollback_failures.push(format!("{}: {error}", original_path.display()));
                        }
                    }
                    let rollback = if rollback_failures.is_empty() {
                        String::new()
                    } else {
                        format!("; rollback failures: {}", rollback_failures.join(", "))
                    };
                    return Err(XtaskError::SnapshotUpdateFailed(format!(
                        "failed to refresh Cypress corpus file {}: {source}{rollback}",
                        path.display()
                    )));
                }
            }
            return Ok(());
        }

        let failures = crate::cmd::validate_cypress_source_observations(manifest, &observations);
        return if failures.is_empty() {
            Ok(())
        } else {
            Err(XtaskError::AlignmentCheckFailed(failures.join("\n")))
        };
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
        identity: String,
        rollback: Option<ImportedFixtureSnapshot>,
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

    let report_path = crate::cmd::target_root().join("import-upstream-cypress.report.txt");
    let mut report_lines: Vec<String> = Vec::new();

    fn deferred_with_baselines_reason(
        diagram_dir: &str,
        fixture_text: &str,
    ) -> Option<&'static str> {
        match diagram_dir {
            "flowchart" if fixture_text.contains("$$") => {
                return Some("flowchart math (deferred)");
            }
            "sequence" if fixture_text.contains("$$") => {
                return Some("sequence math (deferred)");
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
        fixture_stem: &str,
        fixture_text: &str,
        flowchart_elk_parity_fixtures: bool,
    ) -> Option<&'static str> {
        match diagram_dir {
            "flowchart" => {
                if flowchart_elk_parity_fixtures
                    && super::super::upstream_svg_policy::flowchart_elk_svg_parity_admitted(
                        fixture_stem,
                    )
                {
                    return None;
                }

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
            "sequence"
                // Our pinned upstream baseline renderer (tools/mermaid-cli) currently fails to
                // render these "half-arrow" operators, so keep the fixture for traceability under
                // `_deferred` without baselines.
                if looks_like_sequence_half_arrows(fixture_text) => {
                    return Some("sequence half-arrows (deferred; no upstream baselines yet)");
                }
            _ => {}
        }
        None
    }

    fn deferred_keep_baselines_reason(
        diagram_dir: &str,
        stem: &str,
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

                // ELK layout and unsupported looks are currently out of scope for parity-gated
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
                if let Some(look) = imported_fixture_config_look(fixture_text)
                    && !matches!(look.as_str(), "classic" | "handDrawn")
                {
                    return Some("class frontmatter config.look unsupported (deferred)");
                }
            }
            "flowchart" => {
                let admitted_flowchart_elk_parity =
                    crate::cmd::flowchart_elk_svg_parity_admitted(stem);
                // Flowchart ELK has a lightweight renderer path, but full SVG parity is tracked in
                // a dedicated layout lane. Keep unadmitted upstream SVG baselines traceable under
                // `_deferred`.
                if (fixture_text.contains("\n  layout: elk")
                    || fixture_text.contains("\nlayout: elk"))
                    && !admitted_flowchart_elk_parity
                    && let Some(reason) = crate::cmd::flowchart_elk_svg_parity_skip_reason(stem)
                {
                    return Some(reason);
                }

                // Flowchart now admits `handDrawn` alongside `classic`; keep other look variants
                // deferred until their source-backed DOM contract is implemented.
                if let Some(look) = imported_fixture_config_look(fixture_text)
                    && !matches!(look.as_str(), "classic" | "handDrawn")
                {
                    return Some("flowchart frontmatter config.look unsupported (deferred)");
                }

                // Mermaid also has a dedicated `flowchart-elk` diagram type. Keep these fixtures
                // in `_deferred` until the ELK layout lane admits them to SVG parity.
                if fixture_text
                    .lines()
                    .any(|l| l.trim_start().starts_with("flowchart-elk"))
                    && !admitted_flowchart_elk_parity
                    && let Some(reason) = crate::cmd::flowchart_elk_svg_parity_skip_reason(stem)
                {
                    return Some(reason);
                }

                // Mermaid also supports icon shorthands inside node labels, e.g.
                // `A(\"fab:fa-twitter Twitter\")` / `B(\"fa:fa-coffee Coffee\")`.
                if !admitted_flowchart_elk_parity
                    && (fixture_text.contains("fa:fa-")
                    || fixture_text.contains("fab:fa-")
                    || fixture_text.contains("far:fa-")
                    || fixture_text.contains("fas:fa-")
                    || fixture_text.contains("fal:fa-")
                    || fixture_text.contains("fad:fa-"))
                {
                    return Some("flowchart icon labels (deferred)");
                }
            }
            "sequence"
                // Mermaid's sequence diagram v2 supports "central connections" where the arrow
                // contains circles on the actor lifelines, e.g. `Alice ()->>() Bob`.
                // merman does not implement this rendering yet, so keep the upstream SVG for
                // traceability but move the fixture under `_deferred` to keep `verify` green.
            if fixture_text.contains(" ()-") || fixture_text.contains("()-") => {
                    return Some("sequence central connections (deferred)");
                }
            _ => {}
        }
        None
    }

    fn is_suspicious_blank_svg(svg_path: &Path) -> Result<bool, XtaskError> {
        let head = fs::read_to_string(svg_path).map_err(|source| XtaskError::ReadFile {
            path: svg_path.display().to_string(),
            source,
        })?;
        let first = head.lines().next().unwrap_or_default();
        Ok(first.contains(r#"viewBox="-8 -8 16 16""#)
            || first.contains(r#"viewBox="0 0 16 16""#)
            || first.contains(r#"style="max-width: 16px"#))
    }

    fn reject_fixture(f: &CreatedFixture) -> Result<(), XtaskError> {
        reject_imported_fixture_transaction(&f.diagram_dir, &f.stem, &f.path, f.rollback.as_ref())
    }

    fn cleanup_deferred_fixture_files(f: &CreatedFixture) -> Result<(), XtaskError> {
        crate::cmd::import::cleanup_deferred_fixture_files(&f.diagram_dir, &f.stem)
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
    let mut imported_kept = 0usize;
    let mut imported_deferred = 0usize;
    let mut claimed_stems = HashSet::<(String, String)>::new();
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

    let reserved_exact_stems = candidates
        .iter()
        .filter(|candidate| {
            candidate
                .fixtures_dir
                .join(format!("{}.mmd", candidate.stem))
                .exists()
                || crate::cmd::fixtures_root()
                    .join("_deferred")
                    .join(&candidate.diagram_dir)
                    .join(format!("{}.mmd", candidate.stem))
                    .exists()
        })
        .map(|candidate| (candidate.diagram_dir.clone(), candidate.stem.clone()))
        .collect::<HashSet<_>>();

    for c in candidates {
        let existing = existing_by_diagram
            .entry(c.diagram_dir.clone())
            .or_insert_with(|| {
                load_existing_imported_fixtures(
                    &workspace_lock,
                    &c.fixtures_dir,
                    &c.diagram_dir,
                    canonical_cypress_fixture_identity,
                )
            });
        let existing_path = existing.get(&c.identity).cloned();

        let deferred_fixtures_dir = crate::cmd::fixtures_root()
            .join("_deferred")
            .join(&c.diagram_dir);
        let source_slug = clamp_slug(slugify(&c.block.source_stem), 48);
        let test_slug = clamp_slug(
            slugify(c.block.test_name.as_deref().unwrap_or("example")),
            64,
        );
        let prefix = format!("upstream_cypress_{source_slug}_{test_slug}_");
        let exact_exists = c.fixtures_dir.join(format!("{}.mmd", c.stem)).exists()
            || deferred_fixtures_dir
                .join(format!("{}.mmd", c.stem))
                .exists();
        let body_matched_stems = existing_fixture_stems_by_body(
            &[&c.fixtures_dir, &deferred_fixtures_dir],
            &prefix,
            &c.body,
        )?;
        let stem = resolve_existing_cypress_stem(
            &c.diagram_dir,
            &c.stem,
            exact_exists,
            &body_matched_stems,
            &claimed_stems,
            &reserved_exact_stems,
        )
        .map_err(|ambiguous| {
            XtaskError::SnapshotUpdateFailed(format!(
                "ambiguous existing Cypress fixture identity for {} [{}]: {}",
                c.block.source_spec.display(),
                c.block.test_name.as_deref().unwrap_or("example"),
                ambiguous.join(", ")
            ))
        })?;
        let allow_duplicate_body = flowchart_elk_parity_fixtures
            && c.diagram_dir == "flowchart"
            && crate::cmd::flowchart_elk_svg_parity_admitted(&stem);
        if flowchart_elk_parity_fixtures && !allow_duplicate_body {
            continue;
        }
        if !claimed_stems.insert((c.diagram_dir.clone(), stem.clone())) {
            return Err(XtaskError::SnapshotUpdateFailed(format!(
                "duplicate Cypress fixture identity resolved for {}: {}/{}",
                c.block.source_spec.display(),
                c.diagram_dir,
                stem
            )));
        }

        let out_path = c.fixtures_dir.join(format!("{stem}.mmd"));
        if out_path.exists() && !overwrite {
            skipped.push(format!("skip (already exists): {}", out_path.display()));
            continue;
        }
        let deferred_out_path = crate::cmd::fixtures_root()
            .join("_deferred")
            .join(&c.diagram_dir)
            .join(format!("{stem}.mmd"));
        let source_addressed_overwrite = source_addressed_active_overwrite(overwrite, &out_path);
        if !allow_duplicate_body
            && let Some(existing_path) = existing_path.as_deref()
            && !source_addressed_overwrite
            && !should_revalidate_deferred_fixture(
                existing_path,
                &deferred_out_path,
                with_baselines,
                overwrite,
            )
        {
            skipped.push(format!(
                "skip (duplicate content): {} -> {}",
                c.block.source_spec.display(),
                existing_path.display()
            ));
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
                &stem,
                &out_path,
            )?)
        } else {
            None
        };
        if let Err(error) = write_imported_fixture(&c.diagram_dir, &stem, &out_path, &c.body) {
            return Err(rollback_imported_fixture_snapshots(error, rollback.iter()));
        }
        if with_baselines
            && let Err(error) =
                validate_exact_import_candidate_filter(&c.diagram_dir, &stem, &out_path)
        {
            return Err(rollback_imported_fixture_snapshots(error, rollback.iter()));
        }

        let f = CreatedFixture {
            diagram_dir: c.diagram_dir,
            stem,
            path: out_path,
            identity: c.identity,
            rollback,
            source_spec: c.block.source_spec,
            source_idx_in_file: c.block.idx_in_file,
            source_call: c.block.call,
            source_test_name: c.block.test_name,
        };

        if !with_baselines {
            record_imported_fixture_content(
                existing,
                f.identity.clone(),
                f.path.clone(),
                &[f.path.as_path(), deferred_out_path.as_path()],
            );
            created.push(f);
            imported_kept += 1;
            if let Some(max) = limit
                && imported_kept >= max
            {
                break;
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
            reject_fixture(&f)?;
            continue;
        }

        if let Some(reason) = deferred_keep_fixture_only_reason(
            &f.diagram_dir,
            &f.stem,
            &fixture_text,
            flowchart_elk_parity_fixtures,
        ) {
            report_lines.push(format!(
                "DEFERRED_NO_BASELINES\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\treason={reason}",
                f.diagram_dir,
                f.stem,
                f.source_spec.display(),
                f.source_idx_in_file,
                f.source_call,
                f.source_test_name.clone().unwrap_or_default(),
            ));
            let deferred_path = defer_fixture(&f, false, overwrite)?;
            imported_deferred += 1;
            skipped.push(format!(
                "skip (deferred without baselines): {} ({reason})",
                deferred_path.display(),
            ));
            record_imported_fixture_content(
                existing,
                f.identity.clone(),
                deferred_path,
                &[f.path.as_path(), deferred_out_path.as_path()],
            );
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
        match super::super::gen_upstream_svgs_with_transaction_locks(
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
            Ok(()) => {}
            Err(error) => {
                let msg = match candidate_upstream_svg_failure(error, &f.path) {
                    Ok(msg) => msg,
                    Err(error) => {
                        return Err(rollback_imported_fixture_snapshots(
                            error,
                            f.rollback.iter(),
                        ));
                    }
                };
                let is_error_diagram_spec = f
                    .source_spec
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n == "errorDiagram.spec.js");

                let fixture_only_reason = deferred_keep_fixture_only_reason(
                    &f.diagram_dir,
                    &f.stem,
                    &fixture_text,
                    flowchart_elk_parity_fixtures,
                );
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

                    let deferred_path = defer_fixture(&f, false, overwrite)?;
                    imported_deferred += 1;
                    skipped.push(format!(
                        "skip (deferred without baselines): {} ({reason})",
                        deferred_path.display()
                    ));
                    record_imported_fixture_content(
                        existing,
                        f.identity.clone(),
                        deferred_path,
                        &[f.path.as_path(), deferred_out_path.as_path()],
                    );
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
                    reject_fixture(&f)?;
                }
                continue;
            }
        }

        let svg_path = crate::cmd::fixtures_root()
            .join("upstream-svgs")
            .join(&f.diagram_dir)
            .join(format!("{}.svg", f.stem));
        if is_suspicious_blank_svg(&svg_path)
            .map_err(|error| rollback_imported_fixture_snapshots(error, f.rollback.iter()))?
        {
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
            reject_fixture(&f)?;
            continue;
        }

        if let Some(reason) = deferred_keep_baselines_reason(&f.diagram_dir, &f.stem, &fixture_text)
        {
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
            let _ = defer_fixture(&f, true, overwrite)?;
            imported_deferred += 1;
            record_imported_fixture_content(
                existing,
                f.identity.clone(),
                deferred_out_path.clone(),
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
            let err = match candidate_snapshot_failure(error, &f.path) {
                Ok(message) => message,
                Err(error) => {
                    return Err(rollback_imported_fixture_snapshots(
                        error,
                        f.rollback.iter(),
                    ));
                }
            };
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
            reject_fixture(&f)?;
            continue;
        }

        if let Err(error) = super::super::update_layout_snapshots(vec![
            "--diagram".to_string(),
            f.diagram_dir.clone(),
            "--filter".to_string(),
            f.stem.clone(),
        ]) {
            let err = match candidate_snapshot_failure(error, &f.path) {
                Ok(message) => message,
                Err(error) => {
                    return Err(rollback_imported_fixture_snapshots(
                        error,
                        f.rollback.iter(),
                    ));
                }
            };
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
            reject_fixture(&f)?;
            continue;
        }

        // Parity gate (matches `xtask verify` by default). Flowchart ELK parity fixtures use the
        // canonical ELK renderer and the same measurement profile as `check-flowchart-elk-parity`.
        let mut compare_args = vec![
            "--check-dom".to_string(),
            "--dom-mode".to_string(),
            "parity".to_string(),
            "--dom-decimals".to_string(),
            "3".to_string(),
            "--diagram".to_string(),
            f.diagram_dir.clone(),
            "--filter".to_string(),
            f.stem.clone(),
        ];
        if flowchart_elk_parity_fixtures && f.diagram_dir == "flowchart" {
            compare_args.extend([
                "--flowchart-text-measurer".to_string(),
                "vendored".to_string(),
            ]);
        }
        if let Err(error) = super::super::compare_all_svgs_with_transaction_locks(
            compare_args,
            transaction_locks
                .as_ref()
                .expect("baseline import transaction must hold a family lock")
                .family_lock(),
            transaction_locks
                .as_ref()
                .expect("baseline import transaction must hold a toolchain lock")
                .toolchain_lock(),
        ) {
            let msg = match candidate_svg_compare_failure(error, &f.path, &f.stem) {
                Ok(message) => message,
                Err(error) => {
                    return Err(rollback_imported_fixture_snapshots(
                        error,
                        f.rollback.iter(),
                    ));
                }
            };
            let msg_head = msg.lines().next().unwrap_or("svg compare failed");
            let reason = "svg dom parity mismatch (deferred)";
            report_lines.push(format!(
                "DEFERRED_WITH_BASELINES\t{}\t{}\t{}\tblock_idx={}\tcall={}\ttest={}\treason={reason}\terr={msg_head}",
                f.diagram_dir,
                f.stem,
                f.source_spec.display(),
                f.source_idx_in_file,
                f.source_call,
                f.source_test_name.clone().unwrap_or_default(),
            ));
            skipped.push(format!(
                "skip (svg dom parity mismatch; deferred): {} ({msg_head})",
                f.path.display(),
            ));
            let _ = defer_fixture(&f, true, overwrite)?;
            imported_deferred += 1;
            record_imported_fixture_content(
                existing,
                f.identity.clone(),
                deferred_out_path.clone(),
                &[f.path.as_path(), deferred_out_path.as_path()],
            );
            continue;
        }

        record_imported_fixture_content(
            existing,
            f.identity.clone(),
            f.path.clone(),
            &[f.path.as_path(), deferred_out_path.as_path()],
        );
        if let Err(error) = cleanup_deferred_fixture_files(&f) {
            return Err(rollback_imported_fixture_snapshots(
                error,
                f.rollback.iter(),
            ));
        }
        let mut f = f;
        f.rollback = None;
        created.push(f);

        imported_kept += 1;
        if let Some(max) = limit
            && imported_kept >= max
        {
            break;
        }
    }

    if !report_lines.is_empty() {
        if let Some(parent) = report_path.parent() {
            fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
                path: parent.display().to_string(),
                source,
            })?;
        }
        let header = format!(
            "# import-upstream-cypress report (Mermaid{baseline_label})\n# generated_at={}\n",
            crate::cmd::timestamps::current_local_report_timestamp_milliseconds()
        );
        let mut out = String::new();
        out.push_str(&header);
        out.push_str(&report_lines.join("\n"));
        out.push('\n');
        fs::write(&report_path, out).map_err(|source| XtaskError::WriteFile {
            path: report_path.display().to_string(),
            source,
        })?;
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
            let mut svg_parity_deferred = 0usize;
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
                } else if s.starts_with("skip (svg dom parity mismatch; deferred):") {
                    svg_parity_deferred += 1;
                } else {
                    other += 1;
                }
            }

            let mut msg = String::from("no fixtures were imported");
            msg.push_str(&format!(
                " (skipped: {dup} duplicate, {exists} exists, {deferred} deferred, {upstream_failed} upstream_failed, {blank_svg} blank_svg, {snapshot_failed} snapshot_failed, {layout_snapshot_failed} layout_snapshot_failed, {svg_parity_deferred} svg_parity_deferred, {other} other)"
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

#[cfg(test)]
mod tests {
    use super::{
        canonical_cypress_fixture_identity, materialize_cypress_fixture_source,
        resolve_existing_cypress_stem, should_apply_cypress_options,
        split_cypress_yaml_frontmatter,
    };
    use crate::cmd::import::normalize_imported_diagram_dir;
    use crate::cmd::javascript_source::CypressRenderHelper;
    use std::collections::HashSet;

    #[test]
    fn cypress_empty_options_do_not_create_frontmatter() {
        assert!(!should_apply_cypress_options(&serde_json::json!(null)));
        assert!(!should_apply_cypress_options(&serde_json::json!({})));
        assert!(should_apply_cypress_options(
            &serde_json::json!({ "theme": "dark" })
        ));
    }

    #[test]
    fn cypress_identity_uses_config_semantics_but_preserves_diagram_source() {
        let old_fixture = "---\nconfig:\n  z: 0.0\n  a: 1.0\n---\nflowchart LR\n  A --> B\n";
        let current_source = "---\nconfig:\n  a: 1\n  z: 0\n---\nflowchart LR\n  A --> B\n";
        let different_diagram = "---\nconfig:\n  a: 1\n  z: 0\n---\nflowchart LR\n  A --> C\n";

        assert_eq!(
            canonical_cypress_fixture_identity(old_fixture),
            canonical_cypress_fixture_identity(current_source)
        );
        assert_ne!(
            canonical_cypress_fixture_identity(current_source),
            canonical_cypress_fixture_identity(different_diagram)
        );
    }

    #[test]
    fn cypress_frontmatter_overrides_initialize_options() {
        let source = r#"---
config:
  theme: base
  themeVariables:
    primaryColor: '#ff0000'
---
flowchart LR
  A --> B
"#;
        let materialized = materialize_cypress_fixture_source(
            source,
            CypressRenderHelper::RenderGraph,
            &serde_json::json!({
                "theme": "forest",
                "securityLevel": "loose",
                "themeVariables": {
                    "primaryColor": "#00ff00",
                    "fontFamily": "sans-serif",
                },
            }),
        )
        .expect("static options should materialize");
        let (yaml, _) = split_cypress_yaml_frontmatter(&materialized)
            .expect("materialized fixture should retain frontmatter");
        let frontmatter: serde_json::Value =
            serde_saphyr::from_str(yaml).expect("frontmatter should parse");

        assert_eq!(frontmatter["config"]["theme"], "base");
        assert_eq!(frontmatter["config"]["securityLevel"], "loose");
        assert_eq!(
            frontmatter["config"]["themeVariables"]["primaryColor"],
            "#ff0000"
        );
        assert_eq!(
            frontmatter["config"]["themeVariables"]["fontFamily"],
            "sans-serif"
        );
    }

    #[test]
    fn cypress_img_snapshot_defaults_are_materialized_before_frontmatter() {
        let source = r#"---
config:
  fontSize: 20px
  architecture:
    seed: 7
---
flowchart LR
  A --> B
"#;
        let materialized = materialize_cypress_fixture_source(
            source,
            CypressRenderHelper::ImgSnapshotTest,
            &serde_json::json!({
                "fontFamily": "Fira Code",
                "sequence": {
                    "noteFontFamily": "monospace",
                    "actorFontFamily": "ignored",
                },
                "architecture": { "rankSpacing": 42 },
                "cynefin": { "seed": 9 },
            }),
        )
        .expect("static options should materialize");
        let (yaml, _) = split_cypress_yaml_frontmatter(&materialized)
            .expect("materialized fixture should retain frontmatter");
        let frontmatter: serde_json::Value =
            serde_saphyr::from_str(yaml).expect("frontmatter should parse");
        let config = &frontmatter["config"];

        assert_eq!(config["fontFamily"], "Fira Code");
        assert_eq!(config["fontSize"], "20px");
        assert_eq!(config["handDrawnSeed"], 1);
        assert_eq!(config["architecture"]["seed"], 7);
        assert_eq!(config["architecture"]["rankSpacing"], 42);
        assert_eq!(config["cynefin"]["seed"], 9);
        assert_eq!(config["sequence"]["actorFontFamily"], "courier");
        assert_eq!(config["sequence"]["noteFontFamily"], "monospace");
        assert_eq!(config["sequence"]["messageFontFamily"], "courier");
    }

    #[test]
    fn cypress_render_graph_defaults_materialize_without_img_snapshot_fonts() {
        let materialized = materialize_cypress_fixture_source(
            "flowchart LR\n  A --> B\n",
            CypressRenderHelper::RenderGraph,
            &serde_json::json!({}),
        )
        .expect("static options should materialize");
        let (yaml, _) = split_cypress_yaml_frontmatter(&materialized)
            .expect("renderGraph defaults should create frontmatter");
        let frontmatter: serde_json::Value =
            serde_saphyr::from_str(yaml).expect("frontmatter should parse");
        let config = &frontmatter["config"];

        assert_eq!(config["handDrawnSeed"], 1);
        assert_eq!(config["architecture"]["seed"], 1);
        assert_eq!(config["cynefin"]["seed"], 1);
        assert!(config.get("fontFamily").is_none());
        assert!(config.get("fontSize").is_none());
        assert!(config.get("sequence").is_none());
    }

    #[test]
    fn cypress_stem_resolution_keeps_distinct_exact_content_identities() {
        let diagram = "flowchart";
        let mut claimed = HashSet::new();

        let first = resolve_existing_cypress_stem(
            diagram,
            "source_case_001",
            true,
            &["source_case_001".to_string()],
            &claimed,
            &HashSet::new(),
        )
        .expect("first exact identity should resolve");
        claimed.insert((diagram.to_string(), first.clone()));
        let second = resolve_existing_cypress_stem(
            diagram,
            "source_case_002",
            true,
            &["source_case_002".to_string()],
            &claimed,
            &HashSet::new(),
        )
        .expect("second exact identity should resolve");

        assert_eq!(first, "source_case_001");
        assert_eq!(second, "source_case_002");
    }

    #[test]
    fn cypress_stem_resolution_prefers_unclaimed_content_identity() {
        let claimed = HashSet::from([("flowchart".to_string(), "source_case_001".to_string())]);

        assert_eq!(
            resolve_existing_cypress_stem(
                "flowchart",
                "source_case_003",
                false,
                &["source_case_002".to_string()],
                &claimed,
                &HashSet::new(),
            ),
            Ok("source_case_002".to_string())
        );
    }

    #[test]
    fn cypress_stem_resolution_prefers_body_identity_over_a_drifted_ordinal() {
        assert_eq!(
            resolve_existing_cypress_stem(
                "flowchart",
                "source_case_002",
                true,
                &["source_case_001".to_string()],
                &HashSet::new(),
                &HashSet::new(),
            ),
            Ok("source_case_001".to_string())
        );
    }

    #[test]
    fn cypress_stem_resolution_does_not_claim_another_candidates_exact_identity() {
        let reserved = HashSet::from([("flowchart".to_string(), "source_case_001".to_string())]);

        let earlier_complex_candidate = resolve_existing_cypress_stem(
            "flowchart",
            "source_case_003",
            false,
            &["source_case_001".to_string()],
            &HashSet::new(),
            &reserved,
        )
        .expect("candidate must keep its own identity");
        let exact_candidate = resolve_existing_cypress_stem(
            "flowchart",
            "source_case_001",
            true,
            &["source_case_001".to_string()],
            &HashSet::from([("flowchart".to_string(), earlier_complex_candidate.clone())]),
            &reserved,
        )
        .expect("reserved exact identity must remain available");

        assert_eq!(earlier_complex_candidate, "source_case_003");
        assert_eq!(exact_candidate, "source_case_001");
    }

    #[test]
    fn cypress_stem_resolution_keeps_new_content_at_its_hash_identity() {
        assert_eq!(
            resolve_existing_cypress_stem(
                "flowchart",
                "source_case_hash",
                false,
                &[],
                &HashSet::new(),
                &HashSet::new(),
            ),
            Ok("source_case_hash".to_string())
        );
    }

    #[test]
    fn cypress_stem_resolution_rejects_duplicate_content_identities() {
        let body_matches = ["source_case_001".to_string(), "source_case_002".to_string()];

        assert_eq!(
            resolve_existing_cypress_stem(
                "flowchart",
                "source_case_hash",
                false,
                &body_matches,
                &HashSet::new(),
                &HashSet::new(),
            ),
            Err(body_matches.to_vec())
        );
    }

    #[test]
    fn cypress_cynefin_detector_id_maps_to_its_fixture_directory() {
        assert_eq!(normalize_imported_diagram_dir("cynefin"), Some("cynefin"));
    }

    #[test]
    fn cypress_railroad_detector_ids_map_to_their_fixture_directories() {
        let actual = ["railroad", "railroadEbnf", "railroadAbnf", "railroadPeg"]
            .map(normalize_imported_diagram_dir);

        assert_eq!(
            actual,
            [
                Some("railroad"),
                Some("railroadEbnf"),
                Some("railroadAbnf"),
                Some("railroadPeg"),
            ]
        );
    }
}

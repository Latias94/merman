use crate::{
    DiagnosticFix, DiagnosticFixEdit, SourceMap,
    source_directives::{ByteSpan, init_directive_spans},
};
use merman_core::{
    Engine, ParseOptions,
    preprocess::{FrontmatterBlock, parse_frontmatter_yaml_fields, split_frontmatter_block},
};
use serde_json::{Map, Value};
use std::sync::OnceLock;

pub(crate) fn init_directives_to_frontmatter_fix(
    source: &str,
    source_map: &SourceMap,
) -> Option<DiagnosticFix> {
    let init_directives = init_directive_spans(source);
    if init_directives.is_empty() {
        return None;
    }

    let config = merged_diagram_config(source)?;
    if matches!(&config, Value::Object(map) if map.is_empty()) {
        return None;
    }

    let removals = init_directives
        .into_iter()
        .map(|directive| directive_removal_span(source, directive.full))
        .collect::<Vec<_>>();

    frontmatter_config_fix(
        source,
        source_map,
        config,
        removals,
        "Move init directive config into frontmatter",
    )
}

pub(crate) fn frontmatter_config_fix(
    source: &str,
    source_map: &SourceMap,
    config: Value,
    removals: Vec<ByteSpan>,
    title: &'static str,
) -> Option<DiagnosticFix> {
    let frontmatter = frontmatter_config_edit(source, config)?;
    let mut removals = removals;
    removals.sort_by_key(|span| (span.start, span.end));
    removals.dedup();

    let mut edits = Vec::new();
    if frontmatter.span.start == 0 && frontmatter.span.end == 0 {
        if let Some(first_removal) = removals.first().copied().filter(|span| span.start == 0) {
            removals.remove(0);
            edits.push(DiagnosticFixEdit::new(
                source_map
                    .span(first_removal.start, first_removal.end)
                    .ok()?,
                frontmatter.replacement,
            ));
        } else {
            edits.push(DiagnosticFixEdit::new(
                source_map
                    .span(frontmatter.span.start, frontmatter.span.end)
                    .ok()?,
                frontmatter.replacement,
            ));
        }
    } else {
        edits.push(DiagnosticFixEdit::new(
            source_map
                .span(frontmatter.span.start, frontmatter.span.end)
                .ok()?,
            frontmatter.replacement,
        ));
    }

    for removal in removals {
        edits.push(DiagnosticFixEdit::new(
            source_map.span(removal.start, removal.end).ok()?,
            "",
        ));
    }

    Some(DiagnosticFix::new(title, edits).preferred())
}

fn merged_diagram_config(source: &str) -> Option<Value> {
    migration_engine()
        .parse_metadata_sync(source, ParseOptions::strict())
        .ok()
        .flatten()
        .map(|metadata| metadata.config.as_value().clone())
}

fn migration_engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(Engine::new)
}

struct FrontmatterEdit {
    span: ByteSpan,
    replacement: String,
}

fn frontmatter_config_edit(source: &str, config: Value) -> Option<FrontmatterEdit> {
    let newline = newline_for_source(source);
    let Some(frontmatter) = split_frontmatter_block(source) else {
        return Some(FrontmatterEdit {
            span: ByteSpan { start: 0, end: 0 },
            replacement: frontmatter_document(
                frontmatter_fields_with_config(Map::new(), config)?,
                "",
                newline,
            )?,
        });
    };

    let newline = newline_for_source(&source[frontmatter.full.start..frontmatter.full.end]);
    let existing_fields = parse_frontmatter_fields(frontmatter.dedented_body.as_ref())?;
    if !existing_fields.contains_key("config") {
        return Some(FrontmatterEdit {
            span: ByteSpan {
                start: frontmatter.body.end,
                end: frontmatter.body.end,
            },
            replacement: frontmatter_config_insertion(source, &frontmatter, config, newline)?,
        });
    }
    if frontmatter_contains_lossy_yaml_syntax(frontmatter.dedented_body.as_ref()) {
        return None;
    }

    Some(FrontmatterEdit {
        span: ByteSpan {
            start: frontmatter.full.start,
            end: frontmatter.full.end,
        },
        replacement: frontmatter_document(
            frontmatter_fields_with_config(existing_fields, config)?,
            frontmatter.indent,
            newline,
        )?,
    })
}

fn frontmatter_fields_with_config(
    mut fields: Map<String, Value>,
    config: Value,
) -> Option<Map<String, Value>> {
    fields.insert("config".to_string(), config);
    Some(fields)
}

fn frontmatter_config_insertion(
    source: &str,
    frontmatter: &FrontmatterBlock<'_>,
    config: Value,
    newline: &str,
) -> Option<String> {
    let mut fields = Map::new();
    fields.insert("config".to_string(), config);
    let body = frontmatter_body(fields)?;
    let mut insertion = String::new();
    let existing_body = &source[frontmatter.body.start..frontmatter.body.end];
    let insertion_splits_crlf =
        existing_body.ends_with('\r') && source[frontmatter.body.end..].starts_with('\n');
    if !existing_body.trim().is_empty() {
        if insertion_splits_crlf {
            insertion.push('\n');
        } else if !existing_body.ends_with('\n') {
            insertion.push_str(newline);
        }
    }
    insertion.push_str(&frontmatter_body_with_indent(
        &body,
        frontmatter.indent,
        newline,
    ));
    if insertion_splits_crlf {
        insertion.push('\r');
    } else if frontmatter.body.start == frontmatter.body.end {
        insertion.push_str(newline);
    }
    Some(insertion)
}

fn frontmatter_contains_lossy_yaml_syntax(yaml_body: &str) -> bool {
    yaml_body
        .lines()
        .any(frontmatter_line_contains_lossy_yaml_syntax)
}

fn frontmatter_line_contains_lossy_yaml_syntax(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('#')
        || line.contains(" #")
        || line.contains(" &")
        || line.contains(": &")
        || line.contains(" *")
        || line.contains(": *")
        || trimmed.starts_with("<<:")
        || trimmed == "?"
        || trimmed.starts_with("? ")
        || frontmatter_line_starts_with_flow_complex_key(trimmed)
        || trimmed.starts_with('!')
        || trimmed.starts_with("- !")
        || line.contains(": !")
        || frontmatter_line_uses_block_scalar(trimmed)
}

fn frontmatter_line_starts_with_flow_complex_key(trimmed: &str) -> bool {
    flow_complex_key_colon_offset(trimmed).is_some()
        || trimmed
            .strip_prefix("- ")
            .is_some_and(|item| flow_complex_key_colon_offset(item.trim_start()).is_some())
}

fn flow_complex_key_colon_offset(trimmed: &str) -> Option<usize> {
    let close = match trimmed.as_bytes().first().copied()? {
        b'[' => b']',
        b'{' => b'}',
        _ => return None,
    };
    let close_offset = trimmed.as_bytes().iter().position(|byte| *byte == close)?;
    trimmed[close_offset + 1..]
        .trim_start()
        .starts_with(':')
        .then_some(close_offset)
}

fn frontmatter_line_uses_block_scalar(trimmed: &str) -> bool {
    trimmed
        .split_once(':')
        .map(|(_, value)| value.trim_start())
        .is_some_and(|value| value.starts_with('|') || value.starts_with('>'))
}

fn frontmatter_document(fields: Map<String, Value>, indent: &str, newline: &str) -> Option<String> {
    let body = frontmatter_body(fields)?;
    Some(frontmatter_document_with_indent(&body, indent, newline))
}

fn frontmatter_body(fields: Map<String, Value>) -> Option<String> {
    let mut body = serde_yaml::to_string(&Value::Object(fields)).ok()?;
    if let Some(stripped) = body.strip_prefix("---\n") {
        body = stripped.to_string();
    }
    if let Some(stripped) = body.strip_suffix("...\n") {
        body = stripped.to_string();
    }
    if !body.ends_with('\n') {
        body.push('\n');
    }
    Some(body)
}

fn frontmatter_body_with_indent(body: &str, indent: &str, newline: &str) -> String {
    let mut document = String::with_capacity(body.len() + (indent.len() * body.lines().count()));
    for (index, line) in body.split_inclusive('\n').enumerate() {
        if index > 0 {
            document.push_str(newline);
        }
        document.push_str(indent);
        document.push_str(line.trim_end_matches('\n'));
    }
    document
}

fn frontmatter_document_with_indent(body: &str, indent: &str, newline: &str) -> String {
    let mut document =
        String::with_capacity(body.len() + (indent.len() * (body.lines().count() + 2)) + 8);
    document.push_str(indent);
    document.push_str("---");
    document.push_str(newline);
    for line in body.split_terminator('\n') {
        document.push_str(indent);
        document.push_str(line);
        document.push_str(newline);
    }
    document.push_str(indent);
    document.push_str("---");
    document.push_str(newline);
    document
}

fn parse_frontmatter_fields(yaml_body: &str) -> Option<Map<String, Value>> {
    parse_frontmatter_yaml_fields(yaml_body).ok()
}

fn newline_for_source(source: &str) -> &'static str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn directive_removal_span(source: &str, directive: ByteSpan) -> ByteSpan {
    let line_start = source[..directive.start]
        .rfind('\n')
        .map_or(0, |idx| idx + 1);
    let line_end = source[directive.end..]
        .find('\n')
        .map_or(source.len(), |relative| directive.end + relative);
    let line_end_with_newline = if line_end < source.len() {
        line_end + 1
    } else {
        line_end
    };

    if source[line_start..directive.start].trim().is_empty()
        && source[directive.end..line_end].trim().is_empty()
    {
        ByteSpan {
            start: line_start,
            end: line_end_with_newline,
        }
    } else {
        directive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply_fix(source: &str, fix: &DiagnosticFix) -> String {
        let mut edited = source.to_string();
        let mut edits = fix.edits.clone();
        edits.sort_by(|left, right| {
            right
                .span
                .byte_start
                .cmp(&left.span.byte_start)
                .then_with(|| right.span.byte_end.cmp(&left.span.byte_end))
        });

        for edit in edits {
            edited.replace_range(edit.span.byte_start..edit.span.byte_end, &edit.replacement);
        }

        edited
    }

    fn assert_only_crlf_newlines(text: &str) {
        let bytes = text.as_bytes();
        for (index, byte) in bytes.iter().enumerate() {
            if *byte == b'\n' {
                assert!(index > 0 && bytes[index - 1] == b'\r');
            }
        }
    }

    #[test]
    fn init_directive_migration_inserts_frontmatter_and_removes_directive_line() {
        let source = "%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        let fix = init_directives_to_frontmatter_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);

        assert!(fix.is_preferred);
        assert!(edited.starts_with("---\nconfig:\n"));
        assert!(edited.contains("theme: dark\n"));
        assert!(!edited.contains("%%{ init"));
        assert!(edited.contains("flowchart TD\nA-->B\n"));
        assert_eq!(fix.edits.len(), 1);
    }

    #[test]
    fn init_directive_migration_preserves_existing_frontmatter_fields() {
        let source = "---\ntitle: Demo\ncustom: keep\nconfig:\n  theme: default\n---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        let fix = init_directives_to_frontmatter_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);

        assert!(edited.starts_with("---\ntitle: Demo\ncustom: keep\nconfig:\n"));
        assert!(edited.contains("theme: dark\n"));
        assert!(!edited.contains("%%{ init"));
        assert_eq!(fix.edits.len(), 2);
    }

    #[test]
    fn init_directive_migration_preserves_crlf_when_creating_frontmatter() {
        let source = "%%{ init: {\"theme\":\"dark\"} }%%\r\nflowchart TD\r\nA-->B\r\n";
        let source_map = SourceMap::new(source);

        let fix = init_directives_to_frontmatter_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);

        assert!(edited.starts_with("---\r\nconfig:\r\n"));
        assert!(edited.contains("theme: dark\r\n"));
        assert!(!edited.contains("%%{ init"));
        assert_only_crlf_newlines(&edited);
    }

    #[test]
    fn init_directive_migration_preserves_crlf_when_inserting_config() {
        let source = "---\r\ntitle: Demo\r\ncustom: keep\r\n---\r\n%%{ init: {\"theme\":\"dark\"} }%%\r\nflowchart TD\r\nA-->B\r\n";
        let source_map = SourceMap::new(source);

        let fix = init_directives_to_frontmatter_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);

        assert!(
            edited.starts_with("---\r\ntitle: Demo\r\ncustom: keep\r\nconfig:\r\n"),
            "{edited:?}"
        );
        assert!(edited.contains("theme: dark\r\n"));
        assert!(!edited.contains("%%{ init"));
        assert_only_crlf_newlines(&edited);
    }

    #[test]
    fn init_directive_migration_preserves_crlf_when_rewriting_config() {
        let source = "---\r\ntitle: Demo\r\nconfig:\r\n  theme: default\r\n---\r\n%%{ init: {\"theme\":\"dark\"} }%%\r\nflowchart TD\r\nA-->B\r\n";
        let source_map = SourceMap::new(source);

        let fix = init_directives_to_frontmatter_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);

        assert!(edited.starts_with("---\r\ntitle: Demo\r\nconfig:\r\n"));
        assert!(edited.contains("theme: dark\r\n"));
        assert!(!edited.contains("%%{ init"));
        assert_only_crlf_newlines(&edited);
    }

    #[test]
    fn init_directive_migration_inserts_config_without_dropping_frontmatter_comments() {
        let source = "---\n# keep rationale\ntitle: Demo\ncustom: keep\n---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        let fix = init_directives_to_frontmatter_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);

        assert!(edited.starts_with("---\n# keep rationale\ntitle: Demo\ncustom: keep\nconfig:\n"));
        assert!(edited.contains("theme: dark\n"));
        assert!(!edited.contains("%%{ init"));
        assert_eq!(edited.matches("# keep rationale").count(), 1);
        assert_eq!(fix.edits.len(), 2);
    }

    #[test]
    fn init_directive_migration_skips_lossy_config_rewrite_for_commented_frontmatter() {
        let source = "---\n# keep rationale\ntitle: Demo\nconfig:\n  theme: default\n---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        assert!(init_directives_to_frontmatter_fix(source, &source_map).is_none());
    }

    #[test]
    fn init_directive_migration_ignores_non_string_frontmatter_keys_without_dropping_fields() {
        let source = "---\ntitle: Demo\n? [non, string, key]\n: ignored\ncustom: keep\nconfig:\n  theme: default\n---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        assert!(init_directives_to_frontmatter_fix(source, &source_map).is_none());
    }

    #[test]
    fn init_directive_migration_skips_lossy_config_rewrite_for_block_scalar_frontmatter() {
        let source = "---\ntitle: Demo\nnotes: |\n  keep exact text\nconfig:\n  theme: default\n---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        assert!(init_directives_to_frontmatter_fix(source, &source_map).is_none());
    }

    #[test]
    fn init_directive_migration_skips_lossy_config_rewrite_for_flow_style_complex_keys() {
        for source in [
            "---\ntitle: Demo\n[non, string, key]: ignored\nconfig:\n  theme: default\n---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n",
            "---\ntitle: Demo\nmetadata:\n  [non, string, key]: ignored\nconfig:\n  theme: default\n---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n",
        ] {
            let source_map = SourceMap::new(source);

            assert!(init_directives_to_frontmatter_fix(source, &source_map).is_none());
        }
    }

    #[test]
    fn init_directive_migration_preserves_effective_diagram_config() {
        let source = "---\nconfig:\n  flowchart:\n    curve: basis\n---\n%%{ initialize: {\"theme\":\"dark\",\"flowchart\":{\"htmlLabels\":false}} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let engine = Engine::new();
        let original = engine
            .parse_metadata_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("original metadata");

        let fix = init_directives_to_frontmatter_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);
        let migrated = engine
            .parse_metadata_sync(&edited, ParseOptions::strict())
            .unwrap()
            .expect("migrated metadata");

        assert_eq!(migrated.config.as_value(), original.config.as_value());
        assert!(!edited.contains("%%{ initialize"));
    }

    #[test]
    fn init_directive_migration_updates_indented_frontmatter_with_core_semantics() {
        let source = "  ---\n  title: Demo\n  config:\n    theme: default\n  ---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let engine = Engine::new();
        let original = engine
            .parse_metadata_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("original metadata");

        let fix = init_directives_to_frontmatter_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);
        let migrated = engine
            .parse_metadata_sync(&edited, ParseOptions::strict())
            .unwrap()
            .expect("migrated metadata");

        assert!(edited.starts_with("  ---\n"));
        assert!(!edited.starts_with("---\n  ---\n"));
        assert_eq!(edited.matches("title: Demo").count(), 1);
        assert!(!edited.contains("%%{ init"));
        assert_eq!(migrated.config.as_value(), original.config.as_value());
        assert_eq!(migrated.title.as_deref(), Some("Demo"));
    }
}

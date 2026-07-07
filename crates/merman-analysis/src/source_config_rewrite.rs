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

pub(crate) fn flowchart_html_labels_to_root_fix(
    source: &str,
    source_map: &SourceMap,
) -> Option<DiagnosticFix> {
    let mut config = merged_deprecated_html_labels_config(source)?;
    if !move_flowchart_html_labels_to_root(&mut config) {
        return None;
    }

    let removals = init_directive_spans(source)
        .into_iter()
        .map(|directive| directive_removal_span(source, directive.full))
        .collect::<Vec<_>>();

    frontmatter_config_fix(
        source,
        source_map,
        config,
        removals,
        "Move deprecated `flowchart.htmlLabels` to root `htmlLabels`",
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

fn merged_deprecated_html_labels_config(source: &str) -> Option<Value> {
    merged_diagram_config(source)
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
    let Some(frontmatter) = split_frontmatter_block(source) else {
        return Some(FrontmatterEdit {
            span: ByteSpan { start: 0, end: 0 },
            replacement: frontmatter_document(
                frontmatter_fields_with_config(Map::new(), config)?,
                "",
            )?,
        });
    };

    let existing_fields = parse_frontmatter_fields(frontmatter.dedented_body.as_ref())?;
    if !existing_fields.contains_key("config") {
        return Some(FrontmatterEdit {
            span: ByteSpan {
                start: frontmatter.body.end,
                end: frontmatter.body.end,
            },
            replacement: frontmatter_config_insertion(source, &frontmatter, config)?,
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
) -> Option<String> {
    let mut fields = Map::new();
    fields.insert("config".to_string(), config);
    let body = frontmatter_body(fields)?;
    let mut insertion = String::new();
    if !source[frontmatter.body.start..frontmatter.body.end]
        .trim()
        .is_empty()
    {
        insertion.push('\n');
    }
    insertion.push_str(&frontmatter_body_with_indent(&body, frontmatter.indent));
    if frontmatter.body.start == frontmatter.body.end {
        insertion.push('\n');
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
        || trimmed.starts_with('!')
        || trimmed.starts_with("- !")
        || line.contains(": !")
        || frontmatter_line_uses_block_scalar(trimmed)
}

fn frontmatter_line_uses_block_scalar(trimmed: &str) -> bool {
    trimmed
        .split_once(':')
        .map(|(_, value)| value.trim_start())
        .is_some_and(|value| value.starts_with('|') || value.starts_with('>'))
}

fn frontmatter_document(fields: Map<String, Value>, indent: &str) -> Option<String> {
    let body = frontmatter_body(fields)?;
    Some(frontmatter_document_with_indent(&body, indent))
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

fn frontmatter_body_with_indent(body: &str, indent: &str) -> String {
    let mut document = String::with_capacity(body.len() + (indent.len() * body.lines().count()));
    for (index, line) in body.split_inclusive('\n').enumerate() {
        if index > 0 {
            document.push('\n');
        }
        document.push_str(indent);
        document.push_str(line.trim_end_matches('\n'));
    }
    document
}

fn frontmatter_document_with_indent(body: &str, indent: &str) -> String {
    let mut document =
        String::with_capacity(body.len() + (indent.len() * (body.lines().count() + 2)) + 8);
    document.push_str(indent);
    document.push_str("---\n");
    for line in body.split_inclusive('\n') {
        document.push_str(indent);
        document.push_str(line);
    }
    document.push_str(indent);
    document.push_str("---\n");
    document
}

fn move_flowchart_html_labels_to_root(config: &mut Value) -> bool {
    let Value::Object(root) = config else {
        return false;
    };
    let root_had_html_labels = root.contains_key("htmlLabels");

    let Some((html_labels, cleanup_path)) = take_flowchart_html_labels(root) else {
        return false;
    };

    if !root_had_html_labels {
        root.insert("htmlLabels".to_string(), html_labels);
    }

    match cleanup_path {
        FlowchartHtmlLabelsPath::Direct { flowchart_is_empty } => {
            if flowchart_is_empty {
                root.remove("flowchart");
            }
        }
        FlowchartHtmlLabelsPath::DirectiveConfigWrappedFlowchart { flowchart_is_empty } => {
            if flowchart_is_empty {
                root.remove("flowchart");
            }
        }
    }

    true
}

enum FlowchartHtmlLabelsPath {
    Direct { flowchart_is_empty: bool },
    DirectiveConfigWrappedFlowchart { flowchart_is_empty: bool },
}

fn take_flowchart_html_labels(
    root: &mut Map<String, Value>,
) -> Option<(Value, FlowchartHtmlLabelsPath)> {
    if let Some(result) = take_direct_flowchart_html_labels(root) {
        return Some(result);
    }
    if let Some(result) = take_directive_config_wrapped_flowchart_html_labels(root) {
        return Some(result);
    }
    None
}

fn take_direct_flowchart_html_labels(
    root: &mut Map<String, Value>,
) -> Option<(Value, FlowchartHtmlLabelsPath)> {
    let Value::Object(flowchart) = root.get_mut("flowchart")? else {
        return None;
    };
    let html_labels = flowchart.remove("htmlLabels")?;
    let flowchart_is_empty = flowchart.is_empty();

    Some((
        html_labels,
        FlowchartHtmlLabelsPath::Direct { flowchart_is_empty },
    ))
}

fn take_directive_config_wrapped_flowchart_html_labels(
    root: &mut Map<String, Value>,
) -> Option<(Value, FlowchartHtmlLabelsPath)> {
    let Value::Object(flowchart) = root.get_mut("flowchart")? else {
        return None;
    };
    let (html_labels, nested_flowchart_is_empty) = {
        let Value::Object(nested_flowchart) = flowchart.get_mut("flowchart")? else {
            return None;
        };
        let html_labels = nested_flowchart.remove("htmlLabels")?;
        (html_labels, nested_flowchart.is_empty())
    };
    if nested_flowchart_is_empty {
        flowchart.remove("flowchart");
    }
    let flowchart_is_empty = flowchart.is_empty();

    Some((
        html_labels,
        FlowchartHtmlLabelsPath::DirectiveConfigWrappedFlowchart { flowchart_is_empty },
    ))
}

fn parse_frontmatter_fields(yaml_body: &str) -> Option<Map<String, Value>> {
    parse_frontmatter_yaml_fields(yaml_body).ok()
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

    #[test]
    fn flowchart_html_labels_to_root_fix_promotes_deprecated_key() {
        let source = "---\nconfig:\n  flowchart:\n    curve: basis\n---\n%%{ init: {\"flowchart\":{\"htmlLabels\":false}} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        let fix = flowchart_html_labels_to_root_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);

        assert!(fix.is_preferred);
        assert!(edited.contains("htmlLabels: false"));
        assert!(!edited.contains("flowchart:\n    htmlLabels: false"));
        assert!(edited.contains("config:\n  flowchart:\n    curve: basis"));
    }

    #[test]
    fn flowchart_html_labels_to_root_fix_preserves_unrelated_nested_config_key() {
        let source = "---\nconfig:\n  config:\n    keep: true\n---\n%%{ init: {\"flowchart\":{\"htmlLabels\":false}} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let engine = Engine::new();

        let fix = flowchart_html_labels_to_root_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);
        let migrated = engine
            .parse_metadata_sync(&edited, ParseOptions::strict())
            .unwrap()
            .expect("migrated metadata");
        let config = migrated.config.as_value();

        assert_eq!(config.pointer("/htmlLabels"), Some(&Value::Bool(false)));
        assert_eq!(config.pointer("/config/keep"), Some(&Value::Bool(true)));
        assert!(config.get("keep").is_none());
    }

    #[test]
    fn flowchart_html_labels_to_root_fix_promotes_raw_config_html_labels() {
        let source = "%%{init: {\"config\": {\"htmlLabels\": false, \"secure\": [\"x\"], \"__proto__\": {\"polluted\": true}, \"themeCSS\": \"url(data:text/css,a)\"}, \"theme\": \"base\"}}%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        let fix = flowchart_html_labels_to_root_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);

        assert!(fix.is_preferred);
        assert!(edited.contains("config:"));
        assert!(edited.contains("htmlLabels: false"));
        assert!(edited.contains("theme: base"));
        assert!(!edited.contains("secure"));
        assert!(!edited.contains("__proto__"));
        assert!(!edited.contains("polluted"));
        assert!(!edited.contains("url(data:"));
        assert!(!edited.contains("config:\n  config:"));
    }

    #[test]
    fn flowchart_html_labels_to_root_fix_uses_core_semantics_for_config_wrapped_flowchart() {
        let source = "%%{init: {\"config\": {\"flowchart\": {\"htmlLabels\": true, \"curve\": \"basis\", \"secure\": [\"x\"], \"__proto__\": {\"polluted\": true}, \"themeCSS\": \"url(data:text/css,a)\"}}, \"theme\": \"base\"}}%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        let fix = flowchart_html_labels_to_root_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);

        assert!(fix.is_preferred);
        assert!(edited.contains("htmlLabels: true"));
        assert!(edited.contains("theme: base"));
        assert!(edited.contains("curve: basis"));
        assert!(!edited.contains("secure"));
        assert!(!edited.contains("__proto__"));
        assert!(!edited.contains("polluted"));
        assert!(!edited.contains("url(data:"));
        assert!(!edited.contains("flowchart:\n    htmlLabels: true"));
    }
}

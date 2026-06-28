use crate::{
    DiagnosticFix, DiagnosticFixEdit, SourceMap,
    source_directives::{ByteSpan, init_directive_spans},
};
use merman_core::{Engine, ParseOptions};
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
    let mut config = merged_diagram_config(source)?;
    for directive_config in init_directive_config_values(source) {
        deep_merge_config(&mut config, directive_config);
    }
    Some(config)
}

fn migration_engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(Engine::new)
}

fn init_directive_config_values(source: &str) -> Vec<Value> {
    init_directive_spans(source)
        .into_iter()
        .filter_map(|directive| {
            let body = source.get(directive.keyword.end..directive.full.end)?;
            let rest = body.trim_start();
            let rest = rest.strip_prefix(':')?.trim_start();
            let body = rest.strip_suffix("}%%").unwrap_or(rest).trim();
            json5::from_str::<Value>(body).ok()
        })
        .collect()
}

fn deep_merge_config(target: &mut Value, source: Value) {
    match (target, source) {
        (Value::Object(target), Value::Object(source)) => {
            for (key, value) in source {
                match target.get_mut(&key) {
                    Some(existing) => deep_merge_config(existing, value),
                    None => {
                        target.insert(key, value);
                    }
                }
            }
        }
        (target, source) => {
            *target = source;
        }
    }
}

struct FrontmatterEdit {
    span: ByteSpan,
    replacement: String,
}

fn frontmatter_config_edit(source: &str, config: Value) -> Option<FrontmatterEdit> {
    let Some(frontmatter) = split_frontmatter(source) else {
        return Some(FrontmatterEdit {
            span: ByteSpan { start: 0, end: 0 },
            replacement: frontmatter_document(frontmatter_fields_with_config(Map::new(), config)?)?,
        });
    };

    let existing_body = source.get(frontmatter.body.start..frontmatter.body.end)?;
    let existing_fields = parse_frontmatter_fields(existing_body)?;

    Some(FrontmatterEdit {
        span: frontmatter.full,
        replacement: frontmatter_document(frontmatter_fields_with_config(
            existing_fields,
            config,
        )?)?,
    })
}

fn frontmatter_fields_with_config(
    mut fields: Map<String, Value>,
    config: Value,
) -> Option<Map<String, Value>> {
    fields.insert("config".to_string(), config);
    Some(fields)
}

fn frontmatter_document(fields: Map<String, Value>) -> Option<String> {
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
    Some(format!("---\n{body}---\n"))
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
        FlowchartHtmlLabelsPath::ConfigWrapped {
            flowchart_is_empty,
            config_is_empty,
        } => {
            if flowchart_is_empty && let Some(Value::Object(config)) = root.get_mut("config") {
                config.remove("flowchart");
            }
            if config_is_empty {
                root.remove("config");
            }
        }
    }

    true
}

enum FlowchartHtmlLabelsPath {
    Direct {
        flowchart_is_empty: bool,
    },
    ConfigWrapped {
        flowchart_is_empty: bool,
        config_is_empty: bool,
    },
}

fn take_flowchart_html_labels(
    root: &mut Map<String, Value>,
) -> Option<(Value, FlowchartHtmlLabelsPath)> {
    let (html_labels, flowchart_is_empty) = {
        let Some(Value::Object(flowchart)) = root.get_mut("flowchart") else {
            return take_config_wrapped_flowchart_html_labels(root);
        };
        let Some(html_labels) = flowchart.remove("htmlLabels") else {
            return take_config_wrapped_flowchart_html_labels(root);
        };
        let flowchart_is_empty = flowchart.is_empty();
        (html_labels, flowchart_is_empty)
    };

    Some((
        html_labels,
        FlowchartHtmlLabelsPath::Direct { flowchart_is_empty },
    ))
}

fn take_config_wrapped_flowchart_html_labels(
    root: &mut Map<String, Value>,
) -> Option<(Value, FlowchartHtmlLabelsPath)> {
    let Value::Object(config) = root.get_mut("config")? else {
        return None;
    };
    let (html_labels, flowchart_is_empty, config_is_empty) = {
        let Value::Object(flowchart) = config.get_mut("flowchart")? else {
            return None;
        };
        let html_labels = flowchart.remove("htmlLabels")?;
        let flowchart_is_empty = flowchart.is_empty();
        let config_is_empty = flowchart_is_empty && config.len() == 1;
        (html_labels, flowchart_is_empty, config_is_empty)
    };

    Some((
        html_labels,
        FlowchartHtmlLabelsPath::ConfigWrapped {
            flowchart_is_empty,
            config_is_empty,
        },
    ))
}

fn parse_frontmatter_fields(yaml_body: &str) -> Option<Map<String, Value>> {
    let raw_yaml: serde_yaml::Value = serde_yaml::from_str(yaml_body).ok()?;
    match serde_json::to_value(raw_yaml).unwrap_or(Value::Null) {
        Value::Object(map) => Some(map),
        other => {
            drop(other);
            Some(Map::new())
        }
    }
}

struct FrontmatterBlock {
    full: ByteSpan,
    body: ByteSpan,
}

fn split_frontmatter(source: &str) -> Option<FrontmatterBlock> {
    let after_marker = source.strip_prefix("---")?;
    let open_line_end = after_marker.find('\n')?;
    if !after_marker[..open_line_end].trim().is_empty() {
        return None;
    }

    let body_start = 3 + open_line_end + 1;
    let rest = &source[body_start..];
    let mut offset = 0usize;

    for line in rest.split_inclusive('\n') {
        let without_newline = line.trim_end_matches(['\r', '\n']);
        if without_newline.trim() == "---" {
            let body_end = body_start + offset;
            let full_end = body_start + offset + line.len();
            return Some(FrontmatterBlock {
                full: ByteSpan {
                    start: 0,
                    end: full_end,
                },
                body: ByteSpan {
                    start: body_start,
                    end: body_end,
                },
            });
        }
        offset += line.len();
    }

    None
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
}

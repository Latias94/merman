use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CypressRenderHelper {
    ImgSnapshotTest,
    RenderGraph,
}

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
        return Err("collected Cypress options are not an object".to_string());
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

    // Keep these defaults aligned with the pinned `cypress/helpers/util.ts` implementation.
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
        return Err("collected Cypress options are not an object".to_string());
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

#[cfg(test)]
mod tests {
    use super::{
        CypressRenderHelper, materialize_cypress_fixture_source, split_cypress_yaml_frontmatter,
    };

    #[test]
    fn frontmatter_overrides_collected_initialize_options() {
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
        .expect("collected options should materialize");
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
    fn img_snapshot_defaults_are_materialized_before_frontmatter() {
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
        .expect("collected options should materialize");
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
    fn render_graph_defaults_do_not_add_snapshot_fonts() {
        let materialized = materialize_cypress_fixture_source(
            "flowchart LR\n  A --> B\n",
            CypressRenderHelper::RenderGraph,
            &serde_json::json!({}),
        )
        .expect("collected options should materialize");
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
}

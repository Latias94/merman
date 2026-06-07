use crate::{DetectorRegistry, Error, MermaidConfig, Result};
use regex::Regex;
use serde_json::Value;
use std::borrow::Cow;
use std::sync::OnceLock;

macro_rules! cached_regex {
    ($fn_name:ident, $pat:literal) => {
        fn $fn_name() -> &'static Regex {
            static RE: OnceLock<Regex> = OnceLock::new();
            RE.get_or_init(|| Regex::new($pat).expect("preprocess regex must compile"))
        }
    };
}

cached_regex!(re_crlf, r"\r\n?");
cached_regex!(re_tag, r"<(\w+)([^>]*)>");
cached_regex!(re_attr_eq_double_quoted, "=\"([^\"]*)\"");
cached_regex!(re_style_hex, r"style.*:\S*#.*;");
cached_regex!(re_classdef_hex, r"classDef.*:\S*#.*;");
cached_regex!(re_entity, r"#\w+;");
cached_regex!(re_int, r"^\+?\d+$");
#[derive(Debug, Clone)]
pub struct PreprocessResult {
    pub code: String,
    pub title: Option<String>,
    pub config: MermaidConfig,
}

const FRONTMATTER_DIAGRAM_CONFIG_KEYS: &[&str] = &[
    "architecture",
    "block",
    "c4",
    "class",
    "er",
    "flowchart",
    "gantt",
    "gitGraph",
    "journey",
    "kanban",
    "mindmap",
    "packet",
    "pie",
    "quadrantChart",
    "radar",
    "requirement",
    "sankey",
    "sequence",
    "state",
    "timeline",
    "xyChart",
];

const FRONTMATTER_DIAGRAM_CONFIG_ALIASES: &[(&str, &str)] = &[
    ("classDiagram", "class"),
    ("erDiagram", "er"),
    ("stateDiagram", "state"),
    ("xychart", "xyChart"),
];

const MAX_CONFIG_NESTING_DEPTH: usize = crate::MAX_DIAGRAM_NESTING_DEPTH;

pub fn preprocess_diagram(input: &str, registry: &DetectorRegistry) -> Result<PreprocessResult> {
    preprocess_diagram_with_known_type(input, registry, None)
}

pub fn preprocess_diagram_with_known_type(
    input: &str,
    registry: &DetectorRegistry,
    diagram_type: Option<&str>,
) -> Result<PreprocessResult> {
    let cleaned = cleanup_text(input);
    let (without_frontmatter, title, mut frontmatter_config) =
        process_frontmatter(cleaned.as_ref())?;
    let (without_directives, directive_config) =
        process_directives(without_frontmatter, registry, diagram_type)?;

    frontmatter_config.deep_merge(directive_config.as_value());

    let code = crate::utils::cleanup_mermaid_comments(without_directives.as_ref());
    Ok(PreprocessResult {
        code: code.into_owned(),
        title,
        config: frontmatter_config,
    })
}

fn cleanup_text(input: &str) -> Cow<'_, str> {
    let mut s: Cow<'_, str> = if input.contains('\r') {
        Cow::Owned(re_crlf().replace_all(input, "\n").into_owned())
    } else {
        Cow::Borrowed(input)
    };

    // Mermaid encodes `#quot;`-style sequences before parsing (`encodeEntities(...)`).
    // This is required because `#` and `;` are significant in several grammars (comments and
    // statement separators), and the encoded placeholders are later decoded by the renderer.
    //
    // Source of truth: `packages/mermaid/src/utils.ts::encodeEntities` at Mermaid@11.12.2.
    if s.contains('#') {
        s = Cow::Owned(encode_mermaid_entities_like_upstream(s.as_ref()));
    }

    // Mermaid performs this HTML attribute rewrite as part of preprocessing.
    if s.contains('<') && s.contains("=\"") {
        s = Cow::Owned(
            re_tag()
                .replace_all(s.as_ref(), |caps: &regex::Captures| {
                    let tag = &caps[1];
                    let attrs = &caps[2];
                    let attrs = re_attr_eq_double_quoted().replace_all(attrs, "='$1'");
                    format!("<{tag}{attrs}>")
                })
                .into_owned(),
        );
    }

    s
}

fn encode_mermaid_entities_like_upstream(text: &str) -> String {
    if !text.contains('#') {
        return text.to_string();
    }

    // Mirrors Mermaid `encodeEntities` (Mermaid@11.12.2):
    //
    // 1) Protect `style...:#...;` and `classDef...:#...;` so color hex fragments are not mistaken
    //    as entities by the `/#\\w+;/g` pass.
    // 2) Encode `#<name>;` and `#<number>;` sequences into placeholders that do not contain `#`/`;`.
    let mut txt = text.to_string();

    if txt.contains("style") && txt.contains(';') {
        txt = re_style_hex()
            .replace_all(&txt, |caps: &regex::Captures| {
                let s = caps.get(0).map(|m| m.as_str()).unwrap_or_default();
                s.strip_suffix(';').unwrap_or(s).to_string()
            })
            .to_string();
    }

    if txt.contains("classDef") && txt.contains(';') {
        txt = re_classdef_hex()
            .replace_all(&txt, |caps: &regex::Captures| {
                let s = caps.get(0).map(|m| m.as_str()).unwrap_or_default();
                s.strip_suffix(';').unwrap_or(s).to_string()
            })
            .to_string();
    }

    if txt.contains(';') {
        txt = re_entity()
            .replace_all(&txt, |caps: &regex::Captures| {
                let s = caps.get(0).map(|m| m.as_str()).unwrap_or_default();
                let inner = s
                    .strip_prefix('#')
                    .and_then(|s| s.strip_suffix(';'))
                    .unwrap_or("");
                let is_int = re_int().is_match(inner);
                if is_int {
                    format!("ﬂ°°{inner}¶ß")
                } else {
                    format!("ﬂ°{inner}¶ß")
                }
            })
            .to_string();
    }

    txt
}

fn process_frontmatter(input: &str) -> Result<(&str, Option<String>, MermaidConfig)> {
    if !input.trim_start().starts_with("---") {
        return Ok((input, None, MermaidConfig::empty_object()));
    }

    let Some((yaml_body, stripped)) = split_frontmatter(input) else {
        return Ok((input, None, MermaidConfig::empty_object()));
    };

    if config_nesting_exceeds_limit(yaml_body) {
        return Err(Error::InvalidFrontMatterYaml {
            message: format!("config nesting exceeds {MAX_CONFIG_NESTING_DEPTH} levels"),
        });
    }

    let raw_yaml: serde_yaml::Value =
        serde_yaml::from_str(yaml_body).map_err(|e| Error::InvalidFrontMatterYaml {
            message: e.to_string(),
        })?;
    let parsed = serde_json::to_value(raw_yaml).unwrap_or(Value::Null);
    let parsed_obj = match parsed {
        Value::Object(obj) => obj,
        other => {
            crate::config::drop_value_nonrecursive(other);
            Default::default()
        }
    };

    let mut title = None;
    let mut display_mode = None;

    if let Some(Value::String(t)) = parsed_obj.get("title") {
        title = Some(t.clone());
    }
    if let Some(Value::String(dm)) = parsed_obj.get("displayMode") {
        display_mode = Some(dm.clone());
    }

    let mut config = MermaidConfig::empty_object();
    merge_top_level_frontmatter_diagram_configs(&parsed_obj, &mut config);
    if let Some(v) = parsed_obj.get("config") {
        config.deep_merge(v);
    }
    crate::config::mirror_legacy_font_family_into_theme_variables(&mut config);
    if let Some(dm) = display_mode {
        config.set_value("gantt.displayMode", Value::String(dm));
    }

    crate::config::drop_value_nonrecursive(Value::Object(parsed_obj));
    Ok((stripped, title, config))
}

fn split_frontmatter(input: &str) -> Option<(&str, &str)> {
    let after_marker = input.strip_prefix("---")?;
    let open_line_end = after_marker.find('\n')?;
    if !after_marker[..open_line_end].trim().is_empty() {
        return None;
    }

    let body_start = 3 + open_line_end + 1;
    let rest = &input[body_start..];
    let mut offset = 0usize;

    for line in rest.split_inclusive('\n') {
        let without_newline = line.trim_end_matches(['\r', '\n']);
        if without_newline.trim() == "---" {
            let body = &rest[..offset];
            let stripped = &rest[offset + line.len()..];
            return Some((body, stripped));
        }
        offset += line.len();
    }

    None
}

fn merge_top_level_frontmatter_diagram_configs(
    parsed_obj: &serde_json::Map<String, Value>,
    config: &mut MermaidConfig,
) {
    // Mermaid upstream only consumes `config`, but users commonly read docs examples as allowing
    // diagram config namespaces at the YAML root. Keep this compatibility narrow and explicit.
    for &(source_key, target_key) in FRONTMATTER_DIAGRAM_CONFIG_ALIASES {
        if let Some(value) = parsed_obj.get(source_key) {
            config.set_value(target_key, crate::config::clone_value_nonrecursive(value));
        }
    }

    for &key in FRONTMATTER_DIAGRAM_CONFIG_KEYS {
        if let Some(value) = parsed_obj.get(key) {
            config.set_value(key, crate::config::clone_value_nonrecursive(value));
        }
    }
}

fn process_directives<'a>(
    input: &'a str,
    registry: &DetectorRegistry,
    diagram_type: Option<&str>,
) -> Result<(Cow<'a, str>, MermaidConfig)> {
    let directives = detect_directives(input)?;
    if directives.is_empty() {
        return Ok((Cow::Borrowed(input), MermaidConfig::empty_object()));
    }
    let init = detect_init(&directives, input, registry, diagram_type)?;
    let wrap = directives.iter().any(|d| d.ty == "wrap");

    let mut merged = init;
    if wrap {
        merged.set_value("wrap", Value::Bool(true));
    }

    Ok((Cow::Owned(remove_directives(input)), merged))
}

fn detect_init(
    directives: &[Directive],
    input: &str,
    registry: &DetectorRegistry,
    diagram_type: Option<&str>,
) -> Result<MermaidConfig> {
    let mut merged = MermaidConfig::empty_object();
    let mut config_for_detect = MermaidConfig::empty_object();

    for d in directives {
        if d.ty != "init" && d.ty != "initialize" {
            continue;
        }

        let mut args = match &d.args {
            Some(v) => crate::config::clone_value_nonrecursive(v),
            None => Value::Object(Default::default()),
        };

        sanitize_directive(&mut args);

        // Mermaid moves a top-level `config` directive field into the diagram-type-specific config.
        if let Some(diagram_specific) = args
            .get("config")
            .map(crate::config::clone_value_nonrecursive)
        {
            let detected = diagram_type.map(|t| t.to_string()).or_else(|| {
                registry
                    .detect_type(input, &mut config_for_detect)
                    .ok()
                    .map(ToString::to_string)
            });

            if let Some(mut ty) = detected {
                if ty == "flowchart-v2" {
                    ty = "flowchart".to_string();
                }
                if let Value::Object(obj) = &mut args {
                    if let Some(old) = obj.insert(ty, diagram_specific) {
                        crate::config::drop_value_nonrecursive(old);
                    }
                    if let Some(old) = obj.remove("config") {
                        crate::config::drop_value_nonrecursive(old);
                    }
                }
            }
        }
        crate::config::mirror_legacy_font_family_into_theme_variables_value(&mut args);

        merged.deep_merge(&args);
    }

    Ok(merged)
}

#[derive(Debug, Clone)]
struct Directive {
    ty: String,
    args: Option<Value>,
}

fn detect_directives(input: &str) -> Result<Vec<Directive>> {
    let mut out = Vec::new();
    let mut pos = 0;
    let trimmed = input.trim();
    if !trimmed.contains("%%{") {
        return Ok(out);
    }

    // Mermaid's directive parser effectively treats single quotes as double quotes for JSON-like
    // directive bodies. Keep this behavior, but only pay the allocation when directives exist.
    let text = trimmed.replace('\'', "\"");

    while let Some(rel) = text[pos..].find("%%{") {
        let start = pos + rel;
        let content_start = start + 3;
        let Some(rel_end) = text[content_start..].find("}%%") else {
            break;
        };
        let content_end = content_start + rel_end;
        let raw = text[content_start..content_end].trim();

        if let Some(d) = parse_directive(raw)? {
            out.push(d);
        }

        pos = content_end + 3;
    }

    Ok(out)
}

#[derive(Clone)]
enum DirectiveValuePathSegment {
    Key(String),
    Index(usize),
}

fn sanitize_directive(value: &mut Value) {
    let mut stack = vec![Vec::<DirectiveValuePathSegment>::new()];

    while let Some(path) = stack.pop() {
        let Some(current) = directive_value_at_path_mut(value, &path) else {
            continue;
        };

        match current {
            Value::Object(map) => {
                if let Some(old) = map.remove("secure") {
                    crate::config::drop_value_nonrecursive(old);
                }

                let blocked_keys = map
                    .keys()
                    .filter(|key| key.starts_with("__"))
                    .cloned()
                    .collect::<Vec<_>>();
                for key in blocked_keys {
                    if let Some(old) = map.remove(&key) {
                        crate::config::drop_value_nonrecursive(old);
                    }
                }

                let child_keys = map.keys().cloned().collect::<Vec<_>>();
                for key in child_keys.into_iter().rev() {
                    let mut child_path = path.clone();
                    child_path.push(DirectiveValuePathSegment::Key(key));
                    stack.push(child_path);
                }
            }
            Value::Array(arr) => {
                for idx in (0..arr.len()).rev() {
                    let mut child_path = path.clone();
                    child_path.push(DirectiveValuePathSegment::Index(idx));
                    stack.push(child_path);
                }
            }
            Value::String(s) => {
                let blocked = s.contains('<') || s.contains('>') || s.contains("url(data:");
                if blocked {
                    s.clear();
                }
            }
            _ => {}
        }
    }
}

fn directive_value_at_path_mut<'a>(
    mut value: &'a mut Value,
    path: &[DirectiveValuePathSegment],
) -> Option<&'a mut Value> {
    for segment in path {
        match segment {
            DirectiveValuePathSegment::Key(key) => {
                value = value.as_object_mut()?.get_mut(key)?;
            }
            DirectiveValuePathSegment::Index(idx) => {
                value = value.as_array_mut()?.get_mut(*idx)?;
            }
        }
    }
    Some(value)
}

fn remove_directives(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pos = 0;
    while let Some(rel) = text[pos..].find("%%{") {
        let start = pos + rel;
        out.push_str(&text[pos..start]);
        let after_start = start + 3;
        if let Some(rel_end) = text[after_start..].find("}%%") {
            let end = after_start + rel_end + 3;
            pos = end;
        } else {
            return out;
        }
    }
    out.push_str(&text[pos..]);
    out
}

fn parse_directive(raw: &str) -> Result<Option<Directive>> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }

    let mut chars = raw.chars().peekable();
    let mut ty = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphanumeric() || c == '_' {
            ty.push(c);
            chars.next();
            continue;
        }
        break;
    }
    if ty.is_empty() {
        return Ok(None);
    }

    while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
        chars.next();
    }

    let args = if matches!(chars.peek(), Some(':')) {
        chars.next();
        while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
            chars.next();
        }
        let rest: String = chars.collect();
        let rest = rest.trim();
        if rest.is_empty() {
            None
        } else if rest.starts_with('{') || rest.starts_with('[') {
            if config_nesting_exceeds_limit(rest) {
                return Err(Error::InvalidDirectiveJson {
                    message: format!("config nesting exceeds {MAX_CONFIG_NESTING_DEPTH} levels"),
                });
            }
            Some(
                json5::from_str::<Value>(rest).map_err(|e| Error::InvalidDirectiveJson {
                    message: e.to_string(),
                })?,
            )
        } else {
            Some(Value::String(rest.to_string()))
        }
    } else {
        None
    };

    Ok(Some(Directive { ty, args }))
}

fn config_nesting_exceeds_limit(text: &str) -> bool {
    max_flow_collection_depth(text) > MAX_CONFIG_NESTING_DEPTH
        || max_yaml_indent_depth(text) > MAX_CONFIG_NESTING_DEPTH
}

fn max_flow_collection_depth(text: &str) -> usize {
    let mut max_depth = 0usize;
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;

    for ch in text.chars() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == q {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '{' | '[' => {
                depth = depth.saturating_add(1);
                max_depth = max_depth.max(depth);
            }
            '}' | ']' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    max_depth
}

fn max_yaml_indent_depth(text: &str) -> usize {
    let mut indents = Vec::<usize>::new();
    let mut max_depth = 0usize;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let indent = line.len() - line.trim_start_matches(' ').len();
        while indents.last().is_some_and(|prev| indent <= *prev) {
            indents.pop();
        }
        indents.push(indent);
        let inline_sequence_depth = yaml_inline_sequence_indicator_count(trimmed);
        max_depth = max_depth.max(indents.len() + inline_sequence_depth.saturating_sub(1));
    }

    max_depth
}

fn yaml_inline_sequence_indicator_count(mut text: &str) -> usize {
    let mut count = 0usize;
    loop {
        let Some(after_dash) = text.strip_prefix('-') else {
            return count;
        };
        if after_dash
            .chars()
            .next()
            .is_some_and(|ch| !ch.is_whitespace())
        {
            return count;
        }
        count += 1;
        text = after_dash.trim_start();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;

    #[test]
    fn sanitize_directive_handles_deep_values_with_small_stack() {
        const DEPTH: usize = 2_048;
        let mut value = deep_directive_value(DEPTH, Value::String("<blocked>".to_string()));

        let handle = std::thread::Builder::new()
            .name("preprocess-deep-directive-sanitize".to_string())
            .stack_size(64 * 1024)
            .spawn(move || {
                sanitize_directive(&mut value);
                assert_eq!(
                    deep_directive_leaf(&value, DEPTH).and_then(Value::as_str),
                    Some("")
                );
                crate::config::drop_value_nonrecursive(value);
            })
            .expect("spawn deep directive sanitizer test");
        handle
            .join()
            .expect("deep directive sanitizer should finish without stack overflow");
    }

    #[test]
    fn config_nesting_counts_inline_yaml_sequence_indicators() {
        let yaml = format!(
            "config:\n  {}\"leaf\"",
            "- ".repeat(MAX_CONFIG_NESTING_DEPTH + 1)
        );
        assert!(config_nesting_exceeds_limit(&yaml));
    }

    fn deep_directive_value(depth: usize, leaf: Value) -> Value {
        let mut value = leaf;
        for idx in (0..depth).rev() {
            let mut map = Map::new();
            map.insert(format!("k{idx}"), value);
            value = Value::Object(map);
        }
        value
    }

    fn deep_directive_leaf(mut value: &Value, depth: usize) -> Option<&Value> {
        for idx in 0..depth {
            value = value.as_object()?.get(&format!("k{idx}"))?;
        }
        Some(value)
    }
}

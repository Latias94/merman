use crate::{DetectorRegistry, Error, MermaidConfig, Result};
use regex::Regex;
use serde_json::Value;
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
cached_regex!(
    re_frontmatter,
    r"(?s)^-{3}\s*[\n\r](.*?)[\n\r]-{3}\s*[\n\r]+"
);

#[derive(Debug, Clone)]
pub struct PreprocessResult {
    pub code: String,
    pub title: Option<String>,
    pub config: MermaidConfig,
}

pub fn preprocess_diagram(input: &str, registry: &DetectorRegistry) -> Result<PreprocessResult> {
    preprocess_diagram_with_known_type(input, registry, None)
}

pub fn preprocess_diagram_with_known_type(
    input: &str,
    registry: &DetectorRegistry,
    diagram_type: Option<&str>,
) -> Result<PreprocessResult> {
    let cleaned = cleanup_text(input);
    let (without_frontmatter, title, mut frontmatter_config) = process_frontmatter(&cleaned)?;
    let (without_directives, directive_config) =
        process_directives(&without_frontmatter, registry, diagram_type)?;

    frontmatter_config.deep_merge(directive_config.as_value());

    let code = cleanup_comments(&without_directives);
    Ok(PreprocessResult {
        code,
        title,
        config: frontmatter_config,
    })
}

fn cleanup_text(input: &str) -> String {
    let mut s = re_crlf().replace_all(input, "\n").to_string();

    // Mermaid encodes `#quot;`-style sequences before parsing (`encodeEntities(...)`).
    // This is required because `#` and `;` are significant in several grammars (comments and
    // statement separators), and the encoded placeholders are later decoded by the renderer.
    //
    // Source of truth: `packages/mermaid/src/utils.ts::encodeEntities` at Mermaid@11.12.2.
    s = encode_mermaid_entities_like_upstream(&s);

    // Mermaid performs this HTML attribute rewrite as part of preprocessing.
    s = re_tag()
        .replace_all(&s, |caps: &regex::Captures| {
            let tag = &caps[1];
            let attrs = &caps[2];
            let attrs = re_attr_eq_double_quoted().replace_all(attrs, "='$1'");
            format!("<{tag}{attrs}>")
        })
        .to_string();

    s
}

fn encode_mermaid_entities_like_upstream(text: &str) -> String {
    // Mirrors Mermaid `encodeEntities` (Mermaid@11.12.2):
    //
    // 1) Protect `style...:#...;` and `classDef...:#...;` so color hex fragments are not mistaken
    //    as entities by the `/#\\w+;/g` pass.
    // 2) Encode `#<name>;` and `#<number>;` sequences into placeholders that do not contain `#`/`;`.
    let mut txt = text.to_string();

    txt = re_style_hex()
        .replace_all(&txt, |caps: &regex::Captures| {
            let s = caps.get(0).map(|m| m.as_str()).unwrap_or_default();
            s.strip_suffix(';').unwrap_or(s).to_string()
        })
        .to_string();

    txt = re_classdef_hex()
        .replace_all(&txt, |caps: &regex::Captures| {
            let s = caps.get(0).map(|m| m.as_str()).unwrap_or_default();
            s.strip_suffix(';').unwrap_or(s).to_string()
        })
        .to_string();

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

    txt
}

fn cleanup_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for line in input.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("%%") && !trimmed.starts_with("%%{") {
            continue;
        }
        out.push_str(line);
    }
    out.trim_start().to_string()
}

fn process_frontmatter(input: &str) -> Result<(String, Option<String>, MermaidConfig)> {
    let Some(caps) = re_frontmatter().captures(input) else {
        return Ok((input.to_string(), None, MermaidConfig::empty_object()));
    };

    let yaml_body = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
    let raw_yaml: serde_yaml::Value =
        serde_yaml::from_str(yaml_body).map_err(|e| Error::InvalidFrontMatterYaml {
            message: e.to_string(),
        })?;

    let parsed = serde_json::to_value(raw_yaml).unwrap_or(Value::Null);
    let parsed_obj = parsed.as_object().cloned().unwrap_or_default();

    let mut title = None;
    let mut config_value = Value::Object(Default::default());
    let mut display_mode = None;

    if let Some(Value::String(t)) = parsed_obj.get("title") {
        title = Some(t.clone());
    }
    if let Some(v) = parsed_obj.get("config") {
        config_value = v.clone();
    }
    if let Some(Value::String(dm)) = parsed_obj.get("displayMode") {
        display_mode = Some(dm.clone());
    }

    let mut config = MermaidConfig::empty_object();
    config.deep_merge(&config_value);
    if let Some(dm) = display_mode {
        config.set_value("gantt.displayMode", Value::String(dm));
    }

    let stripped = input[caps.get(0).unwrap().end()..].to_string();
    Ok((stripped, title, config))
}

fn process_directives(
    input: &str,
    registry: &DetectorRegistry,
    diagram_type: Option<&str>,
) -> Result<(String, MermaidConfig)> {
    let directives = detect_directives(input)?;
    let init = detect_init(&directives, input, registry, diagram_type)?;
    let wrap = directives.iter().any(|d| d.ty == "wrap");

    let mut merged = init;
    if wrap {
        merged.set_value("wrap", Value::Bool(true));
    }

    Ok((remove_directives(input), merged))
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
            Some(v) => v.clone(),
            None => Value::Object(Default::default()),
        };

        sanitize_directive(&mut args);

        // Mermaid moves a top-level `config` directive field into the diagram-type-specific config.
        if let Some(diagram_specific) = args.get("config").cloned() {
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
                    obj.insert(ty, diagram_specific);
                    obj.remove("config");
                }
            }
        }

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

fn sanitize_directive(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("secure");
            map.retain(|k, _| !k.starts_with("__"));
            for (_, v) in map.iter_mut() {
                sanitize_directive(v);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                sanitize_directive(v);
            }
        }
        Value::String(s) => {
            let blocked = s.contains('<') || s.contains('>') || s.contains("url(data:");
            if blocked {
                *s = String::new();
            }
        }
        _ => {}
    }
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
            Some(
                serde_json::from_str::<Value>(rest).map_err(|e| Error::InvalidDirectiveJson {
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

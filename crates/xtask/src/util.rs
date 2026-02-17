use crate::XtaskError;
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

pub(crate) fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension().is_some_and(|e| e == ext)
}

pub(crate) fn is_file_with_extension(path: &Path, ext: &str) -> bool {
    path.is_file() && has_extension(path, ext)
}

pub(crate) fn fmt_f64(v: f64) -> String {
    // Keep output stable and human-readable:
    // - round to 3 decimals
    // - trim trailing zeros
    // - keep at least 1 decimal place (e.g. `73.0`, not `73`)
    let rounded = (v * 1000.0).round() / 1000.0;
    let mut s = format!("{rounded:.3}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.push('0');
    }
    if s == "-0.0" { "0.0".to_string() } else { s }
}

pub(crate) fn read_text(path: &Path) -> Result<String, XtaskError> {
    fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })
}

pub(crate) fn read_text_normalized(path: &Path) -> Result<String, XtaskError> {
    let text = read_text(path)?;
    let normalized_line_endings = text.replace("\r\n", "\n");
    Ok(normalized_line_endings.trim_end().to_string())
}

pub(crate) fn extract_add_to_set_string_array(
    src: &str,
    ident: &str,
) -> Result<Vec<String>, XtaskError> {
    let needle = format!("const {ident} = addToSet({{}}, [");
    let start = src
        .find(&needle)
        .ok_or_else(|| XtaskError::ParseDompurify(format!("missing {ident} definition")))?;
    let bracket_start = start + needle.len() - 1; // points at '['
    extract_string_array_at(src, bracket_start)
}

pub(crate) fn extract_frozen_string_array(
    src: &str,
    ident: &str,
) -> Result<Vec<String>, XtaskError> {
    let needle = format!("const {ident} = freeze([");
    let start = src
        .find(&needle)
        .ok_or_else(|| XtaskError::ParseDompurify(format!("missing {ident} definition")))?;
    let bracket_start = start + needle.len() - 1; // points at '['
    extract_string_array_at(src, bracket_start)
}

pub(crate) fn extract_string_array_at(
    src: &str,
    bracket_start: usize,
) -> Result<Vec<String>, XtaskError> {
    let bytes = src.as_bytes();
    if *bytes.get(bracket_start).unwrap_or(&0) != b'[' {
        return Err(XtaskError::ParseDompurify("expected array '['".to_string()));
    }

    let mut out: Vec<String> = Vec::new();
    let mut i = bracket_start + 1;
    let mut in_string = false;
    let mut cur = String::new();

    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            match b {
                b'\\' => {
                    // Minimal escape handling: keep the escaped character verbatim.
                    if i + 1 >= bytes.len() {
                        return Err(XtaskError::ParseDompurify(
                            "unterminated escape".to_string(),
                        ));
                    }
                    let next = bytes[i + 1] as char;
                    cur.push(next);
                    i += 2;
                    continue;
                }
                b'\'' => {
                    out.push(cur.clone());
                    cur.clear();
                    in_string = false;
                    i += 1;
                    continue;
                }
                _ => {
                    cur.push(b as char);
                    i += 1;
                    continue;
                }
            }
        }

        match b {
            b'\'' => {
                in_string = true;
                i += 1;
            }
            b']' => return Ok(out),
            _ => i += 1,
        }
    }

    Err(XtaskError::ParseDompurify("unterminated array".to_string()))
}

pub(crate) fn extract_defaults(schema: &YamlValue, root: &YamlValue) -> Option<JsonValue> {
    let schema = expand_schema(schema, root);

    if let Some(default) = schema
        .as_mapping()
        .and_then(|m| m.get(YamlValue::String("default".to_string())))
    {
        return yaml_to_json(default).ok();
    }

    if let Some(any_of) = schema
        .as_mapping()
        .and_then(|m| m.get(YamlValue::String("anyOf".to_string())))
        .and_then(|v| v.as_sequence())
    {
        for s in any_of {
            if let Some(d) = extract_defaults(s, root) {
                return Some(d);
            }
        }
    }

    if let Some(one_of) = schema
        .as_mapping()
        .and_then(|m| m.get(YamlValue::String("oneOf".to_string())))
        .and_then(|v| v.as_sequence())
    {
        for s in one_of {
            if let Some(d) = extract_defaults(s, root) {
                return Some(d);
            }
        }
    }

    let is_object_type = schema
        .as_mapping()
        .and_then(|m| m.get(YamlValue::String("type".to_string())))
        .and_then(|v| v.as_str())
        == Some("object");

    let props = schema
        .as_mapping()
        .and_then(|m| m.get(YamlValue::String("properties".to_string())))
        .and_then(|v| v.as_mapping());

    if is_object_type || props.is_some() {
        let mut out: BTreeMap<String, JsonValue> = BTreeMap::new();
        if let Some(props) = props {
            for (k, v) in props {
                let Some(k) = k.as_str() else { continue };
                if let Some(d) = extract_defaults(v, root) {
                    out.insert(k.to_string(), d);
                }
            }
        }
        if out.is_empty() {
            return None;
        }
        return Some(JsonValue::Object(out.into_iter().collect()));
    }

    None
}

pub(crate) fn expand_schema(schema: &YamlValue, root: &YamlValue) -> YamlValue {
    let mut schema = schema.clone();
    schema = resolve_ref(&schema, root).unwrap_or(schema);

    let all_of = schema
        .as_mapping()
        .and_then(|m| m.get(YamlValue::String("allOf".to_string())))
        .and_then(|v| v.as_sequence())
        .cloned();

    if let Some(all_of) = all_of {
        let mut merged = schema.clone();
        if let Some(m) = merged.as_mapping_mut() {
            m.remove(YamlValue::String("allOf".to_string()));
        }
        for s in all_of {
            let s = expand_schema(&s, root);
            merged = merge_yaml(merged, s);
        }
        merged
    } else {
        schema
    }
}

pub(crate) fn resolve_ref(schema: &YamlValue, root: &YamlValue) -> Result<YamlValue, XtaskError> {
    let Some(map) = schema.as_mapping() else {
        return Ok(schema.clone());
    };
    let Some(ref_str) = map
        .get(YamlValue::String("$ref".to_string()))
        .and_then(|v| v.as_str())
    else {
        return Ok(schema.clone());
    };
    let target = resolve_ref_target(ref_str, root)?;
    let mut base = expand_schema(target, root);

    // Overlay other keys on top of the resolved target.
    let mut overlay = YamlValue::Mapping(map.clone());
    if let Some(m) = overlay.as_mapping_mut() {
        m.remove(YamlValue::String("$ref".to_string()));
    }
    base = merge_yaml(base, overlay);
    Ok(base)
}

fn resolve_ref_target<'a>(r: &str, root: &'a YamlValue) -> Result<&'a YamlValue, XtaskError> {
    if !r.starts_with("#/") {
        return Err(XtaskError::InvalidRef(r.to_string()));
    }
    let mut cur = root;
    for seg in r.trim_start_matches("#/").split('/') {
        let Some(map) = cur.as_mapping() else {
            return Err(XtaskError::UnresolvedRef(r.to_string()));
        };
        let key = YamlValue::String(seg.to_string());
        cur = map
            .get(&key)
            .ok_or_else(|| XtaskError::UnresolvedRef(r.to_string()))?;
    }
    Ok(cur)
}

pub(crate) fn merge_yaml(mut base: YamlValue, overlay: YamlValue) -> YamlValue {
    match (&mut base, overlay) {
        (YamlValue::Mapping(dst), YamlValue::Mapping(src)) => {
            for (k, v) in src {
                match dst.get_mut(&k) {
                    Some(existing) => {
                        let merged = merge_yaml(existing.clone(), v);
                        *existing = merged;
                    }
                    None => {
                        dst.insert(k, v);
                    }
                }
            }
            base
        }
        (_, v) => v,
    }
}

pub(crate) fn yaml_to_json(v: &YamlValue) -> Result<JsonValue, serde_json::Error> {
    serde_json::to_value(v)
}

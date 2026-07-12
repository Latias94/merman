use crate::XtaskError;
use serde_json::Value as JsonValue;
use std::fs;
use std::path::Path;

pub(crate) fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension().is_some_and(|e| e == ext)
}

pub(crate) fn is_file_with_extension(path: &Path, ext: &str) -> bool {
    path.is_file() && has_extension(path, ext)
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

pub(crate) fn extract_defaults(schema: &JsonValue, root: &JsonValue) -> Option<JsonValue> {
    let schema = expand_schema(schema, root);

    if let Some(default) = schema.as_object().and_then(|m| m.get("default")).cloned() {
        if default.as_object().is_some_and(|value| {
            value.len() == 1
                && value
                    .get("$ref")
                    .and_then(JsonValue::as_str)
                    .is_some_and(|reference| reference.starts_with("#/"))
        }) {
            return extract_defaults(&default, root).or(Some(default));
        }
        return Some(default);
    }

    if let Some(any_of) = schema
        .as_object()
        .and_then(|m| m.get("anyOf"))
        .and_then(|v| v.as_array())
    {
        for s in any_of {
            if let Some(d) = extract_defaults(s, root) {
                return Some(d);
            }
        }
    }

    if let Some(one_of) = schema
        .as_object()
        .and_then(|m| m.get("oneOf"))
        .and_then(|v| v.as_array())
    {
        for s in one_of {
            if let Some(d) = extract_defaults(s, root) {
                return Some(d);
            }
        }
    }

    let is_object_type = schema
        .as_object()
        .and_then(|m| m.get("type"))
        .and_then(|v| v.as_str())
        == Some("object");
    let props = schema
        .as_object()
        .and_then(|m| m.get("properties"))
        .and_then(|v| v.as_object());

    if is_object_type || props.is_some() {
        let mut out = std::collections::BTreeMap::<String, JsonValue>::new();
        if let Some(props) = props {
            for (k, v) in props {
                if let Some(d) = extract_defaults(v, root) {
                    out.insert(k.clone(), d);
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

pub(crate) fn expand_schema(schema: &JsonValue, root: &JsonValue) -> JsonValue {
    let mut schema = schema.clone();
    schema = resolve_ref(&schema, root).unwrap_or(schema);

    let all_of = schema
        .as_object()
        .and_then(|m| m.get("allOf"))
        .and_then(|v| v.as_array())
        .cloned();

    if let Some(all_of) = all_of {
        let mut merged = JsonValue::Object(Default::default());
        for s in all_of {
            let s = expand_schema(&s, root);
            merged = merge_yaml(merged, s);
        }
        let mut overlay = schema.clone();
        if let Some(m) = overlay.as_object_mut() {
            m.remove("allOf");
        }
        merged = merge_yaml(merged, overlay);
        merged
    } else {
        schema
    }
}

pub(crate) fn resolve_ref(schema: &JsonValue, root: &JsonValue) -> Result<JsonValue, XtaskError> {
    let Some(map) = schema.as_object() else {
        return Ok(schema.clone());
    };
    let Some(ref_str) = map.get("$ref").and_then(|v| v.as_str()) else {
        return Ok(schema.clone());
    };
    let target = resolve_ref_target(ref_str, root)?;
    let mut base = expand_schema(target, root);

    // Overlay other keys on top of the resolved target.
    let mut overlay = JsonValue::Object(map.clone());
    if let Some(m) = overlay.as_object_mut() {
        m.remove("$ref");
    }
    base = merge_yaml(base, overlay);
    Ok(base)
}

fn resolve_ref_target<'a>(r: &str, root: &'a JsonValue) -> Result<&'a JsonValue, XtaskError> {
    if !r.starts_with("#/") {
        return Err(XtaskError::InvalidRef(r.to_string()));
    }
    let mut cur = root;
    for seg in r.trim_start_matches("#/").split('/') {
        let Some(map) = cur.as_object() else {
            return Err(XtaskError::UnresolvedRef(r.to_string()));
        };
        cur = map
            .get(seg)
            .ok_or_else(|| XtaskError::UnresolvedRef(r.to_string()))?;
    }
    Ok(cur)
}

pub(crate) fn merge_yaml(mut base: JsonValue, overlay: JsonValue) -> JsonValue {
    match (&mut base, overlay) {
        (JsonValue::Object(dst), JsonValue::Object(src)) => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn extract_defaults_all_of_keeps_local_property_defaults_over_refs() {
        let schema = serde_saphyr::from_str::<Value>(
            r#"
$defs:
  BaseDiagramConfig:
    type: object
    properties:
      useMaxWidth:
        type: boolean
        default: true
properties:
  ishikawa:
    allOf:
      - $ref: '#/$defs/BaseDiagramConfig'
    type: object
    properties:
      useMaxWidth:
        type: boolean
        default: false
      diagramPadding:
        type: integer
        default: 20
"#,
        )
        .expect("schema should parse");

        let defaults = extract_defaults(&schema, &schema).expect("schema should produce defaults");

        assert_eq!(
            defaults,
            json!({
                "ishikawa": {
                    "diagramPadding": 20,
                    "useMaxWidth": false
                }
            })
        );
    }

    #[test]
    fn extract_defaults_expands_reference_stored_as_object_default() {
        let schema = json!({
            "$defs": {
                "AxisConfig": {
                    "type": "object",
                    "properties": {
                        "labelRotation": {
                            "type": "number",
                            "default": 0
                        },
                        "showLabel": {
                            "type": "boolean",
                            "default": true
                        }
                    }
                }
            },
            "type": "object",
            "properties": {
                "xyChart": {
                    "type": "object",
                    "properties": {
                        "xAxis": {
                            "type": "object",
                            "default": {
                                "$ref": "#/$defs/AxisConfig"
                            }
                        }
                    }
                }
            }
        });

        let defaults = extract_defaults(&schema, &schema).expect("schema should produce defaults");

        assert_eq!(
            defaults,
            json!({
                "xyChart": {
                    "xAxis": {
                        "labelRotation": 0,
                        "showLabel": true
                    }
                }
            })
        );
    }
}

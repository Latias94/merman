use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
enum XtaskError {
    #[error("usage: xtask <command> ...")]
    Usage,
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("failed to read file {path}: {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write file {path}: {source}")]
    WriteFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse YAML schema: {0}")]
    ParseYaml(#[from] serde_yaml::Error),
    #[error("failed to process JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid $ref: {0}")]
    InvalidRef(String),
    #[error("unresolved $ref: {0}")]
    UnresolvedRef(String),
    #[error("failed to parse dompurify dist file: {0}")]
    ParseDompurify(String),
    #[error("verification failed:\n{0}")]
    VerifyFailed(String),
}

fn main() -> Result<(), XtaskError> {
    let mut args = std::env::args().skip(1);
    let Some(cmd) = args.next() else {
        return Err(XtaskError::Usage);
    };

    match cmd.as_str() {
        "gen-default-config" => gen_default_config(args.collect()),
        "gen-dompurify-defaults" => gen_dompurify_defaults(args.collect()),
        "verify-generated" => verify_generated(args.collect()),
        other => Err(XtaskError::UnknownCommand(other.to_string())),
    }
}

fn gen_default_config(args: Vec<String>) -> Result<(), XtaskError> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Err(XtaskError::Usage);
    }

    let mut schema_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--schema" => {
                i += 1;
                schema_path = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let schema_path = schema_path.unwrap_or_else(|| {
        PathBuf::from("repo-ref/mermaid/packages/mermaid/src/schemas/config.schema.yaml")
    });
    let out_path = out_path
        .unwrap_or_else(|| PathBuf::from("crates/merman-core/src/generated/default_config.json"));

    let schema_text = fs::read_to_string(&schema_path).map_err(|source| XtaskError::ReadFile {
        path: schema_path.display().to_string(),
        source,
    })?;
    let schema_yaml: YamlValue = serde_yaml::from_str(&schema_text)?;

    let Some(root_defaults) = extract_defaults(&schema_yaml, &schema_yaml) else {
        return Err(XtaskError::InvalidRef(
            "schema produced no defaults (unexpected)".to_string(),
        ));
    };

    let pretty = serde_json::to_string_pretty(&root_defaults)?;
    let out_dir = out_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    fs::write(&out_path, pretty).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}

fn verify_generated(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() && !(args.len() == 1 && (args[0] == "--help" || args[0] == "-h")) {
        return Err(XtaskError::Usage);
    }
    if args.len() == 1 {
        return Err(XtaskError::Usage);
    }

    let tmp_dir = PathBuf::from("target/xtask");
    fs::create_dir_all(&tmp_dir).map_err(|source| XtaskError::WriteFile {
        path: tmp_dir.display().to_string(),
        source,
    })?;

    let mut failures = Vec::new();

    // Verify default config JSON.
    let expected_config = PathBuf::from("crates/merman-core/src/generated/default_config.json");
    let actual_config = tmp_dir.join("default_config.actual.json");
    gen_default_config(vec![
        "--schema".to_string(),
        "repo-ref/mermaid/packages/mermaid/src/schemas/config.schema.yaml".to_string(),
        "--out".to_string(),
        actual_config.display().to_string(),
    ])?;
    let expected_config_json: JsonValue = serde_json::from_str(&read_text(&expected_config)?)?;
    let actual_config_json: JsonValue = serde_json::from_str(&read_text(&actual_config)?)?;
    if expected_config_json != actual_config_json {
        failures.push(format!(
            "default config mismatch: regenerate with `cargo run -p xtask -- gen-default-config` ({})",
            expected_config.display()
        ));
    }

    // Verify DOMPurify allowlists.
    let expected_purify = PathBuf::from("crates/merman-core/src/generated/dompurify_defaults.rs");
    let actual_purify = tmp_dir.join("dompurify_defaults.actual.rs");
    gen_dompurify_defaults(vec![
        "--src".to_string(),
        "repo-ref/dompurify/dist/purify.cjs.js".to_string(),
        "--out".to_string(),
        actual_purify.display().to_string(),
    ])?;
    if read_text_normalized(&expected_purify)? != read_text_normalized(&actual_purify)? {
        failures.push(format!(
            "dompurify defaults mismatch: regenerate with `cargo run -p xtask -- gen-dompurify-defaults` ({})",
            expected_purify.display()
        ));
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::VerifyFailed(failures.join("\n")))
}

fn read_text(path: &Path) -> Result<String, XtaskError> {
    fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })
}

fn read_text_normalized(path: &Path) -> Result<String, XtaskError> {
    let text = read_text(path)?;
    let normalized_line_endings = text.replace("\r\n", "\n");
    Ok(normalized_line_endings.trim_end().to_string())
}

fn gen_dompurify_defaults(args: Vec<String>) -> Result<(), XtaskError> {
    let mut src_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--src" => {
                i += 1;
                src_path = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let src_path =
        src_path.unwrap_or_else(|| PathBuf::from("repo-ref/dompurify/dist/purify.cjs.js"));
    let out_path = out_path
        .unwrap_or_else(|| PathBuf::from("crates/merman-core/src/generated/dompurify_defaults.rs"));

    let src_text = fs::read_to_string(&src_path).map_err(|source| XtaskError::ReadFile {
        path: src_path.display().to_string(),
        source,
    })?;

    let html_tags = extract_frozen_string_array(&src_text, "html$1")?;
    let svg_tags = extract_frozen_string_array(&src_text, "svg$1")?;
    let svg_filters = extract_frozen_string_array(&src_text, "svgFilters")?;
    let mathml_tags = extract_frozen_string_array(&src_text, "mathMl$1")?;

    let html_attrs = extract_frozen_string_array(&src_text, "html")?;
    let svg_attrs = extract_frozen_string_array(&src_text, "svg")?;
    let mathml_attrs = extract_frozen_string_array(&src_text, "mathMl")?;
    let xml_attrs = extract_frozen_string_array(&src_text, "xml")?;

    let default_data_uri_tags =
        extract_add_to_set_string_array(&src_text, "DEFAULT_DATA_URI_TAGS")?;
    let default_uri_safe_attrs =
        extract_add_to_set_string_array(&src_text, "DEFAULT_URI_SAFE_ATTRIBUTES")?;

    let allowed_tags = unique_sorted_lowercase(
        html_tags
            .into_iter()
            .chain(svg_tags)
            .chain(svg_filters)
            .chain(mathml_tags),
    );

    let allowed_attrs = unique_sorted_lowercase(
        html_attrs
            .into_iter()
            .chain(svg_attrs)
            .chain(mathml_attrs)
            .chain(xml_attrs),
    );

    let data_uri_tags = unique_sorted_lowercase(default_data_uri_tags);
    let uri_safe_attrs = unique_sorted_lowercase(default_uri_safe_attrs);

    let out_dir = out_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let rust = render_dompurify_defaults_rs(
        &allowed_tags,
        &allowed_attrs,
        &uri_safe_attrs,
        &data_uri_tags,
    );
    fs::write(&out_path, rust).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}

fn render_dompurify_defaults_rs(
    allowed_tags: &[String],
    allowed_attrs: &[String],
    uri_safe_attrs: &[String],
    data_uri_tags: &[String],
) -> String {
    fn render_slice(name: &str, values: &[String]) -> String {
        let mut out = String::new();
        out.push_str(&format!("pub const {name}: &[&str] = &[\n"));
        for v in values {
            out.push_str(&format!("    {v:?},\n"));
        }
        out.push_str("];\n\n");
        out
    }

    let mut out = String::new();
    out.push_str("// This file is @generated by `cargo run -p xtask -- gen-dompurify-defaults`.\n");
    out.push_str("// Source: `repo-ref/dompurify/dist/purify.cjs.js` (DOMPurify 3.2.5)\n\n");
    out.push_str(&render_slice("DEFAULT_ALLOWED_TAGS", allowed_tags));
    out.push_str(&render_slice("DEFAULT_ALLOWED_ATTR", allowed_attrs));
    out.push_str(&render_slice("DEFAULT_URI_SAFE_ATTRIBUTES", uri_safe_attrs));
    out.push_str(&render_slice("DEFAULT_DATA_URI_TAGS", data_uri_tags));
    out
}

fn unique_sorted_lowercase<I>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut set = std::collections::BTreeSet::new();
    for v in values {
        set.insert(v.to_ascii_lowercase());
    }
    set.into_iter().collect()
}

fn extract_add_to_set_string_array(src: &str, ident: &str) -> Result<Vec<String>, XtaskError> {
    let needle = format!("const {ident} = addToSet({{}}, [");
    let start = src
        .find(&needle)
        .ok_or_else(|| XtaskError::ParseDompurify(format!("missing {ident} definition")))?;
    let bracket_start = start + needle.len() - 1; // points at '['
    extract_string_array_at(src, bracket_start)
}

fn extract_frozen_string_array(src: &str, ident: &str) -> Result<Vec<String>, XtaskError> {
    let needle = format!("const {ident} = freeze([");
    let start = src
        .find(&needle)
        .ok_or_else(|| XtaskError::ParseDompurify(format!("missing {ident} definition")))?;
    let bracket_start = start + needle.len() - 1; // points at '['
    extract_string_array_at(src, bracket_start)
}

fn extract_string_array_at(src: &str, bracket_start: usize) -> Result<Vec<String>, XtaskError> {
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

fn extract_defaults(schema: &YamlValue, root: &YamlValue) -> Option<JsonValue> {
    let schema = expand_schema(schema, root);

    if let Some(default) = schema
        .as_mapping()
        .and_then(|m| m.get(&YamlValue::String("default".to_string())))
    {
        return yaml_to_json(default).ok();
    }

    if let Some(any_of) = schema
        .as_mapping()
        .and_then(|m| m.get(&YamlValue::String("anyOf".to_string())))
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
        .and_then(|m| m.get(&YamlValue::String("oneOf".to_string())))
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
        .and_then(|m| m.get(&YamlValue::String("type".to_string())))
        .and_then(|v| v.as_str())
        == Some("object");

    let props = schema
        .as_mapping()
        .and_then(|m| m.get(&YamlValue::String("properties".to_string())))
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

fn expand_schema(schema: &YamlValue, root: &YamlValue) -> YamlValue {
    let mut schema = schema.clone();
    schema = resolve_ref(&schema, root).unwrap_or(schema);

    let all_of = schema
        .as_mapping()
        .and_then(|m| m.get(&YamlValue::String("allOf".to_string())))
        .and_then(|v| v.as_sequence())
        .cloned();

    if let Some(all_of) = all_of {
        let mut merged = schema.clone();
        if let Some(m) = merged.as_mapping_mut() {
            m.remove(&YamlValue::String("allOf".to_string()));
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

fn resolve_ref(schema: &YamlValue, root: &YamlValue) -> Result<YamlValue, XtaskError> {
    let Some(map) = schema.as_mapping() else {
        return Ok(schema.clone());
    };
    let Some(ref_str) = map
        .get(&YamlValue::String("$ref".to_string()))
        .and_then(|v| v.as_str())
    else {
        return Ok(schema.clone());
    };
    let target = resolve_ref_target(ref_str, root)?;
    let mut base = expand_schema(target, root);

    // Overlay other keys on top of the resolved target.
    let mut overlay = YamlValue::Mapping(map.clone());
    if let Some(m) = overlay.as_mapping_mut() {
        m.remove(&YamlValue::String("$ref".to_string()));
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

fn merge_yaml(mut base: YamlValue, overlay: YamlValue) -> YamlValue {
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

fn yaml_to_json(v: &YamlValue) -> Result<JsonValue, serde_json::Error> {
    serde_json::to_value(v)
}

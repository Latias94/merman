use crate::XtaskError;
use std::fs;
use std::path::{Path, PathBuf};

fn fixtures_root() -> PathBuf {
    crate::cmd::fixtures_root()
}

fn upstream_svg_path(diagram_dir: &str, stem: &str) -> PathBuf {
    fixtures_root()
        .join("upstream-svgs")
        .join(diagram_dir)
        .join(format!("{stem}.svg"))
}

fn deferred_fixture_dir(diagram_dir: &str) -> PathBuf {
    fixtures_root().join("_deferred").join(diagram_dir)
}

fn deferred_fixture_path(diagram_dir: &str, stem: &str) -> PathBuf {
    deferred_fixture_dir(diagram_dir).join(format!("{stem}.mmd"))
}

fn deferred_upstream_svg_dir(diagram_dir: &str) -> PathBuf {
    fixtures_root()
        .join("_deferred")
        .join("upstream-svgs")
        .join(diagram_dir)
}

fn deferred_upstream_svg_path(diagram_dir: &str, stem: &str) -> PathBuf {
    deferred_upstream_svg_dir(diagram_dir).join(format!("{stem}.svg"))
}

fn golden_json_path(diagram_dir: &str, stem: &str) -> PathBuf {
    fixtures_root()
        .join(diagram_dir)
        .join(format!("{stem}.golden.json"))
}

fn layout_golden_json_path(diagram_dir: &str, stem: &str) -> PathBuf {
    fixtures_root()
        .join(diagram_dir)
        .join(format!("{stem}.layout.golden.json"))
}

fn site_config_overrides_path() -> PathBuf {
    fixtures_root()
        .join("_config")
        .join("site_config_overrides.json")
}

fn fixture_relative_path(diagram_dir: &str, stem: &str) -> String {
    format!("{diagram_dir}/{stem}.mmd")
}

fn normalize_security_level(value: &str) -> Option<&'static str> {
    match value.trim() {
        "loose" => Some("loose"),
        "sandbox" => Some("sandbox"),
        _ => None,
    }
}

fn security_level_from_json(value: &serde_json::Value) -> Option<&'static str> {
    if let Some(level) = value
        .get("securityLevel")
        .and_then(serde_json::Value::as_str)
        .and_then(normalize_security_level)
    {
        return Some(level);
    }

    value
        .get("config")
        .and_then(|config| config.get("securityLevel"))
        .and_then(serde_json::Value::as_str)
        .and_then(normalize_security_level)
}

fn security_level_from_yaml(value: &serde_yaml::Value) -> Option<&'static str> {
    let mapping = value.as_mapping()?;
    let direct = serde_yaml::Value::String("securityLevel".to_string());
    if let Some(level) = mapping
        .get(&direct)
        .and_then(serde_yaml::Value::as_str)
        .and_then(normalize_security_level)
    {
        return Some(level);
    }

    let config_key = serde_yaml::Value::String("config".to_string());
    mapping
        .get(&config_key)
        .and_then(serde_yaml::Value::as_mapping)
        .and_then(|config| config.get(&direct))
        .and_then(serde_yaml::Value::as_str)
        .and_then(normalize_security_level)
}

fn yaml_mapping_str<'a>(mapping: &'a serde_yaml::Mapping, key: &str) -> Option<&'a str> {
    mapping
        .get(serde_yaml::Value::String(key.to_string()))
        .and_then(serde_yaml::Value::as_str)
}

fn config_look_from_yaml(value: &serde_yaml::Value) -> Option<&str> {
    let mapping = value.as_mapping()?;
    if let Some(look) = yaml_mapping_str(mapping, "look") {
        return Some(look);
    }

    mapping
        .get(serde_yaml::Value::String("config".to_string()))
        .and_then(serde_yaml::Value::as_mapping)
        .and_then(|config| yaml_mapping_str(config, "look"))
}

fn split_yaml_frontmatter(input: &str) -> Option<(&str, &str)> {
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

fn security_level_from_frontmatter(body: &str) -> Option<&'static str> {
    let (yaml, _) = split_yaml_frontmatter(body)?;
    let parsed = serde_yaml::from_str::<serde_yaml::Value>(yaml).ok()?;
    security_level_from_yaml(&parsed)
}

pub(crate) fn imported_fixture_config_look(body: &str) -> Option<String> {
    let (yaml, _) = split_yaml_frontmatter(body)?;
    let parsed = serde_yaml::from_str::<serde_yaml::Value>(yaml).ok()?;
    config_look_from_yaml(&parsed).map(str::to_string)
}

fn security_level_from_directives(body: &str) -> Option<&'static str> {
    let mut start = 0usize;
    while let Some(rel_start) = body[start..].find("%%{") {
        let content_start = start + rel_start + 3;
        let Some(rel_end) = body[content_start..].find("}%%") else {
            break;
        };
        let content_end = content_start + rel_end;
        let raw = body[content_start..content_end].trim();
        start = content_end + 3;

        let Some((directive, args)) = raw.split_once(':') else {
            continue;
        };
        let directive = directive.trim();
        if directive != "init" && directive != "initialize" {
            continue;
        }

        let args = args.trim().replace('\'', "\"");
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&args) else {
            continue;
        };
        if let Some(level) = security_level_from_json(&value) {
            return Some(level);
        }
    }
    None
}

fn imported_fixture_site_config(body: &str) -> Option<serde_json::Value> {
    let security_level =
        security_level_from_frontmatter(body).or_else(|| security_level_from_directives(body))?;

    Some(serde_json::json!({
        "securityLevel": security_level
    }))
}

fn read_site_config_overrides() -> Result<serde_json::Map<String, serde_json::Value>, XtaskError> {
    let path = site_config_overrides_path();
    if !path.exists() {
        return Ok(serde_json::Map::new());
    }

    let text = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|err| XtaskError::SnapshotUpdateFailed(err.to_string()))?;
    match value {
        serde_json::Value::Object(map) => Ok(map),
        other => Err(XtaskError::SnapshotUpdateFailed(format!(
            "fixture site config override manifest must be a JSON object, got {other:?}"
        ))),
    }
}

fn write_site_config_overrides(
    overrides: serde_json::Map<String, serde_json::Value>,
) -> Result<(), XtaskError> {
    let path = site_config_overrides_path();
    let pretty = render_site_config_overrides(&overrides)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&path, pretty).map_err(|source| XtaskError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

fn render_site_config_overrides(
    overrides: &serde_json::Map<String, serde_json::Value>,
) -> Result<String, XtaskError> {
    let mut out = String::from("{\n");
    for (idx, (key, value)) in overrides.iter().enumerate() {
        if idx > 0 {
            out.push_str(",\n");
        }
        let key = serde_json::to_string(key)
            .map_err(|err| XtaskError::SnapshotUpdateFailed(err.to_string()))?;
        let value = render_json_inline(value)?;
        out.push_str("  ");
        out.push_str(&key);
        out.push_str(": ");
        out.push_str(&value);
    }
    out.push_str("\n}\n");
    Ok(out)
}

fn render_json_inline(value: &serde_json::Value) -> Result<String, XtaskError> {
    match value {
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            serde_json::to_string(value)
                .map_err(|err| XtaskError::SnapshotUpdateFailed(err.to_string()))
        }
        serde_json::Value::String(value) => serde_json::to_string(value)
            .map_err(|err| XtaskError::SnapshotUpdateFailed(err.to_string())),
        serde_json::Value::Array(items) => {
            let rendered = items
                .iter()
                .map(render_json_inline)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("[{}]", rendered.join(", ")))
        }
        serde_json::Value::Object(map) => {
            if map.is_empty() {
                return Ok("{}".to_string());
            }
            let mut rendered = Vec::with_capacity(map.len());
            for (key, value) in map {
                let key = serde_json::to_string(key)
                    .map_err(|err| XtaskError::SnapshotUpdateFailed(err.to_string()))?;
                rendered.push(format!("{key}: {}", render_json_inline(value)?));
            }
            Ok(format!("{{ {} }}", rendered.join(", ")))
        }
    }
}

fn apply_site_config_override(
    overrides: &mut serde_json::Map<String, serde_json::Value>,
    relative_path: String,
    body: &str,
) -> bool {
    match imported_fixture_site_config(body) {
        Some(site_config) => {
            if overrides.get(&relative_path) == Some(&site_config) {
                false
            } else {
                overrides.insert(relative_path, site_config);
                true
            }
        }
        None => overrides.remove(&relative_path).is_some(),
    }
}

fn update_site_config_override(
    diagram_dir: &str,
    stem: &str,
    body: &str,
) -> Result<(), XtaskError> {
    let relative_path = fixture_relative_path(diagram_dir, stem);
    let mut overrides = read_site_config_overrides()?;
    if apply_site_config_override(&mut overrides, relative_path, body) {
        write_site_config_overrides(overrides)?;
    }
    Ok(())
}

fn remove_site_config_override(diagram_dir: &str, stem: &str) {
    let Ok(mut overrides) = read_site_config_overrides() else {
        return;
    };
    if overrides
        .remove(&fixture_relative_path(diagram_dir, stem))
        .is_some()
    {
        let _ = write_site_config_overrides(overrides);
    }
}

fn move_or_copy_then_remove(src: &Path, dst: &Path, replace_existing: bool) {
    if dst.exists() {
        if replace_existing {
            let _ = fs::remove_file(dst);
        } else {
            let _ = fs::remove_file(src);
            return;
        }
    }

    let _ = fs::rename(src, dst)
        .or_else(|_| fs::copy(src, dst).map(|_| ()))
        .and_then(|_| fs::remove_file(src));
}

pub(crate) fn cleanup_fixture_files(diagram_dir: &str, stem: &str, path: &Path) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(upstream_svg_path(diagram_dir, stem));
    let _ = fs::remove_file(golden_json_path(diagram_dir, stem));
    let _ = fs::remove_file(layout_golden_json_path(diagram_dir, stem));
    remove_site_config_override(diagram_dir, stem);
}

pub(crate) fn cleanup_deferred_fixture_files(diagram_dir: &str, stem: &str) {
    let _ = fs::remove_file(deferred_fixture_path(diagram_dir, stem));
    let _ = fs::remove_file(deferred_upstream_svg_path(diagram_dir, stem));
}

pub(crate) fn write_imported_fixture(
    diagram_dir: &str,
    stem: &str,
    path: &Path,
    body: &str,
) -> Result<(), XtaskError> {
    fs::write(path, body.as_bytes()).map_err(|source| XtaskError::WriteFile {
        path: path.display().to_string(),
        source,
    })?;
    update_site_config_override(diagram_dir, stem, body)
}

pub(crate) fn defer_fixture_files_with_replace_existing(
    diagram_dir: &str,
    stem: &str,
    path: &Path,
    keep_upstream_svg: bool,
    replace_existing: bool,
) -> PathBuf {
    let deferred_fixture_dir = deferred_fixture_dir(diagram_dir);
    let _ = fs::create_dir_all(&deferred_fixture_dir);

    let deferred_fixture_path = deferred_fixture_path(diagram_dir, stem);
    move_or_copy_then_remove(path, &deferred_fixture_path, replace_existing);

    if keep_upstream_svg {
        let upstream_svg_path = upstream_svg_path(diagram_dir, stem);
        if upstream_svg_path.exists() {
            let deferred_svg_dir = deferred_upstream_svg_dir(diagram_dir);
            let _ = fs::create_dir_all(&deferred_svg_dir);

            let deferred_svg_path = deferred_upstream_svg_path(diagram_dir, stem);
            move_or_copy_then_remove(&upstream_svg_path, &deferred_svg_path, replace_existing);
        }
    } else {
        let _ = fs::remove_file(upstream_svg_path(diagram_dir, stem));
    }

    let _ = fs::remove_file(golden_json_path(diagram_dir, stem));
    let _ = fs::remove_file(layout_golden_json_path(diagram_dir, stem));
    remove_site_config_override(diagram_dir, stem);

    deferred_fixture_path
}

#[cfg(test)]
mod tests {
    use super::{
        apply_site_config_override, imported_fixture_config_look, imported_fixture_site_config,
        render_site_config_overrides,
    };
    use serde_json::json;

    #[test]
    fn imported_fixture_site_config_detects_loose_json_directive() {
        let site_config = imported_fixture_site_config(
            r#"%%{init: {"securityLevel":"loose","theme":"dark"}}%%
flowchart TD
  A-->B
"#,
        )
        .expect("loose security level should become site config");

        assert_eq!(site_config["securityLevel"], "loose");
    }

    #[test]
    fn imported_fixture_site_config_detects_single_quote_directive() {
        let site_config = imported_fixture_site_config(
            r#"%%{init: {'securityLevel':'sandbox'}}%%
flowchart TD
  A-->B
"#,
        )
        .expect("single-quoted directive should become site config");

        assert_eq!(site_config["securityLevel"], "sandbox");
    }

    #[test]
    fn imported_fixture_site_config_detects_nested_config_directive() {
        let site_config = imported_fixture_site_config(
            r#"%%{init: {"config": {"securityLevel": "loose"}}}%%
flowchart TD
  A-->B
"#,
        )
        .expect("nested config directive should become site config");

        assert_eq!(site_config["securityLevel"], "loose");
    }

    #[test]
    fn imported_fixture_site_config_detects_sandbox_yaml_frontmatter() {
        let site_config = imported_fixture_site_config(
            r#"---
config:
  securityLevel: sandbox
---
flowchart TD
  A-->B
"#,
        )
        .expect("sandbox security level should become site config");

        assert_eq!(site_config["securityLevel"], "sandbox");
    }

    #[test]
    fn imported_fixture_site_config_detects_root_yaml_frontmatter() {
        let site_config = imported_fixture_site_config(
            r#"---
securityLevel: loose
---
flowchart TD
  A-->B
"#,
        )
        .expect("root frontmatter security level should become site config");

        assert_eq!(site_config["securityLevel"], "loose");
    }

    #[test]
    fn imported_fixture_config_look_detects_nested_yaml_frontmatter() {
        let look = imported_fixture_config_look(
            r#"---
config:
  look: handDrawn
---
flowchart TD
  A-->B
"#,
        );

        assert_eq!(look.as_deref(), Some("handDrawn"));
    }

    #[test]
    fn imported_fixture_config_look_detects_root_yaml_frontmatter() {
        let look = imported_fixture_config_look(
            r#"---
look: neo
---
flowchart TD
  A-->B
"#,
        );

        assert_eq!(look.as_deref(), Some("neo"));
    }

    #[test]
    fn render_site_config_overrides_keeps_compact_entry_lines() {
        let mut overrides = serde_json::Map::new();
        overrides.insert(
            "flowchart/a.mmd".to_string(),
            json!({ "securityLevel": "loose" }),
        );
        overrides.insert(
            "flowchart/b.mmd".to_string(),
            json!({ "securityLevel": "sandbox" }),
        );

        let rendered = render_site_config_overrides(&overrides).expect("render overrides");

        assert_eq!(
            rendered,
            "{\n  \"flowchart/a.mmd\": { \"securityLevel\": \"loose\" },\n  \"flowchart/b.mmd\": { \"securityLevel\": \"sandbox\" }\n}\n"
        );
    }

    #[test]
    fn imported_fixture_site_config_ignores_strict_default() {
        assert!(
            imported_fixture_site_config(
                r#"%%{init: {"securityLevel":"strict"}}%%
flowchart TD
  A-->B
"#
            )
            .is_none()
        );
    }

    #[test]
    fn imported_fixture_site_config_ignores_label_text() {
        assert!(
            imported_fixture_site_config(
                r#"flowchart TD
  A["securityLevel: loose"]
"#
            )
            .is_none()
        );
    }

    #[test]
    fn apply_site_config_override_adds_detected_security_level() {
        let mut overrides = serde_json::Map::new();

        let changed = apply_site_config_override(
            &mut overrides,
            "flowchart/example.mmd".to_string(),
            r#"%%{init: {"securityLevel":"loose"}}%%
flowchart TD
  A-->B
"#,
        );

        assert!(changed);
        assert_eq!(overrides["flowchart/example.mmd"]["securityLevel"], "loose");
    }

    #[test]
    fn apply_site_config_override_removes_stale_entry_for_default_fixture() {
        let mut overrides = serde_json::Map::new();
        overrides.insert(
            "flowchart/example.mmd".to_string(),
            serde_json::json!({ "securityLevel": "loose" }),
        );

        let changed = apply_site_config_override(
            &mut overrides,
            "flowchart/example.mmd".to_string(),
            "flowchart TD\n  A-->B\n",
        );

        assert!(changed);
        assert!(overrides.get("flowchart/example.mmd").is_none());
    }

    #[test]
    fn apply_site_config_override_skips_unchanged_manifest() {
        let mut overrides = serde_json::Map::new();
        overrides.insert(
            "flowchart/example.mmd".to_string(),
            serde_json::json!({ "securityLevel": "sandbox" }),
        );

        let changed = apply_site_config_override(
            &mut overrides,
            "flowchart/example.mmd".to_string(),
            r#"%%{init: {"securityLevel":"sandbox"}}%%
flowchart TD
  A-->B
"#,
        );

        assert!(!changed);
        assert_eq!(
            overrides["flowchart/example.mmd"]["securityLevel"],
            "sandbox"
        );
    }
}

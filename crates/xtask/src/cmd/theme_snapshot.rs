//! Pinned Mermaid theme snapshot and behavior-oracle generation.
//!
//! Theme classes are ordered JavaScript programs. A compact runtime artifact gives the pure-Rust
//! evaluator exact no-override and dark-mode snapshots, while a repository-level audit artifact
//! locks the value-shape behavior that is easy to lose when translating JavaScript truthiness and
//! merge semantics. Both artifacts come from one projection of the content-pinned runtime.

use super::mermaid_reference::MermaidProjectionRuntime;
use crate::XtaskError;
use serde::Deserialize;
use serde_json::{Map as JsonMap, Value as JsonValue, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) const THEME_RUNTIME_OUTPUT: &str =
    "crates/merman-core/src/generated/theme_variables_12_1_0.json";
pub(super) const THEME_AUDIT_OUTPUT: &str =
    "fixtures/_verification/theme_variables_oracle_12_1_0.json";
const THEME_RUNTIME_SCHEMA_VERSION: u32 = 2;
const THEME_AUDIT_SCHEMA_VERSION: u32 = 1;
const GENERATOR_COMMAND: &str = "cargo run -p xtask -- gen-theme-snapshot";
const THEME_NAMES: &[&str] = &[
    "default",
    "base",
    "dark",
    "forest",
    "neutral",
    "neo",
    "neo-dark",
    "redux",
    "redux-dark",
    "redux-color",
    "redux-dark-color",
];
const COMMON_ORACLE_CASE_IDS: &[&str] = &[
    "primary-null",
    "primary-empty",
    "primary-number",
    "primary-object",
    "dark-and-primary",
];
const BASE_ORACLE_CASE_IDS: &[&str] = &[
    "font-null",
    "font-empty",
    "font-number",
    "font-size-null",
    "scale-null",
    "dark-null",
    "dark-empty",
    "dark-number-zero",
    "dark-number-one",
    "dark-string",
    "radar-axis-null",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeThemeProjection {
    version: String,
    themes: JsonValue,
    dark_mode_true: JsonValue,
    oracle_cases: JsonValue,
}

struct GenerateOptions {
    reference_bundle: Option<PathBuf>,
    runtime_out_path: PathBuf,
    audit_out_path: PathBuf,
}

pub(crate) fn gen_theme_snapshot(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_generate_options(args)?;
    let runtime = MermaidProjectionRuntime::load(options.reference_bundle.as_deref())?;
    let projection = project_mermaid_runtime(&runtime)?;
    let (runtime_artifact, audit_artifact) = build_theme_artifacts(projection, &runtime)?;
    write_compact_json(&options.runtime_out_path, &runtime_artifact)?;
    write_pretty_json(&options.audit_out_path, &audit_artifact)
}

fn build_theme_artifacts(
    projection: RuntimeThemeProjection,
    runtime: &MermaidProjectionRuntime,
) -> Result<(JsonValue, JsonValue), XtaskError> {
    let oracle_case_count = projection
        .oracle_cases
        .as_array()
        .ok_or_else(|| {
            XtaskError::ThemeSnapshotProjection(
                "runtime projection field `oracleCases` must be an array".to_string(),
            )
        })?
        .len();
    let provenance = json!({
        "generator": GENERATOR_COMMAND,
        "mermaidVersion": runtime.version,
        "mermaidPackageSha256": runtime.package_sha256,
        "mermaidSourceTag": runtime.source_tag,
        "mermaidSourceCommit": runtime.source_commit,
    });
    let dark_mode_true_overrides = theme_overrides(&projection.themes, &projection.dark_mode_true)?;
    let mut runtime_artifact = json!({
        "schemaVersion": THEME_RUNTIME_SCHEMA_VERSION,
        "provenance": provenance.clone(),
        "themes": projection.themes,
        "darkModeTrueOverrides": dark_mode_true_overrides,
        "oracleCaseCount": oracle_case_count,
    });
    let mut audit_artifact = json!({
        "schemaVersion": THEME_AUDIT_SCHEMA_VERSION,
        "provenance": provenance,
        "oracleCases": projection.oracle_cases,
    });
    sort_json_value_keys(&mut runtime_artifact);
    sort_json_value_keys(&mut audit_artifact);
    Ok((runtime_artifact, audit_artifact))
}

/// Runtime schema 2: exact top-level set/remove deltas against each default theme.
/// Values (including objects and null) are replaced atomically; this is not JSON merge patch.
fn theme_overrides(base: &JsonValue, dark: &JsonValue) -> Result<JsonValue, XtaskError> {
    let mut overrides = JsonMap::new();
    for theme in THEME_NAMES {
        let base = base
            .get(theme)
            .and_then(JsonValue::as_object)
            .ok_or_else(|| {
                XtaskError::ThemeSnapshotProjection(format!(
                    "missing default theme object `{theme}`"
                ))
            })?;
        let dark = dark
            .get(theme)
            .and_then(JsonValue::as_object)
            .ok_or_else(|| {
                XtaskError::ThemeSnapshotProjection(format!("missing dark theme object `{theme}`"))
            })?;
        let assignments: JsonMap<_, _> = dark
            .iter()
            .filter(|(key, value)| base.get(*key) != Some(*value))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        let removals: Vec<_> = base
            .keys()
            .filter(|key| !dark.contains_key(*key))
            .cloned()
            .collect();
        overrides.insert(
            (*theme).to_string(),
            json!({"set": assignments, "remove": removals}),
        );
    }
    Ok(JsonValue::Object(overrides))
}

fn parse_generate_options(args: Vec<String>) -> Result<GenerateOptions, XtaskError> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        return Err(XtaskError::Usage);
    }

    let mut reference_bundle = None;
    let mut runtime_out_path = None;
    let mut audit_out_path = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--reference-bundle" => {
                index += 1;
                reference_bundle = Some(PathBuf::from(args.get(index).ok_or(XtaskError::Usage)?));
            }
            "--out" => {
                index += 1;
                runtime_out_path = Some(PathBuf::from(args.get(index).ok_or(XtaskError::Usage)?));
            }
            "--audit-out" => {
                index += 1;
                audit_out_path = Some(PathBuf::from(args.get(index).ok_or(XtaskError::Usage)?));
            }
            _ => return Err(XtaskError::Usage),
        }
        index += 1;
    }

    if reference_bundle.is_some() && (runtime_out_path.is_none() || audit_out_path.is_none()) {
        return Err(XtaskError::Usage);
    }
    Ok(GenerateOptions {
        reference_bundle,
        runtime_out_path: runtime_out_path.unwrap_or_else(|| PathBuf::from(THEME_RUNTIME_OUTPUT)),
        audit_out_path: audit_out_path.unwrap_or_else(|| PathBuf::from(THEME_AUDIT_OUTPUT)),
    })
}

fn project_mermaid_runtime(
    runtime: &MermaidProjectionRuntime,
) -> Result<RuntimeThemeProjection, XtaskError> {
    let output = Command::new("node")
        .arg("--input-type=module")
        .arg("-e")
        .arg(RUNTIME_THEME_PROJECTION_SCRIPT)
        .current_dir(&runtime.workspace)
        .output()
        .map_err(|error| {
            XtaskError::ThemeSnapshotProjection(format!(
                "failed to execute the pinned Mermaid theme projection: {error}"
            ))
        })?;
    if !output.status.success() {
        return Err(XtaskError::ThemeSnapshotProjection(format!(
            "pinned Mermaid theme projection failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let projection: RuntimeThemeProjection = serde_json::from_slice(&output.stdout)?;
    validate_projection(projection, &runtime.version)
}

fn validate_projection(
    projection: RuntimeThemeProjection,
    expected_version: &str,
) -> Result<RuntimeThemeProjection, XtaskError> {
    if projection.version != expected_version {
        return Err(XtaskError::ThemeSnapshotProjection(format!(
            "runtime projection requires Mermaid {}, found {}",
            expected_version, projection.version
        )));
    }

    for (label, value) in [
        ("themes", &projection.themes),
        ("darkModeTrue", &projection.dark_mode_true),
    ] {
        let Some(object) = value.as_object() else {
            return Err(XtaskError::ThemeSnapshotProjection(format!(
                "runtime projection field `{label}` must be an object"
            )));
        };
        let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
        let expected = THEME_NAMES.iter().copied().collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(XtaskError::ThemeSnapshotProjection(format!(
                "runtime projection field `{label}` has theme keys {actual:?}, expected {expected:?}"
            )));
        }
        if object.values().any(|theme| !theme.is_object()) {
            return Err(XtaskError::ThemeSnapshotProjection(format!(
                "runtime projection field `{label}` contains a non-object theme"
            )));
        }
    }

    let Some(oracle_cases) = projection.oracle_cases.as_array() else {
        return Err(XtaskError::ThemeSnapshotProjection(
            "runtime projection field `oracleCases` must be an array".to_string(),
        ));
    };
    let mut expected_cases = BTreeSet::new();
    for theme in THEME_NAMES {
        for id in COMMON_ORACLE_CASE_IDS {
            expected_cases.insert(((*theme).to_string(), id.to_string()));
        }
    }
    for id in BASE_ORACLE_CASE_IDS {
        expected_cases.insert(("base".to_string(), id.to_string()));
    }

    let mut actual_cases = BTreeSet::new();
    for case in oracle_cases {
        let theme = case.get("theme").and_then(JsonValue::as_str);
        let id = case.get("id").and_then(JsonValue::as_str);
        let status = case.get("status").and_then(JsonValue::as_str);
        let (Some(theme), Some(id), Some(status)) = (theme, id, status) else {
            return Err(XtaskError::ThemeSnapshotProjection(
                "every oracle case must have string `theme`, `id`, and `status` fields".to_string(),
            ));
        };
        let valid_result = match status {
            "ok" => case.get("selected").is_some_and(JsonValue::is_object),
            "error" => case.get("error").is_some_and(JsonValue::is_string),
            _ => false,
        };
        if !valid_result {
            return Err(XtaskError::ThemeSnapshotProjection(format!(
                "oracle case `{theme}/{id}` has an invalid `{status}` result"
            )));
        }
        if !actual_cases.insert((theme.to_string(), id.to_string())) {
            return Err(XtaskError::ThemeSnapshotProjection(format!(
                "oracle case `{theme}/{id}` is duplicated"
            )));
        }
    }
    if actual_cases != expected_cases {
        return Err(XtaskError::ThemeSnapshotProjection(format!(
            "runtime oracle cases are incomplete: found {actual_cases:?}, expected {expected_cases:?}"
        )));
    }
    Ok(projection)
}

fn sort_json_value_keys(value: &mut JsonValue) {
    match value {
        JsonValue::Object(object) => {
            for child in object.values_mut() {
                sort_json_value_keys(child);
            }
            let mut sorted = JsonMap::new();
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                if let Some(child) = object.remove(&key) {
                    sorted.insert(key, child);
                }
            }
            *object = sorted;
        }
        JsonValue::Array(values) => {
            for value in values {
                sort_json_value_keys(value);
            }
        }
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) | JsonValue::String(_) => {}
    }
}

fn write_pretty_json(path: &Path, value: &JsonValue) -> Result<(), XtaskError> {
    write_json(path, serde_json::to_string_pretty(value)?)
}

fn write_compact_json(path: &Path, value: &JsonValue) -> Result<(), XtaskError> {
    write_json(path, serde_json::to_string(value)?)
}

fn write_json(path: &Path, mut output: String) -> Result<(), XtaskError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
        path: parent.display().to_string(),
        source,
    })?;
    output.push('\n');
    fs::write(path, output).map_err(|source| XtaskError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

const RUNTIME_THEME_PROJECTION_SCRIPT: &str = r#"
import fs from 'node:fs';
import mermaid from 'mermaid';

const packageJson = JSON.parse(fs.readFileSync('./node_modules/mermaid/package.json', 'utf8'));
const themes = [
  'default', 'base', 'dark', 'forest', 'neutral', 'neo', 'neo-dark', 'redux',
  'redux-dark', 'redux-color', 'redux-dark-color',
];
const selectedPaths = [
  'primaryColor', 'fontFamily', 'fontSize', 'cScale0', 'cScalePeer0', 'cScaleInv0',
  'cScaleLabel0', 'scaleLabelColor', 'darkMode', 'edgeLabelBackground', 'rowOdd',
  'rowEven', 'surface0', 'surfacePeer0', 'git0', 'gitInv0', 'radar.axisColor',
];

const cloneJson = (value) => JSON.parse(JSON.stringify(value));
const resolve = (theme, overrides) => {
  mermaid.initialize({ theme, logLevel: 'fatal', themeVariables: overrides });
  return cloneJson(mermaid.mermaidAPI.getConfig().themeVariables);
};
const readPath = (value, path) => {
  let current = value;
  for (const key of path.split('.')) {
    if (current === null || typeof current !== 'object' || !Object.hasOwn(current, key)) {
      return { state: 'missing' };
    }
    current = current[key];
  }
  return { state: 'value', value: current };
};
const runCase = (id, theme, overrides) => {
  try {
    const variables = resolve(theme, overrides);
    return {
      id,
      theme,
      overrides,
      status: 'ok',
      selected: Object.fromEntries(selectedPaths.map((path) => [path, readPath(variables, path)])),
    };
  } catch (error) {
    return {
      id,
      theme,
      overrides,
      status: 'error',
      error: String(error?.message ?? error),
    };
  }
};

const defaultThemes = {};
const darkModeTrue = {};
const oracleCases = [];
for (const theme of themes) {
  defaultThemes[theme] = resolve(theme, undefined);
  darkModeTrue[theme] = resolve(theme, { darkMode: true });
  oracleCases.push(
    runCase('primary-null', theme, { primaryColor: null }),
    runCase('primary-empty', theme, { primaryColor: '' }),
    runCase('primary-number', theme, { primaryColor: 17 }),
    runCase('primary-object', theme, { primaryColor: { invalid: true } }),
    runCase('dark-and-primary', theme, { darkMode: true, primaryColor: '#123456' }),
  );
}

for (const [id, overrides] of [
  ['font-null', { fontFamily: null }],
  ['font-empty', { fontFamily: '' }],
  ['font-number', { fontFamily: 17 }],
  ['font-size-null', { fontSize: null }],
  ['scale-null', { cScale0: null }],
  ['dark-null', { darkMode: null }],
  ['dark-empty', { darkMode: '' }],
  ['dark-number-zero', { darkMode: 0 }],
  ['dark-number-one', { darkMode: 1 }],
  ['dark-string', { darkMode: 'false' }],
  ['radar-axis-null', { radar: { axisColor: null } }],
]) {
  oracleCases.push(runCase(id, 'base', overrides));
}

console.log(JSON.stringify({
  version: packageJson.version,
  themes: defaultThemes,
  darkModeTrue,
  oracleCases,
}));
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_overrides_encode_assignment_null_and_removed_keys_losslessly() {
        let base = json!({"same": false, "removed": 1, "nullable": 2, "object": {"old": 1}});
        let dark = json!({"same": false, "nullable": null, "object": {"new": 2}, "added": 0});
        let base_themes = JsonValue::Object(
            THEME_NAMES
                .iter()
                .map(|name| ((*name).to_string(), base.clone()))
                .collect(),
        );
        let dark_themes = JsonValue::Object(
            THEME_NAMES
                .iter()
                .map(|name| ((*name).to_string(), dark.clone()))
                .collect(),
        );
        let overrides = theme_overrides(&base_themes, &dark_themes).unwrap();
        for theme in THEME_NAMES {
            assert_eq!(
                overrides[theme],
                json!({
                    "set": {"nullable": null, "object": {"new": 2}, "added": 0},
                    "remove": ["removed"]
                })
            );
        }
    }

    #[test]
    fn projection_requires_every_supported_theme_and_oracle_evidence() {
        let themes = THEME_NAMES
            .iter()
            .map(|theme| ((*theme).to_string(), json!({ "primaryColor": "#fff" })))
            .collect::<JsonMap<_, _>>();
        let mut oracle_cases = Vec::new();
        for theme in THEME_NAMES {
            for id in COMMON_ORACLE_CASE_IDS {
                oracle_cases.push(json!({
                    "id": id,
                    "theme": theme,
                    "status": "ok",
                    "selected": {},
                }));
            }
        }
        for id in BASE_ORACLE_CASE_IDS {
            oracle_cases.push(json!({
                "id": id,
                "theme": "base",
                "status": "ok",
                "selected": {},
            }));
        }
        let projection = RuntimeThemeProjection {
            version: crate::cmd::PINNED_MERMAID_VERSION.to_string(),
            themes: JsonValue::Object(themes.clone()),
            dark_mode_true: JsonValue::Object(themes),
            oracle_cases: JsonValue::Array(oracle_cases),
        };

        let projection = validate_projection(projection, crate::cmd::PINNED_MERMAID_VERSION)
            .expect("complete projection is valid");
        let (runtime, audit) =
            build_theme_artifacts(projection, &MermaidProjectionRuntime::selected()).unwrap();

        assert!(runtime.get("oracleCases").is_none());
        assert!(runtime.get("themes").is_some_and(JsonValue::is_object));
        assert!(
            runtime
                .get("darkModeTrueOverrides")
                .is_some_and(JsonValue::is_object)
        );
        assert_eq!(
            runtime.get("oracleCaseCount").and_then(JsonValue::as_u64),
            Some(
                (THEME_NAMES.len() * COMMON_ORACLE_CASE_IDS.len() + BASE_ORACLE_CASE_IDS.len())
                    as u64
            )
        );
        assert_eq!(runtime["schemaVersion"], 2);
        assert_eq!(audit["schemaVersion"], 1);
        assert!(runtime.get("darkModeTrue").is_none());
        for theme in THEME_NAMES {
            assert_eq!(
                runtime["darkModeTrueOverrides"][theme],
                json!({"set": {}, "remove": []})
            );
        }
        assert!(audit.get("themes").is_none());
        assert!(audit.get("darkModeTrue").is_none());
        assert!(audit.get("oracleCases").is_some_and(JsonValue::is_array));
        assert_eq!(runtime.get("provenance"), audit.get("provenance"));
    }
}

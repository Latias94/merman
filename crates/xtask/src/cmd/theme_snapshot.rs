//! Pinned Mermaid theme snapshot and behavior-oracle generation.
//!
//! Theme classes are ordered JavaScript programs. The checked-in artifact gives the pure-Rust
//! evaluator exact no-override and dark-mode snapshots, while the compact oracle matrix locks the
//! value-shape behavior that is easy to lose when translating JavaScript truthiness and merge
//! semantics.

use crate::XtaskError;
use serde::Deserialize;
use serde_json::{Map as JsonMap, Value as JsonValue, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const THEME_SNAPSHOT_OUTPUT: &str = "crates/merman-core/src/generated/theme_variables_11_16_1.json";
const THEME_ARTIFACT_SCHEMA_VERSION: u32 = 1;
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
    out_path: PathBuf,
}

pub(crate) fn gen_theme_snapshot(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_generate_options(args)?;
    let projection = project_pinned_mermaid_runtime()?;
    let mut artifact = json!({
        "schemaVersion": THEME_ARTIFACT_SCHEMA_VERSION,
        "provenance": {
            "generator": GENERATOR_COMMAND,
            "mermaidVersion": crate::cmd::PINNED_MERMAID_VERSION,
            "mermaidPackageSha256": crate::cmd::PINNED_MERMAID_PACKAGE_SHA256,
            "mermaidSourceTag": crate::cmd::MERMAID_SOURCE_TAG,
            "mermaidSourceCommit": crate::cmd::MERMAID_SOURCE_COMMIT,
        },
        "themes": projection.themes,
        "darkModeTrue": projection.dark_mode_true,
        "oracleCases": projection.oracle_cases,
    });
    sort_json_value_keys(&mut artifact);
    write_pretty_json(&options.out_path, &artifact)
}

fn parse_generate_options(args: Vec<String>) -> Result<GenerateOptions, XtaskError> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        return Err(XtaskError::Usage);
    }

    let mut out_path = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--out" => {
                index += 1;
                out_path = args.get(index).map(PathBuf::from);
            }
            _ => return Err(XtaskError::Usage),
        }
        index += 1;
    }

    Ok(GenerateOptions {
        out_path: out_path.unwrap_or_else(|| PathBuf::from(THEME_SNAPSHOT_OUTPUT)),
    })
}

fn project_pinned_mermaid_runtime() -> Result<RuntimeThemeProjection, XtaskError> {
    if merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION != crate::cmd::PINNED_MERMAID_VERSION
    {
        return Err(XtaskError::ThemeSnapshotProjection(format!(
            "theme generation targets Mermaid {}, but the workspace pins {}",
            crate::cmd::PINNED_MERMAID_VERSION,
            merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION
        )));
    }

    let tools_root = crate::cmd::workspace_root().join("tools/mermaid-cli");
    validate_pinned_mermaid_runtime(&tools_root)?;

    let output = Command::new("node")
        .arg("--input-type=module")
        .arg("-e")
        .arg(RUNTIME_THEME_PROJECTION_SCRIPT)
        .current_dir(&tools_root)
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
    validate_projection(projection)
}

fn validate_pinned_mermaid_runtime(tools_root: &Path) -> Result<(), XtaskError> {
    let runtime_root = tools_root.join("node_modules/mermaid");
    if !runtime_root.join("package.json").is_file() {
        return Err(XtaskError::MissingReference(format!(
            "the pinned Mermaid theme runtime is missing at `{}`; run `npm ci --prefix tools/mermaid-cli`",
            runtime_root.display()
        )));
    }

    crate::cmd::validate_mermaid_cli_install(tools_root)?;
    let package_hash = crate::cmd::upstream_svg_package_tree_sha256(&runtime_root)?;
    if package_hash != crate::cmd::PINNED_MERMAID_PACKAGE_SHA256 {
        return Err(XtaskError::ThemeSnapshotProjection(format!(
            "installed mermaid@{} content differs from the pinned package: expected {}, found {package_hash}",
            crate::cmd::PINNED_MERMAID_VERSION,
            crate::cmd::PINNED_MERMAID_PACKAGE_SHA256
        )));
    }
    Ok(())
}

fn validate_projection(
    projection: RuntimeThemeProjection,
) -> Result<RuntimeThemeProjection, XtaskError> {
    if projection.version != crate::cmd::PINNED_MERMAID_VERSION {
        return Err(XtaskError::ThemeSnapshotProjection(format!(
            "runtime projection requires Mermaid {}, found {}",
            crate::cmd::PINNED_MERMAID_VERSION,
            projection.version
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
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
        path: parent.display().to_string(),
        source,
    })?;
    let mut output = serde_json::to_string_pretty(value)?;
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

        validate_projection(projection).expect("complete projection is valid");
    }
}

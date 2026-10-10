//! Pinned Mermaid runtime default-config generation.
//!
//! Mermaid builds its effective defaults in JavaScript from the JSON schema, `defaultConfig.ts`,
//! theme variables, and `assignWithDepth`. Replaying that construction in Rust creates a second
//! implementation that can drift. This generator instead projects the installed, content-pinned
//! Mermaid runtime into two deterministic artifacts:
//! - the JSON-valued runtime config; and
//! - the key shape contributed by JSON values, functions, and explicit `undefined` values.

use super::mermaid_reference::MermaidProjectionRuntime;
use super::{sort_json_value_keys, write_pretty_json};
use crate::XtaskError;
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use std::collections::BTreeSet;
#[cfg(test)]
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_CONFIG_OUTPUT: &str = "crates/merman-core/src/generated/default_config.json";
const DEFAULT_CONFIG_SHAPE_OUTPUT: &str =
    "crates/merman-core/src/generated/default_config_shape.json";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeProjectionPayload {
    version: String,
    values: JsonValue,
    config_keys: Vec<String>,
    undefined_paths: Vec<String>,
    function_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct DefaultConfigProjection {
    version: String,
    values: JsonValue,
    config_keys: BTreeSet<String>,
    undefined_paths: BTreeSet<String>,
    function_paths: BTreeSet<String>,
}

impl DefaultConfigProjection {
    fn from_runtime(
        payload: RuntimeProjectionPayload,
        expected_version: &str,
    ) -> Result<Self, XtaskError> {
        if payload.version != expected_version {
            return Err(XtaskError::DefaultConfigProjection(format!(
                "runtime projection requires Mermaid {}, found {}",
                expected_version, payload.version
            )));
        }
        if !payload.values.is_object() {
            return Err(XtaskError::DefaultConfigProjection(
                "Mermaid runtime projection returned a non-object value plane".to_string(),
            ));
        }
        if payload.values.get("themeVariables").is_some() {
            return Err(XtaskError::DefaultConfigProjection(
                "Mermaid runtime value plane must exclude separately generated themeVariables"
                    .to_string(),
            ));
        }

        let config_keys = unique_strings("configKeys", payload.config_keys)?;
        let undefined_paths = unique_strings("undefinedPaths", payload.undefined_paths)?;
        let function_paths = unique_strings("functionPaths", payload.function_paths)?;
        if !undefined_paths.is_disjoint(&function_paths) {
            return Err(XtaskError::DefaultConfigProjection(
                "a Mermaid runtime path cannot be both undefined and a function".to_string(),
            ));
        }
        for path in undefined_paths.iter().chain(function_paths.iter()) {
            let key = path.rsplit('.').next().unwrap_or_default();
            if key.is_empty() || !config_keys.contains(key) {
                return Err(XtaskError::DefaultConfigProjection(format!(
                    "runtime shape path `{path}` is missing its leaf key from configKeys"
                )));
            }
        }

        Ok(Self {
            version: payload.version,
            values: payload.values,
            config_keys,
            undefined_paths,
            function_paths,
        })
    }

    fn shape_json(&self) -> JsonValue {
        json!({
            "baselineVersion": self.version,
            "configKeys": self.config_keys,
            "undefinedPaths": self.undefined_paths,
            "functionPaths": self.function_paths,
        })
    }
}

struct GenerateOptions {
    reference_bundle: Option<PathBuf>,
    out_path: PathBuf,
    shape_out_path: PathBuf,
}

pub(crate) fn gen_default_config(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_generate_options(args)?;
    let runtime = MermaidProjectionRuntime::load(options.reference_bundle.as_deref())?;
    let mut projection = project_mermaid_runtime(&runtime)?;
    sort_json_value_keys(&mut projection.values);
    let mut shape = projection.shape_json();
    sort_json_value_keys(&mut shape);
    write_pretty_json(&options.out_path, &projection.values)?;
    write_pretty_json(&options.shape_out_path, &shape)
}

fn parse_generate_options(args: Vec<String>) -> Result<GenerateOptions, XtaskError> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Err(XtaskError::Usage);
    }

    let mut reference_bundle = None;
    let mut out_path = None;
    let mut shape_out_path = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--reference-bundle" => {
                index += 1;
                reference_bundle = Some(PathBuf::from(args.get(index).ok_or(XtaskError::Usage)?));
            }
            "--out" => {
                index += 1;
                out_path = args.get(index).map(PathBuf::from);
            }
            "--shape-out" => {
                index += 1;
                shape_out_path = args.get(index).map(PathBuf::from);
            }
            _ => return Err(XtaskError::Usage),
        }
        index += 1;
    }

    if reference_bundle.is_some() && (out_path.is_none() || shape_out_path.is_none()) {
        return Err(XtaskError::Usage);
    }
    let out_was_explicit = out_path.is_some();
    let out_path = out_path.unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_OUTPUT));
    let shape_out_path = shape_out_path.unwrap_or_else(|| {
        if out_was_explicit {
            out_path.with_file_name("default_config_shape.json")
        } else {
            PathBuf::from(DEFAULT_CONFIG_SHAPE_OUTPUT)
        }
    });
    Ok(GenerateOptions {
        reference_bundle,
        out_path,
        shape_out_path,
    })
}

fn project_mermaid_runtime(
    runtime: &MermaidProjectionRuntime,
) -> Result<DefaultConfigProjection, XtaskError> {
    let output = Command::new("node")
        .arg("--input-type=module")
        .arg("-e")
        .arg(RUNTIME_PROJECTION_SCRIPT)
        .current_dir(&runtime.workspace)
        .output()
        .map_err(|error| {
            XtaskError::DefaultConfigProjection(format!(
                "failed to execute the pinned Mermaid runtime projection: {error}"
            ))
        })?;
    if !output.status.success() {
        return Err(XtaskError::DefaultConfigProjection(format!(
            "pinned Mermaid runtime projection failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let payload: RuntimeProjectionPayload = serde_json::from_slice(&output.stdout)?;
    DefaultConfigProjection::from_runtime(payload, &runtime.version)
}

fn unique_strings(field: &str, values: Vec<String>) -> Result<BTreeSet<String>, XtaskError> {
    let expected_len = values.len();
    let values = values
        .into_iter()
        .map(|value| {
            if value.is_empty() {
                Err(XtaskError::DefaultConfigProjection(format!(
                    "runtime projection field `{field}` contains an empty string"
                )))
            } else {
                Ok(value)
            }
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if values.len() != expected_len {
        return Err(XtaskError::DefaultConfigProjection(format!(
            "runtime projection field `{field}` contains duplicate entries"
        )));
    }
    Ok(values)
}

const RUNTIME_PROJECTION_SCRIPT: &str = r#"
import fs from 'node:fs';
import mermaid from 'mermaid';

const packageJson = JSON.parse(fs.readFileSync('./node_modules/mermaid/package.json', 'utf8'));
const defaultConfig = mermaid.mermaidAPI.defaultConfig;
const currentConfig = mermaid.mermaidAPI.getConfig();

const projectValue = (value, path = '') => {
  if (path === 'themeVariables' || typeof value === 'undefined' || typeof value === 'function') {
    return undefined;
  }
  if (Array.isArray(value)) {
    return value.map((item, index) => projectValue(item, `${path}.${index}`));
  }
  if (value !== null && typeof value === 'object') {
    const projected = {};
    for (const key of Object.keys(value).sort()) {
      if (key === '$ref') continue;
      const childPath = path ? `${path}.${key}` : key;
      const child = projectValue(value[key], childPath);
      if (child !== undefined) projected[key] = child;
    }
    return projected;
  }
  return value;
};

const configKeys = new Set();
const collectKeys = (value) => {
  for (const key of Object.keys(value)) {
    if (Array.isArray(value[key])) continue;
    configKeys.add(key);
    if (value[key] !== null && typeof value[key] === 'object') collectKeys(value[key]);
  }
};
collectKeys(defaultConfig);

const undefinedPaths = [];
const functionPaths = [];
const collectKinds = (value, path = '') => {
  if (value === null || typeof value !== 'object') return;
  for (const key of Object.keys(value)) {
    const childPath = path ? `${path}.${key}` : key;
    if (typeof value[key] === 'undefined') undefinedPaths.push(childPath);
    else if (typeof value[key] === 'function') functionPaths.push(childPath);
    else collectKinds(value[key], childPath);
  }
};
collectKinds(defaultConfig);

console.log(JSON.stringify({
  version: packageJson.version,
  values: projectValue(currentConfig),
  configKeys: [...configKeys].sort(),
  undefinedPaths: undefinedPaths.sort(),
  functionPaths: functionPaths.sort(),
}));
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_payload() -> RuntimeProjectionPayload {
        RuntimeProjectionPayload {
            version: crate::cmd::PINNED_MERMAID_VERSION.to_string(),
            values: json!({ "flowchart": {}, "secure": ["secure"] }),
            config_keys: vec![
                "flowchart".to_string(),
                "messageFont".to_string(),
                "nodeColors".to_string(),
                "secure".to_string(),
            ],
            undefined_paths: vec!["sankey.nodeColors".to_string()],
            function_paths: vec!["sequence.messageFont".to_string()],
        }
    }

    #[test]
    fn runtime_projection_separates_json_values_from_non_json_shape() {
        let projection = DefaultConfigProjection::from_runtime(
            valid_payload(),
            crate::cmd::PINNED_MERMAID_VERSION,
        )
        .expect("projection succeeds");

        assert!(projection.values["flowchart"].is_object());
        assert!(projection.values.get("nodeColors").is_none());
        assert!(projection.config_keys.contains("nodeColors"));
        assert!(projection.config_keys.contains("messageFont"));
        assert!(projection.undefined_paths.contains("sankey.nodeColors"));
        assert!(projection.function_paths.contains("sequence.messageFont"));
    }

    #[test]
    fn runtime_projection_rejects_shape_paths_without_config_keys() {
        let mut payload = valid_payload();
        payload.config_keys.retain(|key| key != "nodeColors");

        let error =
            DefaultConfigProjection::from_runtime(payload, crate::cmd::PINNED_MERMAID_VERSION)
                .expect_err("missing shape leaf should fail");
        assert!(error.to_string().contains("nodeColors"));
    }

    #[test]
    fn generated_artifacts_keep_upstream_value_and_shape_planes_separate() {
        let root = crate::cmd::workspace_root();
        let values: JsonValue = serde_json::from_str(
            &fs::read_to_string(root.join(DEFAULT_CONFIG_OUTPUT))
                .expect("generated default config should read"),
        )
        .expect("generated default config should parse");
        let shape: JsonValue = serde_json::from_str(
            &fs::read_to_string(root.join(DEFAULT_CONFIG_SHAPE_OUTPUT))
                .expect("generated default config shape should read"),
        )
        .expect("generated default config shape should parse");

        assert_eq!(values["secure"].as_array().map(Vec::len), Some(6));
        assert!(values["flowchart"].get("htmlLabels").is_none());
        assert_eq!(values["c4"]["flowchart"]["htmlLabels"], JsonValue::Null);
        assert_eq!(values["pie"]["useWidth"], 984);
        assert_eq!(values["railroad"], json!({}));
        assert!(values["sankey"].get("nodeColors").is_none());

        assert_eq!(shape["baselineVersion"], crate::cmd::PINNED_MERMAID_VERSION);
        let config_keys = shape["configKeys"]
            .as_array()
            .expect("configKeys should be an array");
        assert!(config_keys.iter().any(|key| key == "nodeColors"));
        assert!(config_keys.iter().any(|key| key == "messageFont"));
    }

    #[test]
    fn output_sorting_is_recursive() {
        let mut value = json!({
            "z": 1,
            "a": { "textPosition": 0.75, "donutHole": 0 },
            "m": [{ "b": true, "a": false }]
        });
        sort_json_value_keys(&mut value);
        assert_eq!(
            value
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["a", "m", "z"]
        );
        assert_eq!(
            value["a"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["donutHole", "textPosition"]
        );
        assert_eq!(
            value["m"][0]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
    }
}

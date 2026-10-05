//! Pinned Mermaid theme snapshot and behavior-oracle generation.
//!
//! Theme classes are ordered JavaScript programs. The checked-in artifacts give the pure-Rust
//! evaluator exact constructor, no-override, and dark-mode snapshots, while compact final-value
//! and stage oracles lock the ordering, truthiness, and replay behavior that is easy to lose when
//! translating mutable JavaScript theme classes.

use super::mermaid_reference::MermaidProjectionRuntime;
use super::{sort_json_value_keys, write_pretty_json};
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
const THEME_RUNTIME_SCHEMA_VERSION: u32 = 3;
const THEME_AUDIT_SCHEMA_VERSION: u32 = 2;
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
    "object-field-number",
];
const ORACLE_SELECTED_PATHS: &[&str] = &[
    "primaryColor",
    "fontFamily",
    "fontSize",
    "cScale0",
    "cScalePeer0",
    "cScaleInv0",
    "cScaleLabel0",
    "scaleLabelColor",
    "darkMode",
    "edgeLabelBackground",
    "rowOdd",
    "rowEven",
    "surface0",
    "surfacePeer0",
    "git0",
    "gitInv0",
    "radar.axisColor",
    "radar",
];
const CLASSIC_STAGE_ORACLE_PATHS: &[&str] = &[
    "primaryColor",
    "secondaryColor",
    "tertiaryColor",
    "primaryBorderColor",
    "secondaryBorderColor",
    "tertiaryBorderColor",
    "primaryTextColor",
    "secondaryTextColor",
    "tertiaryTextColor",
    "mainBkg",
    "nodeBkg",
    "cScale0",
    "cScale2",
    "cScale3",
    "fillType0",
    "gradientStart",
    "gradientStop",
    "requirementBackground",
];
const OPERATOR_STAGE_ORACLE_PATHS: &[&str] =
    &["primaryBorderColor", "gradientStart", "pieOpacity", "venn1"];
const NULL_ELISION_STAGE_ORACLE_PATHS: &[&str] = &["venn1"];
const COLOR_LIMIT_STAGE_ORACLE_PATHS: &[&str] = &[
    "THEME_COLOR_LIMIT",
    "darkMode",
    "scaleLabelColor",
    "cScaleInv8",
    "cScaleInv9",
    "cScalePeer8",
    "cScalePeer9",
    "cScaleLabel8",
    "cScaleLabel9",
    "pie8",
    "pie9",
    "tagBorder",
    "primaryBorderColor",
    "tagLabelBorder",
];
const FALSY_OR_STAGE_ORACLE_PATHS: &[&str] = &["stateBkg", "primaryTextColor", "stateLabelColor"];
const REPLAY_OBJECT_STAGE_ORACLE_PATHS: &[&str] = &["radar", "xyChart"];
const GRADIENT_STAGE_ORACLE_PATHS: &[&str] = &["nodeBorder", "useGradient"];
const AGENTFLOW_STAGE_ORACLE_PATHS: &[&str] = &["secondaryBorderColor", "flowContainerStroke"];

#[derive(Debug, Clone, Copy)]
struct StageOracleSpec {
    theme: &'static str,
    id: &'static str,
    selected_paths: &'static [&'static str],
}

const STAGE_ORACLE_CASES: &[StageOracleSpec] = &[
    StageOracleSpec {
        theme: "base",
        id: "node-border-finalize",
        selected_paths: GRADIENT_STAGE_ORACLE_PATHS,
    },
    StageOracleSpec {
        theme: "base",
        id: "node-border-explicit-gradient",
        selected_paths: GRADIENT_STAGE_ORACLE_PATHS,
    },
    StageOracleSpec {
        theme: "base",
        id: "partial-objects",
        selected_paths: REPLAY_OBJECT_STAGE_ORACLE_PATHS,
    },
    StageOracleSpec {
        theme: "base",
        id: "nested-object-replay",
        selected_paths: REPLAY_OBJECT_STAGE_ORACLE_PATHS,
    },
    StageOracleSpec {
        theme: "base",
        id: "secondary-border",
        selected_paths: AGENTFLOW_STAGE_ORACLE_PATHS,
    },
    StageOracleSpec {
        theme: "dark",
        id: "falsy-container-stroke",
        selected_paths: AGENTFLOW_STAGE_ORACLE_PATHS,
    },
    StageOracleSpec {
        theme: "dark",
        id: "primary-and-derived-replay",
        selected_paths: CLASSIC_STAGE_ORACLE_PATHS,
    },
    StageOracleSpec {
        theme: "forest",
        id: "primary-and-derived-replay",
        selected_paths: CLASSIC_STAGE_ORACLE_PATHS,
    },
    StageOracleSpec {
        theme: "neutral",
        id: "primary-and-derived-replay",
        selected_paths: CLASSIC_STAGE_ORACLE_PATHS,
    },
    StageOracleSpec {
        theme: "dark",
        id: "color-limit-and-tag-replay",
        selected_paths: COLOR_LIMIT_STAGE_ORACLE_PATHS,
    },
    StageOracleSpec {
        theme: "forest",
        id: "color-limit-and-tag-replay",
        selected_paths: COLOR_LIMIT_STAGE_ORACLE_PATHS,
    },
    StageOracleSpec {
        theme: "neutral",
        id: "color-limit-and-tag-replay",
        selected_paths: COLOR_LIMIT_STAGE_ORACLE_PATHS,
    },
    StageOracleSpec {
        theme: "dark",
        id: "falsy-or-chain",
        selected_paths: FALSY_OR_STAGE_ORACLE_PATHS,
    },
    StageOracleSpec {
        theme: "dark",
        id: "operator-and-replay",
        selected_paths: OPERATOR_STAGE_ORACLE_PATHS,
    },
    StageOracleSpec {
        theme: "dark",
        id: "null-elision",
        selected_paths: NULL_ELISION_STAGE_ORACLE_PATHS,
    },
];
const THEME_STAGE_NAMES: &[&str] = &[
    "constructorPrepared",
    "overridesApplied",
    "afterUpdate",
    "explicitReplay",
    "finalResolved",
];

fn expected_stage_oracle_overrides(spec: StageOracleSpec) -> JsonValue {
    match spec.id {
        "primary-and-derived-replay" => {
            json!({ "primaryColor": "#123456", "nodeBkg": "#abcdef" })
        }
        "color-limit-and-tag-replay" => json!({
            "THEME_COLOR_LIMIT": "0x9",
            "darkMode": false,
            "scaleLabelColor": false,
            "cScaleInv8": false,
            "cScaleInv9": false,
            "cScalePeer8": false,
            "cScalePeer9": false,
            "cScaleLabel8": false,
            "cScaleLabel9": false,
            "pie8": false,
            "pie9": false,
            "tagLabelBorder": "#123456",
        }),
        "falsy-or-chain" => json!({
            "stateBkg": false,
            "primaryTextColor": false,
            "quadrant2TextFill": "#000",
            "quadrant3TextFill": "#000",
            "quadrant4TextFill": "#000",
        }),
        "operator-and-replay" => json!({
            "primaryBorderColor": "#123456",
            "gradientStart": "#abcdef",
            "pieOpacity": 0,
            "venn1": 0,
        }),
        "null-elision" => json!({ "venn1": null }),
        "node-border-finalize" => json!({ "nodeBorder": "#123456" }),
        "node-border-explicit-gradient" => json!({ "nodeBorder": "#123456", "useGradient": true }),
        "partial-objects" => {
            json!({ "radar": { "axisColor": "#123456" }, "xyChart": { "titleColor": "#abcdef" } })
        }
        "nested-object-replay" => json!({ "radar": { "axisColor": { "custom": "#123456" } } }),
        "secondary-border" => json!({ "secondaryBorderColor": "#123456" }),
        "falsy-container-stroke" => {
            json!({ "secondaryBorderColor": "#123456", "flowContainerStroke": false })
        }
        _ => unreachable!("stage oracle IDs are fixed by STAGE_ORACLE_CASES"),
    }
}

fn runtime_oracle_manifest() -> JsonValue {
    let mut oracle_cases = Vec::with_capacity(
        THEME_NAMES.len() * COMMON_ORACLE_CASE_IDS.len() + BASE_ORACLE_CASE_IDS.len(),
    );
    for theme in THEME_NAMES {
        for id in COMMON_ORACLE_CASE_IDS {
            oracle_cases.push(json!({
                "id": id,
                "theme": theme,
                "overrides": expected_oracle_overrides(theme, id),
            }));
        }
    }
    for id in BASE_ORACLE_CASE_IDS {
        oracle_cases.push(json!({
            "id": id,
            "theme": "base",
            "overrides": expected_oracle_overrides("base", id),
        }));
    }
    let stage_cases = STAGE_ORACLE_CASES
        .iter()
        .map(|spec| {
            json!({
                "id": spec.id,
                "theme": spec.theme,
                "overrides": expected_stage_oracle_overrides(*spec),
                "paths": spec.selected_paths,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "themes": THEME_NAMES,
        "selectedPaths": ORACLE_SELECTED_PATHS,
        "oracleCases": oracle_cases,
        "stageNames": THEME_STAGE_NAMES,
        "stageCases": stage_cases,
    })
}

fn expected_oracle_overrides(theme: &str, id: &str) -> JsonValue {
    match id {
        "primary-null" => json!({ "primaryColor": null }),
        "primary-empty" => json!({ "primaryColor": "" }),
        "primary-number" => json!({ "primaryColor": 17 }),
        "primary-object" => json!({ "primaryColor": { "invalid": true } }),
        "dark-and-primary" => json!({ "darkMode": true, "primaryColor": "#123456" }),
        "font-null" if theme == "base" => json!({ "fontFamily": null }),
        "font-empty" if theme == "base" => json!({ "fontFamily": "" }),
        "font-number" if theme == "base" => json!({ "fontFamily": 17 }),
        "font-size-null" if theme == "base" => json!({ "fontSize": null }),
        "scale-null" if theme == "base" => json!({ "cScale0": null }),
        "dark-null" if theme == "base" => json!({ "darkMode": null }),
        "dark-empty" if theme == "base" => json!({ "darkMode": "" }),
        "dark-number-zero" if theme == "base" => json!({ "darkMode": 0 }),
        "dark-number-one" if theme == "base" => json!({ "darkMode": 1 }),
        "dark-string" if theme == "base" => json!({ "darkMode": "false" }),
        "radar-axis-null" if theme == "base" => json!({ "radar": { "axisColor": null } }),
        "object-field-number" if theme == "base" => json!({ "radar": 17 }),
        _ => unreachable!("oracle IDs and themes are fixed by the oracle manifest"),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeThemeProjection {
    version: String,
    prepared_constructors: JsonValue,
    themes: JsonValue,
    dark_mode_true: JsonValue,
    oracle_cases: JsonValue,
    stage_oracle_cases: JsonValue,
}

#[derive(Debug)]
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
        "preparedConstructors": projection.prepared_constructors,
        "themes": projection.themes,
        "darkModeTrueOverrides": dark_mode_true_overrides,
        "oracleCaseCount": oracle_case_count,
    });
    let mut audit_artifact = json!({
        "schemaVersion": THEME_AUDIT_SCHEMA_VERSION,
        "provenance": provenance,
        "oracleCases": projection.oracle_cases,
        "stageOracleCases": projection.stage_oracle_cases,
    });
    sort_json_value_keys(&mut runtime_artifact);
    sort_json_value_keys(&mut audit_artifact);
    Ok((runtime_artifact, audit_artifact))
}

/// Exact top-level set/remove deltas against each default theme.
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
    let (runtime_out_path, audit_out_path) = match (runtime_out_path, audit_out_path) {
        (None, None) => (
            PathBuf::from(THEME_RUNTIME_OUTPUT),
            PathBuf::from(THEME_AUDIT_OUTPUT),
        ),
        (Some(runtime_out_path), None) => {
            let audit_out_path = paired_output_path(&runtime_out_path, THEME_AUDIT_OUTPUT);
            (runtime_out_path, audit_out_path)
        }
        (None, Some(audit_out_path)) => {
            let runtime_out_path = paired_output_path(&audit_out_path, THEME_RUNTIME_OUTPUT);
            (runtime_out_path, audit_out_path)
        }
        (Some(runtime_out_path), Some(audit_out_path)) => (runtime_out_path, audit_out_path),
    };
    if runtime_out_path == audit_out_path {
        return Err(XtaskError::ThemeSnapshotProjection(
            "runtime and audit artifacts require distinct output paths".to_string(),
        ));
    }
    Ok(GenerateOptions {
        reference_bundle,
        runtime_out_path,
        audit_out_path,
    })
}

fn paired_output_path(anchor: &Path, default_output: &str) -> PathBuf {
    anchor.with_file_name(
        Path::new(default_output)
            .file_name()
            .expect("generated output constants name files"),
    )
}

fn project_mermaid_runtime(
    runtime: &MermaidProjectionRuntime,
) -> Result<RuntimeThemeProjection, XtaskError> {
    let runtime_oracle_manifest = serde_json::to_string(&runtime_oracle_manifest())?;
    let output = Command::new("node")
        .arg("--input-type=module")
        .arg("-e")
        .arg(RUNTIME_THEME_PROJECTION_SCRIPT)
        .env("MERMAN_THEME_ORACLE_MANIFEST", runtime_oracle_manifest)
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
        ("preparedConstructors", &projection.prepared_constructors),
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
        if object
            .values()
            .any(|theme| theme.as_object().is_none_or(JsonMap::is_empty))
        {
            return Err(XtaskError::ThemeSnapshotProjection(format!(
                "runtime projection field `{label}` contains an empty or non-object theme"
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
        let Some(case_object) = case.as_object() else {
            return Err(XtaskError::ThemeSnapshotProjection(
                "every oracle case must be an object".to_string(),
            ));
        };
        let theme = case.get("theme").and_then(JsonValue::as_str);
        let id = case.get("id").and_then(JsonValue::as_str);
        let status = case.get("status").and_then(JsonValue::as_str);
        let (Some(theme), Some(id), Some(status)) = (theme, id, status) else {
            return Err(XtaskError::ThemeSnapshotProjection(
                "every oracle case must have string `theme`, `id`, and `status` fields".to_string(),
            ));
        };
        if !expected_cases.contains(&(theme.to_string(), id.to_string())) {
            return Err(XtaskError::ThemeSnapshotProjection(format!(
                "oracle case `{theme}/{id}` is not part of the fixed oracle manifest"
            )));
        }
        let expected_overrides = expected_oracle_overrides(theme, id);
        if case.get("overrides") != Some(&expected_overrides) {
            return Err(XtaskError::ThemeSnapshotProjection(format!(
                "oracle case `{theme}/{id}` has overrides {}, expected {expected_overrides}",
                case.get("overrides")
                    .map(JsonValue::to_string)
                    .unwrap_or_else(|| "missing".to_string())
            )));
        }
        match status {
            "ok" => {
                let expected_fields = ["id", "overrides", "selected", "status", "theme"]
                    .into_iter()
                    .collect::<BTreeSet<_>>();
                let actual_fields = case_object
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>();
                if actual_fields != expected_fields {
                    return Err(XtaskError::ThemeSnapshotProjection(format!(
                        "oracle case `{theme}/{id}` has fields {actual_fields:?}, expected {expected_fields:?}"
                    )));
                }
                let Some(selected) = case.get("selected").and_then(JsonValue::as_object) else {
                    return Err(XtaskError::ThemeSnapshotProjection(format!(
                        "oracle case `{theme}/{id}` must have an object `selected` result"
                    )));
                };
                let actual_paths = selected.keys().map(String::as_str).collect::<BTreeSet<_>>();
                let expected_paths = ORACLE_SELECTED_PATHS
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>();
                if actual_paths != expected_paths {
                    return Err(XtaskError::ThemeSnapshotProjection(format!(
                        "oracle case `{theme}/{id}` has selected paths {actual_paths:?}, expected {expected_paths:?}"
                    )));
                }
                for (path, observation) in selected {
                    validate_oracle_observation(theme, id, path, observation)?;
                }
            }
            "error" => {
                let expected_fields = ["error", "id", "overrides", "status", "theme"]
                    .into_iter()
                    .collect::<BTreeSet<_>>();
                let actual_fields = case_object
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>();
                let error = case.get("error").and_then(JsonValue::as_str);
                if actual_fields != expected_fields || error.is_none_or(str::is_empty) {
                    return Err(XtaskError::ThemeSnapshotProjection(format!(
                        "oracle case `{theme}/{id}` has an invalid error result"
                    )));
                }
            }
            _ => {
                return Err(XtaskError::ThemeSnapshotProjection(format!(
                    "oracle case `{theme}/{id}` has invalid status `{status}`"
                )));
            }
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

    let Some(stage_oracle_cases) = projection.stage_oracle_cases.as_array() else {
        return Err(XtaskError::ThemeSnapshotProjection(
            "runtime projection field `stageOracleCases` must be an array".to_string(),
        ));
    };
    let mut actual_stage_cases = BTreeSet::new();
    for case in stage_oracle_cases {
        let Some(case_object) = case.as_object() else {
            return Err(XtaskError::ThemeSnapshotProjection(
                "every stage oracle case must be an object".to_string(),
            ));
        };
        let actual_case_fields = case_object
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected_case_fields = ["id", "overrides", "stages", "theme"]
            .into_iter()
            .collect::<BTreeSet<_>>();
        if actual_case_fields != expected_case_fields {
            return Err(XtaskError::ThemeSnapshotProjection(format!(
                "stage oracle case fields are {actual_case_fields:?}, expected {expected_case_fields:?}"
            )));
        }
        let theme = case.get("theme").and_then(JsonValue::as_str);
        let id = case.get("id").and_then(JsonValue::as_str);
        let overrides = case.get("overrides").and_then(JsonValue::as_object);
        let stages = case.get("stages").and_then(JsonValue::as_object);
        let (Some(theme), Some(id), Some(_overrides), Some(stages)) =
            (theme, id, overrides, stages)
        else {
            return Err(XtaskError::ThemeSnapshotProjection(
                "every stage oracle case must have string `theme` and `id`, object `overrides`, and object `stages` fields"
                    .to_string(),
            ));
        };
        let Some(spec) = STAGE_ORACLE_CASES
            .iter()
            .find(|spec| spec.theme == theme && spec.id == id)
        else {
            return Err(XtaskError::ThemeSnapshotProjection(format!(
                "stage oracle case `{theme}/{id}` is not part of the schema-v2 contract"
            )));
        };
        let expected_overrides = expected_stage_oracle_overrides(*spec);
        if case.get("overrides") != Some(&expected_overrides) {
            return Err(XtaskError::ThemeSnapshotProjection(format!(
                "stage oracle case `{theme}/{id}` has overrides {}, expected {expected_overrides}",
                case.get("overrides").unwrap()
            )));
        }
        let actual_stages = stages.keys().map(String::as_str).collect::<BTreeSet<_>>();
        let expected_stages = THEME_STAGE_NAMES.iter().copied().collect::<BTreeSet<_>>();
        if actual_stages != expected_stages {
            return Err(XtaskError::ThemeSnapshotProjection(format!(
                "stage oracle case `{theme}/{id}` has stages {actual_stages:?}, expected {expected_stages:?}"
            )));
        }
        let expected_paths = spec.selected_paths.iter().copied().collect::<BTreeSet<_>>();
        for (stage, selected) in stages {
            let Some(selected) = selected.as_object() else {
                return Err(XtaskError::ThemeSnapshotProjection(format!(
                    "stage oracle case `{theme}/{id}` stage `{stage}` must be an object"
                )));
            };
            let actual_paths = selected.keys().map(String::as_str).collect::<BTreeSet<_>>();
            if actual_paths != expected_paths {
                return Err(XtaskError::ThemeSnapshotProjection(format!(
                    "stage oracle case `{theme}/{id}` stage `{stage}` has paths {actual_paths:?}, expected {expected_paths:?}"
                )));
            }
            for (path, observation) in selected {
                validate_stage_observation(theme, id, stage, path, observation)?;
            }
        }
        if !actual_stage_cases.insert((theme.to_string(), id.to_string())) {
            return Err(XtaskError::ThemeSnapshotProjection(format!(
                "stage oracle case `{theme}/{id}` is duplicated"
            )));
        }
    }
    let expected_stage_cases = STAGE_ORACLE_CASES
        .iter()
        .map(|spec| (spec.theme.to_string(), spec.id.to_string()))
        .collect::<BTreeSet<_>>();
    if actual_stage_cases != expected_stage_cases {
        return Err(XtaskError::ThemeSnapshotProjection(format!(
            "runtime stage oracle cases are incomplete: found {actual_stage_cases:?}, expected {expected_stage_cases:?}"
        )));
    }
    Ok(projection)
}

fn validate_oracle_observation(
    theme: &str,
    id: &str,
    path: &str,
    observation: &JsonValue,
) -> Result<(), XtaskError> {
    validate_observation(&format!("{theme}/{id}/{path}"), observation)
}

fn validate_stage_observation(
    theme: &str,
    id: &str,
    stage: &str,
    path: &str,
    observation: &JsonValue,
) -> Result<(), XtaskError> {
    validate_observation(&format!("{theme}/{id}/{stage}/{path}"), observation)
}

fn validate_observation(context: &str, observation: &JsonValue) -> Result<(), XtaskError> {
    let Some(observation) = observation.as_object() else {
        return Err(XtaskError::ThemeSnapshotProjection(format!(
            "oracle observation `{context}` must be an object"
        )));
    };
    let Some(state) = observation.get("state").and_then(JsonValue::as_str) else {
        return Err(XtaskError::ThemeSnapshotProjection(format!(
            "oracle observation `{context}` must have a string `state`"
        )));
    };
    let actual_fields = observation
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected_fields = match state {
        "missing" => ["state"].into_iter().collect::<BTreeSet<_>>(),
        "value" => ["state", "value"].into_iter().collect::<BTreeSet<_>>(),
        _ => {
            return Err(XtaskError::ThemeSnapshotProjection(format!(
                "oracle observation `{context}` has invalid state `{state}`"
            )));
        }
    };
    if actual_fields != expected_fields {
        return Err(XtaskError::ThemeSnapshotProjection(format!(
            "oracle observation `{context}` has fields {actual_fields:?}, expected {expected_fields:?}"
        )));
    }
    Ok(())
}

fn write_compact_json(path: &Path, value: &JsonValue) -> Result<(), XtaskError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
        path: parent.display().to_string(),
        source,
    })?;
    let mut output = serde_json::to_string(value)?;
    output.push('\n');
    fs::write(path, output).map_err(|source| XtaskError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

const RUNTIME_THEME_PROJECTION_SCRIPT: &str = r#"
import fs from 'node:fs';
import { randomUUID } from 'node:crypto';
import { isDeepStrictEqual } from 'node:util';
import mermaid from 'mermaid';

const packageJson = JSON.parse(fs.readFileSync('./node_modules/mermaid/package.json', 'utf8'));
const oracleManifest = JSON.parse(process.env.MERMAN_THEME_ORACLE_MANIFEST);
const themes = oracleManifest.themes;
const selectedPaths = oracleManifest.selectedPaths;
const stageNames = oracleManifest.stageNames;

const cloneJson = (value) => JSON.parse(JSON.stringify(value));
const resolve = (theme, overrides) => {
  mermaid.initialize({ theme, logLevel: 'fatal', themeVariables: overrides });
  return cloneJson(mermaid.mermaidAPI.getConfig().themeVariables);
};
const containsSentinel = (value, sentinels) => {
  if (value === null || typeof value !== 'object') {
    return false;
  }
  if (sentinels.some((sentinel) => Object.hasOwn(value, sentinel))) {
    return true;
  }
  return Object.values(value).some((child) => containsSentinel(child, sentinels));
};
const resolveStages = (theme, overrides, { directUndefined = false } = {}) => {
  if (overrides === null || Array.isArray(overrides) || typeof overrides !== 'object') {
    throw new Error(`theme ${theme} stage overrides must be a plain JSON object`);
  }
  const arrayIndexKeys = Object.keys(overrides).filter((key) => {
    const index = Number(key);
    return Number.isInteger(index) && index >= 0 && index < 2 ** 32 - 1 && String(index) === key;
  });
  if (arrayIndexKeys.length !== 0) {
    throw new Error(
      `theme ${theme} stage overrides contain array-index keys: ${arrayIndexKeys.join(',')}`,
    );
  }

  const nonce = randomUUID().replaceAll('-', '');
  const stageStartSentinel = `__merman_theme_stage_start_${nonce}__`;
  const stageEndSentinel = `__merman_theme_stage_end_${nonce}__`;
  if (Object.getOwnPropertyDescriptor(Object.prototype, stageStartSentinel) !== undefined ||
      Object.getOwnPropertyDescriptor(Object.prototype, stageEndSentinel) !== undefined) {
    throw new Error('theme stage sentinel names already exist on Object.prototype');
  }

  const events = [];
  let stageReceiver;
  const record = (marker, value, receiver) => {
    if (value !== true) {
      throw new Error(`theme ${theme} stage sentinel received a non-true value`);
    }
    if (typeof receiver?.calculate !== 'function' ||
        typeof receiver?.updateColors !== 'function') {
      throw new Error(`theme ${theme} stage sentinel reached a non-Theme receiver`);
    }
    if (stageReceiver === undefined) {
      stageReceiver = receiver;
    } else if (stageReceiver !== receiver) {
      throw new Error(`theme ${theme} stage sentinels reached multiple Theme receivers`);
    }
    events.push({
      marker,
      variables: cloneJson(receiver),
      undefinedOwnKeys: Object.keys(receiver).filter((key) => receiver[key] === undefined).sort(),
    });
  };

  let resolved;
  let installedStart = false;
  let installedEnd = false;
  try {
    Object.defineProperty(Object.prototype, stageStartSentinel, {
      configurable: true,
      set(value) { record('start', value, this); },
    });
    installedStart = true;
    Object.defineProperty(Object.prototype, stageEndSentinel, {
      configurable: true,
      set(value) { record('end', value, this); },
    });
    installedEnd = true;
    const instrumentedOverrides = {
      [stageStartSentinel]: true,
      ...overrides,
      [stageEndSentinel]: true,
    };
    mermaid.initialize({ theme, logLevel: 'fatal', themeVariables: instrumentedOverrides });
    resolved = cloneJson(mermaid.mermaidAPI.getConfig().themeVariables);
  } finally {
    const removedStart = !installedStart || Reflect.deleteProperty(Object.prototype, stageStartSentinel);
    const removedEnd = !installedEnd || Reflect.deleteProperty(Object.prototype, stageEndSentinel);
    if (!removedStart || !removedEnd ||
        Object.getOwnPropertyDescriptor(Object.prototype, stageStartSentinel) !== undefined ||
        Object.getOwnPropertyDescriptor(Object.prototype, stageEndSentinel) !== undefined) {
      throw new Error('failed to remove theme stage sentinels from Object.prototype');
    }
  }

  const markers = events.map(({ marker }) => marker);
  if (events.length !== 4 || markers.join(',') !== 'start,end,start,end') {
    throw new Error(
      `theme ${theme} produced stage markers ${markers.join(',')}; expected start,end,start,end`,
    );
  }
  for (const [index, event] of events.entries()) {
    const expectedUndefinedOwnKeys = theme === 'default' && index <= 1
      ? ['rowEven', 'rowOdd', 'scaleLabelColor']
      : [];
    if (!isDeepStrictEqual(event.undefinedOwnKeys, expectedUndefinedOwnKeys)) {
      throw new Error(
        `theme ${theme} stage ${stageNames[index]} has own undefined keys ` +
        `${event.undefinedOwnKeys.join(',')}; expected ${expectedUndefinedOwnKeys.join(',')}`,
      );
    }
  }
  const snapshots = Object.fromEntries(
    stageNames.slice(0, 4).map((stage, index) => [stage, events[index].variables]),
  );
  snapshots.finalResolved = resolved;
  const sentinels = [stageStartSentinel, stageEndSentinel];
  if (containsSentinel(snapshots, sentinels) || containsSentinel(resolved, sentinels)) {
    throw new Error(`theme ${theme} leaked a stage sentinel into generated evidence`);
  }
  const directResolved = resolve(theme, directUndefined ? undefined : overrides);
  if (!isDeepStrictEqual(resolved, directResolved)) {
    throw new Error(`theme ${theme} final stage differs from the resolved config`);
  }
  return { snapshots, resolved: directResolved };
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

const preparedConstructors = {};
const resolvedWithoutOverrides = {};
const resolvedDarkModeTrue = {};
const oracleCases = [];
const stageOracleCases = [];
for (const theme of themes) {
  const withoutOverrides = resolveStages(theme, {}, { directUndefined: true });
  preparedConstructors[theme] = withoutOverrides.snapshots.constructorPrepared;
  resolvedWithoutOverrides[theme] = withoutOverrides.resolved;
  resolvedDarkModeTrue[theme] = resolve(theme, { darkMode: true });
}

for (const { id, theme, overrides } of oracleManifest.oracleCases) {
  oracleCases.push(runCase(id, theme, overrides));
}

for (const { id, theme, overrides, paths } of oracleManifest.stageCases) {
  const { snapshots } = resolveStages(theme, overrides);
  stageOracleCases.push({
    id,
    theme,
    overrides,
    stages: Object.fromEntries(
      Object.entries(snapshots).map(([stage, variables]) => [
        stage,
        Object.fromEntries(paths.map((path) => [path, readPath(variables, path)])),
      ]),
    ),
  });
}

console.log(JSON.stringify({
  version: packageJson.version,
  preparedConstructors,
  themes: resolvedWithoutOverrides,
  darkModeTrue: resolvedDarkModeTrue,
  oracleCases,
  stageOracleCases,
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

    fn complete_projection() -> RuntimeThemeProjection {
        let themes = THEME_NAMES
            .iter()
            .map(|theme| ((*theme).to_string(), json!({ "primaryColor": "#fff" })))
            .collect::<JsonMap<_, _>>();
        let mut oracle_cases = Vec::new();
        let selected = ORACLE_SELECTED_PATHS
            .iter()
            .map(|path| ((*path).to_string(), json!({ "state": "missing" })))
            .collect::<JsonMap<_, _>>();
        for theme in THEME_NAMES {
            for id in COMMON_ORACLE_CASE_IDS {
                oracle_cases.push(json!({
                    "id": id,
                    "theme": theme,
                    "overrides": expected_oracle_overrides(theme, id),
                    "status": "ok",
                    "selected": selected.clone(),
                }));
            }
        }
        for id in BASE_ORACLE_CASE_IDS {
            oracle_cases.push(json!({
                "id": id,
                "theme": "base",
                "overrides": expected_oracle_overrides("base", id),
                "status": "ok",
                "selected": selected.clone(),
            }));
        }
        let mut stage_oracle_cases = Vec::new();
        for spec in STAGE_ORACLE_CASES {
            let overrides = expected_stage_oracle_overrides(*spec);
            let selected = spec
                .selected_paths
                .iter()
                .map(|path| ((*path).to_string(), json!({ "state": "missing" })))
                .collect::<JsonMap<_, _>>();
            let stages = THEME_STAGE_NAMES
                .iter()
                .map(|stage| ((*stage).to_string(), JsonValue::Object(selected.clone())))
                .collect::<JsonMap<_, _>>();
            stage_oracle_cases.push(json!({
                "id": spec.id,
                "theme": spec.theme,
                "overrides": overrides,
                "stages": stages,
            }));
        }
        RuntimeThemeProjection {
            version: crate::cmd::PINNED_MERMAID_VERSION.to_string(),
            prepared_constructors: JsonValue::Object(themes.clone()),
            themes: JsonValue::Object(themes.clone()),
            dark_mode_true: JsonValue::Object(themes),
            oracle_cases: JsonValue::Array(oracle_cases),
            stage_oracle_cases: JsonValue::Array(stage_oracle_cases),
        }
    }

    #[test]
    fn projection_requires_every_supported_theme_and_oracle_evidence() {
        validate_projection(complete_projection(), crate::cmd::PINNED_MERMAID_VERSION)
            .expect("complete projection is valid");
    }

    #[test]
    fn projection_rejects_stage_oracles_with_missing_selected_paths() {
        let mut projection = complete_projection();
        projection.stage_oracle_cases[0]["stages"]["afterUpdate"]
            .as_object_mut()
            .unwrap()
            .remove("nodeBorder");

        let error =
            validate_projection(projection, crate::cmd::PINNED_MERMAID_VERSION).unwrap_err();
        assert!(error.to_string().contains("has paths"));
    }

    #[test]
    fn projection_rejects_malformed_stage_observations() {
        let mut projection = complete_projection();
        projection.stage_oracle_cases[0]["stages"]["afterUpdate"]["nodeBorder"] =
            json!({ "state": "missing", "value": "must not coexist" });

        let error =
            validate_projection(projection, crate::cmd::PINNED_MERMAID_VERSION).unwrap_err();
        assert!(error.to_string().contains("has fields"));
    }

    #[test]
    fn projection_rejects_changed_stage_override_values() {
        let mut projection = complete_projection();
        projection.stage_oracle_cases[0]["overrides"]["THEME_COLOR_LIMIT"] = json!(8);

        let error =
            validate_projection(projection, crate::cmd::PINNED_MERMAID_VERSION).unwrap_err();
        assert!(error.to_string().contains("has overrides"));
    }

    #[test]
    fn projection_rejects_changed_oracle_override_values() {
        let mut projection = complete_projection();
        projection.oracle_cases[0]["overrides"]["primaryColor"] = json!("#fff");

        let error =
            validate_projection(projection, crate::cmd::PINNED_MERMAID_VERSION).unwrap_err();
        assert!(error.to_string().contains("has overrides"));
    }

    #[test]
    fn generated_artifacts_separate_runtime_data_from_test_oracles() {
        let (runtime, oracles) =
            build_theme_artifacts(complete_projection(), &MermaidProjectionRuntime::selected())
                .unwrap();

        assert_eq!(
            runtime
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            [
                "preparedConstructors",
                "provenance",
                "themes",
                "darkModeTrueOverrides",
                "oracleCaseCount",
                "schemaVersion",
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            oracles
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            [
                "oracleCases",
                "provenance",
                "schemaVersion",
                "stageOracleCases"
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(runtime["schemaVersion"], 3);
        assert_eq!(oracles["schemaVersion"], 2);
        assert_eq!(runtime["provenance"], oracles["provenance"]);
        for theme in THEME_NAMES {
            assert_eq!(
                runtime["darkModeTrueOverrides"][theme],
                json!({"set": {}, "remove": []})
            );
        }
    }

    #[test]
    fn generate_options_accept_independent_runtime_and_oracle_outputs() {
        let options = parse_generate_options(vec![
            "--out".to_string(),
            "runtime.json".to_string(),
            "--audit-out".to_string(),
            "oracles.json".to_string(),
        ])
        .expect("both output paths are valid");

        assert_eq!(options.runtime_out_path, PathBuf::from("runtime.json"));
        assert_eq!(options.audit_out_path, PathBuf::from("oracles.json"));

        let runtime_only =
            parse_generate_options(vec!["--out".to_string(), "tmp/runtime.json".to_string()])
                .expect("runtime output derives a colocated audit output");
        assert_eq!(
            runtime_only.runtime_out_path,
            PathBuf::from("tmp/runtime.json")
        );
        assert_eq!(
            runtime_only.audit_out_path,
            PathBuf::from("tmp/theme_variables_oracle_12_1_0.json")
        );

        let oracle_only = parse_generate_options(vec![
            "--audit-out".to_string(),
            "tmp/oracles.json".to_string(),
        ])
        .expect("audit output derives a colocated runtime output");
        assert_eq!(
            oracle_only.audit_out_path,
            PathBuf::from("tmp/oracles.json")
        );
        assert_eq!(
            oracle_only.runtime_out_path,
            PathBuf::from("tmp/theme_variables_12_1_0.json")
        );
    }

    #[test]
    fn generate_options_reject_output_path_collisions() {
        for args in [
            vec![
                "--out".to_string(),
                "same.json".to_string(),
                "--audit-out".to_string(),
                "same.json".to_string(),
            ],
            vec![
                "--out".to_string(),
                "tmp/theme_variables_oracle_12_1_0.json".to_string(),
            ],
            vec![
                "--audit-out".to_string(),
                "tmp/theme_variables_12_1_0.json".to_string(),
            ],
        ] {
            let error = parse_generate_options(args).expect_err("output paths must not collide");
            assert!(error.to_string().contains("distinct output paths"));
        }
    }
}

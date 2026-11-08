#![cfg(feature = "svg")]

mod support;

use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use support::run_with_stdin;

fn paired_recipe() -> Value {
    json!({
        "schema_version": 1,
        "kind": "design",
        "design": {
            "tokens": {"surface": {"type": "paint", "value": "#f8fafc"}},
            "rules": [{"target": "node", "style": {"fill": {"ref": "surface"}}}],
            "appearances": {
                "light": {},
                "dark": {
                    "tokens": {"surface": {"type": "paint", "value": "#16324a"}}
                }
            },
            "default_appearance": "dark"
        }
    })
}

fn run(directory: &Path, args: &[&str]) -> Output {
    Command::new(assert_cmd::cargo_bin!("merman-cli"))
        .current_dir(directory)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn success(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(output.stdout.ends_with(b"\n"));
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn recipe_inspection_uses_the_shared_authoring_operation_for_file_and_stdin() {
    let directory = tempfile::tempdir().unwrap();
    let recipe = paired_recipe();
    let raw = serde_json::to_vec(&recipe).unwrap();
    let path = directory.path().join("theme.json");
    fs::write(&path, &raw).unwrap();
    let file = run(directory.path(), &["theme", "inspect", "theme.json"]);
    let inspection = success(&file);
    assert_eq!(inspection["schema_version"], 1);
    assert_eq!(
        inspection["supported_appearances"],
        json!(["light", "dark"])
    );
    assert_eq!(inspection["default_appearance"], "dark");
    assert_eq!(inspection["appearances"].as_array().unwrap().len(), 2);

    let stdin = run_with_stdin(
        &["theme", "inspect", "-"],
        std::str::from_utf8(&raw).unwrap(),
    );
    assert_eq!(success(&stdin), inspection);
    assert_eq!(fs::read(&path).unwrap(), raw, "inspection is read-only");

    let policy =
        merman::svg::ThemeResourcePolicy::for_profile(merman::svg::CLI_DEFAULT_RESOURCE_PROFILE);
    let expected = merman_bindings_core::materialize_theme_design_json_with_resource_policy(
        &serde_json::to_vec(&json!({
            "schema_version": 1,
            "action": "inspect",
            "recipe": recipe
        }))
        .unwrap(),
        &policy,
    )
    .unwrap();
    assert_eq!(
        inspection,
        serde_json::from_slice::<Value>(&expected).unwrap()
    );
}

#[test]
fn concrete_recipe_inspection_does_not_infer_appearance_support() {
    for recipe in [
        json!({"schema_version": 1, "kind": "design", "design": {}}),
        json!({"schema_version": 1, "kind": "complete_spec", "complete_spec": {}}),
    ] {
        let output = run_with_stdin(
            &["theme", "inspect", "-"],
            &serde_json::to_string(&recipe).unwrap(),
        );
        let inspection = success(&output);
        assert_eq!(inspection["schema_version"], 1);
        assert_eq!(inspection["supported_appearances"], json!([]));
        assert!(inspection["default_appearance"].is_null());
    }
}

#[test]
fn inspect_rejects_invalid_inactive_appearances_and_keeps_raw_duplicates_visible() {
    let mut invalid = paired_recipe();
    invalid["design"]["appearances"]["light"]["tokens"]["surface"]["value"] = json!(true);
    let invalid = serde_json::to_string(&invalid).unwrap();
    for raw in [
        invalid.as_str(),
        r#"{"schema_version":1,"kind":"design","design":{"rules":[],"rules":[]}}"#,
        r#"{"schema_version":1,"kind":"design","design":{"appearances":{"light":{},"light":{}},"default_appearance":"light"}}"#,
        r#"{"schema_version":2,"kind":"design","design":{}}"#,
    ] {
        let output = run_with_stdin(&["theme", "inspect", "-"], raw);
        assert_eq!(output.status.code(), Some(2), "{raw}: {output:?}");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("invalid portable theme recipe"), "{stderr}");
        if raw.contains(r#""rules":[],"rules":[]"#) || raw.contains(r#""light":{},"light":{}"#) {
            assert!(stderr.contains("duplicate"), "{stderr}");
        }
    }
}

#[test]
fn inspect_explains_that_render_selection_wrappers_are_not_portable_recipes() {
    for raw in [
        r#"{"preset":"editor","appearance":"dark"}"#,
        r#"{"recipe":{"schema_version":1,"kind":"design","design":{}}}"#,
        r#"{"spec":{}}"#,
        r#"{"rules":[]}"#,
    ] {
        let output = run_with_stdin(&["theme", "inspect", "-"], raw);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("expected schema 1"), "{stderr}");
        assert!(stderr.contains("render --theme-file"), "{stderr}");
    }
}

#[test]
fn inspect_obeys_the_theme_encoded_input_budget() {
    let directory = tempfile::tempdir().unwrap();
    let policy = merman::svg::ThemeResourcePolicy::for_profile(
        merman::model::resources::ResourceProfile::Constrained,
    );
    let maximum = policy
        .value(merman::svg::ThemeResourceLimitId::MaxThemeEncodedBytes)
        .unwrap();
    fs::write(
        directory.path().join("oversized.json"),
        vec![b' '; maximum + 1],
    )
    .unwrap();
    let output = run(
        directory.path(),
        &[
            "theme",
            "inspect",
            "oversized.json",
            "--resource-profile",
            "constrained",
        ],
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("max_theme_encoded_bytes"),
        "{output:?}"
    );
}

#[test]
fn inspect_help_and_deadline_preserve_the_machine_output_contract() {
    let directory = tempfile::tempdir().unwrap();
    let help = run(directory.path(), &["theme", "inspect", "--help"]);
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("schema-1") && help.contains("stdin"));
    assert!(help.contains("render --theme-file"));
    assert!(!help.contains("--json") && !help.contains("--theme-appearance"));

    let timeout = run(
        directory.path(),
        &[
            "theme",
            "inspect",
            "missing.json",
            "--operation-timeout-ms",
            "0",
        ],
    );
    assert!(!timeout.status.success());
    assert!(timeout.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&timeout.stderr);
    assert!(stderr.to_ascii_lowercase().contains("deadline"), "{stderr}");
    assert!(!stderr.contains("doesn't exist"), "{stderr}");
}

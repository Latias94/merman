use chrono::NaiveDate;
mod common;

use common::legacy_init_theme_compat_config;
use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::family;
use regex::Regex;
use serde_json::Value as JsonValue;
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn round_f64(v: f64, decimals: u32) -> f64 {
    let p = 10_f64.powi(decimals as i32);
    (v * p).round() / p
}

fn round_json_numbers(v: &mut JsonValue, decimals: u32) {
    match v {
        JsonValue::Number(n) => {
            let Some(f) = n.as_f64() else {
                return;
            };
            let r = round_f64(f, decimals);
            if let Some(nn) = serde_json::Number::from_f64(r) {
                *v = JsonValue::Number(nn);
            }
        }
        JsonValue::Array(arr) => {
            for item in arr {
                round_json_numbers(item, decimals);
            }
        }
        JsonValue::Object(map) => {
            for (_k, val) in map.iter_mut() {
                round_json_numbers(val, decimals);
            }
        }
        _ => {}
    }
}

fn first_json_mismatch(expected: &JsonValue, actual: &JsonValue, path: &str) -> Option<String> {
    match (expected, actual) {
        (JsonValue::Array(expected), JsonValue::Array(actual)) => {
            for (index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
                let child_path = format!("{path}/{index}");
                if let Some(mismatch) = first_json_mismatch(expected, actual, &child_path) {
                    return Some(mismatch);
                }
            }
            (expected.len() != actual.len()).then(|| {
                format!(
                    "{path}: expected array length {}, actual {}",
                    expected.len(),
                    actual.len()
                )
            })
        }
        (JsonValue::Object(expected), JsonValue::Object(actual)) => {
            for (key, expected) in expected {
                let escaped_key = key.replace('~', "~0").replace('/', "~1");
                let child_path = format!("{path}/{escaped_key}");
                let Some(actual) = actual.get(key) else {
                    return Some(format!("{child_path}: missing from actual value"));
                };
                if let Some(mismatch) = first_json_mismatch(expected, actual, &child_path) {
                    return Some(mismatch);
                }
            }
            actual
                .keys()
                .find(|key| !expected.contains_key(*key))
                .map(|key| format!("{path}/{key}: unexpected in actual value"))
        }
        (JsonValue::Number(expected), JsonValue::Number(actual)) if expected != actual => {
            let delta = expected
                .as_f64()
                .zip(actual.as_f64())
                .map(|(expected, actual)| (expected - actual).abs());
            Some(match delta {
                Some(delta) => format!(
                    "{path}: expected {expected}, actual {actual}, absolute delta {delta:e}"
                ),
                None => format!("{path}: expected {expected}, actual {actual}"),
            })
        }
        _ if expected != actual => Some(format!("{path}: expected {expected}, actual {actual}")),
        _ => None,
    }
}

fn normalize_dynamic_fields(diagram_type: &str, v: &mut JsonValue) {
    // Mermaid gitGraph auto-generates commit ids using random hex suffixes.
    // Normalize these ids so snapshots are stable across runs.
    if diagram_type == "gitGraph" {
        let re = Regex::new(r"\b(\d+)-[0-9a-f]{7}\b").expect("gitGraph id regex must compile");

        fn walk(re: &Regex, v: &mut JsonValue) {
            match v {
                JsonValue::String(s) if re.is_match(s) => {
                    *s = re.replace_all(s, "$1-<dynamic>").to_string();
                }
                JsonValue::Array(arr) => {
                    for item in arr {
                        walk(re, item);
                    }
                }
                JsonValue::Object(map) => {
                    for (_k, val) in map.iter_mut() {
                        walk(re, val);
                    }
                }
                _ => {}
            }
        }

        walk(&re, v);
        return;
    }

    // Mermaid block diagram auto-generates internal ids using random base36 suffixes.
    if diagram_type == "block" {
        let re = Regex::new(r"id-[a-z0-9]+-(\d+)").expect("block id regex must compile");

        fn walk(re: &Regex, v: &mut JsonValue) {
            match v {
                JsonValue::String(s) if re.is_match(s) => {
                    *s = re.replace_all(s, "id-<id>-$1").to_string();
                }
                JsonValue::Array(arr) => {
                    for item in arr {
                        walk(re, item);
                    }
                }
                JsonValue::Object(map) => {
                    for (_k, val) in map.iter_mut() {
                        walk(re, val);
                    }
                }
                _ => {}
            }
        }

        walk(&re, v);
    }
}

fn is_parser_only_fixture(path: &Path) -> bool {
    let diagram = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str());
    let stem = path.file_stem().and_then(|name| name.to_str());
    diagram
        .zip(stem)
        .and_then(|(diagram, stem)| {
            merman_fixture_render_context::parser_only_fixture_reason(diagram, stem)
        })
        .is_some()
}

fn collect_mmd_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if dir
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|name| name.starts_with('_'))
        {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "upstream-svgs") {
                    continue;
                }
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|name| name.starts_with('_'))
                {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|e| e == "mmd") {
                if is_parser_only_fixture(&path) {
                    continue;
                }
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn parser_only_layout_exclusions_use_exact_family_facts() {
    assert!(is_parser_only_fixture(Path::new(
        "fixtures/xychart/upstream_xychart_header_only_jison_spec_parser_only_.mmd"
    )));
    assert!(!is_parser_only_fixture(Path::new(
        "fixtures/xychart/new_parser_only_spec.mmd"
    )));
    assert!(!is_parser_only_fixture(Path::new(
        "fixtures/sequence/upstream_xychart_header_only_jison_spec_parser_only_.mmd"
    )));
}

#[test]
fn fixtures_match_layout_golden_snapshots_when_present() {
    let runtime_policy = merman_core::runtime::RuntimePolicy::deterministic()
        .try_with_fixed_local_offset_minutes(0)
        .expect("valid UTC offset")
        .with_fixed_today(Some(
            NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid date"),
        ));
    let environment = merman_render::environment::RenderEnvironment::deterministic()
        .with_text_measurement_policy(
            merman_render::environment::TextMeasurementPolicy::deterministic(),
        )
        .with_runtime_policy(runtime_policy.clone());
    let fixtures_root = workspace_root().join("fixtures");
    let mmd_files = collect_mmd_files(&fixtures_root);
    assert!(
        !mmd_files.is_empty(),
        "no .mmd fixtures found under {}",
        fixtures_root.display()
    );

    // Keep time-dependent diagrams (e.g. Gantt) deterministic for fixtures.
    let engine = Engine::new()
        .with_site_config(legacy_init_theme_compat_config())
        .with_runtime_policy(runtime_policy);
    let layout_opts = LayoutOptions::default();
    let mut failures: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let golden_path = mmd_path.with_extension("layout.golden.json");
        if !golden_path.is_file() {
            continue;
        }

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(engine.parse_diagram_for_render_model(
            &text,
            ParseOptions {
                suppress_errors: true,
            },
        )) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let diagram_type = parsed.metadata().diagram_type.clone();
        let session = environment.begin_session().expect("begin render session");
        let artifact = match family::prepare(parsed, &layout_opts, session) {
            Ok(v) => v,
            Err(
                merman_render::Error::UnsupportedDiagram { .. }
                | merman_render::Error::MissingCapability { .. },
            ) => {
                continue;
            }
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let mut layout_json = match artifact.layout_json() {
            Ok(mut artifact_json) => artifact_json
                .get_mut("layout")
                .map(JsonValue::take)
                .expect("layout artifact contains layout projection"),
            Err(err) => {
                failures.push(format!(
                    "layout serialization failed for {}: {err}",
                    mmd_path.display()
                ));
                continue;
            }
        };
        round_json_numbers(&mut layout_json, 3);

        let mut actual = serde_json::json!({
            "diagramType": diagram_type,
            "layout": layout_json,
        });
        normalize_dynamic_fields(&diagram_type, &mut actual);

        let expected_text = match fs::read_to_string(&golden_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "failed to read golden {}: {err}",
                    golden_path.display()
                ));
                continue;
            }
        };

        let mut expected: JsonValue = match serde_json::from_str(&expected_text) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "failed to parse golden {}: {err}",
                    golden_path.display()
                ));
                continue;
            }
        };
        normalize_dynamic_fields(&diagram_type, &mut expected);

        if actual != expected {
            let first_mismatch = first_json_mismatch(&expected, &actual, "")
                .unwrap_or_else(|| "unable to locate unequal values".to_owned());
            failures.push(format!(
                "layout snapshot mismatch for {}\n  expected: {}\n  first mismatch: {}\n  hint: regenerate via `cargo run -p xtask -- update-layout-snapshots --filter {}`",
                mmd_path.display(),
                golden_path.display(),
                first_mismatch,
                mmd_path.file_stem().and_then(|s| s.to_str()).unwrap_or("")
            ));
        }
    }

    if !failures.is_empty() {
        panic!("{}", failures.join("\n\n"));
    }
}

#[test]
fn layout_snapshot_mismatch_diagnostic_reports_the_first_json_path() {
    let expected = serde_json::json!({"layout": {"nodes": [{"x": 1.25}]}});
    let actual = serde_json::json!({"layout": {"nodes": [{"x": 1.251}]}});

    let mismatch = first_json_mismatch(&expected, &actual, "").expect("values differ");
    assert!(mismatch.contains("/layout/nodes/0/x"));
    assert!(mismatch.contains("absolute delta"));
}

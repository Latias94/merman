use chrono::NaiveDate;
use merman_core::{Engine, ParseOptions};
use merman_render::{LayoutOptions, layout_parsed};
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

fn normalize_dynamic_fields(diagram_type: &str, v: &mut JsonValue) {
    // Mermaid gitGraph auto-generates commit ids using random hex suffixes.
    // Normalize these ids so snapshots are stable across runs.
    if diagram_type == "gitGraph" {
        let re = Regex::new(r"\b(\d+)-[0-9a-f]{7}\b").expect("gitGraph id regex must compile");

        fn walk(re: &Regex, v: &mut JsonValue) {
            match v {
                JsonValue::String(s) => {
                    if re.is_match(s) {
                        *s = re.replace_all(s, "$1-<dynamic>").to_string();
                    }
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
                JsonValue::String(s) => {
                    if re.is_match(s) {
                        *s = re.replace_all(s, "id-<id>-$1").to_string();
                    }
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

fn collect_mmd_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "upstream-svgs") {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|e| e == "mmd") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn fixtures_match_layout_golden_snapshots_when_present() {
    let fixtures_root = workspace_root().join("fixtures");
    let mmd_files = collect_mmd_files(&fixtures_root);
    assert!(
        !mmd_files.is_empty(),
        "no .mmd fixtures found under {}",
        fixtures_root.display()
    );

    // Keep time-dependent diagrams (e.g. Gantt) deterministic for fixtures.
    let engine = Engine::new().with_fixed_today(Some(
        NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid date"),
    ));
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

        let parsed = match futures::executor::block_on(engine.parse_diagram(
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

        let layouted = match layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let mut layout_json =
            serde_json::to_value(&layouted.layout).expect("serialize layout to JSON");
        round_json_numbers(&mut layout_json, 3);

        let mut actual = serde_json::json!({
            "diagramType": parsed.meta.diagram_type,
            "layout": layout_json,
        });
        normalize_dynamic_fields(&parsed.meta.diagram_type, &mut actual);

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
        normalize_dynamic_fields(&parsed.meta.diagram_type, &mut expected);

        if actual != expected {
            failures.push(format!(
                "layout snapshot mismatch for {}\n  expected: {}\n  actual:   {}\n  hint: regenerate via `cargo run -p xtask -- update-layout-snapshots --filter {}`",
                mmd_path.display(),
                golden_path.display(),
                "<computed>",
                mmd_path.file_stem().and_then(|s| s.to_str()).unwrap_or("")
            ));
        }
    }

    if !failures.is_empty() {
        panic!("{}", failures.join("\n\n"));
    }
}

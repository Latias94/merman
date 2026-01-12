use merman_core::{Engine, ParseOptions};
use regex::Regex;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

fn ms_to_local_iso(ms: i64) -> Option<String> {
    let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)?;
    Some(
        dt.with_timezone(&chrono::Local)
            .format("%Y-%m-%dT%H:%M:%S%.3f")
            .to_string(),
    )
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn fixtures_root() -> PathBuf {
    workspace_root().join("fixtures")
}

fn list_fixture_mmd_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
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

fn normalize_model(diagram_type: &str, model: &mut Value) {
    let Value::Object(obj) = model else {
        return;
    };

    obj.remove("config");

    // Mermaid mindmap includes a random UUID v4-based diagram id.
    if diagram_type == "mindmap" && obj.get("diagramId").is_some() {
        obj.insert(
            "diagramId".to_string(),
            Value::String("<dynamic>".to_string()),
        );
    }

    // Mermaid gantt uses local time; epoch millis are timezone-dependent and not portable as goldens.
    // Normalize task timestamps into local ISO strings so snapshots are stable across timezones.
    if diagram_type == "gantt" {
        let date_format = obj
            .get("dateFormat")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if matches!(date_format, "x" | "X") {
            return;
        }

        let Some(tasks) = obj.get_mut("tasks").and_then(Value::as_array_mut) else {
            return;
        };
        for task in tasks {
            let Value::Object(task_obj) = task else {
                continue;
            };
            for key in ["startTime", "endTime", "renderEndTime"] {
                let Some(v) = task_obj.get_mut(key) else {
                    continue;
                };
                let Some(ms) = v
                    .as_i64()
                    .or_else(|| v.as_u64().and_then(|n| i64::try_from(n).ok()))
                else {
                    continue;
                };
                if let Some(s) = ms_to_local_iso(ms) {
                    *v = Value::String(s);
                }
            }
        }
    }

    // Mermaid gitGraph auto-generates commit ids using random hex suffixes.
    // Normalize these ids so snapshots are stable across runs.
    if diagram_type == "gitGraph" {
        let re = Regex::new(r"\b(\d+)-[0-9a-f]{7}\b").expect("gitGraph id regex must compile");

        fn walk(re: &Regex, v: &mut Value) {
            match v {
                Value::String(s) => {
                    if re.is_match(s) {
                        *s = re.replace_all(s, "$1-<dynamic>").to_string();
                    }
                }
                Value::Array(arr) => {
                    for item in arr {
                        walk(re, item);
                    }
                }
                Value::Object(map) => {
                    for (_k, val) in map.iter_mut() {
                        walk(re, val);
                    }
                }
                _ => {}
            }
        }

        walk(&re, model);
    }
}

fn snapshot_value(diagram_type: &str, mut model: Value) -> Value {
    normalize_model(diagram_type, &mut model);

    let mut out = Map::new();
    out.insert(
        "diagramType".to_string(),
        Value::String(diagram_type.to_string()),
    );
    out.insert("model".to_string(), model);
    Value::Object(out)
}

#[test]
fn fixtures_match_golden_snapshots() {
    let fixtures = list_fixture_mmd_files(&fixtures_root());
    assert!(
        !fixtures.is_empty(),
        "no fixtures found under {}",
        fixtures_root().display()
    );

    let engine = Engine::new();
    for mmd_path in fixtures {
        let text = std::fs::read_to_string(&mmd_path)
            .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", mmd_path.display()));
        let parsed =
            futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
                .unwrap_or_else(|e| panic!("parse failed for {}: {e}", mmd_path.display()))
                .unwrap_or_else(|| panic!("no diagram detected in {}", mmd_path.display()));

        let snapshot = snapshot_value(&parsed.meta.diagram_type, parsed.model);
        let golden_path = mmd_path.with_extension("golden.json");
        let golden_text = std::fs::read_to_string(&golden_path).unwrap_or_else(|_| {
            panic!(
                "missing golden snapshot {} (generate with `cargo run -p xtask -- update-snapshots`)",
                golden_path.display()
            )
        });
        let golden: Value = serde_json::from_str(&golden_text)
            .unwrap_or_else(|e| panic!("invalid golden JSON {}: {e}", golden_path.display()));

        assert_eq!(
            snapshot,
            golden,
            "snapshot mismatch for {} (update with `cargo run -p xtask -- update-snapshots`)",
            mmd_path.display()
        );
    }
}

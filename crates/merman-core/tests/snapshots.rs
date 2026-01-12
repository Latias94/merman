use merman_core::{Engine, ParseOptions};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

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

fn snapshot_value(diagram_type: &str, mut model: Value) -> Value {
    if let Value::Object(obj) = &mut model {
        obj.remove("config");
    }
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

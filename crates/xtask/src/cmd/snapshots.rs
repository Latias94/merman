use crate::XtaskError;
use crate::util::*;
use regex::Regex;
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub(crate) fn update_layout_snapshots(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;
    let mut decimals: u32 = 3;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args
                    .get(i)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "all".to_string());
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--decimals" => {
                i += 1;
                decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
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

    fn normalize_layout_snapshot(diagram_type: &str, v: &mut JsonValue) {
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

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_root = if diagram == "all" {
        workspace_root.join("fixtures")
    } else {
        workspace_root.join("fixtures").join(&diagram)
    };

    let mut mmd_files = Vec::new();
    let mut stack = vec![fixtures_root.clone()];
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
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
                {
                    continue;
                }
                if let Some(ref f) = filter {
                    if !path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.contains(f))
                    {
                        continue;
                    }
                }
                mmd_files.push(path);
            }
        }
    }
    mmd_files.sort();
    if mmd_files.is_empty() {
        return Err(XtaskError::LayoutSnapshotUpdateFailed(format!(
            "no .mmd fixtures found under {}",
            fixtures_root.display()
        )));
    }

    let engine = merman::Engine::new()
        .with_site_config(merman::MermaidConfig::from_value(
            serde_json::json!({ "handDrawnSeed": 1 }),
        ))
        .with_fixed_today(Some(
            chrono::NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid date"),
        ))
        .with_fixed_local_offset_minutes(Some(0));
    let layout_opts = merman_render::LayoutOptions::default();
    let mut failures = Vec::new();

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(engine.parse_diagram(
            &text,
            merman::ParseOptions {
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

        if diagram != "all" {
            let dt = parsed.meta.diagram_type.as_str();
            let matches = dt == diagram
                || (diagram == "er" && matches!(dt, "er" | "erDiagram"))
                || (diagram == "flowchart" && dt == "flowchart-v2")
                || (diagram == "state" && dt == "stateDiagram")
                || (diagram == "class" && matches!(dt, "class" | "classDiagram"))
                || (diagram == "gitgraph" && dt == "gitGraph")
                || (diagram == "quadrantchart" && dt == "quadrantChart");
            if !matches {
                continue;
            }
        }

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(merman_render::Error::UnsupportedDiagram { .. }) => {
                // Layout snapshots are only defined for diagram types currently supported by
                // `merman-render::layout_parsed`. Skip unsupported diagrams so `--diagram all`
                // can be used for "all supported layout diagrams".
                continue;
            }
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let mut layout_json = match serde_json::to_value(&layouted.layout) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "failed to serialize layout JSON for {}: {err}",
                    mmd_path.display()
                ));
                continue;
            }
        };
        round_json_numbers(&mut layout_json, decimals);

        let mut out = serde_json::json!({
            "diagramType": parsed.meta.diagram_type,
            "layout": layout_json,
        });
        normalize_layout_snapshot(&parsed.meta.diagram_type, &mut out);

        let pretty = match serde_json::to_string_pretty(&out) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "failed to pretty-print JSON for {}: {err}",
                    mmd_path.display()
                ));
                continue;
            }
        };

        let out_path = mmd_path.with_extension("layout.golden.json");
        if let Some(parent) = out_path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                failures.push(format!("failed to create dir {}: {err}", parent.display()));
                continue;
            }
        }
        if let Err(err) = fs::write(&out_path, format!("{pretty}\n")) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::LayoutSnapshotUpdateFailed(failures.join("\n")))
    }
}

pub(crate) fn check_alignment(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() && !(args.len() == 1 && (args[0] == "--help" || args[0] == "-h")) {
        return Err(XtaskError::Usage);
    }
    if args.len() == 1 {
        return Err(XtaskError::Usage);
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let alignment_dir = workspace_root.join("docs").join("alignment");
    let fixtures_root = workspace_root.join("fixtures");

    let mut failures: Vec<String> = Vec::new();

    // 1) Every *_MINIMUM.md should have a *_UPSTREAM_TEST_COVERAGE.md sibling.
    let mut minimum_docs: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&alignment_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.ends_with("_MINIMUM.md") {
                minimum_docs.push(path);
            }
        }
    }
    minimum_docs.sort();
    for min_path in &minimum_docs {
        let Some(stem) = min_path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix("_MINIMUM.md"))
        else {
            continue;
        };
        let cov = alignment_dir.join(format!("{stem}_UPSTREAM_TEST_COVERAGE.md"));
        if !cov.exists() {
            failures.push(format!(
                "missing upstream coverage doc for {stem}: expected {}",
                cov.display()
            ));
        }
    }

    fn strip_reference_suffix(s: &str) -> &str {
        // Normalize "path:line" and "path#Lline" forms to just "path" for existence checks.
        if let Some((left, right)) = s.rsplit_once(':') {
            if right.chars().all(|c| c.is_ascii_digit()) {
                return left;
            }
        }
        if let Some((left, right)) = s.rsplit_once("#L") {
            if right.chars().all(|c| c.is_ascii_digit()) {
                return left;
            }
        }
        s
    }

    fn is_probably_relative_path(s: &str) -> bool {
        s.starts_with("fixtures/")
            || s.starts_with("docs/")
            || s.starts_with("crates/")
            || s.starts_with("repo-ref/")
    }

    fn contains_glob(s: &str) -> bool {
        s.contains('*') || s.contains('?') || s.contains('[') || s.contains(']')
    }

    // 2) Every `fixtures/**/*.mmd` must have a sibling `.golden.json`.
    let mut mmd_files = Vec::new();
    let mut stack = vec![fixtures_root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|e| e == "mmd") {
                mmd_files.push(path);
            }
        }
    }
    mmd_files.sort();
    for mmd in &mmd_files {
        let golden = mmd.with_extension("golden.json");
        if !golden.exists() {
            failures.push(format!(
                "missing golden snapshot for fixture {} (expected {})",
                mmd.display(),
                golden.display()
            ));
        }
    }

    // 3) Coverage docs should not reference non-existent local files.
    let backtick_re = Regex::new(r"`([^`]+)`")
        .map_err(|e| XtaskError::AlignmentCheckFailed(format!("invalid regex: {e}")))?;

    let mut coverage_docs: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&alignment_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.ends_with("_UPSTREAM_TEST_COVERAGE.md") {
                coverage_docs.push(path);
            }
        }
    }
    coverage_docs.sort();

    for cov_path in &coverage_docs {
        let text = read_text(cov_path)?;
        for caps in backtick_re.captures_iter(&text) {
            let raw = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let raw = strip_reference_suffix(raw.trim());
            if raw.is_empty() {
                continue;
            }
            if !is_probably_relative_path(raw) {
                continue;
            }
            if contains_glob(raw) {
                continue;
            }
            let path = workspace_root.join(raw);
            // `repo-ref/*` repositories are optional workspace checkouts (not committed).
            // We only require `fixtures/`, `docs/`, and `crates/` references to exist.
            if raw.starts_with("repo-ref/") && !path.exists() {
                continue;
            }
            if !path.exists() {
                failures.push(format!(
                    "broken reference in {}: `{}` does not exist",
                    cov_path.display(),
                    raw
                ));
                continue;
            }
            if raw.starts_with("fixtures/") && raw.ends_with(".mmd") {
                let golden = path.with_extension("golden.json");
                if !golden.exists() {
                    failures.push(format!(
                        "broken reference in {}: missing golden for `{}` (expected {})",
                        cov_path.display(),
                        raw,
                        golden.display()
                    ));
                }
            }
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::AlignmentCheckFailed(failures.join("\n")))
}

pub(crate) fn verify_generated(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() && !(args.len() == 1 && (args[0] == "--help" || args[0] == "-h")) {
        return Err(XtaskError::Usage);
    }
    if args.len() == 1 {
        return Err(XtaskError::Usage);
    }

    let tmp_dir = PathBuf::from("target/xtask");
    fs::create_dir_all(&tmp_dir).map_err(|source| XtaskError::WriteFile {
        path: tmp_dir.display().to_string(),
        source,
    })?;

    let mut failures = Vec::new();

    // Verify default config JSON.
    let expected_config = PathBuf::from("crates/merman-core/src/generated/default_config.json");
    let actual_config = tmp_dir.join("default_config.actual.json");
    super::gen_default_config(vec![
        "--schema".to_string(),
        "repo-ref/mermaid/packages/mermaid/src/schemas/config.schema.yaml".to_string(),
        "--out".to_string(),
        actual_config.display().to_string(),
    ])?;
    let expected_config_json: JsonValue = serde_json::from_str(&read_text(&expected_config)?)?;
    let actual_config_json: JsonValue = serde_json::from_str(&read_text(&actual_config)?)?;
    if expected_config_json != actual_config_json {
        failures.push(format!(
            "default config mismatch: regenerate with `cargo run -p xtask -- gen-default-config` ({})",
            expected_config.display()
        ));
    }

    // Verify DOMPurify allowlists.
    let expected_purify = PathBuf::from("crates/merman-core/src/generated/dompurify_defaults.rs");
    let actual_purify = tmp_dir.join("dompurify_defaults.actual.rs");
    super::gen_dompurify_defaults(vec![
        "--src".to_string(),
        "repo-ref/dompurify/dist/purify.cjs.js".to_string(),
        "--out".to_string(),
        actual_purify.display().to_string(),
    ])?;
    if read_text_normalized(&expected_purify)? != read_text_normalized(&actual_purify)? {
        failures.push(format!(
            "dompurify defaults mismatch: regenerate with `cargo run -p xtask -- gen-dompurify-defaults` ({})",
            expected_purify.display()
        ));
    }

    // Verify generated C4 type textLength table.
    let expected_c4_textlength =
        PathBuf::from("crates/merman-render/src/generated/c4_type_textlength_11_12_2.rs");
    let actual_c4_textlength = tmp_dir.join("c4_type_textlength_11_12_2.actual.rs");
    super::gen_c4_textlength(vec![
        "--in".to_string(),
        "fixtures/upstream-svgs/c4".to_string(),
        "--out".to_string(),
        actual_c4_textlength.display().to_string(),
    ])?;
    if read_text_normalized(&expected_c4_textlength)?
        != read_text_normalized(&actual_c4_textlength)?
    {
        failures.push(format!(
            "c4 textLength table mismatch: regenerate with `cargo run -p xtask -- gen-c4-textlength` ({})",
            expected_c4_textlength.display()
        ));
    }

    // Verify generated Flowchart font metrics table.
    let expected_flowchart_font_metrics =
        PathBuf::from("crates/merman-render/src/generated/font_metrics_flowchart_11_12_2.rs");
    let actual_flowchart_font_metrics = tmp_dir.join("font_metrics_flowchart_11_12_2.actual.rs");
    super::gen_font_metrics(vec![
        "--in".to_string(),
        "fixtures/upstream-svgs/flowchart".to_string(),
        "--out".to_string(),
        actual_flowchart_font_metrics.display().to_string(),
        "--font-size".to_string(),
        "16".to_string(),
    ])?;
    if read_text_normalized(&expected_flowchart_font_metrics)?
        != read_text_normalized(&actual_flowchart_font_metrics)?
    {
        failures.push(format!(
            "flowchart font metrics mismatch: regenerate with `cargo run -p xtask -- gen-font-metrics --in fixtures/upstream-svgs/flowchart --out crates/merman-render/src/generated/font_metrics_flowchart_11_12_2.rs --font-size 16` ({})",
            expected_flowchart_font_metrics.display()
        ));
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::VerifyFailed(failures.join("\n")))
}

pub(crate) fn update_snapshots(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_root = if diagram == "all" {
        workspace_root.join("fixtures")
    } else {
        workspace_root.join("fixtures").join(&diagram)
    };

    let mut mmd_files = Vec::new();
    let mut stack = vec![fixtures_root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|e| e == "mmd") {
                mmd_files.push(path);
            }
        }
    }
    mmd_files.sort();
    if let Some(f) = filter.as_deref() {
        mmd_files.retain(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
        });
    }
    if mmd_files.is_empty() {
        return Err(XtaskError::SnapshotUpdateFailed(format!(
            "no .mmd fixtures found under {}",
            fixtures_root.display()
        )));
    }

    // Pin `handDrawnSeed` so Rough.js-dependent output is deterministic and comparable to
    // `fixtures/upstream-svgs/**` (generated with Mermaid config `handDrawnSeed: 1`).
    //
    // Also pin "today" so time-dependent diagrams (notably Gantt) remain deterministic and the
    // generated snapshots match the test harness (`crates/merman-core/tests/snapshots.rs`).
    let engine = merman::Engine::new()
        .with_site_config(merman::MermaidConfig::from_value(
            serde_json::json!({ "handDrawnSeed": 1 }),
        ))
        .with_fixed_today(Some(
            chrono::NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid date"),
        ))
        .with_fixed_local_offset_minutes(Some(0));
    let mut failures = Vec::new();

    fn ms_to_local_iso(ms: i64) -> Option<String> {
        let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)?;
        Some(
            dt.with_timezone(&chrono::FixedOffset::east_opt(0).expect("UTC offset must be valid"))
                .format("%Y-%m-%dT%H:%M:%S%.3f")
                .to_string(),
        )
    }

    let re_gitgraph_id = Regex::new(r"\b(\d+)-[0-9a-f]{7}\b")
        .map_err(|e| XtaskError::SnapshotUpdateFailed(format!("invalid gitGraph id regex: {e}")))?;
    let re_block_id = Regex::new(r"id-[a-z0-9]+-(\d+)")
        .map_err(|e| XtaskError::SnapshotUpdateFailed(format!("invalid block id regex: {e}")))?;

    fn walk_replace(re: &Regex, replacement: &str, v: &mut JsonValue) {
        match v {
            JsonValue::String(s) => {
                if re.is_match(s) {
                    *s = re.replace_all(s, replacement).to_string();
                }
            }
            JsonValue::Array(arr) => {
                for item in arr {
                    walk_replace(re, replacement, item);
                }
            }
            JsonValue::Object(map) => {
                for (_k, val) in map.iter_mut() {
                    walk_replace(re, replacement, val);
                }
            }
            _ => {}
        }
    }

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(engine.parse_diagram(
            &text,
            merman::ParseOptions {
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

        if diagram != "all" {
            let dt = parsed.meta.diagram_type.as_str();
            let matches = dt == diagram
                || (diagram == "er" && matches!(dt, "er" | "erDiagram"))
                || (diagram == "flowchart" && dt == "flowchart-v2")
                || (diagram == "state" && dt == "stateDiagram")
                || (diagram == "class" && matches!(dt, "class" | "classDiagram"))
                || (diagram == "gitgraph" && dt == "gitGraph")
                || (diagram == "quadrantchart" && dt == "quadrantChart");
            if !matches {
                continue;
            }
        }

        let mut model = parsed.model;
        if let JsonValue::Object(obj) = &mut model {
            obj.remove("config");
            if parsed.meta.diagram_type == "mindmap" && obj.get("diagramId").is_some() {
                obj.insert(
                    "diagramId".to_string(),
                    JsonValue::String("<dynamic>".to_string()),
                );
            }

            if parsed.meta.diagram_type == "gantt" {
                let date_format = obj
                    .get("dateFormat")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("")
                    .trim();
                if !matches!(date_format, "x" | "X") {
                    if let Some(tasks) = obj.get_mut("tasks").and_then(JsonValue::as_array_mut) {
                        for task in tasks {
                            let JsonValue::Object(task_obj) = task else {
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
                                    *v = JsonValue::String(s);
                                }
                            }
                        }
                    }
                }
            }
        }

        if parsed.meta.diagram_type == "gitGraph" {
            walk_replace(&re_gitgraph_id, "$1-<dynamic>", &mut model);
        }

        if parsed.meta.diagram_type == "block" {
            walk_replace(&re_block_id, "id-<id>-$1", &mut model);
        }

        let out = serde_json::json!({
            "diagramType": parsed.meta.diagram_type,
            "model": model,
        });

        let pretty = match serde_json::to_string_pretty(&out) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "failed to serialize JSON for {}: {err}",
                    mmd_path.display()
                ));
                continue;
            }
        };

        let out_path = mmd_path.with_extension("golden.json");
        if let Some(parent) = out_path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                failures.push(format!("failed to create dir {}: {err}", parent.display()));
                continue;
            }
        }
        if let Err(err) = fs::write(&out_path, format!("{pretty}\n")) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::SnapshotUpdateFailed(failures.join("\n")))
}

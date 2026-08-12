use crate::XtaskError;
use crate::cmd::{MmdFixtureScan, collect_mmd_fixtures, fixtures_root_for_diagram};
use crate::util::*;
use merman_fixture_render_context::RenderContextCatalog;
use regex::Regex;
use serde_json::Value as JsonValue;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn snapshot_selector_accepts(selector: &str, diagram_type: &str) -> bool {
    // `--diagram <dir>` is a directory selector. Fixtures in that directory can still parse into
    // the error diagram under `suppress_errors=true`, and those goldens keep the fixture corpus
    // aligned with the test harness.
    if selector == "all" || diagram_type == "error" || diagram_type == selector {
        return true;
    }

    match selector {
        "er" => diagram_type == "erDiagram",
        "flowchart" => matches!(diagram_type, "flowchart-v2" | "flowchart-elk"),
        "state" => diagram_type == "stateDiagram",
        "class" => matches!(diagram_type, "class" | "classDiagram"),
        "gitgraph" => diagram_type == "gitGraph",
        "quadrantchart" => diagram_type == "quadrantChart",
        _ => false,
    }
}

fn fixture_render_context_catalog() -> &'static RenderContextCatalog {
    static CATALOG: OnceLock<RenderContextCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        RenderContextCatalog::load(crate::cmd::fixtures_root()).unwrap_or_else(|error| {
            panic!("failed to load the committed fixture render-context catalog: {error}")
        })
    })
}

pub(crate) fn fixture_site_config_for_path(path: &Path) -> Option<merman::MermaidConfig> {
    let fixtures_root = crate::cmd::fixtures_root();
    let is_committed_fixture = path.starts_with(&fixtures_root)
        || fs::canonicalize(path)
            .ok()
            .zip(fs::canonicalize(&fixtures_root).ok())
            .is_some_and(|(path, root)| path.starts_with(root));
    if !is_committed_fixture {
        return None;
    }

    fixture_render_context_catalog()
        .context_for_fixture(path)
        .unwrap_or_else(|error| {
            panic!(
                "failed to resolve fixture render context for {}: {error}",
                path.display()
            )
        })
        .map(|context| merman::MermaidConfig::from_value(context.site_config_value()))
}

fn layout_snapshot_site_config() -> merman::MermaidConfig {
    // Keep this aligned with `crates/merman-render/tests/common/mod.rs` so
    // regenerated layout goldens match the test harness exactly.
    merman::MermaidConfig::from_value(serde_json::json!({
        "secure": [
            "secure",
            "securityLevel",
            "startOnLoad",
            "maxTextSize",
            "suppressErrorRendering",
            "maxEdges"
        ]
    }))
}

fn layout_snapshot_environment() -> merman::svg::RenderEnvironment {
    merman::svg::RenderEnvironment::deterministic()
        .with_text_measurement_policy(merman::svg::TextMeasurementPolicy::deterministic())
}

pub(crate) fn update_layout_snapshots(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "all".to_string();
    let mut filter: Option<String> = None;
    let mut decimals: u32 = 3;
    let mut existing_only = false;

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
            "--existing-only" => existing_only = true,
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

    let fixtures_root = fixtures_root_for_diagram(&diagram);
    let mmd_files = collect_mmd_fixtures(
        &fixtures_root,
        MmdFixtureScan {
            filter: filter.as_deref(),
            recursive: true,
            skip_private_dirs: true,
            skip_parser_only: true,
            skip_upstream_svgs: true,
        },
    );
    if mmd_files.is_empty() {
        return Err(XtaskError::LayoutSnapshotUpdateFailed(format!(
            "no .mmd fixtures found under {}",
            fixtures_root.display()
        )));
    }

    let runtime_policy = merman::runtime::RuntimePolicy::deterministic()
        .try_with_fixed_local_offset_minutes(0)
        .expect("valid UTC offset")
        .with_fixed_today(Some(
            merman_core::time::CivilDate::new(2026, 2, 15).expect("valid date"),
        ));
    let environment = layout_snapshot_environment().with_runtime_policy(runtime_policy.clone());
    let engine = merman::Engine::new()
        .with_site_config(layout_snapshot_site_config())
        .with_runtime_policy(runtime_policy);
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

        let fixture_engine = match fixture_site_config_for_path(&mmd_path) {
            Some(site_config) => engine.clone().with_site_config(site_config),
            None => engine.clone(),
        };
        let parsed =
            match futures::executor::block_on(fixture_engine.parse_diagram_for_render_model(
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

        if !snapshot_selector_accepts(&diagram, parsed.metadata().diagram_type.as_str()) {
            continue;
        }
        let diagram_type = parsed.metadata().diagram_type.clone();

        let session = match environment.begin_session() {
            Ok(session) => session,
            Err(err) => {
                failures.push(format!("render session failed: {err}"));
                continue;
            }
        };
        let artifact = match merman_render::family::prepare(parsed, &layout_opts, session) {
            Ok(v) => v,
            Err(merman_render::Error::UnsupportedDiagram { .. }) => {
                // Layout snapshots are only defined for renderable built-in families. Skip
                // unsupported diagrams so `--diagram all` remains useful for the full corpus.
                continue;
            }
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let mut artifact_json = match artifact.layout_json() {
            Ok(value) => value,
            Err(err) => {
                failures.push(format!(
                    "failed to serialize layout JSON for {}: {err}",
                    mmd_path.display()
                ));
                continue;
            }
        };
        let Some(mut layout_json) = artifact_json
            .as_object_mut()
            .and_then(|object| object.remove("layout"))
        else {
            failures.push(format!(
                "layout artifact for {} omitted its layout projection",
                mmd_path.display()
            ));
            continue;
        };
        round_json_numbers(&mut layout_json, decimals);

        let mut out = serde_json::json!({
            "diagramType": diagram_type,
            "layout": layout_json,
        });
        normalize_layout_snapshot(&diagram_type, &mut out);

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
        if existing_only && !out_path.is_file() {
            continue;
        }
        if let Some(parent) = out_path.parent()
            && let Err(err) = fs::create_dir_all(parent)
        {
            failures.push(format!("failed to create dir {}: {err}", parent.display()));
            continue;
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

    let workspace_root = crate::cmd::workspace_root();
    let fixtures_root = crate::cmd::fixtures_root();

    let mut failures: Vec<String> = Vec::new();
    failures.extend(crate::cmd::structured_admission_alignment_failures(
        &fixtures_root,
    ));
    failures.extend(crate::cmd::committed_cypress_corpus_alignment_failures(
        &workspace_root,
    ));
    failures.extend(crate::cmd::committed_flowchart_elk_collection_failures(
        &workspace_root,
    ));

    fn is_flowchart_elk_parity_fixture(path: &Path) -> bool {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            return false;
        };
        let is_flowchart_fixture = path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            == Some("flowchart");
        is_flowchart_fixture && crate::cmd::flowchart_elk_svg_parity_admitted(stem)
    }

    // Every ordinary `fixtures/**/*.mmd` must have a sibling `.golden.json`.
    // `fixtures/_deferred/**` contains fixtures that were intentionally kept out of the alignment
    // gates (e.g. upstream CLI renders an error, or depends on unsupported options). Do not require
    // goldens for these files.
    // Flowchart ELK parity fixtures are governed by the dedicated upstream SVG parity gate instead
    // of the broad semantic golden snapshot lane.
    let mmd_files = collect_mmd_fixtures(
        &fixtures_root,
        MmdFixtureScan {
            recursive: true,
            skip_private_dirs: true,
            skip_upstream_svgs: true,
            ..MmdFixtureScan::default()
        },
    );
    for mmd in &mmd_files {
        if is_flowchart_elk_parity_fixture(mmd) {
            continue;
        }
        let golden = mmd.with_extension("golden.json");
        if !golden.exists() {
            failures.push(format!(
                "missing golden snapshot for fixture {} (expected {})",
                mmd.display(),
                golden.display()
            ));
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::AlignmentCheckFailed(failures.join("\n")))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeneratedArtifactCheck {
    BindingContract,
    CapabilitySurface,
    DefaultConfig,
    DompurifyDefaults,
    EditorLanguageContract,
    LalrpopParsers,
    NativeAbi,
    PlaygroundExampleCatalog,
    ResourceContract,
    ThemeSnapshot,
    TextMeasurementProtocol,
    TypstProfileConstants,
    WebDiagramCatalog,
}

fn verify_default_config_checks() -> [GeneratedArtifactCheck; 1] {
    [GeneratedArtifactCheck::DefaultConfig]
}

fn verify_dompurify_defaults_checks() -> [GeneratedArtifactCheck; 1] {
    [GeneratedArtifactCheck::DompurifyDefaults]
}

fn verify_web_diagram_catalog_checks() -> [GeneratedArtifactCheck; 1] {
    [GeneratedArtifactCheck::WebDiagramCatalog]
}

fn verify_theme_snapshot_checks() -> [GeneratedArtifactCheck; 1] {
    [GeneratedArtifactCheck::ThemeSnapshot]
}

fn verify_lalrpop_parsers_checks() -> [GeneratedArtifactCheck; 1] {
    [GeneratedArtifactCheck::LalrpopParsers]
}

fn verify_generated_checks() -> [GeneratedArtifactCheck; 13] {
    [
        GeneratedArtifactCheck::BindingContract,
        GeneratedArtifactCheck::CapabilitySurface,
        GeneratedArtifactCheck::DefaultConfig,
        GeneratedArtifactCheck::DompurifyDefaults,
        GeneratedArtifactCheck::EditorLanguageContract,
        GeneratedArtifactCheck::LalrpopParsers,
        GeneratedArtifactCheck::NativeAbi,
        GeneratedArtifactCheck::PlaygroundExampleCatalog,
        GeneratedArtifactCheck::ResourceContract,
        GeneratedArtifactCheck::ThemeSnapshot,
        GeneratedArtifactCheck::TextMeasurementProtocol,
        GeneratedArtifactCheck::TypstProfileConstants,
        GeneratedArtifactCheck::WebDiagramCatalog,
    ]
}

impl GeneratedArtifactCheck {
    fn label(self) -> &'static str {
        match self {
            GeneratedArtifactCheck::BindingContract => "binding contract",
            GeneratedArtifactCheck::CapabilitySurface => "capability surface",
            GeneratedArtifactCheck::DefaultConfig => "default config",
            GeneratedArtifactCheck::DompurifyDefaults => "dompurify defaults",
            GeneratedArtifactCheck::EditorLanguageContract => "editor language contract",
            GeneratedArtifactCheck::LalrpopParsers => "checked-in LALRPOP parsers",
            GeneratedArtifactCheck::NativeAbi => "native ABI",
            GeneratedArtifactCheck::PlaygroundExampleCatalog => "Playground example catalog",
            GeneratedArtifactCheck::ResourceContract => "resource contract",
            GeneratedArtifactCheck::ThemeSnapshot => "Mermaid theme snapshot",
            GeneratedArtifactCheck::TextMeasurementProtocol => "text-measurement protocol",
            GeneratedArtifactCheck::TypstProfileConstants => "Typst profile constants",
            GeneratedArtifactCheck::WebDiagramCatalog => "web diagram catalog",
        }
    }
}

fn validate_verify_generated_args(args: &[String]) -> Result<(), XtaskError> {
    if args.is_empty() {
        return Ok(());
    }
    Err(XtaskError::Usage)
}

fn verify_generated_artifact_checks(
    args: Vec<String>,
    checks: &[GeneratedArtifactCheck],
) -> Result<(), XtaskError> {
    validate_verify_generated_args(&args)?;
    let tmp_dir = PathBuf::from("target/xtask");
    fs::create_dir_all(&tmp_dir).map_err(|source| XtaskError::WriteFile {
        path: tmp_dir.display().to_string(),
        source,
    })?;

    let mut failures = Vec::new();
    let aggregate_errors = checks.len() > 1;
    for check in checks {
        match verify_generated_artifact_check(*check, &tmp_dir) {
            Ok(Some(failure)) => failures.push(failure),
            Ok(None) => {}
            Err(err) if aggregate_errors => {
                failures.push(format!("{} verification error: {err}", check.label()));
            }
            Err(err) => return Err(err),
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::VerifyFailed(failures.join("\n")))
}

fn verify_generated_artifact_check(
    check: GeneratedArtifactCheck,
    tmp_dir: &Path,
) -> Result<Option<String>, XtaskError> {
    match check {
        GeneratedArtifactCheck::BindingContract => super::verify_binding_contract_artifacts(),
        GeneratedArtifactCheck::CapabilitySurface => super::verify_capability_surface_artifacts(),
        GeneratedArtifactCheck::DefaultConfig => verify_default_config_artifact(tmp_dir),
        GeneratedArtifactCheck::DompurifyDefaults => verify_dompurify_defaults_artifact(tmp_dir),
        GeneratedArtifactCheck::EditorLanguageContract => {
            super::verify_editor_language_contract_artifacts()
        }
        GeneratedArtifactCheck::LalrpopParsers => super::verify_lalrpop_parsers_artifacts(),
        GeneratedArtifactCheck::NativeAbi => super::verify_native_abi_artifacts(),
        GeneratedArtifactCheck::PlaygroundExampleCatalog => {
            super::verify_playground_example_catalog(Vec::new()).map(|()| None)
        }
        GeneratedArtifactCheck::ResourceContract => super::verify_resource_contract_artifacts(),
        GeneratedArtifactCheck::ThemeSnapshot => verify_theme_snapshot_artifact(tmp_dir),
        GeneratedArtifactCheck::TextMeasurementProtocol => {
            super::verify_text_measurement_protocol_artifacts()
        }
        GeneratedArtifactCheck::TypstProfileConstants => {
            super::verify_typst_profile_constants_artifact()
        }
        GeneratedArtifactCheck::WebDiagramCatalog => verify_web_diagram_catalog_artifact(tmp_dir),
    }
}

fn verify_default_config_artifact(tmp_dir: &Path) -> Result<Option<String>, XtaskError> {
    let expected_config = PathBuf::from("crates/merman-core/src/generated/default_config.json");
    let expected_shape =
        PathBuf::from("crates/merman-core/src/generated/default_config_shape.json");
    let actual_config = tmp_dir.join("default_config.actual.json");
    let actual_shape = tmp_dir.join("default_config_shape.actual.json");
    super::gen_default_config(vec![
        "--out".to_string(),
        actual_config.display().to_string(),
        "--shape-out".to_string(),
        actual_shape.display().to_string(),
    ])?;
    let expected_config_json: JsonValue = serde_json::from_str(&read_text(&expected_config)?)?;
    let actual_config_json: JsonValue = serde_json::from_str(&read_text(&actual_config)?)?;
    let expected_shape_json: JsonValue = serde_json::from_str(&read_text(&expected_shape)?)?;
    let actual_shape_json: JsonValue = serde_json::from_str(&read_text(&actual_shape)?)?;
    if expected_config_json != actual_config_json || expected_shape_json != actual_shape_json {
        return Ok(Some(format!(
            "default config value/shape mismatch: regenerate with `cargo run -p xtask -- gen-default-config` ({}, {})",
            expected_config.display(),
            expected_shape.display()
        )));
    }

    Ok(None)
}

fn verify_dompurify_defaults_artifact(tmp_dir: &Path) -> Result<Option<String>, XtaskError> {
    let expected_purify = PathBuf::from("crates/merman-core/src/generated/dompurify_defaults.rs");
    let actual_purify = tmp_dir.join("dompurify_defaults.actual.rs");
    super::gen_dompurify_defaults(vec![
        "--out".to_string(),
        actual_purify.display().to_string(),
    ])?;
    if read_text_normalized(&expected_purify)? != read_text_normalized(&actual_purify)? {
        return Ok(Some(format!(
            "dompurify defaults mismatch: regenerate with `cargo run -p xtask -- gen-dompurify-defaults` ({})",
            expected_purify.display()
        )));
    }

    Ok(None)
}

fn verify_theme_snapshot_artifact(tmp_dir: &Path) -> Result<Option<String>, XtaskError> {
    let expected = PathBuf::from("crates/merman-core/src/generated/theme_variables_11_16_1.json");
    let actual = tmp_dir.join("theme_variables_11_16_1.actual.json");
    super::gen_theme_snapshot(vec!["--out".to_string(), actual.display().to_string()])?;
    let expected_json: JsonValue = serde_json::from_str(&read_text(&expected)?)?;
    let actual_json: JsonValue = serde_json::from_str(&read_text(&actual)?)?;
    if expected_json != actual_json {
        return Ok(Some(format!(
            "Mermaid theme snapshot mismatch: regenerate with `cargo run -p xtask -- gen-theme-snapshot` ({})",
            expected.display()
        )));
    }
    Ok(None)
}

fn verify_web_diagram_catalog_artifact(tmp_dir: &Path) -> Result<Option<String>, XtaskError> {
    let expected = PathBuf::from("platforms/web/src/generated/diagram-catalog.ts");
    let actual = tmp_dir.join("diagram-catalog.actual.ts");
    super::gen_web_diagram_catalog(vec!["--out".to_string(), actual.display().to_string()])?;
    if read_text_normalized(&expected)? != read_text_normalized(&actual)? {
        return Ok(Some(format!(
            "web diagram catalog mismatch: regenerate with `cargo run -p xtask -- gen-web-diagram-catalog` ({})",
            expected.display()
        )));
    }

    Ok(None)
}

pub(crate) fn verify_default_config(args: Vec<String>) -> Result<(), XtaskError> {
    let checks = verify_default_config_checks();
    verify_generated_artifact_checks(args, &checks)
}

pub(crate) fn verify_dompurify_defaults(args: Vec<String>) -> Result<(), XtaskError> {
    let checks = verify_dompurify_defaults_checks();
    verify_generated_artifact_checks(args, &checks)
}

pub(crate) fn verify_lalrpop_parsers(args: Vec<String>) -> Result<(), XtaskError> {
    let checks = verify_lalrpop_parsers_checks();
    verify_generated_artifact_checks(args, &checks)
}

pub(crate) fn verify_theme_snapshot(args: Vec<String>) -> Result<(), XtaskError> {
    let checks = verify_theme_snapshot_checks();
    verify_generated_artifact_checks(args, &checks)
}

pub(crate) fn verify_web_diagram_catalog(args: Vec<String>) -> Result<(), XtaskError> {
    let checks = verify_web_diagram_catalog_checks();
    verify_generated_artifact_checks(args, &checks)
}

pub(crate) fn verify_generated(args: Vec<String>) -> Result<(), XtaskError> {
    let checks = verify_generated_checks();
    verify_generated_artifact_checks(args, &checks)
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

    let fixtures_root = fixtures_root_for_diagram(&diagram);
    let mmd_files = collect_mmd_fixtures(
        &fixtures_root,
        MmdFixtureScan {
            filter: filter.as_deref(),
            recursive: true,
            skip_private_dirs: true,
            skip_upstream_svgs: true,
            ..MmdFixtureScan::default()
        },
    );
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
    let runtime_policy = merman::runtime::RuntimePolicy::deterministic()
        .try_with_fixed_local_offset_minutes(0)
        .expect("valid UTC offset")
        .with_fixed_today(Some(
            merman_core::time::CivilDate::new(2026, 2, 15).expect("valid date"),
        ));
    let engine = merman::Engine::new()
        .with_site_config(merman::MermaidConfig::from_value(
            serde_json::json!({ "handDrawnSeed": 1 }),
        ))
        .with_runtime_policy(runtime_policy);
    let mut failures = Vec::new();

    let re_gitgraph_id = Regex::new(r"\b(\d+)-[0-9a-f]{7}\b")
        .map_err(|e| XtaskError::SnapshotUpdateFailed(format!("invalid gitGraph id regex: {e}")))?;
    let re_block_id = Regex::new(r"id-[a-z0-9]+-(\d+)")
        .map_err(|e| XtaskError::SnapshotUpdateFailed(format!("invalid block id regex: {e}")))?;

    fn walk_replace(re: &Regex, replacement: &str, v: &mut JsonValue) {
        match v {
            JsonValue::String(s) if re.is_match(s) => {
                *s = re.replace_all(s, replacement).to_string();
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

        let fixture_engine = match fixture_site_config_for_path(&mmd_path) {
            Some(site_config) => engine.clone().with_site_config(site_config),
            None => engine.clone(),
        };

        let parsed = match futures::executor::block_on(fixture_engine.parse_diagram(
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

        if !snapshot_selector_accepts(&diagram, parsed.meta.diagram_type.as_str()) {
            continue;
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
                if !matches!(date_format, "x" | "X")
                    && let Some(tasks) = obj.get_mut("tasks").and_then(JsonValue::as_array_mut)
                {
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
                            *v = JsonValue::String(crate::cmd::timestamps::unix_millis_to_utc_iso(
                                ms,
                            ));
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
        if let Some(parent) = out_path.parent()
            && let Err(err) = fs::create_dir_all(parent)
        {
            failures.push(format!("failed to create dir {}: {err}", parent.display()));
            continue;
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

#[cfg(test)]
mod tests {
    use super::{
        GeneratedArtifactCheck, MmdFixtureScan, collect_mmd_fixtures,
        fixture_render_context_catalog, fixture_site_config_for_path, layout_snapshot_environment,
        snapshot_selector_accepts, validate_verify_generated_args, verify_default_config_checks,
        verify_dompurify_defaults_checks, verify_generated_checks, verify_lalrpop_parsers_checks,
        verify_theme_snapshot_checks, verify_web_diagram_catalog_checks,
    };
    use crate::cmd::is_parser_only_fixture;
    use crate::cmd::workspace_root;

    #[test]
    fn generated_artifact_verify_commands_have_expected_scope() {
        assert_eq!(
            verify_default_config_checks(),
            [GeneratedArtifactCheck::DefaultConfig]
        );
        assert_eq!(
            verify_dompurify_defaults_checks(),
            [GeneratedArtifactCheck::DompurifyDefaults]
        );
        assert_eq!(
            verify_generated_checks(),
            [
                GeneratedArtifactCheck::BindingContract,
                GeneratedArtifactCheck::CapabilitySurface,
                GeneratedArtifactCheck::DefaultConfig,
                GeneratedArtifactCheck::DompurifyDefaults,
                GeneratedArtifactCheck::EditorLanguageContract,
                GeneratedArtifactCheck::LalrpopParsers,
                GeneratedArtifactCheck::NativeAbi,
                GeneratedArtifactCheck::PlaygroundExampleCatalog,
                GeneratedArtifactCheck::ResourceContract,
                GeneratedArtifactCheck::ThemeSnapshot,
                GeneratedArtifactCheck::TextMeasurementProtocol,
                GeneratedArtifactCheck::TypstProfileConstants,
                GeneratedArtifactCheck::WebDiagramCatalog,
            ]
        );
        assert_eq!(
            verify_lalrpop_parsers_checks(),
            [GeneratedArtifactCheck::LalrpopParsers]
        );
        assert_eq!(
            GeneratedArtifactCheck::BindingContract.label(),
            "binding contract"
        );
        assert_eq!(
            GeneratedArtifactCheck::CapabilitySurface.label(),
            "capability surface"
        );
        assert_eq!(
            GeneratedArtifactCheck::DefaultConfig.label(),
            "default config"
        );
        assert_eq!(
            GeneratedArtifactCheck::DompurifyDefaults.label(),
            "dompurify defaults"
        );
        assert_eq!(
            GeneratedArtifactCheck::EditorLanguageContract.label(),
            "editor language contract"
        );
        assert_eq!(
            GeneratedArtifactCheck::LalrpopParsers.label(),
            "checked-in LALRPOP parsers"
        );
        assert_eq!(GeneratedArtifactCheck::NativeAbi.label(), "native ABI");
        assert_eq!(
            GeneratedArtifactCheck::PlaygroundExampleCatalog.label(),
            "Playground example catalog"
        );
        assert_eq!(
            GeneratedArtifactCheck::ResourceContract.label(),
            "resource contract"
        );
        assert_eq!(
            verify_theme_snapshot_checks(),
            [GeneratedArtifactCheck::ThemeSnapshot]
        );
        assert_eq!(
            GeneratedArtifactCheck::ThemeSnapshot.label(),
            "Mermaid theme snapshot"
        );
        assert_eq!(
            GeneratedArtifactCheck::TextMeasurementProtocol.label(),
            "text-measurement protocol"
        );
        assert_eq!(
            GeneratedArtifactCheck::TypstProfileConstants.label(),
            "Typst profile constants"
        );
        assert_eq!(
            verify_web_diagram_catalog_checks(),
            [GeneratedArtifactCheck::WebDiagramCatalog]
        );
        assert_eq!(
            GeneratedArtifactCheck::WebDiagramCatalog.label(),
            "web diagram catalog"
        );
    }

    #[test]
    fn generated_artifact_verify_commands_reject_args() {
        assert!(validate_verify_generated_args(&[]).is_ok());
        assert!(validate_verify_generated_args(&["--help".to_string()]).is_err());
        assert!(validate_verify_generated_args(&["unexpected".to_string()]).is_err());
    }

    #[test]
    fn collect_mmd_fixtures_honors_snapshot_scan_policy() {
        let fixtures_root = workspace_root().join("fixtures");
        let files = collect_mmd_fixtures(
            &fixtures_root,
            MmdFixtureScan {
                recursive: true,
                skip_private_dirs: true,
                skip_upstream_svgs: true,
                ..MmdFixtureScan::default()
            },
        );

        assert!(
            !files.is_empty(),
            "expected fixtures under {}",
            fixtures_root.display()
        );
        assert!(
            files.iter().all(|path| {
                path.components()
                    .filter_map(|component| component.as_os_str().to_str())
                    .all(|component| component != "_deferred" && component != "upstream-svgs")
            }),
            "snapshot scans must not enter private fixture dirs or upstream SVG baselines"
        );
        assert!(
            files.iter().any(|path| is_parser_only_fixture(path)),
            "semantic snapshot scans keep parser-only fixtures"
        );
    }

    #[test]
    fn collect_mmd_fixtures_can_skip_parser_only_fixtures() {
        let fixtures_root = workspace_root().join("fixtures");
        let files = collect_mmd_fixtures(
            &fixtures_root,
            MmdFixtureScan {
                skip_private_dirs: true,
                skip_parser_only: true,
                recursive: true,
                skip_upstream_svgs: true,
                ..MmdFixtureScan::default()
            },
        );

        assert!(
            !files.iter().any(|path| is_parser_only_fixture(path)),
            "layout snapshot scans must skip parser-only fixtures"
        );
    }

    #[test]
    fn collect_mmd_fixtures_filters_by_file_name() {
        let fixtures_root = workspace_root().join("fixtures");
        let files = collect_mmd_fixtures(
            &fixtures_root,
            MmdFixtureScan {
                filter: Some("upstream_sankey_allows_proto_id_sankey_header_parser_only_spec"),
                recursive: true,
                skip_private_dirs: true,
                skip_upstream_svgs: true,
                ..MmdFixtureScan::default()
            },
        );

        assert_eq!(files.len(), 1);
        assert!(files[0].file_name().and_then(|n| n.to_str()).is_some_and(
            |name| name == "upstream_sankey_allows_proto_id_sankey_header_parser_only_spec.mmd"
        ));
    }

    #[test]
    fn fixture_render_context_catalog_projects_site_config_for_every_entry() {
        let fixtures_root = workspace_root().join("fixtures");
        let contexts = fixture_render_context_catalog()
            .contexts()
            .collect::<Vec<_>>();
        assert!(!contexts.is_empty(), "expected committed render contexts");

        for context in contexts {
            let path = fixtures_root.join(context.fixture());
            let config = fixture_site_config_for_path(&path)
                .unwrap_or_else(|| panic!("missing render context for {}", path.display()));
            assert_eq!(
                config.get_str("securityLevel"),
                Some(context.security_level().as_str()),
                "site config for {}",
                context.fixture()
            );
        }
    }

    #[test]
    fn snapshot_selector_accepts_directory_aliases() {
        let cases = [
            ("all", "flowchart-v2", true),
            ("er", "erDiagram", true),
            ("er", "er", true),
            ("flowchart", "flowchart-v2", true),
            ("flowchart", "flowchart-elk", true),
            ("state", "stateDiagram", true),
            ("class", "classDiagram", true),
            ("class", "class", true),
            ("gitgraph", "gitGraph", true),
            ("quadrantchart", "quadrantChart", true),
            ("state", "classDiagram", false),
        ];

        for (selector, diagram_type, expected) in cases {
            assert_eq!(
                snapshot_selector_accepts(selector, diagram_type),
                expected,
                "{selector} should accept {diagram_type}: {expected}"
            );
        }
    }

    #[test]
    fn snapshot_selector_keeps_error_diagrams_for_scoped_runs() {
        assert!(snapshot_selector_accepts("class", "error"));
    }

    #[test]
    fn layout_snapshot_generation_uses_the_verifier_measurement_profile() {
        let session = layout_snapshot_environment()
            .begin_session()
            .expect("begin layout snapshot render session");
        let route = session.text_measurement_route(merman::svg::TextMeasurementPhase::Layout);

        assert_eq!(
            route.primary.profile().as_str(),
            "merman.deterministic-text"
        );
        assert_eq!(route.fallback, None);
    }
}
